//! Ink state in the session: which tool is active, the current pen, highlighter, and eraser,
//! and each page's strokes. Strokes live in memory for now; the `.notecraft` canvas block
//! stores them in PR 1.9 (the `Stroke` type already serializes to that block's `ink` entries).

use std::collections::BTreeMap;
use std::sync::Arc;

use notecraft_canvas::ink::InkLayer;
use notecraft_core::{InkColor, InkTexture};

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

/// How big the point eraser is (OneNote's Small, Medium, and Large erasers). The front end
/// maps a size to a radius in screen pixels; the stroke eraser has no size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EraserSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl EraserSize {
    pub fn id(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" => Some(Self::Large),
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
    /// The pencil or a textured pen; [`InkTexture::Plain`] for a plain pen and a highlighter.
    pub texture: InkTexture,
}

impl Default for PenStyle {
    fn default() -> Self {
        Self {
            color: InkColor::BLACK,
            width_pt: 1.0,
            texture: InkTexture::Plain,
        }
    }
}

impl PenStyle {
    /// A pen of `texture` with its starting color and `width_pt`.
    pub fn textured(texture: InkTexture, width_pt: f32) -> Self {
        Self {
            color: texture.swatch(),
            width_pt,
            texture,
        }
    }

    /// This style in `color`. A textured pen whose look is its own (not the pencil) becomes a
    /// plain pen in that color: picking a color is asking for a plain color.
    pub fn with_color(mut self, color: InkColor) -> Self {
        self.color = color;
        if !self.texture.uses_color() {
            self.texture = InkTexture::Plain;
        }
        self
    }

    /// The widest the stroke gets, in page units.
    pub fn width_units(self) -> f32 {
        self.width_pt * PAGE_UNITS_PER_POINT
    }
}

/// Yellow, 8 pt: the highlighter the Draw tab starts with.
const DEFAULT_HIGHLIGHTER: PenStyle = PenStyle {
    color: InkColor([0xFF, 0xF1, 0x76, 0xFF]),
    width_pt: 8.0,
    texture: InkTexture::Plain,
};

/// Most tools the Draw tab keeps. Past this, Add does nothing.
pub const MAX_PEN_TOOLS: usize = 24;

/// What a tool of the Draw tab's list draws with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ToolKind {
    #[default]
    Pen,
    Highlighter,
}

impl ToolKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Pen => "pen",
            Self::Highlighter => "highlighter",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "pen" => Some(Self::Pen),
            "highlighter" => Some(Self::Highlighter),
            _ => None,
        }
    }

    /// The pen or highlighter a new tool of this kind starts as.
    pub fn default_style(self) -> PenStyle {
        match self {
            Self::Pen => PenStyle {
                color: InkColor([0, 0, 0, 0xFF]),
                width_pt: 1.0,
                texture: InkTexture::Plain,
            },
            Self::Highlighter => DEFAULT_HIGHLIGHTER,
        }
    }
}

/// One pen or highlighter in the Draw tab's list: the user can add, change, and delete these,
/// and the front end keeps the list in the app settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenTool {
    pub kind: ToolKind,
    pub style: PenStyle,
}

impl PenTool {
    pub fn new(kind: ToolKind, color: InkColor, width_pt: f32) -> Self {
        Self {
            kind,
            style: PenStyle {
                color,
                width_pt,
                texture: InkTexture::Plain,
            },
        }
    }

    /// A pen of `texture` (the pencil or a textured pen) with its starting color.
    pub fn textured(texture: InkTexture, width_pt: f32) -> Self {
        Self {
            kind: ToolKind::Pen,
            style: PenStyle::textured(texture, width_pt),
        }
    }

    /// The tool as saved in the settings: kind id, `#RRGGBB[AA]` color, width in points, and
    /// the texture id (empty for a plain pen).
    pub fn to_saved(self) -> (&'static str, String, f32, &'static str) {
        (
            self.kind.id(),
            self.style.color.to_hex(),
            self.style.width_pt,
            self.style.texture.id(),
        )
    }

