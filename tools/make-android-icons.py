#!/usr/bin/env python3
"""The launcher icon for the Android app, from `assets/logo.svg`.

Atur's rule, stated three times and applied here too: *"you must change svg
file size for each place and export a image icon exactly with svg for that
place"*. Android asks for five densities and an adaptive-icon foreground, so
that is six renders from the vector -- **not one render scaled six ways**,
which is what makes a mark look soft at 48 px and blocky at 192.

    python tools/make-android-icons.py

Writes `android/app/src/main/res/mipmap-*/ic_launcher.png` and the adaptive
icon's foreground layer.

# The two shapes Android wants

* **`ic_launcher.png`** -- the whole icon, tile and all, for Android 7 and
  below and as the fallback everywhere. The same rounded blue tile the Windows
  `.ico` uses, so the app is recognisably the same product on both.
* **`ic_launcher_foreground.png`** -- for Android 8 and up, which composes a
  launcher-chosen shape from a background layer and a foreground layer. The
  foreground is transparent except for the mark, and **the mark must sit inside
  the middle 66%**: the outer third is a safe zone the launcher may mask away
  entirely, and a mark drawn into it comes out clipped on exactly the devices
  whose launcher crops hardest.

Both are centred by ink, the same rule `make-ico.py` follows -- an SVG's canvas
and its drawing are different rectangles, and this file's first path is a
full-canvas white background.
"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RES = ROOT / "android" / "app" / "src" / "main" / "res"

# Android's density buckets, and the launcher-icon size each one wants.
DENSITIES = {
    "mdpi": 48,
    "hdpi": 72,
    "xhdpi": 96,
    "xxhdpi": 144,
    "xxxhdpi": 192,
}

# An adaptive icon's layers are 108dp; the visible circle is the middle 72dp
# and the guaranteed-safe area is the middle 66dp. The mark is drawn to 60% so
# it survives the most aggressive launcher mask with room to spare.
ADAPTIVE = {k: round(v * 108 / 48) for k, v in DENSITIES.items()}
ADAPTIVE_INK = 0.60

# The same tile and mark as the Windows icon, from make-ico.py. One product,
# one mark, one colour.
TILE = (0x00, 0x00, 0xF2)
MARK = (255, 255, 255)
INK_INSET = 0.04
RADIUS_FRAC = 0.22


def load_tools():
    """`make-ico.py`'s helpers, rather than a second copy of them.

    Both scripts rasterise the same vector with the same ink box; two
    implementations of that is two places for a centring fix to be missing
    from, which is the bug this whole exercise started as.
    """
    import importlib.util

    ns = {}
    for name in ("rasterise-logo.py", "make-ico.py"):
        path = ROOT / "tools" / name
        spec = importlib.util.spec_from_file_location(name.replace("-", "_")[:-3], path)
        mod = importlib.util.module_from_spec(spec)
        # make-ico.py runs its own main() only under __main__, so importing is
        # safe and gives us its png_bytes and rounded_mask.
        spec.loader.exec_module(mod)
        for k in dir(mod):
            if not k.startswith("__"):
                ns[k] = getattr(mod, k)
    missing = [n for n in ("parse_paths", "rasterise", "ink_box", "SVG", "png_bytes") if n not in ns]
    if missing:
        raise SystemExit(f"tools/ did not provide: {', '.join(missing)}")
    return ns


def main():
    ns = load_tools()
    paths = ns["parse_paths"](ns["SVG"].read_text(encoding="utf-8"))
    bx, by, side = ns["ink_box"](paths)
    rasterise = ns["rasterise"]
    png_bytes = ns["png_bytes"]

    def ink(px, inset):
        """Ink coverage over a `px` tile, the mark inset by `inset` each side."""
        inner = max(1, int(round(px * (1 - 2 * inset))))
        # Same parity as the tile, so the margin splits evenly -- the exact bug
        # that put four of the nine Windows icon sizes a pixel off centre.
        if (px - inner) % 2:
            inner = max(1, inner - 1)
        off = (px - inner) // 2
        assert px - inner - off == off, f"{px}px: not centred"
        art = rasterise(
            paths, inner, inner, inner * 8, inner * 8,
            inner * 8 / side, inner * 8 / side, ss=8, origin=(bx, by),
        )
        out = [[0.0] * px for _ in range(px)]
        for y in range(inner):
            for x in range(inner):
                r, g, b = art[y][x]
                # The source is black on white; the mark is knocked out in
                # white, so it inverts here.
                out[y + off][x + off] = 1.0 - (r * 299 + g * 587 + b * 114) / 1000.0 / 255.0
        return out

    def rounded(px):
        r = RADIUS_FRAC * px
        mask = []
        for y in range(px):
            row = []
            for x in range(px):
                hits = 0
                for sy in range(4):
                    for sx in range(4):
                        fx, fy = x + (sx + 0.5) / 4, y + (sy + 0.5) / 4
                        cx = min(max(fx, r), px - r)
                        cy = min(max(fy, r), px - r)
                        if (fx - cx) ** 2 + (fy - cy) ** 2 <= r * r:
                            hits += 1
                row.append(hits / 16)
            mask.append(row)
        return mask

    wrote = 0
    for bucket, px in DENSITIES.items():
        cov = ink(px, INK_INSET)
        mask = rounded(px)
        grid = [
            [
                tuple(int(TILE[c] + (MARK[c] - TILE[c]) * cov[y][x]) for c in range(3))
                + (int(round(mask[y][x] * 255)),)
                for x in range(px)
            ]
            for y in range(px)
        ]
        out = RES / f"mipmap-{bucket}" / "ic_launcher.png"
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_bytes(png_bytes(grid))
        wrote += 1

        # The adaptive foreground: the mark alone, transparent around it, drawn
        # small enough to survive the launcher's mask.
        apx = ADAPTIVE[bucket]
        fcov = ink(apx, (1 - ADAPTIVE_INK) / 2)
        fgrid = [
            [MARK + (int(round(fcov[y][x] * 255)),) for x in range(apx)]
            for y in range(apx)
        ]
        fout = RES / f"mipmap-{bucket}" / "ic_launcher_foreground.png"
        fout.write_bytes(png_bytes(fgrid))
        wrote += 1

        # **The knob's centre badge.** The mode knob on Android draws its body
        # with Canvas, but the mark in the middle is Atur's and is never
        # redrawn -- so it is rendered from the same SVG, at each density,
        # exactly like the launcher icons above. Full bleed: the knob's own
        # collar is the border, so an inset here would only make it small.
        bpx = max(48, px * 2)
        bcov = ink(bpx, 0.02)
        bgrid = [
            [MARK + (int(round(bcov[y][x] * 255)),) for x in range(bpx)]
            for y in range(bpx)
        ]
        bout = RES / f"drawable-{bucket}" / "knob_badge.png"
        bout.parent.mkdir(parents=True, exist_ok=True)
        bout.write_bytes(png_bytes(bgrid))
        wrote += 1

    print(f"wrote {wrote} PNGs under {RES}", file=sys.stderr)
    for bucket, px in DENSITIES.items():
        print(f"  mipmap-{bucket:<8} {px:>3}px launcher, {ADAPTIVE[bucket]:>3}px foreground", file=sys.stderr)


if __name__ == "__main__":
    main()
