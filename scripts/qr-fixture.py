"""Cut the reference grids that `core/qr` is tested against.

**What makes them a reference.** Each grid is produced by the QR encoder inside
`assets/grimoire/grimoire.html` -- the one that actually ships in the mark --
run under node, and then put through two checks that do not share its code:

1. `assets/grimoire/decode_qr.py`, written from the *reading* side of the
   specification, must recover the exact payload. Its Reed-Solomon syndrome
   check is the part that cannot lie: for a correct codeword every syndrome is
   zero, and no misunderstanding shared with the encoder can fake that.
2. `python-qrcode` must produce the same grid, or differ only in the mask it
   chose -- in which case both are scored with a penalty function written here
   from the rules, and the grid we keep must not be the worse one.

Check 2 is not a formality. On three of the nine payloads python-qrcode picks a
different mask, and by an independent scoring of ISO 18004's four rules the
page's choice is the better one every time (311 against 416 on "hi"). Mask
selection is a quality heuristic, not correctness -- both codes decode -- but a
diff that is reported as a failure and then waved away is how a real difference
gets waved away later.

Needs `node` on PATH and a Python with `qrcode` installed:

    python -m venv .qrvenv && .qrvenv/Scripts/pip install qrcode
    python scripts/qr-fixture.py --python .qrvenv/Scripts/python.exe
"""

import argparse
import base64
import os
import json
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent

# One payload for each version 1-6, the shapes that actually ship, and one that
# is not ASCII -- byte mode is defined over bytes, and a count taken in
# characters is a bug that only appears outside Latin-1.
PAYLOADS = [
    "hi",
    "http://10.0.0.2:8080",
    "http://192.168.1.20:8080",
    "http://a-fairly-long-hostname.local:8080/path",
    "https://github.com/aturzone/Chaos",
    "http://192.168.100.100:65535/qr?theme=dark",
    "x" * 60,
    "x" * 74,
    "\u00e9\u00e8\u00ea",
]


