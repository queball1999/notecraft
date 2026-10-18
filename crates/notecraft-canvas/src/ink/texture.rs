//! Procedural textures for the textured pens and the pencil. Every texture is a small image
//! computed from integer hashes and smooth noise, so it is the same on every machine and in
//! every run, and nothing is stored in a page or bundled with the app.
//!
//! Two kinds of image:
//!
//! * a stroke texture follows the stroke: its horizontal axis is the arc length along the
//!   centerline (one repeat per `period`) and its vertical axis runs across the stroke, edge
//!   to edge. Rainbow, galaxy, lava, ocean, and the metals are these.
//! * a page texture is a seamless tile pinned to the page: the pencil's grain. The pencil
//!   draws a stroke's color through it.
//!
//! The front end uploads [`image`] once per texture, then gives each mesh vertex the
//! coordinates from [`uv`]. The paint cost of a stroke is the same as a plain one.

use std::f32::consts::TAU;

use notecraft_core::{InkTexture, Vec2};

/// Where a texture's coordinates come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// Along and across the stroke.
    Stroke,
    /// The page position, repeating.
    Page,
}

/// The size of a texture's image and how it maps onto a stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    pub width: usize,
    pub height: usize,
    pub anchor: Anchor,
    /// Page units one image width covers, per unit of stroke width (a stroke texture).
    ratio: f32,
    /// The least page units one image width covers (a stroke texture), or the tile size in
    /// page units (a page texture).
    min_period: f32,
    /// Whether the stroke's own color tints the image (the pencil).
    pub tinted: bool,
}

/// The pencil's grain tile covers this many page units each way.
const PENCIL_TILE: f32 = 96.0;

/// How a texture maps onto strokes, or `None` for a plain stroke.
pub fn spec(texture: InkTexture) -> Option<Spec> {
    let stroke = |width, height, ratio, min_period| Spec {
        width,
        height,
        anchor: Anchor::Stroke,
        ratio,
        min_period,
        tinted: false,
    };
    Some(match texture {
        InkTexture::Plain => return None,
        InkTexture::Pencil => Spec {
            width: 128,
            height: 128,
            anchor: Anchor::Page,
            ratio: 0.0,
            min_period: PENCIL_TILE,
            tinted: true,
        },
        InkTexture::Rainbow => stroke(256, 32, 20.0, 260.0),
        InkTexture::Galaxy => stroke(512, 64, 8.0, 160.0),
        InkTexture::Lava => stroke(512, 64, 8.0, 160.0),
        InkTexture::Ocean => stroke(384, 48, 8.0, 160.0),
        InkTexture::Gold | InkTexture::Silver | InkTexture::Bronze | InkTexture::RoseGold => {
            stroke(256, 48, 12.0, 220.0)
        }
    })
}

/// A texture's image: `width * height` RGBA pixels, not premultiplied, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// The image of `texture`, or `None` for a plain stroke. Deterministic.
pub fn image(texture: InkTexture) -> Option<TextureImage> {
    let spec = spec(texture)?;
    let mut rgba = Vec::with_capacity(spec.width * spec.height * 4);
    for y in 0..spec.height {
        for x in 0..spec.width {
            rgba.extend_from_slice(&texel(texture, x, y));
        }
    }
    Some(TextureImage {
        width: spec.width,
        height: spec.height,
        rgba,
    })
}

/// The image coordinates (0 to 1 each way) of a point of a stroke of `stroke_width` page
/// units: `along` and `across` are the mesh vertex's (see `StrokeMesh`), `pos` its page
/// position. A stroke texture repeats along the stroke; across is kept half a pixel inside
/// the image so the two edges never blend. Both coordinates are finite for any input.
pub fn uv(texture: InkTexture, stroke_width: f32, along: f32, across: f32, pos: Vec2) -> [f32; 2] {
    let Some(spec) = spec(texture) else {
        return [0.0, 0.0];
    };
    let clean = |v: f32| if v.is_finite() { v } else { 0.0 };
    match spec.anchor {
        Anchor::Page => [
            clean(pos.x) / spec.min_period,
            clean(pos.y) / spec.min_period,
        ],
        Anchor::Stroke => {
            let period = (clean(stroke_width).max(0.0) * spec.ratio).max(spec.min_period);
            let edge = 0.5 / spec.height as f32;
            let across = clean(across).clamp(-1.0, 1.0);
            [
                clean(along) / period,
                edge + (1.0 - 2.0 * edge) * (across * 0.5 + 0.5),
            ]
        }
    }
}

