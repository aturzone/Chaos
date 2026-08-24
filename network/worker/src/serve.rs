//! The loop: accept a connection, answer frames until it closes.
//!
//! # One connection, one thread, requests in order
//!
//! A worker answers a *layer at a time* for a token that is otherwise stopped
//! — the main device cannot start layer 18 until layer 17 comes back. So there
//! is nothing to overlap within one stream, and a thread per connection is both
//! the simplest and the fastest thing here. Several main devices would each get
//! their own thread; whether that is ever wanted is a question for after the
//! measurement, not before.
//!
//! # Errors are frames, not closed sockets
//!
//! A worker that drops the connection on a bad request tells the main device
//! only that something happened. `Kind::Failed` carries the sentence, and the
//! connection survives — which matters because the main device's fallback is to
//! read that expert from its own disk and carry on, and it should not have to
//! reconnect to ask the next question.

use crate::slice::Slice;
use crate::wire::{self, Activations, Compute, Held, Kind};
use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

/// What one exchange cost, for the measurement that decides this whole design.
#[derive(Debug, Default, Clone, Copy)]
pub struct Timing {
    pub requests: u64,
    pub jobs: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    /// Time inside `Slice::compute`, i.e. arithmetic only.
    pub compute_nanos: u64,
}

/// Serve until the listener is dropped.
pub fn serve(
    listener: TcpListener,
    slice: Arc<Slice>,
    threads: usize,
    mut on_client: impl FnMut(&std::net::SocketAddr),
) -> std::io::Result<()> {
    for stream in listener.incoming() {
        let stream = stream?;
        if let Ok(peer) = stream.peer_addr() {
            on_client(&peer);
        }
        let slice = slice.clone();
        std::thread::spawn(move || {
            if let Err(e) = handle(stream, &slice, threads) {
                // A client that hangs up mid-request is ordinary, not an
                // error worth a line: the main device finished with this
                // worker, or moved on after a timeout.
                if !matches!(&e, wire::Error::Io(io)
                    if io.kind() == std::io::ErrorKind::UnexpectedEof
                        || io.kind() == std::io::ErrorKind::ConnectionReset
                        || io.kind() == std::io::ErrorKind::ConnectionAborted)
                {
                    eprintln!("worker: {e}");
                }
            }
        });
    }
    Ok(())
}

/// One connection.
pub fn handle(stream: TcpStream, slice: &Slice, threads: usize) -> wire::Result<()> {
    // **Nagle off.** Every exchange here is a small write followed by a wait
    // for the reply, which is precisely the shape Nagle delays — up to 40 ms
    // per layer, 43 layers a token. It would have looked like a slow worker.
    stream.set_nodelay(true)?;
    let mut r = BufReader::new(stream.try_clone()?);
    let mut w = BufWriter::new(stream);

    loop {
        let (kind, body) = match wire::read_frame(&mut r) {
            Ok(v) => v,
            Err(wire::Error::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(())
            }
            Err(e) => return Err(e),
        };

        match kind {
            Kind::Hello => {
                let held = Held {
                    model: slice.model.clone(),
                    layers: slice.layers.clone(),
                    experts: slice.held.clone(),
                    bytes: slice.bytes,
                    width: slice.width,
                };
                wire::write_frame(&mut w, Kind::Held, &held.encode())?;
            }
            Kind::Compute => {
                let req = match Compute::decode(&body) {
                    Ok(v) => v,
                    Err(e) => {
                        wire::write_frame(&mut w, Kind::Failed, format!("{e}").as_bytes())?;
                        continue;
                    }
                };
                match slice.compute(
                    req.layer,
                    &req.jobs,
                    req.tokens,
                    req.width,
                    &req.hidden,
                    threads,
                ) {
                    Ok(values) => {
                        let ans = Activations {
                            width: req.width,
                            jobs: req.jobs.len() as u32,
                            values,
                        };
                        wire::write_frame(&mut w, Kind::Activations, &ans.encode())?;
                    }
                    Err(e) => {
                        // **A frame, not a closed socket.** The main device's
                        // answer to this is to read the expert from its own
                        // disk; it should not have to reconnect to ask the
                        // next question.
                        wire::write_frame(&mut w, Kind::Failed, format!("{e}").as_bytes())?;
                    }
                }
            }
            other => {
                wire::write_frame(
                    &mut w,
                    Kind::Failed,
                    format!("a worker does not answer {other:?}").as_bytes(),
                )?;
            }
        }
    }
}

/// The main device's end of one worker.
pub struct Client {
    r: BufReader<TcpStream>,
    w: BufWriter<TcpStream>,
    pub held: Held,
    pub timing: Timing,
}

impl Client {
    /// Connect and ask what the worker holds.
    pub fn connect(addr: &str) -> wire::Result<Client> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        let mut r = BufReader::new(stream.try_clone()?);
        let mut w = BufWriter::new(stream);
        wire::write_frame(&mut w, Kind::Hello, &[])?;
        let (kind, body) = wire::read_frame(&mut r)?;
        let held = match kind {
            Kind::Held => Held::decode(&body)?,
            Kind::Failed => return Err(wire::Error::Remote(String::from_utf8_lossy(&body).into())),
            other => return Err(wire::Error::Remote(format!("expected Held, got {other:?}"))),
        };
        Ok(Client {
            r,
            w,
            held,
            timing: Timing::default(),
        })
    }

    /// Whether this worker can answer for an expert at all.
    ///
    /// Checked before sending, because a request a worker cannot serve costs a
    /// full round trip to be told so — and the fallback (read it locally) is
    /// available without asking.
    pub fn holds(&self, layer: u32, expert: u32) -> bool {
        self.held.layers.contains(&layer) && self.held.experts.binary_search(&expert).is_ok()
    }

    /// Send one request and wait for the answer.
    pub fn compute(&mut self, req: &Compute) -> wire::Result<Activations> {
        let payload = req.encode();
        self.timing.requests += 1;
        self.timing.jobs += req.jobs.len() as u64;
        self.timing.bytes_in += (payload.len() + wire::HEADER) as u64;

        wire::write_frame(&mut self.w, Kind::Compute, &payload)?;
        let (kind, body) = wire::read_frame(&mut self.r)?;
        self.timing.bytes_out += (body.len() + wire::HEADER) as u64;
        match kind {
            Kind::Activations => Activations::decode(&body),
            Kind::Failed => Err(wire::Error::Remote(String::from_utf8_lossy(&body).into())),
            other => Err(wire::Error::Remote(format!(
                "expected Activations, got {other:?}"
            ))),
        }
    }
}
