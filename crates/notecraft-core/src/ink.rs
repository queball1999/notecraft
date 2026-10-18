//! Ink stroke types: what a stroke is, independent of how it is drawn or stored. The
//! serialized form matches the `ink` entries of the canvas block (docs/markdown.md section
//! 3.4): a stroke is `{ id, tool, color, width, points: [[x, y, pressure], ...] }`.

use serde::{Deserialize, Serialize};

use crate::geometry::{Aabb, Vec2};

/// Most points a stroke may hold (docs/markdown.md section 9).
pub const MAX_STROKE_POINTS: usize = 20_000;
/// Thinnest and widest stroke, in page units.
pub const MIN_STROKE_WIDTH: f32 = 0.1;
pub const MAX_STROKE_WIDTH: f32 = 200.0;

/// One point of a stroke: page position and pressure from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "[f32; 3]", into = "[f32; 3]")]
pub struct InkPoint {
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
}

impl InkPoint {
    pub const fn new(x: f32, y: f32, pressure: f32) -> Self {
        Self { x, y, pressure }
    }

    pub const fn pos(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    /// This point with each value rounded to two decimal places, as the file stores it.
    pub fn rounded(self) -> Self {
        let round = |v: f32| (v * 100.0).round() / 100.0;
        Self::new(round(self.x), round(self.y), round(self.pressure))
    }
}

impl From<[f32; 3]> for InkPoint {
    /// Lenient: a value that is not finite becomes 0, and pressure is clamped to 0 to 1.
    fn from([x, y, pressure]: [f32; 3]) -> Self {
        let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
        Self::new(finite(x), finite(y), finite(pressure).clamp(0.0, 1.0))
    }
}

impl From<InkPoint> for [f32; 3] {
    fn from(p: InkPoint) -> Self {
        [p.x, p.y, p.pressure]
    }
}

/// Which tool made a stroke. Ids are the `tool` values in the canvas block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InkTool {
    #[default]
    Pen,
    Highlighter,
}

/// How a pen stroke looks beyond its color: the textured pens and the pencil of the Draw tab.
/// The ids are the `texture` values in the canvas block (docs/markdown.md section 3.4). A
/// stroke with no texture, or one this version does not know, is a plain color stroke.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InkTexture {
    /// A plain stroke in the stroke's color.
    #[default]
    Plain,
    /// Graphite: a grainy, semi-opaque stroke in the stroke's color.
    Pencil,
    /// Hues that cycle along the stroke.
    Rainbow,
    /// A deep nebula of violet and teal with scattered stars.
    Galaxy,
    /// A dark crust cut by glowing orange and yellow veins.
    Lava,
    /// Blue-green water with pale wavy bands.
    Ocean,
    /// Polished metals: a sheen across the stroke, a slow shimmer along it.
    Gold,
    Silver,
    Bronze,
    RoseGold,
}

impl InkTexture {
    /// Every texture the Draw tab offers (not [`InkTexture::Plain`]), in gallery order.
    pub const ALL: [Self; 9] = [
        Self::Pencil,
        Self::Rainbow,
        Self::Galaxy,
        Self::Lava,
        Self::Ocean,
        Self::Gold,
        Self::Silver,
        Self::Bronze,
        Self::RoseGold,
    ];

