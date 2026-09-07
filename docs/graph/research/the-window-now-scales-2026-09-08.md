---
topic: The window declared per-monitor DPI awareness for eight releases and then scaled nothing, so on any scaled display every control and every glyph was drawn smaller than designed. Now it scales, in one piece, with the check that proves it living inside the process rather than outside.
status: resolved 2026-09-08. Verified at 120 DPI across all six pages; the instrument is `gui/app/src/placement.rs` and `CHAOS_LAYOUT_DUMP`.
links:
  - ../reference/hard-won-facts.md
  - ../backlog/app-redesign.md
---

# The window now scales

**Atur, 2026-09-07**: *"you worked the appearance much better; it's not like
Windows 98 — make the appearance much more professional"*, and *"I want you to
check the app pixel by pixel yourself"*.

This was the pixel answer, and it was one defect rather than a hundred.

## What was wrong

`become_dpi_aware()` asked Windows for `PER_MONITOR_AWARE_V2` before any window
existed, correctly and early. **Declaring awareness moves the responsibility to
the application; it does not discharge it.** Nothing then scaled: `theme::metric`
was eleven raw `i32` constants and `theme::size` six raw font heights, used as
physical pixels at 114 call sites.

This laptop reports **120 DPI, a 1.25x scale**. So a button designed as 32
pixels was drawn 32 physical pixels — 20% smaller than intended, 33% at 150%,
half size at 200%. Every other window on the desktop scaled; this one did not,
and that difference is most of what "looks unprofessional" means.

## The design: two conversions, not 150 edits

All geometry derives from **one** input, the client rect, and reaches the screen
through a handful of exits. So the window computes entirely in *design units* —
the numbers already written in `theme::metric`, meaningful at 96 DPI — and
converts only at those exits. Nothing in between changed, which is why the
literal offsets scattered through `layout()` are still correct.

`chaos_app::scale::Scale` holds a DPI and does integer arithmetic:

| Exit | What changed |
|---|---|
| `layout()` | client rect into design units at the top; every rectangle back to pixels in the apply loop |
| `paint()` | client rect into design units; the buffer and the blit stay physical |
| `text`, `fill` | the two functions that touch `DrawTextW` and `FillRect` — one conversion each, so `label`, `rule` and `frame` needed none |
| `make_font` | the height, by magnitude: `-15` becomes `-19`, not `-18` |
| `StretchDIBits` ×2 | destination converted, and the mark **rasterised at the physical size** rather than blitted up from 96 DPI |
| `draw_item` | `rcItem` into design units once, so `draw_combo` and `draw_list_row` needed none |
| `text_width` | returns design units, because every caller adds a design-unit pad to it |
| item heights, `EM_SETMARGINS`, `WM_GETMINMAXINFO`, `opening_geometry` | converted at the call |

**Three things were nearly missed and each would have been visible.**
`text_width` measures with a physical font and its answer was being compared
against design-unit widths. `rcItem` arrives physical. And the two logo blits
would have drawn a 96-DPI raster into a 120-DPI box — sharp arithmetic, blurry
mark.

`WM_DPICHANGED` takes the rectangle Windows suggests, rebuilds every font,
re-applies the item heights and re-runs the layout. The old fonts are deleted
**after** the new ones are in place: a control holding a deleted `HFONT` draws
with the system default until its next `WM_SETFONT`.

## How it was verified

Not by looking. **A screen grab is uniform black on this machine**, and the
external route was already known to lie: `powershell.exe` is DPI-unaware, so
Windows virtualises every coordinate it reads back from an aware window — a
32-pixel button came back as 26, and three attempts at an external check
produced three sets of confident wrong numbers.

So the check moved inside. `gui/app/src/placement.rs` is a pure function over
rectangles — no window, no marshalling, no display scale — that reports a
control off an edge, overlapping another, or too small to hit. It has ten of its
own tests. `layout()` hands it the list it is about to apply, and
`CHAOS_LAYOUT_DUMP` writes the result to a file: page, DPI, client rect, and
every control in both design units and pixels.

Driving all six pages with `scripts/run-through.ps1`:

```
page Chaos  dpi 120  client 1166x718 du  (1458x898 px)  controls 16
   760  du   236, 134  200x32    px   295, 168  250x40
   764  du   236, 182  360x32    px   295, 228  450x40
   ...
  clean
```

**A `metric::BUTTON` control measures 40 physical pixels at 120 DPI**, which is
exactly the number the design said to check for. Nine layout passes across six
pages, all clean; the run-through pressed 34 controls with nothing blocking the
UI thread longer than **35.8 ms** -- the worst of six runs, the others 18.5,
19.1, 20.6, 24.0 and 25.1 ms. Quoting the worst rather than the last, because
the last one is whichever number the machine happened to produce.

## Resizing, which is where it really earned its keep

Atur also asked for the window to be responsive — *"something like Telegram"*.
The dump above is one window size. Driving the window through **five sizes and
all six pages** — 43 layouts — found three defects that a single size hides
completely, and every one of them is reachable by dragging a corner:

- **IMAGE: DRAW and STOP slid on top of the guidance dropdown.** They were
  pinned to the right edge at `x + w - 250`, which walks *left* as the window
  narrows, and below about 800 units of content width it walks into the
  controls beside it. They wrap to their own row now.
- **SETTINGS: SAVE and RESET were drawn through the last field.** The buttons
  sat at a fixed offset from the page bottom while the form grew from the top,
  so on a short window the two met. They follow the form now, and the form is
  measured by **visible** row heights — a combo is laid out at its dropped
  height and appears at `metric::CONTROL`, so the naive measurement would have
  put the buttons a hundred units below anything visible.
- **`MIN_H` was 60 units too small**, which is the one worth stating plainly:
  **the window enforced a minimum size at which its own tallest page did not
  fit.** Fixing the two overlaps above did not make the content fit, it made
  the layout stop lying about it — SAVE and RESET moved from *on top of* a
  field to two units *below the strip*, where the check could see them. 680.

**43 layouts, five window sizes, six pages, no problems.** That is the claim,
and it is reproducible: `CHAOS_LAYOUT_DUMP`, then drive `MoveWindow` and the
rail through whatever range is interesting.

**One trap in the driving script, not the app.** `FindWindowW` declared through
`Add-Type` without `CharSet=CharSet.Unicode` marshals its class name as ANSI
and silently returns null — indistinguishable from "the app is not running".
`run-through.ps1` had it right and the new script did not.

## Two findings from the instrument's first run

**It reported the STOP button off the bottom edge on all six pages** — and it
was the *check* that was wrong. The strip along the bottom is reserved from
pages, and the strip's own control is entitled to it. `Placed::chrome` is that
distinction, and both halves are now tests: the strip's button is fine there,
a page control in the same place is not. A check that cries wolf six times per
run is a check nobody reads.

**It found a genuine design defect the eye had missed**: `ID_CLAUDE_CODE` was
laid out at the full content width, 902 design units — an 1128-pixel bar beside
buttons of 92 and 200. It was the only full-width button in the app. Now 260,
still the widest because it is the page's primary action.
