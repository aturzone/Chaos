---
topic: Can Claude Code be pointed at a Chaos node instead of Anthropic's API, and would it be usable? Measured end to end by logging what Claude Code actually sends and timing that exact prompt through the engine
status: MEASURED 2026-09-07. Mechanically possible and three pieces are missing; on THIS laptop it is not usable — one turn costs ~6 minutes of prefill. The tool set is the dominant lever (40,254 tokens at 28 tools, 11,706 at 6) and prefix caching is mandatory, not optional.
links:
  - ../reference/hard-won-facts.md
  - ../backlog/devices-as-resources.md
---

# Claude Code against a Chaos node

**Atur's goal, 2026-09-07**: run a project with Claude Code driven by a model
Chaos is serving locally, connected through the existing `claude-free` command.

Answered by measurement rather than by reading the API documentation, because
the Messages API is large and Claude Code uses a particular slice of it. A
33-line Python server logged the real request and answered it; then that exact
prompt went through `chaos-run` on the engine.

## The mechanism exists already, and Atur was using it

`claude-free` is a `.cmd` that sets `ANTHROPIC_BASE_URL` to a third-party
gateway, sets a key, sets `ANTHROPIC_MODEL=claude-opus-5`, and runs `claude`.
So **Claude Code accepts an arbitrary base URL** — pointing it at
`http://127.0.0.1:<port>` is the same trick with a different host. Nothing needs
to be reverse-engineered to get that far.

Two things bite immediately:

- **The model name is validated client-side, before any request is sent.**
  `ANTHROPIC_MODEL=chaos-probe` produced
  `[claude-code:unrecognized_model]` and exit 0 — the probe never saw a byte.
  A name from its own catalogue (`claude-opus-5`) is required, so the server has
  to ignore the requested name and answer with whatever is loaded. That is
  exactly what `claude-free` does.
- **`CLAUDE_CODE_MAX_CONTEXT_TOKENS` has to be set** to the local model's real
  window, or Claude Code assumes 200k and compacts against the wrong number.

## What it sends

`POST /v1/messages?beta=true`, `anthropic-version: 2023-06-01`, a long
`anthropic-beta` list, `x-api-key`, and a body with `model`, `max_tokens: 64000`,
`stream: true`, `system` (a **list** of 3 blocks), `messages`, `tools`,
`metadata`, `thinking`, `context_management`, `output_config`.

**Content-Length on a bare `claude -p "hi"`: 162,229 bytes.**

## The number that decides it

Fed through `chaos-run` with the real Qwen3 tokenizer, the default request is
**40,255 tokens** — and every model on this machine has a **32,768-token native
context**. Claude Code's opening request does not fit before the user types
anything.

**The tool definitions are the whole problem, and `--tools` is the lever:**

| `--tools` | defs | chars the model reads | tokens |
|---|---|---|---|
| `default` | 28 | 160,893 | **40,254** |
| `Read,Write,Edit,Bash,Glob,Grep` | 6 | 46,790 | **11,706** |
| `Read,Edit,Bash` | 3 | 42,223 | 10,564 |
| `""` | 0 | 36,591 | 9,155 — the system prompt floor |

Six tools fit a 32k window with room to spare. **The floor is 9,155 tokens**:
Claude Code's own system prompt, which cannot be reduced.

## What it costs, measured

That 6-tool prompt through `chaos-run` on **Qwen3-4B**, the fastest model here
that could plausibly hold a conversation:

```
prompt     12,531 tokens
generate   24 tokens in 22.1s (1.09 tok/s)
kv cache   12,531 positions, 1762.2 MiB, f16
time       0.0s disk, 28.6s qkv, 196.9s attention, 116.1s ffn, 1.2s other
total      376.7s
```

**~354 seconds of prefill, then 1.09 tok/s.** Two things to take from it:

- Generation at 12.5k context is **1.09 tok/s against 8.27 tok/s** short —
  7.6x slower, which is a sharper context penalty than the 2.68x the README
  records at 4k.
- Attention is 197s of the 354s. This is dense attention over 12.5k positions,
  not a streaming problem: `0.0s disk`.

A Claude Code task is several turns — tool call, result, next turn — and **each
turn re-prefills the whole prompt**. "Read a file and fix a typo" is three or
four turns: **half an hour**, most of it re-reading the same 12.5k tokens.

## The three missing pieces, in the order they matter

1. **Prefix caching across requests.** `network/serve` has none: every request
   prefills from scratch. Claude Code already marks the prefix with
   `cache_control: {"type":"ephemeral"}`, so the client side is done and the
   server ignores it. **This is what makes the difference between unusable and
   usable** — with it, turn 1 pays 6 minutes and every later turn pays only for
   the delta.
2. **`POST /v1/messages`**, with SSE in Anthropic's event order
   (`message_start`, `content_block_start`, `content_block_delta`,
   `content_block_stop`, `message_delta`, `message_stop`). Chaos serves
   `/v1/chat/completions` in OpenAI's shape and nothing in Anthropic's.
3. **Tool calling, which Chaos does not have at all.** Zero references to
   `tools`, `tool_calls`, `tool_choice` or `function_call` anywhere in the
   workspace. It needs the `tools` array rendered into the prompt — the
   container's own chat template takes a `tools` variable and `chaos-jinja` can
   set it — and the model's output parsed back into `tool_use` blocks. Without
   this Claude Code connects and can hold a conversation while being unable to
   read a file.

## The honest verdict

**Buildable, and not usable on this laptop.** The blocker is not the protocol,
it is 354 seconds of prefill per turn on a 15.7 GiB CPU machine. Even with all
three pieces built, a turn is minutes.

**What would change the answer is hardware, not code.** The 5090 box in
`CLAUDE.md` (32 GiB VRAM, 64 GiB RAM) fits Qwen3-30B-A3B entirely in VRAM, where
a 12.5k prefill is seconds rather than minutes. That is where this feature is
worth demonstrating.

**What is worth building here anyway**: prefix caching (1) helps every Chaos
user, not just this one, and tool calling (3) is a capability the OpenAI surface
is also missing — an editor pointed at `/v1/chat/completions` cannot use tools
either. Both stand on their own merits.
