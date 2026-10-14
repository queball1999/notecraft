//! Paper: the rule lines or grid behind a page, and its paper size. Ids are the `style` and
//! `size` argument values of `view.rule_lines` and `view.paper_size`.

use notecraft_core::Vec2;

use crate::viewport::UNITS_PER_INCH;

/// Most lines drawn along one axis; anything denser is skipped rather than drawn.
pub const MAX_LINES: usize = 4096;

/// Whether a rule line style draws lines, a grid, or nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuleKind {
    None,
    /// Horizontal lines only.
    Lines,
    /// Horizontal and vertical lines.
    Grid,
}

/// The rule line styles of OneNote's Rule Lines menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RuleLines {
    #[default]
    None,
    Narrow,
    College,
    Standard,
    Wide,
    GridSmall,
    GridMedium,
    GridLarge,
    GridXLarge,
}

impl RuleLines {
    pub const ALL: [Self; 9] = [
        Self::None,
        Self::Narrow,
        Self::College,
        Self::Standard,
        Self::Wide,
        Self::GridSmall,
        Self::GridMedium,
        Self::GridLarge,
        Self::GridXLarge,
    ];

    /// The command argument value, for example `college` or `grid_small`.
    pub fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Narrow => "narrow",
            Self::College => "college",
            Self::Standard => "standard",
            Self::Wide => "wide",
            Self::GridSmall => "grid_small",
            Self::GridMedium => "grid_medium",
            Self::GridLarge => "grid_large",
            Self::GridXLarge => "grid_xlarge",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|style| style.id() == id)
    }

    pub fn kind(self) -> RuleKind {
        match self {
            Self::None => RuleKind::None,
            Self::Narrow | Self::College | Self::Standard | Self::Wide => RuleKind::Lines,
            Self::GridSmall | Self::GridMedium | Self::GridLarge | Self::GridXLarge => {
                RuleKind::Grid
            }
        }
    }

    /// Distance between lines, in page units (1/96 inch). Zero for [`RuleLines::None`].
    /// Ruled spacings follow US notebook paper: narrow 1/4 inch, college 9/32, standard
    /// 5/16, wide 11/32. Grids are 1/8, 1/4, 1/2, and 1 inch.
    pub fn spacing(self) -> f32 {
        let inches = match self {
            Self::None => 0.0,
            Self::Narrow => 8.0 / 32.0,
            Self::College => 9.0 / 32.0,
            Self::Standard => 10.0 / 32.0,
            Self::Wide => 11.0 / 32.0,
            Self::GridSmall => 1.0 / 8.0,
            Self::GridMedium => 1.0 / 4.0,
            Self::GridLarge => 1.0 / 2.0,
            Self::GridXLarge => 1.0,
        };
        inches * UNITS_PER_INCH
    }
}

/// The paper sizes of OneNote's Paper Size menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PaperSize {
    /// No fixed size: the page grows with its content.
    #[default]
    Auto,
    Letter,
    Legal,
    A4,
    A5,
}

impl PaperSize {
    pub const ALL: [Self; 5] = [Self::Auto, Self::Letter, Self::Legal, Self::A4, Self::A5];

    /// Width of an auto-sized page for Page Width zoom while it has no content: US Letter.
    pub const AUTO_WIDTH: f32 = 8.5 * UNITS_PER_INCH;

    /// The command argument value, for example `a4`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Letter => "letter",
            Self::Legal => "legal",
            Self::A4 => "a4",
            Self::A5 => "a5",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|size| size.id() == id)
    }

    /// Width and height in page units (portrait), or `None` for [`PaperSize::Auto`].
    pub fn size(self) -> Option<Vec2> {
        let mm = UNITS_PER_INCH / 25.4;
        match self {
            Self::Auto => None,
            Self::Letter => Some(Vec2::new(8.5, 11.0) * UNITS_PER_INCH),
            Self::Legal => Some(Vec2::new(8.5, 14.0) * UNITS_PER_INCH),
            Self::A4 => Some(Vec2::new(210.0, 297.0) * mm),
            Self::A5 => Some(Vec2::new(148.0, 210.0) * mm),
        }
    }

    /// The width Page Width zoom fits to.
    pub fn fit_width(self) -> f32 {
        self.size().map_or(Self::AUTO_WIDTH, |size| size.x)
    }
}

