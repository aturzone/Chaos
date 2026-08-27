# Burning Grimoire — an interactive QR mark

`grimoire.html` is one self-contained page. A book carrying the Chaos mark burns
with blue fire inside a lit rune circle; tapping it swings the camera overhead
and the book **turns its own pages** — five sheets, each starting after the one
beneath it and bending as it goes — until the left leaf shows what this node is
and the code burns itself into the right one. Tapping again shuts it.

There is no ground. The host application composites the rite over its own
background, so all that is drawn is the book, the candles, the circle and what
they throw.

No libraries. The 3-D renderer, the flame, the bloom and the QR encoder are all
in the file, because the page is published as an artifact and that sandbox blocks
every external host except Google Fonts.

## Where the code points

Resolved while the page runs, in this order:

1. `window.CHAOS_ENDPOINT` — a string or a function. **This is the integration
   point.** The host sets it and fires `window.dispatchEvent(new Event(
   "chaos:endpoint"))`, or calls `window.__grimoire.setEndpoint(url)`.
2. `?endpoint=` or `#endpoint=` in the address.
3. `location.origin`, when the page is served by the node itself and that origin
   is not loopback. **This is the good case**: served by Chaos on
   `http://192.168.1.42:8099`, that origin *is* the route another machine uses,
   and it changes when the network does.
4. `TARGET_URL`, the fallback constant.

The page re-resolves on `online`, `offline`, `navigator.connection`'s `change`,
tab visibility, a `chaos:endpoint` event, and a four-second poll — and re-cuts
the code whenever the answer moves.

**A browser cannot read the Wi-Fi SSID.** There is no API for it, and none for
the LAN address either: host ICE candidates come back as mDNS `.local` names now,
and a STUN server is an external host the artifact sandbox will not reach. The
network cannot be sniffed from inside a page; it has to be handed in, or inferred
from where the page was served. Options 1 and 3 are those two answers.

## The two leaves

- **Right**: the code, and nothing else. No plate, no frame, no caption — the
  quiet zone a scanner needs is just clean paper, and a white square announcing
  "here is a QR code" is the one thing it does not need. It arrives by burning
  in: a front crosses the leaf, the paper chars and glows along it, and the
  modules are simply there behind it.
- **Left**: what this node actually is — model, weights, context, device,
  tokens per second, route. From `window.CHAOS_STATUS` or a same-origin
  `/status`. **Every field is an em-dash until the node reports it.** Filling
  them with plausible figures would make the leaf a picture of an instrument;
  an invented tokens-per-second is worse than a blank one.

  ```js
  window.__grimoire.setStatus({ model: "...", quant: "...", size: "...",
    context: "...", device: "...", tokensPerSecond: 0.42, promptMsPerToken: 1640 })
  ```

## Themes

Both are built. Light is the application's default: black leather on white, with
a deep saturated circle. Dark inverts the book to white vellum on near-black.
They are not a recolour of each other — on black, blue fire is ADDED to the
ground and blooms; on white there is nothing left to add to, so the same fire is
laid over the ground instead and the bloom is nearly off. That is what
`PALETTES[*].blend` selects.

**The blue is the brand's, `#0000F2`.** It is declared in
`android/app/src/main/res/values/colors.xml`, under a comment saying that palette
is "the same as the desktop window, so the two read as one product". An earlier
pass here concluded there was no brand blue and invented a sky blue — it had read
an *untracked leftover* file whose accent was orange. Check `git ls-files` before
concluding a colour does not exist.

The whole ramp sits on that one hue and varies only in value, because "the same
blue" means the same hue. Note where it is NOT used: the ambient and key light
are near-white with a blue cast. `#0000F2` is what glows — fire, circle, the mark
— and using it to *illuminate* multiplies every surface by pure blue and turns
white vellum into navy card.

The mark on the cover is the real `assets/logo.svg`, inlined and rendered at
size. The page turns its luminance into an alpha mask so the artwork can be
tinted and haloed without being redrawn.

## Preview

```bash
python -m http.server 8123 --directory assets/grimoire
```

Then open `http://localhost:8123/grimoire.html`. Append a query string when
iterating — the server sends `Last-Modified` and the browser will happily serve a
stale page, which is worth an hour of measuring code you are not running.

## Verifying the code actually scans

A QR that renders is not a QR that scans, and the failure is silent. The encoder
here is checked two ways, and the second one found a real bug:

```bash
python decode_qr.py grid.txt          # independent decoder, from the read side
```

