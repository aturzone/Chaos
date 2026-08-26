"""Decode a QR grid from first principles, and say where it breaks.

Written from the reading side of the spec rather than the writing side,
so it is not just the encoder run backwards.  The Reed-Solomon syndrome
check is the part that cannot lie: for a correct codeword every syndrome
S_i = C(a^i) is zero, and no misunderstanding shared with the encoder
can make a wrong codeword satisfy that.
"""
import sys, pathlib

# ---- GF(256) --------------------------------------------------------
EXP = [0] * 512
LOG = [0] * 256
x = 1
for i in range(255):
    EXP[i] = x
    LOG[x] = i
    x <<= 1
    if x & 0x100:
        x ^= 0x11D
for i in range(255, 512):
    EXP[i] = EXP[i - 255]

def gmul(a, b):
    if a == 0 or b == 0:
        return 0
    return EXP[LOG[a] + LOG[b]]

# ---- tables ---------------------------------------------------------
ECC = {
    1: {"L": (7,1,19,0,0),  "M": (10,1,16,0,0), "Q": (13,1,13,0,0),  "H": (17,1,9,0,0)},
    2: {"L": (10,1,34,0,0), "M": (16,1,28,0,0), "Q": (22,1,22,0,0),  "H": (28,1,16,0,0)},
    3: {"L": (15,1,55,0,0), "M": (26,1,44,0,0), "Q": (18,2,17,0,0),  "H": (22,2,13,0,0)},
    4: {"L": (20,1,80,0,0), "M": (18,2,32,0,0), "Q": (26,2,24,0,0),  "H": (16,4,9,0,0)},
    5: {"L": (26,1,108,0,0),"M": (24,2,43,0,0), "Q": (18,2,15,2,16), "H": (22,2,11,2,12)},
    6: {"L": (18,2,68,0,0), "M": (16,4,27,0,0), "Q": (24,4,19,0,0),  "H": (28,4,15,0,0)},
}
ALIGN = {1: [], 2: [6,18], 3: [6,22], 4: [6,26], 5: [6,30], 6: [6,34]}
LEVEL_FROM_BITS = {1: "L", 0: "M", 3: "Q", 2: "H"}