/// Positions of lines `spacing` apart, counted from page coordinate 0, that fall within
/// `min..=max`. Empty when the spacing is not positive, the range is not finite, or more
/// than [`MAX_LINES`] lines would be needed.
pub fn line_positions(spacing: f32, min: f32, max: f32) -> impl Iterator<Item = f32> {
    let valid = spacing > 0.0 && spacing.is_finite() && min.is_finite() && max.is_finite();
    let (first, count) = if valid && max >= min {
        let first = (min / spacing).ceil();
        let last = (max / spacing).floor();
        let count = last - first + 1.0;
        if count >= 0.0 && count <= MAX_LINES as f32 {
            // count is in 0..=MAX_LINES, so the cast is exact.
            (first, count as usize)
        } else {
            (0.0, 0)
        }
    } else {
        (0.0, 0)
    };
    (0..count).map(move |i| (first + i as f32) * spacing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for style in RuleLines::ALL {
            assert_eq!(RuleLines::from_id(style.id()), Some(style));
        }
        for size in PaperSize::ALL {
            assert_eq!(PaperSize::from_id(size.id()), Some(size));
        }
        assert_eq!(RuleLines::from_id("dotted"), None);
        assert_eq!(PaperSize::from_id("A4"), None);
    }

    #[test]
    fn spacing_grows_from_narrow_to_wide() {
        let ruled = [
            RuleLines::Narrow,
            RuleLines::College,
            RuleLines::Standard,
            RuleLines::Wide,
        ];
        assert!(ruled.windows(2).all(|w| w[0].spacing() < w[1].spacing()));
        assert_eq!(RuleLines::Narrow.spacing(), 24.0);
        assert_eq!(RuleLines::GridXLarge.spacing(), 96.0);
        assert_eq!(RuleLines::None.spacing(), 0.0);
        assert_eq!(RuleLines::None.kind(), RuleKind::None);
        assert_eq!(RuleLines::Wide.kind(), RuleKind::Lines);
        assert_eq!(RuleLines::GridSmall.kind(), RuleKind::Grid);
    }

    #[test]
    fn paper_sizes() {
        assert_eq!(PaperSize::Auto.size(), None);
        assert_eq!(PaperSize::Letter.size(), Some(Vec2::new(816.0, 1056.0)));
        let a4 = PaperSize::A4.size().unwrap_or_default();
        assert!((a4.x - 793.7).abs() < 0.1 && (a4.y - 1122.5).abs() < 0.1);
        assert_eq!(PaperSize::Auto.fit_width(), 816.0);
        assert_eq!(PaperSize::Legal.fit_width(), 816.0);
    }

    #[test]
    fn lines_cover_the_range_only() {
        let lines: Vec<f32> = line_positions(24.0, 10.0, 80.0).collect();
        assert_eq!(lines, [24.0, 48.0, 72.0]);
        let lines: Vec<f32> = line_positions(24.0, -30.0, 0.0).collect();
        assert_eq!(lines, [-24.0, 0.0]);
        assert_eq!(line_positions(24.0, 25.0, 47.0).count(), 0);
    }

    #[test]
    fn lines_are_bounded() {
        assert_eq!(line_positions(0.0, 0.0, 100.0).count(), 0);
        assert_eq!(line_positions(-5.0, 0.0, 100.0).count(), 0);
        assert_eq!(line_positions(1.0, 100.0, 0.0).count(), 0);
        assert_eq!(line_positions(1.0, f32::NAN, 10.0).count(), 0);
        assert_eq!(line_positions(1.0, 0.0, f32::INFINITY).count(), 0);
        assert_eq!(line_positions(1e-3, 0.0, 1e6).count(), 0);
        assert_eq!(line_positions(1.0, 0.0, 4095.0).count(), MAX_LINES);
    }
}
