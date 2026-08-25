---
topic: the launch experience — a logo animation, then a gas-stove knob that picks the mode
status: open
links:
  - the-big-bang-5-tok-s.md
  - android-app.md
  - ../reference/hard-won-facts.md
---

# The mode knob

Atur, 2026-08-25:

> the app first page — from now we need a loading svg animation with logo,
> exactly a beautiful one. And after that I want you to make a screw like the
> old gas stove screw, with high detail and quality, and a three-dimensional
> appearance, where the user first selects the mode choice … and then the app is
> launched based on the mode they choose. There should also be an option to
> change the mode … This way, the first mode is selected and the additional
> options are not all messy.

**The problem it solves is real.** The window currently opens on six pages of
controls with no idea what the person wants to do. The CHAOS page already asks
the only question that changes everything else — *what is this machine to the
others* — and it asks it fifth, in a corner.

**And the four roles are already four detents.** A stove knob has positions; so
does this:

```
        ALONE                CORE
          \                   /
           \    ( knob )     /
           /                 \
          /                   \
       HELPER               CLIENT
```

| detent | what the app becomes |
|---|---|
| **ALONE** | everything local, `127.0.0.1`, no route in |
| **CORE** | holds the models and answers; address + key shown |
| **HELPER** | lends memory and cores to a CORE |
| **CLIENT** | uses a CORE elsewhere, loads nothing here |

## The flow

1. **Splash** — the logo, animated, while the model catalogue scan and the
   hardware probe run. Those already take ~1 s on a cold start, so the animation
   covers real work rather than being a delay added for decoration.
2. **The knob** — four detents, turn to choose, press to enter.
3. **The app**, showing only what that mode can do. A CLIENT has no cache or
   thread settings, because those decide how a model *runs* and belong to
   whichever machine runs it.
4. **Back to the knob** from a control in the shell, always reachable.

## The logo animation

**`assets/logo.svg` is Atur's mark and is never substituted.** The animation
animates *it* — fade, scale, a mask sweep — and does not redraw it, does not
approximate it, and does not replace it with something generated. This rule was
bought once already; see `reference/hard-won-facts.md`.

## Two hard constraints found while drawing it

### 1. The knob cannot go through the existing SVG pipeline

`tools/rasterise-logo.py` is the project's own rasteriser and it implements
**exactly the subset `logo.svg` needs**: closed `M`/`C`/`Z` paths, solid fills,
`translate`. Its docstring says so and says why — this workspace has no
dependencies and the banner was not going to be the first one.

`assets/knob.svg` needs **radial and linear gradients, `<circle>`, `<rect rx>`,
`<ellipse>`, and `<use transform="rotate()">`**. None of that is in the subset,
and a gradient is not an optional detail here: **the chamfer ring and the
specular arc are what make it read as three-dimensional at all.**

So there are three routes and only one is honest about cost:

| route | cost |
|---|---|
| extend the rasteriser to gradients, circles, rects and transforms | a real 2-D renderer, written from scratch, to draw one control |
| pre-render frames with an external tool | an external dependency the project does not have, and none is installed here |
| **draw it natively per platform** | GDI+ on Windows (a system DLL, like `user32`), `Canvas` on Android — both have radial gradients as primitives |

**Take the third.** `assets/knob.svg` is then the *specification* — the
geometry, the radii, the colour stops — and each app renders it with its own
2-D API. A rotating control wants to be drawn, not blitted, anyway: pre-rendered
frames are either coarse or enormous.

### 1b. The centre is the logo, embedded

Atur: *"instead of that circle in the middle, put the Chaos logo."* The centre
boss is now `assets/logo.svg` itself — all 43 paths, scaled and clipped to a
circle, **embedded rather than redrawn**. The mark is his; it is never
approximated, regenerated or replaced, and `tools/make-mode-dial.py` reads the
real file every time it builds.

The logo is a black mark on white, so it needs no recolouring to sit on a white
boss: it reads as a moulded cap, which is what a stove knob's centre badge is.

### 2. Nobody has seen it

There is **no SVG renderer on this machine that supports gradients** — no
`cairosvg`, no PIL, no `rsvg-convert`, no Inkscape — and this session has no
composited display, which is the same wall that made `shot-app.ps1` return
solid black.

**So the knob has been drawn and not looked at.** It is valid XML with the
intended structure — 48 knurl ridges, 8 circles, 6 gradients, a recessed
pointer, a centre boss — and whether it is *beautiful*, which is the actual
requirement, is unverified. **Atur opens `assets/knob.svg` in a browser and
says.** That is a five-second check for him and an impossible one here, and
shipping artwork neither of us has looked at is how a "high detail and quality"
requirement quietly becomes a grey circle.

## The plate

`assets/mode-dial.svg` is the whole control as the launch screen shows it: a
dark plate, four detents, labels, and the knob turned to CORE. **The knob is
inlined into it, not referenced** — a browser will not load a local file through
`<image href>`, so a referenced knob shows as an empty box, which would have
wrecked exactly the review this file exists for.

**12 o'clock is CORE and the detents are 90 degrees apart, so the pointer's
angle *is* the mode** and nothing has to be looked up:

```
              CORE
               |
    ALONE  ---(*)---  CLIENT
               |
             HELPER
```

## What has to be built, per platform

| platform | splash | knob | mode-gated shell |
|---|---|---|---|
| **Windows** | animate `logo.svg` in the existing owner-drawn paint | GDI+ | the six pages exist; gate them |
| **Android** | `Canvas`, or an `AnimatedVectorDrawable` | `Canvas` | the pages do not exist yet |
| **macOS** | — | — | **there is no window at all** |
| **Linux** | — | — | **there is no window at all** |

**The honest note about "all apps".** `gui/app` is raw Win32. macOS and Linux
have `chaos-run` and `chaos-serve` and no GUI whatsoever, so "the knob in every
app" means writing two new native front-ends before it can be true there. The
route that gets full function to every device *without* that is the one already
shipped: every platform runs `chaos-serve`, and the knob's CLIENT detent points
at a CORE.

## Order

1. **Atur looks at `knob.svg`** and it is right, or it is iterated until it is.
2. **Windows**: splash, knob, mode-gated shell. One platform proves the design.
3. **Android**: the same three, plus the pages it is missing.
4. **macOS / Linux**: only if a native window is really wanted over CLI + CORE,
   and that is a decision, not a task.

## Definition of done

- Launching shows the logo animating, then the knob, and nothing else.
- Turning to each of the four detents and pressing enters a shell showing only
  that mode's controls.
- The mode can be changed again without restarting.
- The choice persists, and `settings.txt` already has `role` for it.
- `poke-app.ps1` drives all four detents with no blocking call over ~50 ms.
