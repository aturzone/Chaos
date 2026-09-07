//! Whether a laid-out page is actually usable: nothing off the edge, nothing
//! on top of anything else, nothing too small to hit.
//!
//! # Why this is a module and not a script
//!
//! Atur, 2026-09-07: *"I want you to check the app pixel by pixel yourself"*.
//! Three attempts to do that from outside the process produced three sets of
//! confident, wrong numbers, and the script was deleted rather than kept. The
//! causes are in `reference/hard-won-facts.md`; the decisive one is that
//! `powershell.exe` is DPI-unaware, so Windows virtualises every coordinate it
//! reads back from an aware window — a 32-pixel button comes back as 26 and
//! every derived conclusion is wrong by 25%.
//!
//! So the check lives here: a pure function over rectangles, with no window,
//! no marshalling and no display scale involved. `layout` hands it the list it
//! is about to apply and the window's own client rect, both in design units.
//! It runs in CI over synthetic pages and in the app over real ones.
//!
//! # What it deliberately does not check
//!
//! Whether the *text* fits. A label that overflows its control is a real
//! defect and this cannot see it: the width of a string depends on the font
//! Windows picked, which needs a device context. `DT_END_ELLIPSIS` is the
//! standing answer — every string drawn through `label` is truncated with an
//! ellipsis rather than clipped mid-glyph — so an overflow degrades visibly
//! rather than looking like a bug.

/// A control, where `layout` decided to put it. Design units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// How tall the control actually *appears*.
    ///
    /// **This is the field that stops the check from crying wolf.** A combo
    /// box is sized by its dropped height — `layout` asks for
    /// `CONTROL + COMBO_ROW * 4` because Windows gives the closed box only
    /// what its item height needs, and a single-row request produces a list
    /// with nothing in it. So the rectangle handed to `MoveWindow` is three
    /// times the height of the thing on screen, and comparing *that* against
    /// its neighbours reports an overlap under every dropdown in the app.
    pub visible_h: i32,
    /// Whether this control belongs to the shell rather than to a page.
    ///
    /// **The strip along the bottom is reserved from pages and is home to the
    /// STOP button.** The first run of this check reported that button off the
    /// bottom edge on all six pages, which is the check being wrong rather
    /// than the layout: `page_rect` stops short of the strip precisely so a
    /// page cannot draw into it, and the strip's own control is entitled to
    /// the space. The rail buttons are chrome for the same reason.
    pub chrome: bool,
}

impl Placed {
    /// A page control whose whole rectangle is visible, which is most of them.
    pub fn new(id: i32, x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            id,
            x,
            y,
            w,
            h,
            visible_h: h,
            chrome: false,
        }
    }

    /// A control belonging to the shell -- the rail, or the strip -- which may
    /// use the space pages are kept out of.
    pub fn chrome(id: i32, x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            chrome: true,
            ..Self::new(id, x, y, w, h)
        }
    }

    /// A dropdown: the rectangle is its dropped extent, `visible` its closed
    /// height.
    pub fn dropdown(id: i32, x: i32, y: i32, w: i32, h: i32, visible: i32) -> Self {
        Self {
            id,
            x,
            y,
            w,
            h,
            visible_h: visible,
            chrome: false,
        }
    }

    fn visible_box(self) -> (i32, i32, i32, i32) {
        (self.x, self.y, self.x + self.w, self.y + self.visible_h)
    }
}

/// Something wrong with a page's geometry, in the words the fix needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// Off the edge of the window, or under the strip, or behind the rail.
    Outside { id: i32, edge: &'static str },
    /// Too small to read or to hit. 16x16 is the smallest target any of these
    /// pages intends; anything under it is a layout arithmetic mistake, not a
    /// design decision.
    TooSmall { id: i32, w: i32, h: i32 },
    /// Two controls on the same page, in the same place.
    ///
    /// **The defect this module was written for.** The mode knob was painted
    /// underneath nine live controls at startup for two releases: every one of
    /// them was created, positioned, given a font and wired to an action, and
    /// none of them could be pressed.
    Overlap { a: i32, b: i32 },
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::Outside { id, edge } => write!(f, "control {id} runs past the {edge} edge"),
            Problem::TooSmall { id, w, h } => write!(f, "control {id} is only {w}x{h}"),
            Problem::Overlap { a, b } => write!(f, "controls {a} and {b} overlap"),
        }
    }
}

