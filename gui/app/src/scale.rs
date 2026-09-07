//! Display scaling: the one conversion between design units and pixels.
//!
//! # Why this module exists
//!
//! The window asks Windows for `PER_MONITOR_AWARE_V2` before any window is
//! created, correctly and early. **Declaring awareness moves the
//! responsibility to the application; it does not discharge it.** For eight
//! releases nothing then scaled: `theme::metric` is eleven raw `i32`
//! constants and `theme::size` six raw font heights, all used as physical
//! pixels. On a 125% display — this laptop, and most laptops — every control
//! and every glyph was drawn 20% smaller than designed, 33% smaller at 150%
//! and half size at 200%. Every other window on the desktop scaled; this one
//! did not, and that difference is most of what "looks unprofessional" means.
//!
//! Atur, 2026-09-07: *"make the appearance much more professional"*, and
//! *"I want you to check the app pixel by pixel yourself"*. This is the pixel
//! answer, and it is one defect rather than a hundred.
//!
//! # Why the conversion lives here rather than at 150 call sites
//!
//! The obvious fix — turn every `metric::X` into `metric::x()` and multiply —
//! touches every call site, every literal offset and every paint function, and
//! a single missed one puts a painted background 20% away from the buttons it
//! is meant to sit behind. That is worse than the bug.
//!
//! All geometry in the window derives from **one** input, the client rect, and
//! reaches the screen through **one** of a handful of exits: `MoveWindow`, the
//! four drawing helpers, `StretchDIBits`, and `CreateFontW`. So the window
//! computes entirely in *design units* — the numbers already written in
//! `theme::metric`, meaningful at 96 DPI — and converts only at those exits.
//! Nothing in between changes, which is why the literals scattered through
//! `layout()` are still correct.
//!
//! # Rounding
//!
//! Half away from zero, and **negative inputs matter**: `CreateFontW` takes a
//! negative height to mean "character height, not cell height", so `size::BODY`
//! is `-15` and must scale to `-19` rather than to `-18` or `0`.
//!
//! [`Scale::rect`] additionally guarantees that a rectangle with positive
//! width or height keeps it. A one-unit hairline — the only divider this
//! design has — must not round away to nothing at any scale.

/// The DPI at which every constant in `theme` is written.
pub const DESIGN_DPI: i32 = 96;

/// A display scale, held as its DPI so the arithmetic stays in integers.
///
/// `Copy` and two words wide: it is passed by value into paint helpers that
/// run hundreds of times per frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scale {
    dpi: i32,
}

impl Default for Scale {
    /// 96 DPI — the identity, and what every conversion falls back to when
    /// Windows cannot be asked.
    fn default() -> Self {
        Self { dpi: DESIGN_DPI }
    }
}

impl Scale {
    /// From a DPI as Windows reports it.
    ///
    /// Clamped to a sane range: `GetDpiForWindow` returns 0 for a window that
    /// is not yet real, and a 0 here would collapse the whole interface to a
    /// point. 480 is 500%, past the largest scale Windows offers.
    pub fn from_dpi(dpi: u32) -> Self {
        let dpi = (dpi as i32).clamp(DESIGN_DPI / 2, 480);
        Self { dpi }
    }

    /// The DPI this scale represents.
    pub fn dpi(self) -> i32 {
        self.dpi
    }

    /// Whether this is the identity, in which case every conversion is a
    /// no-op and the interface is what it always was.
    pub fn is_identity(self) -> bool {
        self.dpi == DESIGN_DPI
    }

    /// Design units to physical pixels.
    pub fn px(self, design: i32) -> i32 {
        scaled(design, self.dpi, DESIGN_DPI)
    }

    /// Physical pixels to design units.
    ///
    /// Needed wherever a measurement comes *back* from Windows in device
    /// units and is then compared against a constant written in design units:
    /// the client rect, `DRAWITEMSTRUCT::rcItem`, and the width of a string
    /// measured with a physical font.
    pub fn du(self, physical: i32) -> i32 {
        scaled(physical, DESIGN_DPI, self.dpi)
    }

