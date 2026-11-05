//! Tests for equations in the flow layout.

use egui::{Painter, Pos2, Rect, TextureId};
use notecraft_engine::TextCursor;
use notecraft_model::Container;

use super::math::PLACEHOLDER;
use super::{layout, Flow, Input, Item, COLUMN_WIDTH, LEFT};
use crate::canvas::pictures::{MathKey, MathShown, NoPictures, PictureSource, Shown};
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
            tags: None,
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
