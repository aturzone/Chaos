//! A QR encoder, and how to print one in a terminal.
//!
//! **Why this exists.** The mark that `chaos-serve` puts in a browser carries
//! the node's route, so another machine can find it by pointing a camera at
//! the screen. A node running on a headless server has no screen -- and that
//! is precisely the machine most worth reaching, since the whole reason to run
//! Chaos remotely is to connect to it from somewhere else. A terminal can draw
//! a scannable QR code, so the CLI is not a lesser tier: it hands out its route
//! the same way the window does.
//!
//! **This is a second implementation of something already written.** The
//! browser page has its own encoder in JavaScript, and a second implementation
//! is normally a second place for every fix to be missing from -- a rule this
//! workspace states out loud about the forward pass. It is accepted here for
//! one reason: a terminal cannot run JavaScript, and the alternative is a node
//! that can serve the mark but not show it. The risk is paid for by testing
//! the two against each other: `tests/reference_grids.rs` asserts this
//! encoder is **bit-for-bit identical** to the page's, module by module, and
//! the page's is in turn bit-for-bit identical to `python-qrcode`, mask choice
//! included. Two implementations that agree module-for-module across every
//! version and payload length are not two chances to be wrong.
//!
//! **Scope: byte mode, versions 1-6.** Version 7 and up carry an extra 18-bit
//! version block; leaving it out removes a whole class of bug in exchange for
//! capacity nobody here needs. Version 6 at level Q holds 74 bytes, and the
//! longest thing this encodes is `http://255.255.255.255:65535`, which is 28.

/// Error correction level. Higher recovers more damage and holds less.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    /// ~7% recoverable.
    L,
    /// ~15%.
    M,
    /// ~25%. What the mark uses, and the default here: a code on a screen is
    /// photographed at an angle, in a reflection, out of focus.
    Q,
    /// ~30%.
    H,
}

impl Level {
    fn index(self) -> usize {
        match self {
            Level::L => 0,
            Level::M => 1,
            Level::Q => 2,
            Level::H => 3,
        }
    }

    /// The two-bit code that goes in the format word. Not the same order as
    /// the table index, which is the kind of thing that produces a code no
    /// scanner will touch while every module looks right.
    fn format_bits(self) -> u32 {
        match self {
            Level::L => 1,
            Level::M => 0,
            Level::Q => 3,
            Level::H => 2,
        }
    }

    /// Parse `l`/`m`/`q`/`h`, in either case.
    pub fn parse(s: &str) -> Option<Level> {
        match s.to_ascii_lowercase().as_str() {
            "l" => Some(Level::L),
            "m" => Some(Level::M),
            "q" => Some(Level::Q),
            "h" => Some(Level::H),
            _ => None,
        }
    }
}

/// `[ec per block, group-1 blocks, group-1 data, group-2 blocks, group-2 data]`
/// indexed by `[version - 1][level]`. Every row satisfies
/// `blocks * (data + ec) == the version's total codewords`, which a test checks
/// rather than a reader.
const ECC: [[[usize; 5]; 4]; 6] = [
    [[7, 1, 19, 0, 0], [10, 1, 16, 0, 0], [13, 1, 13, 0, 0], [17, 1, 9, 0, 0]],
    [[10, 1, 34, 0, 0], [16, 1, 28, 0, 0], [22, 1, 22, 0, 0], [28, 1, 16, 0, 0]],
    [[15, 1, 55, 0, 0], [26, 1, 44, 0, 0], [18, 2, 17, 0, 0], [22, 2, 13, 0, 0]],
    [[20, 1, 80, 0, 0], [18, 2, 32, 0, 0], [26, 2, 24, 0, 0], [16, 4, 9, 0, 0]],
    [[26, 1, 108, 0, 0], [24, 2, 43, 0, 0], [18, 2, 15, 2, 16], [22, 2, 11, 2, 12]],
    [[18, 2, 68, 0, 0], [16, 4, 27, 0, 0], [24, 4, 19, 0, 0], [28, 4, 15, 0, 0]],
];

/// Total codewords per version, for the consistency test above.
#[cfg(test)]
const TOTAL_CODEWORDS: [usize; 6] = [26, 44, 70, 100, 134, 172];

/// Alignment-pattern centres per version. Version 1 has none.
const ALIGN: [&[usize]; 6] = [&[], &[6, 18], &[6, 22], &[6, 26], &[6, 30], &[6, 34]];

