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

# `scan` was removed in v0.0.34 with the reader it named, so it must be
# treated as what it is: not a command. **This check used to assert the
# opposite** -- that `chaos scan` said "not built" -- which is how a refusal
# pointing at two deleted features survived a release with a green surface run.
out=$("$B/chaos$EXT" scan 2>&1)
if printf '%s' "$out" | grep -q "is not a command"; then
  ok "chaos scan is not a command, and says so"
else
  bad "chaos scan still claims to be something: $(printf '%s' "$out" | head -1)"
fi

# ---- 3. the mark, in a bare terminal --------------------------------------
if out=$("$B/chaos-qr$EXT" "http://127.0.0.1:$PORT" 2>&1) && [ -n "$out" ]; then
  ok "chaos-qr printed a code ($(printf '%s' "$out" | wc -l) lines)"
else
  bad "chaos-qr printed nothing for a route"
fi

# ---- 2b. no help text names something that was deleted --------------------
# **The bug this catches shipped for a whole release.** `chaos scan` refused to
# decode and helpfully named the two readers that did work -- the phone app's
# SCAN button and `/scan` in a browser. v0.0.34 deleted both, the command went
# on naming them, and a unit test asserted that it did. A working command giving
# confident directions to somewhere demolished is worse than a missing one.
#
# `chaos-serve --help` had the same shape: it offered `--emit-pages` writing
# "qr.html and scan.html" for "the Android APK", when scan.html and the APK were
# both gone and it writes one file.
#
# So: nothing any binary prints when asked for help may name a deleted feature.
# One check over every binary, rather than a memory of which ones to look at.
ghosts=0
for b in chaos chaos-run chaos-serve chaos-probe chaos-pull chaos-qr chaos-meta \
         chaos-draw chaos-worker chaos-model-info gguf-info; do
  [ -x "$B/$b$EXT" ] || continue
  help=$("$B/$b$EXT" --help 2>&1 || true)
  for ghost in 'scan\.html' '[Aa]ndroid' '\.apk' '/scan' 'READ A CODE' 'CHANGE MODE'; do
    if printf '%s' "$help" | grep -qE "$ghost"; then
      bad "$b --help names '$ghost', which was deleted"
      ghosts=1
    fi
  done
done
[ "$ghosts" -eq 0 ] && ok "no binary's help names a deleted feature"

# ---- 3b. the page, emitted with no C toolchain ----------------------------
# **Written for the Android release, which is gone; kept because the reason
# outlived it.** Any host that embeds the mark wants the same bytes the browser
# serves, without building an inference engine to get them. That used to be
# `chaos-serve --emit-pages`, which links ggml, so writing one HTML file meant a
# second full cmake of llama.cpp -- and it failed the first time it ever ran.
# `chaos-qr` has no ggml and emits the same page.
#
# A page that fetches a stylesheet is the failure that matters: an embedded copy
# may have no network at all, and a `<link>` to Google Fonts renders the art in
# a fallback face.
EMIT=$(mktemp -d 2>/dev/null || echo "./smoke-pages")
if "$B/chaos-qr$EXT" --emit-pages "$EMIT" >/dev/null 2>&1; then
  bad_page=0
  for f in qr; do
    p="$EMIT/$f.html"
    [ -s "$p" ] || { bad "chaos-qr --emit-pages wrote no $f.html"; bad_page=1; continue; }
    grep -q '<link' "$p" && { bad "$f.html fetches a stylesheet"; bad_page=1; }
    grep -q 'SIL Open Font License' "$p" || { bad "$f.html lost the font licence"; bad_page=1; }
    grep -q 'data:font/woff2;base64,' "$p" || { bad "$f.html has no embedded font"; bad_page=1; }
  done
  [ "$bad_page" -eq 0 ] && ok "chaos-qr --emit-pages wrote a self-contained page"
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

for route in / /qr /mark /health /status /v1/models; do
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

# The mark must be **self-contained**: a page that fetches
# anything does not work on a machine with no internet, which is the whole point
# of a node printing its own route.
#
# **Only things that FETCH count.** The first version matched any `href="http`,
# which flagged the page's one ordinary `<a>` link to the project's repository --
# an anchor fetches nothing. What must not appear is a stylesheet link, a script
# or image source, or an `@import`: the source HTML *does* carry Google Fonts
# `<link>` tags and the assembly replaces them with embedded fonts, so this is
# the check that the assembly really happened.
for route in /qr; do
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

# **The Anthropic surface, which is what Claude Code speaks.** A separate
# protocol from the one above, not a variant: system blocks, content blocks,
# tools, and a load-bearing `stop_reason`. Checked here because the route being
# absent is exactly how "devices cannot connect" looked one release ago, and a
# unit test cannot see a route that was never wired into the table.
code=$(curl -s -o /tmp/smoke-body -w "%{http_code}" -X POST   -H 'content-type: application/json'   -d '{"model":"claude-opus-5","max_tokens":16,"messages":[{"role":"user","content":[{"type":"text","text":"hi"}]}]}'   "http://127.0.0.1:$PORT/v1/messages" 2>/dev/null)
if [ "$code" = "200" ] && grep -q '"type":"message"' /tmp/smoke-body 2>/dev/null    && grep -q '"stop_reason"' /tmp/smoke-body 2>/dev/null; then
  ok "POST /v1/messages -> 200, an Anthropic message"
else
  bad "POST /v1/messages -> ${code:-no response}"
fi

# The query string Claude Code actually sends. The router matches a
# query-stripped path, and this is what proves it.
code=$(curl -s -o /dev/null -w "%{http_code}" -X POST   -H 'content-type: application/json'   -d '{"max_tokens":8,"messages":[{"role":"user","content":"hi"}]}'   "http://127.0.0.1:$PORT/v1/messages?beta=true" 2>/dev/null)
if [ "$code" = "200" ]; then
  ok "POST /v1/messages?beta=true -> 200 (the query string Claude Code sends)"
else
  bad "POST /v1/messages?beta=true -> ${code:-no response}"
fi

# The count a client compacts on, from the real tokenizer.
code=$(curl -s -o /tmp/smoke-body -w "%{http_code}" -X POST   -H 'content-type: application/json'   -d '{"messages":[{"role":"user","content":"hi"}]}'   "http://127.0.0.1:$PORT/v1/messages/count_tokens" 2>/dev/null)
if [ "$code" = "200" ] && grep -q '"input_tokens"' /tmp/smoke-body 2>/dev/null; then
  ok "POST /v1/messages/count_tokens -> 200 with a count"
else
  bad "POST /v1/messages/count_tokens -> ${code:-no response}"
fi

# A malformed body must be refused by name rather than crashing the node.
code=$(curl -s -o /tmp/smoke-body -w "%{http_code}" -X POST   -H 'content-type: application/json' -d 'not json'   "http://127.0.0.1:$PORT/v1/messages" 2>/dev/null)
if [ "$code" = "400" ] && grep -q '"type":"error"' /tmp/smoke-body 2>/dev/null; then
  ok "POST /v1/messages with a bad body -> 400, named"
else
  bad "POST /v1/messages with a bad body -> ${code:-no response}, expected 400"
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
