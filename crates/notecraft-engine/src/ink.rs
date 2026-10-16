//! Ink state in the session: which tool is active, the current pen, and each page's strokes.
//! Strokes live in memory for now; the `.notecraft` canvas block stores them in PR 1.9 (the
//! `Stroke` type already serializes to that block's `ink` entries).

use std::collections::BTreeMap;
use std::sync::Arc;

use notecraft_canvas::ink::InkLayer;
use notecraft_core::InkColor;

/// Page units per point: the page is 96 per inch, a point is 1/72 inch.
pub const PAGE_UNITS_PER_POINT: f32 = 96.0 / 72.0;
/// Thinnest and thickest pen the commands accept, in points.
pub const MIN_PEN_POINTS: f32 = 0.25;
pub const MAX_PEN_POINTS: f32 = 72.0;

/// What dragging on the canvas does. Ids are the values `draw.*` commands report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DrawTool {
    /// Typing and selecting (the default).
    #[default]
    Type,
    /// Drawing with the pen.
    Pen,
}

impl DrawTool {
    pub fn id(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Pen => "pen",
        }
    }
}

/// The pen the Draw tab has selected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenStyle {
    pub color: InkColor,
    /// Width in points, as the ribbon shows it.
    pub width_pt: f32,
}

impl Default for PenStyle {
    fn default() -> Self {
        Self {
            color: InkColor::BLACK,
            width_pt: 1.0,
        }
    }
}

impl PenStyle {
    /// The widest the stroke gets, in page units.
    pub fn width_units(self) -> f32 {
        self.width_pt * PAGE_UNITS_PER_POINT
    }
}

/// Everything ink in the session. Layers are shared copies (`Arc`) so the command guard's
/// snapshot of the session stays cheap for commands that do not touch ink.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InkState {
    pub(crate) tool: DrawTool,
    pub(crate) pen: PenStyle,
    /// Strokes by page id.
    pub(crate) layers: BTreeMap<String, Arc<InkLayer>>,
}

impl InkState {
    pub fn tool(&self) -> DrawTool {
        self.tool
    }

    pub fn pen(&self) -> PenStyle {
        self.pen
    }

    /// The strokes of page `page`, if it has any.
    pub fn layer(&self, page: &str) -> Option<&InkLayer> {
        self.layers.get(page).map(Arc::as_ref)
    }
}
