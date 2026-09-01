# TinyLlama's chat format detects as GlmEdge, and CI cannot see it

> Found 2026-09-01 by running the `--ignored` suite **against real containers**,
> which CI never does. Unrelated to anything else in that session; filed on its
> own so it is not buried in a performance PR.

## The failure

```
core/tokenizer/tests/real_vocab.rs:208
  assertion `left == right` failed:
    wrong format for C:/Projects/models/tinyllama/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf
    left:  GlmEdge
    right: Zephyr
```

`chat_formats_are_detected_from_real_containers` asserts TinyLlama-1.1B-Chat is
**Zephyr** — which it is; that is the template the model was trained with. The
detector answers **GlmEdge**.

## Why it matters more than a wrong enum

`apply_chat_template` renders the prompt from that format. A wrong template does
not error and does not crash: it produces a prompt whose turn markers the model
has never seen, so **the model continues the user's message instead of answering
it.** That is the same failure class as a wrong forward pass — fluent, plausible,
wrong — and `CLAUDE.md` is explicit that it is the one this project must not ship.

`chaos-serve` and `chaos connect` both go through the chat path, so anyone running
TinyLlama through the API gets this.

## Why CI is green

The test is `#[ignore]`d because it needs real containers, and **CI runs
`cargo test -- --ignored` with no models on disk**, where every such test skips.
`ci.yml`'s own comment is honest about it — *"this exercises the skip path"* — and
`CLAUDE.md` records that a full run reports **42 ignored**, so a green run is not a
full run.

This is the first bug that gap has actually hidden, which makes it evidence rather
than a worry.

## What to do

1. **Read the detector** (`chaos_tokenizer::Tokenizer::chat_format`) and find why
   TinyLlama's metadata matches GlmEdge first. Almost certainly an ordering
   problem: a broad pattern tested before a narrow one, in a chain where the first
   match wins. The same shape of bug as `a_longer_name_that_starts_with_an_allowed_one_is_not_allowed`
   guards against elsewhere in this repo.
2. **Check the other containers on this machine** with the same test — it asserts
   several, and only TinyLlama was reached before the panic aborted the case. There
   may be more than one wrong.
3. **Then decide whether CI should hold a small chat-template container.** The
   correctness job already fetches a 397 MB Qwen2-0.5B; TinyLlama Q4_K_M is
   ~640 MB. If that is too much, at least make the *skip* visible — a test that
   silently skips in CI and fails on a real machine is worth an explicit
   `CHAOS_REQUIRE_MODEL_TESTS`-style gate, which that job already has for the
   forward pass.

## What this is not

Not caused by the KV-cache change shipped the same day: `chaos-tokenizer` depends
on `chaos-gguf` and `chaos-model` only, never on `chaos-arch`, so it cannot see
anything in the forward pass. Verified before filing.