/// The smallest rectangle that counts as a control rather than a mistake.
const MIN_TARGET: i32 = 16;

/// Check one page's placements against the area they have to live in.
///
/// `client` is `(left, top, right, bottom)` in design units — the window's
/// client rect, which is the only input all the geometry derives from.
/// `reserved_bottom` is the strip along the bottom, which is painted rather
/// than laid out and would otherwise look like empty space a control may use.
pub fn problems(
    client: (i32, i32, i32, i32),
    reserved_bottom: i32,
    placed: &[Placed],
) -> Vec<Problem> {
    let (cl, ct, cr, cb) = client;
    let floor = cb - reserved_bottom;
    let mut out = Vec::new();

    for p in placed {
        if p.w < MIN_TARGET || p.visible_h < MIN_TARGET {
            out.push(Problem::TooSmall {
                id: p.id,
                w: p.w,
                h: p.visible_h,
            });
        }
        let (l, t, r, _) = p.visible_box();
        // The bottom is checked against the visible height, not the dropped
        // one: a dropdown near the strip is *meant* to open over it.
        let b = t + p.visible_h;
        // Chrome owns the strip; a page does not.
        let bottom = if p.chrome { cb } else { floor };
        for (over, edge) in [
            (l < cl, "left"),
            (t < ct, "top"),
            (r > cr, "right"),
            (b > bottom, "bottom"),
        ] {
            if over {
                out.push(Problem::Outside { id: p.id, edge });
            }
        }
    }

    for (i, a) in placed.iter().enumerate() {
        for b in &placed[i + 1..] {
            if overlaps(a.visible_box(), b.visible_box()) {
                out.push(Problem::Overlap { a: a.id, b: b.id });
            }
        }
    }
    out
}

