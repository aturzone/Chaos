# Step count changes the picture and never settles — and one run is a lesson in the metric (2026-08-24)

`small-images-are-the-model-2026-08-24.md` found that the denoiser's velocity is
genuinely worse at small grids: **2.65x worse in error terms at grid 16 than at
grid 64**, before any stepping. That is the model, and nothing here can change
it.

It left exactly one lever. If each step points a little wrong, **more, smaller
steps** might accumulate less error than fewer large ones. The app defaults to
20 steps at every size, and if 50 were meaningfully better at 256×256 the
default should depend on the size.

## The measurement

`cargo run --release -p chaos-image --example steps-at-small-grids`

**Integration error against a latent that is actually known.** Generating from
pure noise has no ground truth; denoising a *partly* noised real latent does.
Mix a real latent with noise at `sigma`, integrate the flow back to zero with
`n` Euler steps, compare to the latent it started from. Same noise for every
step count, unconditional twin so no prompt is involved.

### From σ = 0.6 — the valid run

Before any denoising the error is **0.9932**.

| steps | error | seconds |
|---|---|---|
| 4 | **0.3683** | 56 |
| 8 | 0.3722 | 114 |
| 20 | 0.3753 | 289 |
| 30 | 0.3763 | 457 |
| 50 | 0.3770 | 970 |

**Flat, and if anything backwards.** Denoising works — 0.99 down to 0.37 — but
the step count does not move it: a 2.4% spread across a 12x range of step
counts, with the cheapest slightly ahead. **50 steps costs 17x what 4 does and
buys nothing measurable.**

That is not surprising once stated. A rectified-flow model is trained to predict
the *straight-line* velocity to the endpoint, so a perfect model would be exact
in one step. More steps only help where the true path curves; what they
certainly do is query a slightly-wrong model more times.

**So integration error is not what limits small images.** The error floor here
is the model's own accuracy, which is the previous node's finding.

### From σ = 0.95 — and this run does not measure what it was built to

| steps | error | seconds |
|---|---|---|
| 4 | 1.0141 | 103 |
| 8 | 1.2058 | 202 |
| 20 | 1.3219 | 348 |
| 50 | 1.3887 | 712 |

Read naively this says more steps are dramatically *worse*. **It says no such
thing, and the run is kept here because the mistake is the useful part.**

At σ=0.95 the input is 5% latent and 95% noise. There is essentially no signal
left to recover, so the model does not reconstruct *this* picture — it generates
a different one, which is what it is for. "Reconstruction error" then measures
how firmly it commits to that other picture. More steps means a more resolved
image, which is **further from an original it was never recovering**. The
starting error is 1.5725 and every row is above 1.0, i.e. every result is worse
than simply predicting zero — a number that should have stopped the reading
before it started.

**The metric has a validity range and this run left it.** The code comment
already said "not 1.0: at pure noise there is no latent left to recover"; the
threshold was simply set too high. σ=0.6 is inside; σ=0.95 is not.

## What this does and does not establish

**Established**: over the range where reconstruction is defined, step count does
not reduce integration error at grid 16. There is no cheap sampler win here.

**Not established by the above, and it is the question a user actually has**:
whether step count changes the *generated* picture. Generation starts from pure
noise and has no ground truth, so reconstruction error cannot see it.

## So it was measured, and the answer is yes — and it never settles

`cargo run --release -p chaos-image --example steps-change-the-picture 16`

The one thing measurable without an opinion: do two step counts produce the same
latent? Same starting noise throughout, unconditional twin, grid 16.

| cos | 4 | 8 | 20 | 50 |
|---|---|---|---|---|
| **4** | 1.0000 | 0.8895 | 0.7447 | 0.6719 |
| **8** | 0.8895 | 1.0000 | 0.8966 | 0.8024 |
| **20** | 0.7447 | 0.8966 | 1.0000 | 0.9043 |
| **50** | 0.6719 | 0.8024 | 0.9043 | 1.0000 |

**Step count changes the picture substantially.** 4 against the default 20 is
0.7447 — those are different images, not the same image slightly refined.

**And it does not converge.** That is the finding, and it is easy to miss. Every
*adjacent* pair sits at about 0.90 — 4↔8 is 0.8895, 8↔20 is 0.8966, 20↔50 is
0.9043 — while distant pairs fall away, 4↔50 reaching 0.6719. If the sampler
were converging, 20 and 50 would agree far more closely than 4 and 8 do. They do
not. **Each increase keeps moving the output by about the same amount, so there
is no step count at which the picture has settled and no step count that is
"correct".**

That is consistent with the velocity measurement in the sibling node: at grid 16
the model's predicted direction has cosine 0.858 against the truth, and a flow
that imperfect does not have one endpoint that finer discretisation approaches.

## What this means for the default, and what it does not

**It is now an aesthetic question, and this project has no instrument for one.**
The rule here is that an image decoder is checked by round trip and never by
looking — but that rule exists because round trip *can* check a decoder.
Nothing can check which of two different-but-equally-valid pictures is better.

What can be said in numbers:

| steps | time, relative to the default | same picture as the default? |
|---|---|---|
| 4 | **0.2x** | no — cos 0.7447 |
| 8 | 0.4x | no — cos 0.8966 |
| 20 | 1.0x | — |
| 50 | 2.5x | no — cos 0.9043 |

**If 4 steps looks acceptable, 256×256 gets five times faster.** That is worth
somebody's minute of looking, so four renders of one prompt at 4/8/20/50 steps —
same seed, guidance off, so nothing but the step count differs — are written to
`~/.chaos/images/step-comparison/`.

**The default was not changed.** Not on reconstruction evidence, which is
outside its range for this question; and not on the cosines above, which say the
pictures differ without saying which is better.

## The standing lesson

This is the same shape as the two other corrections made today — a one-latent
conditioning claim, and an arena limit standing in for a model limit. Every one
of them is a measurement that answers a *nearby* question confidently. The check
that catches it is not more repetitions; it is asking what the number would look
like if the instrument were pointed at the wrong thing, and noticing that here
it would look exactly like this.
