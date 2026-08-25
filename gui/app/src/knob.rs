//! The mode knob, rendered per pixel.
//!
//! # Why per pixel and not GDI shapes
//!
//! `assets/knob.svg` is the specification, and what makes it read as
//! three-dimensional is **gradients**: the chamfer ring where the face turns
//! over into the skirt, and one specular arc. Plain GDI has no gradient fill,
//! and approximating one with sixty concentric ellipses bands visibly at the
//! size this is drawn. So the knob is scan-converted here, the same way
//! [`crate::art::logo_coverage`] scan-converts the mark, and blitted once.
//!
//! # The one place the two-colour rule is suspended
//!
//! `art.rs` says the palette is `#000000` and `#FFFFFF`, nothing between, and
//! that is right for every other surface in this window. **A knob has to be
//! shaded or it is a circle.** Atur asked for "high detail and quality, and a
//! three-dimensional appearance" and approved the render; this module is the
//! exception, and it is the only one.
//!
//! # Light does not turn with the knob
//!
//! Turning a real knob does not move the window it sits near. So each sample
//! is shaded in **screen space** and its geometry — pointer, knurl — is looked
//! up in **knob space**, by rotating the sample backwards through the current
//! angle. Rotating everything together is the bug that makes a rendered knob
//! look like a printed picture being spun.

use crate::theme::Rgb;

/// Where the four detents sit, in degrees from twelve o'clock.
///
/// **A 180 degree sweep across the top, and it stops at both ends.** A control
/// that can spin forever has no first position and no last, so the detents stop
/// meaning anything; a stove knob travels an arc. Left to right is a ramp of
/// involvement, the way a stove goes from OFF to HIGH.
pub const DETENTS: [(f64, &str); 4] = [
    (-90.0, "ALONE"),
    (-30.0, "CLIENT"),
    (30.0, "HELPER"),
    (90.0, "CORE"),
];

/// The name shown on the dial, which is not the settings key.
///
/// `Role::as_str` is `"alone"` because that is what goes in `settings.txt`.
/// A dial is engraved in capitals.
pub fn title(role: crate::settings::Role) -> &'static str {
    let a = angle_of(role);
    for (d, name) in DETENTS {
        if (d - a).abs() < 1e-9 {
            return name;
        }
    }
    "ALONE"
}

/// The angle for a role, and the role for an angle.
pub fn angle_of(role: crate::settings::Role) -> f64 {
    match role {
        crate::settings::Role::Alone => -90.0,
        crate::settings::Role::Client => -30.0,
        crate::settings::Role::Helper => 30.0,
        crate::settings::Role::Core => 90.0,
    }
}

/// The nearest detent to an angle, which is what a drag snaps to.
pub fn role_at(angle: f64) -> crate::settings::Role {
    let mut best = crate::settings::Role::Alone;
    let mut d = f64::MAX;
    for (a, _) in DETENTS {
        if (angle - a).abs() < d {
            d = (angle - a).abs();
            best = match a as i32 {
                -90 => crate::settings::Role::Alone,
                -30 => crate::settings::Role::Client,
                30 => crate::settings::Role::Helper,
                _ => crate::settings::Role::Core,
            };
        }
    }
    best
}

type Stop = (f64, (f64, f64, f64));

/// Interpolate a stop list at `t`, exactly as SVG does: constant outside the
/// ends, linear between neighbours.
fn ramp(t: f64, stops: &[Stop]) -> (f64, f64, f64) {
    let t = t.clamp(0.0, 1.0);
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        let (a, ca) = w[0];
        let (b, cb) = w[1];
        if t <= b {
            let k = if (b - a).abs() < 1e-9 {
                0.0
            } else {
                (t - a) / (b - a)
            };
            return (
                ca.0 + (cb.0 - ca.0) * k,
                ca.1 + (cb.1 - ca.1) * k,
                ca.2 + (cb.2 - ca.2) * k,
            );
        }
    }
    stops[stops.len() - 1].1
}

const fn c(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    (r as f64, g as f64, b as f64)
}

/// The face: light from the upper left, falling off to a warm grey rather than
/// to black, because white plastic in a lit room never goes black.
const BODY: [Stop; 5] = [
    (0.00, c(0xff, 0xff, 0xff)),
    (0.40, c(0xf8, 0xf8, 0xf6)),
    (0.70, c(0xe7, 0xe7, 0xe3)),
    (0.89, c(0xd2, 0xd2, 0xcd)),
    (1.00, c(0xbc, 0xbc, 0xb7)),
];

const SKIRT: [Stop; 4] = [
    (0.00, c(0xed, 0xed, 0xea)),
    (0.55, c(0xdc, 0xdc, 0xd7)),
    (0.85, c(0xb6, 0xb6, 0xb0)),
    (1.00, c(0x96, 0x96, 0x8f)),
];

