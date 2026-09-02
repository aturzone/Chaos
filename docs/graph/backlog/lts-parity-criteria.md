---
topic: What "on par with llama.cpp, nothing missing" actually means — the checklist that decides when v0.0.X LTS ships
status: proposed, awaiting Atur
links: [lts-0-0-0.md, the-big-bang.md, ../research/the-plateau-was-ours-2026-08-10.md]
---

The goal, in Atur's words: **standards-compliant, a person can easily open any
model, best performance against llama.cpp on the criteria, all of its options
and capabilities, nothing left missing — then tag v0.0.X LTS.** Only after that,
20 tok/s.

This node turns that into a checklist that can be ticked, because "nothing left
missing" is otherwise unfalsifiable and would never ship. Every row is either
**done**, **gap**, or **won't** — and a `won't` needs a reason written here, not
a shrug.

## The honest starting position

Chaos today opens **three** architectures and **one** tokenizer family. That is
the real distance to "any model", and it is far larger than the performance gap.

| | Chaos | llama.cpp |
|---|---|---|
| architectures | **10** (`deepseek4`, `gemma2`, `gemma3`, `llama`, `phi3`, `qwen2`, `qwen3`, `qwen3moe`, `stablelm`, `starcoder2`) — each diffed against llama.cpp on **eight** prompts, because three certified `starcoder2` while its pre-tokenizer was wrong | ~100. Every entry is diffed against llama.cpp token for token; `gemma2` sat here for weeks without that and was **wrong** — see `../research/gemma-was-running-silu-2026-08-11.md` |
| tokenizer families | **5** (spm, bpe, wpm, ugm, rwkv) — RWKV is *implemented, not verified*: no container on this machine, so it is tested against a hand-built vocabulary through the real loading path, never against llama.cpp | 6 (spm, bpe, wpm, ugm, rwkv, plamo2) |
| quant types | all ggml can decode | same |
| CLI flags | **182 of 182 recognised — 158 implemented, 24 declined with a reason, 0 swallowed.** Computed 2026-08-15 by intersecting `llama-completion --help` with the parser's match arms and its `REFUSED` table, not tallied — the previous "195 recognised, 170 implemented, 25 declined" was counted by reading and was wrong three ways: it counted short aliases into the denominator, it double-counted `--jinja` as both implemented and declined, and it missed that `--flash-attn` was in **neither** and was being silently taken as the prompt | **182** (counted, not estimated: `llamacpp-flag-audit.md`) |
| chat templates | **54 names, 52 verified byte-identical** against a captured llama.cpp fixture | **54** (counted from `llama-chat.cpp`'s own table, not estimated) |
| samplers | **20** | **20** (counted from `llama_sampler_init_*` in `llama.h`) |
| backends | CPU | CPU, CUDA, Metal, Vulkan, ROCm, SYCL |

## A. Open any model — the biggest gap, and the cheapest wins in it

**Two hard requirements block the entire Llama family today**, and neither is
deep:

1. `Qwen3Model::required_tensors` demands `attn_q_norm` / `attn_k_norm` on every
   block. Qwen3 has per-head QK norm; **llama, mistral, qwen2, gemma and phi do
   not**, so a container without them is refused before a byte is read.
2. `Tokenizer::from_metadata` refuses anything but `tokenizer.ggml.model ==
   "gpt2"`. The Llama family ships `"llama"` (SentencePiece).

| ticket | what | unlocks | state |
|---|---|---|---|
| **A1** | make QK-norm optional in the dense path | `llama`, `mistral`, `qwen2` structurally | **DONE** |
| **A2** | SPM tokenizer (`tokenizer.ggml.model = "llama"`) | the whole Llama/Mistral family's text | **DONE** — verified on TinyLlama |
| **A3** | accept the `llama` arch name and its metadata aliases | Llama 1/2/3, TinyLlama, CodeLlama, Vicuna, most finetunes | **DONE** — verified on TinyLlama and Llama-3.2 |
| A4 | `gemma`/`gemma2` | Gemma family | **gemma2 DONE 2026-08-10** — post-norms, attention soft-cap (50, into the fused kernel), final soft-cap (30), `sqrt(n_embd)` embedding scaling. Output matches llama.cpp exactly. **Sliding window implemented 2026-08-10** — a second mask for the even layers; verified above the window against llama.cpp at 5201 tokens, and the layer parity shown to be load-bearing by flipping it. The 4096 refusal is gone. See `../research/gemma2-sliding-window-2026-08-10.md` |
| A5 | `phi3`, `qwen2` explicit | Phi, Qwen2 | **phi3 DONE 2026-08-10** — fused `attn_qkv` *and* fused `ffn_up` split into views; verified against llama.cpp's own output |
| A6 | WPM + UGM tokenizers | BERT-family, T5-family | **WPM DONE 2026-08-10** — verified token-for-token against `llama-tokenize` on `all-MiniLM-L6-v2`, 13 cases. GGUF stores WordPiece in *SentencePiece* spelling (`▁capital` + bare `ization`), not HuggingFace's `##`, so a textbook implementation `[UNK]`s every ordinary word without erroring: `../research/wordpiece-spelling-2026-08-10.md`. **UGM DONE 2026-08-10** — Viterbi lattice, `USER_DEFINED` scored 0, `f64` path sums; verified on `flan-t5-small`, 5 cases. **Caveat: the precompiled charsmap (NFKC) is not applied**, so input not already in normal form (fullwidth, ligatures) diverges: `../research/unigram-lattice-2026-08-10.md`. **Pre-tokenizers DONE 2026-08-10** — `tokenizer.ggml.pre` was read by nobody, so every BPE container was split with DeepSeek's rule: Qwen's digits grouped in threes instead of singly and `don't` was cut into three pieces on *both* families. `llama-bpe`/`llama3`, `qwen2` and `joyai-llm` now verified against real containers; every other variant is **refused by name** rather than guessed: `../research/pretokenizer-was-ignored-2026-08-10.md` |
| A7 | tied embeddings (`output.weight` absent → reuse `token_embd`) | many small models | **DONE** — Llama-3.2-1B is tied and loads |
| A8 | a clear error naming the *architecture* and what is missing | every unsupported model | **DONE 2026-08-10** — unverified architectures are **refused**; `--force` on the CLI only, never on the server |

