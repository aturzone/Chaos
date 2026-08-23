//! The experts a worker holds, and the arithmetic it runs on them.
//!
//! # What a worker is
//!
//! `chaos-run` without a token loop. It reads an assigned set of experts off
//! disk **once**, at startup, into RAM — and from then on it is memory that
//! answers, which is the entire point. The main device keeps routing, sampling
//! and the KV cache, so nothing here is per-token state.
//!
//! # Why the same kernels
//!
//! The FFN below is the one `deepseek4_forward` runs locally, op for op:
//! `mul_mat_id` for gate and up, a clamp on each, `swiglu_split`, then
//! `mul_mat_id` for down. **Not a reimplementation.** On a forward pass a
//! second implementation is not a maintenance cost, it is a silent wrongness
//! waiting to happen — the failure mode is fluent nonsense, never a crash, and
//! `tests/parity.rs` is what stands between this and that.
//!
//! # Why the weights are re-bound per request
//!
//! Binding is zero-copy: `WeightSet::bind` hands ggml a pointer into the bytes
//! this module already holds. So a fresh context per request costs a few
//! microseconds of bookkeeping and no memory, and it avoids a self-referential
//! struct holding a context that borrows from itself. The streaming path
//! already works this way, a `WeightSet` per block.

use chaos_ggml::{Context, WeightSet};
use chaos_gguf::GgmlType;
use chaos_model::Model;
use std::sync::Arc;

/// The three stacked tensors an MoE layer's experts live in.
///
/// One tensor per role, each holding every expert end to end — which is why a
/// slice is `size / n_expert` and expert `e` starts at `e * slice`.
const ROLES: [&str; 3] = ["ffn_gate_exps", "ffn_up_exps", "ffn_down_exps"];

#[derive(Debug)]
pub enum Error {
    Model(String),
    Ggml(String),
    /// Asked for a layer or an expert this worker does not hold.
    NotHeld {
        layer: u32,
        expert: u32,
    },
    /// The hidden state is not the width these weights expect.
    Width {
        want: u32,
        got: u32,
    },
    /// This container has no routed experts at all.
    NotMoe,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Model(m) => write!(f, "{m}"),
            Error::Ggml(m) => write!(f, "{m}"),
            Error::NotHeld { layer, expert } => {
                write!(
                    f,
                    "this worker does not hold expert {expert} of layer {layer}"
                )
            }
            Error::Width { want, got } => write!(
                f,
                "hidden state is {got} wide, these weights are {want} wide"
            ),
            Error::NotMoe => write!(f, "this container has no routed experts"),
        }
    }
}

impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// One layer's worth of held experts, packed.
struct Layer {
    /// Gate, up, down — in `ROLES` order. Each holds `held.len()` slices end to
    /// end, in the order of `held`.
    packed: [Arc<[u8]>; 3],
    /// `[rows, cols]` of one expert, per role.
    dims: [[u64; 2]; 3],
    ty: [GgmlType; 3],
}

/// The experts this worker holds, resident.
pub struct Slice {
    /// The container these came from, so a main device cannot route a Qwen3
    /// token to V4-Flash weights and get plausible nonsense back.
    pub model: String,
    pub width: u32,
    /// Model-numbered experts held, ascending. The same set in every layer:
    /// a worker that held different experts in different layers would make
    /// assignment a per-layer problem for no gain.
    pub held: Vec<u32>,
    pub layers: Vec<u32>,
    pub bytes: u64,
    /// The clamp the reference path applies to gate and up. Read from the
    /// container rather than assumed — it is architecture-specific and a wrong
    /// one is invisible.
    limit: f32,
    by_layer: std::collections::BTreeMap<u32, Layer>,
}