/// A finished code: `size` by `size` modules, one byte each, 1 for dark.
#[derive(Clone, Debug)]
pub struct Code {
    pub size: usize,
    pub version: usize,
    pub level: Level,
    modules: Vec<u8>,
}

impl Code {
    /// Whether the module at `(x, y)` is dark. Out of range is light, which is
    /// what the quiet zone is.
    pub fn dark(&self, x: isize, y: isize) -> bool {
        if x < 0 || y < 0 || x as usize >= self.size || y as usize >= self.size {
            return false;
        }
        self.modules[y as usize * self.size + x as usize] == 1
    }

    /// The grid as rows of `0`/`1`, for tests and for diffing against another
    /// implementation.
    pub fn rows(&self) -> Vec<String> {
        (0..self.size)
            .map(|y| {
                (0..self.size)
                    .map(|x| if self.dark(x as isize, y as isize) { '1' } else { '0' })
                    .collect()
            })
            .collect()
    }
}

/// What went wrong, in words a person can act on.
#[derive(Debug)]
pub enum Error {
    /// The payload does not fit in version 6 at this level.
    TooLong { bytes: usize, capacity: usize, level: Level },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::TooLong { bytes, capacity, level } => write!(
                f,
                "{bytes} bytes is more than the {capacity} a version-6 code holds at level {level:?}. \
                 Shorten it, or drop to level L for {} bytes.",
                ECC[5][Level::L.index()][1] * ECC[5][Level::L.index()][2] - 2
            ),
        }
    }
}

impl std::error::Error for Error {}

// --------------------------------------------------------------------------
// GF(256), the Reed-Solomon field, primitive polynomial 0x11d.
// --------------------------------------------------------------------------

struct Gf {
    exp: [u8; 512],
    log: [u8; 256],
}

impl Gf {
    fn new() -> Gf {
        let mut exp = [0u8; 512];
        let mut log = [0u8; 256];
        let mut x: u16 = 1;
        for (i, slot) in exp.iter_mut().take(255).enumerate() {
            *slot = x as u8;
            log[x as usize] = i as u8;
            x <<= 1;
            if x & 0x100 != 0 {
                x ^= 0x11d;
            }
        }
        for i in 255..512 {
            exp[i] = exp[i - 255];
        }
        Gf { exp, log }
    }

    fn mul(&self, a: u8, b: u8) -> u8 {
        if a == 0 || b == 0 {
            0
        } else {
            self.exp[self.log[a as usize] as usize + self.log[b as usize] as usize]
        }
    }

    /// `g(x) = product of (x - a^i)`, coefficients highest degree first.
    fn gen_poly(&self, n: usize) -> Vec<u8> {
        let mut g = vec![1u8];
        for i in 0..n {
            let mut ng = vec![0u8; g.len() + 1];
            for j in 0..g.len() {
                ng[j] ^= g[j];
                ng[j + 1] ^= self.mul(g[j], self.exp[i]);
            }
            g = ng;
        }
        g
    }

    fn remainder(&self, data: &[u8], ec_len: usize) -> Vec<u8> {
        let g = self.gen_poly(ec_len);
        let mut buf = vec![0u8; data.len() + ec_len];
        buf[..data.len()].copy_from_slice(data);
        for i in 0..data.len() {
            let factor = buf[i];
            if factor == 0 {
                continue;
            }
            for j in 0..g.len() {
                buf[i + j] ^= self.mul(g[j], factor);
            }
        }
        buf[data.len()..].to_vec()
    }
}

/// The eight masks, by index. `(row, col)`.
fn mask(m: usize, i: usize, j: usize) -> bool {
    match m {
        0 => (i + j) % 2 == 0,
        1 => i % 2 == 0,
        2 => j % 3 == 0,
        3 => (i + j) % 3 == 0,
        4 => ((i / 2) + (j / 3)) % 2 == 0,
        5 => (i * j) % 2 + (i * j) % 3 == 0,
        6 => ((i * j) % 2 + (i * j) % 3) % 2 == 0,
        _ => ((i + j) % 2 + (i * j) % 3) % 2 == 0,
    }
}

