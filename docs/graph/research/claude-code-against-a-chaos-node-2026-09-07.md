---
topic: Can Claude Code be pointed at a Chaos node instead of Anthropic's API, and would it be usable? Measured end to end by logging what Claude Code actually sends and timing that exact prompt through the engine
status: BUILT AND WORKING 2026-09-07. Claude Code drives a local model end to end -- tool call, execution, result, correct answer. `/v1/messages` with tools and a prefix cache are in; turn 2 went 135.6s -> 52.9s reusing 3,793 of 3,917 tokens. Still minutes per turn on this CPU machine, and that is hardware.
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


## BUILT, and it works end to end

All three pieces landed the same day the measurement was taken, and the first
real task completed: `claude -p "Read notes.txt and tell me what city it names"`
against Qwen3-4B on a Chaos node. The model emitted a `Read` tool call, Claude
Code executed it, fed the `tool_result` back, and the model answered **Lyon** --
quoting the file with Read's own line numbering, so it plainly read the result
rather than guessing.

| piece | where |
|---|---|
| `POST /v1/messages`, tools, Anthropic SSE | `network/serve/src/anthropic.rs`, 22 tests |
| prefix cache across requests | `run_prompt`, `State::truncate_to` |
| the wrapper and the setup path | `scripts/claude-chaos.cmd`, `docs/CLAUDE-CODE.md` |

**Measured on the two-turn task**, six tools:

```
before the prefix cache   turn 1 243.5s   turn 2 135.6s
after                     turn 1 386.0s   turn 2  52.9s   3,793 of 3,917 reused
```

Turn 1 differs because the model generated 904 tokens rather than 562, not
because anything regressed. **Turn 2 is the number that matters: 2.6x.**

Reuse is verified not to change the answer -- the same turn warm and cold gives
identical output, checked with temperature 0 and a deliberately clobbered cache
in between. That check matters more than the speed one: a mismatched prefix is
silently wrong output, not an error.

### The model choice is about tool calls, and the ranking is not the obvious one

Two whole tasks, five tools, same node, same prompts:

| model | read a file | write a file |
|---|---|---|
| **Qwen3-4B** (2.3 GB) | **called `Read`**, acted on the result, answered correctly | **called `Write`**, file created with correct contents |
| Qwen2.5-Coder-7B-Instruct (4.4 GB) | not tried | **refused twice.** Printed the code and said *"you can save this as hello.py"*; on a second attempt with firmer wording it suggested `echo ... > hello.py` instead. No file either time |

**So the code model is the wrong choice and the small general one is right**,
which inverts the recommendation this node made before the build. A model that
will not emit a tool call is unusable for an agent however good its code is:
it connects, converses, and changes nothing.

**One hypothesis was tested and failed.** The tool instruction ended *"call a
function only when you need its result. Otherwise answer normally"*, and the
7B plainly took the second half as permission. Rewriting it as a true statement
-- that prose does not touch the user's files and a call is the only thing that
does -- changed the shape of the refusal and not the refusal. The wording is
kept because it is more accurate, **not because it fixed anything**.

The lever that might work is **grammar-constrained decoding**: `Params.grammar`
and `chaos-grammar` are both here. It is not attempted, because forcing the
syntax would force a call on every turn and sometimes prose is the right answer.
Constraining only *after* the model has emitted `<tool_call>` is the shape that
would work, and is unbuilt.


### Three things the build found that the measurement had not

- **`chaos-serve` capped the dense path at 2,048 tokens** and `-c` could only
  lower it, because the server prefilled in one pass while `chaos-run` had
  chunked for months. **That made every agent client impossible**, not just
  Claude Code -- an editor sending one file exceeds it. Now chunked, ceiling
  16,384, chosen from KV memory rather than from the model.
- **Qwen3's `<think>` blocks reached the client as the answer.** The very first
  live request spent all 40 tokens of budget reasoning and returned the
  reasoning. `strip_thinking` handles it, and it can be simpler than the phone's
  `ThinkFilter` because this path buffers whole answers.
- **The single-threaded accept loop can wedge.** After two turns the port stayed
  in `Listen` with three connections in `CLOSE_WAIT` and new ones timing out.
  There was a 3 s read timeout and no write timeout, on the reasoning that
  "writes can take minutes" -- true of a whole stream, false of one `write_all`.
  A 60 s write deadline bounds it. **Observed once, not reproduced on demand**,
  so that is the mechanism fixed rather than a demonstrated repair.

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

## The three pieces that were missing -- all three now built

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

**Built, working, and slow.** The protocol was never the hard part; 354 seconds
of prefill on a 15.7 GiB CPU machine is. The prefix cache removes that cost
from every turn after the first: **turn 2 went from 135.6 s to 52.9 s, 2.6x**.
The two-turn *totals* are not comparable -- 379 s before, 439 s after --
because turn 1 generated 904 tokens in the second run against 562 in the
first, and that is generation rather than prefill. A four-turn task is still a
coffee break.

**What changes the answer is hardware, not code.** The 5090 box in `CLAUDE.md`
(32 GiB VRAM, 64 GiB RAM) fits Qwen3-30B-A3B entirely in VRAM, where a 12.5k
prefill is seconds rather than minutes. That is where this is worth
demonstrating, and where it stops being a demo.

