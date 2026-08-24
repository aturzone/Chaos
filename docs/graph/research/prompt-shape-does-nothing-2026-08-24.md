# It is the content, not the JSON shape (2026-08-24)

**Two corrections to a published claim, and one feature killed before it
shipped.**

Chaos has said in four places — README, `docs/APP.md`, `chaos-draw --help` and
the CHANGELOG — that *"structured, JSON-shaped prompts condition about three
times as strongly as a bare phrase"*. That came from
`backlog/image-generation-ideogram-4.md`, measured on **one latent**:

| prompt style | cos(A, B) | effect |
|---|---|---|
| seven words | 0.9897 | 1.0% |
| structured JSON | 0.9667 | 3.3% |

## What eight latents say

`cargo run --release -p chaos-image --example structure-helps`

Same method — run the conditional denoiser under two very different prompts and
measure how far apart the answers are — over eight noise seeds instead of one.
Higher "effect" (`1 - cos`) means the prompt did more work.

| seed | bare phrase | wrapped in empty JSON | written out by hand |
|---|---|---|---|
| 1 | 0.30% | 0.58% | 0.41% |
| 2 | 0.52% | 0.37% | 6.22% |
| 3 | 0.32% | 0.49% | 2.96% |
| 4 | 0.31% | 0.36% | 5.25% |
| 5 | 0.54% | 0.23% | 2.33% |
| 6 | 0.32% | 0.23% | 5.51% |
| 7 | 0.34% | 0.27% | 4.73% |
| 8 | 0.47% | 0.35% | 7.83% |
| **mean** | **0.39%** | **0.36%** | **4.40%** |
| **vs bare** | 1.0x | **0.9x** | **11.3x** |

## Correction 1: the effect is 11.3x, not 3x

The published number is not wrong in direction, it is wrong in size, and it
**understates** the case. A properly written prompt moves the denoiser eleven
times as far as a phrase, not three.

## Correction 2: the JSON shape does nothing at all

`prompt::structure` wraps a phrase in the same nested frame with the
descriptive fields left **empty** — deliberately, because a wrapper that
invented *golden hour, bokeh, 8k* would draw a different picture than the one
asked for. Measured at **0.9x**: indistinguishable from the bare phrase, and its
range (0.23–0.58%) overlaps the bare phrase's (0.30–0.54%) completely.

So the advice this project has been giving is aimed at the wrong thing. It is
not the braces. **It is the sentences**: "soft even studio lighting from above,
gentle shadow beneath the apple", "a plain white studio backdrop meeting a white
tabletop, no visible seam", a named colour palette. Writing those is what earns
the 11.3x; putting a bare phrase inside `{"high_level_description": ...}` earns
nothing.

**A button that wrapped a prompt was written and is not being shipped**, because
this measurement says it would do nothing while looking like it did something.
The harness that killed it is kept.

## Why the old number was believable, and the real lesson

**One latent is not a measurement here.** Look at the spread in the
hand-written column: 0.41% at seed 1 and 7.83% at seed 8, a factor of nineteen.
The first version of this experiment used a single seed and reported that a
hand-written prompt conditioned **fourteen times** more strongly; a different
seed reported it conditioned **three times less**. Both runs were correct and
neither was an answer.

`try-conditioning` uses one seed, and reproduces exactly — cos(A, B) 0.9527 at
seed 7 — so it is not wrong, it is under-sampled. It is a good instrument for
"is the text reaching the model at all", which is what it was written for, and a
bad one for "how much does prompt style matter".

This is the third time in this project that a one-sample measurement has said
something confident and wrong. The standing rule already covers it — score out
of sample, with a null and a spread — and this is that rule applied to
conditioning rather than to residency.

## What was changed as a result

- The four places carrying "3x" now say what was measured, and say that the
  shape is not the point.
- The IMAGE page's note tells a user what to *write* rather than what shape to
  write it in.
- No prompt-wrapping button.
