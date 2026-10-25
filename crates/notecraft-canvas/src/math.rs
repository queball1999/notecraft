//! Equations on the canvas (PR 2.7): a formula is typeset by `notecraft_markdown::math` and
//! drawn into a small bitmap with `resvg` (pure Rust), the same way SVG pictures are.
//!
//! A formula is untrusted input, so it is limited like an SVG: the source is bounded by the
//! markdown layer (4 KiB, 32 levels), the bitmap is at most [`MAX_SIDE`] pixels on a side, and
//! typesetting plus drawing run on a helper thread that is given [`RENDER_TIMEOUT`] to finish.
//! Past that the formula is reported as too slow and the canvas shows its source text; the
//! helper thread ends on its own (it holds no lock and writes nothing).

use std::fmt;
use std::sync::mpsc;
use std::time::Duration;

use notecraft_markdown::math::{self, MathError};
use resvg::{tiny_skia, usvg};

/// Longest side of a formula bitmap, in pixels.
pub const MAX_SIDE: u32 = 2048;
/// Widest or tallest formula drawn, in page units. Bigger formulas are not drawn.
pub const MAX_PAGE_UNITS: f32 = 8192.0;
/// How long typesetting and drawing may take. Debug builds (tests included) get far longer:
/// unoptimized code is many times slower. The limit is still there, so a runaway formula ends.
pub const RENDER_TIMEOUT: Duration = if cfg!(debug_assertions) {
    Duration::from_secs(60)
} else {
    Duration::from_secs(2)
};

/// Why a formula could not be drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MathDrawError {
    /// Not valid math, or empty or too long. The text is a short plain reason.
    Invalid(String),
    /// Typesetting took too long, or the result is past the size limit.
    TooLarge,
}

impl fmt::Display for MathDrawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(why) => f.write_str(why),
            Self::TooLarge => f.write_str("the formula is too large or too slow to typeset"),
        }
    }
}

impl std::error::Error for MathDrawError {}

/// A drawn formula. Sizes are in page units (pixels at 100%).
#[derive(Clone, Debug, PartialEq)]
pub struct MathImage {
    pub width: f32,
    /// Height above the baseline.
    pub ascent: f32,
    /// Depth below the baseline.
    pub descent: f32,
    /// The size of `rgba`, in pixels.
    pub texture_width: u32,
    pub texture_height: u32,
    /// `texture_width * texture_height` pixels, four bytes each, not premultiplied.
    pub rgba: Vec<u8>,
}

/// Typeset `src` with an em of `em` page units, filled with `rgb` (0xRRGGBB), and draw it with
/// `scale` bitmap pixels per page unit (at least 1; reduced to stay within [`MAX_SIDE`]).
pub fn draw(
    src: &str,
    display: bool,
    em: f32,
    rgb: u32,
    scale: f32,
) -> Result<MathImage, MathDrawError> {
    draw_within(src, display, em, rgb, scale, RENDER_TIMEOUT)
}

fn draw_within(
    src: &str,
    display: bool,
    em: f32,
    rgb: u32,
    scale: f32,
    timeout: Duration,
) -> Result<MathImage, MathDrawError> {
    let owned = src.to_owned();
    let (sender, receiver) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("notecraft-math".to_owned())
        .spawn(move || {
            // The receiver is gone after a timeout; nothing to do then.
            let _ = sender.send(render(&owned, display, em, rgb, scale));
        });
    match spawned {
        Ok(_) => match receiver.recv_timeout(timeout) {
            Ok(result) => result,
            Err(_) => {
                tracing::debug!("formula took longer than {timeout:?}; skipped");
                Err(MathDrawError::TooLarge)
            }
        },
        // No threads here (a browser build): draw in place, still bounded in size.
        Err(_) => render(src, display, em, rgb, scale),
    }
}

