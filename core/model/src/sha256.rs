//! SHA-256, streaming, in about a hundred and fifty lines.
//!
//! # Why this exists, and why *this* hash
//!
//! §4e demonstrated the failure it is here to catch. Four kilobytes of zeros
//! written into a container's tensor data **loads, exits 0 and answers
//! fluently** — "The capital of France is" gives *" Paris. The capital of Germany
//! is Berlin"* where the intact model says *" Paris. The capital of France is
//! Paris"*. Both plausible, neither flagged. There was no checksum anywhere:
//! `download` verifies `looks_like_gguf`, which is the four magic bytes.
//!
//! **SHA-256 rather than something faster.** A 64-bit non-cryptographic hash
//! would detect bit-rot at several gigabytes a second and would have been a
//! quarter of this code. It was rejected for one practical reason: model
//! publishers, Hugging Face included, publish **SHA-256** digests for their
//! files. Recording the same function means a file can be checked against *the
//! publisher's* value, not merely against our own earlier read of it — which is
//! the difference between "this is the file I downloaded" and "this is the file
//! they published". Trust-on-first-use catches rot; the publisher's digest
//! catches a bad download.
//!
//! **Written out rather than depended on**, like everything else here: the
//! workspace has zero third-party crates and `Cargo.lock` says so.
//!
//! # Correct by construction is not a thing
//!
//! A hash that is subtly wrong is worse than none: it would report a healthy file
//! as corrupt, or worse, agree with itself while disagreeing with every other
//! implementation. So this is tested against the published FIPS 180-4 vectors,
//! against the empty input, across every buffer boundary from 0 to 200 bytes, and
//! against a payload larger than one block fed in deliberately awkward chunks.

/// The eight initial hash values: the fractional parts of the square roots of
/// the first eight primes.
const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// The sixty-four round constants: fractional parts of the cube roots of the
/// first sixty-four primes.
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// A hash in progress. Feed it with [`update`](Sha256::update), finish with
/// [`hex`](Sha256::hex).
///
/// **Streaming on purpose.** A 144 GB container cannot be read into memory to be
/// hashed, which is the only size that matters here.
pub struct Sha256 {
    state: [u32; 8],
    /// Bytes not yet formed into a full 64-byte block.
    buffer: [u8; 64],
    buffered: usize,
    /// Total message length in bytes, which the padding encodes in bits.
    length: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Sha256 {
            state: H0,
            buffer: [0u8; 64],
            buffered: 0,
            length: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.length = self.length.wrapping_add(data.len() as u64);

        // Top up a partial block first.
        if self.buffered > 0 {
            let want = 64 - self.buffered;
            let take = want.min(data.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&data[..take]);
            self.buffered += take;
            data = &data[take..];
            if self.buffered == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffered = 0;
            }
        }

        // Then whole blocks straight out of the caller's slice.
        while data.len() >= 64 {
            let (block, rest) = data.split_at(64);
            let mut b = [0u8; 64];
            b.copy_from_slice(block);
            self.compress(&b);
            data = rest;
        }

        // Keep the remainder.
        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buffered = data.len();
        }
    }

    /// The digest as 32 bytes.
    pub fn finish(mut self) -> [u8; 32] {
        // Padding: a single 1 bit, zeros, then the length in bits as 64 bits big
        // endian, so that the whole message is a multiple of 64 bytes.
        let bits = self.length.wrapping_mul(8);
        self.update_no_count(&[0x80]);
        while self.buffered != 56 {
            self.update_no_count(&[0x00]);
        }
        let len_be = bits.to_be_bytes();
        self.update_no_count(&len_be);
        debug_assert_eq!(self.buffered, 0, "padding did not land on a block boundary");

        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    /// The digest as lower-case hex, which is how every publisher prints it.
    pub fn hex(self) -> String {
        let bytes = self.finish();
        let mut s = String::with_capacity(64);
        for b in bytes {
            s.push(char::from_digit((b >> 4) as u32, 16).unwrap_or('0'));
            s.push(char::from_digit((b & 0x0f) as u32, 16).unwrap_or('0'));
        }
        s
    }

    /// Feed padding without disturbing the recorded message length.
    fn update_no_count(&mut self, data: &[u8]) {
        for &byte in data {
            self.buffer[self.buffered] = byte;
            self.buffered += 1;
            if self.buffered == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffered = 0;
            }
        }
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (s, v) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *s = s.wrapping_add(v);
        }
    }
}

/// Hash a whole slice in one call.
pub fn hex_of(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    h.hex()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The published FIPS 180-4 examples, plus the empty input.
    ///
    /// **These are the reason to trust any of this.** A hash that agrees only
    /// with itself is worthless; these digests are the ones every other
    /// implementation in the world produces.
    #[test]
    fn the_published_vectors() {
        for (input, expected) in [
            (
                "",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                "abc",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
        ] {
            assert_eq!(hex_of(input.as_bytes()), expected, "input {input:?}");
        }
    }

    /// A million 'a's — the vector that catches a broken length field, because
    /// the message is long enough for the bit count to exceed 32 bits of
    /// significance.
    #[test]
    fn a_million_letters() {
        let mut h = Sha256::new();
        let chunk = vec![b'a'; 1000];
        for _ in 0..1000 {
            h.update(&chunk);
        }
        assert_eq!(
            h.hex(),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    /// **Chunking must not change the answer.** This is the property a streaming
    /// hash exists for, and the one a buffer-boundary bug breaks: the same bytes
    /// fed as 1, 63, 64, 65 and 1000-byte pieces must agree.
    #[test]
    fn the_chunking_cannot_change_the_digest() {
        let data: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        let once = hex_of(&data);
        for size in [1usize, 2, 63, 64, 65, 127, 128, 129, 1000, 4096] {
            let mut h = Sha256::new();
            for piece in data.chunks(size) {
                h.update(piece);
            }
            assert_eq!(h.hex(), once, "chunked at {size} bytes");
        }
    }

    /// Every length across the padding boundary, where an off-by-one lives.
    ///
    /// 55/56/57 and 119/120/121 are the interesting ones: the length field needs
    /// eight bytes and the 1 bit needs one, so a 56-byte message pushes the
    /// padding into a second block.
    #[test]
    fn every_length_up_to_two_blocks_is_self_consistent() {
        let data: Vec<u8> = (0..200u32).map(|i| (i * 7 % 256) as u8).collect();
        for n in 0..=200usize {
            let a = hex_of(&data[..n]);
            let mut h = Sha256::new();
            for piece in data[..n].chunks(7) {
                h.update(piece);
            }
            assert_eq!(h.hex(), a, "length {n} disagreed with itself when chunked");
            assert_eq!(a.len(), 64, "length {n} produced a malformed digest");
            assert!(
                a.chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()),
                "length {n} produced non-hex or upper case: {a}"
            );
        }
    }

    /// One flipped bit anywhere must change the digest — the whole point.
    #[test]
    fn a_single_changed_byte_changes_everything() {
        let mut data = vec![0u8; 4096];
        let clean = hex_of(&data);
        for at in [0usize, 1, 63, 64, 2047, 4095] {
            data[at] ^= 0x01;
            let dirty = hex_of(&data);
            assert_ne!(clean, dirty, "flipping byte {at} did not change the digest");
            data[at] ^= 0x01;
        }
        assert_eq!(hex_of(&data), clean, "the data was not restored");
    }
}
