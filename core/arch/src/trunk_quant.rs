//! Convert the always-read trunk to a narrower dtype at load time — **C7**.
//!
//! # The one lever left, and what it actually buys
//!
//! DeepSeek-V4-Flash's container is mixed precision, and the mix runs the wrong
//! way for a machine like this one. Its 256 routed experts — 137 GiB, streamed
//! from disk and never resident — are `MXFP4` at 0.53 bytes a weight. Its
//! **always-read trunk**, the 7.38 GiB that must stay in RAM for the whole
//! session, is `Q8_0` at 1.06. The set that has to fit is stored at twice the
//! width of the set that does not.
//!
//! `the-disk-is-no-longer-the-ceiling-2026-09-01` measured why that matters, and
//! it is **not** arithmetic. A generated token is 0.815 s of expert reads and
//! 0.478 s of compute, and the expert cache — the only thing that removes disk
//! traffic — is sized from what residency leaves behind:
//! `clamp(total - resident - 5 GiB, 0, 6 GiB)`, which on 15.7 GiB against a
//! 7.38 GiB trunk is 3.34 GiB. The measured cache curve has a **cliff**: 3 GiB
//! returned 0.721 tok/s and 6 GiB returned 0.352, because 7.38 + 6 of 15.7
//! leaves nothing for the page cache. Halve the trunk and the same 6 GiB leaves
//! 5.8 GiB free — the cliff moves, and the hit rate that was measured at 33.1%
//! becomes reachable.
//!
//! **So this is a memory lever, not a speed lever.** `trunk_mat_vec_dtypes`
//! measures the trunk's own mat-vec at `Q8_0` 0.211–0.252 ms against `Q4_K`
//! 0.196–0.228 across four runs in one session: the ranges overlap, so the
//! arithmetic is unchanged and no compute win is claimed here.
//!
//! # It went through the gate and the gate refused it
//!
//! C5e and the expert-cache default were *exact* — byte-identical output, 50 of
//! 50. This changes what the model computes, so it faced the **lossy** bar agreed
//! with Atur: ≥95% identical completions, no checkable answer lost, perplexity
//! within +1%.
//!
//! **`q4_k` returned 20 of 50 byte-identical, 40.0%.** That is a refusal, not a
//! near miss, and **no tok/s figure from this lever may be quoted** — the gate's
//! own words. It stays in the tree off by default and documented as failing, on
//! the same footing as `--op-offload` (measured 19% slower here, shipped off): a
//! measured-bad option that says so is more use than a deleted branch nobody can
//! re-check.
//!
//! One result inside the failure is worth knowing: **checkable answers came back
//! 41 of 50, exactly the baseline's 41.** Not one factual answer was lost, so the
//! conversion changes how the model words things rather than what it knows. That
//! is a property of a K-quant trunk and not a defence of the lever.
//!
//! `../../../docs/graph/research/requantising-the-trunk-2026-09-02.md` has the
//! whole record, including the three measurement rounds and why the first two
//! could not be trusted.
//!
//! # What it converts, and the two things it will not touch
//!
//! Every resident tensor that is stored `Q8_0` and whose rows are a whole number
//! of the target's blocks. That is the attention projections, the shared expert,
//! and the token embedding. It deliberately leaves:
//!
//! - **`output.weight`** — the projection to 129,280 logits. Its error lands
//!   directly on the sampled token rather than being averaged over a residual
//!   stream, and llama.cpp's own `Q4_K_M` keeps this tensor at `Q6_K` for that
//!   reason. It is 0.52 GiB of the 6.6 GiB available, so the insurance is cheap.
//! - **the router (`ffn_gate_inp`)** — `BF16`, so the type filter already skips
//!   it. Said out loud because it is the tensor that must not move: an error
//!   there changes *which experts are read*, and a routing change is not a small
//!   perturbation of the answer but a different answer.
//!
//! Norms, sinks and the hyper-connection matrices are `F32` and are likewise
//! skipped by the type filter — they are 0.17 GiB in total and quantising a norm
//! is a known way to break a model.
//!
//! # On the CLI only, like `--force`
//!
//! `chaos-serve` loads a resident set too and is deliberately not given this
//! flag. A lossy transform behind an HTTP endpoint changes every answer the node
//! gives, for everyone using it, with nothing in the response saying so — which
//! is the same reason `--force` (run an unverified architecture) exists on the
//! runner and not on the server. Someone who wants a narrower trunk on a server
//! can quantise the container, which is llama.cpp's job and it does it well.

