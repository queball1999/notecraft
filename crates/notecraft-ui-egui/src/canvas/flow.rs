//! Flow layout for a markdown page (read-only): blocks stack top to bottom in one column, like
//! a document (docs/markdown.md section 3.5). `layout` turns a parsed page into drawing items in
//! page units; `paint` draws them. Colors come from theme tokens only.
//!
//! The seam for page text is one function: `layout(painter, &Document, zoom, tokens)`.

use std::sync::Arc;

use egui::epaint::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId, Galley, Painter, Pos2, Rect, Stroke};
use notecraft_core::Vec2;
use notecraft_markdown::{Align, Block, BlockKind, Cell, Document, ListKind, Run, Table};

use super::pictures::{PictureSource, Shown};
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
    /// A picture, or the placeholder that stands in for one that cannot be shown.
    Picture {
        rect: Rect,
        shown: Shown,
        /// Alt text (and the reason, for a placeholder).
        label: Arc<Galley>,
    },
    /// A file attachment: an icon with the file name.
    File {
        rect: Rect,
        name: Arc<Galley>,
        ext: String,
    },
}

/// A file attachment on the page, for clicks.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct FileHit {
    /// In page units.
    pub rect: Rect,
    /// The link as written in the page.
    pub link: String,
    pub name: String,
}

/// A laid-out page.
pub(super) struct Flow {
    items: Vec<Item>,
    pub files: Vec<FileHit>,
    /// Bottom of the content, in page units.
    pub height: f32,
    zoom: f32,
}

struct Layout<'a> {
    painter: &'a Painter,
    tokens: &'a ThemeTokens,
    zoom: f32,
    items: Vec<Item>,
    files: Vec<FileHit>,
    pictures: &'a mut dyn PictureSource,
}

/// Lay out `doc` in flow layout at `zoom` (1.0 is 100%). Pictures come from `pictures`.
pub(super) fn layout(
    painter: &Painter,
    doc: &Document,
    zoom: f32,
    tokens: &ThemeTokens,
    pictures: &mut dyn PictureSource,
) -> Flow {
    let mut ctx = Layout {
        painter,
        tokens,
        files: Vec::new(),
        pictures,
        zoom: if zoom.is_finite() && zoom > 0.0 {
            zoom
        } else {
            1.0
        },
        items: Vec::new(),
    };
    let mut y = TOP;
    ctx.blocks(&doc.blocks, LEFT, COLUMN_WIDTH, &mut y);
    Flow {
        height: y + BOTTOM_MARGIN,
        zoom: ctx.zoom,
        items: ctx.items,
        files: ctx.files,
    }
}

/// Whether a link points at a file in the notebook (not the web, an anchor, or mail).
fn is_file_link(link: &str) -> bool {
    let link = link.trim();
    if link.is_empty() || link.starts_with('#') || link.starts_with("//") {
        return false;
    }
    match link.split_once(':') {
        Some((scheme, _)) => scheme.len() == 1,
        None => true,
    }
}

/// Fixed size of a file attachment object, in page units.
const FILE_HEIGHT: f32 = 44.0;
/// Height of a placeholder for a picture that cannot be shown.
const PLACEHOLDER_HEIGHT: f32 = 72.0;

fn extension_of(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| e.chars().take(4).collect::<String>().to_uppercase())
        .unwrap_or_default()
}

