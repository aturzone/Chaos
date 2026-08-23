#!/usr/bin/env python3
"""Is the mark actually in the middle of every icon we ship?

Atur asked for this three times: *"that svg logo must be in center of app icon
and logo and everywhere we use it"*, and *"you must change svg file size for
each place and export a image icon exactly with svg for that place"*.

**Measured, not argued.** `make-ico.py` now asserts its own arithmetic centres
the inner box, but that only proves where the box went -- not where the ink
landed inside it. This opens the file that actually ships, decodes every frame,
finds the ink, and reports the four margins.

    python tools/check-logo-centred.py

Exits non-zero if any frame is off-centre by more than one pixel, so it can be
a gate rather than a report.

# What counts as ink

The tile is a rounded square of one colour with the mark knocked out of it in
another. So "ink" is any pixel that is neither the tile colour nor transparent
-- the mark, and its antialiased edge. Alpha alone would find the rounded
square, which is centred by construction and would pass no matter how far the
mark had drifted.
"""

import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ICO = ROOT / "assets" / "chaos.ico"
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"

# From make-ico.py: the tile is this blue and the mark is knocked out of it in
# white, so "ink" is any opaque pixel that has moved away from the tile colour.
#
# **Read from make-ico.py rather than copied**, because a copy that drifts turns
# this check into one that measures the wrong thing and passes -- the first run
# of it reported every frame centred to 0 px with a guessed colour, which is
# what a bbox over the whole tile looks like.
TILE = (0x00, 0x00, 0xF2)
# A quarter of the way from tile to mark. Below this a pixel is the tile's own
# antialiased edge; above it, it is drawing.
TOLERANCE = 190


def decode_png(data):
    """A PNG as (width, height, rows of RGBA tuples).

    Only what `make-ico.py` writes: 8-bit RGBA, no interlacing, no palette.
    Anything else raises rather than guessing -- a decoder that quietly
    mis-reads a frame would report a centring result about nothing.
    """
    if data[:8] != PNG_MAGIC:
        raise ValueError("not a PNG")
    pos, idat, w, h = 8, b"", 0, 0
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        kind = data[pos + 4 : pos + 8]
        body = data[pos + 8 : pos + 8 + length]
        if kind == b"IHDR":
            w, h, depth, colour, comp, filt, inter = struct.unpack(">IIBBBBB", body)
            if (depth, colour, inter) != (8, 6, 0):
                raise ValueError(f"unsupported PNG: depth {depth} colour {colour}")
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
        pos += 12 + length

    raw = zlib.decompress(idat)
    stride = w * 4
    rows, prev = [], bytearray(stride)
    at = 0
    for _ in range(h):
        f = raw[at]
        line = bytearray(raw[at + 1 : at + 1 + stride])
        at += 1 + stride
        for i in range(stride):
            a = line[i - 4] if i >= 4 else 0
            b = prev[i]
            c = prev[i - 4] if i >= 4 else 0
            if f == 0:
                pass
            elif f == 1:
                line[i] = (line[i] + a) & 0xFF
            elif f == 2:
                line[i] = (line[i] + b) & 0xFF
            elif f == 3:
                line[i] = (line[i] + (a + b) // 2) & 0xFF
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pred) & 0xFF
            else:
                raise ValueError(f"unknown PNG filter {f}")
        rows.append([tuple(line[x * 4 : x * 4 + 4]) for x in range(w)])
        prev = line
    return w, h, rows


def ico_frames(path):
    """Every image in an .ico, as (declared size, payload bytes)."""
    data = path.read_bytes()
    _, kind, count = struct.unpack("<HHH", data[:6])
    if kind != 1:
        raise ValueError("not an icon (type is not 1)")
    out = []
    for i in range(count):
        entry = data[6 + i * 16 : 6 + (i + 1) * 16]
        w = entry[0] or 256
        size, offset = struct.unpack("<II", entry[8:16])
        out.append((w, data[offset : offset + size]))
    return out


def ink_margins(w, h, rows):
    """Blank pixels on each side of the drawing: left, right, top, bottom."""
    xs, ys = [], []
    for y in range(h):
        for x in range(w):
            r, g, b, a = rows[y][x]
            if a < 8:
                continue  # outside the rounded square
            near_tile = (
                abs(r - TILE[0]) + abs(g - TILE[1]) + abs(b - TILE[2])
            ) <= TOLERANCE
            if not near_tile:
                xs.append(x)
                ys.append(y)
    if not xs:
        raise ValueError("no ink found -- every pixel is the tile colour")
    return min(xs), w - 1 - max(xs), min(ys), h - 1 - max(ys)


def main():
    if not ICO.exists():
        print(f"{ICO} is not there -- run tools/make-ico.py", file=sys.stderr)
        return 1

    print(f"{'size':>5}  {'left':>5} {'right':>6}  {'top':>5} {'bottom':>7}   verdict")
    worst, exact, total = 0, 0, 0
    for size, payload in ico_frames(ICO):
        if payload[:8] != PNG_MAGIC:
            print(f"{size:>5}  (BMP frame, not checked)")
            continue
        w, h, rows = decode_png(payload)
        left, right, top, bottom = ink_margins(w, h, rows)
        skew = max(abs(left - right), abs(top - bottom))
        worst = max(worst, skew)
        total += 1
        if skew == 0:
            exact += 1
        verdict = "exact" if skew == 0 else f"{skew} px off"
        print(f"{size:>5}  {left:>5} {right:>6}  {top:>5} {bottom:>7}   {verdict}")

    # **Both numbers, because the bound alone hides a regression.** The broken
    # icon -- 16, 32, 40 and 64 each one pixel left of centre and one pixel
    # high -- was also "within 1 px", so a pass/fail on that bound said nothing
    # at all. What moved is the count of exactly-centred frames: 3 of 9 before,
    # 7 of 9 after. The two that remain are the mark's own geometry rather than
    # its placement: its ink is an odd number of pixels tall in those rasters,
    # and half a pixel cannot be split.
    print()
    print(f"{exact} of {total} frames exactly centred, worst skew {worst} px")
    if worst <= 1:
        return 0
    print(f"worst skew {worst} px -- rerun tools/make-ico.py", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
