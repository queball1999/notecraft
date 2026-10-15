//! Property tests for the viewport: thousands of random viewports and points, from a fixed
//! seed so failures reproduce. A tiny xorshift generator keeps this free of dependencies.

use notecraft_canvas::viewport::{self, Viewport, MAX_ZOOM, MIN_ZOOM};
use notecraft_core::Vec2;

const CASES: usize = 5_000;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A float in `lo..hi`.
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        let unit = (self.next() >> 40) as f32 / (1u64 << 24) as f32;
        lo + (hi - lo) * unit
    }

    fn point(&mut self, extent: f32) -> Vec2 {
        Vec2::new(self.range(-extent, extent), self.range(-extent, extent))
    }

    fn viewport(&mut self) -> Viewport {
        Viewport::new(self.point(20_000.0), self.range(MIN_ZOOM, MAX_ZOOM))
    }

    /// Any f32 bit pattern, including NaN and infinities.
    fn any_f32(&mut self) -> f32 {
        f32::from_bits((self.next() >> 32) as u32)
    }
}

/// Close enough for f32 at the magnitudes used: relative error, with a floor for values
/// near zero.
fn close(a: Vec2, b: Vec2, scale: f32) -> bool {
    let tol = 1e-4 * scale.max(1.0);
    (a.x - b.x).abs() <= tol && (a.y - b.y).abs() <= tol
}

#[test]
fn screen_page_screen_round_trips() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..CASES {
        let v = rng.viewport();
        let screen = rng.point(4_000.0);
        let back = v.page_to_screen(v.screen_to_page(screen));
        assert!(close(screen, back, 25_000.0 * v.zoom()), "{v:?} {screen:?}");
    }
}

#[test]
fn page_screen_page_round_trips() {
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    for _ in 0..CASES {
        let v = rng.viewport();
        let page = rng.point(20_000.0);
        let back = v.screen_to_page(v.page_to_screen(page));
        assert!(close(page, back, 40_000.0), "{v:?} {page:?}");
    }
}

#[test]
fn zoom_about_keeps_the_anchor_on_the_same_page_point() {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for _ in 0..CASES {
        let mut v = rng.viewport();
        let anchor = rng.point(2_000.0);
        let before = v.screen_to_page(anchor);
        v.zoom_about(anchor, rng.range(0.01, 6.0));
        assert!((MIN_ZOOM..=MAX_ZOOM).contains(&v.zoom()));
        let after = v.screen_to_page(anchor);
        assert!(close(before, after, 40_000.0), "{v:?} {anchor:?}");
    }
}

#[test]
fn pan_then_reverse_pan_returns_home() {
    let mut rng = Rng(0x6A09_E667_F3BC_C908);
    for _ in 0..CASES {
        let mut v = rng.viewport();
        let start = v.origin();
        let delta = rng.point(3_000.0);
        v.pan_by(delta);
        v.pan_by(-delta);
        assert!(close(start, v.origin(), 40_000.0), "{v:?} {delta:?}");
    }
}

#[test]
fn hostile_values_never_escape_the_limits() {
    let mut rng = Rng(0xBB67_AE85_84CA_A73B);
    for _ in 0..CASES {
        let mut v = Viewport::new(Vec2::new(rng.any_f32(), rng.any_f32()), rng.any_f32());
        v.pan_by(Vec2::new(rng.any_f32(), rng.any_f32()));
        v.zoom_about(Vec2::new(rng.any_f32(), rng.any_f32()), rng.any_f32());
        let origin = v.origin();
        assert!(origin.is_finite(), "{v:?}");
        assert!(origin.x.abs() <= viewport::MAX_COORD && origin.y.abs() <= viewport::MAX_COORD);
        assert!((MIN_ZOOM..=MAX_ZOOM).contains(&v.zoom()), "{v:?}");
        let (min, max) = v.visible_page(Vec2::new(rng.any_f32(), rng.any_f32()));
        assert!(min.is_finite() && max.is_finite(), "{v:?}");
        assert!((10..=400).contains(&v.percent()));
    }
}

#[test]
fn zoom_steps_are_monotonic_and_bounded() {
    for percent in 0..=500u16 {
        let up = viewport::zoom_in_step(percent);
        let down = viewport::zoom_out_step(percent);
        assert!((10..=400).contains(&up) && (10..=400).contains(&down));
        assert!(up > percent || up == 400, "{percent} -> {up}");
        assert!(down < percent || down == 10, "{percent} -> {down}");
    }
}
