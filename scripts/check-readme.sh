#!/usr/bin/env bash
# **The README's rule, with a mechanism.**
#
# Atur's rule, 2026-08-31: *"I need a short README that is always updated — the
# bar progress, the document map, and the tok/s of five models measured on a free
# machine. Nothing more, just the main things."*
#
# A rule stated in prose is a rule that drifts. This one has drifted before: the
# README carried **13 architectures nine lines above 14**, an expert read of
# 1.40 GiB/s for two weeks after it measured 2.88, and a `chaos-pull --list`
# count of 13 against a catalogue of 22. So the rule is checked here, and the
# check runs in CI.
#
# What it enforces, and nothing else:
#
#   1. **Short.** A line cap, so "just the main things" is not a matter of taste.
#   2. **Exactly these sections, in this order.** The "nothing more" half of the
#      rule is the half that needs teeth; without it the file grows a section a
#      week and each one looks reasonable on its own.
#   3. **Five models, the five in `speed-five.tsv`** — no fourth row quietly
#      dropped, no sixth quietly added, and the machine named.
#   4. **The speed table is dated**, because a tok/s with no date is a tok/s that
#      cannot be retracted.
#   5. **The ladder agrees with `CHECKLIST.md`**: seven releases, and as many at
#      100% as CHECKLIST has ticked.
#   6. **Every path the document map names exists.**
#
#   bash scripts/check-readme.sh
#
# Exit 0 if the README obeys its own rule, 1 if it does not.
set -uo pipefail

cd "$(git rev-parse --show-toplevel)" || exit 2

MAX_LINES=175
fail=0
say() { echo "::error::$1"; fail=1; }
ok() { echo "ok  $1"; }

[ -f README.md ] || { echo "::error::no README.md"; exit 1; }

# ---- 0. nothing left unmeasured -------------------------------------------
#
# **The README is written before the numbers exist**, which is the right order --
# the shape of the table is a decision, the cells are a measurement. This is what
# stops the draft shipping: every cell that still says PENDING is a number nobody
# took, and a plausible-looking table with one invented row is worse than no
# table at all.
if grep -q 'PENDING' README.md; then
  say "README.md still has PENDING placeholders:
$(grep -n 'PENDING' README.md | sed 's/^/       /')
       Run scripts/speed-five.sh and fill them in. Do not guess a cell."
fi

# ---- 1. short --------------------------------------------------------------
n=$(wc -l < README.md | tr -d ' ')
if [ "$n" -le "$MAX_LINES" ]; then
  ok "length: $n lines (cap $MAX_LINES)"
else
  say "README.md is $n lines; the cap is $MAX_LINES. It was 393 before the rule,
       and 169 after it -- the cap leaves room for a table row, not a section.
       Move the detail to docs/graph/ or CONTRIBUTING.md -- do not raise the cap
       to fit what you just added."
fi

# ---- 2. exactly these sections, in this order ------------------------------
EXPECTED='Install
Run
Speed
Progress
Document map
Licence'
actual=$(grep -E '^## ' README.md | sed 's/^## //')
if [ "$actual" = "$EXPECTED" ]; then
  ok "sections: the six allowed, in order"
else
  say "README.md's sections are not the allowed set.
expected:
$EXPECTED
found:
$actual
       Adding a section is a decision about what the front page is for. Make it
       here, in this list, in the same commit."
fi

# ---- 3. five models, the five in the table ---------------------------------
[ -f scripts/speed-five.tsv ] || { say "scripts/speed-five.tsv is missing"; }
if [ -f scripts/speed-five.tsv ]; then
  models=$(grep -v '^#' scripts/speed-five.tsv | grep -v '^[[:space:]]*$' | cut -f1)
  count=$(printf '%s\n' "$models" | grep -c .)
  if [ "$count" -ne 5 ]; then
    say "speed-five.tsv lists $count models. The rule says five: one huge, two
       medium, one small, one fast. Changing the set means changing the rule."
  fi
  missing=""
  while IFS= read -r m; do
    [ -n "$m" ] || continue
    grep -qF "$m" README.md || missing="$missing $m"
  done <<EOF
$models
EOF
  if [ -n "$missing" ]; then
    say "the speed table has no row for:$missing
       A missing row reads as 'that one is slow'. Run scripts/speed-five.sh."
  else
    ok "speed table: all $count models present"
  fi

  # The machine has to be named, or the numbers mean nothing.
  if grep -qiE 'cores|GiB RAM|nothing else running' README.md; then
    ok "speed table: the machine is named"
  else
    say "the speed table does not say what machine produced it, or that the
       machine was free. Both are the condition under which it is true."
  fi
fi

# ---- 4. the speed table is dated -------------------------------------------
if grep -qE 'measured 20[0-9][0-9]-[01][0-9]-[0-3][0-9]' README.md; then
  ok "speed table: dated"
else
  say "no 'measured YYYY-MM-DD' in README.md. A tok/s with no date cannot be
       retracted, and this project has retracted four."
fi

# ---- 5. the ladder agrees with CHECKLIST.md --------------------------------
rungs=$(grep -cE '^  v0\.0\.(2[4-9]|30) ' README.md)
if [ "$rungs" -eq 7 ]; then
  ok "ladder: seven rungs"
else
  say "README.md's ladder has $rungs rungs; v0.0.24 to v0.0.30 is seven."
fi
done_readme=$(grep -E '^  v0\.0\.(2[4-9]|30) ' README.md | grep -c '100%')
done_check=$(sed -n '/^## The release ladder/,/^---/p' CHECKLIST.md | grep -c '\*\*\[x\]\*\*')
if [ "$done_readme" -eq "$done_check" ]; then
  ok "ladder: $done_readme at 100%, and CHECKLIST.md ticks $done_check"
else
  say "README.md shows $done_readme rungs at 100%; CHECKLIST.md's ladder ticks
       $done_check. Two scoreboards disagreeing is the thing v0.0.24 fixed."
fi

# ---- 6. every path the document map names exists ---------------------------
paths=$(sed -n '/^## Document map/,/^## Licence/p' README.md \
        | grep -oE '`[A-Za-z0-9_./-]+`' | tr -d '`' \
        | grep -E '/|\.md$' | sort -u)
dead=""
for p in $paths; do
  [ -e "$p" ] || dead="$dead $p"
done
n_paths=$(printf '%s\n' "$paths" | grep -c . || true)
if [ -n "$dead" ]; then
  say "the document map points at paths that do not exist:$dead"
elif [ "$n_paths" -lt 6 ]; then
  # **A count, not just an absence of failures.** The first run of this check
  # reported "every path exists (0 checked)" against a README that had no
  # document map at all -- a green line for a missing section, which is exactly
  # the shape of bug the rest of this file exists to catch.
  say "the document map names $n_paths paths. It is the section that tells a
       reader where everything is, and six is the floor."
else
  ok "document map: every path exists ($n_paths checked)"
fi

echo
if [ "$fail" -ne 0 ]; then
  echo "README.md does not obey its own rule."
  exit 1
fi
echo "README.md obeys its rule."
