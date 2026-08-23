//! What travels between the main device and a worker.
//!
//! # The number this protocol exists to exploit
//!
//! **A hidden state is 16 KB and a token's expert weights are 3.3 GB.**
//! Everything here follows from that ratio:
//!
//! | what moves | per token, V4-Flash | over 1 GbE |
//! |---|---|---|
//! | one layer's hidden state | 16 KB | 0.13 ms |
//! | expert-parallel over four machines, there and back | ~6.9 MB | ~55 ms |
//! | the expert weights themselves | 3.3 GB | 26 s — **never** |
//!
//! Against a token that costs 2400 ms today, of which 1560 ms is disk. So the
//! work goes to the weights, never the weights to the work.
//!
//! # The shape of an exchange
//!
//! ```text
//!   main -> worker   Compute { layer 17, jobs [(tok 0, expert 47), (tok 0, expert 191)],
//!                              hidden: 4096 floats }
//!   worker -> main   Activations { 2 blocks of 4096 floats }
//! ```
//!
//! **A job is a (token, expert) pair, and the answer's order is the request's
//! order.** That is the whole contract. It matters because a worker holds only
//! *some* of the experts: for one token routed to six, worker A might own two
//! of them and worker B four, so neither can be told "compute slot 3" and
//! neither needs to be. The main device knows where each answer goes because it
//! knows what it asked for.
//!
//! An earlier draft carried a flat list of experts for the whole batch. That is
//! right for exactly one token and quietly wrong for any other, because each
//! token routes to its *own* experts — and the failure would have been correct
//! shapes with the wrong weights, which is the silent kind.
//!
//! # Why a binary frame and not JSON
//!
//! The payload is a block of `f32`. JSON would be roughly 6x the bytes and cost
//! a parse on both ends, 43 times per token on V4-Flash. Past the handshake the
//! protocol carries no strings at all, so there is nothing a text format would
//! make readable.
//!
//! **The worker is stateless per token**: it holds weights and nothing else —
//! no KV cache, no position, no history. That is what makes a worker dying
//! mid-generation a slowdown rather than a corruption. The main device falls
//! back to reading that expert from its own disk and carries on.

use std::io::{Read, Write};

/// Bytes on the wire before the payload: magic, kind, and a length.
pub const HEADER: usize = 12;

/// `CHW1` — Chaos Worker, version 1.
///
/// **Checked on every frame, not just the handshake.** A protocol that trusts
/// its stream after the first message reads a desynchronised length as a
/// gigabyte allocation, and the failure is an out-of-memory kill rather than a
/// message saying the two ends disagree.
pub const MAGIC: u32 = u32::from_le_bytes(*b"CHW1");

/// The largest payload a frame may declare.
///
/// A hidden state for 2048 tokens at 8192 wide is 64 MB; 256 MB is comfortably
/// above anything real and far below "the length field was garbage". Without a
/// cap, a corrupt length is a `Vec::with_capacity` of whatever four bytes
/// happened to say.
pub const MAX_PAYLOAD: usize = 256 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Kind {
    /// Main -> worker: what do you hold?
    Hello = 1,
    /// Worker -> main: this is what I hold.
    Held = 2,
    /// Main -> worker: run these experts over this hidden state.
    Compute = 3,
    /// Worker -> main: here is what they produced.
    Activations = 4,
    /// Either way: something went wrong, and here is the sentence.
    Failed = 5,
}