/// The 15-bit format word: 5 data bits, a BCH(15,5) remainder, XOR 0x5412.
///
/// Computed rather than tabled: six lines beats a 32-entry table nobody can
/// proofread.
fn format_word(level: Level, m: usize) -> u32 {
    let data = (level.format_bits() << 3) | m as u32;
    let mut rem = data;
    for _ in 0..10 {
        rem = (rem << 1) ^ ((rem >> 9) * 0x537);
    }
    ((data << 10) | rem) ^ 0x5412
}

/// Encode `text` as a QR code at `level`, choosing the smallest version that
/// holds it.
pub fn encode(text: &str, level: Level) -> Result<Code, Error> {
    let bytes = text.as_bytes();
    let li = level.index();

    let mut version = 0usize;
    for v in 1..=6usize {
        let t = &ECC[v - 1][li];
        let capacity = t[1] * t[2] + t[3] * t[4];
        if (4 + 8 + bytes.len() * 8).div_ceil(8) <= capacity {
            version = v;
            break;
        }
    }
    if version == 0 {
        let t = &ECC[5][li];
        return Err(Error::TooLong {
            bytes: bytes.len(),
            capacity: t[1] * t[2] + t[3] * t[4] - 2,
            level,
        });
    }

    let t = &ECC[version - 1][li];
    let ec_len = t[0];
    let total_data = t[1] * t[2] + t[3] * t[4];

    // ---- the bitstream ----------------------------------------------------
    let mut bits: Vec<u8> = Vec::with_capacity(total_data * 8);
    let push = |bits: &mut Vec<u8>, val: u32, len: u32| {
        for k in (0..len).rev() {
            bits.push(((val >> k) & 1) as u8);
        }
    };
    push(&mut bits, 4, 4); // byte mode
    push(&mut bits, bytes.len() as u32, 8); // count: 8 bits for versions 1-9
    for &b in bytes {
        push(&mut bits, b as u32, 8);
    }

    let capacity_bits = total_data * 8;
    for _ in 0..4 {
        if bits.len() >= capacity_bits {
            break;
        }
        bits.push(0);
    }
    while bits.len() % 8 != 0 {
        bits.push(0);
    }
    let pad = [0xECu32, 0x11];
    let mut k = 0;
    while bits.len() < capacity_bits {
        push(&mut bits, pad[k % 2], 8);
        k += 1;
    }

    let mut data_cw = vec![0u8; total_data];
    for (i, cw) in data_cw.iter_mut().enumerate() {
        let mut b = 0u8;
        for k in 0..8 {
            b = (b << 1) | bits[i * 8 + k];
        }
        *cw = b;
    }

    // ---- blocks, parity, interleave ---------------------------------------
    let gf = Gf::new();
    let mut blocks: Vec<&[u8]> = Vec::new();
    let mut parity: Vec<Vec<u8>> = Vec::new();
    let mut at = 0usize;
    for g in 0..2usize {
        let (count, size) = if g == 0 { (t[1], t[2]) } else { (t[3], t[4]) };
        for _ in 0..count {
            let blk = &data_cw[at..at + size];
            at += size;
            parity.push(gf.remainder(blk, ec_len));
            blocks.push(blk);
        }
    }
    let mut stream: Vec<u8> = Vec::with_capacity(total_data + ec_len * blocks.len());
    let longest = blocks.iter().map(|b| b.len()).max().unwrap_or(0);
    for i in 0..longest {
        for b in &blocks {
            if i < b.len() {
                stream.push(b[i]);
            }
        }
    }
    for i in 0..ec_len {
        for p in &parity {
            stream.push(p[i]);
        }
    }

    // ---- the grid ---------------------------------------------------------
    let size = version * 4 + 17;
    let mut modules = vec![0u8; size * size];
    let mut fixed = vec![0u8; size * size];
    let set = |modules: &mut Vec<u8>, fixed: &mut Vec<u8>, x: isize, y: isize, dark: bool| {
        if x < 0 || y < 0 || x as usize >= size || y as usize >= size {
            return;
        }
        let i = y as usize * size + x as usize;
        modules[i] = u8::from(dark);
        fixed[i] = 1;
    };

    // finders, with their separators: the ring at Chebyshev distance 2 is the
    // light gap, everything out to 3 is dark, 4 is the separator.
    for (cx, cy) in [(3isize, 3isize), (size as isize - 4, 3), (3, size as isize - 4)] {
        for dy in -4isize..=4 {
            for dx in -4isize..=4 {
                let d = dx.abs().max(dy.abs());
                set(&mut modules, &mut fixed, cx + dx, cy + dy, d != 2 && d <= 3);
            }
        }
    }

    // timing
    for i in 0..size {
        if fixed[6 * size + i] == 0 {
            set(&mut modules, &mut fixed, i as isize, 6, i % 2 == 0);
        }
        if fixed[i * size + 6] == 0 {
            set(&mut modules, &mut fixed, 6, i as isize, i % 2 == 0);
        }
    }

    // alignment, minus the three that would land on a finder
    let ac = ALIGN[version - 1];
    if !ac.is_empty() {
        let (first, last) = (ac[0], ac[ac.len() - 1]);
        for &cy in ac {
            for &cx in ac {
                let corner = (cx == first && cy == first)
                    || (cx == first && cy == last)
                    || (cx == last && cy == first);
                if corner {
                    continue;
                }
                for dy in -2isize..=2 {
                    for dx in -2isize..=2 {
                        set(
                            &mut modules,
                            &mut fixed,
                            cx as isize + dx,
                            cy as isize + dy,
                            dx.abs().max(dy.abs()) != 1,
                        );
                    }
                }
            }
        }
    }

    // **Reserve the format strip, and skip index 6 in both directions.** The
    // strip runs across the timing patterns but does not own the two modules
    // where they cross, at row 6 column 8 and row 8 column 6. Clearing those
    // leaves each timing line starting light instead of dark -- and the timing
    // line is exactly what a scanner uses to establish the module grid. Two
    // modules out of 1089, invisible in the picture, and every reader refuses
    // the code. This cost a bit-for-bit diff to find.
    for i in 0..9isize {
        if i == 6 {
            continue;
        }
        set(&mut modules, &mut fixed, 8, i, false);
        set(&mut modules, &mut fixed, i, 8, false);
    }
    for i in 0..8isize {
        set(&mut modules, &mut fixed, size as isize - 1 - i, 8, false);
        set(&mut modules, &mut fixed, 8, size as isize - 1 - i, false);
    }
    set(&mut modules, &mut fixed, 8, size as isize - 8, true);

    // ---- payload, two columns at a time, boustrophedon --------------------
    let mut bi = 0usize;
    let mut right = size as isize - 1;
    while right >= 1 {
        if right == 6 {
            right = 5; // the timing column is skipped whole
        }
        for vert in 0..size {
            for j in 0..2isize {
                let x = right - j;
                let upward = ((right + 1) & 2) == 0;
                let y = if upward { size - 1 - vert } else { vert };
                if fixed[y * size + x as usize] == 0 {
                    modules[y * size + x as usize] = if bi < stream.len() * 8 {
                        (stream[bi >> 3] >> (7 - (bi & 7))) & 1
                    } else {
                        0
                    };
                    bi += 1;
                }
            }
        }
        right -= 2;
    }

    // ---- pick the mask by the four penalty rules --------------------------
    let mut best: Option<Vec<u8>> = None;
    let mut best_score = u32::MAX;
    for m in 0..8usize {
        let mut cand = modules.clone();
        for y in 0..size {
            for x in 0..size {
                if fixed[y * size + x] == 0 && mask(m, y, x) {
                    cand[y * size + x] ^= 1;
                }
            }
        }
        write_format(&mut cand, size, level, m);
        let s = penalty(&cand, size);
        if s < best_score {
            best_score = s;
            best = Some(cand);
        }
    }

    Ok(Code {
        size,
        version,
        level,
        modules: best.expect("eight masks always produce one"),
    })
}