**A8 is not cosmetic and should land first.** "Open any model" fails safely only
if the failure says which architecture, which tensor, and whether it is a gap or
a corrupt file. Today an unsupported model reports a missing tensor name.

**A1+A2+A3 together are the single highest-value item in this document**: they
take Chaos from 3 architectures to the majority of GGUF files people actually
download.

## B. Performance against llama.cpp — the criteria

Parity is not one number. These are the cells that must be **≥ llama.cpp**, each
measured back to back in one session with both command lines recorded.

| criterion | V4-Flash | Qwen3-30B-A3B | Qwen3-4B dense |
|---|---|---|---|
| load / time-to-first-token | 1.25x behind | — | — |
| prefill tok/s | 1.25x behind | **ahead** @565, @2206 | **38.5 vs 111.2 — 2.9x behind** |
| generation tok/s | **UNMEASURABLE HERE** — Chaos **0.696** ±1%, llama.cpp **0.16–0.47** across 8 runs of one command line, drifting down within every sequence even with Chaos never started. No ratio published: best gives 1.70x, alternating median 2.32x, worst 4.35x. `../research/the-v4flash-parity-cell-does-not-reproduce-2026-09-01.md` | 1.07 vs 2.16 | **4.27 vs 5.90 — 1.38x behind** (Llama-3.2-1B: 10.12 vs 12.91, 1.28x) |
| memory footprint at equal speed | **ours, by design** | ours | — |
| long-context generation | untested (and the short cell is unmeasurable here) | untested | **3.13 vs 4.53 — 1.45x behind** at 4031 tokens, both engines stable. **The ratio FLIPS**: short context, same session, Chaos **8.39 vs 7.69 — 1.09x ahead**. Chaos slows **2.68x** with 4000 tokens of context against llama.cpp's **1.70x**. `../research/long-context-parity-qwen3-4b-2026-09-01.md` |
| long-context prefill | untested | untested | **60.98 vs 59.77 — parity** (inside the spread) |
| **the long-context generation gap is diagnosed** | — | — | **the KV cache is copied every token**: `cont(permute(kv))` costs **87.55 ms of a 306.7 ms token** at 4031 positions, a **3.25x** attention slope. Removing it projects to ~4.42 tok/s against llama.cpp's 4.49 — the whole gap. Diagnosed, not built: `../research/the-kv-cache-is-copied-every-token-2026-09-01.md` |
| **quality (perplexity)** | untested | untested | **33.6434 vs 34.0293 — 1.13%** |

