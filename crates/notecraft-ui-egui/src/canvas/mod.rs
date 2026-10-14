//! The page canvas: the page the user pans and zooms, below the section tabs. Phase 0 draws
//! an empty page; content and ink arrive in Phase 1.
//!
//! The zoom lives in the session (`view.zoom` and friends), so every zoom, including Ctrl+wheel
//! and pinch, is a command. The scroll position is front-end state kept here.

mod paper;
mod widget;

use notecraft_canvas::viewport::{self, Viewport};
use notecraft_core::Vec2;

pub use widget::show;

/// Canvas state kept between frames.
#[derive(Debug)]
pub struct CanvasUi {
    viewport: Viewport,
    /// The session zoom last applied to `viewport`, in percent.
    seen_percent: u16,
    /// Push the viewport origin into the scroll area on the next frame (after a zoom or a
    /// drag pan); otherwise the scroll area owns the scroll position.
    force_offset: bool,
}

impl Default for CanvasUi {
    fn default() -> Self {
        Self {
            viewport: Viewport::default(),
            seen_percent: 100,
            force_offset: false,
        }
    }
}

impl CanvasUi {
    /// Where the canvas looks now.
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Follow a zoom set by a command. A command zoom keeps the top-left corner of the
    /// canvas on the same page point, so Page Width at the left edge fills the width.
    fn follow_session(&mut self, percent: u16) {
        if percent == self.seen_percent {
            return;
        }
        self.seen_percent = percent;
        if self.viewport.percent() != percent {
            self.viewport
                .zoom_about(Vec2::ZERO, f32::from(percent) / 100.0);
            self.force_offset = true;
        }
    }

    /// Zoom by `factor` about `anchor` (canvas screen point), as Ctrl+wheel or a pinch does.
    /// Returns the new percent when it differs from the session's, for `view.zoom`. A factor
    /// that is not finite and positive is ignored.
    fn zoom_gesture(&mut self, anchor: Vec2, factor: f32) -> Option<u16> {
        if !(factor.is_finite() && factor > 0.0) {
            return None;
        }
        let zoom = self.viewport.zoom() * factor;
        self.viewport.zoom_about(anchor, viewport::clamp_zoom(zoom));
        self.force_offset = true;
        let percent = self.viewport.percent();
        (percent != self.seen_percent).then(|| {
            self.seen_percent = percent;
            percent
        })
    }
}

/// Where page units land on screen this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PageToScreen {
    /// Screen position of page point (0, 0).
    pub origin: egui::Pos2,
    pub zoom: f32,
}

impl PageToScreen {
    pub fn pos(self, page: Vec2) -> egui::Pos2 {
        self.origin + egui::vec2(page.x, page.y) * self.zoom
    }

    pub fn page(self, screen: egui::Pos2) -> Vec2 {
        let v = (screen - self.origin) / self.zoom;
        Vec2::new(v.x, v.y)
    }

    /// The page area inside `screen`, as top-left and bottom-right page points.
    pub fn page_rect(self, screen: egui::Rect) -> (Vec2, Vec2) {
        (self.page(screen.min), self.page(screen.max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_zoom_is_followed_once() {
        let mut canvas = CanvasUi::default();
        canvas.follow_session(200);
        assert_eq!(canvas.viewport().percent(), 200);
        assert!(canvas.force_offset);
        canvas.force_offset = false;
        canvas.follow_session(200);
        assert!(!canvas.force_offset);
    }

    #[test]
    fn gestures_report_only_percent_changes() {
        let mut canvas = CanvasUi::default();
        assert_eq!(canvas.zoom_gesture(Vec2::new(50.0, 50.0), 1.5), Some(150));
        // A tiny pinch that rounds to the same percent is not sent again.
        assert_eq!(canvas.zoom_gesture(Vec2::new(50.0, 50.0), 1.001), None);
        // The session then reports what the gesture sent: nothing to redo.
        canvas.follow_session(150);
        assert_eq!(canvas.zoom_gesture(Vec2::ZERO, 100.0), Some(400));
        assert_eq!(canvas.zoom_gesture(Vec2::ZERO, 0.001), Some(10));
        assert_eq!(canvas.zoom_gesture(Vec2::ZERO, f32::NAN), None);
        assert_eq!(canvas.zoom_gesture(Vec2::ZERO, -2.0), None);
        assert_eq!(canvas.viewport().percent(), 10);
    }

    #[test]
    fn page_to_screen_round_trips() {
        let map = PageToScreen {
            origin: egui::pos2(100.0, 40.0),
            zoom: 2.0,
        };
        let page = Vec2::new(12.0, 30.0);
        assert_eq!(map.pos(page), egui::pos2(124.0, 100.0));
        assert_eq!(map.page(map.pos(page)), page);
    }
}