fn write_format(g: &mut [u8], size: usize, level: Level, m: usize) {
    let f = format_word(level, m);
    let bit = |i: u32| ((f >> i) & 1) as u8;
    for i in 0..=5usize {
        g[i * size + 8] = bit(i as u32);
    }
    g[7 * size + 8] = bit(6);
    g[8 * size + 8] = bit(7);
    g[8 * size + 7] = bit(8);
    for i in 9..15usize {
        g[8 * size + (14 - i)] = bit(i as u32);
    }
    for i in 0..8usize {
        g[8 * size + (size - 1 - i)] = bit(i as u32);
    }
    for i in 8..15usize {
        g[(size - 15 + i) * size + 8] = bit(i as u32);
    }
    g[(size - 8) * size + 8] = 1;
}

fn penalty(g: &[u8], size: usize) -> u32 {
    let mut score = 0u32;
    let at = |y: usize, x: usize| g[y * size + x];

    // 1: runs of five or more, each direction
    for transposed in [false, true] {
        for a in 0..size {
            let get = |b: usize| if transposed { at(b, a) } else { at(a, b) };
            let mut len = 1u32;
            let mut prev = get(0);
            for b in 1..size {
                let v = get(b);
                if v == prev {
                    len += 1;
                } else {
                    if len >= 5 {
                        score += 3 + (len - 5);
                    }
                    prev = v;
                    len = 1;
                }
            }
            if len >= 5 {
                score += 3 + (len - 5);
            }
        }
    }

    // 2: solid two-by-two blocks
    for y in 0..size - 1 {
        for x in 0..size - 1 {
            let v = at(y, x);
            if v == at(y, x + 1) && v == at(y + 1, x) && v == at(y + 1, x + 1) {
                score += 3;
            }
        }
    }

    // 3: the finder-lookalike sequence, either way round
    const A: [u8; 11] = [1, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0];
    const B: [u8; 11] = [0, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1];
    for transposed in [false, true] {
        for a in 0..size {
            for b in 0..=size.saturating_sub(11) {
                let mut ma = true;
                let mut mb = true;
                for k in 0..11 {
                    let v = if transposed { at(b + k, a) } else { at(a, b + k) };
                    if v != A[k] {
                        ma = false;
                    }
                    if v != B[k] {
                        mb = false;
                    }
                }
                if ma {
                    score += 40;
                }
                if mb {
                    score += 40;
                }
            }
        }
    }

    // 4: drift away from half dark
    let dark: u32 = g.iter().map(|&v| v as u32).sum();
    let pct = dark as f64 * 100.0 / (size * size) as f64;
    score += ((pct - 50.0).abs() / 5.0).floor() as u32 * 10;

    score
}

