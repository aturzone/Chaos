//! How fast is the tokenizer, and is it fast enough to ignore?
//!
//! Usage: `chaos-tokbench <model.gguf> [--rounds N] [--text <file>]`
//!
//! # Why this exists
//!
//! §4a of `backlog/v0-0-3-the-complete-version.md` names the tokenizer as
//! **untouched ground worth measuring**, and it was untouched in the strict
//! sense: nothing in the workspace timed it. Prefill and generation are reported
//! by `chaos-run` down to the millisecond and split into qkv, attention, ffn and
//! disk; tokenization happens before any of that and appeared in none of it.
//!
//! An unmeasured stage is not a fast stage. It is a stage nobody can rule out.
//!
//! # What it measures, and what that is worth
//!
//! Encode throughput in MB/s and tokens/s, decode throughput, and a round-trip
//! check that `decode(encode(text))` returns the text. **The round trip is not
//! decoration**: a tokenizer that is fast and lossy is worse than a slow one, and
//! byte-level BPE has a specific way of being lossy — a byte that no merge covers
//! comes back as a replacement character rather than an error.
//!
//! Reads only the container's header, so a 144 GB model costs the same as a
//! 762 MiB one and no weights are touched.

use chaos_gguf::Gguf;
use chaos_tokenizer::Tokenizer;
use std::io::Read;
use std::time::Instant;

/// Enough for the metadata and the tensor index of any container seen so far.
/// A vocab of 150k tokens is a few megabytes of that budget on its own.
const HEADER_BUDGET: usize = 96 * 1024 * 1024;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut path = None;
    let mut rounds = 5usize;
    let mut text_file: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--rounds" => rounds = args.next().and_then(|v| v.parse().ok()).unwrap_or(5),
            "--text" => text_file = args.next(),
            "-h" | "--help" => {
                usage();
                return;
            }
            // **Answered before any argument is taken as a path.** Every shipped
            // binary must answer this -- it is how a person checks an update
            // landed -- and a test reads these sources to be sure. Two of eleven
            // used to; `chaos-gpubench --version` started benchmarking the GPU.
            "-V" | "--version" => {
                println!("chaos-tokbench {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            other => path = Some(other.to_string()),
        }
    }
    let Some(path) = path else {
        usage();
        std::process::exit(2);
    };

    let mut buf = Vec::new();
    match std::fs::File::open(&path) {
        Ok(f) => {
            if let Err(e) = f.take(HEADER_BUDGET as u64).read_to_end(&mut buf) {
                eprintln!("chaos-tokbench: reading {path}: {e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("chaos-tokbench: cannot open {path}: {e}");
            std::process::exit(1);
        }
    }
    let gguf = match Gguf::parse(&buf) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("chaos-tokbench: {path} is not a container we can read: {e}");
            std::process::exit(1);
        }
    };
    let tok = match Tokenizer::from_metadata(&gguf.metadata) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("chaos-tokbench: no usable tokenizer in {path}: {e}");
            std::process::exit(1);
        }
    };

    let text = match &text_file {
        Some(f) => match std::fs::read_to_string(f) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("chaos-tokbench: cannot read {f}: {e}");
                std::process::exit(1);
            }
        },
        None => sample_text(),
    };

    println!(
        "model      {}  ({:?}, vocab {})",
        path.rsplit(['/', '\\']).next().unwrap_or(&path),
        tok.kind(),
        tok.vocab_size()
    );
    println!(
        "text       {} bytes, {} chars",
        text.len(),
        text.chars().count()
    );
    println!("rounds     {rounds}");
    println!();

    // **Warm the caches once and throw it away.** The first encode pays for the
    // vocabulary's first touch, and reporting that as throughput would flatter
    // or slander the tokenizer depending on how big the text was.
    let warm = tok.encode(&text);
    let n_tokens = warm.len();

    let mut enc_ms: Vec<f64> = Vec::with_capacity(rounds);
    let mut dec_ms: Vec<f64> = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let t0 = Instant::now();
        let ids = tok.encode(&text);
        enc_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(&ids);

        let t1 = Instant::now();
        let back = tok.decode(&ids);
        dec_ms.push(t1.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(&back);
    }

    let e = median(&mut enc_ms);
    let d = median(&mut dec_ms);
    let mb = text.len() as f64 / 1_048_576.0;
    println!(
        "           {:>10}  {:>12}  {:>14}",
        "median ms", "MB/s", "tokens/s"
    );
    println!(
        "encode     {e:>10.2}  {:>12.1}  {:>14.0}",
        mb / (e / 1000.0),
        n_tokens as f64 / (e / 1000.0)
    );
    println!(
        "decode     {d:>10.2}  {:>12.1}  {:>14.0}",
        mb / (d / 1000.0),
        n_tokens as f64 / (d / 1000.0)
    );
    println!();
    println!(
        "tokens     {n_tokens} for {} bytes ({:.2} bytes/token)",
        text.len(),
        text.len() as f64 / n_tokens.max(1) as f64
    );
    println!(
        "spread     encode {:.1}%, decode {:.1}%  (max-min over median)",
        spread(&enc_ms),
        spread(&dec_ms)
    );

    // **Fast and wrong is worse than slow.** Reported next to the speed so the
    // two are never quoted apart.
    //
    // **The BOS token has to come off first.** `encode` prepends one when the
    // container asks for it -- "The quick brown fox" is five tokens, not four --
    // so a naive `decode(encode(text))` starts with `<|begin_of_text|>` and
    // differs at character zero. The first version of this check reported
    // exactly that and it looked like total corruption of a byte-level BPE.
    // Comparing the wrong two strings is not a finding.
    let body: &[u32] = if tok.adds_bos() && !warm.is_empty() {
        &warm[1..]
    } else {
        &warm
    };
    let round_trip = tok.decode(body);
    if round_trip == text {
        println!("round trip exact");
    } else {
        let same = round_trip
            .chars()
            .zip(text.chars())
            .take_while(|(a, b)| a == b)
            .count();
        println!(
            "round trip DIFFERS: {} of {} chars matched before diverging",
            same,
            text.chars().count()
        );
        let show: String = round_trip
            .chars()
            .skip(same.saturating_sub(8))
            .take(32)
            .collect();
        let want: String = text.chars().skip(same.saturating_sub(8)).take(32).collect();
        println!("           got  {show:?}");
        println!("           want {want:?}");
    }

    // What this means for a real run, which is the only reason to care.
    let per_642 = e * (642.0 / n_tokens.max(1) as f64);
    println!();
    println!(
        "A 642-token prompt costs about {per_642:.2} ms to tokenize. Chaos prefills\n\
         642 tokens of Llama-3.2-1B in about 2,100 ms, so tokenization is roughly\n\
         {:.3}% of that prompt's prefill.",
        per_642 / 2100.0 * 100.0
    );
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if v.is_empty() {
        return 0.0;
    }
    v[v.len() / 2]
}

