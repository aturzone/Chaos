//! A machine that holds expert weights and answers with activations.
//!
//! **Read `docs/graph/backlog/devices-as-resources.md` before changing
//! anything here.** The arithmetic in that node was done before this code
//! existed, because the obvious version of "use the other machines" loses:
//! serving expert weights over a 125 MB/s link is *twenty times slower* than
//! the NVMe this project already streams them from. What wins is the opposite
//! direction — send the 16 KB hidden state to whichever machine already holds
//! the weights in RAM, and get 16 KB back.
//!
//! | | per token, V4-Flash |
//! |---|---|
//! | a hidden state | **16 KB** |
//! | a token's expert weights | **3.3 GB** |
//!
//! # What is here, and what is deliberately not
//!
//! The plan's own order is: protocol, then a worker that computes, then
//! **measure and stop**. Discovery, assignment from the probe, and
//! tensor-parallel come after a number says they are worth it.

/// Accepting connections, and the main device's end of one.
pub mod serve;
pub mod slice;
pub mod wire;
