# Checklist

**The one page that says what is done and what is not.** Updated in the same commit as
anything that moves an item.

Legend: **[x]** done and verified · **[~]** partly done, the gap is named ·
**[ ]** not started · **[!]** blocked or closed, with the reason

> **Rewritten 2026-08-31.** This file was one of four scoreboards that disagreed in 22
> places. It also disagreed with *itself*: A4 was ticked and simultaneously listed as
> "the gate" and as the next thing to do, and B7 claimed a model ran on a phone "with no
> PC involved" while D6 recorded that only an emulator had ever run it. Both are fixed
> below. `STATUS.md` is the scoreboard; this is the tick-list; `docs/graph/` holds the
> reasoning.

---

## The release ladder — v0.0.24 → v0.0.30

**v0.0.30 is the release built to LTS standard, and nothing is tagged until its gate is
green.** Full contents in `STATUS.md`.

| | release | state |
|---|---|---|
| L1 | **v0.0.24 — One truth**: one scoreboard, 22 contradictions resolved, dead epics retired | **[x]** merged #151. STATUS 5,144 -> 205 lines, 26 dead tickets retired |
| L2 | **v0.0.25 — Guard the binary**: CI runs the correctness suite against a real model | **[x]** merged #152. E4, E6, E7, E8, E9 |
| L3 | **v0.0.26 — Measure before optimising** | **[x]** merged #153-155. `F` is **93% arithmetic** and the router is 31% of it; the expert read is **2.88 GiB/s**, already 84% of the drive; the 3x GPU contradiction is resolved. **Three claims corrected, two of them mine** |
| L4 | **v0.0.27 — Quality harness**, then the levers | **[~]** the harness is in (C6). Next: **C5c the router**, behind the *exact* bar |
| L5 | **v0.0.28 — Any machine, any model**: quant selection, self-configuration | **[~]** `--auto` exists, T3/T4 open |
| L6 | **v0.0.29 — Every platform, actually run**: 4 of 9 assets never executed | **[ ]** |
| L7 | **v0.0.30 — LTS**: the parity gate, the competitive claim, a support policy | **[ ]** |

---

## A. The launch experience — the mode knob

| | item | state |
|---|---|---|
| A1 | `assets/knob.svg` drawn, Chaos logo at its centre | **[x]** 70 KB, logo embedded not redrawn |
| A2 | `assets/mode-dial.svg` — four detents across a 180° top sweep | **[x]** no dark plate, transparent, knob inlined |
| A3 | `tools/make-mode-dial.py` regenerates both | **[x]** |
| A4 | **Atur has looked at them and they are right** | **[x]** approved 2026-08-26 |
| A5 | Logo animation on launch, from `logo.svg` | **[x]** fade + settle, skippable, first frame 8 ms |
| A6 | Knob drawn natively on Windows | **[x]** `knob.rs`, scan-converted per pixel, 6 tests |
| A7 | Turning + pressing selects a mode and enters the shell | **[x]** drag, arrows, ENTER — verified on the live window |
| A8 | Mode changeable again without restart | **[x]** ESC returns to the knob, and **asks first** since v0.0.22 |
| A9 | Shell shows only that mode's controls | **[x]** HELPER hides CHAT/MODELS/IMAGE, checked on screen |
| A10 | Same on Android (`Canvas`) | **[x]** the dial is the launcher; turn, enter, remembered |
| A11 | The launch flow driven on the live window | **[x]** splash 8/1/1 ms, knob 59-70 ms, ESC 10 ms |
| A12 | The mode is asked **once** and remembered | **[x]** `mode_chosen`, v0.0.22; CHAOS left the rail for a badge at its foot |

**A4 was the gate and is closed.** The paragraph telling a future session to open the
SVG in a browser stood for five days after Atur approved it, and A4 stayed at the top of
"Next three" the whole time — a ticked box that still read as blocking.

## A-bis. What Atur reported on 2026-08-26, and what it cost