    /// The id stored in the file; empty for a plain stroke.
    pub fn id(self) -> &'static str {
        match self {
            Self::Plain => "",
            Self::Pencil => "pencil",
            Self::Rainbow => "rainbow",
            Self::Galaxy => "galaxy",
            Self::Lava => "lava",
            Self::Ocean => "ocean",
            Self::Gold => "gold",
            Self::Silver => "silver",
            Self::Bronze => "bronze",
            Self::RoseGold => "rose_gold",
        }
    }

    /// The texture with this id (any case). An unknown id gives `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        Self::ALL
            .into_iter()
            .find(|texture| texture.id().eq_ignore_ascii_case(text))
    }

    /// The name shown in the UI.
    pub fn name(self) -> &'static str {
        match self {
            Self::Plain => "Pen",
            Self::Pencil => "Pencil",
            Self::Rainbow => "Rainbow",
            Self::Galaxy => "Galaxy",
            Self::Lava => "Lava",
            Self::Ocean => "Ocean",
            Self::Gold => "Gold",
            Self::Silver => "Silver",
            Self::Bronze => "Bronze",
            Self::RoseGold => "Rose Gold",
        }
    }

    /// Whether the stroke's own color shapes the look. Only the pencil does; the others have
    /// a look of their own and keep a color only as the plain stroke older readers draw.
    pub fn uses_color(self) -> bool {
        matches!(self, Self::Plain | Self::Pencil)
    }

    /// The color a new pen of this texture starts with: the pencil's graphite, or a color that
    /// stands for the texture where it falls back to a plain stroke.
    pub fn swatch(self) -> InkColor {
        let rgb = match self {
            Self::Plain => return InkColor::BLACK,
            Self::Pencil => [0x3A, 0x3A, 0x3A],
            Self::Rainbow => [0xB1, 0x46, 0xC2],
            Self::Galaxy => [0x4B, 0x2E, 0x83],
            Self::Lava => [0xE8, 0x59, 0x0C],
            Self::Ocean => [0x0A, 0x7E, 0xA4],
            Self::Gold => [0xD4, 0xAF, 0x37],
            Self::Silver => [0xA8, 0xAE, 0xB6],
            Self::Bronze => [0xA6, 0x6A, 0x2E],
            Self::RoseGold => [0xD8, 0x93, 0x8A],
        };
        InkColor([rgb[0], rgb[1], rgb[2], 0xFF])
    }
}

impl Serialize for InkTexture {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.id())
    }
}

impl<'de> Deserialize<'de> for InkTexture {
    /// Lenient: text that is not a known texture is a plain stroke.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Ok(Self::parse(&text).unwrap_or_default())
    }
}

/// A stroke color as RGBA. Serialized as `#RRGGBB`, or `#RRGGBBAA` when not opaque.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InkColor(pub [u8; 4]);

impl InkColor {
    pub const BLACK: Self = Self([0x1F, 0x1F, 0x1F, 0xFF]);

    /// Parse `#RRGGBB` or `#RRGGBBAA` (any case). Anything else is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        if !matches!(digits.len(), 6 | 8) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| {
            digits
                .get(i..i + 2)
                .and_then(|d| u8::from_str_radix(d, 16).ok())
        };
        let alpha = if digits.len() == 8 { byte(6)? } else { u8::MAX };
        Some(Self([byte(0)?, byte(2)?, byte(4)?, alpha]))
    }

    /// `#RRGGBB` when opaque, `#RRGGBBAA` otherwise; uppercase.
    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.0;
        if a == u8::MAX {
            format!("#{r:02X}{g:02X}{b:02X}")
        } else {
            format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
        }
    }
}

impl Default for InkColor {
    fn default() -> Self {
        Self::BLACK
    }
}

impl Serialize for InkColor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for InkColor {
    /// Lenient: text that is not a color becomes the default pen color.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Ok(Self::parse(&text).unwrap_or_default())
    }
}

/// One ink stroke. `width` is the widest the stroke gets, in page units; pressure narrows it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub id: String,
    #[serde(default)]
    pub tool: InkTool,
    #[serde(default)]
    pub color: InkColor,
    pub width: f32,
    /// A texture for a pen stroke; [`InkTexture::Plain`] (and not written) for the rest.
    #[serde(default, skip_serializing_if = "is_plain")]
    pub texture: InkTexture,
    pub points: Vec<InkPoint>,
}

fn is_plain(texture: &InkTexture) -> bool {
    *texture == InkTexture::Plain
}

impl Stroke {
    /// This stroke with its width and point count forced into range (a width that is not
    /// finite becomes 1). Use it on anything read from a file or a command.
    pub fn bounded(mut self) -> Self {
        // Only a pen has a texture.
        if self.tool == InkTool::Highlighter {
            self.texture = InkTexture::Plain;
        }
        self.width = if self.width.is_finite() {
            self.width.clamp(MIN_STROKE_WIDTH, MAX_STROKE_WIDTH)
        } else {
            1.0
        };
        self.points.truncate(MAX_STROKE_POINTS);
        self
    }

    /// The box around the stroke including its width, or `None` for an empty stroke.
    pub fn bounds(&self) -> Option<Aabb> {
        let mut points = self.points.iter().map(|p| p.pos());
        let first = points.next()?;
        let bounds = points.fold(Aabb::from_point(first), Aabb::include);
        Some(bounds.inflate(self.width / 2.0))
    }
}

/// Most distance checks one [`simplify`] call may spend before it falls back to even thinning.
const SIMPLIFY_BUDGET: usize = 20_000_000;

