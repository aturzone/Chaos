# A negative prompt mostly turns guidance down (2026-08-24)

**The plan said this needed "a real pipeline change and a quality harness to
judge it".** The pipeline change is twenty lines. The harness took three runs,
and **the first two answered a different question than the one asked** — once by
confounding two variables, once by measuring at half the size the product
actually draws at.

## The change

Guidance here extrapolates away from a **separately trained unconditional twin**
fed no text at all — not an empty prompt. `ideogram4_uncond` is its own 5.26 GiB
file. A negative prompt replaces that reference with **the conditional denoiser
under the negative text**: same weights, different conditioning.

```
      no negative prompt                      with a negative prompt
  v = guide(cond(x,P), twin(x), s)        v = guide(cond(x,P), cond(x,N), s)
  two models, 10.5 GiB                    one model, 5.26 GiB
```

Two consequences beyond the feature:

- **The twin is never opened**, so a negative prompt halves the denoiser's
  memory while keeping both forward passes. `Paths::missing_for(twin: bool)`
  stops demanding a 5.26 GiB file that will not be read — which also fixes
  `--cfg 1`, where the twin was required and never opened.
- **`Some("")` is not `None` by accident.** An empty text box is not a request
  to condition the model on emptiness, and the conditional model was never
  trained on it. Trimmed and treated as absent, with a test.

## The instrument

`cargo run --release -p chaos-image --example negative-prompt-moves [grid] [sigma] [seeds] [long]`

One prompt, several negative texts, the same latents. For each reference,
`1 - cos(v_cond, v_ref)` — how far guidance has to push — plus the distances
**between the references themselves**, and what the guided step does.

The predictions were written into the example's doc comment before it ran: the
identity row must read 0.00%; meaning should outrank length; and where the twin
sits decides whether a negative guides more or less strongly than leaving it
off.

## The answer, at the size the app actually draws

**Grid 32 — 512×512, which is `Request::default()`.** Three seeds, σ 0.7.

```
reference                tokens     mean      min      max
twin (no text)                -    7.29%    5.93%    8.35%
the prompt itself            16    0.00%    0.00%    0.00%
boilerplate                  24    0.13%    0.11%    0.18%
same idea, long              60    0.47%    0.37%    0.58%
different, long              66    0.86%    0.68%    1.10%
```

`boilerplate` is *"blurry, low quality, distorted, deformed, ugly, watermark,
text"* — the negative prompt a person actually types first.

The identity row is 0.00%, so the conditioning reaches the model.

**It responds to the subject.** `different, long` is a mountain range;
`same idea, long` is the same apple written out. Matched in length, opposite in
subject, and the mountain sits **1.8x further** from the prompt. Between
themselves they are **0.34%** apart — real, and in the sensible direction.

**And that is the smaller half of the finding.** The twin sits at **7.29%**.
The strongest negative reaches **0.86%**, 12% of it; the one people write
reaches **0.13%**, **56x weaker than the twin**. What a negative prompt mainly
does is move the reference nearly on top of the prompt, leaving guidance nothing
to extrapolate along:

```
1 - cos(guide(v_cond, v_twin), guide(v_cond, v_neg))   at cfg 4

the prompt itself        20.38%   <-- the baseline: this row IS no guidance
boilerplate              21.07%
same idea, long          22.12%
different, long          23.85%
```

With the negative equal to the prompt, `guide` returns `v_cond` exactly, so
20.38% is the distance between **the twin-guided step and no guidance at all**.

**`boilerplate` lands 0.69 points from that, out of 20.** To within a rounding
error of this instrument, *"blurry, low quality, distorted"* as a negative
prompt **is** turning guidance off. The written-out ones reach 22–24%, still
much nearer the unguided step than the twin's.

**Arithmetic, not measurement**: `guide(c, n, s) = c + (s-1)(c - n)`, so matching
the twin's guidance strength from a negative needs `s - 1` scaled by
`7.29 / 0.86` — about **cfg 25** in place of cfg 4, and about **cfg 170** for
the boilerplate one. Neither has been run and a cfg that large has its own
problems; they are here to give the size of the gap, not as a recommendation.

## Why this pipeline in particular

**This is as much a fact about the twin as about negative prompts.** Most
diffusion pipelines do classifier-free guidance against the *same* model fed an
empty prompt, so their reference is already close to the conditional velocity
and `|c - u|` is small by construction. Ideogram 4 ships a **separately trained**
unconditional model, and that reference sits 7.29% away — far further than any
text this measured.

So swapping the twin for a negative prompt is not "the same guidance, aimed
differently". It is trading an unusually strong reference for an unusually weak
one. The feature is worth having and the twin is worth keeping as the default;
what would be wrong is a control that implied the two were interchangeable.

**One caveat this measure cannot settle.** It reads magnitude, not usefulness
per unit. The twin's correction pushes away from *no text at all* — a generic
direction; a negative's pushes away from something specific, which could matter
more per unit of displacement. That is a real possibility, and 56x is a large
gap for it to close.

## The two runs that answered the wrong question

### Confounded

The first table compared `same idea, long` (60 tokens, the apple) against
`a different picture` (14 tokens, a mountain) and found the first moved the
reference **eleven times further**. That reads as "length beats meaning" — but
those two differ in length *and* subject, in opposite directions, so it
confounded exactly the two things it existed to separate. Same shape as the four
mistakes recorded in `step-count-never-settles-2026-08-24.md`.

### Measured at a size the product does not use

The length-matched control fixed the confound and was run at **grid 16**, which
is **256×256 — half the default**, and the size
`small-images-are-the-model-2026-08-24.md` measured the denoiser to be **2.65x
worse at** than grid 64 before any prompt is involved.

At grid 16, over eight seeds, it said the opposite:

```
                          grid 16        grid 32 (default)
same subject, 16 -> 60 tokens    4.30%           0.47%
different subject, both long     1.28%           0.34%
distance from prompt: same/diff  4.30 / 4.32     0.47 / 0.86
```

**At grid 16 the subject ordering vanishes** — 4.30 against 4.32 is one
measurement twice — and the response is dominated by how much text there is. At
grid 32 the length effect collapses by **9x** and the subject ordering appears.

So the grid-16 result is not a fact about negative prompts. It is another
symptom of the disease `small-images-are-the-model` named: at 256×256 this
denoiser responds to properties of the conditioning that carry no meaning.
**A conditioning question may not be asked at grid 16.** This node nearly
shipped that answer, with a matrix and a verdict.

## What ships, and what it says

`chaos-draw --negative`, with the measurement in its own help text rather than
the phrase every other tool uses:

```
--negative P   guide away from THIS text instead of the untexted twin.
               Measured at 512x512: it follows what you write, and it pulls
               8-56x more weakly than the twin -- a short one such as "blurry,
               low quality" is within 0.7 points of turning guidance OFF.
               Write it out properly, or leave it alone and keep the twin.
               The twin is then not needed on disk at all.
```

It is not the STRUCTURE button, which measured 0.9x — indistinguishable from
nothing — and was not shipped. This does something real and the memory saving is
real. What it does not do is steer as strongly as its name suggests, and a
control that implied otherwise would be the lie
`prompt-shape-does-nothing-2026-08-24.md` exists to retract.

## What this cannot say

- **Whether any picture is better.** Nothing here scores beauty. It says the
  lever is connected, what it is connected to, and how hard it pulls.
- **Anything about a full trajectory.** One timestep at σ 0.7. A 2% difference
  per step could compound over twenty steps or cancel.
- **Anything about grid 64.** The trend from 16 to 32 is large and in one
  direction; 1024×1024 has not been asked.
