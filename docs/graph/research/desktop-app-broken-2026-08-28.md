---
topic: the installed desktop app, reported broken by Atur on 2026-08-27
status: resolved — one real defect, fixed and verified; the "second defect" was my own instrument and is retracted
links:
  - ../backlog/v0-0-3-the-complete-version.md
  - ../reference/hard-won-facts.md
---

# The installed desktop app was broken, and here is which part

Atur, 2026-08-27, on the installed v0.0.21:

> the latest version was very broken — even mode selection got mixed up inside
> the application, even though mode selection happens once and you enter that
> mode. Everything was falling apart.

**Reproduced 2026-08-28 on the installed build, not on a `cargo run`.** **One
real defect**, fixed and verified. A second was reported here and is now
**retracted**: it was a mistake in the probe, not in the app. Both halves of that
are the same lesson — the existing instrument hid the real defect by bypassing
the launch screen, and my new probe invented a false one by reading a control the
wrong way.

## First: the plan's own description of the shape was wrong

`backlog/v0-0-3-the-complete-version.md` §0b says the desktop "does not work
that way" and that the four roles live only on the CHAOS page inside the running
app. **That is stale.** `gui/app` has had a launch screen the whole time:
`paint_launch`, `ui.launched`, `knob_input`, `back_to_knob`, and a whole
`knob.rs`. The mode *is* asked once, on a launch screen, exactly like the phone.

So the answer to §0b's third deliverable — "decide, and write down, when the
desktop asks for a mode" — is that it already asks at launch. What it did *not*
do is give that screen the window.

**The remaining decision, made by Atur on 2026-08-28: asked once, then
remembered.** The knob showed on every launch because `launched` started `false`
and nothing consulted the saved `role`. A new `mode_chosen` setting carries the
answer — `role` cannot, because its default is a real role, so a machine nobody
asked reads identically to one whose owner chose ALONE. Measured on the real
window: first launch 0 controls on-screen and the file gains `mode_chosen =
true`; second launch 9 controls, straight into the saved mode; ESC returns to the
knob in both. An existing settings.txt has no such key, so everyone upgrading is
asked exactly once more — the intended migration, not a bug.

That change forced a second one. The window opened on `Page::Chat`
unconditionally, and `pages_for(Helper)` has no CHAT — so a *remembered* HELPER
would have opened a page its own rail cannot reach, the same mismatch as the knob
under the shell arriving by the other door. Startup now takes the first page the
mode can reach, which is CHAT for three modes of four.

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

## RETRACTED — "the CHAOS page arrives blank" was my probe, not the app

**There is no second defect. The CHAOS page works.** This section is kept rather
than deleted because the mistake is more instructive than the non-bug.

What was reported: arriving at the CHAOS page, `ID_CORE_ADDR` (764),
`ID_CORE_KEY` (765) and `ID_CHAOS_STATUS` (769) all read empty, for
`role = client` and `role = alone` alike, in the installed v0.0.21 as well as in
a fresh build. A marker written into those fields from outside the process
**survived** navigating CHAOS → CHAT → CHAOS, which was taken as proof that
`fill_chaos_fields` never ran.

Every one of those observations was real. The conclusion was wrong.

### What the instrumentation said

A file trace in `fill_chaos_fields` and at `show_page`'s call to it, run against
an isolated profile (`USERPROFILE` pointed at a throwaway `.chaos`, so nothing of
Atur's was touched):

```
show_page: p=Chat is_chaos=false
show_page: p=Chaos is_chaos=true
fill_chaos_fields: entered
fill_chaos_fields: role=Alone addr="127.0.0.1:8231" status_len=105 main_hwnd=0x708f0 ctl769=0x208ba ctl764=0x208ac
fill_chaos_fields: after write, 769 reads "Nothing outside this machine can reach it. Choose CORE to let a phone or another computer use this model."
```

It runs, it computes the right strings, and **it reads its own write back
correctly from inside the process**. Two candidates from the earlier version of
this node were eliminated first, and cheaply, from outside: there is exactly
**one** top-level `ChaosAppWindow` (so `main_hwnd()` cannot be a different
window), and among 56 child windows there are **no duplicate control ids**.

### The actual cause: `GetWindowText` cross-process reads a caption

**`GetWindowTextW` called from another process does not send `WM_GETTEXT`.** That
is documented and deliberate — it stops a hung target from hanging the caller —
and the consequence is that it returns only the window's *caption*. A BUTTON's
label **is** its caption, so every button in every earlier transcript read
correctly and nothing looked wrong. An EDIT's text is not its caption, so every
EDIT in another process reads as the empty string no matter what is in it.

Read the same three controls with `WM_GETTEXT` and they were never blank:

| id | `GetWindowTextW` cross-process | `SendMessageW(WM_GETTEXT)` |
|---|---|---|
| 764 | len 0 | `127.0.0.1:8231` |
| 765 | len 0 | the 48-character key |
| 769 | len 0 | 105 characters of guidance |

And the marker "surviving" has the same explanation: a cross-process
`SetWindowTextW` marshals through USER32 and *does* set the caption, so the probe
could read back its own write — while the app's in-process write went to the edit
buffer, where the probe could never see it. The probe was reading a field only
the probe had ever written.

`scripts/run-through.ps1` had the same bug in its `TextOf`, which is why 764, 765
and 769 printed as blank labels in its CHAOS transcript. It now sends
`WM_GETTEXT` and falls back to the caption, and the transcript shows the real
address, key and guidance.

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