use std::fmt;
use std::sync::Arc;

use chaos_gguf::GgmlType;
use chaos_io::SkewedBuf;
use chaos_model::{Model, ResidentSet};

use crate::Result;

/// `GGML_TYPE_Q8_0` — the only source type this converts from.
///
/// Not "any quantised type": going from a 4-bit type to another 4-bit type is
/// loss for nothing, and the `F32`/`BF16` tensors in this container are norms
/// and routers, which must not move at all.
const Q8_0: u32 = 8;

/// The name of the tensor whose error reaches the sampled token directly.
const OUTPUT_PROJECTION: &str = "output.weight";

/// Rows converted per `dequantize` call.
///
/// The conversion goes through `f32`, and a whole tensor as `f32` does not fit:
/// `output.weight` is 529 M weights, which is 2.1 GiB of floats on a machine
/// that has already spent 7.38 GiB on the trunk. Rows are the natural unit
/// because a K-quant's scales never span a row boundary, so a band of rows
/// converts exactly as the whole tensor would.
const CHUNK_ROWS: usize = 128;

/// What a requantisation pass did.
#[derive(Debug, Clone, Default)]
pub struct TrunkQuantReport {
    pub converted: usize,
    pub skipped_type: usize,
    pub skipped_shape: usize,
    pub skipped_by_name: usize,
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub seconds: f64,
    pub target: Option<GgmlType>,
}

impl TrunkQuantReport {
    /// Bytes the conversion gave back to the rest of the machine.
    pub fn saved(&self) -> u64 {
        self.bytes_before.saturating_sub(self.bytes_after)
    }
}

impl fmt::Display for TrunkQuantReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const GIB: f64 = (1u64 << 30) as f64;
        let target = self
            .target
            .and_then(|t| t.name())
            .unwrap_or("a narrower type");
        write!(
            f,
            "{} tensors Q8_0 -> {target}, {:.2} -> {:.2} GiB, {:.2} GiB freed in {:.1}s",
            self.converted,
            self.bytes_before as f64 / GIB,
            self.bytes_after as f64 / GIB,
            self.saved() as f64 / GIB,
            self.seconds
        )
    }
}

/// The dtypes this will convert *to*, by the name a person types.
///
/// Deliberately short. `Q4_K` is the lever's point, `Q5_K` and `Q6_K` are the
/// two steps back towards `Q8_0` for a machine where quality matters more than
/// the last gigabyte. Everything else — the `IQ*` types especially — needs an
/// importance matrix this build does not compute, and `chaos_ggml::quantize`
/// refuses them by name rather than producing bad bytes quietly.
///
/// `Some(None)` is "leave the trunk alone", which is a real answer and not a
/// parse failure — `None` is the parse failure.
pub fn target_from_name(name: &str) -> Option<Option<GgmlType>> {
    match name.to_ascii_lowercase().as_str() {
        "off" | "none" | "q8_0" => Some(None),
        "q4_k" | "q4k" => Some(Some(GgmlType(12))),
        "q5_k" | "q5k" => Some(Some(GgmlType(13))),
        "q6_k" | "q6k" => Some(Some(GgmlType(14))),
        _ => None,
    }
}

/// The names `--trunk-quant` accepts, for an error message.
pub const TRUNK_QUANT_NAMES: &str = "off, q4_k, q5_k, q6_k";

/// Should this tensor be left exactly as the container stores it?
fn keep_as_stored(name: &str) -> bool {
    name == OUTPUT_PROJECTION
}

