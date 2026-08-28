---
topic: §4e — what happens when things go wrong, tested rather than reasoned about
status: resolved — five failure modes exercised, one real bug fixed, two gaps left open
links:
  - ../backlog/v0-0-3-the-complete-version.md
  - 4d-tests-2026-08-28.md
  - chaos-pull-corrupt-resume.md
---

# §4e: production readiness

§4e names the failure modes: **a corrupt model, a full disk, a network dropping
mid-stream, two instances at once, crash behaviour, and install/update/uninstall
on a clean machine.** Five were exercised on a real node with a throwaway
profile. One could not be simulated safely and is declared. One real bug was
found and fixed, and **one thing I reported as a bug was my own probe again.**

## 1. A corrupt model — the worst result here, and nothing detects it

Four kinds of broken container, none touching the real model:

| broken how | what happens | exit |
|---|---|---|
| zero bytes | `truncated while reading magic: needed 4 bytes, 0 left` | **1** |
| 200 random bytes | `not a GGUF file (magic was 0xe5024024, expected 0x46554747)` | **1** |
| truncated to 1 MB (a stopped download) | `truncated while reading string: needed 8 bytes, 5 left` | **1** |
| **4 KiB of zeros written into the weights** | **loads, runs, answers fluently** | **0** |

The first three are exemplary: they name the byte counts and the expected magic,
and they exit non-zero.

**The fourth is the project's central trap, demonstrated.** A structurally valid
container with 4 KiB zeroed deep in the tensor data loads with no warning, exits
0, and answers the same prompt *differently*:

```
prompt:  "The capital of France is"
intact:  " Paris. The capital of France is Paris. The capital of"
corrupt: " Paris. The capital of Germany is Berlin. The capital of"
```

Both are fluent. Both are plausible. Nothing anywhere reports damage. **A corrupt
model is indistinguishable from a working one from the outside**, which is the
same fact `hard-won-facts.md` states about a wrong forward pass, arriving by a
different door.

**And there is no checksum.** `chaos_model::download` verifies with
`looks_like_gguf` — the magic bytes — not a hash. So a download that resumed
badly, or a file that bit-rotted, keeps a valid header and produces confident
wrong answers forever. `chaos-pull-corrupt-resume` already records the adjacent
version of this: a resumed download can end up *too large* and pass every check.

**Recommended, not done here**: record each container's size and a hash at pull
time in `~/.chaos/models`, and check it on load behind a flag. That is a real
feature with a real cost (hashing 144 GB is minutes), so it is Atur's call — but
the current state should be stated in the README rather than discovered.

## 2. Two instances at once — it pays for the model before it checks the port

A node was running on 8231. Starting a second server on the same port:

```
model      llama (direct (cache bypassed))
weights    147 tensors, 0.74 GiB bound in 0.7s (zero-copy)
chaos-serve: Only one usage of each socket address ... is normally permitted. (os error 10048)
exit 1
```

It exits non-zero, which is right. Two things are wrong with getting there:

- **The port is bound last.** It loaded the entire model first — 0.7 s for a
  762 MiB file, and **minutes for a 144 GB one** — before discovering the address
  was taken. Binding first, or probing the port before the load, is a few lines
  and saves the whole wasted read.
- **The message is the raw OS string.** Compare it with the GGUF errors above,
  which name what was needed and what was found. This one does not say *which*
  port, that another node may be running, or that `chaos stop` and `--port` exist.
  `chaos start` already refuses cleanly when its own pid file shows a live node —
  but that only covers nodes it started.

## 3. A network dropping mid-stream — and a correction

**What I first reported was wrong.** I killed the node four seconds into an
answer, saw the text stop mid-word at "Sevent" and the client exit 0, and called
it a bug: a dropped connection indistinguishable from a finished one.

Then I ran the same prompt and **killed nothing at all**. It still ended at
"Sevent", exit 0. The node had hit its default `max_tokens` and sent `[DONE]`; the
answer was *complete* and the exit code was correct. My kill landed after the
generation had already finished. **Identical truncation point in two runs was the
tell, and I should have checked it before writing the word "defect".** That is the
third time today a probe was wrong and the code was right.

**A genuine mid-stream drop, killing at 0.8 s:**

