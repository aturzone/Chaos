---
topic: A browser chat UI stays a tracked bar and is not worked on — Chaos is reached by the CLI, by chaos connect, or by a native app per platform, so a browser is a fourth client rather than a missing one
status: decided 2026-09-02 by Atur
links:
  - ../backlog/cli-first-class-tier.md
  - ../backlog/app-to-production.md
---

# The browser UI is a bar, not a task

## Context

`README.md` carries a coverage bar reading **Browser UI 33% — 2 of 6 things a
chat UI needs**, which invites the reading that four things are missing and
someone should go and build them.

## The decision

**Keep the bar; do not work on it now.** Atur, 2026-09-02, in his own framing:
Chaos is a *server* for a model, the CLI is how a server is driven, and the GUI
tier is already several native applications. A browser client would be a fourth
way in, not a gap in the first three.

So the four unbuilt items stay counted — a bar that quietly drops what it cannot
do is the kind of accounting this repository has spent its time removing — and
nothing is scheduled against them.

## What already reaches a node without a browser

- `chaos-run` on the machine itself.
- `chaos serve` plus `chaos connect`, which streams a completion over HTTP/1.1
  with no curl and no TLS, from another machine on a network you control.
- `/v1/chat/completions`, `/v1/completions` and `/v1/models`, so any existing
  OpenAI-shaped client already works — which is the strongest argument here: the
  browser UI most people want is one they already have.
- The desktop window and the Android app, per platform.
- `/qr` and `/scan`, served by the node, for a headless machine to print its own
  route.

## What this does not say

It does not say a browser UI is worthless. It says the next hour is better spent
on the parity gate and the release than on a fifth client, and that the bar is
the honest place to keep that debt visible until someone chooses otherwise.