impl Kind {
    fn from_u32(v: u32) -> Option<Kind> {
        Some(match v {
            1 => Kind::Hello,
            2 => Kind::Held,
            3 => Kind::Compute,
            4 => Kind::Activations,
            5 => Kind::Failed,
            _ => return None,
        })
    }
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    /// The stream is not speaking this protocol, or has lost its place.
    BadMagic(u32),
    UnknownKind(u32),
    /// A length field no real message would carry.
    TooLarge(usize),
    /// The payload is not the size its own header implies.
    Truncated {
        want: usize,
        got: usize,
    },
    /// The other end said what went wrong.
    Remote(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::BadMagic(got) => write!(
                f,
                "not a Chaos worker stream (magic {got:#010x}, expected {MAGIC:#010x})"
            ),
            Error::UnknownKind(k) => write!(f, "unknown message kind {k}"),
            Error::TooLarge(n) => write!(f, "frame declares {n} bytes, over the {MAX_PAYLOAD} cap"),
            Error::Truncated { want, got } => {
                write!(f, "frame said {want} bytes and carried {got}")
            }
            Error::Remote(m) => write!(f, "the worker said: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Write one frame: magic, kind, length, payload.
pub fn write_frame(w: &mut impl Write, kind: Kind, payload: &[u8]) -> Result<()> {
    if payload.len() > MAX_PAYLOAD {
        return Err(Error::TooLarge(payload.len()));
    }
    let mut head = [0u8; HEADER];
    head[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    head[4..8].copy_from_slice(&(kind as u32).to_le_bytes());
    head[8..12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    w.write_all(&head)?;
    w.write_all(payload)?;
    // **Flushed here, not left to the caller.** A request sitting in a buffer
    // while both ends wait to read is a deadlock that looks exactly like a slow
    // worker, and it would be diagnosed as one.
    w.flush()?;
    Ok(())
}

/// Read one frame.
pub fn read_frame(r: &mut impl Read) -> Result<(Kind, Vec<u8>)> {
    let mut head = [0u8; HEADER];
    r.read_exact(&mut head)?;
    let magic = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
    if magic != MAGIC {
        return Err(Error::BadMagic(magic));
    }
    let kind = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
    let kind = Kind::from_u32(kind).ok_or(Error::UnknownKind(kind))?;
    let len = u32::from_le_bytes([head[8], head[9], head[10], head[11]]) as usize;
    if len > MAX_PAYLOAD {
        return Err(Error::TooLarge(len));
    }
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)?;
    Ok((kind, payload))
}

/// What a worker holds, answered to `Hello`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// The container this worker's weights came from.
    ///
    /// **This is not politeness.** Routing a Qwen3 token to a worker holding
    /// V4-Flash experts does not fail: it returns activations of exactly the
    /// right shape and entirely the wrong content, and the result is fluent
    /// nonsense with nothing in any log. The main device compares this before
    /// it sends anything.
    pub model: String,
    /// Which layers this worker can answer for.
    pub layers: Vec<u32>,
    /// Which experts within those layers, in the model's own numbering.
    pub experts: Vec<u32>,
    /// Bytes of expert weight held resident.
    pub bytes: u64,
    /// `n_embd`, so a shape disagreement is caught at the handshake rather than
    /// inside a matmul.
    pub width: u32,
}

impl Held {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let name = self.model.as_bytes();
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&self.bytes.to_le_bytes());
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&(self.layers.len() as u32).to_le_bytes());
        for l in &self.layers {
            out.extend_from_slice(&l.to_le_bytes());
        }
        out.extend_from_slice(&(self.experts.len() as u32).to_le_bytes());
        for e in &self.experts {
            out.extend_from_slice(&e.to_le_bytes());
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Held> {
        let mut c = Cursor::new(b);
        let n = c.u32()? as usize;
        let model = String::from_utf8_lossy(c.take(n)?).into_owned();
        let bytes = c.u64()?;
        let width = c.u32()?;
        let layers = c.u32s()?;
        let experts = c.u32s()?;
        Ok(Held {
            model,
            layers,
            experts,
            bytes,
            width,
        })
    }
}

/// One unit of work: apply `expert` to token `token`'s hidden state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Job {
    pub token: u32,
    /// The expert's index in the **model's** numbering, not an index into
    /// whatever subset this worker happens to hold. The main device must not
    /// have to know how a worker arranged its slice.
    pub expert: u32,
}

/// Run these experts over this hidden state.
#[derive(Debug, Clone, PartialEq)]
pub struct Compute {
    pub layer: u32,
    /// Tokens in this batch.
    pub tokens: u32,
    /// `n_embd`.
    pub width: u32,
    /// What to compute. The answer comes back in this order.
    pub jobs: Vec<Job>,
    /// `tokens * width` floats, token-major.
    pub hidden: Vec<f32>,
}

impl Compute {
    /// Bytes this will occupy on the wire, without building it.
    ///
    /// The main device uses this to decide whether a worker is worth asking:
    /// below some size the round trip costs more than the local disk read it
    /// would replace, and that crossover is the whole design.
    pub fn wire_bytes(&self) -> usize {
        HEADER + 16 + self.jobs.len() * 8 + self.hidden.len() * 4
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.wire_bytes() - HEADER);
        out.extend_from_slice(&self.layer.to_le_bytes());
        out.extend_from_slice(&self.tokens.to_le_bytes());
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&(self.jobs.len() as u32).to_le_bytes());
        for j in &self.jobs {
            out.extend_from_slice(&j.token.to_le_bytes());
            out.extend_from_slice(&j.expert.to_le_bytes());
        }
        for v in &self.hidden {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Compute> {
        let mut c = Cursor::new(b);
        let layer = c.u32()?;
        let tokens = c.u32()?;
        let width = c.u32()?;
        let nj = c.u32()? as usize;
        let mut jobs = Vec::with_capacity(nj.min(1 << 16));
        for _ in 0..nj {
            jobs.push(Job {
                token: c.u32()?,
                expert: c.u32()?,
            });
        }
        // **The declared shape decides the read, and is checked against what is
        // left.** Trusting either alone lets a truncated frame become a shorter
        // hidden state of the right type — which computes, and is wrong.
        let hidden = c.f32s((tokens as usize) * (width as usize))?;
        Ok(Compute {
            layer,
            tokens,
            width,
            jobs,
            hidden,
        })
    }
}

/// What the experts produced: one `width`-long block per job, in the order the
/// jobs were asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct Activations {
    pub width: u32,
    pub jobs: u32,
    pub values: Vec<f32>,
}

