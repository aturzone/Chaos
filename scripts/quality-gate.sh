#!/usr/bin/env bash
# The gate that stands between "faster" and "broken".
#
# **Nothing that changes what the model computes may ship without passing this**,
# and `CHECKLIST.md` has said so since the ladder was written: *"C6 is not
# optional. It changes what the model computes, and a wrong forward pass here is
# fluent nonsense, never a crash."*
#
# `the-big-bang-5-tok-s.md` §5 asks for three things and this is all three:
#
#   1. a diff against the baseline on a fixed prompt set;
#   2. fifty prompts with answers checkable without reading prose;
#   3. **a threshold agreed before the runs**, so the decision is not made after
#      seeing the speed.
#
# # The thresholds, and why there is more than one
#
# Atur's decision, 2026-08-31: **different bars per lever**, because these are
# different kinds of change and one bar would be wrong for both.
#
#   exact   The change must not alter the answer at all. Selecting the top 6 of
#           256 by value is the top 6 by value however it is computed, so the
#           only legitimate difference is tie-breaking order. **100% of answers
#           byte-identical**, and any difference must be shown to be a tie before
#           it is accepted -- by a human, not by this script.
#
#   lossy   The change is known to alter the arithmetic: 2-bit experts, top-k
#           routing, a requantised trunk. **>= 95% of answers byte-identical, no
#           checkable answer that was right may become wrong, and perplexity may
#           not rise by more than 1%.**
#
# The perplexity band is the part that catches a change which keeps the greedy
# path and wrecks the distribution underneath it -- 95% agreement with a doubled
# perplexity is not a faster model, it is a worse one that happens to agree.
#
# **1% is a judgement and it is deliberately tight.** Verified against 1 MiB of
# zeros written into a container: 22.0% identical, four checkable answers lost,
# and perplexity 1.0708 -> 1.0832, a **1.16% rise**. So the band is set just
# below a change that is unambiguously damage -- which means the first *legitimate*
# lever to fail it will fail it narrowly.
#
# When that happens, argue it with the numbers and move the constant in a commit
# that says why. **Do not widen it in the same change it is blocking**, and do not
# re-record the baseline to make a comparison pass; both turn the gate into a
# formality, which is the one failure mode it cannot survive.
#
# # The one legitimate reason to re-record: the reference itself was wrong
#
# **A tokenizer fix invalidates a baseline.** On 2026-09-03 the `joyai-llm`
# pre-tokenizer turned out to be missing llama.cpp's first regex alternative, so
# V4-Flash had been fed `.` + `md` where the model expects `.md`. Every recorded
# answer for that model came from the wrong tokenisation, and a later gate run
# against them would have reported a large "changed" count caused by the **fix**
# rather than by the lever under test.
#
# So: re-record when the *reference* was measuring the wrong thing, never when the
# *candidate* fails. Keep the old one beside the new with a note saying which bug
# it predates -- a superseded baseline is evidence about when a defect was
# present. `quality-baseline/` is gitignored, so that note lives with the files.
#
# # Why byte-identical text is the same thing as top-1 agreement
#
# At `--temp 0` generation is greedy, so the emitted text **is** the sequence of
# top-1 tokens. Comparing the text compares the argmax at every position, with no
# logit plumbing and nothing to get subtly wrong in the harness itself. §5 asked
# for "top-1 agreement rate"; this is that, measured the cheap way.
#
# What it does not give is KL divergence, which needs the distributions. That is
# why the perplexity band is here: it is the distribution check, taken from the
# facility `chaos-run --perplexity` already has.
#
#   # record what the model does today, before changing anything
#   bash scripts/quality-gate.sh --model M.gguf --record
#
#   # after the change
#   bash scripts/quality-gate.sh --model M.gguf --lever exact
#   bash scripts/quality-gate.sh --model M.gguf --lever lossy
#
#   # a lever that is a FLAG rather than an environment variable
#   bash scripts/quality-gate.sh --model M.gguf --lever lossy #     --flags "--trunk-quant q4_k" --ppl scripts/ppl-corpus.txt
#
# Exit 0 if the bar for that lever is met, 1 if it is not, and 2 if the harness
# could not run -- which is never reported as a pass.
set -uo pipefail

cd "$(git rev-parse --show-toplevel)" || exit 2

