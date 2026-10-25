//! Equations (PR 2.7): `$...$` inline and `$$...$$` display LaTeX math (docs/markdown.md
//! section 3.2).
//!
//! The markdown parser finds the formulas (comrak's dollar math); this module bounds them,
//! writes them back, and typesets them with `latex-rust` (pure Rust, TeX-style layout with an
//! embedded OpenType math font; docs/architecture.md records the choice).
//!
//! A formula is untrusted input. It may not be over [`MAX_FORMULA_BYTES`], and the parser
//! stops at [`MAX_NESTING_DEPTH`] levels of nesting. A formula past either limit, or that
//! does not typeset, is never an error for the page: it stays in the text as its source and
//! the canvas shows that source.

use std::sync::OnceLock;

use latex_rust::{
    layout_with_em_size_pt, parse_with_options, Color, Dim, MathFont, MathStyle, ParseOptions,
    SvgOptions,
};

use crate::block::{Math, Run};

/// Longest formula source kept as a formula, in bytes (4 KiB).
pub const MAX_FORMULA_BYTES: usize = 4096;
/// Deepest nesting of braces, groups, and fractions typeset.
pub const MAX_NESTING_DEPTH: usize = 32;
/// Largest em size typeset, in pixels. Larger values are clamped.
pub const MAX_EM_PX: f32 = 512.0;

/// The source of a formula as it is written in a page: `$src$` or `$$src$$`.
pub fn wrap(src: &str, math: Math) -> String {
    match math {
        Math::None => src.to_owned(),
        Math::Inline => format!("${src}$"),
        Math::Display => format!("$${src}$$"),
    }
}

/// The run for a formula the parser found. A formula over the limit, or empty, is kept as
/// plain text including its dollar signs.
pub(crate) fn run_of(literal: &str, display: bool) -> Run {
    let math = if display { Math::Display } else { Math::Inline };
    if literal.len() > MAX_FORMULA_BYTES || literal.trim().is_empty() {
        return Run::plain(wrap(literal, math));
    }
    Run {
        text: literal.to_owned(),
        math,
        ..Run::default()
    }
}

/// Why a formula could not be typeset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MathError {
    /// Empty, or over [`MAX_FORMULA_BYTES`].
    OutOfBounds,
    /// The formula is not valid LaTeX math, or uses something the layout does not support.
    Invalid(String),
}

impl std::fmt::Display for MathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfBounds => f.write_str("the formula is empty or too long"),
            Self::Invalid(why) => write!(f, "cannot typeset the formula: {why}"),
        }
    }
}

impl std::error::Error for MathError {}

/// A typeset formula: an SVG document and its size in units of the em.
#[derive(Clone, Debug, PartialEq)]
pub struct Typeset {
    /// A self-contained SVG made of paths and rectangles only (no text, images, or scripts),
    /// `width_em * em_px` pixels wide.
    pub svg: String,
    pub width_em: f32,
    /// Height above the baseline.
    pub height_em: f32,
    /// Depth below the baseline.
    pub depth_em: f32,
}

fn font() -> Result<&'static MathFont, MathError> {
    static FONT: OnceLock<Result<MathFont, String>> = OnceLock::new();
    FONT.get_or_init(|| MathFont::stix_two_math().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|why| MathError::Invalid(why.clone()))
}

fn em(dim: &Dim) -> f32 {
    match dim.as_ratio() {
        Some((n, d)) if d > 0 => {
            let v = n as f64 / d as f64;
            if v.is_finite() {
                v.clamp(0.0, 1.0e6) as f32
            } else {
                0.0
            }
        }
        _ => 0.0,
    }
}

/// Typeset `src` with an em of `em_px` pixels, filled with `rgb` (0xRRGGBB). `display` picks
/// the roomier display style used for `$$...$$`.
///
/// Unbounded work is cut off by the limits above, but a hostile formula can still be slow to
/// lay out; callers that must stay responsive run this on a helper thread with a time limit
/// (the canvas does).
pub fn typeset(src: &str, display: bool, em_px: f32, rgb: u32) -> Result<Typeset, MathError> {
    if src.len() > MAX_FORMULA_BYTES || src.trim().is_empty() {
        return Err(MathError::OutOfBounds);
    }
    let em_px = if em_px.is_finite() {
        em_px.clamp(1.0, MAX_EM_PX)
    } else {
        16.0
    };
    // CSS pixels to points, to hundredths.
    let em_pt = Dim::ratio((f64::from(em_px) * 75.0).round() as i64, 100);
    let font = font()?;
    let (ast, _colors) =
        parse_with_options(src, &ParseOptions::new().with_max_depth(MAX_NESTING_DEPTH))
            .map_err(|e| MathError::Invalid(e.to_string()))?;
    let style = if display {
        MathStyle::Display
    } else {
        MathStyle::Text
    };
    let tree = layout_with_em_size_pt(&ast, font, style, &em_pt)
        .map_err(|e| MathError::Invalid(e.to_string()))?;
    let options = SvgOptions {
        font_size_pt: em_pt,
        color: Color::rgb(
            ((rgb >> 16) & 0xff) as u8,
            ((rgb >> 8) & 0xff) as u8,
            (rgb & 0xff) as u8,
        ),
        display,
    };
    let svg = latex_rust::render_svg(&tree, font, &options)
        .map_err(|e| MathError::Invalid(e.to_string()))?;
    Ok(Typeset {
        svg,
        width_em: em(&tree.width),
        height_em: em(&tree.height),
        depth_em: em(&tree.depth),
    })
}
