---
topic: §4c — does the folder structure still say what it means
status: resolved — the buckets hold, two real inconsistencies found, one decision for Atur
links:
  - ../backlog/v0-0-3-the-complete-version.md
  - 4b-claimed-versus-works-2026-08-28.md
---

# §4c: folder structure

`core/` `cli/` `network/` `gui/`, per the Rust book's workspace chapter. §4c asks
whether it still holds and names three open items. **It holds. Two
inconsistencies are real, and one of them is a decision rather than a defect.**

## The three items the plan named

### 1. "`crates/` exists untracked and is not the real tree — check `git ls-files`"

**Confirmed, and it does not exist in this worktree.** `git ls-files` at the top
level is exactly the documented shape:

```
.cargo .claude .github  android assets cli core docs gui network scripts tools
+ the root documents
```

No `crates/`. The warning stays worth keeping — it cost a false finding once, a
brand colour "that does not exist" read out of a leftover directory — but there is
nothing stale here to trip over. **The check is `git ls-files`, not `ls`.**

### 2. "`core/qr` was added this round"

Fine as it is. `core/qr` is a library with one binary beside it, which is the
same shape as `core/probe` and `core/gguf`. Two more libraries joined it since:
**`core/config`** (the settings file both tiers read) and **`core/http`** (enough
HTTP to ask a node for status). Both are pure libraries with no binary, and both
build without ggml, which is the property CI checks separately.

### 3. "`cli/` still contains one crate while `core/` contains a dozen binaries"

**Half fixed, and the other half is a genuine tension between two stated rules.**

`cli/` now has **two** crates — `cli/chaos` joined `cli/run` — so the lopsidedness
is smaller. But `core/` still holds **11 of the workspace's 19 binaries**:

| bucket | crates | binaries |
|---|---|---|
| `core/` | 15 | 11 |
| `cli/` | 2 | 2 |
| `network/` | 2 | 2 |
| `gui/` | 2 | 2 |
| `android/` | 1 | 0 |

The two rules that collide, both from `CLAUDE.md`:

- *"a `core/` of libraries, a `cli/` of command-line binaries"* — by which 11
  command-line binaries in `core/` is wrong;
- *"Benchmarks stay beside the crate they measure"* — by which they are exactly
  right.

**Both rules are good and they disagree because "binary" is doing two jobs.**
Splitting the eleven by what they are for resolves it cleanly:

| kind | binaries | belongs where |
|---|---|---|
| **benchmarks** — measure one crate, developer tools | `chaos-iobench`, `chaos-loadbench`, `chaos-gpubench`, `chaos-kernelbench`, `chaos-spectrum`, `chaos-tokbench` | beside their crate, as the second rule says |
| **inspectors and tools** — a person types them | `chaos-probe`, `chaos-pull`, `chaos-model-info`, `chaos-meta`, `gguf-info`, `chaos-qr`, `chaos-draw` | `cli/`, by the first rule |

**Recommendation: leave them where they are, and say why.** Moving seven
binaries means editing three staging loops in `release.yml`, the installer's file
list, `make-linux-packages.sh`, the README, and every doc that names a path — for
a tidiness gain, with a real chance of dropping one on the way (see below for what
happens when a binary falls off a list). The cheaper fix is to write the rule down
so the next person does not read `core/` as a mistake: **`core/` holds a crate's
own tools; `cli/` holds the front door and the runner.** `chaos <verb>` now makes
this invisible to users anyway — `chaos probe` and `chaos fit` reach the tools in
`core/` without anyone knowing where they live.

## Found here: 3 of 6 benchmarks ship, with no rule saying which

The release workflow's staging loops carry `chaos-gpubench`, `chaos-iobench` and
`chaos-loadbench`, and **not** `chaos-spectrum`, `chaos-kernelbench` or
`chaos-tokbench`. There is no stated rule, and no comment explaining the split —
it looks like the list simply grew as binaries were added and stopped growing.

This is the same failure that hid `chaos-qr` from every ship list while the brand
tier claimed it reached a bare terminal: **a binary in no ship list does not
exist**, and nothing checks the lists against the manifests.

**This is a decision for Atur, not a defect to fix quietly**, because either
answer changes what a user gets:

- **ship all six** — additive, nothing is taken away, the archive grows by a few
  hundred kilobytes, and a user who never runs a benchmark is unaffected;
- **ship none** — cleaner install, but *removes* three binaries someone may
  already be using, which is a breaking change for them.

Neither is done here. What is done is that it is written down.

## The counts keep drifting, so two more are now mechanisms

**Three documented counts went stale in a single day**: the test count (caught by
the existing `scripts/check-test-count.sh`), the architecture count (13 where the
list had 14, caught in §4b), and the binary count (`CLAUDE.md` said seventeen,
then eighteen, while the workspace had nineteen).

`core/arch/tests/documented_counts.rs` now enforces the last two, counted from
the source of truth rather than tallied:

- `VERIFIED_ARCHITECTURES.len()` against the README's sentence, and it also
  asserts the *neighbouring* wrong counts are absent — that is how the stale one
  survived, with the right number appearing elsewhere in the file;
- the number of `[[bin]]` targets across all five buckets against `CLAUDE.md`'s
  "Nineteen binaries, not five", with the offending names printed on failure.

**A sentence is not a mechanism** — the comment already in `nav.rs` that failed to
stop CHAOS being added the same way IMAGE was.

**And the ship lists are a mechanism too, now.** The same test file asserts the
three staging loops in `release.yml` name only real `[[bin]]` targets, that the
two Windows lists are identical, and that the Unix list differs only by
`chaos-app`. It does **not** decide which binaries ought to ship — that is the
open question above — only that the three lists cannot disagree.

**The test was tested.** Removing `chaos-qr` from one Windows list — the exact
shape of the real bug — makes it fail with *"windows list 1 is missing
[\"chaos-qr\"], which the unix list ships"*, and the workflow was restored
byte-for-byte afterwards. An untested test is a sentence with a `#[test]` on it.

## What §4c changes

- The bucket scheme **holds**; nothing needs moving.
- The `core/`-versus-`cli/` tension is **named and resolved by writing the rule
  down** rather than by churn: `core/` holds a crate's own tools, `cli/` holds the
  front door and the runner.
- Two more counts became **mechanisms** instead of sentences.
- One **open decision for Atur**: ship all six benchmarks, or none. Currently
  three, for no stated reason.
- The **ship lists became a mechanism**: the three staging loops must name real
  binaries and must agree with each other. Verified by breaking it on purpose.