/// The file name inside a link, decoded.
fn link_file_name(link: &str) -> String {
    let decoded = percent_decode(link);
    decoded
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// Percent-decode a link (a `%` without two hex digits stays).
fn percent_decode(link: &str) -> String {
    let bytes = link.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        let hex = |at: usize| bytes.get(at).and_then(|c| char::from(*c).to_digit(16));
        if b == b'%' {
            if let (Some(h), Some(l)) = (hex(i + 1), hex(i + 2)) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
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
        match &block.kind {
            BlockKind::Heading { level, runs } => {
                let size = BODY_SIZE * heading_scale(*level);
                *y += size * 0.35;
                let h = self.text(runs, size, true, tokens.text, x, *y, width);
                *y += h + size * 0.3;
            }
            BlockKind::Paragraph { runs } if self.is_file_paragraph(runs) => {
                if let Some(run) = runs.first() {
                    self.file(run, x, width, y);
                }
            }
            BlockKind::Paragraph { runs } if runs.iter().any(|r| r.image) => {
                let mut pending: Vec<Run> = Vec::new();
                for run in runs {
                    if !run.image {
                        pending.push(run.clone());
                        continue;
                    }
                    self.paragraph_text(&mut pending, x, width, y);
                    self.picture(run, x, width, y);
                }
                self.paragraph_text(&mut pending, x, width, y);
            }
            BlockKind::Paragraph { runs } => {
                let h = self.text(runs, BODY_SIZE, false, tokens.text, x, *y, width);
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

    /// A paragraph that is one link to a file in the notebook: an attachment object.
    fn is_file_paragraph(&self, runs: &[Run]) -> bool {
        matches!(runs, [run] if !run.image && !run.code
            && run.link.as_deref().is_some_and(is_file_link)
            && !run.text.trim().is_empty())
    }

    /// Lay out and clear the text collected before or after a picture.
    fn paragraph_text(&mut self, pending: &mut Vec<Run>, x: f32, width: f32, y: &mut f32) {
        let blank = pending.iter().all(|r| r.text.trim().is_empty());
        if !blank {
            let h = self.text(pending, BODY_SIZE, false, self.tokens.text, x, *y, width);
            *y += h + BODY_SIZE * 0.4;
        }
        pending.clear();
    }

    fn small_galley(&self, text: &str, color: Color32, width: f32) -> Arc<Galley> {
        let run = Run::plain(text);
        self.galley(&[run], BODY_SIZE * 0.85, false, color, width)
    }

    /// A picture at its natural size, shrunk to fit the column; or a placeholder.
    fn picture(&mut self, run: &Run, x: f32, width: f32, y: &mut f32) {
        let link = run.link.clone().unwrap_or_default();
        let shown = if is_file_link(&link) {
            self.pictures.picture(&link)
        } else {
            Shown::Failed("pictures from the web are not loaded".to_owned())
        };
        let alt = if run.text.trim().is_empty() {
            link_file_name(&link)
        } else {
            run.text.clone()
        };
        let (w, h, label) = match &shown {
            Shown::Ready { width: w, height: h, .. } => {
                let scale = (width / w.max(1.0)).min(1.0);
                (w * scale, h * scale, alt)
            }
            Shown::Loading => (width.min(240.0), PLACEHOLDER_HEIGHT, "Loading picture".to_owned()),
            Shown::Failed(why) => (
                width.min(320.0),
                PLACEHOLDER_HEIGHT,
                format!("Picture not shown: {alt} ({why})"),
            ),
        };
        let galley = self.small_galley(&label, self.tokens.text_muted, (w - 16.0).max(40.0));
        let rect = Rect::from_min_size(Pos2::new(x, *y), egui::vec2(w.max(1.0), h.max(1.0)));
        self.items.push(Item::Picture {
            rect,
            shown,
            label: galley,
        });
        *y += h + BODY_SIZE * 0.6;
    }

    /// A file attachment: icon and name. Clicking it opens the file.
    fn file(&mut self, run: &Run, x: f32, width: f32, y: &mut f32) {
        let link = run.link.clone().unwrap_or_default();
        let name = run.text.trim().to_owned();
        let w = (width * 0.6).clamp(180.0, 360.0).min(width);
        let galley = self.small_galley(&name, self.tokens.text, w - FILE_HEIGHT - 8.0);
        let rect = Rect::from_min_size(Pos2::new(x, *y), egui::vec2(w, FILE_HEIGHT));
        self.files.push(FileHit {
            rect,
            link: link.clone(),
            name: name.clone(),
        });
        self.items.push(Item::File {
            rect,
            name: galley,
            ext: extension_of(&link_file_name(&link)),
        });
        *y += FILE_HEIGHT + BODY_SIZE * 0.6;
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
        for run in runs {
            let is_bold = bold || run.bold;
            let family = if run.code {
                FontFamily::Monospace
            } else {
                FontFamily::Proportional
            };
            let code_size = if run.code { size * 0.92 } else { size };
            let mut text_color = if run.link.is_some() {
                self.tokens.accent
            } else {
                color
            };
            if only_bold && !is_bold {
                text_color = Color32::TRANSPARENT;
            }
            let underline = if run.link.is_some() && text_color != Color32::TRANSPARENT {
                Stroke::new(1.0, text_color)
            } else {
                Stroke::NONE
            };
            let strikethrough = if run.strike && text_color != Color32::TRANSPARENT {
                Stroke::new(1.0, text_color)
            } else {
                Stroke::NONE
            };
            let background = if run.code && !only_bold {
                mix(self.tokens.canvas_bg, self.tokens.text, 0.09)
            } else {
                Color32::TRANSPARENT
            };
            job.append(
                &run.text,
                0.0,
                TextFormat {
                    font_id: FontId::new(code_size * self.zoom, family),
                    color: text_color,
                    italics: run.italic,
                    underline,
                    strikethrough,
                    background,
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
            Item::Picture { rect, shown, label } => {
                let r = Rect::from_min_max(to_screen(rect.min), to_screen(rect.max));
                if !r.intersects(clip) {
                    continue;
                }
                match shown {
                    Shown::Ready { texture, .. } => {
                        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                        painter.image(*texture, r, uv, Color32::WHITE);
                    }
                    Shown::Loading | Shown::Failed(_) => {
                        painter.rect_filled(r, 4.0 * z, mix(tokens.canvas_bg, tokens.text, 0.06));
                        painter.rect_stroke(
                            r,
                            4.0 * z,
                            Stroke::new(1.0, tokens.border),
                            egui::StrokeKind::Inside,
                        );
                        painter.galley(r.min + egui::vec2(8.0, 8.0) * z, label.clone(), tokens.text_muted);
                    }
                }
            }
            Item::File { rect, name, ext } => {
                let r = Rect::from_min_max(to_screen(rect.min), to_screen(rect.max));
                if !r.intersects(clip) {
                    continue;
                }
                painter.rect_filled(r, 4.0 * z, mix(tokens.canvas_bg, tokens.text, 0.05));
                painter.rect_stroke(r, 4.0 * z, Stroke::new(1.0, tokens.border), egui::StrokeKind::Inside);
                // A page with a folded corner, drawn in code, with the extension on it.
                let icon = Rect::from_min_size(
                    r.min + egui::vec2(8.0, 6.0) * z,
                    egui::vec2(26.0, 32.0) * z,
                );
                painter.rect_stroke(icon, 2.0 * z, Stroke::new(1.2, tokens.accent), egui::StrokeKind::Inside);
                let fold = 7.0 * z;
                painter.line_segment(
                    [icon.right_top() - egui::vec2(fold, 0.0), icon.right_top() + egui::vec2(0.0, fold)],
                    Stroke::new(1.2, tokens.accent),
                );
                if !ext.is_empty() {
                    painter.text(
                        icon.center() + egui::vec2(0.0, 5.0 * z),
                        egui::Align2::CENTER_CENTER,
                        ext,
                        FontId::proportional(7.5 * z),
                        tokens.accent,
                    );
                }
                let text_at = Pos2::new(icon.max.x + 8.0 * z, r.center().y - name.size().y / 2.0);
                painter.galley(text_at, name.clone(), tokens.text);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::pictures::NoPictures;
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

    #[test]
    fn flow_grows_with_content_and_shows_every_block() {
        let tokens = tokens();
        let short = notecraft_markdown::parse("Hello");
        let long = notecraft_markdown::parse(&"Paragraph of words.\n\n".repeat(60));
        let (h_short, h_long, texts) = with_painter(|painter| {
            let short = layout(painter, &short, 1.0, &tokens, &mut NoPictures);
            let long = layout(painter, &long, 1.0, &tokens, &mut NoPictures);
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
        let doc = notecraft_markdown::parse(notecraft_engine::mock::WELCOME_SOURCE);
        let flow = with_painter(|painter| layout(painter, &doc, 1.0, &tokens, &mut NoPictures));
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
    fn hostile_pages_lay_out_without_panicking() {
        let tokens = tokens();
        let deep = notecraft_markdown::parse(&format!("{}x", "> ".repeat(3000)));
        let wide =
            notecraft_markdown::parse(&format!("{}\n{}\n", "|a".repeat(500), "|-".repeat(500)));
        let empty = notecraft_markdown::parse("");
        with_painter(|painter| {
            for doc in [&deep, &wide, &empty] {
                for zoom in [0.1, 1.0, 4.0, f32::NAN, 0.0] {
                    let flow = layout(painter, doc, zoom, &tokens, &mut NoPictures);
                    assert!(flow.height.is_finite());
                }
            }
        });
    }
}