MASKS = [
    lambda i, j: (i + j) % 2 == 0,
    lambda i, j: i % 2 == 0,
    lambda i, j: j % 3 == 0,
    lambda i, j: (i + j) % 3 == 0,
    lambda i, j: (i // 2 + j // 3) % 2 == 0,
    lambda i, j: (i * j) % 2 + (i * j) % 3 == 0,
    lambda i, j: ((i * j) % 2 + (i * j) % 3) % 2 == 0,
    lambda i, j: ((i + j) % 2 + (i * j) % 3) % 2 == 0,
]

def format_word(level_bits, mask):
    data = (level_bits << 3) | mask
    rem = data
    for _ in range(10):
        rem = (rem << 1) ^ ((rem >> 9) * 0x537)
    return ((data << 10) | rem) ^ 0x5412

ALL_FORMATS = {}
for lb in (0, 1, 2, 3):
    for mk in range(8):
        ALL_FORMATS[format_word(lb, mk)] = (LEVEL_FROM_BITS[lb], mk)

# ---- read the grid --------------------------------------------------
rows = pathlib.Path(sys.argv[1]).read_text().strip().split("\n")
n = len(rows)
g = [[int(c) for c in r] for r in rows]
version = (n - 17) // 4
print("grid %dx%d -> version %d" % (n, n, version))

# ---- mark every function module (independently of the encoder) ------
fn = [[False] * n for _ in range(n)]
def mark(cx, cy, r):
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            y, x = cy + dy, cx + dx
            if 0 <= x < n and 0 <= y < n:
                fn[y][x] = True
for c in ((3, 3), (n - 4, 3), (3, n - 4)):
    mark(c[0], c[1], 4)                      # finder plus separator
for i in range(n):
    fn[6][i] = True
    fn[i][6] = True
ac = ALIGN[version]
for cy in ac:
    for cx in ac:
        if (cx, cy) in ((ac[0], ac[0]), (ac[0], ac[-1]), (ac[-1], ac[0])):
            continue
        mark(cx, cy, 2)
for i in range(9):
    fn[i][8] = True
    fn[8][i] = True
for i in range(8):
    fn[8][n - 1 - i] = True
    fn[n - 1 - i][8] = True

# ---- format information ---------------------------------------------
def read_format_copy1():
    bits = [0] * 15
    for i in range(6):
        bits[i] = g[i][8]
    bits[6] = g[7][8]
    bits[7] = g[8][8]
    bits[8] = g[8][7]
    for i in range(9, 15):
        bits[i] = g[8][14 - i]
    return sum(b << i for i, b in enumerate(bits))

def read_format_copy2():
    bits = [0] * 15
    for i in range(8):
        bits[i] = g[8][n - 1 - i]
    for i in range(8, 15):
        bits[i] = g[n - 15 + i][8]
    return sum(b << i for i, b in enumerate(bits))

f1, f2 = read_format_copy1(), read_format_copy2()
print("format words: copy1=0x%04X copy2=0x%04X %s"
      % (f1, f2, "(agree)" if f1 == f2 else "(DISAGREE)"))

best, bestd = None, 99
for word, (lvl, mk) in ALL_FORMATS.items():
    d = bin(word ^ f1).count("1")
    if d < bestd:
        bestd, best = d, (lvl, mk, word)
level, mask, exact = best
print("format -> level %s, mask %d (hamming distance %d from a legal word)"
      % (level, mask, bestd))
if bestd:
    print("  !! the format word is not one of the 32 legal ones")

# ---- unmask and read the payload ------------------------------------
u = [[g[y][x] ^ (1 if (not fn[y][x] and MASKS[mask](y, x)) else 0)
      for x in range(n)] for y in range(n)]

bits = []
right = n - 1
while right >= 1:
    if right == 6:
        right = 5
    for vert in range(n):
        for j in range(2):
            xx = right - j
            upward = ((right + 1) & 2) == 0
            yy = (n - 1 - vert) if upward else vert
            if not fn[yy][xx]:
                bits.append(u[yy][xx])
    right -= 2

cw = [sum(bits[i * 8 + k] << (7 - k) for k in range(8))
      for i in range(len(bits) // 8)]
print("read %d bits -> %d codewords" % (len(bits), len(cw)))

ecl, g1, d1, g2, d2 = ECC[version][level]
nblocks = g1 + g2
total_data = g1 * d1 + g2 * d2
sizes = [d1] * g1 + [d2] * g2
print("expecting %d blocks, %d data codewords, %d ec per block"
      % (nblocks, total_data, ecl))

# ---- de-interleave ---------------------------------------------------
blocks = [[] for _ in range(nblocks)]
idx = 0
longest = max(sizes)
for i in range(longest):
    for b in range(nblocks):
        if i < sizes[b]:
            blocks[b].append(cw[idx]); idx += 1
ecblocks = [[] for _ in range(nblocks)]
for i in range(ecl):
    for b in range(nblocks):
        ecblocks[b].append(cw[idx]); idx += 1

# ---- the check that cannot lie --------------------------------------
ok = True
for b in range(nblocks):
    full = blocks[b] + ecblocks[b]
    bad = []
    for s in range(ecl):
        acc = 0
        for c in full:
            acc = gmul(acc, EXP[s]) ^ c
        if acc:
            bad.append(s)
    if bad:
        ok = False
        print("block %d: %d of %d syndromes NON-ZERO -> parity is wrong"
              % (b, len(bad), ecl))
    else:
        print("block %d: all %d syndromes zero -> parity is correct" % (b, ecl))

# ---- parse the message ----------------------------------------------
data = []
for b in blocks:
    data.extend(b)
stream = []
for c in data:
    for k in range(7, -1, -1):
        stream.append((c >> k) & 1)

pos = 0
def take(k):
    global pos
    v = 0
    for _ in range(k):
        v = (v << 1) | stream[pos]; pos += 1
    return v

mode = take(4)
if mode != 4:
    print("mode indicator is %d, expected 4 (byte)" % mode)
else:
    ln = take(8)
    raw = bytes(take(8) for _ in range(ln))
    print("decoded %d bytes: %r" % (ln, raw.decode("utf-8", "replace")))

print("VERDICT:", "decodable" if ok else "NOT decodable")
sys.exit(0 if ok else 1)
