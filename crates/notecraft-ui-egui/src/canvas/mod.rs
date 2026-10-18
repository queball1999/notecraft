//! The page canvas: the page the user pans and zooms, below the section tabs. Phase 0 draws
//! the paper and the page title area; content and ink arrive in Phase 1.
//!
//! The zoom lives in the session (`view.zoom` and friends), so every zoom, including Ctrl+wheel
//! and pinch, is a command. The scroll position is front-end state kept here.

mod editing;
#[cfg(test)]
mod editing_tests;
mod flow;
#[cfg(test)]
mod formatting_tests;
mod ink;
#[cfg(test)]
mod ink_tests;
mod keymap;
mod multiply;
pub(crate) mod paper;
mod picture_ui;
pub(crate) mod pictures;
mod rawtext;
mod title;
mod widget;

use notecraft_canvas::viewport::{self, Viewport};
use notecraft_core::Vec2;

pub use multiply::install as install_multiply;
pub use widget::show;

/// The egui memory id of the flag that is true while the user types in the canvas.
pub(crate) fn typing_flag() -> egui::Id {
    egui::Id::new("notecraft.canvas.typing")
}

/// A page parsed for display and editing.
#[derive(Debug)]
pub(crate) struct Parsed {
    pub doc: notecraft_markdown::Document,
    pub runs: Vec<notecraft_markdown::ContainerRun>,
    /// The page text without markdown syntax, for screen readers.
    pub plain: String,
}

/// Canvas state kept between frames.
#[derive(Debug)]
pub struct CanvasUi {
    viewport: Viewport,
    /// The session zoom last applied to `viewport`, in percent.
    seen_percent: u16,
    /// Push the viewport origin into the scroll area on the next frame (after a zoom or a
    /// drag pan); otherwise the scroll area owns the scroll position.
    force_offset: bool,
    /// The parsed page shown now, with the page id and source revision it was parsed for.
    parsed: Option<(String, u64, std::sync::Arc<Parsed>)>,
    /// Pointer and keyboard editing state.
    edit: editing::EditUi,
    /// The stroke being drawn and the stroke mesh cache.
    ink: ink::InkUi,
    /// Textures of the pictures on the page shown.
    pictures: pictures::PictureCache,
    /// The page title being edited in place.
    title: Option<title::TitleEdit>,
}

impl Default for CanvasUi {
    fn default() -> Self {
        Self {
            viewport: Viewport::default(),
            seen_percent: 100,
            force_offset: false,
            parsed: None,
            edit: editing::EditUi::default(),
            ink: ink::InkUi::default(),
            pictures: pictures::PictureCache::default(),
            title: None,
        }
    }
}

impl CanvasUi {
    /// Pictures decoded and pictures that could not be shown, on the page shown (tests).
    #[cfg(test)]
    pub(crate) fn picture_counts(&self) -> (usize, usize) {
        self.pictures.counts()
    }

    /// The parsed document for `source`, parsing again only when the page or its text changed.
    fn document(&mut self, source: notecraft_engine::PageSource<'_>) -> std::sync::Arc<Parsed> {
        let kept = self
            .parsed
            .take()
            .filter(|(id, rev, _)| id == source.page_id && *rev == source.revision);
        let entry = kept.unwrap_or_else(|| {
            let doc = notecraft_markdown::parse(source.visible_text());
            let runs = notecraft_markdown::split_containers(source.visible_text(), &doc);
            let plain = doc.plain_text();
            let parsed = Parsed { doc, runs, plain };
            (
                source.page_id.to_owned(),
                source.revision,
                std::sync::Arc::new(parsed),
            )
        });
        self.parsed.insert(entry).2.clone()
    }

    /// The plain text of the page shown, for screen readers.
    fn reader_text(&self) -> &str {
        self.parsed.as_ref().map_or("", |(_, _, p)| &p.plain)
    }

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