impl Slice {
    /// Read `experts` of `layers` out of `path` and hold them in memory.
    ///
    /// **This is the slow part, and it happens once.** Reading 3.3 GB at
    /// startup is the trade the whole design makes: pay the disk once so every
    /// later token is answered from RAM.
    pub fn load(
        path: &str,
        layers: &[u32],
        experts: &[u32],
        limit: f32,
        mut on_progress: impl FnMut(u32, u64),
    ) -> Result<Slice> {
        let model = Model::open_split(path).map_err(|e| Error::Model(format!("{path}: {e}")))?;
        let width = model
            .arch_u64("embedding_length")
            .ok_or(Error::NotMoe)
            .map_err(|_| Error::Model("no embedding_length in the container".into()))?
            as u32;

        let mut held: Vec<u32> = experts.to_vec();
        held.sort_unstable();
        held.dedup();

        let mut by_layer = std::collections::BTreeMap::new();
        let mut bytes = 0u64;
        for &il in layers {
            let mut packed: Vec<Arc<[u8]>> = Vec::with_capacity(3);
            let mut dims = [[0u64; 2]; 3];
            let mut ty = [GgmlType(0); 3];

            for (r, role) in ROLES.iter().enumerate() {
                let name = format!("blk.{il}.{role}.weight");
                let loc = model
                    .location(&name)
                    .ok_or(Error::NotMoe)
                    .map_err(|_| Error::Model(format!("{name} is not in this container")))?
                    .clone();
                let n_expert = *loc.dims.last().ok_or(Error::NotMoe)?;
                if n_expert == 0 {
                    return Err(Error::NotMoe);
                }
                let stride = loc.size / n_expert;

                // One contiguous buffer, slices in `held` order. Packing them
                // here rather than per request is what makes a request a matmul
                // over a stack instead of a gather.
                let mut buf = vec![0u8; held.len() * stride as usize];
                for (p, &e) in held.iter().enumerate() {
                    if e as u64 >= n_expert {
                        return Err(Error::NotHeld {
                            layer: il,
                            expert: e,
                        });
                    }
                    let src = model
                        .read_tensor_range(&name, e as u64 * stride, stride)
                        .map_err(|err| Error::Model(format!("{name} expert {e}: {err}")))?;
                    buf[p * stride as usize..(p + 1) * stride as usize].copy_from_slice(&src);
                }
                bytes += buf.len() as u64;
                dims[r] = [loc.dims[0], loc.dims[1]];
                ty[r] = loc.ty;
                packed.push(Arc::from(buf.into_boxed_slice()));
            }

            on_progress(il, bytes);
            by_layer.insert(
                il,
                Layer {
                    packed: [packed[0].clone(), packed[1].clone(), packed[2].clone()],
                    dims,
                    ty,
                },
            );
        }

        Ok(Slice {
            model: std::path::Path::new(path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string()),
            width,
            held,
            layers: layers.to_vec(),
            bytes,
            limit,
            by_layer,
        })
    }

    /// Where an expert sits in this worker's packed order.
    fn position(&self, layer: u32, expert: u32) -> Result<i32> {
        if !self.by_layer.contains_key(&layer) {
            return Err(Error::NotHeld { layer, expert });
        }
        self.held
            .binary_search(&expert)
            .map(|p| p as i32)
            .map_err(|_| Error::NotHeld { layer, expert })
    }