// --------------------------------------------------------------------------
// Printing one in a terminal.
// --------------------------------------------------------------------------

/// How to draw a code as text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Render {
    /// Half-block characters with **explicit** black-on-white colour codes.
    ///
    /// The default, and the only one whose contrast does not depend on the
    /// terminal's theme. A QR code is defined as dark modules on a light
    /// ground; a renderer that leaves the ground to the terminal produces an
    /// inverted code on half the machines in the world, and while many
    /// scanners cope with inversion, "many" is not a property worth shipping.
    Ansi,
    /// Half-block characters, no colour: two module rows per line of text.
    ///
    /// For a terminal that mangles escape codes. Assumes a **light**
    /// background; pass [`Render::UnicodeInverted`] on a dark one.
    Unicode,
    /// [`Render::Unicode`] with dark and light swapped, for a dark terminal.
    UnicodeInverted,
    /// Two ASCII characters per module, no Unicode and no colour.
    ///
    /// The fallback for a console that can render neither -- Windows `cmd.exe`
    /// in a legacy code page draws half-blocks as mojibake. Twice as tall on
    /// screen as the others and correspondingly harder to fit, but a scanner
    /// reads it.
    Ascii,
}

impl Code {
    /// Draw the code as lines of text, with a quiet zone of `quiet` modules.
    ///
    /// **Four is not decoration.** The specification requires a light margin of
    /// four modules; a reader locates the finder patterns by their 1:1:3:1:1
    /// run, and without the margin the run at the edge is truncated by whatever
    /// the terminal drew next to it.
    pub fn render(&self, how: Render, quiet: usize) -> String {
        match how {
            Render::Ansi => self.half_blocks(quiet, true, false),
            Render::Unicode => self.half_blocks(quiet, false, false),
            Render::UnicodeInverted => self.half_blocks(quiet, false, true),
            Render::Ascii => self.ascii(quiet),
        }
    }