    /// A tool from its saved form. An unknown kind or a color that does not parse gives
    /// `None`; a width out of range or not a number is put in range; a texture that is not
    /// known (or sits on a highlighter) is dropped, leaving a plain pen in the saved color.
    pub fn from_saved(kind: &str, color: &str, width_pt: f32, texture: &str) -> Option<Self> {
        let kind = ToolKind::parse(kind)?;
        let color = InkColor::parse(color)?;
        let mut tool = Self::new(kind, color, width_pt).bounded();
        if kind == ToolKind::Pen {
            tool.style.texture = InkTexture::parse(texture).unwrap_or_default();
        }
        Some(tool)
    }

    /// The width, kept inside what the commands accept.
    fn bounded(mut self) -> Self {
        let w = self.style.width_pt;
        self.style.width_pt = if w.is_finite() {
            w.clamp(MIN_PEN_POINTS, MAX_PEN_POINTS)
        } else {
            PenStyle::default().width_pt
        };
        self
    }
}

/// The tools a fresh install starts with: black, red, blue, and green pens and a yellow
/// highlighter.
pub fn default_pen_tools() -> Vec<PenTool> {
    let pen = |hex: &str| {
        PenTool::new(
            ToolKind::Pen,
            InkColor::parse(hex).unwrap_or(InkColor::BLACK),
            1.0,
        )
    };
    vec![
        pen("#000000"),
        pen("#E81123"),
        pen("#3AA7E8"),
        pen("#3DAE49"),
        PenTool {
            kind: ToolKind::Highlighter,
            style: DEFAULT_HIGHLIGHTER,
        },
    ]
}

/// Everything ink in the session. Layers are shared copies (`Arc`) so the command guard's
/// snapshot of the session stays cheap for commands that do not touch ink.
#[derive(Clone, Debug, PartialEq)]
pub struct InkState {
    pub(crate) tool: DrawTool,
    pub(crate) pen: PenStyle,
    pub(crate) highlighter: PenStyle,
    pub(crate) eraser: EraserMode,
    pub(crate) eraser_size: EraserSize,
    /// Strokes by page id.
    pub(crate) layers: BTreeMap<String, Arc<InkLayer>>,
    /// The Draw tab's list of pens and highlighters.
    pub(crate) tools: Vec<PenTool>,
    /// The entry of `tools` in use, while the pen or highlighter is that entry's.
    pub(crate) selected: Option<usize>,
}

impl Default for InkState {
    fn default() -> Self {
        Self {
            tool: DrawTool::default(),
            pen: PenStyle::default(),
            highlighter: DEFAULT_HIGHLIGHTER,
            eraser: EraserMode::default(),
            eraser_size: EraserSize::default(),
            layers: BTreeMap::new(),
            tools: default_pen_tools(),
            selected: None,
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

    /// The point eraser's size.
    pub fn eraser_size(&self) -> EraserSize {
        self.eraser_size
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

    /// The Draw tab's pens and highlighters, in order.
    pub fn tools(&self) -> &[PenTool] {
        &self.tools
    }

    /// The index in [`tools`](Self::tools) of the tool in use, if one is.
    pub fn selected_tool(&self) -> Option<usize> {
        self.selected.filter(|i| *i < self.tools.len())
    }

    /// Replace the tool list (restoring saved settings). Entries past [`MAX_PEN_TOOLS`] are
    /// dropped and widths are put in range. The selection is cleared.
    pub(crate) fn replace_tools(&mut self, tools: impl IntoIterator<Item = PenTool>) {
        self.tools = tools
            .into_iter()
            .take(MAX_PEN_TOOLS)
            .map(PenTool::bounded)
            .collect();
        self.selected = None;
    }

    /// Copy the style of the selected tool from the pen or highlighter in use, keeping the
    /// list in step when the color or width changes.
    pub(crate) fn sync_selected(&mut self) {
        let style = self.brush();
        let kind = self.tool;
        if let Some(tool) = self.selected.and_then(|i| self.tools.get_mut(i)) {
            let same = matches!(
                (kind, tool.kind),
                (DrawTool::Pen, ToolKind::Pen) | (DrawTool::Highlighter, ToolKind::Highlighter)
            );
            if same {
                tool.style = style;
            }
        }
    }

    /// The strokes of page `page`, if it has any.
    pub fn layer(&self, page: &str) -> Option<&InkLayer> {
        self.layers.get(page).map(Arc::as_ref)
    }
}
