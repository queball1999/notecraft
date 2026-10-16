//! Ink state in the session: which tool is active, the current pen, highlighter, and eraser,
//! and each page's strokes. Strokes live in memory for now; the `.notecraft` canvas block
//! stores them in PR 1.9 (the `Stroke` type already serializes to that block's `ink` entries).

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
    /// Drawing with the highlighter.
    Highlighter,
    /// Erasing; [`EraserMode`] says whole strokes or parts of them.
    Eraser,
}

impl DrawTool {
    pub fn id(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Pen => "pen",
            Self::Highlighter => "highlighter",
            Self::Eraser => "eraser",
        }
    }
}

/// What the eraser removes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EraserMode {
    /// A whole stroke the eraser touches.
    #[default]
    Stroke,
    /// Only the part under the eraser; the rest of the stroke stays as separate strokes.
    Point,
}

impl EraserMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Stroke => "stroke",
            Self::Point => "point",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "stroke" => Some(Self::Stroke),
            "point" => Some(Self::Point),
            _ => None,
        }
    }
}

/// The pen or highlighter the Draw tab has selected.
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

/// Yellow, 8 pt: the highlighter the Draw tab starts with.
const DEFAULT_HIGHLIGHTER: PenStyle = PenStyle {
    color: InkColor([0xFF, 0xF1, 0x76, 0xFF]),
    width_pt: 8.0,
};

/// Everything ink in the session. Layers are shared copies (`Arc`) so the command guard's
/// snapshot of the session stays cheap for commands that do not touch ink.
#[derive(Clone, Debug, PartialEq)]
pub struct InkState {
    pub(crate) tool: DrawTool,
    pub(crate) pen: PenStyle,
    pub(crate) highlighter: PenStyle,
    pub(crate) eraser: EraserMode,
    /// Strokes by page id.
    pub(crate) layers: BTreeMap<String, Arc<InkLayer>>,
}

impl Default for InkState {
    fn default() -> Self {
        Self {
            tool: DrawTool::default(),
            pen: PenStyle::default(),
            highlighter: DEFAULT_HIGHLIGHTER,
            eraser: EraserMode::default(),
            layers: BTreeMap::new(),
        }
    }
}

impl InkState {
    pub fn tool(&self) -> DrawTool {
        self.tool
    }

    pub fn pen(&self) -> PenStyle {
        self.pen
    }

    pub fn highlighter(&self) -> PenStyle {
        self.highlighter
    }

    pub fn eraser(&self) -> EraserMode {
        self.eraser
    }

    /// The style `draw.color` and `draw.thickness` change: the highlighter's while it is the
    /// tool, the pen's otherwise.
    pub fn brush(&self) -> PenStyle {
        match self.tool {
            DrawTool::Highlighter => self.highlighter,
            _ => self.pen,
        }
    }

    pub(crate) fn brush_mut(&mut self) -> &mut PenStyle {
        match self.tool {
            DrawTool::Highlighter => &mut self.highlighter,
            _ => &mut self.pen,
        }
    }

    /// The strokes of page `page`, if it has any.
    pub fn layer(&self, page: &str) -> Option<&InkLayer> {
        self.layers.get(page).map(Arc::as_ref)
    }
}