/// The chamfer. This one ring is most of what makes it read as 3-D, which is
/// why the whole module exists.
const BEZEL: [Stop; 4] = [
    (0.00, c(0xff, 0xff, 0xff)),
    (0.30, c(0xfb, 0xfb, 0xf9)),
    (0.62, c(0xcd, 0xcd, 0xc7)),
    (1.00, c(0xa4, 0xa4, 0x9e)),
];

/// Radii as fractions of the knob's own radius, from `assets/knob.svg` where
/// the body is r=206 in a 256 half-extent.
const R_SKIRT: f64 = 1.000;
const R_BEZEL: f64 = 0.835; // 172/206
const R_FACE: f64 = 0.777; // 160/206
const R_SCRIBE: f64 = 0.660; // 136/206
const R_COLLAR: f64 = 0.466; // 96/206
const R_BOSS: f64 = 0.427; // 88/206
const KNURL_IN: f64 = 0.845;
const KNURL_OUT: f64 = 0.980;
const KNURL_N: f64 = 48.0;
const PTR_IN: f64 = 0.505;
const PTR_OUT: f64 = 0.786;
const PTR_HALF: f64 = 0.038;

/// Render the knob into a bottom-up BGRA buffer of `size` by `size`.
///
/// `angle` is degrees clockwise from twelve o'clock. `bg` is what shows through
/// outside the body, so the caller's page colour is what the shadow falls on.
pub fn render(size: usize, angle: f64, bg: Rgb, logo: &[u8], logo_px: usize) -> Vec<u8> {
    let size = size.max(8);
    let n = size as f64;
    let half = n / 2.0;
    // The body leaves room for the shadow, which is offset down.
    let radius = half * 0.90;
    let (sa, ca) = (-angle.to_radians()).sin_cos();

    let bgc = c(
        ((bg >> 16) & 0xFF) as u8,
        ((bg >> 8) & 0xFF) as u8,
        (bg & 0xFF) as u8,
    );

    let mut px = vec![0u8; size * size * 4];
    // 3x3 supersampling. The zone edges below are hard, so this is the only
    // source of antialiasing and three levels per axis is enough at the sizes
    // a window actually draws: the mark's own rasteriser needed 8x8 only
    // because it has one-pixel rays.
    const SS: usize = 3;
    for y in 0..size {
        for x in 0..size {
            let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
            for sy in 0..SS {
                for sx in 0..SS {
                    let fx = x as f64 + (sx as f64 + 0.5) / SS as f64;
                    let fy = y as f64 + (sy as f64 + 0.5) / SS as f64;
                    let s = sample(fx, fy, half, radius, sa, ca, bgc, logo, logo_px, size);
                    r += s.0;
                    g += s.1;
                    b += s.2;
                }
            }
            let k = (SS * SS) as f64;
            // Bottom-up DIB, so the row is mirrored here rather than the image
            // being upside down.
            let i = ((size - 1 - y) * size + x) * 4;
            px[i] = (b / k).round().clamp(0.0, 255.0) as u8;
            px[i + 1] = (g / k).round().clamp(0.0, 255.0) as u8;
            px[i + 2] = (r / k).round().clamp(0.0, 255.0) as u8;
            px[i + 3] = 0;
        }
    }
    px
}