| | report | state |
|---|---|---|
| R1 | "where is logo in center of that" | **[x]** the badge was WHITE ink on a WHITE knob face — invisible. Dark ink now, 825 dark pixels where there were 0 |
| R2 | "the letters of modes need to be better" | **[x]** `0xFF111111` on `#0D1117` — near-black on near-black. Theme colours now; ALONE was also clipped off the left edge |
| R3 | "circle of choose mode need sound" | **[x]** a system click and a clock-tick haptic per detent crossed, not per pixel dragged |
| R4 | **"android can not do any one of works in windows"** | **[~]** ALONE and CORE run the real engine on the phone tier. HELPER is still reserved, and only an emulator has ever run it — see B7/D6 |

**How R1–R3 were missed.** The knob was tested for *speed* (8 ms a frame), *input*
(arrows, ENTER, ESC, taps) and *structure* (which labels exist in the view hierarchy).
**Nobody looked at the pixels.** A screenshot decoded and sampled catches all three in a
minute, and that is now how it is checked.

## B. Full options on every platform

| | item | state |
|---|---|---|
| B1 | Windows: Chat, Models, Image, Monitor, Settings, CHAOS | **[x]** |
| B2 | CHAOS roles: ALONE / CORE / HELPER / CLIENT | **[x]** shipped v0.0.18 |
| B3 | A CORE is reachable from a phone | **[x]** the `--host` fix, v0.0.18; re-verified 2026-08-31 over the LAN |
| B4 | Android: client + CHAOS section | **[x]** v0.0.18 |
| B5 | Android: the engine runs in-process | **[x]** JNI bridge, v0.0.20 |
| B6 | Android: model picker, Image, Monitor, Settings | **[~]** the mode drives the screen and its text; the extra pages are not built |
| B7 | Android Phase B: model files on device + token loop | **[~]** **7 tokens at 3.94 tok/s with no server involved — on an Android 34 emulator.** The engine and the loop are real; the hardware is not. Real hardware is D6 |
| B8 | **HELPER actually does work** — a CORE routes experts to it | **[ ]** protocol done, routing not |
| B9 | macOS: a window | **[!]** none exists; CLI + CORE is the route today |
| B10 | Linux: a window | **[!]** none exists; CLI + CORE is the route today |
| B11 | macOS or Linux has run a model **at all** | **[ ]** they build and pass unit tests in CI; no model has ever been run on either, and macOS has no direct-I/O path |

## C. Speed — the ladder to 5 tok/s

Plan and the corrected arithmetic: `docs/graph/backlog/the-big-bang-5-tok-s.md`.

