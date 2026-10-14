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
}