/// Reduce `points` to at most `max` points with Ramer-Douglas-Peucker (docs/markdown.md section
/// 3.4). The first and last points are kept. The tolerance doubles until the result fits; a
/// huge or adversarial input falls back to even thinning, so the cost is bounded.
pub fn simplify(points: &[InkPoint], max: usize) -> Vec<InkPoint> {
    let max = max.max(2);
    if points.len() <= max {
        return points.to_vec();
    }
    // Thin hugely long input first so the passes below stay cheap.
    let limit = max.saturating_mul(2);
    let stride = points.len().div_ceil(limit).max(1);
    let mut work: Vec<InkPoint> = points.iter().copied().step_by(stride).collect();
    if let Some(last) = points.last() {
        if work.last() != Some(last) {
            work.push(*last);
        }
    }
    let mut budget = SIMPLIFY_BUDGET;
    let mut eps = 0.05_f64;
    for _ in 0..16 {
        match rdp(&work, eps, &mut budget) {
            Some(kept) if kept.len() <= max => return kept,
            Some(_) => eps *= 2.0,
            None => break,
        }
    }
    thin(&work, max)
}

/// Keep `max` evenly spaced points, always including the first and last.
fn thin(points: &[InkPoint], max: usize) -> Vec<InkPoint> {
    let n = points.len();
    if n <= max {
        return points.to_vec();
    }
    let denom = max.saturating_sub(1).max(1);
    (0..max)
        .filter_map(|i| points.get(i * (n - 1) / denom).copied())
        .collect()
}

fn seg_distance(p: InkPoint, a: InkPoint, b: InkPoint) -> f64 {
    let (px, py) = (f64::from(p.x), f64::from(p.y));
    let (ax, ay) = (f64::from(a.x), f64::from(a.y));
    let (dx, dy) = (f64::from(b.x) - ax, f64::from(b.y) - ay);
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq <= f64::EPSILON {
        0.0
    } else {
        (((px - ax) * dx + (py - ay) * dy) / len_sq).clamp(0.0, 1.0)
    };
    ((px - ax - dx * t).powi(2) + (py - ay - dy * t).powi(2)).sqrt()
}

/// One RDP pass. `None` when the distance budget ran out.
fn rdp(points: &[InkPoint], eps: f64, budget: &mut usize) -> Option<Vec<InkPoint>> {
    let n = points.len();
    let mut keep = vec![false; n];
    if let Some(first) = keep.first_mut() {
        *first = true;
    }
    if let Some(last) = keep.last_mut() {
        *last = true;
    }
    let mut stack = vec![(0usize, n.saturating_sub(1))];
    while let Some((lo, hi)) = stack.pop() {
        if hi <= lo + 1 {
            continue;
        }
        let (a, b) = (*points.get(lo)?, *points.get(hi)?);
        let mut far = (0.0_f64, lo);
        for i in lo + 1..hi {
            *budget = budget.checked_sub(1)?;
            let d = seg_distance(*points.get(i)?, a, b);
            if d > far.0 {
                far = (d, i);
            }
        }
        if far.0 > eps {
            if let Some(slot) = keep.get_mut(far.1) {
                *slot = true;
            }
            stack.push((lo, far.1));
            stack.push((far.1, hi));
        }
    }
    Some(
        points
            .iter()
            .zip(&keep)
            .filter_map(|(p, k)| k.then_some(*p))
            .collect(),
    )
}

#[cfg(test)]
mod simplify_tests {
    use super::*;

