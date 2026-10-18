//! Picture decoding for the canvas: PNG, JPEG, WebP, and SVG files become RGBA pixels, bounded so
//! a hostile or huge file never exhausts memory or the GPU (docs/markdown.md section 9).
//!
//! A picture is checked before it is decoded: the file may not be over [`MAX_FILE_BYTES`], and
//! the size in its header may not pass [`MAX_SIDE`] or [`MAX_PIXELS`]. A picture bigger than
//! [`TEXTURE_SIDE`] on its long side is shrunk for the texture, so zoomed-out pages stay
//! cheap; the natural size is kept for layout. Nothing here panics: every failure is a
//! [`PictureError`] the canvas shows as a placeholder.

use std::fmt;
use std::fs::File;
use std::io::{self, BufReader, Cursor, Read};
use std::path::Path;

use image::{ImageFormat, ImageReader, Limits};

/// Largest picture file read, in bytes (32 MiB).
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
/// Widest or tallest picture decoded, in pixels.
pub const MAX_SIDE: u32 = 16_384;
/// Most pixels in a decoded picture (36 million, about 144 MiB as RGBA).
pub const MAX_PIXELS: u64 = 36_000_000;
/// Longest side of the texture made for a picture, in pixels.
pub const TEXTURE_SIDE: u32 = 2048;

/// Why a picture could not be shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PictureError {
    /// The file is not there or cannot be read.
    Missing,
    /// The file or the picture in it is over a limit.
    TooLarge,
    /// The file is not a PNG, JPEG, WebP, or SVG picture.
    Unsupported,
    /// The picture is damaged.
    Damaged,
}

impl fmt::Display for PictureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Missing => "the file is missing",
            Self::TooLarge => "the picture is too large",
            Self::Unsupported => "not a PNG, JPEG, WebP, or SVG picture",
            Self::Damaged => "the picture is damaged",
        })
    }
}

impl std::error::Error for PictureError {}

/// A decoded picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    /// The size of the picture in the file, in pixels.
    pub width: u32,
    pub height: u32,
    /// The size of `rgba`, in pixels: the picture shrunk to fit [`TEXTURE_SIDE`].
    pub texture_width: u32,
    pub texture_height: u32,
    /// `texture_width * texture_height` pixels, four bytes each, not premultiplied.
    pub rgba: Vec<u8>,
}

/// Read and decode the picture file at `path`.
pub fn load_picture(path: &Path) -> Result<Picture, PictureError> {
    let file = File::open(path).map_err(|_| PictureError::Missing)?;
    let meta = file.metadata().map_err(|_| PictureError::Missing)?;
    if !meta.is_file() {
        return Err(PictureError::Missing);
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(PictureError::TooLarge);
    }
    let mut bytes = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0));
    BufReader::new(file)
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|err: io::Error| {
            tracing::debug!("cannot read picture: {err}");
            PictureError::Missing
        })?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(PictureError::TooLarge);
    }
    decode_picture(&bytes)
}

/// Decode `bytes` as a PNG, JPEG, WebP, or SVG picture within the limits.
pub fn decode_picture(bytes: &[u8]) -> Result<Picture, PictureError> {
    decode_with(bytes, MAX_SIDE, MAX_PIXELS)
}