| | item | state |
|---|---|---|
| C0 | Machine bandwidth measured — **30.8 GiB/s** peak, 17.9 at one thread | **[x]** `chaos-membench` |
| C1 | The law `tok/s ≈ 19 / resident GiB`, nine models over 23× | **[x]** |
| C2 | Queue-depth benchmark, QD 1→16 | **[x]** `chaos-qdbench`, 2026-08-26: 1.34 / 2.07 / 3.20 / **3.41** / 3.36 GiB/s |
| C3 | Gate mass carried by experts 4–6 | **[x]** measured: weights 33.5 / 20.6 / 15.0 / 12.1 / 10.1 / **8.8%** against a uniform 16.7% — the tail is **31% of the mass**, so top-3 routing is *bounded, not free*. The source is careful that weight is not contribution, so a perplexity run could still find the tail cheap; what is settled is that it is not the free win it was filed as |
| C4 | Rung 1 — concurrent expert reads | **[x] already shipped, and the ladder was wrong about it.** `READERS = 8`, `READER_HANDLES = 8`, one file handle per reader. The research node's "the engine reads experts one at a time" is contradicted by the code and by the engine's own 2.02 GiB/s, which QD-1 (1.34) cannot produce |
| C4b | Close the remaining I/O gap | **[x] measured, and it is nearly closed already.** With the trunk resident and no expert cache, the engine reads **3268 MiB in 1.11 s = 2.88 GiB/s** — past every figure in the docs (1.40, 1.88, 2.02) and **84% of the drive's QD-8 peak**. Remaining headroom **1.19× on the disk, 1.10× on the token**, not the 1.69× I claimed |
| C5 | **Profile `F`** — the 0.84 s per token that never touches the disk | **[x]** done 2026-08-31, trunk resident, three runs (0.494 / 0.510 / 0.511 tok/s). Token = **60% disk, 40% F**, and **F is 93% arithmetic**: `compute` 0.44 s, **the router 0.22 s**, construction only 0.05 s. A first pass said the opposite by trusting a comment instead of reading `moe_routing`, which runs `ctx.compute` in its middle |
| C5b | ~~Build the block graph once~~ | **[!] withdrawn same day.** Re-measured with the router timed separately: graph construction is **0.05 s of F's 0.71 s**, not 0.36 s. Worth ~3% of a token, not 20% |
| C5c | ~~Select the top-6 experts on the CPU~~ | **[!] dead, measured before it was written.** `CHAOS_ROUTE_SPLIT` separates the halves: the BF16 `mul_mat` costs **0.256 s** and `argsort_top_k` costs **~0.000 s**. The sort was never the cost, and a CPU selection would still need the same matmul. The 1.26x was also inflated — it divided the block-sum by a wall-clock tok/s; removing the router **entirely** is **1.13x** |
| C5d | ~~Convert `ffn_gate_inp` from BF16 at load~~ | **[!] dead, measured before it was written.** `router_matmul_dtypes` times that exact shape: **F32 0.1503 ms, BF16 0.1501 ms — 1.00x**, and both are 43x faster than the 6.4 ms the engine pays. The dtype was never the cost |
| C5e | **Stop recomputing the block tail** | **[ ]** `ctx.compute(&topk)` evaluates everything `topk` depends on, which reaches back through `ffn_norm` into `layer_tail` — and the block's final `compute` then does it **again**. Proof: the argsort blocks' final compute is **0.0101 s** against the hash blocks' **0.0100 s**, so the early evaluation was extra, not early. Copy `ffn_norm`/`probs3`/`topk` into leaf tensors before the downstream graph is built. **0.221 s of a 1.980 s token = 1.13x**, and **exact** — copying a computed value changes nothing |
| C6 | **Quality harness** — ≥50 checkable prompts + thresholds agreed first | **[x]** `scripts/quality-gate.sh`. **Different bars per lever** (Atur, 2026-08-31): *exact* needs **100% byte-identical**, *lossy* needs **≥95% identical, no checkable regression, perplexity +≤1%**. Byte-identical greedy text **is** top-1 agreement, so no logit plumbing. Verified against 1 MiB of zeros: **22.0% identical, 4 checkables lost, perplexity +1.16%** — all three fired independently |
| C7 | **Requantise the always-read trunk** — it is Q8_0 while the experts are Q4_K | **[ ]** the one lever no V4-Flash document has ever costed. ~7.38 → ~3.91 GiB, and it would fit the 5.11 GiB of free VRAM |
| C8 | Rung 2 — 2-bit experts | **[ ]** behind C6 |
| C9 | Vulkan generation is 2.2× slower than CPU — why? | **[x]** **both measurements are right.** They used different context lengths: short prompt **1.5–2.0× faster** on the card, ~1050-token prompt **0.93×, slower**. `ngl-ladder` had already seen the mechanism without connecting it. The sentence needs its context clause or it is wrong |
| C10 | 5 tok/s on V4-Flash on this laptop | **[!]** **closed by the disk, not the code.** 137 GiB of experts cannot be resident, so they cross a 3.41 GiB/s drive every token — even *one-bit* experts with a free trunk and zero arithmetic cap at 4.26 tok/s |
| C11 | 20 tok/s on V4-Flash on this laptop | **[!]** needs 64.4 GiB/s against 30.8 |

**C6 is not optional.** C7 and C8 change what the model computes, and a wrong forward
pass here is fluent nonsense, never a crash.

## D. Shipped, and what shipping did not prove