/// Convert every eligible resident tensor to `target`, in place.
///
/// Each tensor is taken out of the set, converted, and handed back smaller, so
/// the peak is one tensor's bands rather than a second copy of the trunk — the
/// same exchange weight repacking makes, and for the same reason.
///
/// On any failure the original bytes go back into the set before the error
/// propagates: leaving a trunk tensor absent would make it stream from disk on
/// every token forever, which is far worse than the conversion not happening.
pub fn requantise(
    resident: &mut ResidentSet,
    model: &Model,
    target: GgmlType,
    threads: usize,
) -> Result<TrunkQuantReport> {
    let start = std::time::Instant::now();
    let mut report = TrunkQuantReport {
        target: Some(target),
        ..Default::default()
    };
    let threads = threads.max(1);

    let mut names = resident.names();
    // Largest first, and deterministic, so two runs convert in the same order.
    names.sort_by_key(|n| {
        let size = model.location(n).map(|l| l.size).unwrap_or(0);
        (std::cmp::Reverse(size), n.clone())
    });

    for name in names {
        if keep_as_stored(&name) {
            report.skipped_by_name += 1;
            continue;
        }
        let Some(loc) = model.location(&name) else {
            continue;
        };
        // The container's type is the right question here: nothing has been
        // converted yet, so an override already present means this pass ran
        // twice, and converting a K-quant again would be loss for nothing.
        if loc.ty.0 != Q8_0 || resident.type_of(&name).is_some() {
            report.skipped_type += 1;
            continue;
        }
        let (ne0, nrows) = match loc.dims.as_slice() {
            [a] => (*a as i64, 1i64),
            [a, rest @ ..] => (*a as i64, rest.iter().product::<u64>() as i64),
            [] => continue,
        };
        // A row that is not a whole number of the target's blocks cannot be
        // converted at all, and padding it would change the tensor's shape.
        let (Ok(src_row), Ok(dst_row)) = (
            chaos_ggml::row_size(loc.ty, ne0),
            chaos_ggml::row_size(target, ne0),
        ) else {
            report.skipped_shape += 1;
            continue;
        };
        if dst_row >= src_row {
            // Converting upwards would spend RAM to lose accuracy.
            report.skipped_shape += 1;
            continue;
        }

        let Some(original) = resident.take(&name) else {
            continue;
        };
        if original.len() != src_row * nrows as usize {
            // The container's index and the bytes disagree. Put it back and
            // leave it alone rather than convert something unexplained.
            report.skipped_shape += 1;
            resident.put_back(name, original);
            continue;
        }

        match convert(loc.ty, target, ne0, nrows, &original[..], threads) {
            Ok(smaller) => {
                report.converted += 1;
                report.bytes_before += original.len() as u64;
                report.bytes_after += smaller.len() as u64;
                drop(original);
                resident.replace(name, target, Arc::new(smaller));
            }
            Err(e) => {
                resident.put_back(name, original);
                return Err(e);
            }
        }
    }

    report.seconds = start.elapsed().as_secs_f64();
    Ok(report)
}

