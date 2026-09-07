#!/usr/bin/env bash
# Run Claude Code against a local Chaos node instead of Anthropic's API.
#
# The companion to `claude-chaos.cmd`, and the same reasoning: Claude Code reads
# ANTHROPIC_BASE_URL and will talk to whatever answers there. What it needs from
# the other end is the Messages API with tools, which Chaos serves -- see
# `network/serve/src/anthropic.rs` and `docs/CLAUDE-CODE.md`.
#
#   claude-chaos.sh "your prompt"        one turn, then exit
#   claude-chaos.sh                      interactive
#
# Every value is overridable:
#   CHAOS_PORT=9000 CHAOS_TOOLS=Read,Edit claude-chaos.sh
set -u

PORT="${CHAOS_PORT:-8231}"
KEY="${CHAOS_KEY:-local}"

# **The tool set is the single biggest lever on whether this works at all.**
# Claude Code's default twenty-eight tool definitions are 40,255 tokens -- more
# than the 32,768-token context of the models this runs on, before the user
# types anything. Six brings it to 11,706.
TOOLS="${CHAOS_TOOLS:-Read,Write,Edit,Bash,Glob,Grep}"

# **A name from Claude Code's own catalogue, not the local model's.** The name
# is validated client-side BEFORE any request is sent: an unknown one gives
# `[claude-code:unrecognized_model]` and exit 0, with the node never seeing a
# byte. The node ignores it and answers with whatever it has loaded.
MODEL="${CHAOS_MODEL:-claude-opus-5}"

# The real window of the local model. Without this Claude Code assumes 200k and
# compacts against a number the node cannot hold.
CONTEXT="${CHAOS_CONTEXT:-16384}"

if ! command -v claude >/dev/null 2>&1; then
  echo "Claude Code is not installed, or not on PATH." >&2
  echo >&2
  echo "  npm install -g @anthropic-ai/claude-code" >&2
  echo >&2
  echo "That needs Node.js. Then run this again." >&2
  exit 127
fi

if ! curl -s -m 3 -o /dev/null "http://127.0.0.1:$PORT/health" 2>/dev/null; then
  echo "No Chaos node is answering on port $PORT."
  echo
  echo "Start one first, with a model that will call tools:"
  echo
  echo "  chaos-serve <model.gguf> --port $PORT --context $CONTEXT"
  echo
  echo "**Pick the model on whether it calls tools, not on how good it is at"
  echo "code.** Measured: Qwen3-4B calls them, Qwen2.5-Coder-7B does not -- it"
  echo "prints the code and tells you to save it yourself. A model that will"
  echo "not emit a tool call is unusable here however good its code is."
  echo
  echo "Then run this again."
  exit 1
fi

# Its own config directory, so pointing Claude Code at a local model does not
# disturb the history, settings or authentication of a normal `claude`.
export CLAUDE_CONFIG_DIR="${CLAUDE_CONFIG_DIR:-$HOME/.claude-chaos}"
export ANTHROPIC_BASE_URL="http://127.0.0.1:$PORT"
export ANTHROPIC_API_KEY="$KEY"
export ANTHROPIC_MODEL="$MODEL"
export CLAUDE_CODE_MAX_CONTEXT_TOKENS="$CONTEXT"

echo "Claude Code -> Chaos on port $PORT, tools: $TOOLS"
echo "A turn takes minutes on a CPU machine. That is the model, not a hang."
echo
exec claude --tools "$TOOLS" "$@"
