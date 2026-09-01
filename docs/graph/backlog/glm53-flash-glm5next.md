# Running GLM-5.3-Flash — `glm5_next`, and the two things in the way

> Asked for by Atur, 2026-08-31: *"if we have space,
> `orcarouter/GLM-5.3-Flash-Uncensored-FP8` is good for testing huge models
> running, like V4-Flash."*
>
> **Status: not runnable today, for two reasons that are both measurable and
> neither of which is the model's size.** Written down so the answer is a plan
> rather than a shrug, and so nobody downloads 306 GiB to find out.

## What it is

From the repository card:

| | |
|---|---|
| architecture | `Glm5NextForConditionalGeneration`, HF `model_type` **`glm5_next`** |
| parameters | ~321 B total, **~18 B active** |
| experts | **288 routed, top-8**, plus 1 shared |
| layers | 45, plus an MTP block |
| attention | hybrid gated-linear + sparse |
| residual | **Manifold-Constrained Hyper-Connections (mHC), 4-wide** |
| context | 1,048,576 |
| extras | an integrated vision/video tower |
| the FP8 repo | safetensors, block-FP8 + BF16, **306 GiB on disk** |

## Why Chaos wants it, specifically

**It is the second model that proves the thing this engine exists for.** V4-Flash
is one data point: 144 GiB streaming on a 15.7 GiB laptop. A 321 B model with 288
experts at top-8 is the same shape again with a different container, and a claim
that holds on two unrelated MoE families is a different kind of claim from one
that holds on one.

**And mHC 4-wide is V4-Flash's hyper-connections.** `Deepseek4Config` carries
`hc_mult: 4` and `hc_sinkhorn_iterations: 20`, and `dsv4_hc_pre`, `dsv4_hc_post`
and `dsv4_hc_comb` are already written, tested and diffed. That is the part of a
new architecture port that usually costs the most and it is done — the same
residual algebra, `x[dst] = f(x)*post[dst] + sum_src x[src]*comb[dst, src]` with a
Sinkhorn-normalised combination matrix. **Do not assume the constants transfer**:
4-wide is the width, not the arithmetic, and "mHC" against "hyper_connection" is
a naming match, which this project has been burned by before. It is a head start
on reading, not on verifying.

## Blocker 1 — the FP8 repository does not fit, and is the wrong format anyway

`chaos-run` reads GGUF. The repo Atur linked is safetensors, and this workspace's
only safetensors reader is `core/image`, for diffusion weights — there is no
LLM-side safetensors path and no FP8 dequantiser.

The space arithmetic, measured 2026-08-31: **341.9 GiB free** on this drive
(367.1 GB). The FP8 repo is **306 GiB**. Downloading it leaves 35.9 GiB, and a
GGUF conversion writes a *second* copy — so the FP8 route needs roughly 480 GiB
and is out by 140.

A community **Q4 GGUF** of the same model is roughly 170–190 GiB and does fit,
with room for V4-Flash to stay where it is. Several exist
(`AtomicChat/GLM-5.3-Flash-GGUF`, `Blackfrost-AI/GLM-5.3-Flash-DERISKED-GGUF`,
and others). They are community quantisations of a model whose support is not
upstream yet, which matters for blocker 2 and for trusting the file at all.

**The cheap first step, before any large download:** a split GGUF's metadata
lives in shard 1, and shard 1 is tiny — V4-Flash's is **5.2 MB** of a 144 GiB
model. Fetch that shard alone and run `gguf-info` / `chaos-meta` on it. That
gives the real block count, expert count, expert tensor sizes and therefore the
**always-read set and the per-token expert bytes** — which is the number that
decides whether it runs here at all — for five megabytes instead of 180 GiB.

## Blocker 2 — there is nothing to diff against

`VERIFIED_ARCHITECTURES` means *an eight-prompt diff against llama.cpp*, and
CLAUDE.md is explicit that an architecture name is not a shape and an exit code
is not a diff. **llama.cpp has no `glm5_next` in any released build.** Support
exists only in open pull requests — ggml-org/llama.cpp
[#27752](https://github.com/ggml-org/llama.cpp/pull/27752) (converter plus text
graph, text-only) and [#27754](https://github.com/ggml-org/llama.cpp/pull/27754)
(NextN/MTP for speculative decoding) — with
[#27922](https://github.com/ggml-org/llama.cpp/issues/27922) tracking the
request.

CI pins a llama.cpp commit precisely so that a green run means the same thing
tomorrow. Diffing against a pull request is still a real diff, but the doc has to
record the exact SHA of the PR build, not the branch name, or the comparison is
unrepeatable — the same rule as any competitive claim here.

So until `glm5_next` is upstream, the honest ceiling for this port is: **runs
under `--force`, diffed against a named PR build, and not written down as
"proven".** That is a worthwhile state to reach; it is not the same as verified,
and the difference must appear in the README's own row.

## Definition of done

1. Shard 1 of a Q4 GGUF read, and `chaos-model-info` printing a fit and a tok/s
   prediction for this machine **before** anything large is downloaded.
2. `glm5next` in `chaos-arch` with the gated-linear and sparse attention paths,
   the mHC residual reusing `dsv4_hc_*` where the algebra is genuinely the same
   and *not* where it merely looks the same.
3. An eight-prompt diff against a llama.cpp build from a **named commit**, with
   both command lines and both outputs in a research node.
4. A measured tok/s on this machine, from repeats alternating with llama.cpp in
   one session, and the per-token expert bytes stated beside it.
5. The README's speed table decides then: a row here means the five-model set
   changes, which means `scripts/speed-five.tsv` and the rule change together.

## What this is not

**It is not a way to reach 5 tok/s.** A 321 B model at Q4 streams *more* bytes
per token than V4-Flash does, not fewer — 288 experts at top-8 against 256 at
top-6 — so it lands further down the same disk-bound curve. It is a breadth
claim, not a speed one, and pitching it as a speed result would be the same
mistake as the four numbers this project has already retracted.

Related: `../research/v4flash-has-no-slack-2026-08-10.md` (why there is no
redundancy to exploit in a container like this), `../reference/hard-won-facts.md`,
`the-plan-v0-1-0.md`.
