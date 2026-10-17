//! Flow layout for a markdown page (read-only): blocks stack top to bottom in one column, like
//! a document (docs/markdown.md section 3.5). `layout` turns a parsed page into drawing items in
//! page units; `paint` draws them. Colors come from theme tokens only.
//!
//! The seam for page text is one function: `layout(painter, &Document, zoom, tokens)`.

use std::ops::Range;
use std::sync::Arc;

use egui::epaint::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId, Galley, Painter, Pos2, Rect, Stroke};
use notecraft_canvas::text;
use notecraft_core::Vec2;
use notecraft_engine::TextCursor;
use notecraft_markdown::{
    markup, Align, Block, BlockKind, Cell, ContainerRun, Document, ListKind, Run, Script, Span,
    Table,
};
use notecraft_model::Container;

use super::rawtext::Raw;
use super::PageToScreen;
use crate::themes::{mix, ThemeTokens};

/// Left edge of the text column, in page units (matches the title area).
pub const LEFT: f32 = 48.0;
/// Top of the text, in page units: below the title, its line, and the date.
pub const TOP: f32 = 124.0;
/// Width of the text column, in page units (7.5 inches).
pub const COLUMN_WIDTH: f32 = 720.0;
/// Body text size in page units (11 pt).
pub const BODY_SIZE: f32 = 11.0 * 96.0 / 72.0;
/// Space kept below the last block, in page units.
const BOTTOM_MARGIN: f32 = 96.0;
/// Indent of nested lists and quotes, in page units.
const INDENT: f32 = 24.0;
/// Most drawing items made for one page; the rest of a huge page is left out.
const MAX_ITEMS: usize = 40_000;
/// Most table columns laid out.
const MAX_COLUMNS: usize = 64;

/// A drawing item. Positions are in page units, except galleys, which are laid out at
/// the zoom they were made for.
enum Item {
    Text {
        pos: Pos2,
        galley: Arc<Galley>,
        /// The bold parts again, offset by a hair: egui ships no bold face.
        bold: Option<Arc<Galley>>,
    },
    Fill {
        rect: Rect,
        color: Color32,
    },
    Line {
        from: Pos2,
        to: Pos2,
        color: Color32,
    },
    Dot {
        center: Pos2,
        radius: f32,
    },
    Check {
        rect: Rect,
        checked: bool,
    },
}

/// Gap between containers stacked in the flow, in page units.
const CONTAINER_GAP: f32 = BODY_SIZE * 0.8;
/// Most bytes of a raw region laid out; a bigger selection is not shown as raw text.
const MAX_RAW_BYTES: usize = 256 * 1024;

/// Everything the layout needs to know about the page.
pub(super) struct Input<'a> {
    pub text: &'a str,
    pub doc: &'a Document,
    pub runs: &'a [ContainerRun],
    /// Geometry of each run, in the same order as `runs`.
    pub geometry: &'a [Container],
    /// The caret and selection while the page is being edited.
    pub cursor: Option<TextCursor>,
    /// The IME composition shown at the caret.
    pub preedit: &'a str,
}

/// Where one container landed, in page units.
pub(super) struct ContainerBox {
    /// Index of the run in `Input::runs`.
    pub run: usize,
    pub rect: Rect,
}

/// Where one top-level block landed, in page units.
pub(super) struct BlockBox {
    pub run: usize,
    pub span: Span,
    pub rect: Rect,
}

/// A laid-out page.
pub(super) struct Flow {
    items: Vec<Item>,
    /// Bottom of the content, in page units.
    pub height: f32,
    /// Right edge of the content, in page units.
    pub width: f32,
    zoom: f32,
    pub containers: Vec<ContainerBox>,
    pub blocks: Vec<BlockBox>,
    pub raws: Vec<Raw>,
    /// The caret, in page units, when it is in one of the raw regions.
    pub caret: Option<Rect>,
}

struct Layout<'a> {
    painter: &'a Painter,
    tokens: &'a ThemeTokens,
    zoom: f32,
    items: Vec<Item>,
    blocks_out: Vec<BlockBox>,
    raws: Vec<Raw>,
    caret: Option<Rect>,
    /// An alignment comment was just read; it applies to the next paragraph or heading.
    pending_align: Option<egui::Align>,
    /// Alignment of the paragraph being laid out.
    align: egui::Align,
}

