#!/usr/bin/env bash
# Are the documents actually maintained, or only numerous?
#
# **Atur's rule, 2026-09-02**: the README carries a bar for documentation the same
# way it carries one for CLI flags, and a bar has to be a ratio of two counted
# things or it is decoration. So this counts three things, and two of the three
# were drifting when they were first counted:
#
#   1. **Every graph node is in `INDEX.md`.** Ten were not -- 102 research files
#      against 98 index lines, 32 backlog files against 26. `CLAUDE.md` tells a
#      session to read `INDEX.md` first and then only the nodes a task links to,
#      so an unindexed node is a node nobody will ever read. **Gated.**
#   2. **Every node declares what it is** (`topic:` in its frontmatter). 46 of 145
#      do not, almost all of them in `backlog/`. **Counted, not gated.**
#   3. **Every relative path a document points at exists.** E2's rule, written
#      because `NOTICE` and `SECURITY.md` both named a `crates/` tree that had not
#      existed for weeks. `check-readme.sh` enforces it for the README's own map;
#      this covers the root documents and the index. **Gated.**
#
# The README's bar is (1) and (2) together -- a node is *in order* when it is
# indexed and declared -- so the number shown is the honest one rather than the
# flattering one. Exits non-zero on a gated failure.
set -uo pipefail

cd "$(git rev-parse --show-toplevel)" || exit 2

GRAPH=docs/graph
INDEX=$GRAPH/INDEX.md
SECTIONS="research backlog decisions reference history"

fail=0
say() { echo "$@"; }
bad() { echo "::error::$*"; fail=1; }

[ -f "$INDEX" ] || { bad "$INDEX is missing, and it is the entry point"; exit 1; }

