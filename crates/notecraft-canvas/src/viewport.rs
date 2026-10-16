//! The canvas viewport: which part of the page is on screen and how large it is drawn.
//!
//! Page units are 1/96 inch, so at 100% zoom one page unit is one screen point. Screen
//! coordinates here are relative to the top-left corner of the canvas area, not the window;
//! the front end adds the canvas position.

use notecraft_core::Vec2;

/// Smallest zoom, 10%, as in OneNote.
pub const MIN_ZOOM: f32 = 0.1;
/// Largest zoom, 400%, as in OneNote.
pub const MAX_ZOOM: f32 = 4.0;
/// [`MIN_ZOOM`] in percent.
pub const MIN_PERCENT: u16 = 10;
/// [`MAX_ZOOM`] in percent.
pub const MAX_PERCENT: u16 = 400;
/// The levels Zoom In and Zoom Out step through, in percent.
pub const ZOOM_STEPS: [u16; 10] = [10, 25, 50, 75, 100, 125, 150, 200, 300, 400];
/// Page units per inch.
pub const UNITS_PER_INCH: f32 = 96.0;
/// Furthest the viewport origin may be from the page origin, in page units (about 1.6 km).
/// Keeps every coordinate well inside the range where `f32` is precise.
pub const MAX_COORD: f32 = 6_000_000.0;

/// `zoom` clamped to [`MIN_ZOOM`, `MAX_ZOOM`]; NaN or infinity becomes 100%.
pub fn clamp_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() {
        zoom.clamp(MIN_ZOOM, MAX_ZOOM)
    } else {
        1.0
    }
}

/// `percent` clamped to [`MIN_PERCENT`, `MAX_PERCENT`].
pub fn clamp_percent(percent: i64) -> u16 {
    let clamped = percent.clamp(i64::from(MIN_PERCENT), i64::from(MAX_PERCENT));
    u16::try_from(clamped).unwrap_or(100)
}

/// A zoom factor as a whole percentage, rounded and clamped.
pub fn zoom_to_percent(zoom: f32) -> u16 {
    // clamp_zoom keeps the value in 0.1..=4.0, so the cast cannot overflow.
    let percent = (clamp_zoom(zoom) * 100.0).round() as i64;
    clamp_percent(percent)
}

/// The next level up from `percent` in [`ZOOM_STEPS`], or the largest zoom.
pub fn zoom_in_step(percent: u16) -> u16 {
    ZOOM_STEPS
        .into_iter()
        .find(|&step| step > percent)
        .unwrap_or(MAX_PERCENT)
}

/// The next level down from `percent` in [`ZOOM_STEPS`], or the smallest zoom.
pub fn zoom_out_step(percent: u16) -> u16 {
    ZOOM_STEPS
        .into_iter()
        .rev()
        .find(|&step| step < percent)
        .unwrap_or(MIN_PERCENT)
}

/// The zoom, in percent, at which `page_width` page units fill `view_width` screen points.
/// Rounded down so the page never overflows; clamped to the zoom limits.
pub fn fit_width_percent(page_width: f32, view_width: f32) -> u16 {
    if !(page_width > 0.0 && view_width > 0.0 && page_width.is_finite()) {
        return 100;
    }
    let zoom = clamp_zoom(view_width / page_width);
    clamp_percent((zoom * 100.0).floor() as i64)
}