#[allow(clippy::too_many_arguments)]
fn sample(
    fx: f64,
    fy: f64,
    half: f64,
    radius: f64,
    sa: f64,
    ca: f64,
    bgc: (f64, f64, f64),
    logo: &[u8],
    logo_px: usize,
    _size: usize,
) -> (f64, f64, f64) {
    // Screen space, centred, in units of the body radius.
    let sx = (fx - half) / radius;
    let sy = (fy - half) / radius;
    let r = (sx * sx + sy * sy).sqrt();

    // Outside the body: the cast shadow, offset down to match the key light.
    if r > R_SKIRT {
        let dy = sy - 0.06;
        let d = (sx * sx + dy * dy).sqrt();
        let a = (1.0 - ((d - R_SKIRT) / 0.16).clamp(0.0, 1.0)).powf(1.6) * 0.34;
        return (bgc.0 * (1.0 - a), bgc.1 * (1.0 - a), bgc.2 * (1.0 - a));
    }

    // **Shading is screen space; geometry is knob space.** Turning the knob
    // must not turn the highlight, so the gradients below read `sx, sy` while
    // the pointer and the knurl read the back-rotated coordinates.
    let kx = sx * ca - sy * sa;
    let ky = sx * sa + sy * ca;

    // The gradient focus sits up and left of centre, which is the light.
    let gd = |cx: f64, cy: f64, gr: f64| {
        let (dx, dy) = (sx - cx, sy - cy);
        (dx * dx + dy * dy).sqrt() / gr
    };

    let mut col = if r > R_BEZEL {
        ramp(gd(-0.12, -0.24, 1.44), &SKIRT)
    } else if r > R_FACE {
        // The chamfer, lit along the upper-left to lower-right diagonal.
        let t = ((sx + sy) / 2.0 + 0.5).clamp(0.0, 1.0);
        ramp(t, &BEZEL)
    } else {
        ramp(gd(-0.24, -0.40, 1.48), &BODY)
    };

    // The knurl: 48 rounded ridges standing on the skirt.
    if r > KNURL_IN && r < KNURL_OUT {
        let ang = ky.atan2(kx);
        let phase = (ang / std::f64::consts::TAU * KNURL_N).fract();
        let phase = if phase < 0.0 { phase + 1.0 } else { phase };
        // A cosine across each ridge: bright on the leading side, dark on the
        // trailing one. Rounded, because a moulded grip has no sharp edges and
        // a sharp one aliases at these sizes.
        let s = (phase * std::f64::consts::TAU).cos();
        let edge =
            ((r - KNURL_IN) / 0.03).clamp(0.0, 1.0) * ((KNURL_OUT - r) / 0.03).clamp(0.0, 1.0);
        let k = s * 26.0 * edge;
        col = (col.0 + k, col.1 + k, col.2 + k);
    }

    // A faint scribed circle on the face, as moulded plastic has.
    if (r - R_SCRIBE).abs() < 0.006 {
        col = (col.0 - 8.0, col.1 - 8.0, col.2 - 8.0);
    }

    // The pointer: a recess, so a dark channel with a lit lower-right lip.
    if r > PTR_IN && r < PTR_OUT && ky < 0.0 && kx.abs() < PTR_HALF {
        let across = kx / PTR_HALF;
        let lip = ((across + 1.0) / 2.0).clamp(0.0, 1.0);
        let dark = 78.0 * (1.0 - lip * 0.55);
        col = (col.0 - dark, col.1 - dark, col.2 - dark + 4.0);
        if across > 0.45 {
            col = (col.0 + 46.0, col.1 + 46.0, col.2 + 46.0);
        }
    }

    // The collar around the badge, then the badge itself.
    if r < R_COLLAR && r > R_BOSS {
        let t = ((sx + sy) / 2.0 + 0.5).clamp(0.0, 1.0);
        col = ramp(t, &BEZEL);
    } else if r <= R_BOSS {
        col = (255.0, 255.0, 255.0);
        // **Atur's mark, scan-converted, never redrawn.** The badge turns with
        // the knob, so it is sampled in knob space like the pointer.
        if logo_px > 0 {
            let u = (kx / R_BOSS + 1.0) / 2.0;
            let v = (ky / R_BOSS + 1.0) / 2.0;
            let ix = (u * logo_px as f64) as isize;
            let iy = (v * logo_px as f64) as isize;
            if ix >= 0 && iy >= 0 && (ix as usize) < logo_px && (iy as usize) < logo_px {
                let a = f64::from(logo[iy as usize * logo_px + ix as usize]) / 255.0;
                col = (col.0 * (1.0 - a), col.1 * (1.0 - a), col.2 * (1.0 - a));
            }
        }
    }

    // One specular arc, upper left, stopping short of the rim so it reads as a
    // highlight on a curved surface rather than a painted stripe.
    if r < R_FACE {
        let d = ((sx + 0.42).powi(2) + (sy + 0.42).powi(2)).sqrt();
        let a = (1.0 - (d / 0.62).clamp(0.0, 1.0)).powf(2.2) * 46.0;
        col = (col.0 + a, col.1 + a, col.2 + a);
    }

    // The outer edge, darkened, so the knob sits on the page rather than
    // floating over it.
    if r > R_SKIRT - 0.012 {
        let k = ((r - (R_SKIRT - 0.012)) / 0.012).clamp(0.0, 1.0) * 34.0;
        col = (col.0 - k, col.1 - k, col.2 - k);
    }

    (
        col.0.clamp(0.0, 255.0),
        col.1.clamp(0.0, 255.0),
        col.2.clamp(0.0, 255.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_detents_span_the_top_half_and_stop() {
        let angles: Vec<f64> = DETENTS.iter().map(|d| d.0).collect();
        assert_eq!(angles, vec![-90.0, -30.0, 30.0, 90.0]);
        // 180 degrees end to end, not 360: a stove knob travels an arc.
        assert_eq!(angles[3] - angles[0], 180.0);
        // Evenly spaced, so a drag between two of them is symmetric.
        for w in angles.windows(2) {
            assert!((w[1] - w[0] - 60.0).abs() < 1e-9);
        }
    }

    #[test]
    fn every_role_has_an_angle_and_comes_back() {
        use crate::settings::Role;
        for role in [Role::Alone, Role::Client, Role::Helper, Role::Core] {
            assert_eq!(role_at(angle_of(role)), role, "{role:?} did not round-trip");
        }
    }

    #[test]
    fn a_drag_snaps_to_the_nearest_detent() {
        use crate::settings::Role;
        assert_eq!(role_at(-89.0), Role::Alone);
        assert_eq!(role_at(-61.0), Role::Alone);
        // Exactly between ALONE and CLIENT; the nearer one wins on either side.
        assert_eq!(role_at(-59.0), Role::Client);
        assert_eq!(role_at(88.0), Role::Core);
        // Past the stops, which a drag can reach before it is clamped.
        assert_eq!(role_at(-140.0), Role::Alone);
        assert_eq!(role_at(140.0), Role::Core);
    }

    #[test]
    fn every_role_has_an_engraved_name() {
        use crate::settings::Role;
        for role in [Role::Alone, Role::Client, Role::Helper, Role::Core] {
            let t = title(role);
            assert!(!t.is_empty());
            assert_eq!(t, t.to_uppercase(), "a dial is engraved in capitals");
            // The name must match the key, or the dial and the file disagree.
            assert_eq!(t.to_lowercase(), role.as_str());
        }
    }

    #[test]
    fn it_renders_a_round_knob_on_the_page_colour() {
        // No logo: this is about the body, and the mark has its own tests.
        let n = 64;
        let px = render(n, -90.0, 0x00FFFFFF, &[], 0);
        assert_eq!(px.len(), n * n * 4);
        let at = |x: usize, y: usize| {
            let i = ((n - 1 - y) * n + x) * 4;
            (px[i + 2], px[i + 1], px[i])
        };
        // The middle is the badge: white, because no mark was supplied.
        let (r, _, _) = at(n / 2, n / 2);
        assert!(r > 240, "the badge should be white, got {r}");
        // A corner is outside the body and keeps the page colour.
        let (cr, _, _) = at(1, 1);
        assert!(cr > 240, "the corner should be the page colour, got {cr}");
        // Somewhere on the skirt is grey: shaded, not flat.
        let (sr, _, _) = at(n / 2, 3);
        assert!(sr < 235, "the rim should be shaded, got {sr}");
    }

    #[test]
    fn the_mark_is_actually_in_the_middle() {
        // **This was never checked, and Atur asked "where is logo in center".**
        // The knob was measured for speed and driven for input; nobody looked
        // at its pixels. A badge that renders blank looks exactly like a badge
        // that renders white.
        let n = 220;
        let logo_px = ((n as f64) * 0.42) as usize;
        let logo = crate::art::logo_scaled(logo_px);
        assert!(
            logo.iter().any(|&a| a > 200),
            "the source coverage is empty, so nothing could be drawn"
        );
        let px = render(n, -90.0, 0x00FFFFFF, &logo, logo_px);
        let at = |x: usize, y: usize| {
            let i = ((n - 1 - y) * n + x) * 4;
            i32::from(px[i + 2])
        };
        // Count dark pixels inside the badge. The mark is black on white, so
        // a rendered badge has plenty and an empty one has none.
        let c = n / 2;
        let rad = (n as f64 * 0.427 * 0.90 / 2.0) as usize;
        let mut ink = 0;
        for y in (c - rad)..(c + rad) {
            for x in (c - rad)..(c + rad) {
                if at(x, y) < 100 {
                    ink += 1;
                }
            }
        }
        assert!(
            ink > 200,
            "the badge is blank: only {ink} dark pixels inside it"
        );
    }

    #[test]
    fn turning_the_knob_does_not_turn_the_light() {
        // The upper-left highlight is a fact about the room, not the control.
        // Render at two angles and check the lit side stays lit.
        let n = 48;
        let bright = |angle: f64| {
            let px = render(n, angle, 0x00FFFFFF, &[], 0);
            let at = |x: usize, y: usize| {
                let i = ((n - 1 - y) * n + x) * 4;
                i32::from(px[i + 2])
            };
            // Upper-left of the face against lower-right of it.
            at(n / 2 - 8, n / 2 - 8) - at(n / 2 + 8, n / 2 + 8)
        };
        let a = bright(-90.0);
        let b = bright(90.0);
        assert!(a > 0, "upper left should be the lit side, got {a}");
        assert!(b > 0, "still lit after turning, got {b}");
        assert!(
            (a - b).abs() < 24,
            "the highlight moved with the knob: {a} then {b}"
        );
    }
}
