#!/usr/bin/env bash
# The five models the README's speed table always reports, measured in one
# session on a machine with nothing else running.
#
# **Atur's rule, 2026-08-31:** the README stays short and always carries three
# things — the progress bars, the document map, and tok/s for five fixed models
# with the machine that produced them named. This script produces the third one,
# and prints it as the exact block the README holds so nobody retypes a number.
#
# # Why a script rather than a note of what was run
#
# This project has retracted four performance claims. Every one of them was a
# number someone measured correctly and then quoted in a context that had moved:
# a different session, a different amount of free RAM, a different model set. So
# the model set is a file (`speed-five.tsv`), the machine is captured in the
# output, and the table is generated rather than edited.
#
# **Only compare within one session.** This machine drifts by up to 25% with its
# own state, so a row measured today against a row measured last week compares
# the machine's mood. That is why all five run here, in order, one invocation.
#
#   bash scripts/speed-five.sh                 # measure, print the README block
#   bash scripts/speed-five.sh --runs 5        # more repeats, same median
#   bash scripts/speed-five.sh --only Qwen2    # one row, for a quick check
#
# Exit 0 when every row measured, 1 when a row did not, and 2 when the harness
# could not start. **A missing row is a failure, not a blank cell** — a speed
# table with a gap reads as "that one is slow" when it may mean "that one did
# not build".
set -uo pipefail

cd "$(git rev-parse --show-toplevel)" || exit 2

RUNS=3
ONLY=""
TABLE="scripts/speed-five.tsv"
PROMPT="Write one sentence about the sea."

while [ $# -gt 0 ]; do
  case "$1" in
    --runs)  RUNS="${2:-3}"; shift 2 ;;
    --only)  ONLY="${2:-}"; shift 2 ;;
    --table) TABLE="${2:-}"; shift 2 ;;
    --prompt) PROMPT="${2:-}"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

EXE="target/release/chaos-run"
[ -f "$EXE.exe" ] && EXE="$EXE.exe"
[ -f "$EXE" ] || { echo "build it first: cargo build --release --bin chaos-run" >&2; exit 2; }
[ -f "$TABLE" ] || { echo "no such model table: $TABLE" >&2; exit 2; }

# ---- the machine, at the moment of measuring -------------------------------
#
# **Printed, not assumed.** "Nothing else was running" is the single condition
# under which these numbers mean anything, and it is the one a reader cannot
# check afterwards. So the free memory at the start of each model goes in the
# output, and the header says what the machine is.
free_gib() {
  # `chaos-probe --quick` knows this, but shelling to it per model costs a
  # process and a hardware sweep; the OS already has the number.
  if command -v powershell.exe >/dev/null 2>&1; then
    powershell.exe -NoProfile -Command \
      '[math]::Round((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory/1MB,2)' 2>/dev/null \
      | tr -d '\r'
  else
    awk '/MemAvailable/ { printf "%.2f", $2/1048576 }' /proc/meminfo 2>/dev/null
  fi
}

others_running() {
  # An orphaned benchmark holding memory is this project's most-recorded
  # measurement trap -- a 9 GiB leftover once looked like a 10x regression.
  local n
  n=$(ps 2>/dev/null | grep -c '[c]haos-' || true)
  echo "${n:-0}"
}

n_other=$(others_running)
if [ "$n_other" -gt 0 ]; then
  echo "WARNING: $n_other chaos process(es) are already running." >&2
  echo "         Every number below is measured against them. Close them first." >&2
fi

echo "machine  $(uname -s) $(uname -m), $(nproc) cores, $(free_gib) GiB free at start"
echo "harness  $RUNS runs per model, median reported, one session"
echo "prompt   \"$PROMPT\""
echo

printf '%-20s %8s %9s %9s %9s   %s\n' model tok/s runs free-GiB load-s note
printf '%-20s %8s %9s %9s %9s   %s\n' -------------------- -------- --------- --------- --------- ----

fail=0
rows=""
while IFS=$'\t' read -r label path predict extra; do
  case "$label" in ''|'#'*) continue ;; esac
  [ -n "$ONLY" ] && case "$label" in *"$ONLY"*) ;; *) continue ;; esac
  if [ ! -f "$path" ]; then
    printf '%-20s %8s %9s %9s %9s   %s\n' "$label" "--" "$RUNS" "--" "--" "NOT ON THIS MACHINE"
    fail=1
    continue
  fi

  before=$(free_gib)
  speeds=""
  load=""
  note=""
  # shellcheck disable=SC2086  # $extra is a flag list and must word-split
  for _ in $(seq 1 "$RUNS"); do
    out=$("$EXE" "$path" "$PROMPT" -n "$predict" --temp 0 $extra 2>&1)
    # **Two spellings, and the first version of this only knew one.** The
    # streaming path prints `generate   N tokens in Ns (0.570 tok/s, 1.8s per
    # token)` and the dense path prints `generated  N tokens in Ns (30.81
    # tok/s)` -- past tense, two decimals, no per-token clause. A pattern that
    # required a space after `generate` matched the MoE model and silently
    # missed all four dense ones, which is why the first run of this harness
    # reported one row and four dashes.
    #
    # Take the LAST such line, never the first: `prefill 5 tokens in 6.5s (0.76
    # tok/s)` carries a tok/s too, and it is the wrong one.
    s=$(printf '%s\n' "$out" | grep -E '^generated? +[0-9]+ tokens' | tail -1 \
        | grep -oE '[0-9]+\.[0-9]+ tok/s' | grep -oE '^[0-9]+\.[0-9]+')
    if [ -z "$s" ]; then
      # **Say what it said.** A row that silently reads "--" hides a refused
      # architecture, a missing shard or an out-of-memory abort as if it were
      # slowness.
      note=$(printf '%s' "$out" | grep -iE 'refus|error|abort|cannot|no such' | head -1 | cut -c1-60)
      [ -n "$note" ] || note="no tok/s line in the output"
      break
    fi
    speeds="$speeds $s"
    [ -n "$load" ] || load=$(printf '%s' "$out" | grep -oE 'loaded +[0-9]+\.[0-9]+s' \
                             | grep -oE '[0-9]+\.[0-9]+' | head -1)
  done

  if [ -n "$note" ]; then
    printf '%-20s %8s %9s %9s %9s   %s\n' "$label" "--" "$RUNS" "$before" "--" "$note"
    fail=1
    continue
  fi

  median=$(printf '%s\n' $speeds | sort -g | awk '{a[NR]=$1} END{print (NR%2)? a[(NR+1)/2] : (a[NR/2]+a[NR/2+1])/2}')
  spread=$(printf '%s\n' $speeds | sort -g | awk '{a[NR]=$1} END{if (a[1]>0) printf "%.0f%% spread", 100*(a[NR]-a[1])/a[1]; else print "--"}')
  printf '%-20s %8s %9s %9s %9s   %s\n' "$label" "$median" "$RUNS" "$before" "${load:---}" "$spread"
  rows="$rows$label|$median|$predict"$'\n'
done < "$TABLE"

echo
echo "---- the block README.md carries, verbatim ----"
echo
printf '%s\n' "$rows" | while IFS='|' read -r label median predict; do
  [ -n "$label" ] || continue
  printf '  %-20s %6s tok/s\n' "$label" "$median"
done

echo
if [ "$fail" -ne 0 ]; then
  echo "A row did not measure. The table is not complete and must not be published." >&2
  exit 1
fi
echo "All five measured. Paste the block above into README.md and date it."
