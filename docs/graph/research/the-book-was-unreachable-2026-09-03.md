---
topic: The mark and the reader were unreachable in three different ways — the window could only show them through a running engine, the reader was handed a LAN address where no camera will open, and on Android the entrance was the bottom of a scroll view — and none of it was in the pages, which is why every test of them passed
status: FIXED in v0.0.32. The window serves the pages itself on loopback, `grimoire` is its own ggml-free crate, and the mode badge is the door on the phone.
links:
  - ../decisions/browser-ui-is-tracked-not-built.md
  - android-app.md
  - ../reference/hard-won-facts.md
---

# The book was unreachable, and the pages were fine

**Atur, testing v0.0.31 on Windows and on a phone**: *"the book of QR code for
Core mode is not available!!! that book where is it!!"*

The pages were not missing. Every test of them passed, and kept passing while
this was broken, because **not one of the three causes was in the pages**. Each
was in how a tier reached them.

## 1. The window could only show the art while a model was loaded

`open_brand_page` did the only thing it could: `ShellExecute` on
`http://<this node>/qr`, a route on the child `chaos-serve`. So the brand pages
were **a feature of a loaded model**. Open the app, turn the dial to CORE, press
the button that shows a book before pressing LOAD — no server, and the browser
reports that the site cannot be reached.

Nothing about a QR code needs 7 GiB of weights. The art was gated on inference
by accident of where it happened to be served from.

## 2. The reader could not open a camera *even with a model loaded*

This one is worse, because it looks like it should work and does not.

`getUserMedia` is refused outside [a secure context]. `https://` counts, and
**loopback** counts; a LAN address does not. In CORE mode the window put its own
LAN address in the box — correct, that is what another device must type — and
then handed that same address to the reader. `http://192.168.1.20:8080/scan` is
a page a browser will happily load and never give a camera to.

The page already handled it honestly: `window.isSecureContext` is checked and it
prints *"A camera only opens on a secure origin… Serve it over https, or from
localhost."* So the symptom was a page that loaded, drew its circle, and then
explained that it could not do the one thing it exists for.

[a secure context]: https://developer.mozilla.org/docs/Web/Security/Secure_Contexts

**The fix separates two addresses that had been one.** The page is *served* from
`127.0.0.1`, which is a secure context, and the endpoint the mark *encodes* is
passed in separately. That works because `resolveEndpoint` prefers an injected
`window.CHAOS_ENDPOINT` over the origin it was served from — and refuses to infer
a loopback endpoint at all, on the stated grounds that it is *"useless to the one
person who matters, the one holding another device"*. The precedence was already
right for exactly this case; nothing had used it.

## 3. On Android it was present and unfindable

The buttons existed: `BrandActivity` is real, the APK carries both pages as
assets, and `BrandTest.kt` tests the routing. They were at the **bottom of the
SETTINGS tab, inside a `ScrollView`, behind a button labelled `MARK`** — the last
tab, below the address and key fields, under a label that does not say "book".

Present and unfindable are the same thing to the person holding the phone. The
**mode badge is now the door**, which is how the desktop already works: there the
badge at the foot of the rail is the CHAOS page's only entrance.

## What made the fix possible, and why it had not been done

The window deliberately depends on no engine crate — `chaos-app` links
`chaos-model`, `chaos-config`, `chaos-probe`, `chaos-gguf` and nothing that needs
a C toolchain. `grimoire` lived in **`chaos-arch`**, the one crate in the
workspace that cannot build without a compiled ggml. So the window could not
assemble a page even though the page is a string, and its only route to the art
was a running server. That is the whole of cause 1.

`CLAUDE.md` had already written down the answer and nobody had taken it:

> *"`chaos_arch::grimoire` has zero ggml references, so moving it to its own
> crate would let the APK step emit the brand pages with no C toolchain and
> delete the host-ggml build that step now needs."*

It is now **`chaos-grimoire`**: its own crate, **zero dependencies**, no ggml.
`chaos_arch::grimoire` is a `pub use` of it, so the server and `--emit-pages`
did not change. Two things fell out:

- **The window serves the pages itself.** `gui/app/src/brand.rs` binds
  `127.0.0.1:0` and answers two fixed paths from two strings in memory. Not a web
  server: loopback only, no file access, and no path parsing to get wrong because
  there are no paths — a request is one of two constants or a 404.
- **The Android release stopped compiling a host llama.cpp.** The emitter was on
  `chaos-serve`, which links ggml, so writing two HTML files meant a second full
  cmake of llama.cpp on every release — a step that **failed the first time it
  ever ran**, on v0.0.22, because each `run:` is its own shell and `GGML_LIB_DIR`
  was not set in it. `chaos-qr --emit-pages` now does it with no C toolchain.

## What to take from this

**A feature can be complete, tested, shipped on every platform, and still
unreachable.** All three causes sat in the last inch — which process serves the
bytes, which of two addresses gets handed to a camera, which screen the entrance
is on. The pages had tests asserting they fetch nothing external, carry their
licence, embed their fonts and push the route in, and every one of those was
green throughout.

`scripts/smoke-the-surface.sh` exists because of the same class of defect one
release earlier (`chaos connect` reading `--port` as a hostname). It now also
proves `chaos-qr --emit-pages` writes two self-contained pages, because the APK
depends on that and nothing else checked it outside CI. **What it still cannot
check is a window and a phone screen**: the desktop half is covered by four tests
over a real socket, and the Android half is one badge nobody here can tap.
