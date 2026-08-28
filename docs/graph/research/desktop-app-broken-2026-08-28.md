---
topic: the installed desktop app, reported broken by Atur on 2026-08-27
status: partly resolved — defect 1 fixed and verified, defect 2 measured and open
links:
  - ../backlog/v0-0-3-the-complete-version.md
  - ../reference/hard-won-facts.md
---

# The installed desktop app was broken, and here is which part

Atur, 2026-08-27, on the installed v0.0.21:

> the latest version was very broken — even mode selection got mixed up inside
> the application, even though mode selection happens once and you enter that
> mode. Everything was falling apart.

**Reproduced 2026-08-28 on the installed build, not on a `cargo run`.** Two
defects, one fixed, one open. Neither was found by the existing tests, and one
of them was actively hidden by the instrument that was supposed to find it.

## First: the plan's own description of the shape was wrong

`backlog/v0-0-3-the-complete-version.md` §0b says the desktop "does not work
that way" and that the four roles live only on the CHAOS page inside the running
app. **That is stale.** `gui/app` has had a launch screen the whole time:
`paint_launch`, `ui.launched`, `knob_input`, `back_to_knob`, and a whole
`knob.rs`. The mode *is* asked once, on a launch screen, exactly like the phone.

So the answer to §0b's third deliverable — "decide, and write down, when the
desktop asks for a mode" — is that it already asks at launch. What it did *not*
do is give that screen the window.

## Defect 1 — the knob was painted underneath the running application

**Fixed.** `WM_CREATE` ends with `show_page(Page::Chat)` (main.rs:532), which
makes the CHAT page's controls *and* the whole shell visible. `ui.launched` is
initialised `false` (main.rs:1078), so `WM_PAINT` returns after `paint_launch`
and never paints the rail or the page.

Those two facts are only compatible if you remember that **the controls are real
HWNDs**. Painting the knob does not cover them. So the window opened with the
mode knob painted and the chat transcript, its composer, SEND, CLEAR, the four
rail buttons and STOP floating on top of it. That is "mode selection got mixed up
inside the application", and it is every bit as bad as Atur said.

`back_to_knob` hides all of them on the way *out* — its comment even says
"**Every child window must go, or they float over the knob**" — so the author
knew the hide was required. Only the way *in* was missing it.

### Measured, before and after

Nine controls, checked by client-rect rather than by `IsWindowVisible`, because
`layout` parks the pages a mode cannot reach at `(-3200,-3200)` and leaves them
"visible":

| step | installed v0.0.21 | after the fix |
|---|---|---|
| fresh open | **9 on-screen** | **0 on-screen** |
| ESC | 9 (no effect) | 0 (no effect) |
| RETURN — enter the mode | 9 | **9** |
| ESC once launched | 0 | 0 |

**ESC having no effect on open is the proof that `launched` was false**, because
`WM_KEYDOWN if launched() && VK_ESCAPE` is the only thing that calls
`back_to_knob`. Nine controls on-screen while ESC does nothing cannot happen
unless the knob owns the paint and not the children.

### The fix

A `!launched()` guard inside `show_page` itself, not at the startup call site,
because every route in has to be covered: startup, the rail, the menu, Ctrl+1..6
and a `WM_COMMAND` from a script. `back_to_knob`'s inline hiding became
`hide_every_control()`, shared by both routes — the two disagreed, and the one
missing the hide was the one that ran at startup.

Regression test: `the_knob_owns_the_screen_before_a_mode_is_chosen` in
`gui/app/tests/ui_rules.rs`, which asserts the guard and its `return` both come
before the first `SW_SHOW`, and that both routes hide through one function.

## Defect 2 — the CHAOS page arrives blank. OPEN

**Not caused by the fix above: the installed v0.0.21 does it too.**

On arriving at the CHAOS page, `ID_CORE_ADDR` (764), `ID_CORE_KEY` (765) and
`ID_CHAOS_STATUS` (769) are all **empty**, for `role = client` and for
`role = alone` alike. `fill_chaos_fields` writes all three unconditionally and
has no early return, and for `Alone` it should put `127.0.0.1:8231` in 764 and a
paragraph of guidance in 769.

**This is the page that exists to tell you what to type into your phone, and it
says nothing.**

Proved rather than inferred. A marker was written into 764 and 769 from outside
the process, then the app navigated CHAOS → CHAT → CHAOS:

```
marker written : 764='CLAUDE-PROBE-764'  769='CLAUDE-PROBE-769'
after away+back: 764='CLAUDE-PROBE-764'  769='CLAUDE-PROBE-769'
```

For `role = client` the fill would have written `""` into 764 and wiped the
marker. **The marker survived, so `fill_chaos_fields` never ran** — and nothing
on the tick timer writes those controls either, or the marker would have gone.

What makes this puzzling, and what the next session should start from: the page
*was* current when this was measured — 9 of 9 CHAOS controls on-screen, 0 of 4
CHAT controls — and only `show_page`'s `for q in nav::PAGES` loop reveals them.
The very same function body then runs `if p == Page::Chaos { fill_chaos_fields();
}` twenty lines later (main.rs:1231). Both cannot be true as written, so one of
the assumptions behind them is wrong. Candidates not yet eliminated:

- `main_hwnd()` inside the process is not the `ChaosAppWindow` a probe finds, so
  `ctl(id)` is null while `GetDlgItem` from outside is not — but then
  `ShowWindow(ctl(id))` could not have revealed the page either;
- a second control carries one of these ids;
- the running binary is not built from the source being read.

The cause is **not** identified. Do not fix it by guessing; instrument it.

## What this says about the instruments

`scripts/run-through.ps1` reported a **clean pass over the broken app** — 22
controls exercised, worst blocking call 48.5 ms, "nothing blocked the window".
It drives pages with `WM_COMMAND`, which goes neither through the rail nor
through the knob, so it walked an application that had never left its launch
screen and never noticed.

That is the sharper version of a trap already in `hard-won-facts.md`: an exit
code is not a diff, **and a green transcript is not a working window.** Two
changes were made:

- it presses RETURN first and **stops** if no rail button is on-screen
  afterwards, rather than reporting HIDDEN for everything;
- it covers the **CHAOS page**, which it never did — the page holding the mode
  controls and the two brand buttons that §1 records as never clicked.

The four role buttons and NEW KEY are listed rather than pressed, because
pressing them reconfigures the machine being inspected. SHOW THE MARK and READ A
CODE are opt-in behind `-Brand` because they open a browser.

**The two brand buttons have now been clicked**, which closes §1's fourth
unverified item. With no core address set they correctly open nothing and take
3.8 ms and 0.5 ms; `open_brand_page` refuses early with "no address yet — choose
a role on this page first". That message goes to the painted strip, not to
control 769, so it cannot be read by a rectangle probe — worth knowing before
someone reads its absence as a second bug.
