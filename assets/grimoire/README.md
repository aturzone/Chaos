# Burning Grimoire — an interactive QR mark

`grimoire.html` is one self-contained page. A black leather book carrying the
Chaos mark burns with blue fire inside a lit ritual circle; tapping it swings the
camera overhead, opens the cover, blazes the circle and writes a QR code onto the
leaf inside. Tapping again shuts it.

No libraries. The 3-D renderer, the flame, the bloom and the QR encoder are all
in the file, because the page is published as an artifact and that sandbox blocks
every external host except Google Fonts.

## The two things worth changing

Both are constants at the top of the script:

- `TARGET_URL` — where the code points. Anything up to 74 bytes fits.
- `C` — the whole palette. The piece is one hue ramp; retint it here.

**The blue is a choice, not the brand.** This repository's accent is orange
(`#ff7a33` / `#d1500f`, in `crates/chaos-arch/src/ui.rs`) and `assets/logo.svg`
is black-and-white line art. There is no brand blue to match, so one was picked.

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

`decode_qr.py` (kept with the session scratch, reproduce it from this note if
needed) locates the format word, unmasks, de-interleaves and computes the
Reed-Solomon **syndromes**. For a correct codeword every syndrome is zero — a
property no misunderstanding shared with the encoder can fake.

Then compare the grid bit-for-bit against an implementation this project did not
write:

```python
q = qrcode.QRCode(version=4, error_correction=ERROR_CORRECT_Q, box_size=1, border=0)
q.add_data("https://github.com/aturzone/Chaos"); q.make(fit=False)
```

Current state: **identical to `python-qrcode`, auto-chosen mask included**, and
all 52 syndromes zero. `segno` differs, but only in its padding codewords, which
no decoder reads.

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
- **`fonts.ready` can resolve having loaded nothing.** Faces are declared lazily
  and nothing in the DOM is set in Cinzel — it exists only inside canvas calls,
  which do not count. Load each face by name before baking a texture, because a
  texture is drawn once and never redrawn.