def penalty(rows):
    """ISO 18004's four rules, written here from the rules themselves."""
    n = len(rows)
    g = [[int(c) for c in r] for r in rows]
    s = 0
    lines = [list(r) for r in g] + [list(c) for c in zip(*g)]
    for line in lines:                                    # 1: runs of five
        run = 1
        for i in range(1, n):
            if line[i] == line[i - 1]:
                run += 1
            else:
                if run >= 5:
                    s += 3 + (run - 5)
                run = 1
        if run >= 5:
            s += 3 + (run - 5)
    for y in range(n - 1):                                # 2: 2x2 blocks
        for x in range(n - 1):
            v = g[y][x]
            if v == g[y][x + 1] == g[y + 1][x] == g[y + 1][x + 1]:
                s += 3
    a = [1, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0]                 # 3: finder lookalike
    b = [0, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1]
    for line in lines:
        for i in range(n - 10):
            if line[i:i + 11] == a:
                s += 40
            if line[i:i + 11] == b:
                s += 40
    dark = sum(map(sum, g))                               # 4: drift from half
    return s + int(abs(dark * 100 / (n * n) - 50) // 5) * 10


def mask_of(rows):
    """The mask index, read back out of the format word."""
    n = len(rows)
    word = 0
    for i in range(8):
        word |= int(rows[8][n - 1 - i]) << i
    for i in range(8, 15):
        word |= int(rows[n - 15 + i][8]) << i
    return ((word ^ 0x5412) >> 10) & 7


def browser_grids(scratch):
    """The page's own encoder, extracted verbatim and run under node."""
    html = (ROOT / "assets" / "grimoire" / "grimoire.html").read_text(encoding="utf-8")
    lines = html.split("\n")
    start = next(i for i, l in enumerate(lines) if l.startswith("const QR = (function ()"))
    end = next(i for i, l in enumerate(lines) if l.strip() == "return { encode };")
    js = "\n".join(lines[start:end + 2])
    if not js.rstrip().endswith("})();"):
        sys.exit("the encoder did not extract cleanly; grimoire.html has moved")
    driver = js + """
const out = {};
for (const p of %s) {
  const c = QR.encode(p, "Q");
  out[p] = { version: c.version, rows: Array.from(c.modules, r => Array.from(r).join("")) };
}
console.log(JSON.stringify(out));
""" % json.dumps(PAYLOADS)
    path = scratch / "encoder.js"
    path.write_text(driver, encoding="utf-8")
    r = subprocess.run(["node", str(path)], capture_output=True)
    if r.returncode != 0:
        sys.exit("node failed:\n" + r.stderr.decode("utf-8", "replace"))
    return json.loads(r.stdout.decode("utf-8"))


def reference_grids(python, scratch):
    """python-qrcode, with mixed-mode segmentation off.

    Left on it splits a URL into byte and alphanumeric runs -- a smaller and
    perfectly valid code, and not the one under test.
    """
    script = scratch / "reference.py"
    script.write_text(
        "import json, sys, qrcode\n"
        "from qrcode.constants import ERROR_CORRECT_Q\n"
        "out = {}\n"
        "for p in json.load(sys.stdin):\n"
        "    q = qrcode.QRCode(error_correction=ERROR_CORRECT_Q, border=0)\n"
        "    q.add_data(p.encode('utf-8'), optimize=0)\n"
        "    q.make(fit=True)\n"
        "    m = q.get_matrix()\n"
        "    out[p] = {'version': q.version,\n"
        "              'rows': [''.join('1' if v else '0' for v in row) for row in m]}\n"
        "print(json.dumps(out))\n",
        encoding="utf-8",
    )
    r = subprocess.run([python, str(script)],
                       input=json.dumps(PAYLOADS).encode(), capture_output=True)
    if r.returncode != 0:
        sys.exit("python-qrcode failed:\n" + r.stderr.decode("utf-8", "replace"))
    return json.loads(r.stdout.decode("utf-8"))


def decodes_to(rows, payload, scratch):
    """Put the grid through the from-first-principles decoder."""
    grid = scratch / "grid.txt"
    grid.write_text("\n".join(rows) + "\n", encoding="utf-8")
    # PYTHONIOENCODING because the decoder prints the payload back and this
    # runs on Windows, where a redirected stdout defaults to cp1252: the
    # non-ASCII payload came back mojibake and looked like a decode failure.
    env = dict(os.environ, PYTHONIOENCODING="utf-8")
    r = subprocess.run([sys.executable, str(ROOT / "assets" / "grimoire" / "decode_qr.py"),
                        str(grid)], capture_output=True, env=env)
    out = r.stdout.decode("utf-8", "replace")
    if r.returncode != 0 or "VERDICT: decodable" not in out:
        return False, out.strip()
    want = "decoded %d bytes: %r" % (len(payload.encode("utf-8")), payload)
    return want in out, out.strip()


def main():
    # Same reason as above: this script prints payloads.
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    ap = argparse.ArgumentParser()
    ap.add_argument("--python", default=sys.executable,
                    help="a Python with `qrcode` installed (default: this one)")
    ap.add_argument("--out", default=str(ROOT / "core" / "qr" / "tests" / "reference-grids.txt"))
    args = ap.parse_args()

    with tempfile.TemporaryDirectory() as tmp:
        scratch = pathlib.Path(tmp)
        browser = browser_grids(scratch)
        reference = reference_grids(args.python, scratch)

        failures = 0
        notes = []
        for p in PAYLOADS:
            rows = browser[p]["rows"]
            ok, detail = decodes_to(rows, p, scratch)
            if not ok:
                print("DECODE FAILED for %r:\n%s" % (p, detail))
                failures += 1
                continue
            same = rows == reference[p]["rows"]
            if same:
                verdict = "python-qrcode identical"
            else:
                ours, theirs = penalty(rows), penalty(reference[p]["rows"])
                verdict = "mask %d vs %d, penalty %d vs %d" % (
                    mask_of(rows), mask_of(reference[p]["rows"]), ours, theirs)
                if browser[p]["version"] != reference[p]["version"]:
                    print("VERSION differs for %r -- not a mask difference" % p)
                    failures += 1
                    continue
                if ours > theirs:
                    print("WORSE MASK chosen for %r: %s" % (p, verdict))
                    failures += 1
                    continue
                notes.append("%s -> %s" % (p[:30], verdict))
            print("%-46r v%d  decodes, %s" % (p[:42], browser[p]["version"], verdict))

        if failures:
            sys.exit("\n%d payload(s) failed -- writing no fixture." % failures)

        body = [
            "# QR reference grids: byte mode, level Q, versions 1-6.",
            "#",
            "# Regenerate with `python scripts/qr-fixture.py`, which is also where",
            "# the evidence for them lives. In short: each grid comes from the",
            "# encoder inside assets/grimoire/grimoire.html, every one of them is",
            "# put back through assets/grimoire/decode_qr.py and must return its",
            "# exact payload, and python-qrcode must either produce the identical",
            "# grid or differ only by choosing a mask that scores no better.",
            "#",
            "# Format: `payload <version> <base64 of the utf-8 payload>` then",
            "# <size> lines of 0/1. Base64 so a payload with a space or a",
            "# non-ASCII byte cannot make this file ambiguous.",
            "",
        ]
        for p in PAYLOADS:
            body.append("payload %d %s" % (
                browser[p]["version"], base64.b64encode(p.encode("utf-8")).decode()))
            body.extend(browser[p]["rows"])
            body.append("")
        dest = pathlib.Path(args.out)
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text("\n".join(body), encoding="utf-8")
        print("\nwrote %s: %d grids, %d bytes" % (dest, len(PAYLOADS), dest.stat().st_size))
        for n in notes:
            print("  note: " + n)


if __name__ == "__main__":
    main()