    /// One line of text per two module rows, which is roughly square on screen
    /// because a character cell is about twice as tall as it is wide.
    fn half_blocks(&self, quiet: usize, colour: bool, invert: bool) -> String {
        let q = quiet as isize;
        let span = self.size as isize + 2 * q;
        let mut out = String::new();
        let mut row = -q;
        while row < self.size as isize + q {
            if colour {
                out.push_str("\x1b[0m");
            }
            for i in 0..span {
                let x = i - q;
                let upper = self.dark(x, row);
                let lower = self.dark(x, row + 1);
                if colour {
                    // Foreground paints the upper half, background the lower.
                    let fg = if upper { 30 } else { 97 };
                    let bg = if lower { 40 } else { 107 };
                    out.push_str(&format!("\x1b[{fg};{bg}m\u{2580}"));
                } else {
                    let (u, l) = if invert { (!upper, !lower) } else { (upper, lower) };
                    out.push(match (u, l) {
                        (true, true) => '\u{2588}',
                        (true, false) => '\u{2580}',
                        (false, true) => '\u{2584}',
                        (false, false) => ' ',
                    });
                }
            }
            if colour {
                out.push_str("\x1b[0m");
            }
            out.push('\n');
            row += 2;
        }
        out
    }

    /// Two characters per module: `##` dark, two spaces light.
    fn ascii(&self, quiet: usize) -> String {
        let q = quiet as isize;
        let span = self.size as isize + 2 * q;
        let mut out = String::new();
        for row in -q..self.size as isize + q {
            for i in 0..span {
                out.push_str(if self.dark(i - q, row) { "##" } else { "  " });
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every row of the table has to add up, or the code is malformed in a way
    /// that still renders.
    #[test]
    fn the_ecc_table_is_consistent() {
        for v in 1..=6usize {
            for li in 0..4usize {
                let t = ECC[v - 1][li];
                let total = t[1] * (t[2] + t[0]) + t[3] * (t[4] + t[0]);
                assert_eq!(
                    total,
                    TOTAL_CODEWORDS[v - 1],
                    "version {v} level {li}: {total} codewords, want {}",
                    TOTAL_CODEWORDS[v - 1]
                );
            }
        }
    }

    #[test]
    fn sizes_and_versions_follow_the_payload() {
        for (text, want_version) in [
            ("hi", 1usize),
            // 24 bytes: 26 codewords once mode and count are in, and version 2
            // at level Q holds 22. The step to 3 is the encoder working.
            ("http://192.168.1.20:8080", 3),
            ("http://a-fairly-long-hostname.local:8080/path", 4),
        ] {
            let c = encode(text, Level::Q).expect("fits");
            assert_eq!(c.version, want_version, "{text:?}");
            assert_eq!(c.size, want_version * 4 + 17);
        }
    }

    /// The two modules that a format strip clobbers if index 6 is not skipped.
    /// They are what a scanner uses to find the grid, and they are invisible.
    #[test]
    fn the_timing_patterns_survive_the_format_strip() {
        let c = encode("http://192.168.1.20:8080", Level::Q).unwrap();
        assert!(c.dark(6, 8) == (8 % 2 == 0), "timing row broken at column 8");
        assert!(c.dark(8, 6) == (8 % 2 == 0), "timing column broken at row 8");
        // And the whole line alternates, starting dark at the finder edge.
        for i in 8..c.size - 8 {
            assert_eq!(c.dark(i as isize, 6), i % 2 == 0, "timing row at {i}");
            assert_eq!(c.dark(6, i as isize), i % 2 == 0, "timing column at {i}");
        }
    }

    #[test]
    fn the_finders_are_where_a_reader_looks() {
        let c = encode("hello", Level::Q).unwrap();
        let n = c.size as isize;
        for (cx, cy) in [(3isize, 3isize), (n - 4, 3), (3, n - 4)] {
            // The 1:1:3:1:1 run a reader scans for, with the light separator
            // on each side of it. Nine modules, and every one of them matters:
            // the ratio is how a scanner finds the code at all.
            let run: String = (-4isize..=4)
                .map(|d| if c.dark(cx + d, cy) { '1' } else { '0' })
                .collect();
            assert_eq!(run, "010111010", "finder at ({cx},{cy})");
        }
    }

    #[test]
    fn too_long_says_what_to_do() {
        let e = encode(&"x".repeat(200), Level::Q).unwrap_err();
        let s = e.to_string();
        assert!(s.contains("200 bytes"), "{s}");
        assert!(s.contains("level L"), "{s}");
    }

    /// The quiet zone is the difference between a code that reads and one that
    /// does not, so its presence is asserted rather than assumed.
    #[test]
    fn every_renderer_leaves_a_margin() {
        let c = encode("http://192.168.1.20:8080", Level::Q).unwrap();
        let ascii = c.render(Render::Ascii, 4);
        let lines: Vec<&str> = ascii.lines().collect();
        assert_eq!(lines.len(), c.size + 8);
        for line in lines.iter().take(4) {
            assert!(line.trim().is_empty(), "top margin is not blank");
        }
        for line in &lines {
            assert!(line.starts_with("        "), "left margin is not four modules");
            assert_eq!(line.chars().count(), (c.size + 8) * 2);
        }
    }

    /// A colour renderer that emits no colour would be an inverted code on a
    /// dark terminal, silently.
    #[test]
    fn the_ansi_renderer_sets_both_halves() {
        let c = encode("hi", Level::Q).unwrap();
        let out = c.render(Render::Ansi, 4);
        assert!(out.contains("\x1b[30;40m"), "no dark-over-dark cell");
        assert!(out.contains("\x1b[97;107m"), "no light-over-light cell");
        assert!(out.contains('\u{2580}'));
    }

    /// **Read every rendering back into modules and compare.**
    ///
    /// This is the only check that says a *drawn* code is the code. A renderer
    /// that dropped a row, doubled a column or lost the last odd line would
    /// still look like a QR code to a person and be unreadable to a scanner --
    /// which is exactly the failure this project has already paid for once, in
    /// two clobbered timing modules out of 1089 that no amount of looking
    /// revealed.
    #[test]
    fn every_rendering_reads_back_as_the_same_code() {
        for text in ["hi", "http://192.168.1.20:8080", &"x".repeat(74)] {
            let c = encode(text, Level::Q).unwrap();
            let want = c.rows();

            // Half-blocks, no colour: two module rows per line of text.
            for (how, inverted) in [(Render::Unicode, false), (Render::UnicodeInverted, true)] {
                let mut got: Vec<String> = Vec::new();
                for line in c.render(how, 4).lines() {
                    let mut upper = String::new();
                    let mut lower = String::new();
                    for ch in line.chars() {
                        let (mut u, mut l) = match ch {
                            '\u{2588}' => (true, true),
                            '\u{2580}' => (true, false),
                            '\u{2584}' => (false, true),
                            ' ' => (false, false),
                            other => panic!("{how:?} drew {other:?}"),
                        };
                        if inverted {
                            u = !u;
                            l = !l;
                        }
                        upper.push(if u { '1' } else { '0' });
                        lower.push(if l { '1' } else { '0' });
                    }
                    got.push(upper);
                    got.push(lower);
                }
                // Strip the four-module margin off every side.
                let inner: Vec<String> = got[4..4 + c.size]
                    .iter()
                    .map(|r| r[4..4 + c.size].to_string())
                    .collect();
                assert_eq!(inner, want, "{how:?} on {:?}", &text[..text.len().min(20)]);
                // And the margin really is blank, on all four sides.
                for r in got.iter().take(4).chain(got[4 + c.size..].iter()) {
                    assert!(r.chars().all(|ch| ch == '0'), "{how:?} margin has ink");
                }
                for r in &got {
                    assert!(r[..4].chars().all(|ch| ch == '0'), "{how:?} left margin");
                    assert!(
                        r[4 + c.size..].chars().all(|ch| ch == '0'),
                        "{how:?} right margin"
                    );
                }
            }

            // Two ASCII characters per module, one line per module row.
            let ascii: Vec<String> = c
                .render(Render::Ascii, 4)
                .lines()
                .skip(4)
                .take(c.size)
                .map(|line| {
                    line.as_bytes()
                        .chunks(2)
                        .skip(4)
                        .take(c.size)
                        .map(|pair| if pair == b"##" { '1' } else { '0' })
                        .collect()
                })
                .collect();
            assert_eq!(ascii, want, "Ascii on {:?}", &text[..text.len().min(20)]);
        }
    }

    #[test]
    fn inverting_swaps_every_cell() {
        let c = encode("hi", Level::Q).unwrap();
        let plain = c.render(Render::Unicode, 0);
        let inv = c.render(Render::UnicodeInverted, 0);
        assert_eq!(plain.chars().count(), inv.chars().count());
        let swap = |ch: char| match ch {
            '\u{2588}' => ' ',
            ' ' => '\u{2588}',
            '\u{2580}' => '\u{2584}',
            '\u{2584}' => '\u{2580}',
            other => other,
        };
        assert_eq!(plain.chars().map(swap).collect::<String>(), inv);
    }
}