# ---- the thresholds, declared before any run -------------------------------
EXACT_MIN_IDENTICAL=100      # per cent
LOSSY_MIN_IDENTICAL=95       # per cent
LOSSY_MAX_PPL_RISE=1.0       # per cent
TOKENS=12                    # generated per prompt
# **Declared, not defaulted.** `chaos-run`'s own default is 512, which is what
# `llama-perplexity` uses and is right for comparing the two engines. Here both
# sides are Chaos, so what matters is that the baseline and the comparison use
# the *same* chunk -- and 512 silently measures nothing on a corpus under 512
# tokens, which is how the first run of this script reported "could not be
# measured" while the real message was sitting in stderr.
PPL_CHUNK=128
# **Use `scripts/ppl-corpus.txt`, and NOT `long-prompt.txt`.**
#
# `long-prompt.txt` is a long *prompt* fixture: 41 unique lines repeated to 80,
# every one of them *"Paragraph N. The engine keeps the always-read weights
# resident..."*. Given 257 tokens of that, a model predicts the rest almost
# perfectly, so **both engines collapse to a perplexity near 1.0** -- Chaos 1.0391
# against llama.cpp 1.0151 at chunk 512 -- and every ratio computed on it is noise
# amplified. It is right for testing context length and wrong for measuring
# quality, and a whole evening's perplexity numbers were taken on it before that
# was noticed.
#
# `ppl-corpus.txt` is the repository's own prose, ~2000 tokens, every line unique.
# It is not a standard benchmark and is not claimed to be one; what it is good for
# is comparing two engines or two builds over the same real English.
#
# A chunk is also at most `PPL_CHUNK` tokens of context, which keeps V4-Flash's
# logits **exact**: skipping the unimplemented lightning indexer is only a no-op
# while `n_tokens / 4` stays inside `indexer_top_k`, and a single 1818-token pass
# printed *"These logits are APPROXIMATE"* where a chunked one does not.

MODEL=""
LEVER=""
RECORD=0
PROMPTS="scripts/quality-prompts.tsv"
BASE=""
PPL_CORPUS=""
# Extra flags for `chaos-run`, so a lever that is a **flag** rather than an
# environment variable can be gated at all.
#
# Every lever until now was an env var (`CHAOS_NO_FREEZE`, `CHAOS_NO_REPACK`), so
# the same binary could be run both ways with nothing passed here. C7 is
# `--trunk-quant q4_k`, and without this the gate could only ever have measured
# the default. Echoed in the header below on purpose: a gate that does not say
# which flags it used is a gate that could be comparing two of the same thing.
EXTRA=""

while [ $# -gt 0 ]; do
  case "$1" in
    --model)    MODEL="${2:-}"; shift 2 ;;
    --lever)    LEVER="${2:-}"; shift 2 ;;
    --record)   RECORD=1; shift ;;
    --prompts)  PROMPTS="${2:-}"; shift 2 ;;
    --baseline) BASE="${2:-}"; shift 2 ;;
    --ppl)      PPL_CORPUS="${2:-}"; shift 2 ;;
    --tokens)   TOKENS="${2:-}"; shift 2 ;;
    --flags)    EXTRA="${2:-}"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

[ -n "$MODEL" ] || { echo "usage: quality-gate.sh --model <gguf> [--record | --lever exact|lossy]" >&2; exit 2; }
[ -f "$MODEL" ] || { echo "no such model: $MODEL" >&2; exit 2; }
[ -f "$PROMPTS" ] || { echo "no such prompt set: $PROMPTS" >&2; exit 2; }
if [ "$RECORD" -eq 0 ] && [ "$LEVER" != "exact" ] && [ "$LEVER" != "lossy" ]; then
  echo "--lever must be exact or lossy (or pass --record)" >&2; exit 2
fi

EXE="target/release/chaos-run"
[ -f "$EXE.exe" ] && EXE="$EXE.exe"
[ -f "$EXE" ] || { echo "build it first: cargo build --release --bin chaos-run" >&2; exit 2; }

STEM=$(basename "$MODEL"); STEM="${STEM%.gguf}"
[ -n "$BASE" ] || BASE="quality-baseline/$STEM"

# ---- run the prompt set ----------------------------------------------------

# Strip comments and blank lines once, so the count is the real count.
LIVE=$(mktemp); trap 'rm -f "$LIVE" "$OUT" 2>/dev/null' EXIT
grep -v '^#' "$PROMPTS" | grep -v '^[[:space:]]*$' > "$LIVE"
N=$(wc -l < "$LIVE" | tr -d ' ')
if [ "$N" -lt 50 ]; then
  echo "the prompt set has $N usable lines and the bar is fifty." >&2
  echo "A gate with a short prompt set is a gate that agrees with anything." >&2
  exit 2
