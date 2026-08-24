# Small images are worse because the denoiser is worse (2026-08-24)

**Atur's report**: 256 and 512 come out visibly worse than a linear reading of
the size ladder suggests, while 1024 is photorealistic.

**Three candidates, needing completely different work:**

1. the denoiser genuinely predicts a worse direction at fewer tokens;
2. the sampler — twenty steps right at 4096 tokens and wrong at 256, or a sigma
   schedule that does not transfer;
3. neither, and 256 tokens simply carry a sixteenth of what 4096 do.

**It is (1).** Measured, monotonically, across five grids.

## The measurement

`cargo run --release -p chaos-image --example velocity-by-grid`

A rectified-flow model is trained on a straight line between a real latent and
pure noise. At noise level `sigma` the input is `latent * (1 - sigma) + noise *
sigma` and the velocity it should predict is exactly `noise - latent`. **Both
terms are known**, so the answer is scored by cosine with no picture involved
and nothing to fool the eye. This is the instrument that found the upside-down
bug; it is used here unchanged, at five sizes.

Same picture at every size, same seed, the **unconditional** twin so no text
encoder is involved and the prompt is not a variable.

| size | grid | tokens | cos σ=0.8 | cos σ=0.5 | **cos σ=0.25** | x0 err σ=0.25 |
|---|---|---|---|---|---|---|
| 256 | 16 | 256 | 0.9330 | 0.9233 | **0.8584** | 0.2198 |
| 384 | 24 | 576 | 0.9508 | 0.9443 | **0.8976** | 0.1904 |
| 512 | 32 | 1024 | 0.9604 | 0.9558 | **0.9185** | 0.1700 |
| 640 | 40 | 1600 | 0.9671 | 0.9633 | **0.9335** | 0.1527 |
| 768 | 48 | 2304 | 0.9724 | 0.9680 | **0.9408** | 0.1433 |

Monotonic in every column, and in the reconstruction error as well.

## What it says

**The denoiser is worse at small grids, and the gap is largest where it
matters most.** At σ=0.8 the spread across the ladder is 0.933 → 0.972; at
σ=0.25 it is 0.858 → 0.941. Heavy noise is the easy case — "point away from the
noise" is most of the answer. Light noise is where the model has to know what
the picture *is*, and that is exactly where 256 tokens fall down.

Read as error rather than agreement, `1 - cos` at σ=0.25 goes 0.142 → 0.059
across the measured range: **the direction is 2.4x worse at grid 16 than at
grid 48**, before a single sampler step has run.

The increments are shrinking — 0.0392, 0.0209, 0.0150, 0.0073 — so the curve is
saturating and grid 64 should land near 0.95. That is consistent with 1024
looking photorealistic and 256 looking flat, and it is not a bug anywhere in
this codebase.

## What follows, and what does not

**Do not go looking for a sampler bug.** That was the obvious next move and the
measurement removes it. The per-step direction is worse before any stepping
happens.

**Worth trying, and not yet tried**: more steps at small grids. If each step's
direction is 2.4x worse, more of them may partly compensate — the app offers 4
to 50 and the default is 20 regardless of size. That is a cheap experiment and
it is the only lever this side of the model.

**Not worth trying**: anything that treats 256 as a scaled-down 1024. The model
was trained at a resolution and it is better near it.

**The honest thing to put in the interface** is what the numbers say, which is
now in `SIZES`: 256 is not "quick, and flat" as a matter of taste — it is
measurably a worse prediction.

## A limitation worth recording

**1024 could not be measured on this machine**, and the reason is not the
denoiser. `vae::encode` builds an **unplanned** graph — every tensor allocated,
none freed — at about 48 KiB per input pixel, so a 1024×1024 encode asks for
**51 GiB** and ggml aborts with `GGML_ASSERT(ctx->mem_buffer != NULL)`. 768
works at 29 GiB of virtual arena; 1024 does not.

`vae::decode_planned` already solves exactly this for the other direction —
`ggml_gallocr` reuses buffers whose lifetimes do not overlap, measured **81x
smaller and bit-identical**. An `encode_planned` mirroring it would lift this
limit, and would also be what img2img at large sizes needs. It is not written,
and this note is the reason to write it.