/// Lay out the page's containers at `zoom` (1.0 is 100%). Containers without a position stack
/// top to bottom (docs/markdown.md section 3.5); the others sit where they were put. The part
/// of a container holding the caret or selection shows as raw markdown.
pub(super) fn layout(
    painter: &Painter,
    input: &Input<'_>,
    zoom: f32,
    tokens: &ThemeTokens,
) -> Flow {
    let mut ctx = Layout {
        painter,
        tokens,
        zoom: if zoom.is_finite() && zoom > 0.0 {
            zoom
        } else {
            1.0
        },
        items: Vec::new(),
        blocks_out: Vec::new(),
        raws: Vec::new(),
        caret: None,
        pending_align: None,
        align: egui::Align::Min,
    };
    let mut y_flow = TOP;
    let mut bottom = TOP;
    let mut right = LEFT * 2.0 + COLUMN_WIDTH;
    let mut containers = Vec::new();
    for (index, run) in input.runs.iter().enumerate() {
        if ctx.full() {
            break;
        }
        let geometry = input
            .geometry
            .get(index)
            .cloned()
            .unwrap_or_else(|| Container::flow(&run.id));
        let (x, top) = match geometry.pos {
            Some(pos) => (pos.x, pos.y),
            None => (LEFT, y_flow),
        };
        let width = geometry.width;
        let mut y = top;
        ctx.run(input, index, run, x, width, &mut y);
        let height = (y - top).max(BODY_SIZE * 1.4);
        containers.push(ContainerBox {
            run: index,
            rect: Rect::from_min_size(Pos2::new(x, top), egui::vec2(width, height)),
        });
        if geometry.pos.is_none() {
            y_flow = top + height + CONTAINER_GAP;
        }
        bottom = bottom.max(top + height);
        right = right.max(x + width + LEFT);
    }
    Flow {
        height: bottom + BOTTOM_MARGIN,
        width: right,
        zoom: ctx.zoom,
        items: ctx.items,
        containers,
        blocks: ctx.blocks_out,
        raws: ctx.raws,
        caret: ctx.caret,
    }
}

/// Lay out raw markdown `slice`. `look` is the size in page units, whether it is monospace, and
/// the width; `composition` is where an IME composition sits (a byte offset in `slice`) and its
/// text. Returns the galley and the composition as (character index, characters).
fn raw_galley(
    painter: &Painter,
    tokens: &ThemeTokens,
    zoom: f32,
    slice: &str,
    look: (f32, bool, f32),
    composition: (Option<usize>, &str),
) -> (Arc<Galley>, Option<(usize, usize)>) {
    let (size, mono, width) = look;
    let (at, preedit_text) = composition;
    let family = if mono {
        FontFamily::Monospace
    } else {
        FontFamily::Proportional
    };
    let format = |underline: bool| TextFormat {
        font_id: FontId::new(size * zoom, family.clone()),
        color: tokens.text,
        underline: if underline {
            Stroke::new(1.0, tokens.text)
        } else {
            Stroke::NONE
        },
        ..TextFormat::default()
    };
    let mut job = LayoutJob::default();
    job.wrap.max_width = (width * zoom).max(1.0);
    let mut preedit = None;
    match at {
        Some(at) if !preedit_text.is_empty() && slice.is_char_boundary(at) => {
            let (before, after) = slice.split_at(at);
            job.append(before, 0.0, format(false));
            job.append(preedit_text, 0.0, format(true));
            job.append(after, 0.0, format(false));
            preedit = Some((before.chars().count(), preedit_text.chars().count()));
        }
        _ => job.append(slice, 0.0, format(false)),
    }
    (painter.layout_job(job), preedit)
}

/// The text size (page units) and font for a raw region that holds `block`.
fn raw_look(block: Option<&Block>) -> (f32, bool) {
    match block.map(|b| &b.kind) {
        Some(BlockKind::Heading { level, .. }) => (BODY_SIZE * heading_scale(*level), false),
        Some(BlockKind::Code { .. }) => (BODY_SIZE * 0.92, true),
        _ => (BODY_SIZE, false),
    }
}

/// The raw layout `block` would have if the caret were in it, placed at `pos`, for turning a
/// click on its styled form into a place in its source text.
pub(super) fn probe(
    painter: &Painter,
    tokens: &ThemeTokens,
    zoom: f32,
    text: &str,
    block: &Block,
    pos: Pos2,
    width: f32,
) -> Raw {
    let slice = block.source(text);
    let (size, mono) = raw_look(Some(block));
    let (galley, _) = raw_galley(
        painter,
        tokens,
        zoom,
        slice,
        (size, mono, width),
        (None, ""),
    );
    Raw {
        run: 0,
        span: block.span.start..block.span.end,
        pos,
        galley,
        zoom,
        text: slice.to_owned(),
        preedit: None,
    }
}

fn heading_scale(level: u8) -> f32 {
    match level {
        1 => 2.0,
        2 => 1.6,
        3 => 1.3,
        4 => 1.15,
        5 => 1.05,
        _ => 1.0,
    }
}

