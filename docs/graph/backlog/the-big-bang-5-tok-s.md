---
topic: V4-Flash at 5 tok/s on a 15.7 GiB laptop — the plan, and the arithmetic it rests on
status: open
links:
  - ../research/v4flash-ram-frontier-2026-08-16.md
  - ../research/v4flash-has-no-slack-2026-08-10.md
  - ../research/machine-bandwidth-2026-08-25.md
  - devices-as-resources.md
  - android-app.md
---

# The Big Bang: V4-Flash at 5 tok/s on this laptop

**The target moved, and it moved for a reason.** Atur asked for 20 tok/s. That
is closed by arithmetic on this hardware and the arithmetic is below. He then
set the real target himself: *"we need at least 5 token on this device with all
problems and limits — if we reach 5 token at least we can think a better device
can run better."* That is the right target: **5 tok/s is reachable here, and it
is the number that proves the approach scales.**

Today V4-Flash generates at **0.43 tok/s**. This plan is how it gets to **5.2**.

---

## 1. What is measured, and when

Everything in this plan rests on these numbers. They were taken on
2026-08-25 on the target machine (i7-13650HX, 15.7 GiB, RTX 3050 6 GB,
SK Hynix NVMe).

| quantity | value | how |
|---|---|---|
| **RAM read bandwidth, peak** | **30.8 GiB/s** at 16 threads | `scripts/bench-memory-bandwidth` — 4 GiB buffer, summed to defeat the optimiser, best of 3 |
| RAM bandwidth, 1 thread | 17.9 GiB/s | same |
| **Generation throughput** | **~19 GiB/s** | five models, `size × tok/s` constant to ±8% |
| **NVMe sequential read** | **3.09 GiB/s** | `chaos-probe`, cache-defeating, 8 GiB file |
| **NVMe on expert reads** | **1.40 GiB/s** | `chaos-run` on V4-Flash, its own report |
| **V4-Flash shape** | 43 blocks, 256 experts, **6 used**, 1 shared | the model's own metadata |
| **Expert bytes per token** | **3.22 GiB** | 43 × 6 × 12.8 MiB, and it matches the 3288 MB measured in `v4flash-has-no-slack` |
| Always-read weights | 7.38 GiB | fits in RAM |
| Routed experts | 137.06 GiB | does not |
| **Expert reuse between tokens** | **~13%** | Chaos's own expert cache |
| `hash_layer_count` | **3** of 43 | metadata |

### The generation law

Across five models, `resident GiB × tok/s` is constant:

```
Falcon3-1B     0.98 GiB x 20.41 = 20.0 GiB/s
Llama-3.2-3B   1.87 GiB x 10.61 = 19.8 GiB/s
Qwen3-4B       2.32 GiB x  7.49 = 17.4 GiB/s
Mistral-7B     4.07 GiB x  5.96 = 24.3 GiB/s
Qwen3-14B      8.38 GiB x  2.40 = 20.1 GiB/s
```

**Generation is memory-bandwidth bound at ~19–20 GiB/s, which is 65% of this
machine's 30.8 GiB/s peak.** So `tok/s ≈ 19 / resident GiB`. That law predicts
every model on this machine, and it is what makes the rest of this plan
arithmetic rather than hope.

---

## 2. Why 20 tok/s is closed, in one table

| | |
|---|---|
| 20 tok/s means | 50 ms per token |
| expert bytes per token | 3.22 GiB |
| therefore required bandwidth | **64.4 GiB/s** |
| this machine's RAM peak | **30.8 GiB/s** |
| this machine's NVMe peak | **3.09 GiB/s** |

**Short by 2.1× against RAM, and by 21× against disk** — and the RAM figure
assumes all 137 GiB of experts are already resident on a 15.7 GiB machine, the
dense trunk is free, and the implementation runs at 100% of peak.

This is not an implementation gap. `kimi-k3-in-c`, the project that prompted
this, hits the same wall from the other side: their **128 GB+ workstation, with
the model entirely in RAM and the disk gone, still takes 5.6 s/token** (0.18
tok/s). Removing the disk does not rescue a memory-bound problem.

**20 tok/s on V4-Flash needs a machine whose memory bus is ≥64 GiB/s with ≥144
GB of capacity.** Those exist — Apple M-series Ultra (192 GB unified at
~800 GB/s), an 8–12 channel EPYC/Threadripper, a multi-GPU box. None of them is
this laptop, and no code change substitutes for a memory bus.

---

## 3. The ladder to 5 tok/s

Each rung is an independent change. The tok/s column comes from the measured
constants above, with residency computed as
`t = B(f/30.8 + (1-f)/disk)`.