/// The color of the texture at image coordinates `(u, v)`, nearest pixel, wrapping. For
/// tests and for anything that needs a color without the GPU.
pub fn sample(texture: InkTexture, u: f32, v: f32) -> [u8; 4] {
    let Some(spec) = spec(texture) else {
        return [0, 0, 0, 0];
    };
    let wrap = |t: f32, n: usize| {
        let t = if t.is_finite() { t } else { 0.0 };
        ((t - t.floor()) * n as f32) as usize % n.max(1)
    };
    texel(texture, wrap(u, spec.width), wrap(v, spec.height))
}

/// One pixel of the image of `texture`.
pub fn texel(texture: InkTexture, x: usize, y: usize) -> [u8; 4] {
    let Some(spec) = spec(texture) else {
        return [0, 0, 0, 0];
    };
    let (w, h) = (spec.width, spec.height);
    let u = (x % w) as f32 / w as f32 + 0.5 / w as f32;
    let v = (y % h) as f32 / h as f32 + 0.5 / h as f32;
    let across = v * 2.0 - 1.0;
    let (rgb, alpha) = match texture {
        InkTexture::Plain => ([0.0; 3], 0.0),
        InkTexture::Pencil => ([1.0; 3], pencil_alpha(u, v)),
        InkTexture::Rainbow => (rainbow(u, across), 1.0),
        InkTexture::Galaxy => (galaxy(u, v, x % w, y % h, (w, h)), 1.0),
        InkTexture::Lava => (lava(u, v, across), 1.0),
        InkTexture::Ocean => (ocean(u, v, across), 1.0),
        InkTexture::Gold => (metal(METAL_GOLD, u, across, x % w, y % h), 1.0),
        InkTexture::Silver => (metal(METAL_SILVER, u, across, x % w, y % h), 1.0),
        InkTexture::Bronze => (metal(METAL_BRONZE, u, across, x % w, y % h), 1.0),
        InkTexture::RoseGold => (metal(METAL_ROSE, u, across, x % w, y % h), 1.0),
    };
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(rgb[0]), byte(rgb[1]), byte(rgb[2]), byte(alpha)]
}

// Noise ---------------------------------------------------------------------------------

/// A hash of a lattice point and a seed, mixed well enough for noise.
fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x85EB_CA6B)
        ^ seed.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// A hash as a number from 0 up to (not including) 1.
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [mix(a[0], b[0], t), mix(a[1], b[1], t), mix(a[2], b[2], t)]
}

/// Value noise on a `cx` by `cy` lattice that wraps in both directions, at `(u, v)` in 0 to 1.
fn noise(u: f32, v: f32, cx: i32, cy: i32, seed: u32) -> f32 {
    let (x, y) = (u * cx as f32, v * cy as f32);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (smooth(x - x0), smooth(y - y0));
    let at = |i: f32, j: f32| unit(hash((i as i32).rem_euclid(cx), (j as i32).rem_euclid(cy), seed));
    let top = mix(at(x0, y0), at(x0 + 1.0, y0), fx);
    let bottom = mix(at(x0, y0 + 1.0), at(x0 + 1.0, y0 + 1.0), fx);
    mix(top, bottom, fy)
}

/// Layers of [`noise`], each twice as fine and half as strong, scaled to 0 to 1.
fn fbm(u: f32, v: f32, cx: i32, cy: i32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut weight, mut total) = (0.0, 1.0, 0.0);
    for octave in 0..octaves {
        let scale = 1_i32 << octave;
        sum += weight * noise(u, v, cx * scale, cy * scale, seed.wrapping_add(octave * 31));
        total += weight;
        weight *= 0.5;
    }
    if total > 0.0 {
        sum / total
    } else {
        0.0
    }
}

fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = (h - h.floor()) * 6.0;
    let (i, f) = (h.floor() as i32, h - h.floor());
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// A smooth bump of height 1 at `center`, `spread` wide.
fn bump(x: f32, center: f32, spread: f32) -> f32 {
    let d = (x - center) / spread;
    (-d * d).exp()
}