**Two of the three pieces stand on their own merits** regardless of Claude
Code. The prefix cache helps every client that re-sends a conversation, and
tool calling was missing from the OpenAI surface too -- an editor pointed at
`/v1/chat/completions` could not use tools either. The 2,048-token dense
ceiling made both of those moot and nobody had noticed.
## Re-run for v0.0.34, 2026-09-08

Atur asked for this verified again before the release: *"ready that auto setup
claude code for agentic code works with last version with API of chaos without
any problem, test it for sure"*. So it was run again, on the release binaries,
with a fact the model could not have guessed.

`notes.txt` in an empty scratch directory contained one invented string,
`BLUEBERRY-7742`. Then, against `Qwen3-4B-Q4_K_M` on this laptop's CPU:

```
chaos-serve Qwen3-4B-Q4_K_M.gguf --port 8231 -c 16384
claude --tools Read,Write,Edit,Bash,Glob,Grep \
  -p "Read the file notes.txt in this directory and tell me the launch code it contains."
```

What the node logged, in order:

```
GET  /health              -> 200
HEAD /api/hello           -> 404          <- fixed in this release
POST /v1/messages?beta=true -> 200 in 329.0s (605 tokens, tool_use)
POST /v1/messages?beta=true -> 200 in 212.5s (227 tokens, end_turn)
```

and what Claude Code printed:

> The launch code in the `notes.txt` file is **BLUEBERRY-7742**.

**10m12s wall clock for the round trip**, and it is a real round trip: turn 1
returned `tool_use`, Claude Code executed the `Read`, and turn 2 returned
`end_turn` carrying a string that existed nowhere except inside that file. The
model could not have produced it without the tool actually running.

### Run again on the finished binaries

The run above was made before `/api/hello` was fixed. Repeated on exactly what
ships, in a fresh directory whose `project.txt` held two invented facts:

```
GET  /health              -> 200
GET  /api/hello           -> 200          <- was a 404
HEAD /api/hello           -> 200
POST /v1/messages?beta=true -> 200 in 273.5s (465 tokens, tool_use)
POST /v1/messages?beta=true -> 200 in  45.0s (114 tokens, end_turn)
```

> The project name is **ORCHID-BRIDGE** and the budget is **41 units**.

Both correct, both only in the file. **6m20s wall clock, against 10m12s** —
and the difference is almost entirely turn 2, **45.0s against 212.5s**, which
is the prefix cache doing what it was built for on a real second turn rather
than a synthetic one.

Two things this run corrected:

- **`HEAD /api/hello` was a 404.** It is the first request Claude Code sends,
  to decide whether the endpoint behind `ANTHROPIC_BASE_URL` is reachable, so
  a perfectly working node logged an error on every start. It answers `200
  {"message":"Hello"}` now, outside the API key -- it carries nothing about the
  node, no model name and no context size, the same reasoning that leaves the
  mark open.
- **The node serialises requests**, one at a time, and that is easy to
  misread. A `curl` sent while Claude Code was mid-turn sat for four minutes
  and looked like a hang; it was a queue. The same request answered in 7.8s
  once the turn was over.

## From the published installer, which is the path a user actually takes

Atur, 2026-09-08: *"make sure that with the new version I can install it and use
a model with Claude Code"*. The runs above used the build tree. This one used
nothing but what the release page serves:

```
curl -> Chaos-v0.0.34-windows-x86_64-Setup.exe   42,212,977 bytes
Setup.exe --silent --prefix <throwaway>          -> 0.0.34, 21 exes,
                                                    claude-chaos.cmd/.sh,
                                                    CLAUDE-CODE.md
<prefix>/bin/chaos-serve.exe Qwen3-4B-Q4_K_M.gguf --port 8231 -c 16384
<prefix>/bin/claude-chaos.cmd -p "Read RELEASE.txt ... secret handshake."

  HEAD /api/hello             -> 200
  POST /v1/messages?beta=true -> 200 in 315.9s (421 tokens, tool_use)
  POST /v1/messages?beta=true -> 200 in  64.9s (162 tokens, end_turn)
```

> The secret handshake from `RELEASE.txt` is **PEWTER-KESTREL-88**

**7m26s**, and the wrapper printed its own banner first — *"A turn takes minutes
on a CPU machine. That is the model, not a hang."* — which is the line that
stops the wait looking like a failure.

Three things worth keeping from it:

- **A throwaway `--prefix` is how to test an installer without risking the
  user's install.** Atur's own copy is v0.0.31; installing over it to prove the
  installer works would have been a poor trade.
- **His `PATH` still resolves `chaos` to v0.0.31.** Installing to a prefix does
  not change that, and neither would installing to the default while a shell was
  already open. Worth saying out loud rather than assuming a user knows.
- **`claude -p` with no stdin prints a warning** — *"no stdin data received in
  3s, proceeding without it"*. Harmless, and it only appears non-interactively:
  a person running the wrapper in a terminal has a TTY. Not a defect, but it
  will show up in any scripted use.

**One measurement that looked like a regression and was not.** A 45 KB request
body appeared to take 67.5s while a 5 KB body took 2.5s, which pointed straight
at the two-phase read timeout added the day before. It was the *test data*: 45,000
identical bytes is a pathological input for a byte-level BPE. With realistic
prose, a 45 KB body answers in 0.0s and a 160 KB body in 0.1s -- and reverting
the timeout reproduced the 68s exactly, which is what proved the change
innocent. **A synthetic input can be adversarial by accident.**
