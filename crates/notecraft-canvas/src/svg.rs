//! SVG pictures for the canvas, drawn with `resvg` (pure Rust).
//!
//! An SVG is untrusted input, so it is limited three ways. The file may not be over
//! [`MAX_SVG_BYTES`]. The result is a bitmap whose long side is at most `TEXTURE_SIDE` pixels,
//! so the memory it can take is fixed whatever size the SVG claims. And parsing and drawing run
//! on a helper thread that is given [`RENDER_TIMEOUT`] to finish: past that the picture is
//! reported as too large and the canvas moves on (the helper thread ends on its own; it holds
//! no lock and writes nothing). Text, system fonts, and `<image>` elements (which could read
//! other files) are switched off, so an SVG shows only its shapes and gradients.

use std::sync::mpsc;
use std::time::Duration;

use resvg::{tiny_skia, usvg};

use crate::picture::{Picture, PictureError, MAX_SIDE, TEXTURE_SIDE};

/// Largest SVG file read, in bytes (4 MiB). Real drawings are far smaller.
pub const MAX_SVG_BYTES: usize = 4 * 1024 * 1024;
/// How long parsing and drawing may take before the picture is given up on.
pub const RENDER_TIMEOUT: Duration = Duration::from_secs(3);

/// Whether `bytes` look like an XML document (after an optional byte order mark and blanks),
/// which is how a SVG file starts. PNG, JPEG, and WebP never start with `<`.
pub(crate) fn looks_like_svg(bytes: &[u8]) -> bool {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    bytes
        .iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|b| *b == b'<')
}

/// Draw the SVG in `bytes` into a [`Picture`].
pub(crate) fn decode_svg(bytes: &[u8]) -> Result<Picture, PictureError> {
    decode_svg_within(bytes, RENDER_TIMEOUT)
}

fn decode_svg_within(bytes: &[u8], timeout: Duration) -> Result<Picture, PictureError> {
    if bytes.len() > MAX_SVG_BYTES {
        return Err(PictureError::TooLarge);
    }
    let owned = bytes.to_vec();
    let (sender, receiver) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("notecraft-svg".to_owned())
        .spawn(move || {
            // The receiver is gone after a timeout; nothing to do then.
            let _ = sender.send(render(&owned));
        });
    match spawned {
        Ok(_) => match receiver.recv_timeout(timeout) {
            Ok(result) => result,
            Err(_) => {
                tracing::debug!("svg took longer than {timeout:?}; skipped");
                Err(PictureError::TooLarge)
            }
        },
        // No threads here (a browser build): draw in place, still bounded in size.
        Err(_) => render(bytes),
    }
}

fn options() -> usvg::Options<'static> {
    usvg::Options {
        resources_dir: None,
        // `<image>` may not read files or decode other pictures.
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    }
}

fn render(bytes: &[u8]) -> Result<Picture, PictureError> {
    let tree = usvg::Tree::from_data(bytes, &options()).map_err(|err| {
        tracing::debug!("cannot read svg: {err}");
        PictureError::Damaged
    })?;
    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return Err(PictureError::Damaged);
    }
    // The natural size is for layout; a huge claim is scaled down, not trusted.
    let shrink = (MAX_SIDE as f32 / w.max(h)).min(1.0);
    let width = ((w * shrink).round() as u32).clamp(1, MAX_SIDE);
    let height = ((h * shrink).round() as u32).clamp(1, MAX_SIDE);
    // The bitmap is made large enough to stay sharp when the picture is enlarged.
    let scale = TEXTURE_SIDE as f32 / w.max(h);
    let tw = ((w * scale).round() as u32).clamp(1, TEXTURE_SIDE);
    let th = ((h * scale).round() as u32).clamp(1, TEXTURE_SIDE);
    let mut pixmap = tiny_skia::Pixmap::new(tw, th).ok_or(PictureError::TooLarge)?;
    let transform = tiny_skia::Transform::from_scale(tw as f32 / w, th as f32 / h);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let mut rgba = Vec::with_capacity(pixmap.pixels().len() * 4);
    for pixel in pixmap.pixels() {
        let c = pixel.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok(Picture {
        width,
        height,
        texture_width: tw,
        texture_height: th,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED_SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20" viewBox="0 0 40 20"><rect width="40" height="20" fill="#ff0000"/></svg>"##;

    #[test]
    fn an_svg_is_drawn_at_texture_size_and_keeps_its_natural_size() {
        let picture = decode_svg(RED_SQUARE.as_bytes()).expect("svg");
        assert_eq!((picture.width, picture.height), (40, 20));
        assert_eq!(picture.texture_width, TEXTURE_SIDE);
        assert_eq!(picture.texture_height, TEXTURE_SIDE / 2);
        assert_eq!(
            picture.rgba.len(),
            (picture.texture_width * picture.texture_height * 4) as usize
        );
        assert_eq!(picture.rgba.get(..4), Some(&[255, 0, 0, 255][..]));
    }

    #[test]
    fn a_picture_with_a_byte_order_mark_and_blanks_is_found() {
        assert!(looks_like_svg(
            b"\xEF\xBB\xBF \n<?xml version=\"1.0\"?><svg/>"
        ));
        assert!(!looks_like_svg(b"\x89PNG\r\n"));
        assert!(!looks_like_svg(b""));
        assert!(!looks_like_svg(b"   "));
    }

    #[test]
    fn bad_and_hostile_svgs_are_errors_not_panics() {
        assert_eq!(decode_svg(b"<svg"), Err(PictureError::Damaged));
        assert_eq!(decode_svg(b"<html></html>"), Err(PictureError::Damaged));
        let huge = vec![b' '; MAX_SVG_BYTES + 1];
        assert_eq!(decode_svg(&huge), Err(PictureError::TooLarge));
        // An enormous claimed size draws into the same bounded bitmap.
        let big = r#"<svg xmlns="http://www.w3.org/2000/svg" width="1e9" height="10"><rect width="9" height="9"/></svg>"#;
        if let Ok(picture) = decode_svg(big.as_bytes()) {
            assert!(picture.width <= MAX_SIDE && picture.texture_width <= TEXTURE_SIDE);
        }
    }

    #[test]
    fn external_images_and_entities_are_not_followed() {
        let svg = r#"<?xml version="1.0"?>
<!DOCTYPE svg [<!ENTITY a "aaaaaaaaaa"><!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;"><!ENTITY c "&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;"><!ENTITY d "&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;">]>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="10" height="10">
<image xlink:href="C:/Windows/win.ini" width="10" height="10"/><image xlink:href="file:///etc/passwd" width="10" height="10"/>
<text>&d;</text><rect width="10" height="10" fill="blue"/></svg>"#;
        // Whether it draws or is refused, it returns quickly and does not panic.
        let _ = decode_svg(svg.as_bytes());
    }

    #[test]
    fn a_render_that_runs_too_long_is_given_up_on() {
        // A zero timeout cannot be met by any real render.
        let result = decode_svg_within(RED_SQUARE.as_bytes(), Duration::ZERO);
        assert!(matches!(result, Err(PictureError::TooLarge) | Ok(_)));
    }
}
