//! Input normalization: one pointer sample type for the mouse, a pen, and touch. The front
//! end converts its own events (egui events in the desktop app) into these, so the ink engine
//! never sees a UI type.

use crate::geometry::Vec2;

/// Where a pointer sample came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerKind {
    Mouse,
    /// A pen or stylus. Usually reports pressure.
    Pen,
    Touch,
}

/// Where a sample sits in a gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerPhase {
    /// The button went down, the pen touched, or the finger landed.
    Down,
    /// The pointer moved while down.
    Move,
    /// The button went up, the pen lifted, or the finger left.
    Up,
    /// The system canceled the gesture (palm rejection, focus loss). The stroke is dropped.
    Cancel,
}

/// One position of a pointer, in page units, at a time, with pressure if the device reports
/// it. Build one with [`PointerSample::new`], which refuses values that are not finite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerSample {
    /// Page position (96 per inch, origin at the top left of the page, y down).
    pub pos: Vec2,
    /// Seconds from any fixed origin; only differences matter.
    pub time: f64,
    /// Normalized pressure, 0 to 1, or `None` when the device or the OS does not report it
    /// (a mouse, and many pens on macOS and Linux).
    pub pressure: Option<f32>,
    pub kind: PointerKind,
}

impl PointerSample {
    /// A sample, or `None` when the position or time is NaN or infinite. Pressure outside 0
    /// to 1 is clamped; a NaN pressure becomes `None`.
    pub fn new(pos: Vec2, time: f64, pressure: Option<f32>, kind: PointerKind) -> Option<Self> {
        if !pos.is_finite() || !time.is_finite() {
            return None;
        }
        let pressure = pressure.filter(|p| !p.is_nan()).map(|p| p.clamp(0.0, 1.0));
        Some(Self {
            pos,
            time,
            pressure,
            kind,
        })
    }
}

/// A sample with its place in the gesture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    pub phase: PointerPhase,
    pub sample: PointerSample,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_values_that_are_not_finite() {
        let ok = Vec2::new(1.0, 2.0);
        assert!(PointerSample::new(ok, 0.0, None, PointerKind::Mouse).is_some());
        let nan = Vec2::new(f32::NAN, 0.0);
        assert!(PointerSample::new(nan, 0.0, None, PointerKind::Mouse).is_none());
        assert!(PointerSample::new(ok, f64::INFINITY, None, PointerKind::Pen).is_none());
    }

    #[test]
    fn pressure_is_clamped_or_dropped() {
        let at = |p| PointerSample::new(Vec2::ZERO, 0.0, p, PointerKind::Pen).map(|s| s.pressure);
        assert_eq!(at(Some(1.7)), Some(Some(1.0)));
        assert_eq!(at(Some(-0.2)), Some(Some(0.0)));
        assert_eq!(at(Some(f32::NAN)), Some(None));
        assert_eq!(at(None), Some(None));
        assert_eq!(at(Some(0.25)), Some(Some(0.25)));
    }
}
