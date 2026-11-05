//! Tests for equations in the flow layout.

use egui::{Painter, Pos2, Rect, TextureId};
use notecraft_engine::TextCursor;
use notecraft_model::Container;

use super::math::PLACEHOLDER;
use super::{layout, Flow, Input, Item, COLUMN_WIDTH, LEFT};
use crate::canvas::pictures::{MathKey, MathPreview, MathShown, NoPictures, PictureSource, Shown};
use crate::themes::{Preset, ThemeTokens};

/// Every formula is 40 wide, 12 above the baseline and 4 below.
struct Formulas;

impl PictureSource for Formulas {
    fn picture(&mut self, _link: &str) -> Shown {
        Shown::Failed("no pictures".to_owned())
    }

    fn math(&mut self, _key: &MathKey) -> MathShown {
        MathShown::Ready {
            texture: TextureId::default(),
            width: 40.0,
            ascent: 12.0,
            descent: 4.0,
        }
    }
}

fn lay(
    text: &str,
    zoom: f32,
    cursor: Option<TextCursor>,
    pictures: &mut dyn PictureSource,
) -> Flow {
    let ctx = egui::Context::default();
    let mut out = None;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let painter: &Painter = ui.painter();
        let tokens = ThemeTokens::for_variant(Preset::M365, false);
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
            objects: &[],
        };
        out = Some(layout(painter, &input, zoom, &tokens, pictures));
    });
    output.textures_delta.clear();
    out.expect("ran once")
}

fn maths(flow: &Flow) -> Vec<Rect> {
    flow.items
        .iter()
        .filter_map(|i| match i {
            Item::Math { rect, .. } => Some(*rect),
            _ => None,
        })
        .collect()
}