    /// A rectangle, edge by edge, into physical pixels.
    ///
    /// **Edges, not origin-plus-size.** Converting a width independently of
    /// the `x` it sits at lets two controls that were flush in design space
    /// land a pixel apart, and a row of them accumulates the error. Scaling
    /// both edges and subtracting keeps adjacent things adjacent.
    ///
    /// A rectangle with positive extent keeps positive extent: a hairline is
    /// one design unit tall and must survive every scale.
    pub fn rect(self, r: (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
        let (left, top, right, bottom) = r;
        let (l, t) = (self.px(left), self.px(top));
        let (mut rr, mut bb) = (self.px(right), self.px(bottom));
        if right > left && rr <= l {
            rr = l + 1;
        }
        if bottom > top && bb <= t {
            bb = t + 1;
        }
        (l, t, rr, bb)
    }

    /// A rectangle back into design units.
    pub fn rect_du(self, r: (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
        (self.du(r.0), self.du(r.1), self.du(r.2), self.du(r.3))
    }
}

/// `v * num / den`, rounded half away from zero.
///
/// `i64` throughout: a client rect edge times 480 overflows nothing, but a
/// stray large value times a DPI in `i32` would, and a silently wrapped
/// coordinate is a control at a negative position rather than an error.
fn scaled(v: i32, num: i32, den: i32) -> i32 {
    let (v, num, den) = (i64::from(v), i64::from(num), i64::from(den));
    let half = den / 2;
    let r = if v >= 0 {
        (v * num + half) / den
    } else {
        -((-v * num + half) / den)
    };
    r.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scales Windows actually offers, and what a 32-unit button becomes
    /// at each. These are the numbers the bug was reported against.
    #[test]
    fn the_windows_scales() {
        for (dpi, button, body) in [
            (96, 32, -15),
            (120, 40, -19),
            (144, 48, -23),
            (168, 56, -26),
            (192, 64, -30),
        ] {
            let s = Scale::from_dpi(dpi);
            assert_eq!(s.px(32), button, "{dpi} dpi button");
            assert_eq!(s.px(-15), body, "{dpi} dpi body font");
        }
    }

    /// 96 DPI must change nothing at all, so a machine at 100% gets exactly
    /// the interface that was designed and reviewed.
    #[test]
    fn identity_is_exactly_identity() {
        let s = Scale::default();
        assert!(s.is_identity());
        for v in [-30, -15, 0, 1, 12, 28, 52, 208, 1180, 4096] {
            assert_eq!(s.px(v), v);
            assert_eq!(s.du(v), v);
        }
        assert_eq!(s.rect((10, 20, 30, 40)), (10, 20, 30, 40));
    }

    /// A font height is negative and its magnitude is what matters. Rounding
    /// toward zero here would make body text 18px at 125% instead of 19px,
    /// and rounding a small one to 0 asks `CreateFontW` for a default face.
    #[test]
    fn negative_heights_round_by_magnitude() {
        let s = Scale::from_dpi(120);
        assert_eq!(s.px(-15), -19); // 18.75
        assert_eq!(s.px(-12), -15); // 15.0
        assert_eq!(s.px(-13), -16); // 16.25
        assert_eq!(s.px(-19), -24); // 23.75
        assert_eq!(s.px(-30), -38); // 37.5, away from zero
        assert_eq!(s.px(15), 19);
    }

    /// The hairline. One design unit, at every scale, still a line.
    #[test]
    fn a_hairline_never_rounds_away() {
        for dpi in [96, 120, 144, 168, 192, 240, 288, 384, 480] {
            let s = Scale::from_dpi(dpi);
            let (l, t, r, b) = s.rect((100, 200, 400, 201));
            assert!(r > l, "{dpi} dpi lost the width");
            assert!(b > t, "{dpi} dpi lost the hairline: {t}..{b}");
        }
    }

    /// Two controls flush in design space stay flush in pixels. Converting
    /// `(x, w)` independently is what breaks this, which is why [`Scale::rect`]
    /// converts edges.
    #[test]
    fn adjacent_stays_adjacent() {
        for dpi in [120, 144, 168, 192] {
            let s = Scale::from_dpi(dpi);
            // A row of seven buttons of an awkward width, end to end.
            let mut x = 28;
            let mut prev_right = None;
            for _ in 0..7 {
                let (l, _, r, _) = s.rect((x, 0, x + 37, 30));
                if let Some(p) = prev_right {
                    assert_eq!(l, p, "{dpi} dpi left a gap at x={x}");
                }
                prev_right = Some(r);
                x += 37;
            }
        }
    }

    /// Both directions, because `du` is what interprets everything Windows
    /// hands back. A round trip may not be exact — 40 physical pixels is 32
    /// design units at 120 DPI, but 41 is also 33 — so the contract is that it
    /// never drifts by more than one unit.
    #[test]
    fn the_round_trip_never_drifts() {
        for dpi in [96, 120, 144, 168, 192, 240] {
            let s = Scale::from_dpi(dpi);
            for v in [0, 1, 12, 28, 30, 32, 52, 104, 208, 620, 940, 1180, 3840] {
                let back = s.du(s.px(v));
                assert!((back - v).abs() <= 1, "{dpi} dpi: {v} -> {back}");
            }
        }
    }

    /// A window that does not exist yet reports 0 DPI, and an unclamped 0
    /// would divide the whole interface to a point.
    #[test]
    fn an_impossible_dpi_cannot_collapse_the_window() {
        assert_eq!(Scale::from_dpi(0).px(208), 104);
        assert!(Scale::from_dpi(0).px(32) > 0);
        assert_eq!(Scale::from_dpi(100_000).dpi(), 480);
        assert!(Scale::from_dpi(u32::MAX).px(1180) > 0);
    }

    /// The rail, the strip and the opening window at 125% — the three numbers
    /// that decide whether the interface looks like its neighbours.
    #[test]
    fn the_shell_at_125_percent() {
        let s = Scale::from_dpi(120);
        assert_eq!(s.px(208), 260, "the rail");
        assert_eq!(s.px(52), 65, "the strip");
        assert_eq!(s.px(1180), 1475, "the opening width");
        assert_eq!(s.px(780), 975, "the opening height");
        // And a physical client rect read back into design units.
        assert_eq!(s.rect_du((0, 0, 1475, 975)), (0, 0, 1180, 780));
    }
}
