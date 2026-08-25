# The prompt for the next session

Paste everything between the lines into a fresh session opened in
`C:\Projects\Bigtea`. It is written to survive a cold start: it states the
target, the constraints that are already measured, the levers that are already
closed, and the order — so the session does not spend its first hour
rediscovering any of it.

---

## PASTE FROM HERE

Read `STATUS.md` and `docs/graph/backlog/the-big-bang-5-tok-s.md` first. They
are the plan; this is the instruction.

**The goal: DeepSeek-V4-Flash generating at 5 tok/s on this laptop.** It is at
0.43 today. 20 tok/s was the original target and is closed — 64.4 GiB/s needed
against 30.8 measured — so do not spend a minute on it, and do not let anything
in this session quietly re-target it.

**These are measured. Do not re-derive them, and do not contradict them without
a measurement of your own:**

- This machine reads **30.8 GiB/s** from RAM at peak, 17.9 at one thread,
  saturating at four. `chaos-membench`.
- **`tok/s ≈ 19 / resident GiB`** for generation, across nine models over a 23x
  size range. Generation runs at 65% of the machine's peak.
- A V4-Flash token reads **3.22 GiB** of experts: 43 blocks x 6 routed x
  12.8 MiB. 137.06 GiB routed, 7.38 GiB always-read.
- The NVMe does **3.09 GiB/s** sequential; Chaos reads experts at **1.40**.
- Expert reuse between consecutive tokens is **~13%**.
- `hash_layer_count` is **3** of 43 blocks.

**Closed. Do not propose these:** speculative decoding or batching (13% reuse),
token-id prefetch (3 of 43 blocks), read/compute overlap (1.03x), `--op-offload`
(19% slower), pinned hot set, expert factorisation, contextual sparsity,
parallel-experts, and the GPU (6 GB VRAM against 137 GiB, and the device path is
2.2x *slower* at generation even when a model fits entirely in VRAM).

### Do these in order

**Phase 0 — one day, no engine code. Each answers a question that decides
whether the rung above it is worth building.**

1. **Queue depth.** A standalone benchmark that reads 12.8 MiB blocks from the
   V4-Flash shards at queue depth 1, 2, 4, 8, 16 — `O_DIRECT`-equivalent, cache
   bypassed, best of three. **The question: does concurrency raise the 1.40
   GiB/s toward 3.09?** If it does not, rung 1 is dead and the ladder starts at
   0.43 rather than 0.96 — say so and continue to Phase 2.
2. **Gate mass.** Instrument the V4-Flash router to record, per token, the
   probability mass carried by the 4th, 5th and 6th experts. **The question: is
   dropping them cheap or catastrophic?** If those three carry a third of the
   mass, rung 3 is closed and the ceiling is ~2.2 tok/s. Report the distribution,
   not just a mean.
3. **A clean baseline.** Browsers hold ~9.28 GiB on this machine and Chaos gets
   1.74 GiB for residency. Close them, re-measure 0.43, and record what it
   becomes with the RAM free. `chaos-probe --quick` names what to close.

**Phase 1 — rung 1, no model change.** Issue a layer's 6 expert reads
concurrently, and start layer n+1's reads while layer n computes. **This is not
the read/compute overlap already measured at 1.03x** — that overlapped one read
with compute; this is several reads with each other, which is what fills a
queue. Target 0.43 -> ~0.96 tok/s.

**Phase 2 — the gate. Nothing past here ships without it.** Build the quality
harness: logit diff against the Q4_K_XL baseline on a fixed prompt set (top-1
agreement and KL per token), plus 50 prompts with checkable answers, plus a
threshold agreed **before** any speed run — suggested ≥95% top-1 agreement and
no regression on the checkable set. **A wrong forward pass in this project
produces fluent nonsense, never a crash.** Rungs 2 and 3 change what the model
computes, so without this harness a speed number is not a result.

**Phase 3 — behind the harness, in this order.** 2-bit experts (-> ~2.2 tok/s),
then top-k reduction, trying 5 and 4 before 3 (-> ~5.2 tok/s).

**Phase 4 — parallel, shares no code.** Android to client parity (model picker
from `/v1/models`, image, monitor, settings — all already served by the API),
then Android Phase B using the JNI bridge shipped in v0.0.20. macOS and Linux
windows only after those, and only if a native window is really wanted over
CLI + a CORE.

### How to work

- **Measure before building.** Every rung above has a cheap question that decides
  it. Answer the question first; the plan says what falsifies each rung.
- **One session, alternating, `Get-Process` first.** An orphaned benchmark
  holding 9 GiB has looked like a 10x regression here before.
- **A rate measured over 32 tokens is not a constant.** Extrapolating one was
  wrong by 1.5x inside a single session. Measure the run you intend to claim.
- Implementation on `ticket/<name>` branches with a PR; docs may go to main.
  Merge when CI is green, verify containment with `git merge-base
  --is-ancestor` before deleting, fast-forward main explicitly, re-run the
  tests on main itself.
- Update `STATUS.md` in the same commit as anything that moves a number.

### What success is

**V4-Flash generating at ≥5 tok/s on this laptop**, on a fixed prompt, with the
quality harness green, the command line in a doc, and the run repeated in one
session. If rungs 2 and 3 both fail, the honest outcome is ~1 tok/s and a
written reason — still 2.3x better than today, still a result, and it must be
reported as what it is rather than dressed up.

## PASTE TO HERE