| # | change | bytes/token | disk | resident | **tok/s** |
|---|---|---|---|---|---|
| 0 | today | 3.22 GiB | 1.40 GiB/s | ~0 | **0.43** |
| 1 | **I/O queue depth** → real NVMe speed | 3.22 GiB | 3.09 GiB/s | ~0 | **0.96** |
| 2 | + **2-bit experts** | 1.61 GiB | 3.09 GiB/s | 15% | **2.21** |
| 3 | + **top-3 routing** | 0.81 GiB | 3.09 GiB/s | 29% | **5.21** |

**Rung 1 is free** — no model change, no quality risk. Rungs 2 and 3 change
what the model computes and **must not be shipped without the quality harness in
§5**.

### Rung 1 — I/O queue depth. The one nobody has tried.

The NVMe does **3.09 GiB/s** sequential. Expert reads get **1.40 GiB/s** — 45%
of the drive. An NVMe reaches its rated speed only with several requests in
flight; Chaos reads 12.8 MiB experts one at a time, so the drive is idle
between requests waiting for the CPU to ask again.

**The change**: issue the 6 experts of a layer as concurrent reads, and start
layer *n+1*'s reads while layer *n* computes. Note this is **not** the
read/compute overlap already measured at 1.03× in
`expert-overlap-does-not-pay`: that overlapped one read with compute. This is
**several reads with each other**, which is what fills a queue.

- **Expected**: 1.40 → 2.8–3.09 GiB/s, so **0.43 → ~0.96 tok/s**.
- **Falsified if**: concurrent reads do not raise measured GiB/s. Test this
  *outside Chaos first* with a standalone benchmark that reads 12.8 MiB blocks
  at queue depths 1/2/4/8/16 — one afternoon, and it settles the rung before a
  line of engine code is written.
- **Risk**: low. No model change.

### Rung 2 — 2-bit experts

Experts are MXFP4 (4 bits). At 2 bits the routed set is 68.5 GiB and a token
reads 1.61 GiB.

- **Expected**: **~2.2 tok/s** with 15% residency.
- **The catch**: this is a different model. MXFP4 → 2-bit on MoE experts is
  known to be lossy, and **a wrong forward pass in this project produces fluent
  nonsense, never a crash.**
- **Falsified if**: the quality harness (§5) shows the output diverging from
  the Q4 baseline beyond an agreed threshold.

### Rung 3 — top-3 instead of top-6 routing

V4-Flash routes 6 of 256 experts. Using the top 3 by gate probability halves
bytes again.

- **Expected**: **~5.2 tok/s** with 29% residency.
- **The catch**: this is a bigger change than quantisation — it alters which
  computation happens, not just its precision.
- **Cheap first test**: top-5, then top-4. The gate probabilities are already
  computed; measure how much mass the 4th–6th experts actually carry. **If the
  bottom three experts hold a small share of the probability mass, this rung is
  nearly free. If they hold a third of it, this rung is closed.** That
  measurement costs one instrumented run and should be done **before** any
  implementation.

### Reclaiming RAM is not optional

Rungs 2 and 3 assume **10 GiB free for expert cache**. Right now Brave holds
4.74 GiB, VS Code 2.50 GiB, Telegram 0.69 GiB, Edge WebView 0.47 GiB —
**9.28 GiB**, and Chaos measured only 1.74 GiB available for residency. Chaos
already reports this. The plan needs it enforced during measurement runs, and
`chaos-probe --quick` already names the processes to close.

---

## 4. What is closed. Do not re-propose.

| lever | why it is closed | evidence |
|---|---|---|
| **Speculative decoding / batching** | expert reuse between tokens is **~13%**. Batching 8 tokens reads ~42 experts instead of 48 — 12%, not the 5× the idea needs | Chaos's own expert cache |
| **Token-id prefetch** | routing is a table lookup on token id — but only for `hash_layer_count = 3` of 43 blocks (7%) | `llama.cpp/src/models/deepseek4.cpp:1147` |
| **Read/compute overlap** | 1.03× | `expert-overlap-does-not-pay` |
| **`--op-offload`** | 19% slower | `op-offload-cannot-pay-2026-08-16` |
| **Pinned hot set, expert factorisation, contextual sparsity** | measured dead | `hard-won-facts.md` |
| **Parallel-experts port** | V4-Flash's routed arithmetic is <5% of a token | `parallel-experts-do-not-transfer` |
| **The GPU** | 6 GB VRAM against 137 GiB of experts; and the measured device path is **2.2× slower at generation** than the CPU even when the model fits entirely in VRAM | `ngl-ladder-2026-08-24` |

---

## 5. The quality harness — build this FIRST

**Rungs 2 and 3 change what the model computes. This project's single most
expensive failure mode is that a wrong forward pass produces fluent nonsense
rather than an error.** `VERIFIED_ARCHITECTURES` exists because of it.

Nothing from rung 2 or 3 may be merged, and no tok/s number from them may be
quoted, until there is a harness that can tell "faster" from "broken":