fn texts(flow: &Flow) -> Vec<String> {
    flow.items
        .iter()
        .filter_map(|i| match i {
            Item::Text { galley, .. } => Some(galley.text().to_owned()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_inline_formula_gets_room_in_the_line_and_is_drawn_there() {
    let flow = lay("alpha $x$ omega\n", 1.0, None, &mut Formulas);
    let rects = maths(&flow);
    assert_eq!(rects.len(), 1);
    let rect = rects[0];
    assert!((rect.width() - 40.0).abs() < 0.01 && (rect.height() - 16.0).abs() < 0.01);
    // The text before it ends at the formula and the text after starts at its right edge.
    let Some(Item::Text { pos, galley, .. }) =
        flow.items.iter().find(|i| matches!(i, Item::Text { .. }))
    else {
        panic!("no text");
    };
    let glyph_x = |c: char| {
        galley.rows.iter().find_map(|r| {
            r.row
                .glyphs
                .iter()
                .find(|g| g.chr == c)
                .map(|g| pos.x + (r.pos.x + g.pos.x))
        })
    };
    let before = glyph_x('a').expect("alpha");
    let after = glyph_x('o').expect("omega");
    assert!(before < rect.min.x);
    assert!(after >= rect.max.x - 0.5, "{after} vs {}", rect.max.x);
    assert!(texts(&flow)[0].contains(PLACEHOLDER));
    assert!(!texts(&flow)[0].contains('$'));
}

#[test]
fn inline_formulas_land_in_the_same_place_at_any_zoom() {
    let one = maths(&lay("alpha $x$ omega\n", 1.0, None, &mut Formulas));
    let two = maths(&lay("alpha $x$ omega\n", 2.0, None, &mut Formulas));
    assert_eq!(one.len(), 1);
    assert_eq!(two.len(), 1);
    assert!((one[0].min.x - two[0].min.x).abs() < 1.5);
    assert!((one[0].min.y - two[0].min.y).abs() < 1.5);
}

#[test]
fn two_formulas_in_one_paragraph_are_both_drawn_in_order() {
    let flow = lay("$a$ and $b$ and more text\n", 1.0, None, &mut Formulas);
    let rects = maths(&flow);
    assert_eq!(rects.len(), 2);
    assert!(rects[0].max.x < rects[1].min.x);
}

#[test]
fn a_display_formula_is_centered_on_its_own_line() {
    let flow = lay("before\n\n$$x$$\n\nafter\n", 1.0, None, &mut Formulas);
    let rects = maths(&flow);
    assert_eq!(rects.len(), 1);
    let rect = rects[0];
    let column = Rect::from_min_size(Pos2::new(LEFT, 0.0), egui::vec2(COLUMN_WIDTH, 1.0));
    assert!((rect.center().x - column.center().x).abs() < 20.0);
    let texts = texts(&flow);
    assert_eq!(texts.len(), 2, "{texts:?}");
    assert!(texts[0].starts_with("before") && texts[1].starts_with("after"));
}

#[test]
fn a_formula_that_is_not_drawn_shows_its_source() {
    let flow = lay("see $x^2$ and\n\n$$y$$\n", 1.0, None, &mut NoPictures);
    assert!(maths(&flow).is_empty());
    let texts = texts(&flow);
    assert!(texts.iter().any(|t| t.contains("$x^2$")), "{texts:?}");
    assert!(texts.iter().any(|t| t.contains("$$y$$")), "{texts:?}");
}

#[test]
fn the_block_with_the_caret_shows_its_source_not_the_formula() {
    let text = "see $x^2$ here\n";
    let caret = TextCursor::caret(text.find('^').expect("caret"));
    let flow = lay(text, 1.0, Some(caret), &mut Formulas);
    assert!(maths(&flow).is_empty());
    assert!(texts(&flow).iter().any(|t| t.contains("$x^2$")));
}

#[test]
fn formulas_in_a_heading_and_a_list_are_drawn() {
    let flow = lay("# Title $x$\n\n- item $y$\n", 1.0, None, &mut Formulas);
    assert_eq!(maths(&flow).len(), 2);
}

#[test]
fn a_page_with_the_placeholder_character_draws_nothing_in_the_wrong_place() {
    let text = format!("one {PLACEHOLDER} two $x$ three\n");
    let flow = lay(&text, 1.0, None, &mut Formulas);
    assert!(maths(&flow).is_empty());
}

/// Previews are 60 wide and 20 high (12 above the baseline, 8 below), or an error.
struct Previews {
    error: Option<&'static str>,
    asked: Vec<String>,
}

impl PictureSource for Previews {
    fn picture(&mut self, _link: &str) -> Shown {
        Shown::Failed("no pictures".to_owned())
    }

    fn math_preview(&mut self, key: &MathKey) -> MathPreview {
        self.asked.push(key.src.clone());
        match self.error {
            Some(why) => MathPreview::Error(why.to_owned()),
            None => MathPreview::Ready {
                texture: TextureId::default(),
                width: 60.0,
                ascent: 12.0,
                descent: 8.0,
            },
        }
    }
}

fn previews() -> Previews {
    Previews {
        error: None,
        asked: Vec::new(),
    }
}

fn source_text(flow: &Flow) -> Option<(Pos2, f32)> {
    flow.items.iter().find_map(|i| match i {
        Item::Text { pos, galley, .. } if galley.text().contains('$') => {
            Some((*pos, galley.size().y / flow.zoom))
        }
        _ => None,
    })
}

fn bottom(flow: &Flow) -> f32 {
    flow.containers
        .iter()
        .map(|c| c.rect.max.y)
        .fold(0.0_f32, f32::max)
}

#[test]
fn the_caret_in_a_formula_shows_a_preview_below_the_source() {
    let text = "see $x^2$ here\n";
    let caret = TextCursor::caret(text.find('^').expect("caret"));
    let mut source = previews();
    let flow = lay(text, 1.0, Some(caret), &mut source);
    assert_eq!(source.asked, vec!["x^2".to_owned()]);
    let rects = maths(&flow);
    assert_eq!(rects.len(), 1, "only the preview is drawn");
    let (pos, height) = source_text(&flow).expect("the source is shown");
    assert!((rects[0].width() - 60.0).abs() < 0.01 && (rects[0].height() - 20.0).abs() < 0.01);
    assert!(rects[0].min.y >= pos.y + height, "below the source line");
    assert!(rects[0].min.x >= pos.x, "inside the column");
    let cards = flow
        .items
        .iter()
        .filter(|i| matches!(i, Item::Fill { .. }))
        .count();
    assert_eq!(cards, 1, "one card behind the formula");
}

#[test]
fn the_preview_makes_room_so_the_text_below_is_not_covered() {
    let text = "see $x^2$ here\n";
    let caret = TextCursor::caret(text.find('^').expect("caret"));
    let with = lay(text, 1.0, Some(caret), &mut previews());
    let without = lay(text, 1.0, Some(caret), &mut Formulas);
    assert!(bottom(&with) >= bottom(&without) + 20.0);
}

#[test]
fn the_caret_outside_the_formula_in_the_same_block_has_no_preview() {
    let text = "see $x^2$ here\n";
    let caret = TextCursor::caret(text.find("here").expect("caret"));
    let mut source = previews();
    let flow = lay(text, 1.0, Some(caret), &mut source);
    assert!(source.asked.is_empty());
    assert!(maths(&flow).is_empty());
}

#[test]
fn a_selection_shows_no_preview() {
    let text = "see $x^2$ here\n";
    let start = text.find('^').expect("caret");
    let mut source = previews();
    let selection = TextCursor {
        anchor: start,
        head: start + 2,
    };
    let flow = lay(text, 1.0, Some(selection), &mut source);
    assert!(source.asked.is_empty());
    assert!(maths(&flow).is_empty());
}

#[test]
fn a_display_formula_previews_while_edited() {
    let text = "$$\\frac{a}{b}$$\n";
    let mut source = previews();
    let flow = lay(text, 1.0, Some(TextCursor::caret(6)), &mut source);
    assert_eq!(source.asked, vec!["\\frac{a}{b}".to_owned()]);
    assert_eq!(maths(&flow).len(), 1);
}

#[test]
fn a_formula_that_does_not_parse_shows_its_error_in_the_preview() {
    let text = "see $\\frac{$ here\n";
    let caret = TextCursor::caret(text.find("frac").expect("caret"));
    let mut source = previews();
    source.error = Some("not valid math");
    let flow = lay(text, 1.0, Some(caret), &mut source);
    assert!(maths(&flow).is_empty());
    let texts = texts(&flow);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("Cannot show this formula: not valid math")),
        "{texts:?}"
    );
}

#[test]
fn a_formula_still_being_typeset_adds_no_card() {
    struct Slow;
    impl PictureSource for Slow {
        fn picture(&mut self, _link: &str) -> Shown {
            Shown::Loading
        }
        fn math_preview(&mut self, _key: &MathKey) -> MathPreview {
            MathPreview::Loading
        }
    }
    let text = "see $x$ here\n";
    let flow = lay(text, 1.0, Some(TextCursor::caret(6)), &mut Slow);
    assert!(maths(&flow).is_empty());
    assert!(!flow.items.iter().any(|i| matches!(i, Item::Fill { .. })));
}

#[test]
fn the_preview_goes_below_unless_the_page_has_no_room() {
    use super::math::preview_top;
    let (top, below) = preview_top(100.0, 120.0, 30.0, 8192.0);
    assert!(below && top >= 120.0);
    let (top, below) = preview_top(8000.0, 8180.0, 30.0, 8192.0);
    assert!(!below && top + 30.0 <= 8000.0);
    let (top, below) = preview_top(5.0, 20.0, 30.0, 40.0);
    assert!(!below && top == 0.0);
}