impl Layout<'_> {
    fn full(&self) -> bool {
        self.items.len() >= MAX_ITEMS
    }

    /// The bytes of `run` shown as raw markdown: the lines of the caret or selection, grown to
    /// whole blocks. `None` when the caret is elsewhere.
    fn region(
        &self,
        input: &Input<'_>,
        run: &ContainerRun,
        blocks: &[Block],
    ) -> Option<Range<usize>> {
        let cursor = input.cursor?;
        let range = cursor.range();
        let content = run.content;
        if range.end < content.start || range.start > content.end {
            return None;
        }
        let (lo, hi) = (range.start.max(content.start), range.end.min(content.end));
        if !cursor.is_caret() && hi <= lo {
            // A selection that only touches the edge of this container.
            return None;
        }
        let spans: Vec<Range<usize>> = blocks.iter().map(|b| b.span.start..b.span.end).collect();
        let region = text::active_region(input.text, &spans, lo, hi, content.start..content.end);
        (region.end.saturating_sub(region.start) <= MAX_RAW_BYTES).then_some(region)
    }

    /// Lay out the content of one container.
    fn run(
        &mut self,
        input: &Input<'_>,
        index: usize,
        run: &ContainerRun,
        x: f32,
        width: f32,
        y: &mut f32,
    ) {
        let blocks = input.doc.blocks.get(run.blocks.clone()).unwrap_or(&[]);
        let region = self.region(input, run, blocks);
        let inside = |block: &Block, r: &Range<usize>| {
            r.end > r.start && block.span.start >= r.start && block.span.end <= r.end
        };
        let mut pending = region.clone();
        for block in blocks {
            if self.full() {
                return;
            }
            if let Some(r) = pending.clone() {
                if block.span.start >= r.end && !inside(block, &r) {
                    self.raw(input, index, r, blocks, x, width, y);
                    pending = None;
                }
            }
            if region.as_ref().is_some_and(|r| inside(block, r)) {
                continue;
            }
            let top = *y;
            self.block(block, x, width, y);
            self.blocks_out.push(BlockBox {
                run: index,
                span: block.span,
                rect: Rect::from_min_max(Pos2::new(x, top), Pos2::new(x + width, (*y).max(top))),
            });
        }
        if let Some(r) = pending {
            self.raw(input, index, r, blocks, x, width, y);
        }
    }

    /// Lay out `region` of the page text as raw markdown, with the selection behind it and
    /// the IME composition at the caret.
    #[allow(clippy::too_many_arguments)]
    fn raw(
        &mut self,
        input: &Input<'_>,
        run: usize,
        region: Range<usize>,
        blocks: &[Block],
        x: f32,
        width: f32,
        y: &mut f32,
    ) {
        let slice = input.text.get(region.clone()).unwrap_or("");
        let first = blocks.iter().find(|b| {
            region.end > region.start && b.span.start >= region.start && b.span.end <= region.end
        });
        let (size, mono) = raw_look(first);
        let head = input
            .cursor
            .map(|c| c.head)
            .filter(|h| *h >= region.start && *h <= region.end);
        let local_head = head.map(|h| h - region.start);
        let (galley, preedit) = raw_galley(
            self.painter,
            self.tokens,
            self.zoom,
            slice,
            (size, mono, width),
            (local_head, input.preedit),
        );
        let height = (galley.size().y / self.zoom).max(size * 1.25);
        let raw = Raw {
            run,
            span: region.clone(),
            pos: Pos2::new(x, *y),
            galley: galley.clone(),
            zoom: self.zoom,
            text: slice.to_owned(),
            preedit,
        };
        if let Some(cursor) = input.cursor.filter(|c| !c.is_caret()) {
            let range = cursor.range();
            let color = mix(self.tokens.accent, self.tokens.canvas_bg, 0.55);
            for rect in raw.selection_rects(range.start, range.end) {
                self.items.push(Item::Fill { rect, color });
            }
        }
        if let Some(head) = head {
            self.caret = Some(raw.caret_rect(head));
        }
        self.items.push(Item::Text {
            pos: Pos2::new(x, *y),
            galley,
            bold: None,
        });
        self.raws.push(raw);
        *y += height + BODY_SIZE * 0.6;
    }

    fn blocks(&mut self, blocks: &[Block], x: f32, width: f32, y: &mut f32) {
        for block in blocks {
            if self.full() {
                return;
            }
            self.block(block, x, width, y);
        }
    }

    fn block(&mut self, block: &Block, x: f32, width: f32, y: &mut f32) {
        let tokens = self.tokens;
        let pending = self.pending_align.take();
        if let BlockKind::Html { literal } = &block.kind {
            if let Some(align) = markup::parse_align_comment(literal) {
                // A paragraph alignment comment draws nothing; it aligns the block below it.
                self.pending_align = Some(egui_align(align));
                return;
            }
        }
        match &block.kind {
            BlockKind::Heading { level, runs } => {
                let size = BODY_SIZE * heading_scale(*level);
                *y += size * 0.35;
                self.align = pending.unwrap_or(egui::Align::Min);
                let h = self.text(runs, size, true, tokens.text, x, *y, width);
                self.align = egui::Align::Min;
                *y += h + size * 0.3;
            }
            BlockKind::Paragraph { runs } => {
                self.align = pending.unwrap_or(egui::Align::Min);
                let h = self.text(runs, BODY_SIZE, false, tokens.text, x, *y, width);
                self.align = egui::Align::Min;
                *y += h + BODY_SIZE * 0.6;
            }
            BlockKind::List { kind, items, tight } => {
                self.list(*kind, items, *tight, x, width, y);
                *y += BODY_SIZE * 0.4;
            }
            BlockKind::Table(table) => {
                self.table(table, x, width, y);
                *y += BODY_SIZE * 0.6;
            }
            BlockKind::Code { literal, .. } | BlockKind::Html { literal } => {
                let is_html = matches!(block.kind, BlockKind::Html { .. });
                let color = if is_html {
                    tokens.text_muted
                } else {
                    tokens.text
                };
                self.code(literal.trim_end_matches('\n'), color, x, width, y);
                *y += BODY_SIZE * 0.6;
            }
            BlockKind::Quote { blocks } => {
                let top = *y;
                self.blocks(blocks, x + INDENT * 0.75, width - INDENT * 0.75, y);
                let bar = Rect::from_min_max(
                    Pos2::new(x + 2.0, top),
                    Pos2::new(x + 5.0, (*y - BODY_SIZE * 0.6).max(top)),
                );
                self.items.push(Item::Fill {
                    rect: bar,
                    color: tokens.border,
                });
            }
            BlockKind::Rule => {
                let at = *y + BODY_SIZE * 0.5;
                self.items.push(Item::Line {
                    from: Pos2::new(x, at),
                    to: Pos2::new(x + width, at),
                    color: tokens.border,
                });
                *y += BODY_SIZE * 1.4;
            }
        }
    }

    fn list(
        &mut self,
        kind: ListKind,
        items: &[notecraft_markdown::ListItem],
        tight: bool,
        x: f32,
        width: f32,
        y: &mut f32,
    ) {
        let first_number = match kind {
            ListKind::Ordered(n) => n,
            _ => 1,
        };
        for (index, item) in items.iter().enumerate() {
            if self.full() {
                return;
            }
            let top = *y;
            let line = BODY_SIZE * 1.4;
            let marker_x = x + 2.0;
            match item.task {
                Some(checked) => {
                    let size = BODY_SIZE * 0.95;
                    let min = Pos2::new(marker_x, top + (line - size) / 2.0);
                    self.items.push(Item::Check {
                        rect: Rect::from_min_size(min, egui::vec2(size, size)),
                        checked,
                    });
                }
                None if matches!(kind, ListKind::Ordered(_)) => {
                    let number = u64::from(first_number).saturating_add(index as u64);
                    let label = vec![Run::plain(format!("{number}."))];
                    self.text(
                        &label,
                        BODY_SIZE,
                        false,
                        self.tokens.text,
                        marker_x,
                        top,
                        INDENT,
                    );
                }
                None => self.items.push(Item::Dot {
                    center: Pos2::new(marker_x + 5.0, top + line / 2.0),
                    radius: 2.2,
                }),
            }
            self.blocks(&item.blocks, x + INDENT + 4.0, width - INDENT - 4.0, y);
            if *y == top {
                *y += line;
            }
            if !tight {
                *y += BODY_SIZE * 0.3;
            }
        }
    }

    fn code(&mut self, literal: &str, color: Color32, x: f32, width: f32, y: &mut f32) {
        let pad = 8.0;
        let size = BODY_SIZE * 0.9;
        let top = *y;
        let galley = self.galley(
            &[Run {
                text: literal.to_owned(),
                code: true,
                ..Run::default()
            }],
            size,
            false,
            color,
            width - 2.0 * pad,
        );
        let height = galley.size().y / self.zoom;
        let fill = mix(self.tokens.canvas_bg, self.tokens.text, 0.07);
        self.items.push(Item::Fill {
            rect: Rect::from_min_size(Pos2::new(x, top), egui::vec2(width, height + 2.0 * pad)),
            color: fill,
        });
        self.items.push(Item::Text {
            pos: Pos2::new(x + pad, top + pad),
            galley,
            bold: None,
        });
        *y += height + 2.0 * pad;
    }

    fn table(&mut self, table: &Table, x: f32, width: f32, y: &mut f32) {
        let columns = std::iter::once(&table.header)
            .chain(&table.rows)
            .map(Vec::len)
            .max()
            .unwrap_or(0)
            .min(MAX_COLUMNS);
        if columns == 0 {
            return;
        }
        let pad = 6.0;
        let col_w = width / columns as f32;
        let grid = self.tokens.border;
        let top = *y;
        let rows =
            std::iter::once((&table.header, true)).chain(table.rows.iter().map(|r| (r, false)));
        for (row, is_header) in rows {
            if self.full() || row.is_empty() {
                continue;
            }
            let row_top = *y;
            let mut cells = Vec::new();
            let mut row_h = BODY_SIZE * 1.4;
            for (c, cell) in row.iter().take(columns).enumerate() {
                let align = table.align.get(c).copied().unwrap_or_default();
                let galley = self.cell(cell, is_header, col_w - 2.0 * pad);
                row_h = row_h.max(galley.size().y / self.zoom + 2.0 * pad);
                cells.push((c, galley, align));
            }
            if is_header {
                self.items.push(Item::Fill {
                    rect: Rect::from_min_size(Pos2::new(x, row_top), egui::vec2(width, row_h)),
                    color: mix(self.tokens.canvas_bg, self.tokens.text, 0.07),
                });
            }
            for (c, galley, align) in cells {
                let gw = galley.size().x / self.zoom;
                let room = (col_w - 2.0 * pad - gw).max(0.0);
                let dx = match align {
                    Align::Right => room,
                    Align::Center => room / 2.0,
                    Align::None | Align::Left => 0.0,
                };
                let pos = Pos2::new(x + c as f32 * col_w + pad + dx, row_top + pad);
                let bold = is_header.then(|| galley.clone());
                self.items.push(Item::Text { pos, galley, bold });
            }
            *y += row_h;
            self.items.push(Item::Line {
                from: Pos2::new(x, *y),
                to: Pos2::new(x + width, *y),
                color: grid,
            });
        }
        self.items.push(Item::Line {
            from: Pos2::new(x, top),
            to: Pos2::new(x + width, top),
            color: grid,
        });
        for c in 0..=columns {
            let cx = x + c as f32 * col_w;
            self.items.push(Item::Line {
                from: Pos2::new(cx, top),
                to: Pos2::new(cx, *y),
                color: grid,
            });
        }
    }

    fn cell(&self, cell: &Cell, header: bool, width: f32) -> Arc<Galley> {
        self.galley(&cell.runs, BODY_SIZE, header, self.tokens.text, width)
    }

    /// Lay out `runs` at (`x`, `y`) wrapped to `width`; returns the height in page units.
    fn text(
        &mut self,
        runs: &[Run],
        size: f32,
        bold: bool,
        color: Color32,
        x: f32,
        y: f32,
        width: f32,
    ) -> f32 {
        if runs.is_empty() {
            return 0.0;
        }
        let galley = self.galley(runs, size, bold, color, width);
        let height = galley.size().y / self.zoom;
        let wants_bold = bold || runs.iter().any(|r| r.bold);
        let ghost = wants_bold.then(|| self.galley_bold_only(runs, size, bold, color, width));
        // With a centered or right alignment the galley is anchored at the middle or right edge.
        let x = match self.align {
            egui::Align::Min => x,
            egui::Align::Center => x + width / 2.0,
            egui::Align::Max => x + width,
        };
        self.items.push(Item::Text {
            pos: Pos2::new(x, y),
            galley,
            bold: ghost,
        });
        height
    }

    fn job(
        &self,
        runs: &[Run],
        size: f32,
        bold: bool,
        color: Color32,
        width: f32,
        only_bold: bool,
    ) -> LayoutJob {
        let mut job = LayoutJob::default();
        job.wrap.max_width = (width * self.zoom).max(1.0);
        job.halign = self.align;
        for run in runs {
            let is_bold = bold || run.bold;
            let family = if run.code {
                FontFamily::Monospace
            } else {
                FontFamily::Proportional
            };
            let mut run_size = if run.code { size * 0.92 } else { size };
            if let Some(points) = run.size_pt {
                run_size *= f32::from(points) / 11.0;
            }
            let valign = match run.script {
                Script::None => egui::Align::Center,
                Script::Sub => egui::Align::BOTTOM,
                Script::Sup => egui::Align::TOP,
            };
            if run.script != Script::None {
                run_size *= 0.7;
            }
            let mut text_color = if run.link.is_some() {
                self.tokens.accent
            } else if let Some(rgb) = run.color {
                rgb_color(rgb)
            } else {
                color
            };
            if only_bold && !is_bold {
                text_color = Color32::TRANSPARENT;
            }
            let underline =
                if (run.link.is_some() || run.underline) && text_color != Color32::TRANSPARENT {
                    Stroke::new(1.0, text_color)
                } else {
                    Stroke::NONE
                };
            let strikethrough = if run.strike && text_color != Color32::TRANSPARENT {
                Stroke::new(1.0, text_color)
            } else {
                Stroke::NONE
            };
            let background = if only_bold {
                Color32::TRANSPARENT
            } else if let Some(rgb) = run.highlight {
                // Blended with the paper so dark text stays readable on it.
                mix(self.tokens.canvas_bg, rgb_color(rgb), 0.7)
            } else if run.code {
                mix(self.tokens.canvas_bg, self.tokens.text, 0.09)
            } else {
                Color32::TRANSPARENT
            };
            job.append(
                &run.text,
                0.0,
                TextFormat {
                    font_id: FontId::new(run_size * self.zoom, family),
                    color: text_color,
                    italics: run.italic,
                    underline,
                    strikethrough,
                    background,
                    valign,
                    ..TextFormat::default()
                },
            );
        }
        job
    }

    fn galley(
        &self,
        runs: &[Run],
        size: f32,
        bold: bool,
        color: Color32,
        width: f32,
    ) -> Arc<Galley> {
        self.painter
            .layout_job(self.job(runs, size, bold, color, width, false))
    }

    fn galley_bold_only(
        &self,
        runs: &[Run],
        size: f32,
        bold: bool,
        color: Color32,
        width: f32,
    ) -> Arc<Galley> {
        self.painter
            .layout_job(self.job(runs, size, bold, color, width, true))
    }
}

