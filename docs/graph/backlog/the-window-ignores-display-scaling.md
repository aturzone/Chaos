---
topic: The window declares per-monitor DPI awareness and then never scales anything, so on a 125% display every control and every font is drawn about 20% smaller than designed — the single largest reason the interface does not look like other applications on the same desktop
status: FOUND 2026-09-07, design written, NOT DONE. It is the next UI change and it must be done in one piece.
links:
  - ../reference/hard-won-facts.md
  - chaos-window-design.md
---

# The window ignores display scaling

**Atur, 2026-09-07**: *"you worked the appearance much better; it's not like
Windows 98 — make the appearance much more professional"*, and *"I want you to
check the app pixel by pixel yourself"*.

This is what the pixels say, and it is one defect rather than a hundred.

## What is wrong

`become_dpi_aware()` asks Windows for `PER_MONITOR_AWARE_V2` before any window
exists, correctly and early. **Declaring awareness moves the responsibility to
the application; it does not discharge it.** Nothing then scales:

- `theme::metric` is eleven raw `i32` constants — `RAIL = 208`, `BUTTON = 32`,
  `INSET = 28` — used directly as physical pixels at about 150 call sites.
- `theme::size` is six raw font heights — `BODY = -15`, `HEADING = -19` — passed
  straight to `CreateFontW` in `make_font`.

This machine's window reports **120 DPI, a 1.25x scale**. So a button designed
as 32 pixels at 96 DPI is drawn 32 physical pixels: **20% smaller than
intended**, and 33% smaller at 150%, 50% at 200%. Every other window on the
desktop scales; this one does not, and that difference is most of what "looks
unprofessional" means.

## Why it must be done in one piece

**Scaling the fonts alone makes it worse.** 15px text in a 32px button is
comfortable; 19px text in the same 32px button overflows it. Metrics and fonts
have to move together or the result is worse than doing nothing.

## The design, which is two conversions rather than 150 edits

The obvious approach — turn every `metric::X` into `metric::x()` and multiply —
touches every call site and every paint function. There is a much smaller one,
because **all geometry derives from a single input**: the client rect from
`GetClientRect`.

1. Read the scale once per window, from `GetDpiForWindow(hwnd) / 96.0`, and keep
   it beside the theme. It changes when the window moves between monitors, so
   `WM_DPICHANGED` must update it and re-run the layout.
2. In `layout()`, divide the client rect **into design space** at the top. Every
   existing line then computes in design units against unchanged constants, and
   nothing in the body needs touching.
3. Multiply every rectangle in `m` **back to physical** at the bottom, in the
   one loop that already walks it.
4. `make_font` multiplies its `px` argument by the same scale. One function,
   every font.

**The paint functions are the part to be careful with**, and the reason this is
not a five-minute change: `paint_rail`, `paint_strip` and the page painters use
`metric::*` directly against the *physical* client rect. Either they take the
same into-design-space conversion at entry, or the rail's painted background
lands 20% away from the rail's buttons — which is worse than the bug.

## How to know it worked

At 120 DPI a `metric::BUTTON` control should measure **40 physical pixels**, not
32. **Do not measure it from PowerShell**: that process is DPI-unaware, so
Windows virtualises every coordinate it reads back and a 32px button already
comes back as 26. Three attempts at an external check produced three sets of
confident wrong numbers before that was understood —
`reference/hard-won-facts.md`.

Measure it from inside instead, or better: **make the layout a pure function**
— client rect and page in, rectangles out — and assert in Rust, at several DPI
scales and several window sizes, that nothing overflows and nothing overlaps.
That needs no window, no marshalling, and runs in CI. It is the instrument this
work should be built on, and it does not exist yet.