fi
echo "model    $MODEL"
echo "prompts  $N, $TOKENS tokens each"
# Word-split deliberately: this is a flag list, not a filename.
# shellcheck disable=SC2206
EXTRA_ARGS=()
[ -n "$EXTRA" ] && EXTRA_ARGS=($EXTRA)
if [ ${#EXTRA_ARGS[@]} -gt 0 ]; then
  echo "flags    ${EXTRA_ARGS[*]}"
else
  echo "flags    none (the built-in default)"
fi
echo

# **What the answer is, and what this got wrong for its whole first day.**
#
# With stderr discarded, `chaos-run` prints exactly two things on stdout: the
# completion, behind an `output` marker, and a `generate N tokens in Ns (X tok/s,
# Ys per token)` line. That second line is a **timing**, so it differs on every
# run of any build -- and the first version of this loop kept it. The consequence
# was not a wrong number, it was a gate that **could not pass**: comparing a build
# against itself would have reported 0 of 50 byte-identical, and the first exact
# lever to reach it did (C5e, whose text was in fact identical).
#
# It survived because the harness was only ever validated against *damage*, where
# a failure looks like a failure whichever way it is caused. The same shape of bug
# as `is_contiguous` in `core/ggml`, found the same day: a check tested in one
# direction only.
#
# The second defect: `${answer#*"$prompt"}` was meant to strip an echoed prompt,
# and `chaos-run` never echoes one. So it did nothing at all -- except on the rows
# where the completion happened to quote the prompt back, where it silently threw
# away the front of the answer. The recorded baseline's first row was
# ` Paris.", generate 11 tokens...` for exactly that reason.
#
# Both are fixed here: drop the trailing `generate` line if that is what the last
# line is, strip the `output` marker from the first, and collapse the rest.
extract() {
  sed '${/^generate  /d;}' \
    | sed '1s/^output *//' \
    | tr '\n' ' ' | sed 's/  */ /g; s/ *$//'
}

OUT=$(mktemp)
: > "$OUT"
right=0
while IFS=$'\t' read -r expected prompt; do
  [ -n "${prompt:-}" ] || continue
  gen=$("$EXE" "$MODEL" "$prompt" -n "$TOKENS" --temp 0 --no-perf     ${EXTRA_ARGS[@]+"${EXTRA_ARGS[@]}"} 2>/dev/null | extract)
  # **An empty completion is a harness failure, not an answer.** A model that
  # printed nothing and a model that printed the wrong thing are both "not
  # identical", and reporting the first as the second is how a broken run gets
  # read as a quality regression.
  if [ -z "$gen" ]; then
    echo "no completion for prompt: $prompt" >&2
    echo "The runner printed nothing on stdout. That is the harness or the" >&2
    echo "binary, not the model, and it is not a comparison." >&2
    exit 2
  fi
  hit=no
  case "$(printf '%s' "$gen" | tr '[:upper:]' '[:lower:]')" in
    *"$(printf '%s' "$expected" | tr '[:upper:]' '[:lower:]')"*) hit=yes; right=$((right+1)) ;;
  esac
  printf '%s\t%s\t%s\n' "$hit" "$expected" "$gen" >> "$OUT"
done < "$LIVE"

got=$(wc -l < "$OUT" | tr -d ' ')
if [ "$got" -ne "$N" ]; then
  echo "ran $got of $N prompts -- the harness failed, which is not a pass." >&2
  exit 2
fi
echo "checkable answers correct: $right of $N"

# ---- perplexity, when a corpus is given -----------------------------------
ppl=""
if [ -n "$PPL_CORPUS" ] && [ -f "$PPL_CORPUS" ]; then
  ppl_out=$("$EXE" "$MODEL" -f "$PPL_CORPUS" --perplexity --ppl-chunk "$PPL_CHUNK"     ${EXTRA_ARGS[@]+"${EXTRA_ARGS[@]}"} 2>&1)
  ppl=$(printf '%s' "$ppl_out" | grep -oE 'perplexity [0-9]+\.[0-9]+' | head -1 | awk '{print $2}')
  if [ -n "$ppl" ]; then
    echo "perplexity: $ppl  (chunk $PPL_CHUNK)"
  else
    # **Say why, not that.** The first version printed "could not be measured"
    # and swallowed a perfectly clear message from chaos-run -- the corpus was
    # 273 tokens against a 512-token chunk, so no chunk reached two tokens. A
    # check that hides its own reason for not running is the shape of bug this
    # whole gate exists to catch.
    echo "perplexity: NOT MEASURED. chaos-run said:"
    printf '%s' "$ppl_out" | tail -2 | sed 's/^/  /'
  fi
fi

# ---- record ----------------------------------------------------------------
if [ "$RECORD" -eq 1 ]; then
  mkdir -p "$BASE"
  cp "$OUT" "$BASE/answers.tsv"
  printf '%s\n' "$right" > "$BASE/checkables-correct.txt"
  [ -n "$ppl" ] && printf '%s\n' "$ppl" > "$BASE/perplexity.txt"
  echo
  echo "RECORDED $BASE/"
  echo "  Record a baseline only from a build you believe. A baseline taken from a"
  echo "  broken engine freezes the breakage in and every later run agrees with it."
  exit 0
fi

# ---- compare ---------------------------------------------------------------
[ -f "$BASE/answers.tsv" ] || {
  echo "no baseline at $BASE/answers.tsv -- record one first with --record" >&2
  exit 2
}

base_right=$(cat "$BASE/checkables-correct.txt" 2>/dev/null || echo 0)
identical=0
regressed=""
line=0
while IFS=$'\t' read -r hit expected gen; do
  line=$((line+1))
  b=$(sed -n "${line}p" "$BASE/answers.tsv")
  b_hit=$(printf '%s' "$b" | cut -f1)
  b_gen=$(printf '%s' "$b" | cut -f3-)
  [ "$gen" = "$b_gen" ] && identical=$((identical+1))
  if [ "$b_hit" = "yes" ] && [ "$hit" = "no" ]; then
    regressed="$regressed
  $expected -- was right, now: $gen"
  fi
done < "$OUT"

pct=$(awk "BEGIN{printf \"%.1f\", 100*$identical/$N}")
echo "byte-identical answers:    $identical of $N  ($pct%)"
echo "checkables in baseline:    $base_right"
[ -n "$regressed" ] && echo "REGRESSED checkable answers:$regressed"

fail=0
case "$LEVER" in
  exact)
    echo
    echo "bar: exact -- $EXACT_MIN_IDENTICAL% byte-identical required"
    if [ "$identical" -ne "$N" ]; then
      echo "FAIL: $((N-identical)) answer(s) changed."
      echo "      An exact lever must not change the answer. If every difference is"
      echo "      a genuine tie, say which and why -- in the commit, with the logits."
      echo "      Do not re-record the baseline to make this pass."
      fail=1
    fi
    ;;
  lossy)
    echo
    echo "bar: lossy -- >=$LOSSY_MIN_IDENTICAL% identical, no checkable regression, perplexity +<=$LOSSY_MAX_PPL_RISE%"
    awk "BEGIN{exit !($pct < $LOSSY_MIN_IDENTICAL)}" && {
      echo "FAIL: $pct% identical, below $LOSSY_MIN_IDENTICAL%."; fail=1; }
    [ -n "$regressed" ] && { echo "FAIL: a checkable answer that was right is now wrong."; fail=1; }
    if [ -n "$ppl" ] && [ -f "$BASE/perplexity.txt" ]; then
      b_ppl=$(cat "$BASE/perplexity.txt")
      rise=$(awk "BEGIN{printf \"%.2f\", 100*($ppl-$b_ppl)/$b_ppl}")
      echo "perplexity: $b_ppl -> $ppl  (${rise}%)"
      awk "BEGIN{exit !($rise > $LOSSY_MAX_PPL_RISE)}" && {
        echo "FAIL: perplexity rose ${rise}%, over the $LOSSY_MAX_PPL_RISE% band."
        echo "      95% agreement with a worse distribution is not a faster model."
        fail=1; }
    else
      # **Not silently excused.** A missing distribution check is a missing
      # third of this gate, and a lossy lever is exactly where it matters.
      echo "WARNING: no perplexity comparison -- pass --ppl <corpus> and record one."
      echo "         For a lossy lever this is a hole in the gate, not a detail."
    fi
    ;;
esac

echo
if [ "$fail" -ne 0 ]; then
  echo "GATE FAILED. The change does not ship, and no tok/s number from it may be quoted."
  exit 1
fi
echo "GATE PASSED for a $LEVER lever."