/// Draw the laid-out page.
pub(super) fn paint(painter: &Painter, map: PageToScreen, flow: &Flow, tokens: &ThemeTokens) {
    let clip = painter.clip_rect();
    let to_screen = |p: Pos2| map.pos(Vec2::new(p.x, p.y));
    let z = map.zoom;
    debug_assert!((z - flow.zoom).abs() < 1e-3 || flow.zoom > 0.0);
    for item in &flow.items {
        match item {
            Item::Text { pos, galley, bold } => {
                let at = to_screen(*pos);
                let rect = Rect::from_min_size(at, galley.size());
                if !rect.intersects(clip) {
                    continue;
                }
                painter.galley(at, galley.clone(), tokens.text);
                if let Some(bold) = bold {
                    painter.galley(
                        at + egui::vec2(0.7 * z.min(2.0), 0.0),
                        bold.clone(),
                        tokens.text,
                    );
                }
            }
            Item::Fill { rect, color } => {
                let r = Rect::from_min_max(to_screen(rect.min), to_screen(rect.max));
                if r.intersects(clip) {
                    painter.rect_filled(r, 0.0, *color);
                }
            }
            Item::Line { from, to, color } => {
                let (a, b) = (to_screen(*from), to_screen(*to));
                if Rect::from_two_pos(a, b).expand(2.0).intersects(clip) {
                    painter.line_segment([a, b], Stroke::new(1.0, *color));
                }
            }
            Item::Dot { center, radius } => {
                let c = to_screen(*center);
                if clip.expand(8.0).contains(c) {
                    painter.circle_filled(c, radius * z, tokens.text);
                }
            }
            Item::Check { rect, checked } => {
                let r = Rect::from_min_max(to_screen(rect.min), to_screen(rect.max));
                if !r.intersects(clip) {
                    continue;
                }
                let stroke = Stroke::new(1.2, tokens.text);
                painter.rect_stroke(r, 2.0 * z, stroke, egui::StrokeKind::Inside);
                if *checked {
                    let w = r.width();
                    let tick = [
                        r.min + egui::vec2(w * 0.22, w * 0.52),
                        r.min + egui::vec2(w * 0.42, w * 0.72),
                        r.min + egui::vec2(w * 0.78, w * 0.28),
                    ];
                    painter.add(egui::Shape::line(
                        tick.to_vec(),
                        Stroke::new(1.6, tokens.accent),
                    ));
                }
            }
        }
    }
}