/// Whether two half-open rectangles share any area. Touching edges do not
/// count: two controls flush against each other are the intended design.
fn overlaps(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENT: (i32, i32, i32, i32) = (0, 0, 1180, 780);
    const STRIP: i32 = 52;

    #[test]
    fn a_clean_page_has_nothing_to_say() {
        let page = [
            Placed::new(1, 28, 104, 300, 32),
            Placed::new(2, 328, 104, 300, 32),
            Placed::new(3, 28, 150, 600, 400),
        ];
        assert_eq!(problems(CLIENT, STRIP, &page), Vec::new());
    }

    /// Flush is not overlapping. Half the rows in this app are laid out edge
    /// to edge and a check that flagged them would be useless.
    #[test]
    fn touching_is_allowed() {
        let page = [
            Placed::new(1, 28, 104, 100, 32),
            Placed::new(2, 128, 104, 100, 32),
            Placed::new(3, 28, 136, 100, 32),
        ];
        assert_eq!(problems(CLIENT, STRIP, &page), Vec::new());
    }

    /// The defect this exists for: a control painted over live ones.
    #[test]
    fn a_control_on_top_of_others_is_reported() {
        let page = [
            Placed::new(1, 28, 104, 200, 32),
            Placed::new(2, 28, 140, 200, 32),
            // A panel over both of them.
            Placed::new(99, 0, 90, 1180, 200),
        ];
        let found = problems(CLIENT, STRIP, &page);
        assert!(
            found.contains(&Problem::Overlap { a: 1, b: 99 }),
            "{found:?}"
        );
        assert!(
            found.contains(&Problem::Overlap { a: 2, b: 99 }),
            "{found:?}"
        );
    }

    /// A dropdown is sized by its dropped height, so the raw rectangle
    /// overlaps whatever is below it by design. Judging it by that rectangle
    /// reports a problem under every dropdown in the app — which is exactly
    /// how a well-meaning check becomes noise nobody reads.
    #[test]
    fn a_dropdowns_dropped_height_is_not_an_overlap() {
        // 30 tall closed, 134 dropped — CONTROL + COMBO_ROW * 4.
        let combo = Placed::dropdown(1, 28, 104, 240, 134, 30);
        let under = Placed::new(2, 28, 140, 240, 30);
        assert_eq!(problems(CLIENT, STRIP, &[combo, under]), Vec::new());

        // And judged by the dropped rectangle it would have been flagged,
        // which is the point of the field.
        let naive = Placed::new(1, 28, 104, 240, 134);
        assert!(problems(CLIENT, STRIP, &[naive, under]).contains(&Problem::Overlap { a: 1, b: 2 }));
    }

    /// Each of the four edges, including the strip — which is painted, not
    /// laid out, so a control that ignores it looks like it is floating over
    /// the status line.
    #[test]
    fn every_edge_is_checked() {
        for (p, edge) in [
            (Placed::new(1, -4, 104, 200, 32), "left"),
            (Placed::new(2, 28, -1, 200, 32), "top"),
            (Placed::new(3, 1100, 104, 200, 32), "right"),
            (Placed::new(4, 28, 700, 200, 32), "bottom"),
        ] {
            let found = problems(CLIENT, STRIP, &[p]);
            assert!(
                found.contains(&Problem::Outside { id: p.id, edge }),
                "{edge}: {found:?}"
            );
        }
    }

    /// A dropdown *may* open over the strip: that is what a dropdown near the
    /// bottom of a window does everywhere else on the desktop.
    #[test]
    fn a_dropdown_may_open_over_the_strip() {
        let combo = Placed::dropdown(1, 28, 690, 240, 134, 30);
        assert_eq!(problems(CLIENT, STRIP, &[combo]), Vec::new());
    }

    /// **The false positive the first real run produced**, kept as a test.
    /// The STOP button sits inside the strip on every page -- that is where
    /// the strip's controls go -- and a check that cannot tell chrome from
    /// page content reports it six times and teaches the reader to skip the
    /// output.
    #[test]
    fn the_strips_own_button_is_not_off_the_bottom() {
        // 405, exactly where `layout` puts it in a 780-tall window.
        let stop = Placed::chrome(405, 1054, 780 - STRIP + 10, 84, 32);
        assert_eq!(problems(CLIENT, STRIP, &[stop]), Vec::new());

        // A *page* control in the same place is still wrong, which is the
        // half of the distinction that has to keep working.
        let page = Placed::new(101, 1054, 780 - STRIP + 10, 84, 32);
        assert!(
            problems(CLIENT, STRIP, &[page]).contains(&Problem::Outside {
                id: 101,
                edge: "bottom"
            })
        );
    }

    /// Chrome is not exempt from the *window*, only from the strip.
    #[test]
    fn chrome_still_has_to_be_in_the_window() {
        let over = Placed::chrome(405, 1054, 770, 84, 32);
        assert!(
            problems(CLIENT, STRIP, &[over]).contains(&Problem::Outside {
                id: 405,
                edge: "bottom"
            })
        );
    }

    /// A zero or negative extent means the arithmetic that produced it
    /// underflowed, which in this layout happens when a window is narrower
    /// than a row of fixed-width controls.
    #[test]
    fn a_collapsed_control_is_reported() {
        let found = problems(CLIENT, STRIP, &[Placed::new(7, 28, 104, 0, 32)]);
        assert!(
            found.contains(&Problem::TooSmall { id: 7, w: 0, h: 32 }),
            "{found:?}"
        );
    }

    /// It reads as a sentence, because the app writes these into a file a
    /// person then looks at.
    #[test]
    fn a_problem_says_what_it_is() {
        assert_eq!(
            Problem::Overlap { a: 704, b: 760 }.to_string(),
            "controls 704 and 760 overlap"
        );
        assert_eq!(
            Problem::Outside {
                id: 12,
                edge: "bottom"
            }
            .to_string(),
            "control 12 runs past the bottom edge"
        );
    }
}
