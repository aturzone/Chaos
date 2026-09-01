//! How much RAM to spend on caching routed experts.
//!
//! Residency policy, which is what this crate is for: the always-read weights and
//! the expert cache compete for one pool, and both this rule and the ceiling it
//! respects came out of one measured curve. `cli/run` calls it; `--auto` should,
//! once it runs on more than the dense path.

/// Where the expert-cache curve stops paying, on the only machine it has been
/// swept on. Past roughly this size the only measurement in the repository is
/// that a 71%-hit cache was the *slowest* configuration tried.
pub const EXPERT_CACHE_CEILING: u64 = 6 << 30;

/// How much expert cache to take when the user has not said, and the always-read
/// set already fits.
///
/// **The default was zero, and zero costs 1.20x on DeepSeek-V4-Flash.** Measured
/// 2026-09-01 on this laptop (15.7 GiB, trunk 7.38 GiB), sixteen generated
/// tokens, three alternating pairs at the peak:
///
/// ```text
///   cache      tok/s    expert-read hits
///   off        0.603    --
///   1 GiB      0.649    14.2%
///   2 GiB      0.695    21.3%
///   3 GiB      0.721    26.8%     <- peak
///   4 GiB      0.620
///   5 GiB      0.505    33.1%
///   6 GiB      0.352
/// ```
///
/// **The hit rate keeps climbing while the speed collapses**, which is the whole
/// shape of the problem: every cached byte is one the OS cannot use, and past the
/// peak the memory pressure costs more than the hits pay for. A user who read the
/// old message -- *"`--cache <GiB>` is now worth measuring"*, with no bound -- and
/// guessed 5 or 6 got a change **1.2x to 1.7x slower** than leaving it off.
///
/// # The rule, and what it rests on
///
/// `total - trunk - RESERVE`, floored at zero and capped at the plateau. Total RAM
/// rather than *free* RAM on purpose: free RAM is measured before the trunk loads
/// and drifts with whatever else the machine is doing, so sizing a long-lived
/// allocation from it is how `--auto` came to want 4.9 GiB here -- a value the
/// curve above measures at **0.505, worse than off**.
///
/// `RESERVE` is 5 GiB: the OS, the KV cache, the per-block arenas, and enough page
/// cache left for the streaming reads not to starve. On this machine the rule
/// picks 15.7 - 7.38 - 5 = **3.3 GiB**, which is the measured peak.
///
/// **One machine's curve.** The shape -- rise, peak, collapse -- is a
/// memory-pressure argument that should hold anywhere; the *position* of the peak
/// is this laptop's, and `RESERVE` is fitted to it. On a machine with more RAM the
/// rule gives more cache, which is the right direction, but the plateau has not
/// been measured there. `--cache N` overrides, and `--cache 0` turns it off.
pub fn expert_cache_bytes(total_ram: u64, resident_bytes: u64) -> u64 {
    /// The OS, the KV cache, the arenas, and page cache for the streaming reads.
    const RESERVE: u64 = 5 << 30;
    total_ram
        .saturating_sub(resident_bytes)
        .saturating_sub(RESERVE)
        .min(EXPERT_CACHE_CEILING)
}

#[cfg(test)]
mod tests {
    use super::{expert_cache_bytes, EXPERT_CACHE_CEILING};

    const GIB: u64 = 1 << 30;

    /// This laptop, and the value the curve says is right.
    ///
    /// 15.7 GiB of RAM, a 7.38 GiB always-read set, 5 GiB reserved: **3.34 GiB**,
    /// measured at 0.724 tok/s against 0.602 with no cache. Pinned because the
    /// whole change is this arithmetic, and a reserve edited without re-measuring
    /// would move the machine off a peak that collapses on the far side -- 5 GiB
    /// of cache measures **slower than none at all**.
    #[test]
    fn this_machine_gets_the_measured_peak() {
        // **GiB, not GB.** The first version of this test wrote 15.7e9 for
        // "15.7 GiB" and expected the answer to come out at the measured peak.
        // It came out at 2.21 GiB and the test failed -- correctly, and on the
        // test rather than on the rule.
        let total = 15_667 * GIB / 1000; // 15.667 GiB, what the probe reports here
        let trunk = 7_380 * GIB / 1000; // 7.38 GiB resident
        let got = expert_cache_bytes(total, trunk);
        let gib = got as f64 / GIB as f64;
        assert!(
            (3.0..3.7).contains(&gib),
            "expected the ~3.3 GiB peak, got {gib:.2} GiB"
        );
    }

    /// **A machine with no room gets no cache, not a negative one.**
    ///
    /// The subtraction is saturating for a reason: a 16 GiB machine holding a
    /// 12 GiB always-read set has nothing to spare, and an underflow here would
    /// wrap to sixteen exabytes and ask the allocator for it.
    #[test]
    fn a_full_machine_gets_nothing() {
        assert_eq!(expert_cache_bytes(16 * GIB, 12 * GIB), 0);
        assert_eq!(expert_cache_bytes(16 * GIB, 20 * GIB), 0);
        assert_eq!(expert_cache_bytes(0, 0), 0);
    }

    /// The plateau caps it, however much RAM there is.
    ///
    /// Past ~6 GiB the only measurement in the repository is that a 71%-hit cache
    /// was the *slowest* configuration tried, so a 128 GiB machine does not get
    /// 100 GiB of expert cache on the strength of arithmetic nobody has run.
    #[test]
    fn a_large_machine_is_capped_at_the_plateau() {
        assert_eq!(expert_cache_bytes(128 * GIB, 8 * GIB), EXPERT_CACHE_CEILING);
        assert_eq!(expert_cache_bytes(64 * GIB, 8 * GIB), EXPERT_CACHE_CEILING);
    }

    /// More resident weights means less cache, monotonically.
    ///
    /// The two compete for one pool, and the ordering is the only property of
    /// this rule that is safe to assume on a machine nobody has measured.
    #[test]
    fn a_bigger_trunk_leaves_a_smaller_cache() {
        let total = 32 * GIB;
        let mut last = u64::MAX;
        for trunk_gib in [4, 8, 12, 16, 20, 24, 28] {
            let got = expert_cache_bytes(total, trunk_gib * GIB);
            assert!(
                got <= last,
                "cache grew as the trunk grew, at {trunk_gib} GiB"
            );
            last = got;
        }
        assert_eq!(last, 0, "a 28 GiB trunk on a 32 GiB machine leaves nothing");
    }
}
