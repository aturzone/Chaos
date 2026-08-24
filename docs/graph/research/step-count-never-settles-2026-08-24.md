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

The cosines say the pictures differ without saying which is better, and that
looks like the end of what measurement can do. **It is not**, and assuming it
was nearly produced a wrong recommendation.

## Four renders, and the pixels settle most of it

One prompt at 4/8/20/50 steps, same seed, guidance off, so nothing but the step
count differs. `~/.chaos/images/step-comparison/`. Measured off the PNGs:

| steps | time | mean | sd | 1st–99th percentile | spread | edge energy |
|---|---|---|---|---|---|---|
| 4 | 0.2x | 146.8 | 10.5 | 123 – 179 | **56** | 8.11 |
| 8 | 0.4x | 136.6 | 16.8 | 97 – 189 | 92 | 11.11 |
| 20 | 1.0x | 133.7 | 35.1 | 33 – 184 | 151 | 11.86 |
| 50 | 2.5x | 121.8 | 43.4 | 21 – 194 | 173 | 12.35 |

*Spread is the 1st-to-99th percentile of luminance out of 255; edge energy is
the mean absolute difference between horizontally adjacent pixels, which a flat
smudge scores low on even when its histogram happens to be wide.*

**Four steps confines the entire image to a 56-level grey band.** Everything is
between 123 and 179. That is "flat" with a number behind it, and it is not a
matter of taste — the picture has not finished forming.

**Edge energy saturates early; global contrast does not.** 8.11 → 11.11 is the
jump from 4 to 8 steps; after that it creeps, 11.86 and 12.35. So local detail
is mostly there by 8 steps while the *range* keeps opening, 92 → 151 → 173.

So the honest reading:

- **4 steps is ruled out by measurement**, not by opinion.
- **8 steps has most of the edge energy at 0.4x the default's time**, and is
  where somebody looking should start.
- **Which of 8, 20 and 50 is best remains aesthetic.** Contrast and edge energy
  are not beauty; a high-contrast image can be garbage. What they can do is
  rule out an image that is nearly uniform, and they have.

**The default was not changed.** Not on reconstruction evidence, which is
outside its range for this question; not on the cosines, which say the pictures
differ without saying which is better; and not on contrast, which rules one
option out without ranking the rest.

## The recommendation this nearly shipped

Before those renders existed, this node said *"if 4 steps looks acceptable,
256×256 gets five times faster"* — on the strength of a flat reconstruction
error and a 17x time difference.

**`chaos-draw` prints the mean and standard deviation of every picture it
writes**, and it had printed sd 12.2 for the four-step render in output that had
already been read. A five-times speed-up was proposed over an image whose own
summary statistic said it was nearly uniform.

That is the fourth instance today of the same shape — and unlike the other
three, the contradicting number was not merely available, it was on screen.

## The standing lesson

Four instances today, and they are one shape:

| | what it answered instead |
|---|---|
| the "3x" conditioning claim | one latent, in a quantity that varies nineteen-fold between them |
| the encoder's 51 GiB abort | an arena limit reported as the model's ceiling |
| σ=0.95 reconstruction | commitment to a *different* picture, read as integration error |
| "4 steps might be fine" | a flat reconstruction error, with the contrast number already on screen |

**Every one answers a nearby question confidently.** More repetitions catch none
of them — the σ=0.95 run would have reproduced perfectly. What catches them is
asking *what would this number look like if the instrument were pointed at the
wrong thing*, and noticing that in each case it would look exactly like what was
observed.

The cheaper habit, which would have caught the last two on its own: **read every
number the tool already printed before drawing a conclusion from one of them.**