/// Where the canvas looks: the page point drawn at the top-left corner of the canvas, and
/// the zoom factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    origin: Vec2,
    zoom: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            origin: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl Viewport {
    /// A viewport with `origin` at the top-left corner, at `zoom`. Bad values are clamped.
    pub fn new(origin: Vec2, zoom: f32) -> Self {
        let mut viewport = Self {
            origin: Vec2::ZERO,
            zoom: clamp_zoom(zoom),
        };
        viewport.set_origin(origin);
        viewport
    }

    /// The page point at the top-left corner of the canvas.
    pub fn origin(&self) -> Vec2 {
        self.origin
    }

    /// The zoom factor: 1.0 is 100%.
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// The zoom as a whole percentage.
    pub fn percent(&self) -> u16 {
        zoom_to_percent(self.zoom)
    }

    /// Move so `origin` is at the top-left corner. Non-finite values become zero and every
    /// coordinate is kept within [`MAX_COORD`].
    pub fn set_origin(&mut self, origin: Vec2) {
        let origin = origin.finite_or_zero();
        self.origin = Vec2::new(
            origin.x.clamp(-MAX_COORD, MAX_COORD),
            origin.y.clamp(-MAX_COORD, MAX_COORD),
        );
    }

    /// Where page point `page` is drawn.
    pub fn page_to_screen(&self, page: Vec2) -> Vec2 {
        (page - self.origin) * self.zoom
    }

    /// Which page point is drawn at `screen`.
    pub fn screen_to_page(&self, screen: Vec2) -> Vec2 {
        screen / self.zoom + self.origin
    }

    /// Move the page by `delta` screen points, as dragging it does: dragging right shows
    /// more of the left side.
    pub fn pan_by(&mut self, delta: Vec2) {
        if delta.is_finite() {
            self.set_origin(self.origin - delta / self.zoom);
        }
    }

    /// Change the zoom while the page point under `anchor` (a screen point) stays put.
    pub fn zoom_about(&mut self, anchor: Vec2, zoom: f32) {
        let anchor = anchor.finite_or_zero();
        let page = self.screen_to_page(anchor);
        self.zoom = clamp_zoom(zoom);
        self.set_origin(page - anchor / self.zoom);
    }

    /// The page area on screen for a canvas of `size` screen points: top-left and
    /// bottom-right corners, in page units. Sizes are clamped to [0, `MAX_COORD`].
    pub fn visible_page(&self, size: Vec2) -> (Vec2, Vec2) {
        let size = size.finite_or_zero();
        let size = Vec2::new(size.x.clamp(0.0, MAX_COORD), size.y.clamp(0.0, MAX_COORD));
        (self.origin, self.screen_to_page(size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_100_percent_at_the_page_origin() {
        let v = Viewport::default();
        assert_eq!(v.zoom(), 1.0);
        assert_eq!(v.percent(), 100);
        assert_eq!(
            v.page_to_screen(Vec2::new(10.0, 20.0)),
            Vec2::new(10.0, 20.0)
        );
    }

    #[test]
    fn zoom_is_clamped_to_10_and_400_percent() {
        assert_eq!(Viewport::new(Vec2::ZERO, 0.01).percent(), 10);
        assert_eq!(Viewport::new(Vec2::ZERO, 99.0).percent(), 400);
        assert_eq!(Viewport::new(Vec2::ZERO, f32::NAN).zoom(), 1.0);
        assert_eq!(Viewport::new(Vec2::ZERO, f32::NEG_INFINITY).zoom(), 1.0);
        assert_eq!(clamp_percent(-5), 10);
        assert_eq!(clamp_percent(i64::MAX), 400);
        assert_eq!(zoom_to_percent(1.234), 123);
    }

    #[test]
    fn transforms_use_origin_and_zoom() {
        let v = Viewport::new(Vec2::new(100.0, 50.0), 2.0);
        assert_eq!(
            v.page_to_screen(Vec2::new(110.0, 60.0)),
            Vec2::new(20.0, 20.0)
        );
        assert_eq!(
            v.screen_to_page(Vec2::new(20.0, 20.0)),
            Vec2::new(110.0, 60.0)
        );
        let (min, max) = v.visible_page(Vec2::new(200.0, 100.0));
        assert_eq!(min, Vec2::new(100.0, 50.0));
        assert_eq!(max, Vec2::new(200.0, 100.0));
    }

    #[test]
    fn panning_moves_the_origin_against_the_drag() {
        let mut v = Viewport::new(Vec2::new(100.0, 100.0), 2.0);
        v.pan_by(Vec2::new(20.0, -40.0));
        assert_eq!(v.origin(), Vec2::new(90.0, 120.0));
        v.pan_by(Vec2::new(f32::NAN, 1.0));
        assert_eq!(v.origin(), Vec2::new(90.0, 120.0));
    }

    #[test]
    fn zoom_about_keeps_the_anchor_still() {
        let mut v = Viewport::new(Vec2::new(30.0, 40.0), 1.0);
        let anchor = Vec2::new(200.0, 150.0);
        let before = v.screen_to_page(anchor);
        v.zoom_about(anchor, 2.5);
        assert_eq!(v.zoom(), 2.5);
        let after = v.screen_to_page(anchor);
        assert!((before - after).x.abs() < 1e-3 && (before - after).y.abs() < 1e-3);
    }

    #[test]
    fn origin_stays_finite_and_bounded() {
        let mut v = Viewport::new(Vec2::new(f32::INFINITY, -1e30), 1.0);
        assert_eq!(v.origin(), Vec2::new(0.0, -MAX_COORD));
        v.pan_by(Vec2::new(-1e30, 0.0));
        assert_eq!(v.origin().x, MAX_COORD);
    }

    #[test]
    fn zoom_steps_go_up_and_down() {
        assert_eq!(zoom_in_step(100), 125);
        assert_eq!(zoom_in_step(110), 125);
        assert_eq!(zoom_in_step(400), 400);
        assert_eq!(zoom_out_step(100), 75);
        assert_eq!(zoom_out_step(90), 75);
        assert_eq!(zoom_out_step(10), 10);
        assert_eq!(zoom_out_step(0), 10);
    }

    #[test]
    fn fit_width_rounds_down_and_clamps() {
        assert_eq!(fit_width_percent(816.0, 1020.0), 125);
        assert_eq!(fit_width_percent(816.0, 1000.0), 122);
        assert_eq!(fit_width_percent(816.0, 10.0), 10);
        assert_eq!(fit_width_percent(816.0, 1e9), 400);
        assert_eq!(fit_width_percent(0.0, 500.0), 100);
        assert_eq!(fit_width_percent(f32::NAN, 500.0), 100);
        assert_eq!(fit_width_percent(816.0, -3.0), 100);
    }
}