/// Convert one tensor, `threads` bands of rows at a time.
fn convert(
    from: GgmlType,
    to: GgmlType,
    ne0: i64,
    nrows: i64,
    src: &[u8],
    threads: usize,
) -> Result<SkewedBuf> {
    let src_row = chaos_ggml::row_size(from, ne0)?;
    let dst_row = chaos_ggml::row_size(to, ne0)?;
    let rows = nrows.max(0) as usize;
    // Skew 0: this buffer is never read into from a drive, and ggml wants
    // 32-byte alignment, which `SkewedBuf` exceeds.
    let mut out = SkewedBuf::new(dst_row * rows, 0);

    {
        let mut rest: &mut [u8] = &mut out;
        let band = rows.div_ceil(threads).max(1);
        std::thread::scope(|scope| -> Result<()> {
            let mut handles = Vec::new();
            let mut row0 = 0usize;
            while row0 < rows {
                let n = band.min(rows - row0);
                let (mine, tail) = rest.split_at_mut(n * dst_row);
                rest = tail;
                let band_src = &src[row0 * src_row..(row0 + n) * src_row];
                handles.push(scope.spawn(move || convert_band(from, to, ne0, n, band_src, mine)));
                row0 += n;
            }
            for h in handles {
                match h.join() {
                    Ok(r) => r?,
                    // A panic inside a quantiser is a bug, not a condition to
                    // paper over: re-raise it with its own payload.
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            }
            Ok(())
        })?;
    }
    Ok(out)
}

/// Convert `rows` rows, in chunks small enough that the `f32` intermediate is
/// measured in megabytes.
fn convert_band(
    from: GgmlType,
    to: GgmlType,
    ne0: i64,
    rows: usize,
    src: &[u8],
    dst: &mut [u8],
) -> Result<()> {
    let src_row = chaos_ggml::row_size(from, ne0)?;
    let dst_row = chaos_ggml::row_size(to, ne0)?;
    let mut row0 = 0usize;
    while row0 < rows {
        let n = CHUNK_ROWS.min(rows - row0);
        let floats = chaos_ggml::dequantize(
            from,
            &src[row0 * src_row..(row0 + n) * src_row],
            n * ne0 as usize,
        )?;
        chaos_ggml::quantize(
            to,
            &floats,
            n as i64,
            ne0,
            &mut dst[row0 * dst_row..(row0 + n) * dst_row],
        )?;
        row0 += n;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_a_person_can_type() {
        assert_eq!(target_from_name("off"), Some(None));
        assert_eq!(target_from_name("q8_0"), Some(None));
        assert_eq!(target_from_name("Q4_K"), Some(Some(GgmlType(12))));
        assert_eq!(target_from_name("q4k"), Some(Some(GgmlType(12))));
        assert_eq!(target_from_name("q5_k"), Some(Some(GgmlType(13))));
        assert_eq!(target_from_name("q6_k"), Some(Some(GgmlType(14))));
        // Refused rather than guessed: an importance-matrix type asked for here
        // would be silently bad, which is the failure mode this file is about.
        assert_eq!(target_from_name("iq2_xxs"), None);
        assert_eq!(target_from_name("q4_0"), None);
        assert_eq!(target_from_name(""), None);
    }

    #[test]
    fn the_logit_projection_is_left_alone() {
        assert!(keep_as_stored("output.weight"));
        assert!(!keep_as_stored("token_embd.weight"));
        assert!(!keep_as_stored("blk.0.attn_q_b.weight"));
        assert!(!keep_as_stored("blk.7.ffn_gate_shexp.weight"));
    }

    #[test]
    fn converting_a_tensor_halves_it_and_keeps_its_values() {
        // A row of a real weight matrix is smooth, not noise. The bound is loose
        // on purpose — what is asserted is that the conversion is a conversion
        // and not a corruption; how much accuracy Q4_K costs is measured in
        // `chaos-ggml`'s own round-trip test.
        const NE0: i64 = 1024;
        const ROWS: i64 = 6;
        let src: Vec<f32> = (0..NE0 * ROWS)
            .map(|i| ((i as f32) * 0.013).sin() * 0.4)
            .collect();

        let q8 = GgmlType(Q8_0);
        let q4k = GgmlType(12);
        let mut stored = vec![0u8; chaos_ggml::row_size(q8, NE0).unwrap() * ROWS as usize];
        chaos_ggml::quantize(q8, &src, ROWS, NE0, &mut stored).expect("store as Q8_0");

        let smaller = convert(q8, q4k, NE0, ROWS, &stored, 3).expect("convert");
        assert_eq!(
            smaller.len(),
            chaos_ggml::row_size(q4k, NE0).unwrap() * ROWS as usize
        );
        assert!(
            (smaller.len() as f64) < 0.6 * stored.len() as f64,
            "Q4_K should be well under 60% of Q8_0: {} vs {}",
            smaller.len(),
            stored.len()
        );

        let back = chaos_ggml::dequantize(q4k, &smaller[..], src.len()).expect("dequantize");
        let rms = (src
            .iter()
            .zip(&back)
            .map(|(a, b)| ((a - b) as f64).powi(2))
            .sum::<f64>()
            / src.len() as f64)
            .sqrt();
        assert!(rms < 0.02, "rms {rms} after a round trip through Q4_K");
    }

    #[test]
    fn every_band_is_converted_whatever_the_thread_count() {
        // Bands are split by rows and written into disjoint slices. If that
        // arithmetic were off by a row, one thread count would leave a band of
        // zeros — which reads as a plausible weight matrix.
        const NE0: i64 = 256;
        const ROWS: i64 = 7;
        let src: Vec<f32> = (0..NE0 * ROWS)
            .map(|i| 0.25 + (i % 11) as f32 * 0.01)
            .collect();
        let q8 = GgmlType(Q8_0);
        let q4k = GgmlType(12);
        let mut stored = vec![0u8; chaos_ggml::row_size(q8, NE0).unwrap() * ROWS as usize];
        chaos_ggml::quantize(q8, &src, ROWS, NE0, &mut stored).expect("store");

        let one = convert(q8, q4k, NE0, ROWS, &stored, 1).expect("one thread");
        for t in [2usize, 3, 7, 16] {
            let many = convert(q8, q4k, NE0, ROWS, &stored, t).expect("many threads");
            assert_eq!(&one[..], &many[..], "thread count {t} changed the bytes");
        }
    }
}