| | item | state |
|---|---|---|
| D1 | v0.0.18 — CHAOS page, the `--host` fix | **[x]** verified from published files |
| D2 | v0.0.19 — Chaos builds and runs on Android | **[x]** |
| D3 | v0.0.20 — engine inside the app, in the published APK | **[x]** confirmed inside |
| D3b | v0.0.21 — a model runs on the phone tier | **[x]** `libchaos_serve.so` confirmed in the published APK |
| D4 | README: version badge, the speed claims | **[x]** badge was 14 releases stale |
| D5 | Install → update → uninstall, models untouched | **[x]** every release — **on Windows only** |
| D5b | **Update from inside an old version** | **[x]** every release v0.0.12–v0.0.22 compiled from its own tag resolves today's; v0.0.5–v0.0.11 have no updater at all. `scripts/check-old-updaters.sh` |
| D6 | The APK on **real hardware** | **[ ]** emulator only — needs Atur's phone |
| D7 | Image: step-count default | **[ ]** Atur's aesthetic call |
| D8 | The `.deb` and the AppImage installed **anywhere** | **[ ]** never, by anybody |
| D9 | A **real camera** on the mark and the reader | **[ ]** everything else in the chain is proven: **210 of 210** at 3–12 px/module across 30 angles, **0 wrong strings**, `scripts/scan-sweep.js` |
| D10 | Every binary reaches every platform | **[x]** 21 of 21, test-enforced both directions, incl. `make-linux-packages.sh` |

## E. Guarding the binary rather than the documents

| | item | state |
|---|---|---|
| E1 | Test / architecture / binary counts machine-checked | **[x]** and the architecture check now reads the progress bar too, which is where it drifted |
| E2 | No document points at a directory that does not exist | **[x]** `NOTICE` and `SECURITY.md` both named a `crates/` tree that has not existed for weeks |
| E3 | The auth gate is tested | **[x]** it had zero tests and is the whole of the server's access control |
| E4 | **CI runs the correctness suite against a real model** | **[x]** a 397 MB Qwen2-0.5B is fetched and verified, and four layers run with `CHAOS_REQUIRE_MODEL_TESTS=1` so a missing model is a failure rather than a skip. Sensitivity measured, not assumed: **1 MiB of zeros in the container moves the byte-exact golden and does *not* move the substring tripwire**, which is why there are two output checks |
| E5 | GPU tests fail rather than skip when a card is expected | **[~]** `CHAOS_REQUIRE_GPU=1` exists; CI never sets it, and must not — no runner has a card |
| E6 | The window is exercised in CI | **[x]** `scripts/app-smoke.ps1` presses RETURN like a person and counts client-rects on screen: **0 at open, 12 after**. Deliberately not `run-through.ps1`, which documents its own hang modes; a `timeout-minutes` backs that up |
| E7 | `chaos-run` has real coverage | **[x]** 8 -> 16, and writing them found a systemic defect: **43 flags parsed their value with `.parse().ok()` and silently used the default**, so `-n notanumber` loaded the model and generated 8 tokens. All refused by name now |
| E8 | The port is bound before the model loads | **[x]** bound at the top of `serve`, before `Model::open_split`. Two nodes on one port: **refused in 135 ms**, with a message naming the port, `chaos status`, `chaos stop` and `--port` |
| E9 | `finish_reason` reaches `chaos connect` | **[x]** a capped answer now ends with `[cut off at the token limit -- ask for more with --max-tokens N]`, and `--max-tokens` exists. Only `length` is narrated; `stop` stays silent |

---

## Next three

1. **C5b** — build the block graph once and rebind its inputs, instead of reconstructing
   it per block per token. `tail` is 0.36 s of a 1.84 s token and it is pure overhead;
   worth ~1.24x with no quality risk. The obvious shape of the fix, not yet costed.
2. **C5** — profile `F`. It caps this machine at 1.19 tok/s and nobody has opened it.
3. **C7** — cost the trunk requantisation, behind **C6**. It is the only untried lever
   that moves the ceiling, and the quality risk is real and unmeasured.
