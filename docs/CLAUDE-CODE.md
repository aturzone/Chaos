# Claude Code on your own model

Chaos serves the **Anthropic Messages API**, so `claude` can be pointed at a
model running on your machine instead of at Anthropic's servers. Tool calling
works: the model reads files, runs commands and edits code through Claude Code's
own tools.

**Read the speed section before you start.** This works, and on a CPU machine a
turn takes minutes.

---

## Zero to a working turn

### 1. A model that can hold a conversation

Not any model. It needs a chat template, tool-call training, and a context
window with room for Claude Code's system prompt.

**Pick it on whether it calls tools, not on how good it is at code.** That
turned out to be the only thing that matters, and it is not the same ranking:

| model | size | calls tools? |
|---|---|---|
| **Qwen3-4B** | 2.3 GB | **yes — measured twice.** Called `Read` and acted on the result; called `Write` and created the file. Start here |
| Qwen2.5-Coder-7B-Instruct | 4.4 GB | **no.** Asked to create a file it printed the code and said *"you can save this as hello.py"*; asked again it suggested an `echo > hello.py` command. Never called the tool, twice, with two different instruction wordings |
| Qwen3-30B-A3B | 18 GB | untested here. Stronger, and needs `--force`: its parity diff has 1 FAIL |

**A model that will not emit a tool call is unusable for this** however good its
code is — the agent connects, converses, and changes nothing. That is the
failure mode to expect, and it looks like the model refusing to work.

Chaos deliberately does not paper over it: a malformed or absent tool call stays
text, because inventing a `tool_use` block would make the agent run something
the model never asked for.

### 2. Start a node

```
chaos-serve <model.gguf> --port 8231 --context 16384
```

`--context` matters. The default is smaller than Claude Code's opening request.

### 3. Run Claude Code against it

```
scripts\claude-chaos.cmd "read notes.txt and tell me what it says"
```

That script is the whole setup: it checks the node is up, sets
`ANTHROPIC_BASE_URL` and the three other variables that matter, restricts the
tool set, and runs `claude`. Every value can be overridden:

```
set CHAOS_PORT=9000
set CHAOS_TOOLS=Read,Edit,Bash
scripts\claude-chaos.cmd
```

With no arguments you get the normal interactive Claude Code, talking to your
model.

---

## The four settings, and why each is needed

Doing it by hand is four environment variables. Each one exists because of
something that otherwise fails:

| variable | value | what goes wrong without it |
|---|---|---|
| `ANTHROPIC_BASE_URL` | `http://127.0.0.1:8231` | it talks to Anthropic |
| `ANTHROPIC_API_KEY` | anything | the client refuses to start with no key |
| `ANTHROPIC_MODEL` | `claude-opus-5` | **the name is validated client-side before any request is sent**: an unknown one gives `[claude-code:unrecognized_model]`, exit 0, and the node never sees a byte. The node ignores the name and answers with whatever it loaded |
| `CLAUDE_CODE_MAX_CONTEXT_TOKENS` | your `--context` | Claude Code assumes 200k and compacts against a number your model cannot hold |

`CLAUDE_CONFIG_DIR` is also set, to a separate directory, so this does not
disturb the history and settings of a normal `claude`.

---

## `--tools` is the setting that decides whether it works at all

Claude Code's **default tool set does not fit in any model that runs here.**
Measured with the real Qwen3 tokenizer:

| `--tools` | definitions | tokens before you type anything |
|---|---|---|
| `default` | 28 | **40,255** |
| `Read,Write,Edit,Bash,Glob,Grep` | 6 | **11,706** |
| `Read,Edit,Bash` | 3 | 10,564 |
| `""` | 0 | 9,155 — Claude Code's own system prompt, irreducible |

A 32,768-token model cannot take the default set *and* leave room for a
conversation. The wrapper defaults to six tools for that reason. Add tools when
you need them and watch the first-turn time grow.

---

## Speed, measured

Qwen3-4B on an i7-13650HX with 15.7 GiB, five tools, two whole tasks:

```
reading a file      turn 1  386.0s  tool_use    turn 2  52.9s   3,793/3,917 reused
writing a file      turn 1  352.4s  tool_use    turn 2  67.4s
```

**Turn 1 is the expensive one.** After it, the node keeps the KV cache and the
next turn only prefills what changed — which took turn 2 from 135.6s to 52.9s
when that landed, a 2.6x saving. The reuse is verified not to change the
answer: the same turn warm and cold produces identical output.

Two things follow:

- **Keep one node running.** Restarting it throws the cache away and the next
  turn pays turn-1 prices again.
- **A CPU machine gives you minutes per turn.** A four-turn task is a coffee
  break. This is the model and the hardware, not a hang — the wrapper says so
  on startup.

What changes this is hardware, not settings. A machine that fits the model in
VRAM prefills 12k tokens in seconds rather than minutes.

---

## When something looks broken

**"unrecognized_model" and it exits immediately.** `ANTHROPIC_MODEL` must be a
name from Claude Code's own catalogue. Use `claude-opus-5`.

**It hangs with no output for minutes.** Expected on turn 1. The node logs
every request with its duration; watch that instead.

**The model answers in prose instead of calling a tool.** It did not emit a
tool call, and Chaos deliberately does not invent one — a fabricated `tool_use`
block would make the agent run something the model never asked for. **Change
the model, and do not assume a code model is better at this**: Qwen2.5-Coder-7B
is trained for code and would not call `Write` on two tries with two different
instruction wordings, while Qwen3-4B does. Fewer tools also helps, by leaving
more of the context for the conversation.

**The answer is the model's reasoning.** Qwen3 writes `<think>` blocks; Chaos
strips them. If you see thinking in the answer, the model used a syntax we do
not recognise — report it with the raw text.

**A prompt is refused as too long.** Reduce `--tools`, or raise `--context` if
you have the memory. The KV cache costs about 1.7 GiB for 12,500 tokens on a
4B model.

---

## What is not built

- **Token-by-token streaming.** The answer is buffered, then sent as one set of
  stream events. A tool call cannot be recognised until `<tool_call>` has been
  seen and text already sent cannot be recalled, so buffering is what makes
  tool use possible at all. At ~1 tok/s the difference is not what you would
  notice.
- **Prompt caching across restarts.** The cache is in memory and lives as long
  as the node.
- **Reuse on the V4-Flash path.** Its cache cannot be truncated to an arbitrary
  position — raw latents live in a ring and the compressed halves summarise
  completed blocks — so that path prefills from scratch every turn. At 0.728
  tok/s it is not the model to drive an agent with anyway.
- **Anything that needs `thinking` blocks, images, or the beta features in
  Claude Code's `anthropic-beta` header.** Unknown blocks are named in the
  transcript rather than silently dropped.

The protocol lives in `network/serve/src/anthropic.rs`; the measurements behind
every number here are in
`docs/graph/research/claude-code-against-a-chaos-node-2026-09-07.md`.
