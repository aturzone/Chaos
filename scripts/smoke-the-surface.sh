#!/usr/bin/env bash
# Exercise everything a person actually touches, against the built binaries.
#
# **This exists because v0.0.30 shipped with a broken front door.** The release
# passed 999 tests, clippy, fmt and four document checks — and nobody had run
# `chaos connect` or opened `/qr` on a real node. `connect` took any
# unrecognised `--flag` as the *hostname*, so `chaos connect --port 8080` answered
# `cannot resolve --port:8080: No such host is known`, which from the outside
# reads as "devices cannot connect and the QR feature is missing".
#
# Atur found it in the first minutes of testing. A unit test cannot: the defect
# was in argument handling at the outermost layer, where nothing had ever looked.
#
# Every check prints PASS or FAIL with what it did, and the script exits non-zero
# if any failed. It needs a small model; it starts its own node and stops it.
#
#   bash scripts/smoke-the-surface.sh [model.gguf] [port]
set -u
cd "$(git rev-parse --show-toplevel)" || exit 2

MODEL="${1:-/c/Projects/models/qwen2/Qwen2-0.5B-Instruct-Q4_K_M.gguf}"
PORT="${2:-18099}"
EXT=""
[ -f "target/release/chaos.exe" ] && EXT=".exe"
B=target/release
LOG=/tmp/smoke-node.log
[ -d /tmp ] || LOG="./smoke-node.log"

pass=0
fail=0
ok()   { echo "PASS  $*"; pass=$((pass + 1)); }
bad()  { echo "FAIL  $*"; fail=$((fail + 1)); }

have() { [ -x "$B/$1$EXT" ]; }

# ---- 1. every binary exists and answers --version -------------------------
# `every_binary_reaches_every_platform` proves they are *staged*. This proves
# they *start*, which is a different claim: Windows once died with 0xC0000135
# before `main` on any machine without MSYS2.
for b in chaos chaos-run chaos-serve chaos-probe chaos-pull chaos-qr chaos-meta \
         chaos-model-info gguf-info chaos-draw chaos-worker; do
  if ! have "$b"; then
    bad "$b is not built"
  elif "$B/$b$EXT" --version >/dev/null 2>&1; then
    ok "$b --version"
  else
    bad "$b --version did not exit 0"
  fi
done

# ---- 2. the front door's own subcommands ----------------------------------
# Each must either work or refuse clearly. A subcommand that prints a confusing
# error is the bug this script was written for.
if "$B/chaos$EXT" --help 2>&1 | grep -q "connect"; then
  ok "chaos --help mentions connect"
else
  bad "chaos --help does not mention connect"
fi

# **The bug that shipped.** An unknown option must be named, not taken as a
# hostname. Checked by message, because exiting non-zero is not enough: the old
# behaviour also exited non-zero, with a DNS error.
out=$("$B/chaos$EXT" connect --port 8080 --status 2>&1)
if printf '%s' "$out" | grep -q "unknown option"; then
  ok "chaos connect refuses an unknown option by name"
elif printf '%s' "$out" | grep -qi "cannot resolve"; then
  bad "chaos connect took '--port' as a HOSTNAME: $(printf '%s' "$out" | head -1)"
else
  bad "chaos connect said something unexpected: $(printf '%s' "$out" | head -1)"
fi

# With no route and no configured core, it must explain itself rather than hang.
out=$("$B/chaos$EXT" connect 2>&1)
if printf '%s' "$out" | grep -q "chaos connect <route>"; then
  ok "chaos connect with no route prints its usage"
else
  bad "chaos connect with no route: $(printf '%s' "$out" | head -1)"
fi

# `scan` is declared NOT BUILT and must say so rather than pretend.
out=$("$B/chaos$EXT" scan 2>&1)
if printf '%s' "$out" | grep -qi "not built"; then
  ok "chaos scan declares itself not built"
else
  bad "chaos scan did not say it is not built: $(printf '%s' "$out" | head -1)"
fi

# ---- 3. the mark and the reader, in a bare terminal -----------------------
if out=$("$B/chaos-qr$EXT" "http://127.0.0.1:$PORT" 2>&1) && [ -n "$out" ]; then
  ok "chaos-qr printed a code ($(printf '%s' "$out" | wc -l) lines)"
else
  bad "chaos-qr printed nothing for a route"
fi

# ---- 3b. the pages, emitted with no C toolchain ---------------------------
# **The Android release depends on this and nothing else checked it here.** The
# APK carries the two pages as assets, emitted at build time so the phone shows
# the same bytes as the browser. That used to be `chaos-serve --emit-pages`,
# which links ggml, so writing two HTML files meant a second full cmake of
# llama.cpp on every release -- and it failed the first time it ever ran.
#
# A page that fetches a stylesheet is the failure that matters: the APK has no
# network on first run, and a `<link>` to Google Fonts renders the art in a
# fallback face.
EMIT=$(mktemp -d 2>/dev/null || echo "./smoke-pages")
if "$B/chaos-qr$EXT" --emit-pages "$EMIT" >/dev/null 2>&1; then
  bad_page=0
  for f in qr scan; do
    p="$EMIT/$f.html"
    [ -s "$p" ] || { bad "chaos-qr --emit-pages wrote no $f.html"; bad_page=1; continue; }
    grep -q '<link' "$p" && { bad "$f.html fetches a stylesheet"; bad_page=1; }
    grep -q 'SIL Open Font License' "$p" || { bad "$f.html lost the font licence"; bad_page=1; }
    grep -q 'data:font/woff2;base64,' "$p" || { bad "$f.html has no embedded font"; bad_page=1; }
  done
  [ "$bad_page" -eq 0 ] && ok "chaos-qr --emit-pages wrote two self-contained pages"
