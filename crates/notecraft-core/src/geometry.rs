//! Plain 2D geometry shared by the engine layers. No UI types: the front end converts.

use std::ops::{Add, Div, Mul, Neg, Sub};

/// A point or a displacement in 2D, in whatever unit the caller uses (page units or screen
/// points).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Whether both coordinates are finite (not NaN or infinite).
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// This vector with each NaN or infinite coordinate replaced by zero.
    pub fn finite_or_zero(self) -> Self {
        let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
        Self::new(finite(self.x), finite(self.y))
    }
}

impl Add for Vec2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Neg for Vec2 {
    type Output = Self;

    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl Div<f32> for Vec2 {
    type Output = Self;

    fn div(self, rhs: f32) -> Self {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

impl Vec2 {
    /// The distance to `other`.
    pub fn distance(self, other: Self) -> f32 {
        let d = other - self;
        d.x.hypot(d.y)
    }

    /// The dot product.
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// The length of this vector.
    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }
}

/// An axis-aligned box in page units. `min` is at most `max` on both axes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Aabb {
    pub min: Vec2,
    pub max: Vec2,
}

impl Aabb {
    pub const fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    /// The box around one point.
    pub const fn from_point(p: Vec2) -> Self {
        Self { min: p, max: p }
    }

    /// The smallest box holding both corners, in any order.
    pub fn from_corners(a: Vec2, b: Vec2) -> Self {
        Self {
            min: Vec2::new(a.x.min(b.x), a.y.min(b.y)),
            max: Vec2::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    /// This box grown by `margin` on every side.
    pub fn inflate(self, margin: f32) -> Self {
        Self {
            min: Vec2::new(self.min.x - margin, self.min.y - margin),
            max: Vec2::new(self.max.x + margin, self.max.y + margin),
        }
    }

    /// The smallest box holding this box and `p`.
    pub fn include(self, p: Vec2) -> Self {
        Self {
            min: Vec2::new(self.min.x.min(p.x), self.min.y.min(p.y)),
            max: Vec2::new(self.max.x.max(p.x), self.max.y.max(p.y)),
        }
    }

    /// Whether the boxes overlap or touch.
    pub fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && other.min.x <= self.max.x
            && self.min.y <= other.max.y
            && other.min.y <= self.max.y
    }

    pub fn contains(self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
}

/// The distance from `p` to the segment `a` to `b`.
pub fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.dot(ab);
    if len_sq <= f32::EPSILON {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, -4.0);
        assert_eq!(a + b, Vec2::new(4.0, -2.0));
        assert_eq!(a - b, Vec2::new(-2.0, 6.0));
        assert_eq!(-a, Vec2::new(-1.0, -2.0));
        assert_eq!(a * 2.0, Vec2::new(2.0, 4.0));
        assert_eq!(b / 2.0, Vec2::new(1.5, -2.0));
    }

    #[test]
    fn non_finite_becomes_zero() {
        assert_eq!(
            Vec2::new(f32::NAN, 1.0).finite_or_zero(),
            Vec2::new(0.0, 1.0)
        );
        assert_eq!(
            Vec2::new(f32::INFINITY, f32::NAN).finite_or_zero(),
            Vec2::ZERO
        );
        assert_eq!(Vec2::new(1.0, 2.0).finite_or_zero(), Vec2::new(1.0, 2.0));
    }

    #[test]
    fn boxes_overlap_and_grow() {
        let a = Aabb::from_corners(Vec2::new(4.0, 4.0), Vec2::new(0.0, 0.0));
        assert_eq!(a.max, Vec2::new(4.0, 4.0));
        let b = Aabb::new(Vec2::new(4.0, 4.0), Vec2::new(6.0, 6.0));
        assert!(a.intersects(b));
        assert!(!a.intersects(Aabb::new(Vec2::new(5.0, 0.0), Vec2::new(6.0, 1.0))));
        assert!(a.inflate(1.0).contains(Vec2::new(-1.0, 5.0)));
        assert_eq!(
            Aabb::from_point(Vec2::ZERO)
                .include(Vec2::new(2.0, -1.0))
                .min,
            Vec2::new(0.0, -1.0)
        );
    }

    #[test]
    fn segment_distance_clamps_to_the_ends() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(10.0, 0.0);
        assert_eq!(distance_to_segment(Vec2::new(5.0, 3.0), a, b), 3.0);
        assert_eq!(distance_to_segment(Vec2::new(-4.0, 3.0), a, b), 5.0);
        assert_eq!(distance_to_segment(Vec2::new(3.0, 4.0), a, a), 5.0);
    }
}