fn render(
    src: &str,
    display: bool,
    em: f32,
    rgb: u32,
    scale: f32,
) -> Result<MathImage, MathDrawError> {
    let typeset = math::typeset(src, display, em, rgb).map_err(|err| match err {
        MathError::OutOfBounds => MathDrawError::Invalid(err.to_string()),
        MathError::Invalid(_) => MathDrawError::Invalid("not valid math".to_owned()),
    })?;
    let em = em.clamp(1.0, math::MAX_EM_PX);
    let (width, ascent, descent) = (
        typeset.width_em * em,
        typeset.height_em * em,
        typeset.depth_em * em,
    );
    let total = ascent + descent;
    if !(width.is_finite() && total.is_finite()) || width <= 0.0 || total <= 0.0 {
        return Err(MathDrawError::Invalid("the formula has no size".to_owned()));
    }
    if width > MAX_PAGE_UNITS || total > MAX_PAGE_UNITS {
        return Err(MathDrawError::TooLarge);
    }
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(&typeset.svg, &options).map_err(|err| {
        tracing::debug!("cannot read typeset formula: {err}");
        MathDrawError::Invalid("not valid math".to_owned())
    })?;
    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return Err(MathDrawError::Invalid("the formula has no size".to_owned()));
    }
    // Pixels per SVG unit, kept inside the bitmap limit.
    let wanted = if scale.is_finite() {
        scale.clamp(1.0, 8.0)
    } else {
        1.0
    };
    let k = wanted.min(MAX_SIDE as f32 / w.max(h));
    let tw = ((w * k).ceil() as u32).clamp(1, MAX_SIDE);
    let th = ((h * k).ceil() as u32).clamp(1, MAX_SIDE);
    let mut pixmap = tiny_skia::Pixmap::new(tw, th).ok_or(MathDrawError::TooLarge)?;
    let transform = tiny_skia::Transform::from_scale(tw as f32 / w, th as f32 / h);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let mut rgba = Vec::with_capacity(pixmap.pixels().len() * 4);
    for pixel in pixmap.pixels() {
        let c = pixel.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok(MathImage {
        width,
        ascent,
        descent,
        texture_width: tw,
        texture_height: th,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_formula_is_drawn_in_the_color_asked_for() {
        let image = draw("x^2 + y^2 = z^2", false, 16.0, 0xFF0000, 2.0).expect("draw");
        assert!(image.width > 20.0 && image.ascent > 5.0);
        assert_eq!(
            image.rgba.len(),
            (image.texture_width * image.texture_height * 4) as usize
        );
        // Something is inked, and everything inked is red.
        let inked: Vec<&[u8]> = image.rgba.chunks(4).filter(|p| p[3] > 200).collect();
        assert!(!inked.is_empty());
        assert!(inked.iter().all(|p| p[0] > 200 && p[1] < 40 && p[2] < 40));
    }

    #[test]
    fn the_bitmap_stays_within_the_side_limit() {
        let wide = "x+".repeat(60) + "x";
        let image = draw(&wide, false, 40.0, 0, 8.0).expect("draw");
        assert!(image.texture_width <= MAX_SIDE && image.texture_height <= MAX_SIDE);
    }

    #[test]
    fn bad_formulas_are_errors_not_panics() {
        for src in [
            "",
            "   ",
            "\\frac{",
            "\\unknown",
            "}{",
            "^",
            "_",
            "\\\\\\\\",
        ] {
            let _ = draw(src, false, 16.0, 0, 1.0);
        }
        assert!(matches!(
            draw("\\frac{", false, 16.0, 0, 1.0),
            Err(MathDrawError::Invalid(_))
        ));
        let long = "x".repeat(math::MAX_FORMULA_BYTES + 1);
        assert!(matches!(
            draw(&long, false, 16.0, 0, 1.0),
            Err(MathDrawError::Invalid(_))
        ));
    }

    #[test]
    fn a_huge_space_is_refused_not_drawn() {
        let result = draw("\\hspace{100000em}x", false, 16.0, 0, 1.0);
        assert!(matches!(
            result,
            Err(MathDrawError::TooLarge | MathDrawError::Invalid(_))
        ));
    }

    #[test]
    fn a_render_that_runs_too_long_is_given_up_on() {
        let result = draw_within("x", false, 16.0, 0, 1.0, Duration::ZERO);
        assert!(matches!(result, Err(MathDrawError::TooLarge) | Ok(_)));
    }
}