# ---- 1. indexed ------------------------------------------------------------
total=0
indexed=0
missing=""
for d in $SECTIONS; do
  for f in "$GRAPH/$d"/*.md; do
    [ -f "$f" ] || continue
    b=$(basename "$f")
    total=$((total + 1))
    if grep -q "^- $d/$b " "$INDEX"; then
      indexed=$((indexed + 1))
    else
      missing="$missing
  $d/$b"
    fi
  done
done

if [ -n "$missing" ]; then
  bad "these graph nodes are in no INDEX.md line, so nothing will ever read them:$missing"
fi

# ---- 2. frontmatter --------------------------------------------------------
# **Counted, not gated, and that is deliberate.** 46 of 145 nodes have no
# frontmatter -- almost every `backlog/` file -- so failing on it would start this
# check red, and a check that starts red is a check people learn to ignore. It is
# the number the README's bar reports instead, so the debt is visible and shrinks
# as nodes are fixed. `reference/` and `history/` are exempt by design: one is a
# standing fact sheet and the other is kept-for-the-reasoning, and neither has a
# status that changes.
#
# **But the debt is ratcheted.** A number that is only counted can still grow, so
# today's count per section is recorded here as a ceiling: fixing files lets the
# ceiling come down, and adding an undeclared node pushes it over and fails. Lower
# these in the same commit as the files you fix -- that is the whole mechanism.
ceiling_for() {
  case "$1" in
    research)  echo 25 ;;
    backlog)   echo 17 ;;
    decisions) echo 4 ;;
    *)         echo 0 ;;
  esac
}

declared=0
undeclared=""
for d in $SECTIONS; do
  section_undeclared=0
  for f in "$GRAPH/$d"/*.md; do
    [ -f "$f" ] || continue
    case "$d" in
      reference|history) declared=$((declared + 1)) ;;
      *)
        if head -12 "$f" | grep -qE '^topic:'; then
          declared=$((declared + 1))
        else
          section_undeclared=$((section_undeclared + 1))
          undeclared="$undeclared $d/$(basename "$f")"
        fi
        ;;
    esac
  done
  cap=$(ceiling_for "$d")
  if [ "$section_undeclared" -gt "$cap" ]; then
    bad "$d/ has $section_undeclared nodes with no topic: and the recorded ceiling is $cap. Declare the new one, or lower the ceiling in check-docs.sh in the same commit as fixing files."
  elif [ "$section_undeclared" -lt "$cap" ]; then
    say "note  $d/ is down to $section_undeclared undeclared from a ceiling of $cap -- lower it"
  fi
done

# ---- 3. every path a document points at exists -----------------------------
# Only paths that look like repo paths: a slash, and a known top-level directory
# or a `.md`/`.sh`/`.rs`/`.toml` suffix. A prose sentence containing a slash is
# not a path, and treating one as a path is how a checker becomes noise.
broken=""
checked=0
for doc in README.md STATUS.md CHECKLIST.md CONTRIBUTING.md SUPPORT.md SECURITY.md NOTICE "$INDEX"; do
  [ -f "$doc" ] || continue
  # Backtick-quoted paths only: the documents quote every real path that way,
  # and it keeps URLs and prose out of the match.
  while read -r path; do
    case "$path" in
      http*|*@*|*' '*) continue ;;
    esac
    # A trailing slash means a directory.
    checked=$((checked + 1))
    if [ ! -e "${path%/}" ]; then
      broken="$broken
  $doc -> $path"
    fi
  done < <(grep -oE '`(core|cli|network|gui|android|scripts|docs|assets|tools)/[A-Za-z0-9._/-]*`' "$doc" \
             | tr -d '`' | sort -u)
done
[ -n "$broken" ] && bad "these documents point at paths that do not exist:$broken"

# ---- the bar ---------------------------------------------------------------
# **One ratio, because the README's rule is that a bar is a ratio of two counted
# things.** A node is *in order* when it is indexed and it declares what it is, so
# the bar is the weaker of the two counts and improves only when the real debt
# does. Today that is the frontmatter: every node is indexed.
in_order=0
for d in $SECTIONS; do
  for f in "$GRAPH/$d"/*.md; do
    [ -f "$f" ] || continue
    b=$(basename "$f")
    grep -q "^- $d/$b " "$INDEX" || continue
    case "$d" in
      reference|history) in_order=$((in_order + 1)) ;;
      *) head -12 "$f" | grep -qE '^topic:' && in_order=$((in_order + 1)) ;;
    esac
  done
done

denom=$total
[ "$denom" -gt 0 ] || denom=1
pct=$((100 * in_order / denom))
say "graph nodes: $total, indexed $indexed, declared $declared, in order $in_order (${pct}%)"
say "paths checked in root documents and the index: $checked"
if [ -n "$undeclared" ]; then
  n=$(printf '%s
' $undeclared | wc -l | tr -d ' ')
  say "$n nodes declare no topic: (counted against the bar, not a failure)"
  printf '%s
' $undeclared | head -5 | sed 's/^/    /'
  [ "$n" -gt 5 ] && say "    ... and $((n - 5)) more"
fi

# The README states the same ratio, and a bar that disagrees with its own checker
# is worse than no bar. Enforced here rather than by eye.
if [ -f README.md ]; then
  stated=$(grep -oE 'Documents +[0-9]+%' README.md | grep -oE '[0-9]+' | head -1)
  claimed=$(grep -oE '[0-9]+ of [0-9]+ graph nodes in order' README.md | head -1)
  if [ -z "$stated" ]; then
    bad "README.md has no 'Documents' bar; add one or delete this check"
  elif [ "$stated" != "$pct" ]; then
    bad "README.md says the documents bar is ${stated}%, the count says ${pct}%"
  elif [ "$claimed" != "$in_order of $total graph nodes in order" ]; then
    bad "README.md says '$claimed', the count says '$in_order of $total graph nodes in order'"
  else
    say "ok  README.md's documents bar matches: ${pct}%, $claimed"
  fi
fi

[ "$fail" -eq 0 ] && say "every node is indexed and every quoted path resolves."
exit "$fail"
