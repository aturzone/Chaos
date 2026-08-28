---
topic: §4b — every headline claim re-checked against a command that proves it
status: resolved — 8 claims checked, 4 corrected, 1 overstated claim retracted
links:
  - ../backlog/v0-0-3-the-complete-version.md
  - ../backlog/llamacpp-flag-audit.md
  - 4a-where-the-time-goes-2026-08-28.md
---

# §4b: claimed versus works

§4b asks for every claim in `STATUS.md` and `README.md` to be re-checked against
a command that proves it, and says: **retract what does not survive**, and that
adding to the retraction list is a good outcome rather than a bad one.

Eight checkable claims. **Four survive as written, three were corrected, and one
is retracted.**

## The scoreboard

| claim | where | verdict |
|---|---|---|
| 165 of llama.cpp's 182 long flags implemented, 17 declined, 0 unrecognised | STATUS | **survives, recomputed** |
| the install table's files exist in the release | README | **survives** — all 9 assets present |
| 20 tok/s on Falcon3-1B | README | **survives**, and is conservative: median 20.72 |
| a 144 GB model at 0.43 tok/s in 15.7 GiB | README | **survives** — its own run log says 0.428 |
| 31 tok/s on Qwen2-0.5B | README | **corrected to 28** — 31 was the best of three, not the median |
| "every one of the 13 architectures was diffed" | README | **corrected to 14** — the list has 14 |
| the install table covers every published asset | README | **corrected** — no row for Intel Mac or ARM Linux |
| **"Proven: Qwen3-30B-A3B generates correct text"** | CLAUDE.md | **RETRACTED as overstated** |

## The retraction: "Proven" was too strong

`CLAUDE.md` opened with *"**Proven**: Qwen3-30B-A3B (17.28 GiB) generates correct
text on a 15.7 GiB machine"*. It does generate, and the text reads correctly. But:

```
$ chaos-run Qwen3-30B-A3B-Q4_K_M.gguf "Hi." -n 2
chaos-run: "qwen3moe" is not an architecture this build has been verified against.
           verified: baichuan, gemma, deepseek4, gemma2, gemma3, internlm2, llama,
                     olmo, phi3, qwen2, qwen3, qwen35, stablelm, starcoder2
           It may load and generate, and be WRONG with no error.
           Pass --force to run it anyway.
```

**The flagship model of the headline needs `--force`.** `core/arch/src/qwen3.rs`
is candid about why, and the reasoning is good: `qwen3moe` was in the list, was
put through the eight-prompt diff for the first time, and came back **1 FAIL + 6
unstable**. One real bug fell out and was fixed — an MoE container has no
`ffn_gate` (its gate is `ffn_gate_exps`), so it was classified ungated and ran
GELU where the reference runs SiLU. The remaining FAIL is a **demonstrated
near-tie**: llama.cpp produces Chaos's exact answer under `-b 1` and `-ub 1`,
which change summation order and nothing else. The source's own conclusion is the
right one — *"Left out regardless, because the rule is that the diff passes — not
that someone argues it should have."*

**So the engine is probably right and the claim is still not earned.** The
project's own trap says it best: a wrong forward pass produces fluent nonsense,
never a crash, and only a diff against llama.cpp counts. A model that fails the
diff cannot be described as "proven" in the same file that states that rule.

Corrected to: *"Runs models far past RAM"*, with the qualification spelled out
and V4-Flash — which **is** verified — named as the one that carries the claim.

## The flag count survives, and my recount was wrong twice first

STATUS's *"165 of llama.cpp's 182 long flags implemented, 17 declined with a
written reason, 0 unrecognised — counted from both binaries rather than by
reading"* is **exactly right**, recomputed today against the same build:

```
$ llama-completion --help | grep -oE '\-\-[a-zA-Z0-9][a-zA-Z0-9-]*' | sort -u | wc -l
182                                    # build daef2b3
  ∩ chaos-run's REFUSED table (1119-1217)   17 declined
  named anywhere else in chaos-run         165 implemented
  in neither                                 0
  ---------------------------------------------
  sum                                      182
```

**What was stale was the audit node, not the claim**:
`backlog/llamacpp-flag-audit.md` still said 158 implemented / 24 declined, from
2026-08-14. Seven flags moved and the node was never updated. Fixed there.

**Two failed recounts before the right one, and both disagreed with STATUS.** The
first took the REFUSED table as lines 1119–1200 when it ends at 1217, and got
168/14. The second used a regex matching a flag only on the same line as its
opening `(`, silently dropping every multi-line tuple, and got 165 implemented /
**0 declined / 17 unrecognised** — the same seventeen flags, miscounted as
*unrecognised*, which would have been a far more alarming finding than the truth.
**When a crude recount disagrees with a number whose source says it was computed,
suspect the recount.**

## The two tok/s headlines, re-measured

The first thing a visitor reads. Three runs each, one session, same prompt:

| model | runs | median | README said |
|---|---|---|---|
| Qwen2-0.5B-Instruct-Q4_K_M | 28.33, 27.30, **31.32** | **28.33** | 31 |
| Falcon3-1B-Instruct-q4_k_m | 20.72, 21.10, 20.33 | **20.72** | 20 |

**31 was the best of three, not the middle one.** It is reachable — it was
reached — but a headline that quotes the top of its own range is the kind of
number this project retracts. Corrected to 28 and 21, with a note under it
saying what the old figures were and that these are medians.

Falcon3's 20 was *conservative* against a 20.72 median, which is the right
direction to be wrong in.

## The install table promised nine files; nine exist

Checked against the published release rather than against the workflow:

```
$ gh api repos/aturzone/Chaos/releases/latest
v0.0.21 — 9 assets
  Chaos-v0.0.21-windows-x86_64-Setup.exe   30.61 MiB
  chaos_0.0.21_amd64.deb                    5.57 MiB
  Chaos-v0.0.21-linux-x86_64.AppImage       6.05 MiB
  Chaos-v0.0.21-linux-x86_64.tar.gz         7.68 MiB
  Chaos-v0.0.21-linux-arm64.tar.gz          7.13 MiB
  Chaos-v0.0.21-macos-arm64.tar.gz          5.85 MiB
  Chaos-v0.0.21-macos-x86_64.tar.gz         6.65 MiB
  Chaos-v0.0.21-windows-x86_64.zip         21.88 MiB
  Chaos-v0.0.21-android-arm64.apk           3.61 MiB
```

Every file the table names is there. **But the table named five rows for seven
platform assets**: someone on an **Intel Mac** or on **ARM Linux** — a Pi, an
Ampere box — found no row for themselves, while the file they needed was sitting
in the release. Both rows added. The release workflow's comment is explicit that
those two runners exist precisely so those users have something that runs; the
README had not caught up.

## What §4b changes

- **One retraction**, and it is the project's own headline: "proven" is not a word
  for a model whose diff reports FAIL. Do not restore it for `qwen3moe` without a
  passing eight-prompt diff.
- **Three corrections**: 13 → 14 architectures, 31 → 28 tok/s (medians, with the
  old numbers shown), and two missing install rows.
- **One node un-staled**: the flag audit, which drifted while the claim it
  supports stayed right.
- **A method note worth keeping**: three of my own recounts today were wrong
  before they were right — the REFUSED range, the multi-line regex, and the
  tokenizer's BOS in §4a. Every one of them disagreed with a documented number,
  and every time the documented number was correct.
