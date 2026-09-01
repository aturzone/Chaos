---
topic: TinyLlama's chat format looked like a user-facing bug and was a stale test — llama.cpp renders exactly what Chaos renders. Filed as a bug, retracted the same hour.
status: resolved — the finding is retracted, the `--ignored` gap it exposed is real
links:
  - the-kv-cache-is-copied-every-token-2026-09-01.md
  - ../reference/hard-won-facts.md
---

# TinyLlama's chat format was a stale test

Running the `--ignored` suite **against real containers** — which CI never does —
turned up one failure in 33:

```
core/tokenizer/tests/real_vocab.rs:208
  wrong format for tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf
    left:  GlmEdge
    right: Zephyr
```

**I filed that as a user-facing bug**: wrong template, so the model would see turn
markers it was never trained on and continue the user's message instead of
answering. It reached `main` in PR #168 as
`backlog/tinyllama-gets-the-wrong-chat-template.md`, and it was wrong.

## What the code says, which I had not read

`ChatFormat::detect` reaches `GlmEdge` deliberately, and the branch carries its
own reasoning:

> **Both tags present is llama.cpp's Falcon-3/GLMEdge branch, and it is checked
> BEFORE zephyr there.** We sent everything with either tag to `Zephyr`, which
> appends the EOS between turns — right for no model in this repository […] Both
> `tinyllama` and `Falcon3` land here, and **neither contains a literal `</s>`** —
> tinyllama writes `eos_token`, the variable — so both are GLMEdge rather than
> Falcon-3.

So the detector was changed on purpose to match llama.cpp, and **the test's
`Zephyr` was the side that had gone stale.** Both files last changed in the same
directory-refactor commit, so history could not arbitrate; the comment could, and
I had not read it before filing.

## Settled against the oracle

`llama-completion` prints its own rendered example for the container it loads:

```
<|system|>
You are a helpful assistant<|user|>
Hello<|assistant|>
```

`chaos-run --chat` on the same container:

```
chat       glmedge template
prompt     "<|user|>\nHello<|assistant|>" -> 14 tokens
```

**The same framing, and no `</s>` anywhere in either.** Zephyr would have put the
EOS between turns, which is what made it wrong. The detector agrees with
llama.cpp; the test now expects `GlmEdge` and records this comparison beside the
case so the next person does not re-file it.

## What I got wrong, and it is the session's pattern

**I diagnosed from a failing test without reading the implementation it tests.**
That is the same mistake as reading a phase timer without reading what falls
between the two `Instant`s — twice, earlier the same day — and as reading `tail`
as hyper-connection work when the whole attention graph was inside it.

A failing assertion says *these two disagree*. It does not say **which one is
wrong**, and the confident-sounding half is not evidence. Especially here, where
the implementation carried fifteen lines explaining precisely why it chose the
answer the test called wrong.

## What survives, and it is worth keeping

**The `--ignored` gap is real.** CI runs `cargo test -- --ignored` with no models
on disk, where all 42 skip; `ci.yml`'s own comment admits it *"exercises the skip
path"*, and `CLAUDE.md` says a green run is not a full run. That gap let a stale
expectation sit in a test nobody could run — for long enough that git history
cannot say how long.

So: **run the `--ignored` suite against real models before believing a green
suite.** It cost one wrong filing to learn, and the instruction is now in
`reference/hard-won-facts.md`. What it found was not a bug in the engine, but it
was a real defect in the test suite, and the test suite is what the engine's
correctness rests on.