    /// Run `jobs` over `hidden`, returning one `width`-long block per job in
    /// the order the jobs were given.
    ///
    /// `hidden` is `tokens * width`, token-major.
    pub fn compute(
        &self,
        layer: u32,
        jobs: &[crate::wire::Job],
        tokens: u32,
        width: u32,
        hidden: &[f32],
        threads: usize,
    ) -> Result<Vec<f32>> {
        if width != self.width {
            return Err(Error::Width {
                want: self.width,
                got: width,
            });
        }
        if jobs.is_empty() {
            return Ok(Vec::new());
        }
        let held = self
            .by_layer
            .get(&layer)
            .ok_or(Error::NotHeld { layer, expert: 0 })?;

        // Every job's expert, as a position in the packed stack. Done before
        // any ggml allocation so an unheld expert is an error rather than an
        // aborted process -- ggml aborts on a bad index, it does not return.
        let mut ids = Vec::with_capacity(jobs.len());
        for j in jobs {
            ids.push(self.position(layer, j.expert)?);
            if j.token >= tokens {
                return Err(Error::NotHeld {
                    layer,
                    expert: j.expert,
                });
            }
        }

        let n_embd = self.width as i64;
        let n_ff = held.dims[0][1] as i64;
        let n_held = self.held.len() as i64;
        let n_jobs = jobs.len() as i64;

        // **Sized up front, because an exhausted ggml arena aborts the whole
        // process with no message.** Four tensors of `n_ff * n_jobs` plus the
        // inputs and a generous multiple for ggml's own bookkeeping.
        let scratch = (n_ff * n_jobs * 4 * 6 + n_embd * n_jobs * 4 * 4) as usize + (64 << 20);
        let ctx = Context::new(scratch).map_err(|e| Error::Ggml(format!("{e}")))?;
        let mut weights = WeightSet::new();

        for (r, role) in ROLES.iter().enumerate() {
            let d = held.dims[r];
            weights
                .bind(
                    &ctx,
                    role,
                    held.ty[r],
                    &[d[0], d[1], n_held as u64],
                    held.packed[r].clone(),
                )
                .map_err(|e| Error::Ggml(format!("binding {role}: {e}")))?;
        }
        // **Bound 2-D, used 3-D.** `WeightSet::bind` collapses every dimension
        // past the first into one -- `[a, rest @ ..] => (a, product(rest))` --
        // so a stack bound as `[n_embd, n_ff, n_held]` arrives as
        // `[n_embd, n_ff * n_held]`. `mul_mat_id` then reads `ne[1]` as the
        // output width and produces a gate `n_held` times too wide, and the
        // failure is `GGML_ASSERT(as->ne[0] == b->ne[0])` inside the *down*
        // matmul, two ops later, with the process aborted and no Rust frame.
        //
        // The reference path reshapes for exactly this reason. So does this.
        let stack = |role: &str, r: usize| -> Result<chaos_ggml::Tensor<'_>> {
            let t = weights
                .get(role)
                .ok_or_else(|| Error::Ggml(format!("{role} was not bound")))?;
            let d = held.dims[r];
            ctx.reshape_3d(t, d[0] as i64, d[1] as i64, n_held)
                .map_err(|e| Error::Ggml(format!("reshaping {role}: {e}")))
        };

        // **One job per column, not one token per column.** Two jobs may name
        // the same token -- that is the normal case, six experts over one
        // hidden state -- so the hidden state is repeated per job rather than
        // indexed. It costs `n_jobs * 16 KB` of arena and removes the only
        // place a token-to-job mapping could be got wrong.
        let cur = ctx
            .new_f32_3d(n_embd, 1, n_jobs)
            .map_err(|e| Error::Ggml(format!("{e}")))?;
        let mut flat = Vec::with_capacity((n_embd * n_jobs) as usize);
        for j in jobs {
            let at = j.token as usize * n_embd as usize;
            flat.extend_from_slice(&hidden[at..at + n_embd as usize]);
        }
        cur.set_f32(&flat)
            .map_err(|e| Error::Ggml(format!("{e}")))?;

        // One expert per column, so `mul_mat_id`'s ids are [1, n_jobs].
        let ids_t = ctx
            .new_i32_2d(1, n_jobs)
            .map_err(|e| Error::Ggml(format!("{e}")))?;
        ids_t
            .set_i32(&ids)
            .map_err(|e| Error::Ggml(format!("{e}")))?;

        let gate = ctx
            .mul_mat_id(&stack("ffn_gate_exps", 0)?, &cur, &ids_t)
            .map_err(|e| Error::Ggml(format!("{e}")))?;
        let gate = ctx
            .clamp(&gate, f32::NEG_INFINITY, self.limit)
            .map_err(|e| Error::Ggml(format!("{e}")))?;
        let up = ctx
            .mul_mat_id(&stack("ffn_up_exps", 1)?, &cur, &ids_t)
            .map_err(|e| Error::Ggml(format!("{e}")))?;
        let up = ctx
            .clamp(&up, -self.limit, self.limit)
            .map_err(|e| Error::Ggml(format!("{e}")))?;
        let act = ctx
            .swiglu_split(&gate, &up)
            .map_err(|e| Error::Ggml(format!("{e}")))?;
        let down = ctx
            .mul_mat_id(&stack("ffn_down_exps", 2)?, &act, &ids_t)
            .map_err(|e| Error::Ggml(format!("{e}")))?;

        // **Not weighted here.** The routing weights belong to the main device:
        // it computed them, and a worker that applied them would need the
        // router's output as well as the hidden state -- more bytes on the wire
        // for arithmetic that costs nothing where it already is.
        ctx.compute(&down, threads)
            .map_err(|e| Error::Ggml(format!("{e}")))?;
        Ok(down.to_vec_f32())
    }
}