```
Here is the count from one to 200, one number per line
chaos: the connection broke after 15 chunk(s): An existing connection was forcibly
       closed by the remote host. (os error 10054). What you have above is incomplete.
CLIENT EXIT=2
```

Correct: it prints what arrived, says the answer is incomplete, says how much
arrived, and exits non-zero.

**The bug was real, just not the one I saw.** `post_sse` returned `Ok` when the
stream ended in a clean EOF without `[DONE]`, and a clean EOF is what a *gracefully*
closed connection gives — a proxy timing out, a node shut down politely, a
half-open socket. A killed process sends RST, which already produced an error, so
the live test could not reach the broken path. Both paths now report truncation,
and **`core/http` has five socket tests against a fake node** — a real
`TcpListener` on an ephemeral port, because a fake that cannot be cut off cannot
reproduce the bug:

- a complete stream delivers every chunk and returns `Ok`;
- **a stream that stops without `[DONE]` is an error** naming how many chunks
  arrived, while still delivering what did arrive;
- a caller that cancels is not an error;
- a 401 carries the node's own message, not just a number;
- `get` reads a body with *or* without `Content-Length`.

Writing that fake node taught one thing worth keeping: **dropping a socket with
unread data in its receive queue makes Windows send RST, not FIN**, so the first
version of the test exercised the wrong path and failed for the wrong reason.
`shutdown(Write)` then draining is what produces a clean EOF.

### Also found: the default token cap truncates mid-word, invisibly

The answer above stops at "Sevent" because the node's default `max_tokens`
expires there. Nothing says so — no "stopped: length" line — and `chaos connect`
has no flag to raise it. A person asking for a long answer gets a short one, cut
mid-word, with no explanation. `finish_reason` is already computed and tested in
`network/serve`; it is simply not surfaced to this client.

## 4. A full disk — declared, not simulated

**Not tested, and deliberately.** Filling the drive that holds this worktree, the
models and the operating system is not a thing to do on somebody's working
machine, and a simulated `ENOSPC` on a temp file proves nothing about the streaming
path. What *can* be said from reading: `chaos_io`'s writes are checked, and
`chaos-pull` resumes with `curl -C -`, which is what makes the corrupt-resume
problem above possible. **Any claim about behaviour on a full disk would be
unmeasured, so no claim is made.**

## 5. Crash behaviour

`Cargo.toml` sets `panic = "abort"` for release. So a panic in any binary is an
immediate process death: no unwinding, no destructor, no backtrace unless the
environment asks for one. `gui/app` relies on this being understood — the whole
"no Win32 call while `UI` is mutably borrowed" rule exists because a `RefCell`
double borrow under `abort` is *"instant, silent process death — no message, no
log, no stack"*, enforced textually by `ui_rules.rs`.

For a node started by `chaos start`, an abort leaves a **stale pid file**, which
`chaos status` reports honestly (*"a pid file is present but the process is
gone"*) and `chaos stop` clears. That path was tested in §3.

## 6. Install, update, uninstall

**Windows: verified**, and by the published files rather than a build —
`scripts/install-update-uninstall.ps1`, with `~/.chaos/models` byte-identical
across the cycle at 18 files and 120,933,521,300 bytes (STATUS has the record).

**Everywhere else: nothing does this.** `make-linux-packages.sh` produces a
`.deb` and an AppImage, both published in v0.0.21, and **neither has been
installed anywhere by anybody**. macOS ships two tarballs with no install script
at all — the README tells the user to `sudo install -m 755` the binaries
themselves. That is the largest untested surface in the release, and it is one
virtual machine away from being tested.

## What §4e changes

- **One real bug fixed**, with five new socket tests: an SSE stream that ends
  without `[DONE]` is now a reported truncation rather than a silent short answer.
- **One correction to my own reporting**: the live "it lies" observation was a
  completed answer, not a dropped one.
- **The corrupt-model result is the finding that matters**: nothing detects
  damage inside the weights, and the demonstration is two fluent answers that
  disagree.
- **Two cheap fixes named, not done**: bind the port before loading the model, and
  surface `finish_reason` so a capped answer says it was capped.
- **Two gaps declared rather than papered over**: full-disk behaviour is
  unmeasured, and install/update/uninstall is unverified on every platform except
  Windows.