else
  bad "chaos-qr --emit-pages failed"
fi
rm -rf "$EMIT"

# ---- 4. a real node, and every route a person or a device asks for --------
if [ ! -f "$MODEL" ]; then
  echo "SKIP  no model at $MODEL -- the node checks need one"
  echo
  echo "passed $pass, failed $fail (node checks skipped)"
  [ "$fail" -eq 0 ] || exit 1
  exit 0
fi

"$B/chaos-serve$EXT" "$MODEL" --port "$PORT" > "$LOG" 2>&1 &
node=$!
for _ in $(seq 1 40); do
  sleep 1
  curl -s -o /dev/null "http://127.0.0.1:$PORT/health" 2>/dev/null && break
done

get() { # path -> prints "code bytes"
  curl -s -o /tmp/smoke-body -w "%{http_code}" "http://127.0.0.1:$PORT$1" 2>/dev/null
}

for route in / /qr /mark /scan /health /status /v1/models; do
  code=$(get "$route")
  size=$(wc -c < /tmp/smoke-body 2>/dev/null | tr -d ' ')
  case "$code" in
    200) ok "GET $route -> 200, $size bytes" ;;
    *)   bad "GET $route -> ${code:-no response}" ;;
  esac
done

# `/favicon.ico` answers **204 by design** -- there is no icon and an empty body
# is the correct way to say so. This script first called that a failure, which is
# the shape of mistake that gets a check ignored.
code=$(get /favicon.ico)
if [ "$code" = "204" ]; then
  ok "GET /favicon.ico -> 204, as designed"
else
  bad "GET /favicon.ico -> ${code:-no response}, expected 204"
fi

# The mark and the reader must be **self-contained**: a page that fetches
# anything does not work on a machine with no internet, which is the whole point
# of a node printing its own route.
#
# **Only things that FETCH count.** The first version matched any `href="http`,
# which flagged the page's one ordinary `<a>` link to the project's repository --
# an anchor fetches nothing. What must not appear is a stylesheet link, a script
# or image source, or an `@import`: the source HTML *does* carry Google Fonts
# `<link>` tags and the assembly replaces them with embedded fonts, so this is
# the check that the assembly really happened.
for route in /qr /scan; do
  get "$route" > /dev/null
  if grep -qE '<link[^>]*href="http|src="http|@import url\(http' /tmp/smoke-body 2>/dev/null; then
    bad "$route fetches something external"
  else
    ok "$route fetches nothing external"
  fi
done

# **Embeddings are implemented on the dense path**, and three places in two
# documents said 501 for weeks after they were built -- including the SUPPORT.md
# written the same day as this script. They come from a real hidden state, not from logits, and the
# V4-Flash path is refused **by name** because its forward pass exposes none.
code=$(curl -s -o /tmp/smoke-body -w "%{http_code}" -X POST   -H 'content-type: application/json' -d '{"input":"hi"}'   "http://127.0.0.1:$PORT/v1/embeddings" 2>/dev/null)
if [ "$code" = "200" ] && grep -q '"embedding"' /tmp/smoke-body 2>/dev/null; then
  ok "POST /v1/embeddings -> 200 with a vector"
elif [ "$code" = "501" ]; then
  bad "POST /v1/embeddings -> 501; it is implemented on the dense path"
else
  bad "POST /v1/embeddings -> ${code:-no response}"
fi

# And the thing this whole script is about: one machine asking another.
if out=$("$B/chaos$EXT" connect "127.0.0.1:$PORT" "The capital of France is" -n 8 2>&1) \
   && printf '%s' "$out" | grep -qi "paris"; then
  ok "chaos connect got an answer from the node"
else
  bad "chaos connect returned: $(printf '%s' "$out" | head -2 | tr '\n' ' ')"
fi

# A completion through the OpenAI surface, which is what an editor uses.
code=$(curl -s -o /tmp/smoke-body -w "%{http_code}" -X POST \
  -H 'content-type: application/json' \
  -d '{"messages":[{"role":"user","content":"hi"}],"max_tokens":8}' \
  "http://127.0.0.1:$PORT/v1/chat/completions" 2>/dev/null)
if [ "$code" = "200" ] && grep -q '"content"' /tmp/smoke-body 2>/dev/null; then
  ok "POST /v1/chat/completions -> 200 with content"
else
  bad "POST /v1/chat/completions -> ${code:-no response}"
fi

kill "$node" 2>/dev/null
wait "$node" 2>/dev/null

echo
echo "passed $pass, failed $fail"
[ "$fail" -eq 0 ] || exit 1