fn decode_with(bytes: &[u8], max_side: u32, max_pixels: u64) -> Result<Picture, PictureError> {
    // An SVG is drawn, not decoded: see the svg module for its own limits.
    if crate::svg::looks_like_svg(bytes) {
        return crate::svg::decode_svg(bytes);
    }
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| PictureError::Unsupported)?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP)
    ) {
        return Err(PictureError::Unsupported);
    }
    // The header alone says how big the picture is; check before decoding anything.
    let (width, height) = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| PictureError::Unsupported)?
        .into_dimensions()
        .map_err(|_| PictureError::Damaged)?;
    let pixels = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 {
        return Err(PictureError::Damaged);
    }
    if width > max_side || height > max_side || pixels > max_pixels {
        return Err(PictureError::TooLarge);
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(max_side);
    limits.max_image_height = Some(max_side);
    limits.max_alloc = Some(max_pixels.saturating_mul(8));
    let mut reader = reader;
    reader.limits(limits);
    let decoded = reader.decode().map_err(|err| {
        tracing::debug!("cannot decode picture: {err}");
        PictureError::Damaged
    })?;
    let long = width.max(height);
    let image = if long > TEXTURE_SIDE {
        let scale = f64::from(TEXTURE_SIDE) / f64::from(long);
        let w = ((f64::from(width) * scale).round() as u32).max(1);
        let h = ((f64::from(height) * scale).round() as u32).max(1);
        decoded.thumbnail_exact(w, h)
    } else {
        decoded
    };
    let rgba = image.to_rgba8();
    Ok(Picture {
        width,
        height,
        texture_width: rgba.width(),
        texture_height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use image::{ImageFormat, Rgba, RgbaImage};

    use super::*;

    fn encode(width: u32, height: u32, format: ImageFormat) -> Vec<u8> {
        let mut image = RgbaImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255]);
        }
        let mut out = Cursor::new(Vec::new());
        // JPEG has no alpha channel.
        let result = if format == ImageFormat::Jpeg {
            image::DynamicImage::ImageRgba8(image)
                .to_rgb8()
                .write_to(&mut out, format)
        } else {
            image.write_to(&mut out, format)
        };
        result.expect("encode");
        out.into_inner()
    }

    #[test]
    fn png_jpeg_and_webp_decode_to_rgba() {
        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
            let picture = decode_picture(&encode(40, 20, format)).expect("decode");
            assert_eq!((picture.width, picture.height), (40, 20), "{format:?}");
            assert_eq!((picture.texture_width, picture.texture_height), (40, 20));
            assert_eq!(picture.rgba.len(), 40 * 20 * 4);
        }
        // Pixels survive a lossless format.
        let picture = decode_picture(&encode(4, 4, ImageFormat::Png)).expect("png");
        assert_eq!(picture.rgba.get(4..8), Some(&[1, 0, 128, 255][..]));
    }

    #[test]
    fn big_pictures_are_shrunk_for_the_texture_but_keep_their_size() {
        let picture = decode_picture(&encode(5000, 100, ImageFormat::Png)).expect("decode");
        assert_eq!((picture.width, picture.height), (5000, 100));
        assert_eq!(picture.texture_width, TEXTURE_SIDE);
        assert_eq!(picture.texture_height, 41);
        assert_eq!(
            picture.rgba.len(),
            (picture.texture_width * picture.texture_height * 4) as usize
        );
    }

    #[test]
    fn pictures_over_a_limit_are_refused_before_decoding() {
        let png = encode(64, 64, ImageFormat::Png);
        assert_eq!(
            decode_with(&png, 32, MAX_PIXELS),
            Err(PictureError::TooLarge)
        );
        assert_eq!(
            decode_with(&png, MAX_SIDE, 1000),
            Err(PictureError::TooLarge)
        );
        assert!(decode_with(&png, 64, 64 * 64).is_ok());
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        assert_eq!(decode_picture(b""), Err(PictureError::Unsupported));
        assert_eq!(
            decode_picture(b"hello world"),
            Err(PictureError::Unsupported)
        );
        assert_eq!(
            decode_picture(b"GIF89a\x01\x00\x01\x00"),
            Err(PictureError::Unsupported)
        );
        // A real header with the data cut off, and with garbage in the middle.
        let png = encode(32, 32, ImageFormat::Png);
        let cut = png.get(..png.len() / 2).expect("half");
        assert_eq!(decode_picture(cut), Err(PictureError::Damaged));
        let mut noisy = png.clone();
        for byte in noisy.iter_mut().skip(40).take(200) {
            *byte = !*byte;
        }
        assert!(decode_picture(&noisy).is_err());
        let jpeg = encode(32, 32, ImageFormat::Jpeg);
        assert!(decode_picture(jpeg.get(..jpeg.len() / 2).expect("half")).is_err());
    }

    #[test]
    fn files_are_checked_for_size_and_existence() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let good = dir.path().join("a.png");
        std::fs::write(&good, encode(8, 8, ImageFormat::Png)).expect("write");
        assert_eq!(load_picture(&good).expect("load").width, 8);
        assert_eq!(
            load_picture(&dir.path().join("none.png")),
            Err(PictureError::Missing)
        );
        assert_eq!(load_picture(dir.path()), Err(PictureError::Missing));
        let corrupt = dir.path().join("b.png");
        std::fs::write(&corrupt, b"\x89PNG\r\n\x1a\nnot really").expect("write");
        assert!(load_picture(&corrupt).is_err());
    }
}