**The release gate is "every cell measured", not "every cell won"** — Atur's
decision, 2026-09-02, and it came from him noticing that the README's v0.0.30 bar
said 20% while the release was being prepared.

He was right, and the bar was measuring the wrong thing. Counting *cells won*
against llama.cpp can never reach 100% from here: generation is 1.38x behind on
dense Qwen3-4B and roughly 2x on Qwen3-30B-A3B, closing either is real
engineering rather than a release step, and one cell is permanently
`unmeasurable here`. A gate that cannot be met is not a gate, it is a reason
never to tag — and this project has already been the other kind of wrong, with 23
releases in 21 days and no stabilisation period for any of them.

So the standard for tagging is: **every cell has a number, or a written reason
why it cannot have one.** That is exactly what `SUPPORT.md` already promises —
"every claim was measured on hardware with the command line recorded" — and it is
a promise one person can keep. Beating llama.cpp everywhere is not.

**The count, recounted.** This document said "18 cells" and that was wrong: the
table is 8 criteria x 3 models = 24, less 5 marked `—` because the criterion does
not apply to that model, so **19**. Six are `untested`:

| model | cell |
|---|---|
| V4-Flash | long-context generation, long-context prefill, quality |
| Qwen3-30B-A3B | long-context generation, long-context prefill, quality |

**13 of 19 measured, 68%.** All six are measurable on this machine, and a seventh
job is not on the list because it already has a number: the Qwen3-4B prefill cell
is measured but **stale**, from before four fixes, so it is re-measured rather
than counted again.

**A cell can also be `unmeasurable here`, and one now is.** That is a third state beside `done` and `gap`, and it needs the same thing a `won't` needs: a reason written down. V4-Flash generation is the first — llama.cpp's figure on this container ranges **0.16 to 0.47 tok/s** for the same command line on the same machine, so alternating against it measures the operating system rather than the engines. Chaos's own figure is stable to 1%. Closing it needs a machine where a 144 GB working set does not fight a 15.7 GiB page cache, not more repetitions.

**The streaming path can be scored at all now, as of 2026-09-02.** The quality
row said `untested` for V4-Flash and the reason was not laziness:
`--perplexity` is plumbed to `run_streaming` and the deepseek4 dispatch returns
from `run` before that, so the flag was **silently ignored on that model** —
found while putting C7 through the gate's *lossy* bar, a third of which is the
perplexity band. It now scores a corpus with the dense path's windowing, so this
cell has become measurable. **It is not measured against llama.cpp yet**, and it
will not be until `llama-perplexity` has run the same corpus with the same
chunking on the same machine, with both command lines recorded here.

