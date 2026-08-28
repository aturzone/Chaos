---
topic: how the reader gets a secure context, decided rather than left open
status: resolved — decided 2026-08-28, and the decision is to accept the limit
links:
  - ../backlog/v0-0-3-the-complete-version.md
  - desktop-app-broken-2026-08-28.md
---

# The camera needs a secure context, and a LAN node cannot give it one

**The question, from §1 of the plan:** a browser will not open a camera except in
a secure context. `https`, or `localhost`, or nothing. The Android app already
solves it — a WebView over a fictitious `https://chaos.invalid` origin the app
answers itself in `shouldInterceptRequest`, which *is* a secure context. The
desktop and a plain browser have no such trick: open
`http://192.168.1.20:8080/scan` on a second machine and the camera will not
open.

The plan lists three ways out and asks for a decision rather than a survey.

## Decided: accept it, and say so where it is felt

**The mark is universal; the reader is a phone feature.** Nothing changes in the
code, and that is the point — the page already fails with a message that names
the reason, and that message stays.

Why this rather than the alternatives:

- **A self-signed certificate and a trust prompt per device** would put a
  full-page security warning in front of the one feature most likely to be
  someone's first contact with Chaos, and teach them to click through TLS
  warnings. It also needs a certificate to exist, be stored, and be rotated — in
  a project whose defining property is that it has no dependencies and downloads
  nothing. The warning is not a cosmetic cost: a stranger being told a Chaos node
  is unsafe is worse than a stranger being told the camera needs a phone.
- **`localhost` only** — the one non-TLS secure origin — makes the reader work on
  the machine that is already running the node, which is the machine least likely
  to need it. A person sitting at the node has a keyboard; the reader exists for
  the person holding a phone.
- **Accepting it costs almost nothing that is actually wanted.** What strangers
  scan is the *mark*, and the mark needs no camera: it renders in any browser, on
  the desktop, on the phone and in a bare terminal. The reader is wanted on a
  device with a camera, and every device with a camera that matters here is
  running the Android app, which already has its secure context.

**Where the limit is now stated, so it is not rediscovered:**

- the page itself, which fails with the reason rather than silently — unchanged,
  and the reason this decision costs no code;
- `chaos scan`, whose refusal names it: *"a camera needs a secure context, so
  this works on localhost and on the phone, not over a LAN address"*;
- `chaos_http`'s module documentation, which refuses `https://` loudly and points
  here for why plain HTTP is the whole of it.

## What would reopen this

**A real iOS need.** The Android answer does not port: it depends on
`shouldInterceptRequest`, and iOS has no equivalent for a `WKWebView` serving a
fictitious origin. If iOS matters, the reader on iOS is a genuinely open problem
and this decision does not cover it — which is why §2 of the plan says to ask
before spending anything there. **Still unanswered by Atur as of 2026-08-28.**

**A node that is already behind TLS.** Someone putting a reverse proxy in front
of a Chaos node gets a secure context for free, and the reader will simply work.
That costs nothing to support and is not planned for; it falls out.

## What was measured, not assumed

- `BarcodeDetector` is **absent on desktop Windows** (measured) and on iOS, which
  is why a detector was written in JavaScript at all rather than delegated.
- The bundled reader is measured at **210 of 210 across 7 scales and 30 angles**,
  with 0 false positives on 40 noise frames — so the thing being gated by the
  secure context works, and this decision is about reach rather than quality.
- **No real camera has yet seen either page.** That is still open, still needs a
  phone, and is unaffected by this decision.