// The looks -----------------------------------------------------------------------------

/// Pencil: a grainy coverage, thick in places and nearly missing in tiny pits, so the paper
/// shows through. Averages about two thirds covered.
fn pencil_alpha(u: f32, v: f32) -> f32 {
    let coarse = fbm(u, v, 8, 8, 2, 11);
    let fine = noise(u, v, 64, 64, 12);
    let grain = 0.55 * coarse + 0.45 * fine;
    let shaped = smooth(((grain - 0.2) / 0.6).clamp(0.0, 1.0));
    0.18 + 0.78 * shaped
}

/// Rainbow: the hue turns once around per repeat; the edges are a little darker and a soft
/// gloss runs along the upper side.
fn rainbow(u: f32, across: f32) -> [f32; 3] {
    let base = hsv(u, 0.82, 1.0 - 0.16 * across * across);
    mix3(base, [1.0; 3], 0.30 * bump(across, -0.35, 0.28))
}

/// Galaxy: a night sky with violet and teal clouds and scattered stars, a few with a glow.
fn galaxy(u: f32, v: f32, x: usize, y: usize, (w, h): (usize, usize)) -> [f32; 3] {
    let night = [0.03, 0.02, 0.14];
    let violet = [0.46, 0.20, 0.78];
    let teal = [0.10, 0.62, 0.74];
    let cloud = fbm(u, v, 4, 2, 4, 21);
    let haze = fbm(u, v, 3, 2, 3, 22);
    let mut c = mix3(night, violet, smooth(((cloud - 0.35) / 0.4).clamp(0.0, 1.0)));
    c = mix3(c, teal, 0.75 * smooth(((haze - 0.5) / 0.35).clamp(0.0, 1.0)));
    let star_at = |i: i32, j: i32| {
        let (i, j) = (i.rem_euclid(w as i32), j.clamp(0, h as i32 - 1));
        let r = hash(i, j, 23);
        if unit(r) < 0.012 {
            0.55 + 0.45 * unit(r.rotate_left(13))
        } else {
            0.0
        }
    };
    let (xi, yi) = (x as i32, y as i32);
    let glow = 0.4
        * star_at(xi - 1, yi)
            .max(star_at(xi + 1, yi))
            .max(star_at(xi, yi - 1))
            .max(star_at(xi, yi + 1));
    let star = star_at(xi, yi).max(glow);
    mix3(c, [1.0, 0.97, 0.92], star.clamp(0.0, 1.0))
}

/// Lava: a dark crust cut by glowing veins that run from deep red through orange to yellow.
fn lava(u: f32, v: f32, across: f32) -> [f32; 3] {
    let n = fbm(u, v, 5, 3, 4, 31);
    let vein = 1.0 - (2.0 * n - 1.0).abs();
    let heat = (vein.powi(3) * 1.5 + 0.12 * fbm(u, v, 9, 6, 2, 32)).clamp(0.0, 1.0);
    let crust = [0.12, 0.03, 0.02];
    let red = [0.60, 0.08, 0.03];
    let orange = [1.0, 0.45, 0.04];
    let yellow = [1.0, 0.90, 0.38];
    let c = if heat < 0.45 {
        mix3(crust, red, heat / 0.45)
    } else if heat < 0.75 {
        mix3(red, orange, (heat - 0.45) / 0.30)
    } else {
        mix3(orange, yellow, (heat - 0.75) / 0.25)
    };
    mix3(c, [0.05, 0.01, 0.01], 0.25 * across * across)
}

/// Ocean: deep blue at the edges, turquoise down the middle, with pale wavy bands.
fn ocean(u: f32, v: f32, across: f32) -> [f32; 3] {
    let deep = [0.03, 0.22, 0.52];
    let shallow = [0.08, 0.60, 0.78];
    let foam = [0.80, 0.96, 0.98];
    let body = mix3(shallow, deep, smooth(across.abs()));
    let wobble = 0.35 * fbm(u, v, 3, 3, 3, 41);
    let wave = ((u * 3.0 + across * 0.25 + wobble) * TAU).sin() * 0.5 + 0.5;
    mix3(body, foam, 0.75 * smooth(((wave - 0.82) / 0.18).clamp(0.0, 1.0)))
}