**The `38.5 vs 111.2` prefill cell is stale and its direction is unknown.** It
dates from 2026-08-10, from a 651-token prompt against llama.cpp's `pp512`, and
since then the dense path gained the KV cache, stopped projecting every position
through the output matrix, stopped aborting on its arena, and lost two `cont`
calls in the KV path — while long prefill at 4031 tokens measured **60.98 vs
59.77, parity**. Re-measure it at a matched length before anyone quotes either
number.

**Quality is measured now, and it was the largest untested claim in this
document.** `chaos-run --ppl-chunk N` reports perplexity using llama.cpp's
windowing (whole chunks only, second half scored, `n_ctx - 1 - n_ctx/2` tokens
each). Llama-3.2-1B: **29.0909 against 29.2456 ± 6.49**; Qwen3-4B: **33.6434
against 34.0293 ± 9.64**. Two architectures, two tokenizer families, both within
~1% — which exercises the tokenizer, RoPE, the causal mask, the KV cache, fused
attention, repacking and the output projection against an independent
implementation. Both sit inside llama.cpp's own error bar, so this is agreement,
**not** a claim to be more accurate: `../research/perplexity-2026-08-10.md`.

**Weight repacking does not transfer to V4-Flash, and the row above does not
move** (2026-08-10). Every always-read tensor in that container with a
repackable shape is `Q8_0`, and ggml's repacked `Q8_0` kernels are NEON and
RISC-V only — 42 offered, 42 declined, 0 repacked on x86. llama.cpp cannot even
load the file with repacking on (a 137 GiB single-range `CPU_REPACK` buffer),
which is why its figures here pass `--no-repack`. The attempt did fix a null
dereference that would have killed `chaos-run` on any `*.Q8_0.gguf`:
`../research/v4flash-repacking-2026-08-10.md`.

**Dense Qwen3-4B has never been compared to llama.cpp at all**, and it is the
cheapest comparison available — it fits in RAM, so it isolates the compute path
from all the streaming machinery. It should be the first cell closed.

Current ceiling on this machine, measured:
`the-plateau-was-ours-2026-08-10.md` puts a V4-Flash token at ~1.54 s of expert
reads + ~0.6 s of everything else, so with R2 overlap it is **~0.65 tok/s against
0.39** — a real 1.7x lead. That is the performance bar for LTS on this model.

## C. Options and capabilities

| ticket | what | state |
|---|---|---|
| C1 | sampling: temperature, top-k, top-p, min-p, repeat penalty, seed | **DONE 2026-08-10** — 10 unit tests, `--llamacpp-defaults` for like-for-like comparison. **Extended same day** with `--frequency-penalty` and `--presence-penalty` (OpenAI's fields and llama.cpp's flags), including `temperature: 0` + a penalty, which is the penalised argmax and would otherwise have run `powf(1e6)` |
| C2 | chat templates from `tokenizer.chat_template` | **DONE 2026-08-10** — 9 families, detected from the real templates; control tokens encode to single ids |
| C3 | streaming responses (SSE) in `chaos-serve` | **DONE 2026-08-10** — plus temperature/top_p/top_k/min_p/seed/stop from the request, EOS and stop sequences give `finish_reason: stop` |
| C4 | `-c` context size, `-b` batch, `-t` threads as flags | **DONE 2026-08-10, then found broken and re-done the same day.** `-t` reached only `deepseek4`; every other architecture ignored it. Now plumbed, plus llama.cpp's `-tb`/`--threads-batch`, because generation and prefill want opposite counts. The generation default is **tuned on real tokens** — 1.66x/1.69x over "all cores". See `../research/threads-were-never-plumbed-2026-08-10.md` |
| C5 | stop sequences, `max_tokens`, `n_predict` | **DONE** — `--stop` (repeatable) on the CLI, string-or-array on the server; both match accumulated text, not tokens |
| C6 | grammar / JSON-schema constrained output | **won't for LTS** — large, and not what an agent needs first |
| C7 | LoRA adapters | **won't for LTS** — no user asking |
| C8 | embeddings endpoint | **won't for LTS** — the graph returns logits, not hidden states. Faking it is worse than a 501, and doing it properly means a second output path |
| C9 | quantise/convert tooling | **won't** — llama.cpp owns this and does it well |

