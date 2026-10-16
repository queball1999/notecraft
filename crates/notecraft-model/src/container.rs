//! Note containers: boxes of text with an id, a place on the page, and a width (OneNote's
//! note containers, docs/markdown.md section 3.3).
//!
//! The model holds the geometry only. A container's text is the run of page markdown after its
//! marker; `notecraft-markdown` finds those runs (L3 sits above this crate, so the blocks are
//! not stored here). The id ties the two together.

use notecraft_core::Vec2;

/// Id of the implicit container that holds the text before the first marker.
pub const IMPLICIT_ID: &str = "c0";
/// Default width of a container, in page units (7.5 inches at 96 per inch).
pub const DEFAULT_WIDTH: f32 = 720.0;
/// Narrowest a container can be resized to, in page units.
pub const MIN_WIDTH: f32 = 48.0;
/// Widest a container can be resized to, in page units.
pub const MAX_WIDTH: f32 = 4096.0;
/// Farthest a container can sit from the page origin, in page units (a sanity bound).
pub const MAX_COORD: f32 = 100_000.0;

/// Where a container sits and how wide it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Container {
    pub id: String,
    /// Top-left corner in page units, or `None` while the container is in the page's flow
    /// layout (containers without a position stack top to bottom).
    pub pos: Option<Vec2>,
    /// Width in page units, always within [`MIN_WIDTH`] and [`MAX_WIDTH`].
    pub width: f32,
}

impl Container {
    /// A container in the flow layout at the default width.
    pub fn flow(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            pos: None,
            width: DEFAULT_WIDTH,
        }
    }

    /// A container placed at `pos`.
    pub fn at(id: impl Into<String>, pos: Vec2, width: f32) -> Self {
        Self {
            id: id.into(),
            pos: Some(clamp_pos(pos)),
            width: clamp_width(width),
        }
    }

    /// True while the container has no position of its own.
    pub fn in_flow(&self) -> bool {
        self.pos.is_none()
    }
}

/// `width` limited to the allowed range; a width that is not a number is the default.
pub fn clamp_width(width: f32) -> f32 {
    if width.is_finite() {
        width.clamp(MIN_WIDTH, MAX_WIDTH)
    } else {
        DEFAULT_WIDTH
    }
}

/// `pos` with each coordinate limited to 0 to [`MAX_COORD`]; a coordinate that is not a
/// number becomes 0.
pub fn clamp_pos(pos: Vec2) -> Vec2 {
    let pos = pos.finite_or_zero();
    Vec2::new(pos.x.clamp(0.0, MAX_COORD), pos.y.clamp(0.0, MAX_COORD))
}

/// Whether `id` is usable as a container id: 1 to 32 ASCII letters, digits, `-` or `_`.
pub fn is_valid_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_is_clamped_and_never_not_a_number() {
        let c = Container::at("c1", Vec2::new(f32::NAN, -5.0), f32::INFINITY);
        assert_eq!(c.pos, Some(Vec2::ZERO));
        assert_eq!(c.width, DEFAULT_WIDTH);
        assert_eq!(clamp_width(1.0), MIN_WIDTH);
        assert_eq!(clamp_width(1.0e9), MAX_WIDTH);
        assert_eq!(clamp_pos(Vec2::new(1.0e9, 3.0)), Vec2::new(MAX_COORD, 3.0));
    }

    #[test]
    fn a_new_flow_container_has_no_position() {
        let c = Container::flow("c0");
        assert!(c.in_flow());
        assert_eq!(c.width, DEFAULT_WIDTH);
    }

    #[test]
    fn ids_are_short_and_plain() {
        assert!(is_valid_id("c1") && is_valid_id("a_b-9"));
        assert!(!is_valid_id("") && !is_valid_id("a b") && !is_valid_id(&"x".repeat(33)));
        assert!(!is_valid_id("c\u{e9}"));
    }
}
