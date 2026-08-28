# Security Policy

## Supported versions

Chaos is at **v0.0.0**, a preview. Only the latest release and `main` receive
fixes. There is no long-term support branch yet, despite the LTS ambition in the
roadmap.

| version | supported |
|---|---|
| `main` | ✅ |
| 0.0.0 | ✅ |
| anything earlier | ❌ (none exists) |

## Reporting a vulnerability

**Please do not open a public issue for a security problem.**

Use GitHub's private reporting:
[Security → Report a vulnerability](https://github.com/aturzone/Chaos/security/advisories/new).

If that is unavailable to you, contact the maintainer through their GitHub
profile ([@aturzone](https://github.com/aturzone)) and ask for a private channel
before sending details.

Expect an acknowledgement within a week. This is a solo project, so please be
patient — and please do give it a reasonable disclosure window before publishing.

## What is in scope

Chaos parses **untrusted binary input**: a GGUF container is a file that may
have come from anywhere on the internet, and its header declares offsets, sizes,
dimensions and types that the parser must not trust. The interesting bug classes
are therefore:

- Memory-safety issues reachable by parsing a malformed or hostile `.gguf`
  (out-of-bounds reads, integer overflow in offset or size arithmetic,
  allocation of an attacker-chosen size).
- Path traversal or unintended file access through shard discovery, which
  derives sibling file names from the path you pass it.
- Any way to make Chaos write outside paths the user explicitly named. Chaos
  opens model files **read-only** and should never write to them.

`crates/chaos-ggml` contains `unsafe` FFI by necessity, and
`crates/chaos-io`'s aligned buffers use raw allocation. Both are the places to
look.

## What is out of scope

- **Model behaviour.** What a model generates is not a Chaos vulnerability.
  Chaos runs weights; it does not endorse their output.
- **Resource exhaustion from a model you chose to run.** Asking Chaos to load a
  144 GB model and running out of memory is the documented behaviour, not a
  denial of service. Chaos reports what will not fit rather than failing
  silently, and that is the intended handling.
- **ggml itself.** Report those upstream to
  [ggml-org/ggml](https://github.com/ggml-org/ggml). If a ggml issue is reachable
  specifically through how *Chaos* calls it, that is in scope here.
- Anything requiring an attacker to already have local code execution as the
  user running Chaos.

## No telemetry, and what the node does expose

**Chaos sends nothing anywhere.** There is no telemetry, no analytics, no crash
reporting and no update ping beyond the one request `chaos-app` makes to a static
JSON file on GitHub Releases to see whether a newer version exists — which
`CHAOS_NO_UPDATE_CHECK` turns off. Model downloads go to the host the user names.
**The dependency list is the whole of the supply chain: `Cargo.lock` holds 22
packages and every one of them is a `chaos-*` crate in this repository**, plus a
statically linked ggml. There is nothing else to audit.

**What a node does expose, measured.** With `--api-key` set, the rule is the
**peer**, not the bind address: this machine is always allowed, and the network
needs the key.

From the network (measured from `192.168.1.105` against a node bound to that
address, with a key set):

```
GET /status      401      GET /qr    200   the mark, so a phone can scan it
GET /health      401      GET /scan  200   the reader
GET /v1/models   401      GET /      200   the browser page
GET /status  with the key 200
```

**`/status` and `/health` name the model, so they are behind the key too.** That
changed in v0.0.23: before it, anyone who could reach the port learned which model
you were running, its context size and the node's route without any key at all.

**The machine itself is never gated**, whatever the key. The window probes
`/health` on `127.0.0.1` to learn whether the server it just started is answering,
and `chaos status` reads `/status` the same way; gating by the bind address would
have broken both. `chaos status` sends the key as well, so it works against a
remote node.

**The mark and the reader stay open deliberately.** A stranger's phone has no key,
and pointing its camera at `/qr` is the entire point of them. `/qr` encodes the
route of a node whose address the caller already had in order to ask.

With **no** key configured nothing is gated at all — which is safe only because
the server binds `127.0.0.1` unless told otherwise. Choosing CORE is what opens
the route, and CORE generates a key rather than asking for one.

## Known and deliberate

Chaos binds weights **zero-copy**: `ggml` tensors point directly into buffers
Chaos owns, and the safety of that arrangement rests on `WeightSet` outliving
every tensor that points into it. This is enforced by the borrow checker and
documented in `core/ggml/src/weights.rs`. If you find a way to defeat
it from safe code, that is a genuine finding and I want to hear about it.