**C1 is required, not optional.** Greedy decoding makes every answer
deterministic and flat; no one will judge quality favourably against llama.cpp
without samplers, and it is a day of work.

## D. Standards compliance

| ticket | what | state |
|---|---|---|
| D1 | read every GGUF metadata type incl. arrays and nested | **DONE 2026-08-10** — 16 tests: hand-written malformed corpus plus two sweeps that need no fuzzing crate (every prefix of a valid container; >1,000 single-byte corruptions). Found one real bug: **a duplicate metadata key overwrote silently**, so a file with two `general.architecture` entries loaded as the second one with no error — now `DuplicateKey`, alongside `EmptyKey` and `DuplicateTensor`, matching llama.cpp. See `../research/malformed-containers-2026-08-10.md` |
| D2 | GGUF v2 and v3 | **DONE 2026-08-10** — v2 and v3 proved to parse identically from in-memory headers, v1 and future versions refused, alignment honoured only when a power of two. **The ticket's premise was wrong**: the `u32`→`u64` length change was v1→v2, not v2→v3, and implementing it as written would have mis-read every real v2 container — llama.cpp has no width branch and refuses v1 outright. Also added: a byte-swapped version is now named as an endianness mismatch instead of "unsupported version 50331648". See `../research/gguf-v2-premise-was-wrong-2026-08-10.md` |
| D3 | split containers (`-00001-of-0000N`) | **done** |
| D4 | every ggml quant type ggml can decode | **done — delegated to ggml** |
| D5 | OpenAI API surface: `/v1/chat/completions`, `/v1/models`, `/v1/completions`, `/v1/embeddings` | **3 of 4 + an honest 501.** Chat streams and serves any supported architecture; `/v1/completions` runs the prompt verbatim; **embeddings refuse with 501** rather than returning a logit-derived vector that would look right and behave like noise |
| D6 | refuse an unsupported container clearly rather than producing nonsense | **DONE** — see A8. Gemma-2 was the proof that it was needed |

## The order

1. **A8** — say clearly what is not supported. Everything else is safer after it.
2. **A1 + A3** — QK-norm optional, accept `llama`. Small, structural.
3. **A2** — SPM tokenizer. Needs unit tests against fixtures; a wrong tokenizer
   produces fluent nonsense, never a crash.
4. **C1** — samplers. Cheap, and quality is judged on it.
5. **B: Qwen3-4B dense vs llama.cpp**, both command lines recorded. The cheapest
   uncollected comparison in the project.
6. **C2 + C3** — chat templates and streaming, which is what makes the server
   usable from an editor.
7. **R2 overlap** — the remaining measured 1.4x on V4-Flash.
8. Then A4–A7, C4, C5, C8, D1, D2, D5.

**Then tag v0.0.X LTS.** Then 20 tok/s.

## What this document deliberately does not promise

Feature parity with *all* of llama.cpp is not achievable and not the goal — it
is years of work by hundreds of people across every backend. `won't` rows above
are the honest boundary. **The LTS claim is: "opens the models people actually
run, matches or beats llama.cpp on the models it supports, and tells you the
truth about your machine before you download 144 GB."** Anything wider than that
would be a claim this project cannot defend, and this project has retracted two
claims already.

## One thing that needs Atur

**Testing A1–A3 needs a Llama-architecture GGUF, and there is none on this
machine.** TinyLlama-1.1B Q4_K_M is ~670 MB — the smallest container that
exercises both the `llama` architecture and the SPM tokenizer. Home internet is
limited, so this is a decision, not an assumption: the code can be written and
unit-tested without it, but **it cannot be called supported until a real
container has been opened and its output checked.**