impl Activations {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + self.values.len() * 4);
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.jobs.to_le_bytes());
        for v in &self.values {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Activations> {
        let mut c = Cursor::new(b);
        let width = c.u32()?;
        let jobs = c.u32()?;
        let values = c.f32s((width as usize) * (jobs as usize))?;
        Ok(Activations {
            width,
            jobs,
            values,
        })
    }

    /// The block one job produced.
    pub fn block(&self, job: usize) -> Option<&[f32]> {
        let w = self.width as usize;
        self.values.get(job * w..(job + 1) * w)
    }
}

/// A reader over a payload that cannot run off the end.
struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(b: &'a [u8]) -> Self {
        Cursor { b, at: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        // Checked as a sum that cannot wrap: `at + n` on a garbage length would
        // otherwise wrap to something small and pass.
        let end = self.at.checked_add(n).ok_or(Error::Truncated {
            want: usize::MAX,
            got: self.b.len(),
        })?;
        if end > self.b.len() {
            return Err(Error::Truncated {
                want: end,
                got: self.b.len(),
            });
        }
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn u64(&mut self) -> Result<u64> {
        let s = self.take(8)?;
        Ok(u64::from_le_bytes([
            s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
        ]))
    }
    fn u32s(&mut self) -> Result<Vec<u32>> {
        let n = self.u32()? as usize;
        // The count is checked against what is actually left before anything is
        // reserved, so a corrupt count cannot become an allocation.
        let s = self.take(n * 4)?;
        Ok(s.chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect())
    }
    fn f32s(&mut self, n: usize) -> Result<Vec<f32>> {
        let s = self.take(n * 4)?;
        Ok(s.chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_survives_the_round_trip() {
        let mut buf = Vec::new();
        write_frame(&mut buf, Kind::Compute, b"payload").unwrap();
        assert_eq!(buf.len(), HEADER + 7);
        let (kind, body) = read_frame(&mut buf.as_slice()).unwrap();
        assert_eq!(kind, Kind::Compute);
        assert_eq!(body, b"payload");
    }

    /// **The magic is checked on every frame, not only the first.** A stream
    /// that has lost its place reads the next four bytes as a length; without
    /// this, a desynchronised connection becomes a gigabyte allocation rather
    /// than a message saying the two ends disagree.
    #[test]
    fn a_stream_that_is_not_this_protocol_is_refused() {
        let junk = b"GET / HTTP/1.1\r\n\r\npadding-to-make-it-long-enough";
        match read_frame(&mut junk.as_slice()) {
            Err(Error::BadMagic(_)) => {}
            other => panic!("expected BadMagic, got {other:?}"),
        }
    }

    #[test]
    fn an_impossible_length_is_refused_before_it_is_allocated() {
        let mut head = [0u8; HEADER];
        head[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        head[4..8].copy_from_slice(&(Kind::Compute as u32).to_le_bytes());
        head[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        match read_frame(&mut head.as_slice()) {
            Err(Error::TooLarge(n)) => assert_eq!(n, u32::MAX as usize),
            other => panic!("expected TooLarge, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_kind_is_named_rather_than_ignored() {
        let mut head = [0u8; HEADER];
        head[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        head[4..8].copy_from_slice(&99u32.to_le_bytes());
        match read_frame(&mut head.as_slice()) {
            Err(Error::UnknownKind(99)) => {}
            other => panic!("expected UnknownKind(99), got {other:?}"),
        }
    }

    #[test]
    fn what_a_worker_holds_survives_the_round_trip() {
        let held = Held {
            model: "DeepSeek-V4-Flash-Q4_K_M".into(),
            layers: (0..43).collect(),
            experts: vec![0, 1, 2, 128, 255],
            bytes: 17_179_869_184,
            width: 4096,
        };
        assert_eq!(Held::decode(&held.encode()).unwrap(), held);
    }

    /// A request at V4-Flash's real shape, and the 16 KB the design rests on.
    #[test]
    fn a_request_carries_sixteen_kilobytes_per_token() {
        let width = 4096u32;
        let req = Compute {
            layer: 17,
            tokens: 1,
            width,
            jobs: vec![
                Job {
                    token: 0,
                    expert: 47,
                },
                Job {
                    token: 0,
                    expert: 191,
                },
            ],
            hidden: (0..width).map(|i| i as f32 * 1e-3).collect(),
        };
        let bytes = req.encode();
        assert_eq!(bytes.len() + HEADER, req.wire_bytes());
        // 16 KB of hidden state plus a handful of bytes of routing.
        assert_eq!(width as usize * 4, 16_384);
        assert!(
            req.wire_bytes() < 17_000,
            "a one-token request is {} bytes",
            req.wire_bytes()
        );
        assert_eq!(Compute::decode(&bytes).unwrap(), req);
    }

    /// **Each token routes to its own experts**, and the protocol has to carry
    /// that. A flat list of experts for the whole batch is right for exactly
    /// one token and silently wrong for any other — right shapes, wrong
    /// weights, no error.
    #[test]
    fn two_tokens_may_route_to_different_experts() {
        let req = Compute {
            layer: 3,
            tokens: 2,
            width: 4,
            jobs: vec![
                Job {
                    token: 0,
                    expert: 11,
                },
                Job {
                    token: 1,
                    expert: 250,
                },
                Job {
                    token: 1,
                    expert: 7,
                },
            ],
            hidden: (0..8).map(|i| i as f32).collect(),
        };
        let back = Compute::decode(&req.encode()).unwrap();
        assert_eq!(back, req);
        assert_eq!(back.jobs[1].token, 1);
        assert_eq!(back.jobs[1].expert, 250);
    }

    /// The answer's order is the request's order, and that is the contract by
    /// which the main device knows where each block goes.
    #[test]
    fn an_answer_is_one_block_per_job_in_order() {
        let width = 4u32;
        let ans = Activations {
            width,
            jobs: 3,
            values: (0..12).map(|i| i as f32).collect(),
        };
        let back = Activations::decode(&ans.encode()).unwrap();
        assert_eq!(back, ans);
        assert_eq!(back.block(0).unwrap(), &[0.0, 1.0, 2.0, 3.0]);
        assert_eq!(back.block(2).unwrap(), &[8.0, 9.0, 10.0, 11.0]);
        assert!(back.block(3).is_none());
    }

    /// **A truncated payload must not decode into a shorter, valid-looking
    /// one.** The shape is declared in the header, so a frame that lost bytes
    /// would otherwise become a hidden state for fewer tokens — which computes,
    /// and is silently wrong.
    #[test]
    fn a_truncated_payload_is_refused_rather_than_reinterpreted() {
        let req = Compute {
            layer: 0,
            tokens: 2,
            width: 8,
            jobs: vec![Job {
                token: 0,
                expert: 1,
            }],
            hidden: (0..16).map(|i| i as f32).collect(),
        };
        let mut bytes = req.encode();
        bytes.truncate(bytes.len() - 4);
        match Compute::decode(&bytes) {
            Err(Error::Truncated { .. }) => {}
            other => panic!("expected Truncated, got {other:?}"),
        }

        let ans = Activations {
            width: 4,
            jobs: 2,
            values: (0..8).map(|i| i as f32).collect(),
        };
        let mut bytes = ans.encode();
        bytes.truncate(bytes.len() - 4);
        assert!(Activations::decode(&bytes).is_err());
    }

    /// A job count that would allocate the world is refused by what is
    /// actually in the buffer, not by a limit somebody guessed.
    #[test]
    fn a_corrupt_count_cannot_become_an_allocation() {
        let mut b = Vec::new();
        b.extend_from_slice(&0u32.to_le_bytes()); // layer
        b.extend_from_slice(&1u32.to_le_bytes()); // tokens
        b.extend_from_slice(&4u32.to_le_bytes()); // width
        b.extend_from_slice(&u32::MAX.to_le_bytes()); // jobs -- a lie
        assert!(Compute::decode(&b).is_err());
    }

    /// Every float, bit for bit. An activation that arrived *nearly* right
    /// would be the worst possible outcome: no error, and a wrong answer.
    #[test]
    fn the_floats_are_exact_and_include_the_awkward_ones() {
        let vals = vec![
            0.0f32,
            -0.0,
            1.0,
            -1.0,
            f32::MIN_POSITIVE,
            f32::MAX,
            f32::MIN,
            1.0 / 3.0,
            -2.5e-8,
        ];
        let ans = Activations {
            width: vals.len() as u32,
            jobs: 1,
            values: vals.clone(),
        };
        let back = Activations::decode(&ans.encode()).unwrap();
        for (a, b) in vals.iter().zip(&back.values) {
            assert_eq!(a.to_bits(), b.to_bits(), "{a} came back as {b}");
        }
    }
}