/// 0xRRGGBB as an opaque color.
fn rgb_color(rgb: u32) -> Color32 {
    Color32::from_rgb(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}

/// The egui alignment for a paragraph alignment.
fn egui_align(align: Align) -> egui::Align {
    match align {
        Align::Center => egui::Align::Center,
        Align::Right => egui::Align::Max,
        Align::Left | Align::None => egui::Align::Min,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::{Preset, ThemeTokens};

    fn with_painter<R>(f: impl FnOnce(&Painter) -> R) -> R {
        let ctx = egui::Context::default();
        let mut out = None;
        let mut f = Some(f);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            if let Some(f) = f.take() {
                out = Some(f(ui.painter()));
            }
        });
        output.textures_delta.clear();
        out.expect("ran once")
    }

    fn tokens() -> ThemeTokens {
        ThemeTokens::for_variant(Preset::M365, false)
    }

    /// Lay out `text` with every container in the flow, and the caret where `cursor` says.
    pub(in crate::canvas) fn lay(
        painter: &Painter,
        text: &str,
        zoom: f32,
        tokens: &ThemeTokens,
        cursor: Option<TextCursor>,
    ) -> Flow {
        let doc = notecraft_markdown::parse(text);
        let runs = notecraft_markdown::split_containers(text, &doc);
        let geometry: Vec<Container> = runs.iter().map(|r| Container::flow(&r.id)).collect();
        let input = Input {
            text,
            doc: &doc,
            runs: &runs,
            geometry: &geometry,
            cursor,
            preedit: "",
        };
        layout(painter, &input, zoom, tokens)
    }

    #[test]
    fn flow_grows_with_content_and_shows_every_block() {
        let tokens = tokens();
        let long = "Paragraph of words.\n\n".repeat(60);
        let (h_short, h_long, texts) = with_painter(|painter| {
            let short = lay(painter, "Hello", 1.0, &tokens, None);
            let long = lay(painter, &long, 1.0, &tokens, None);
            let texts = long
                .items
                .iter()
                .filter(|i| matches!(i, Item::Text { .. }))
                .count();
            (short.height, long.height, texts)
        });
        assert!(h_long > h_short + 500.0);
        assert_eq!(texts, 60);
    }

    #[test]
    fn welcome_page_lays_out_every_block_kind() {
        let tokens = tokens();
        let flow = with_painter(|painter| {
            lay(
                painter,
                notecraft_engine::mock::WELCOME_SOURCE,
                1.0,
                &tokens,
                None,
            )
        });
        let has = |f: fn(&Item) -> bool| flow.items.iter().any(f);
        assert!(has(|i| matches!(i, Item::Text { .. })));
        assert!(has(|i| matches!(i, Item::Check { checked: true, .. })));
        assert!(has(|i| matches!(i, Item::Check { checked: false, .. })));
        assert!(has(|i| matches!(i, Item::Dot { .. })));
        assert!(has(|i| matches!(i, Item::Fill { .. })));
        assert!(has(|i| matches!(i, Item::Line { .. })));
        let text: String = flow
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Text { galley, .. } => Some(galley.text().to_owned()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        for word in [
            "Welcome to NoteCraft",
            "plain markdown",
            "Add a page",
            "Pick a section",
            "Reading comes first",
            "cargo xtask ci",
        ] {
            assert!(text.contains(word), "missing {word}");
        }
    }

    #[test]
    fn a_gfm_table_draws_its_cells_and_grid() {
        let tokens = tokens();
        let flow = with_painter(|painter| {
            lay(
                painter,
                "| Name | Qty |\n|---|---:|\n| Pen | 3 |\n| Ink | 12 |\n",
                1.0,
                &tokens,
                None,
            )
        });
        let cells: Vec<String> = flow
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Text { galley, .. } => Some(galley.text().to_owned()),
                _ => None,
            })
            .collect();
        for word in ["Name", "Qty", "Pen", "3", "Ink", "12"] {
            assert!(cells.iter().any(|c| c == word), "missing cell {word}");
        }
        assert!(flow.items.iter().any(|i| matches!(i, Item::Line { .. })));
    }

    #[test]
    fn formatted_runs_and_alignment_lay_out() {
        let tokens = tokens();
        let plain = with_painter(|p| lay(p, "word\n", 1.0, &tokens, None));
        let styled = with_painter(|p| {
            lay(
                p,
                "<!-- notecraft:align=center -->\n<u>a</u> <sup>b</sup> ==c== <span style=\"color:#FF0000\"><span style=\"font-size:22pt\">d</span></span>\n",
                1.0,
                &tokens,
                None,
            )
        });
        assert!(plain.height.is_finite() && styled.height.is_finite());
        let texts = styled
            .items
            .iter()
            .filter(|i| matches!(i, Item::Text { .. }))
            .count();
        assert_eq!(texts, 1, "the alignment comment draws nothing of its own");
    }

    #[test]
    fn hostile_pages_lay_out_without_panicking() {
        let tokens = tokens();
        let deep = format!("{}x", "> ".repeat(3000));
        let wide = format!("{}\n{}\n", "|a".repeat(500), "|-".repeat(500));
        let markers = "<!-- notecraft:container id=c1 -->\n".repeat(200);
        with_painter(|painter| {
            for text in [deep.as_str(), wide.as_str(), "", markers.as_str()] {
                for zoom in [0.1, 1.0, 4.0, f32::NAN, 0.0] {
                    for cursor in [None, Some(TextCursor::caret(text.len() / 2))] {
                        let flow = lay(painter, text, zoom, &tokens, cursor);
                        assert!(flow.height.is_finite());
                    }
                }
            }
        });
    }

    #[test]
    fn the_caret_block_is_shown_as_raw_markdown_and_the_rest_is_styled() {
        let tokens = tokens();
        let text = "# Title\n\nSome **bold** words.\n\nLast one.\n";
        let at = text.find("Some").expect("para") + 3;
        with_painter(|painter| {
            let flow = lay(painter, text, 1.0, &tokens, Some(TextCursor::caret(at)));
            assert_eq!(flow.raws.len(), 1);
            let raw = flow.raws.first().expect("raw");
            assert_eq!(raw.text, "Some **bold** words.");
            // The other two blocks stay styled blocks.
            assert_eq!(flow.blocks.len(), 2);
            assert!(flow.caret.is_some());
            let caret = flow.caret.expect("caret");
            assert!(raw.rect().expand(2.0).contains(caret.center()));
            // Without a caret nothing is raw.
            let flow = lay(painter, text, 1.0, &tokens, None);
            assert!(flow.raws.is_empty() && flow.caret.is_none());
            assert_eq!(flow.blocks.len(), 3);
        });
    }

    #[test]
    fn a_caret_on_a_new_blank_line_gets_its_own_raw_row() {
        let tokens = tokens();
        let text = "Hello\n\n";
        with_painter(|painter| {
            let flow = lay(painter, text, 1.0, &tokens, Some(TextCursor::caret(6)));
            assert_eq!(flow.blocks.len(), 1);
            assert_eq!(flow.raws.len(), 1);
            let raw = flow.raws.first().expect("raw");
            assert!(raw.span.start >= 6 && raw.text.trim().is_empty());
            let block = flow.blocks.first().expect("block");
            // The empty row sits below the paragraph.
            assert!(raw.pos.y > block.rect.min.y);
        });
    }

    #[test]
    fn selection_across_blocks_is_one_raw_region_with_highlights() {
        let tokens = tokens();
        let text = "one\n\ntwo\n\nthree\n";
        with_painter(|painter| {
            let cursor = TextCursor {
                anchor: 1,
                head: 12,
            };
            let flow = lay(painter, text, 1.0, &tokens, Some(cursor));
            assert_eq!(flow.raws.len(), 1);
            let fills = flow
                .items
                .iter()
                .filter(|i| matches!(i, Item::Fill { .. }))
                .count();
            assert!(fills >= 3, "one highlight per row, got {fills}");
        });
    }

    #[test]
    fn containers_are_placed_and_the_flow_skips_positioned_ones() {
        let tokens = tokens();
        let text = "first\n\n<!-- notecraft:container id=c1 -->\nsecond\n\n<!-- notecraft:container id=c2 -->\nthird\n";
        with_painter(|painter| {
            let doc = notecraft_markdown::parse(text);
            let runs = notecraft_markdown::split_containers(text, &doc);
            assert_eq!(runs.len(), 3);
            let geometry = vec![
                Container::flow("c0"),
                Container::at("c1", Vec2::new(500.0, 300.0), 200.0),
                Container::flow("c2"),
            ];
            let input = Input {
                text,
                doc: &doc,
                runs: &runs,
                geometry: &geometry,
                cursor: None,
                preedit: "",
            };
            let flow = layout(painter, &input, 1.0, &tokens);
            let rect = |i: usize| flow.containers.get(i).expect("box").rect;
            assert_eq!(rect(1).min, Pos2::new(500.0, 300.0));
            assert_eq!(rect(1).width(), 200.0);
            // c2 follows c0 in the flow, not c1.
            assert_eq!(rect(2).min.x, LEFT);
            assert!(rect(2).min.y > rect(0).max.y && rect(2).min.y < 300.0);
            assert!(flow.width >= 500.0 + 200.0);
        });
    }

    #[test]
    fn an_empty_container_still_has_room_for_the_caret() {
        let tokens = tokens();
        let text = "a\n\n<!-- notecraft:container id=c1 -->\n";
        with_painter(|painter| {
            let at = text.len();
            let flow = lay(painter, text, 1.0, &tokens, Some(TextCursor::caret(at)));
            let empty = flow.containers.get(1).expect("c1");
            assert!(empty.rect.height() >= BODY_SIZE);
            assert!(flow.caret.is_some());
        });
    }

    #[test]
    fn an_ime_composition_shows_at_the_caret_without_touching_the_text() {
        let tokens = tokens();
        let text = "ab";
        with_painter(|painter| {
            let doc = notecraft_markdown::parse(text);
            let runs = notecraft_markdown::split_containers(text, &doc);
            let geometry = vec![Container::flow("c0")];
            let input = Input {
                text,
                doc: &doc,
                runs: &runs,
                geometry: &geometry,
                cursor: Some(TextCursor::caret(1)),
                preedit: "\u{4e2d}\u{6587}",
            };
            let flow = layout(painter, &input, 1.0, &tokens);
            let raw = flow.raws.first().expect("raw");
            assert_eq!(raw.text, "ab");
            assert_eq!(raw.galley.text(), "a\u{4e2d}\u{6587}b");
            assert_eq!(raw.preedit, Some((1, 2)));
            // Clicks map back to page offsets around the composition.
            let right_of_b = raw.rect().right_center() + egui::vec2(20.0, 0.0);
            assert_eq!(raw.offset_at(right_of_b), 2);
            let left = raw.rect().left_center() - egui::vec2(20.0, 0.0);
            assert_eq!(raw.offset_at(left), 0);
        });
    }

    #[test]
    fn vertical_moves_leave_the_region_at_its_first_and_last_row() {
        let tokens = tokens();
        let text = "one\ntwo\nthree";
        with_painter(|painter| {
            let flow = lay(painter, text, 1.0, &tokens, Some(TextCursor::caret(5)));
            let raw = flow.raws.first().expect("raw");
            let (up, _) = raw.vertical(5, false, None).expect("row above");
            assert!(up < 4);
            assert!(raw.vertical(0, false, None).is_none());
            assert!(raw.vertical(text.len(), true, None).is_none());
            let (down, _) = raw.vertical(1, true, None).expect("row below");
            assert!((4..8).contains(&down));
            assert_eq!(raw.row_edge(5, false), 4);
            assert_eq!(raw.row_edge(5, true), 7);
        });
    }
}
