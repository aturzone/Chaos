# Checklist

**The one page that says what is done and what is not.** Updated in the same
commit as anything that moves an item; shown to Atur every time.

Legend: **[x]** done and verified · **[~]** partly done, the gap is named ·
**[ ]** not started · **[!]** blocked or closed, with the reason

---

## A. The launch experience — the mode knob

| | item | state |
|---|---|---|
| A1 | `assets/knob.svg` drawn, Chaos logo at its centre | **[x]** 70 KB, logo embedded not redrawn |
| A2 | `assets/mode-dial.svg` — four detents across a 180° top sweep | **[x]** no dark plate, transparent, knob inlined |
| A3 | `tools/make-mode-dial.py` regenerates both | **[x]** |
| A4 | **Atur has looked at them and they are right** | **[x]** approved 2026-08-26 |
| A5 | Logo animation on launch, from `logo.svg` | **[ ]** the knob is up; the animation before it is not |
| A6 | Knob drawn natively on Windows | **[x]** `knob.rs`, scan-converted per pixel, 6 tests |
| A7 | Turning + pressing selects a mode and enters the shell | **[x]** drag, arrows, ENTER — verified on the live window |
| A8 | Mode changeable again without restart | **[x]** ESC returns to the knob, 10 ms |
| A9 | Shell shows only that mode's controls | **[x]** HELPER hides CHAT/MODELS/IMAGE, checked on screen |
| A10 | Same on Android (`Canvas`) | **[ ]** |
| A11 | `poke-app.ps1` drives all four detents, no call over ~50 ms | **[ ]** |

**A4 is the gate.** There is no gradient-capable SVG renderer on this machine —
no cairosvg, no PIL, no rsvg-convert, no Inkscape — and this session has no
composited display. **Open `assets/mode-dial.svg` in a browser.**

---

## B. Full options on every platform

| | item | state |
|---|---|---|
| B1 | Windows: Chat, Models, Image, Monitor, Settings, CHAOS | **[x]** |
| B2 | CHAOS roles: ALONE / CORE / HELPER / CLIENT | **[x]** shipped v0.0.18 |
| B3 | A CORE is reachable from a phone | **[x]** the `--host` fix, v0.0.18 |
| B4 | Android: client + CHAOS section | **[x]** v0.0.18 |
| B5 | Android: the engine runs in-process | **[x]** JNI bridge, v0.0.20 |
| B6 | Android: model picker, Image, Monitor, Settings | **[ ]** all already served by the API |
| B7 | Android Phase B: model files on device + token loop | **[ ]** |
| B8 | **HELPER actually does work** — a CORE routes experts to it | **[ ]** protocol done, routing not |
| B9 | macOS: a window | **[!]** none exists; CLI + CORE is the route today |
| B10 | Linux: a window | **[!]** none exists; CLI + CORE is the route today |

---

## C. Speed — the 5 tok/s ladder

Target set by Atur after 20 tok/s was measured closed. Plan:
`docs/graph/backlog/the-big-bang-5-tok-s.md`.

| | item | state |
|---|---|---|
| C0 | Machine bandwidth measured — **30.8 GiB/s** | **[x]** `chaos-membench` |
| C1 | The law `tok/s ≈ 19 / resident GiB`, nine models | **[x]** |
| C2 | Phase 0a: queue-depth benchmark, QD 1→16 | **[ ]** decides C4 |
| C3 | Phase 0b: gate mass carried by experts 4–6 | **[ ]** decides C7 |
| C4 | Rung 1 — concurrent expert reads → ~0.96 tok/s | **[ ]** |
| C5 | **Quality harness** — logit diff + checkable prompts | **[ ]** ← *gates C6 and C7* |
| C6 | Rung 2 — 2-bit experts → ~2.2 tok/s | **[ ]** |
| C7 | Rung 3 — top-3 routing → ~5.2 tok/s | **[ ]** |
| C8 | 20 tok/s on V4-Flash on this laptop | **[!]** needs 64.4 GiB/s against 30.8 |
| C9 | Vulkan generation is 2.2× slower than CPU — why? | **[ ]** real, measured, unexplained |

**C5 is not optional.** C6 and C7 change what the model computes, and a wrong
forward pass here is fluent nonsense, never a crash.

---

## D. Shipped

| | item | state |
|---|---|---|
| D1 | v0.0.18 — CHAOS page, the `--host` fix | **[x]** verified from published files |
| D2 | v0.0.19 — Chaos builds and runs on Android | **[x]** |
| D3 | v0.0.20 — engine inside the app, in the published APK | **[x]** `libchaos_android.so` confirmed inside |
| D4 | README: version badge, the speed claims | **[x]** badge was 14 releases stale |
| D5 | Install → update → uninstall, models untouched | **[x]** every release |
| D6 | The APK on **real hardware** | **[ ]** emulator only — Atur's phone |
| D7 | Image: step-count default | **[ ]** Atur's aesthetic call |

---

## Next three

1. **A4** — Atur opens `assets/mode-dial.svg` and says yes or what to change.
2. **C2 + C3** — one day, no engine code, and they decide whether the ladder
   reaches 5 tok/s or stops at ~2.2.
3. **A5–A9** — the launch flow on Windows, which proves the design before
   Android repeats it.