    #[test]
    fn short_strokes_are_untouched_and_long_ones_fit() {
        let pts: Vec<InkPoint> = (0..50).map(|i| InkPoint::new(i as f32, 0.0, 0.5)).collect();
        assert_eq!(simplify(&pts, 100), pts);
        // A straight line collapses to its ends.
        assert_eq!(simplify(&pts, 10).len(), 2);
        let wavy: Vec<InkPoint> = (0..5000)
            .map(|i| InkPoint::new(i as f32, ((i as f32) * 0.37).sin() * 20.0, 0.5))
            .collect();
        let out = simplify(&wavy, 300);
        assert!(out.len() <= 300 && out.len() >= 2, "{}", out.len());
        assert_eq!(out.first(), wavy.first());
        assert_eq!(out.last(), wavy.last());
        let huge: Vec<InkPoint> = (0..200_000)
            .map(|i| InkPoint::new(i as f32, ((i * 7919) % 1000) as f32, 0.5))
            .collect();
        assert!(simplify(&huge, MAX_STROKE_POINTS).len() <= MAX_STROKE_POINTS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke() -> Stroke {
        Stroke {
            id: "s1".into(),
            tool: InkTool::Pen,
            color: InkColor::parse("#1F3A93").unwrap_or_default(),
            width: 2.0,
            texture: InkTexture::Plain,
            points: vec![
                InkPoint::new(600.0, 300.0, 0.5),
                InkPoint::new(604.0, 302.0, 0.25),
            ],
        }
    }

    #[test]
    fn a_texture_round_trips_and_a_plain_stroke_writes_none() {
        let plain = serde_json::to_value(stroke()).unwrap();
        assert!(plain.get("texture").is_none());
        let mut gold = stroke();
        gold.texture = InkTexture::RoseGold;
        let json = serde_json::to_value(&gold).unwrap();
        assert_eq!(json["texture"], "rose_gold");
        let back: Stroke = serde_json::from_value(json).unwrap();
        assert_eq!(back, gold);
    }

    #[test]
    fn an_unknown_texture_reads_as_plain() {
        let text = r##"{ "id": "s1", "texture": "sparkle-9000", "width": 1, "points": [[1, 2, 1]] }"##;
        let s: Stroke = serde_json::from_str(text).unwrap();
        assert_eq!(s.texture, InkTexture::Plain);
        for texture in InkTexture::ALL {
            assert_eq!(InkTexture::parse(texture.id()), Some(texture));
            assert_eq!(
                InkTexture::parse(&texture.id().to_uppercase()),
                Some(texture)
            );
        }
        assert_eq!(InkTexture::parse(""), None);
    }

    #[test]
    fn a_highlighter_has_no_texture() {
        let mut s = stroke();
        s.tool = InkTool::Highlighter;
        s.texture = InkTexture::Gold;
        assert_eq!(s.bounded().texture, InkTexture::Plain);
    }

    #[test]
    fn serializes_like_the_canvas_block() {
        let json = serde_json::to_value(stroke()).unwrap();
        assert_eq!(json["tool"], "pen");
        assert_eq!(json["color"], "#1F3A93");
        assert_eq!(json["points"][0][0], 600.0);
        assert_eq!(json["points"][1][2], 0.25);
        let back: Stroke = serde_json::from_value(json).unwrap();
        assert_eq!(back, stroke());
    }

    #[test]
    fn reading_is_lenient() {
        let text = r##"{ "id": "s9", "color": "puce", "width": 1,
            "points": [[1, 2, 7], [3, 4, -1]] }"##;
        let s: Stroke = serde_json::from_str(text).unwrap();
        assert_eq!(s.tool, InkTool::Pen);
        assert_eq!(s.color, InkColor::BLACK);
        assert_eq!(s.points[0].pressure, 1.0);
        assert_eq!(s.points[1].pressure, 0.0);
        assert!(serde_json::from_str::<Stroke>("{").is_err());
    }

    #[test]
    fn bounded_clamps_width_and_points() {
        let mut s = stroke();
        s.width = f32::NAN;
        s.points = vec![InkPoint::new(0.0, 0.0, 0.5); MAX_STROKE_POINTS + 5];
        let s = s.bounded();
        assert_eq!(s.width, 1.0);
        assert_eq!(s.points.len(), MAX_STROKE_POINTS);
        let mut wide = stroke();
        wide.width = 1.0e9;
        assert_eq!(wide.bounded().width, MAX_STROKE_WIDTH);
    }

    #[test]
    fn bounds_include_the_width() {
        let b = stroke().bounds().unwrap();
        assert_eq!(b.min, Vec2::new(599.0, 299.0));
        assert_eq!(b.max, Vec2::new(605.0, 303.0));
        let mut empty = stroke();
        empty.points.clear();
        assert!(empty.bounds().is_none());
    }

    #[test]
    fn colors_parse_strictly_and_round_trip() {
        assert_eq!(InkColor::parse("#ff0000"), Some(InkColor([255, 0, 0, 255])));
        let c = InkColor::parse("#11223380").unwrap();
        assert_eq!(c.to_hex(), "#11223380");
        for bad in ["", "red", "#12345", "123456", "#GG0000", "#\u{e9}e9e9e"] {
            assert_eq!(InkColor::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn rounding_keeps_two_places() {
        let p = InkPoint::new(1.23456, 2.0, 0.987).rounded();
        assert_eq!(p, InkPoint::new(1.23, 2.0, 0.99));
    }
}
