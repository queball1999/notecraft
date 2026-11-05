//! Equations in the flow layout (PR 2.7).
//!
//! A display formula (`$$...$$`) gets a line of its own, centered in the column. An inline
//! formula (`$...$`) sits in the line of text: the text galley reserves its width with a
//! transparent zero-width placeholder character that has that much leading space, and the
//! formula is drawn over the gap once the galley is laid out. A formula that cannot be shown
//! (not valid math, too slow, still loading) is shown as its source text in the code style, so
//! the page never loses content.

use std::collections::HashMap;

use egui::{Color32, Pos2, Rect, TextureId};
use notecraft_markdown::math as source;
use notecraft_markdown::{Math, Run};

use super::super::pictures::{MathKey, MathPreview, MathShown};
use super::{mix, Item, Layout, BODY_SIZE};

/// The character that stands in for an inline formula in a text galley. A word joiner has no
/// width and no look, and does not allow a line break.
pub(super) const PLACEHOLDER: char = '\u{2060}';

/// Size of the em a display formula is typeset with, relative to the body text.
const DISPLAY_SCALE: f32 = 1.2;

/// An inline formula that is ready to draw.
#[derive(Clone, Copy)]
pub(super) struct InlineMath {
    pub texture: TextureId,
    /// In page units.
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
}

/// The inline formulas of the text being laid out, by source and text size (bits).
pub(super) type InlineMap = HashMap<(String, u32), InlineMath>;

fn rgb_of(color: Color32) -> u32 {
    (u32::from(color.r()) << 16) | (u32::from(color.g()) << 8) | u32::from(color.b())
}

/// The run that shows `run`'s source text when the formula cannot be drawn.
pub(super) fn source_run(run: &Run) -> Run {
    Run {
        text: source::wrap(&run.text, run.math),
        code: true,
        ..Run::default()
    }
}

/// Space between the source text and the live preview card, in page units.
const PREVIEW_GAP: f32 = 4.0;
/// Padding inside the preview card, in page units.
const PREVIEW_PAD: f32 = 6.0;
/// Size of the error line in the preview card, relative to the body text.
const PREVIEW_ERROR_SCALE: f32 = 0.85;

/// Where the top of a live preview of height `height` goes: just below the source (which ends
/// at `bottom`), or above it (the source starts at `top`) when the page has no room left
/// below, that is when the preview would end past `limit`. Never above the page top.
pub(super) fn preview_top(top: f32, bottom: f32, height: f32, limit: f32) -> (f32, bool) {
    let below = bottom + PREVIEW_GAP;
    if below + height <= limit {
        (below, true)
    } else {
        ((top - PREVIEW_GAP - height).max(0.0), false)
    }
}