fn spread(v: &[f64]) -> f64 {
    let (mut lo, mut hi) = (f64::MAX, 0.0f64);
    for &x in v {
        lo = lo.min(x);
        hi = hi.max(x);
    }
    let mut sorted: Vec<f64> = v.to_vec();
    let m = median(&mut sorted);
    if m <= 0.0 {
        return 0.0;
    }
    (hi - lo) / m * 100.0
}

/// A page of prose, some code, some punctuation and some non-ASCII.
///
/// **Deliberately mixed.** A tokenizer measured only on English prose is
/// measured on its best case: merges hit, few byte fallbacks. Code and non-Latin
/// text are where a byte-level BPE spends its time.
fn sample_text() -> String {
    let mut s = String::new();
    for _ in 0..200 {
        s.push_str("The quick brown fox jumps over the lazy dog. ");
        s.push_str("fn main() { let x: Vec<u32> = (0..10).collect(); }\n");
        s.push_str("سلام دنیا — こんにちは世界 — Привет, мир!\n");
        s.push_str("0x1F4A9 3.14159 --flag=value {\"key\": [1, 2, 3]}\n");
    }
    s
}

fn usage() {
    println!("usage: chaos-tokbench <model.gguf> [--rounds N] [--text <file>]");
    println!();
    println!("Times encode and decode on a mixed sample of prose, code and");
    println!("non-Latin text, and checks that decode(encode(text)) is exact.");
    println!("Reads only the container's header: no weights are touched, so a");
    println!("144 GB model costs the same as a small one.");
}
