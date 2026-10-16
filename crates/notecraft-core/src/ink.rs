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
    pub points: Vec<InkPoint>,
}

impl Stroke {
    /// This stroke with its width and point count forced into range (a width that is not
    /// finite becomes 1). Use it on anything read from a file or a command.
    pub fn bounded(mut self) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke() -> Stroke {
        Stroke {
            id: "s1".into(),
            tool: InkTool::Pen,
            color: InkColor::parse("#1F3A93").unwrap_or_default(),
            width: 2.0,
            points: vec![
                InkPoint::new(600.0, 300.0, 0.5),
                InkPoint::new(604.0, 302.0, 0.25),
            ],
        }
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