`decode_qr.py` sits next to this file. It locates the format word, unmasks, de-interleaves and computes the
Reed-Solomon **syndromes**. For a correct codeword every syndrome is zero — a
property no misunderstanding shared with the encoder can fake.

Then compare the grid bit-for-bit against an implementation this project did not
write:

```python
q = qrcode.QRCode(version=4, error_correction=ERROR_CORRECT_Q, box_size=1, border=0)
q.add_data("https://github.com/aturzone/Chaos"); q.make(fit=False)
```

Current state for the default target: **identical to `python-qrcode`, auto-chosen
mask included**, and all 52 syndromes zero.

Two implementations differ from this one without either being wrong, and both
differences are worth knowing before chasing them:

- `segno` pads differently — an extra `0x00` before the `EC 11` run. No decoder
  reads padding.
- `python-qrcode` **splits a URL into mixed-mode segments** (byte `http`, then
  alphanumeric for `://192.168.1.42:8099`, which is entirely in the alphanumeric
  charset). This encoder uses one byte segment throughout: less compact, equally
  valid. `decode_qr.py` reads only the first segment, so decoding *their* grid
  reports four bytes, `http`. That is the decoder's limit, not their bug.

To dump the page's own grid: `window.__grimoire.qr()`.

## Traps, each one paid for

- **The format strip does not own the whole of row 8 and column 8.** It crosses
  the timing patterns and skips where they meet, at (6,8) and (8,6). Reserving
  those two cleared them to light, so every timing line started on the wrong
  colour — two modules out of 1089, invisible to the eye, and exactly the thing
  a scanner uses to find the grid. Only the bit-for-bit diff caught it.
- **A hidden tab never fires `requestAnimationFrame`**, so the canvas stays blank
  and looks precisely like a broken renderer. `window.__grimoire.run(n, ms)`
  draws frames on demand.
- **Measure the frame rate after the particle count settles.** Timing the first
  30 frames of a system whose population takes 40 to fill reported 20 ms for
  something that cost 57.
- **Additive flame is the sum, not the sprites.** Anything bright enough to pick
  out individually reads as a bead; no single sprite should be white. Set
  `window.__prof = {}` for per-stage milliseconds.
- **Bloom is a post pass**, so light behind an opaque surface blooms over its
  front — the sigil printed its star across the open pages, over the code. Each
  opaque surface punches itself out of the glow buffer as it is drawn.
- **A centroid sort cannot express "flat on the stone".** The sigil has one depth
  and the slab under it spans many, so half the circle vanished under the tiles
  in front of it. Hence `LAYER`.
- **Face windings fail silently.** A box wound inside-out culls the face you
  wanted and keeps the one behind it; the cover, logo and all, was culled every
  frame. Cull against the world-space normal, not the screen winding.
- **Rotating a finished spread image cannot fit a rolled camera.** Turning the
  artwork turns its gutter edge with it, and the two leaves then join along the
  wrong edges — each showing a quarter-turned copy of the wrong half. The design
  is composed into a canvas whose long axis is always the across-gutter one, and
  the two orientations differ only in which canvas axis that is.
- **Abutting coplanar quads leave a hairline**, and that hairline ran straight
  through the middle of the code. The leaves overlap by `GUTTER_LAP` instead.
- **A rim light on a face seen edge-on draws a bright line.** The two text
  blocks' inner faces meet at the gutter; giving them a sheen drew a highlight
  across the QR. They reach the spine now and take `rim = 0`.
- **A degenerate quad's normal is a division by zero, and a NaN colour does not
  throw** — `fillStyle` simply keeps its previous value and paints something
  arbitrary. The spine is skipped below a hair of angle.
- **Additive rings bead.** 144 segments with round caps means every joint is two
  caps stacked; one path and one stroke instead.
- **Composing beats rotating, twice over.** A leaf shown by a rolled camera
  appears landscape, so its content must be laid out landscape — and rotating
  the finished image cannot fix it, because a sheet is drawn as strips that
  slice the texture and the rolled mapping reassigns which axis they slice.
- **A run of strips double-darkens its own seams.** The shading pass is a
  `multiply` clipped to each strip; where two antialiased clip edges meet it
  lands twice and rules a dark line down the page. Push the overlay path out
  from its centroid, as the texture triangles already were.
- **`fonts.ready` can resolve having loaded nothing.** Faces are declared lazily
  and nothing in the DOM is set in Cinzel — it exists only inside canvas calls,
  which do not count. Load each face by name before baking a texture, because a
  texture is drawn once and never redrawn.
