//! Pictures and file attachments placed on the page (docs/markdown.md section 3.4).
//!
//! The picture or file itself stays in the page text as markdown. A placed object is the
//! position that goes with one occurrence of that link: the `nth` image `![..](src)` (or file
//! paragraph `[name](src)`) in reading order sits at `pos` instead of flowing inside its
//! container.

use notecraft_core::Vec2;

/// Most placed objects of one kind a page keeps.
pub const MAX_OBJECTS: usize = 10_000;
/// Farthest a placed object can sit from the page origin, in page units (a sanity bound).
pub const MAX_COORD: f32 = 100_000.0;

/// What a placed object is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectKind {
    /// A picture: `![alt](src)`.
    Picture,
    /// A file attachment: a paragraph that is one link, `[name](src)`.
    File,
}

/// A picture or file attachment with a place of its own on the page.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedObject {
    pub id: String,
    pub kind: ObjectKind,
    /// The link as written in the page text.
    pub src: String,
    /// Which occurrence of `src` (of this kind), counting from 0 in reading order.
    pub nth: usize,
    /// Top-left corner in page units.
    pub pos: Vec2,
}

impl PlacedObject {
    /// The same object with its corner limited to the page bounds (not-a-number becomes 0).
    pub fn clamped(mut self) -> Self {
        let p = self.pos.finite_or_zero();
        self.pos = Vec2::new(p.x.clamp(0.0, MAX_COORD), p.y.clamp(0.0, MAX_COORD));
        self
    }

    /// Whether this object is the `nth` occurrence of `src` of `kind`.
    pub fn is(&self, kind: ObjectKind, src: &str, nth: usize) -> bool {
        self.kind == kind && self.nth == nth && self.src == src
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_corner_is_clamped_and_never_not_a_number() {
        let o = PlacedObject {
            id: "i1".into(),
            kind: ObjectKind::Picture,
            src: "a.png".into(),
            nth: 0,
            pos: Vec2::new(f32::NAN, -4.0),
        }
        .clamped();
        assert_eq!(o.pos, Vec2::ZERO);
        assert!(o.is(ObjectKind::Picture, "a.png", 0));
        assert!(!o.is(ObjectKind::File, "a.png", 0));
    }
}