impl Layout<'_> {
    /// The live preview of the formula the caret at `head` is in: a small card with the
    /// typeset formula (or why it cannot be typeset) right below the source text, which
    /// occupies `top..bottom` of the page column at `x` and `width`. Returns the height the
    /// card takes below the source, to keep the text under it from being covered.
    pub(super) fn live_preview(
        &mut self,
        text: &str,
        head: usize,
        column: (f32, f32),
        source: (f32, f32),
    ) -> f32 {
        let (x, width) = column;
        let (top, bottom) = source;
        let Some(formula) = source::formula_at(text, head) else {
            return 0.0;
        };
        let em = BODY_SIZE * DISPLAY_SCALE;
        let key = self.math_key(&formula.text, formula.display, em);
        let inner = (width - 2.0 * PREVIEW_PAD).max(1.0);
        enum Body {
            Formula(TextureId),
            Error(std::sync::Arc<egui::Galley>),
        }
        let (body, w, h) = match self.pictures.math_preview(&key) {
            MathPreview::Ready {
                texture,
                width: w,
                ascent,
                descent,
            } => {
                let fit = (inner / w.max(1.0)).min(1.0);
                (Body::Formula(texture), w * fit, (ascent + descent) * fit)
            }
            MathPreview::Loading => return 0.0,
            MathPreview::Error(why) => {
                let size = BODY_SIZE * PREVIEW_ERROR_SCALE;
                let galley = self.painter.layout(
                    format!("Cannot show this formula: {why}"),
                    egui::FontId::new(size * self.zoom, egui::FontFamily::Proportional),
                    self.tokens.text_muted,
                    inner * self.zoom,
                );
                let (w, h) = (galley.size().x / self.zoom, galley.size().y / self.zoom);
                (Body::Error(galley), w, h)
            }
        };
        let card_h = h + 2.0 * PREVIEW_PAD;
        let card_w = (w + 2.0 * PREVIEW_PAD).min(width);
        let (card_top, below) =
            preview_top(top, bottom, card_h, notecraft_canvas::math::MAX_PAGE_UNITS);
        let card = Rect::from_min_size(Pos2::new(x, card_top), egui::vec2(card_w, card_h));
        self.items.push(Item::Fill {
            rect: card,
            color: mix(self.tokens.canvas_bg, self.tokens.text, 0.05),
        });
        let corners = [
            card.left_top(),
            card.right_top(),
            card.right_bottom(),
            card.left_bottom(),
        ];
        for (from, to) in corners.iter().zip(corners.iter().cycle().skip(1)) {
            self.items.push(Item::Line {
                from: *from,
                to: *to,
                color: self.tokens.border,
            });
        }
        let inside = Pos2::new(card.min.x + PREVIEW_PAD, card.min.y + PREVIEW_PAD);
        match body {
            Body::Formula(texture) => self.items.push(Item::Math {
                rect: Rect::from_min_size(inside, egui::vec2(w, h)),
                texture,
            }),
            Body::Error(galley) => self.items.push(Item::Text {
                pos: inside,
                galley,
                bold: None,
            }),
        }
        if below {
            PREVIEW_GAP + card_h
        } else {
            0.0
        }
    }

    fn math_key(&self, src: &str, display: bool, em: f32) -> MathKey {
        let ppp = self.painter.ctx().pixels_per_point();
        MathKey {
            src: src.to_owned(),
            display,
            em_bits: em.to_bits(),
            rgb: rgb_of(self.tokens.text),
            scale: (self.zoom * ppp).ceil().clamp(1.0, 4.0) as u32,
        }
    }

    /// Typeset the inline formulas in `runs` (text of `size` page units) and remember the ones
    /// that are ready, for [`Layout::job`] to reserve room for.
    pub(super) fn prepare_inline_math(&mut self, runs: &[Run], size: f32) {
        for run in runs.iter().filter(|r| r.math == Math::Inline) {
            let key = (run.text.clone(), size.to_bits());
            if self.inline_math.contains_key(&key) {
                continue;
            }
            let shown = self.pictures.math(&self.math_key(&run.text, false, size));
            if let MathShown::Ready {
                texture,
                width,
                ascent,
                descent,
            } = shown
            {
                self.inline_math.insert(
                    key,
                    InlineMath {
                        texture,
                        width,
                        ascent,
                        descent,
                    },
                );
            }
        }
    }

    /// The formulas of `runs` that were laid out as placeholders, in order.
    fn ready_inline(&self, runs: &[Run], size: f32) -> Vec<InlineMath> {
        runs.iter()
            .filter(|r| r.math == Math::Inline)
            .filter_map(|r| self.inline_math.get(&(r.text.clone(), size.to_bits())))
            .copied()
            .collect()
    }

    /// Draw the inline formulas over the room the galley kept for them. `at` is the galley's
    /// position in page units.
    pub(super) fn place_inline_math(
        &mut self,
        runs: &[Run],
        size: f32,
        galley: &egui::Galley,
        at: Pos2,
    ) {
        let ready = self.ready_inline(runs, size);
        if ready.is_empty() {
            return;
        }
        let z = self.zoom.max(0.01);
        let slots = galley
            .rows
            .iter()
            .flat_map(|placed| {
                placed
                    .row
                    .glyphs
                    .iter()
                    .filter(|g| g.chr == PLACEHOLDER)
                    .map(move |g| (placed.pos + g.pos.to_vec2()) / z)
            })
            .collect::<Vec<Pos2>>();
        // A page that has the placeholder character in its own text throws the count off; draw
        // nothing rather than a formula in the wrong place.
        if slots.len() != ready.len() {
            return;
        }
        for (slot, math) in slots.into_iter().zip(ready) {
            let right = at.x + slot.x;
            let baseline = at.y + slot.y;
            let rect = Rect::from_min_size(
                Pos2::new(right - math.width, baseline - math.ascent),
                egui::vec2(math.width, math.ascent + math.descent),
            );
            self.items.push(Item::Math {
                rect,
                texture: math.texture,
            });
        }
    }

    /// A display formula on a line of its own, centered in the column.
    pub(super) fn display_math(&mut self, run: &Run, x: f32, width: f32, y: &mut f32) {
        let em = BODY_SIZE * DISPLAY_SCALE;
        match self.pictures.math(&self.math_key(&run.text, true, em)) {
            MathShown::Ready {
                texture,
                width: w,
                ascent,
                descent,
            } => {
                // Wider than the column: shrunk to fit.
                let fit = (width / w.max(1.0)).min(1.0);
                let (w, h) = (w * fit, (ascent + descent) * fit);
                let pad = BODY_SIZE * 0.4;
                let rect = Rect::from_min_size(
                    Pos2::new(x + (width - w) / 2.0, *y + pad),
                    egui::vec2(w, h),
                );
                self.items.push(Item::Math { rect, texture });
                *y += h + 2.0 * pad + BODY_SIZE * 0.2;
            }
            MathShown::Loading | MathShown::Failed => {
                let mut runs = vec![source_run(run)];
                self.paragraph_text(&mut runs, x, width, y);
            }
        }
    }
}