1. **Logit diff against the Q4 baseline** on a fixed prompt set — the same
   method `VERIFIED_ARCHITECTURES` already uses against llama.cpp. Top-1
   agreement rate and KL divergence per token.
2. **A fixed prompt set with checkable answers** — the project's own
   *"The capital of France is"* class, extended to 50 prompts where a wrong
   answer is recognisable without a human reading prose.
3. **A stated threshold, agreed before the runs**, so the decision is not made
   after seeing the speed. Suggested: ≥95% top-1 agreement and no regression on
   the checkable set.

**This is the gate. Speed without it is not a result.**

---

## 6. The other half: full options on every platform

Atur: *"we need a full export devices — android full option, desktop full,
macos full, linux full, and all of them updated and have fully option."*

| platform | today | missing |
|---|---|---|
| **Windows desktop** | Chat, Models, Image, Monitor, Settings, CHAOS | — |
| **Android** | client + CHAOS section + the engine (v0.0.20) | model files on device, the token loop, image, monitor, settings |
| **macOS** | `chaos-run`, `chaos-serve`, CLI only | no window at all |
| **Linux** | `chaos-run`, `chaos-serve`, `.deb`, AppImage | no window at all |

**The honest position**: `gui/app` is raw Win32. There is no cross-platform
window, and writing three more native front-ends is a larger project than
everything in §3. The realistic route to "full options everywhere" is that
**every platform already has `chaos-serve`, and the CHAOS page makes any device
a CORE or a CLIENT** — so a phone or a Mac gets full function by talking to a
CORE, which is the architecture already built and shipped.

Ordered:

1. **Android to parity as a client**: model picker from `/v1/models`, the image
   page, monitor, settings — all of which are already served by the API.
2. **Android Phase B**: model files on the device and the token loop, using the
   JNI bridge shipped in v0.0.20. `core/probe` already reads a phone correctly,
   so *"a powerful phone or a simple phone"* is answerable by measurement.
3. **macOS and Linux windows**: only after 1 and 2, and only if a native window
   is really wanted rather than the CLI plus a CORE.

---

## 7. Positioning — the claim that is true and is not being made

`kimi-k3-in-c` has 6.8k stars for running a 2.78T model at **26.5 s/token** on
an 8 GB laptop — 0.038 tok/s. Their own README table says so, in seconds per
token.

Chaos, on this laptop, runs **DeepSeek-V4-Flash (144 GB) at 2.4 s/token** — 11×
faster per token on a model 11× smaller, which is the same class of feat — and
**20–31 tok/s on models up to ~1 GiB**, which they cannot do at all.

**Neither claim is on the README.** That is the actual gap, and it is not an
engineering gap. The README should say, with the numbers and the command lines:

- a 144 GB model runs on a 15.7 GiB laptop, and here is the transcript
- 20–31 tok/s on an ordinary CPU for models that fit
- `tok/s ≈ 19 / resident GiB` — a law that lets anyone predict their own machine

---

## 8. Order of work

**Phase 0 — settle the cheap questions (one day, no engine code)**
1. Queue-depth benchmark outside Chaos: 12.8 MiB reads at QD 1/2/4/8/16.
2. Gate-mass measurement: how much probability mass do experts 4–6 carry?
3. Free the 9.28 GiB and re-measure today's 0.43 tok/s baseline clean.

**Phase 1 — rung 1, no model change**
4. Concurrent expert reads; target 0.43 → ~0.96 tok/s.

**Phase 2 — the gate**
5. Build the quality harness (§5). Nothing past here ships without it.

**Phase 3 — rungs 2 and 3, each behind the harness**
6. 2-bit experts → ~2.2 tok/s.
7. top-k reduction → ~5.2 tok/s.

**Phase 4 — platforms and the README** (§6, §7), which can run in parallel
because it shares no code with the above.

**Definition of done for the headline**: V4-Flash generating at **≥5 tok/s** on
this laptop, on a fixed prompt, with the quality harness green, the command line
in a doc, and the run repeated in one session.

---

## 9. What could make this fail, stated in advance

- **Queue depth may not help.** If the 1.40 GiB/s is a filesystem or a
  cache-bypass artefact rather than queue starvation, rung 1 returns nothing and
  the ladder starts at 0.43 instead of 0.96. Phase 0 finds this out in a day.
- **2-bit may break the model.** MoE experts at 2 bits is aggressive.
- **The bottom experts may carry real mass.** Then rung 3 is closed and the
  ceiling is ~2.2 tok/s.
- **Residency assumptions may not hold.** 29% resident assumes 10 GiB free and
  a cache policy that keeps the right experts, against a measured 13% reuse.

**If rungs 2 and 3 both fail, the honest outcome is ~1 tok/s on this machine and
a documented reason.** That is still 2.3× better than today, and it is still a
result. It is not 5, and it would have to be said plainly.
