//! Do Chaos and llama.cpp split the same text into the same ids, on a real
//! container?
//!
//! **This repository has shipped a wrong pre-tokenizer before**: three
//! architectures were certified against llama.cpp while `starcoder2`'s
//! pre-tokenizer was wrong, because a wrong split produces fluent nonsense and
//! `tokenizer.ggml.pre` was read by nobody.
//!
//! Written while chasing a V4-Flash perplexity gap, when llama.cpp reported 294
//! tokens for a file Chaos read as 300. Six tokens is 2%, and it is in the wrong
//! direction to explain that gap — finer splitting lowers per-token perplexity —
//! but a tokenizer that disagrees with the reference on a container we ship
//! support for is a defect on its own terms.
//!
//! Prints the ids so they can be diffed against:
//!
//! ```text
//! llama-tokenize -m <model.gguf> -f <text> --ids --no-repack
//! ```
//!
//! ```text
//! cargo test --release -p chaos-tokenizer --test tokenizer_matches_llamacpp_on_a_container \
//!   -- --ignored --nocapture
//! ```

use std::path::PathBuf;

fn container() -> Option<PathBuf> {
    let p = std::env::var("CHAOS_TEST_GGUF").map(PathBuf::from).ok()?;
    p.exists().then_some(p)
}

#[test]
#[ignore = "needs a container and a text file: CHAOS_TEST_GGUF and CHAOS_TEST_TEXT"]
fn print_the_ids_for_a_file() {
    let Some(model) = container() else {
        eprintln!("SKIPPED: set CHAOS_TEST_GGUF to a container");
        return;
    };
    let Ok(text_path) = std::env::var("CHAOS_TEST_TEXT") else {
        eprintln!("SKIPPED: set CHAOS_TEST_TEXT to a text file");
        return;
    };
    let text = std::fs::read_to_string(&text_path).expect("read the text");
    let m = chaos_model::Model::open_split(&model).expect("open the container");
    let tok = chaos_tokenizer::Tokenizer::from_metadata(m.metadata()).expect("tokenizer");
    let ids = tok.encode(&text);
    println!(
        "adds_bos {}  bos {:?}  count {}",
        tok.adds_bos(),
        tok.bos,
        ids.len()
    );
    let joined: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
    println!("{}", joined.join(","));
}