/// Three tones of a metal, from shadow through body to highlight.
type Metal = [[f32; 3]; 3];

const METAL_GOLD: Metal = [[0.42, 0.29, 0.05], [0.86, 0.68, 0.18], [1.0, 0.96, 0.70]];
const METAL_SILVER: Metal = [[0.30, 0.33, 0.38], [0.74, 0.77, 0.81], [1.0, 1.0, 1.0]];
const METAL_BRONZE: Metal = [[0.28, 0.15, 0.06], [0.66, 0.42, 0.18], [0.96, 0.78, 0.52]];
const METAL_ROSE: Metal = [[0.45, 0.24, 0.24], [0.85, 0.58, 0.52], [1.0, 0.89, 0.84]];

/// A polished metal tube: dark at the edges, a bright highlight band on the upper side, a
/// softer reflection on the lower, a slow shimmer along the stroke, and a hint of grain.
fn metal(tones: Metal, u: f32, across: f32, x: usize, y: usize) -> [f32; 3] {
    let shimmer = 0.09 * ((u * 2.0 + across * 0.12) * TAU).sin();
    let grain = 0.03 * (unit(hash(x as i32, y as i32, 51)) - 0.5);
    let m = 0.58 - 0.34 * across * across
        + 0.50 * bump(across, -0.42, 0.17)
        + 0.16 * bump(across, 0.58, 0.20)
        + shimmer
        + grain;
    let m = m.clamp(0.0, 1.0);
    if m < 0.5 {
        mix3(tones[0], tones[1], m / 0.5)
    } else {
        mix3(tones[1], tones[2], (m - 0.5) / 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<InkTexture> {
        InkTexture::ALL.to_vec()
    }

    #[test]
    fn plain_has_no_image_and_every_texture_has_one_of_the_stated_size() {
        assert!(image(InkTexture::Plain).is_none() && spec(InkTexture::Plain).is_none());
        for texture in all() {
            let spec = spec(texture).unwrap();
            let image = image(texture).unwrap();
            assert_eq!((image.width, image.height), (spec.width, spec.height));
            assert_eq!(image.rgba.len(), spec.width * spec.height * 4);
        }
    }

    #[test]
    fn the_images_are_deterministic() {
        for texture in all() {
            assert_eq!(image(texture), image(texture), "{texture:?}");
            assert_eq!(texel(texture, 7, 5), texel(texture, 7, 5));
        }
        // Sampling at coordinates reads the pixel that covers them.
        assert_eq!(sample(InkTexture::Gold, 0.25, 0.5), texel(InkTexture::Gold, 64, 24));
    }

    #[test]
    fn the_textures_look_different_from_each_other() {
        let images: Vec<TextureImage> = all().into_iter().map(|t| image(t).unwrap()).collect();
        for (i, a) in images.iter().enumerate() {
            for b in &images[i + 1..] {
                assert!(a.rgba != b.rgba);
            }
        }
    }

    #[test]
    fn stroke_textures_tile_horizontally_without_a_seam() {
        for texture in all() {
            let spec = spec(texture).unwrap();
            if spec.anchor != Anchor::Stroke {
                continue;
            }
            // The last column continues into the first about as smoothly as neighbors do.
            let mut seam = 0.0_f32;
            let mut inside = 0.0_f32;
            for y in 0..spec.height {
                let step = |a: [u8; 4], b: [u8; 4]| {
                    (0..3)
                        .map(|i| (f32::from(a[i]) - f32::from(b[i])).abs())
                        .sum::<f32>()
                };
                seam += step(texel(texture, spec.width - 1, y), texel(texture, 0, y));
                inside += step(texel(texture, 10, y), texel(texture, 11, y));
            }
            let rows = spec.height as f32;
            assert!(
                seam / rows < 60.0 + inside / rows * 4.0,
                "{texture:?} seam {} inside {}",
                seam / rows,
                inside / rows
            );
        }
    }

    #[test]
    fn the_rainbow_turns_through_the_hues_along_the_stroke() {
        let red = sample(InkTexture::Rainbow, 0.0, 0.5);
        let green = sample(InkTexture::Rainbow, 1.0 / 3.0, 0.5);
        let blue = sample(InkTexture::Rainbow, 2.0 / 3.0, 0.5);
        assert!(red[0] > red[1] && red[0] > red[2]);
        assert!(green[1] > green[0] && green[1] > green[2]);
        assert!(blue[2] > blue[0] && blue[2] > blue[1]);
    }

    #[test]
    fn a_metal_is_brightest_at_its_highlight_and_darkest_at_the_edge() {
        let lum = |p: [u8; 4]| u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]);
        for texture in [
            InkTexture::Gold,
            InkTexture::Silver,
            InkTexture::Bronze,
            InkTexture::RoseGold,
        ] {
            // Across -0.42 is the highlight band; the middle is the body; the rim is shadow.
            let v = |across: f32| (across + 1.0) / 2.0;
            let (rim, body, shine) = (
                lum(sample(texture, 0.1, v(-0.98))),
                lum(sample(texture, 0.1, v(0.1))),
                lum(sample(texture, 0.1, v(-0.42))),
            );
            assert!(shine > body && body > rim, "{texture:?} {shine} {body} {rim}");
        }
    }

    #[test]
    fn the_pencil_is_grainy_and_semi_opaque() {
        let image = image(InkTexture::Pencil).unwrap();
        let alphas: Vec<u8> = image.rgba.chunks(4).map(|p| p[3]).collect();
        let mean = alphas.iter().map(|a| f32::from(*a)).sum::<f32>() / alphas.len() as f32;
        assert!((0.45 * 255.0..0.85 * 255.0).contains(&mean), "mean {mean}");
        let (lo, hi) = (
            alphas.iter().copied().min().unwrap(),
            alphas.iter().copied().max().unwrap(),
        );
        assert!(lo < 80 && hi > 200, "range {lo} to {hi}");
        // The tint is white: the stroke's color goes through it.
        assert!(image.rgba.chunks(4).all(|p| p[..3] == [255, 255, 255]));
        assert!(spec(InkTexture::Pencil).unwrap().tinted);
    }

    #[test]
    fn coordinates_follow_the_stroke_and_stay_finite() {
        let at = |along: f32, across: f32| uv(InkTexture::Gold, 4.0, along, across, Vec2::ZERO);
        let (a, b) = (at(0.0, 0.0), at(220.0, 0.0));
        // A thin stroke repeats every `min_period`; the same across gives the same v.
        assert!((b[0] - a[0] - 1.0).abs() < 1.0e-4 && a[1] == b[1]);
        // A wide stroke stretches the image so its pixels stay square.
        let wide = uv(InkTexture::Galaxy, 40.0, 320.0, 0.0, Vec2::ZERO);
        assert!((wide[0] - 1.0).abs() < 1.0e-4);
        // Across stays strictly inside the image.
        let (lo, hi) = (at(0.0, -1.0), at(0.0, 1.0));
        assert!(lo[1] > 0.0 && hi[1] < 1.0 && lo[1] < hi[1]);
        for bad in [f32::NAN, f32::INFINITY, -f32::INFINITY] {
            let got = uv(InkTexture::Lava, bad, bad, bad, Vec2::new(bad, bad));
            assert!(got.iter().all(|c| c.is_finite()));
            let got = uv(InkTexture::Pencil, bad, bad, bad, Vec2::new(bad, bad));
            assert!(got.iter().all(|c| c.is_finite()));
        }
        // A page texture is pinned to the page.
        let p = uv(InkTexture::Pencil, 3.0, 99.0, 0.3, Vec2::new(PENCIL_TILE, 48.0));
        assert!((p[0] - 1.0).abs() < 1.0e-5 && (p[1] - 0.5).abs() < 1.0e-5);
    }

    #[test]
    fn sampling_wraps_and_survives_odd_input() {
        for texture in all() {
            let _ = sample(texture, -3.7, 12.2);
            let _ = sample(texture, f32::NAN, f32::INFINITY);
            assert_eq!(sample(texture, 0.3, 0.6), sample(texture, 1.3, -0.4));
        }
        assert_eq!(sample(InkTexture::Plain, 0.5, 0.5), [0, 0, 0, 0]);
    }
}
