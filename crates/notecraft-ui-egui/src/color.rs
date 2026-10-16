//! Parsing and formatting colors typed by the user.
//!
//! Accepted forms, case-insensitive, with surrounding spaces ignored:
//!
//! | Form | Example |
//! |---|---|
//! | `#RGB`, `#RGBA` | `#f80`, `#f808` |
//! | `#RRGGBB`, `#RRGGBBAA` | `#ff8800`, `#ff880080` |
//! | the same without `#` | `ff8800` |
//! | `rgb(r, g, b)` | `rgb(255, 136, 0)`, `rgb(100%, 50%, 0%)` |
//! | `rgba(r, g, b, a)` | `rgba(255, 136, 0, 0.5)`, `rgba(255, 136, 0, 50%)` |
//!
//! Channels are 0-255 or a percentage; alpha is 0-1 or a percentage. Anything else, or input
//! longer than [`MAX_INPUT`], is rejected rather than guessed at.

/// Longest input considered; longer text is not a color.
pub const MAX_INPUT: usize = 64;

/// Red, green, blue, alpha; not premultiplied.
pub type Rgba8 = [u8; 4];

/// Parse a typed color. Returns `None` for anything that is not one of the accepted forms.
pub fn parse(text: &str) -> Option<Rgba8> {
    if text.len() > MAX_INPUT {
        return None;
    }
    let text = text.trim().to_ascii_lowercase();
    if let Some(inner) = function_args(&text, "rgba") {
        return channels(inner, true);
    }
    if let Some(inner) = function_args(&text, "rgb") {
        return channels(inner, false);
    }
    hex(text.strip_prefix('#').unwrap_or(&text))
}

/// `#RRGGBB` when opaque, `#RRGGBBAA` otherwise; uppercase.
pub fn format_hex([r, g, b, a]: Rgba8) -> String {
    if a == u8::MAX {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

fn function_args<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.strip_prefix(name)?
        .trim_start()
        .strip_prefix('(')?
        .strip_suffix(')')
}

fn hex(digits: &str) -> Option<Rgba8> {
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let nibble = |i: usize| {
        digits
            .get(i..=i)
            .and_then(|d| u8::from_str_radix(d, 16).ok())
    };
    let byte = |i: usize| {
        digits
            .get(i..i + 2)
            .and_then(|d| u8::from_str_radix(d, 16).ok())
    };
    match digits.len() {
        3 | 4 => {
            let short = |i: usize| nibble(i).map(|n| n * 17);
            let a = if digits.len() == 4 {
                short(3)?
            } else {
                u8::MAX
            };
            Some([short(0)?, short(1)?, short(2)?, a])
        }
        6 | 8 => {
            let a = if digits.len() == 8 { byte(6)? } else { u8::MAX };
            Some([byte(0)?, byte(2)?, byte(4)?, a])
        }
        _ => None,
    }
}

/// `r, g, b` or `r, g, b, a`.
fn channels(inner: &str, with_alpha: bool) -> Option<Rgba8> {
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    let expected = if with_alpha { 4 } else { 3 };
    if parts.len() != expected {
        return None;
    }
    let mut out = [u8::MAX; 4];
    for (slot, part) in out.iter_mut().zip(&parts).take(3) {
        *slot = channel(part)?;
    }
    if let (true, Some(alpha_text), Some(slot)) = (with_alpha, parts.get(3), out.get_mut(3)) {
        *slot = alpha(alpha_text)?;
    }
    Some(out)
}

/// A color channel: 0-255, or 0%-100%.
fn channel(text: &str) -> Option<u8> {
    let value = if let Some(percent) = text.strip_suffix('%') {
        number(percent)? / 100.0 * 255.0
    } else {
        number(text)?
    };
    to_byte(value, 255.0)
}

/// Alpha: 0-1, or 0%-100%.
fn alpha(text: &str) -> Option<u8> {
    let value = if let Some(percent) = text.strip_suffix('%') {
        number(percent)? / 100.0
    } else {
        number(text)?
    };
    to_byte(value * 255.0, 255.0)
}

fn number(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() || text.len() > 16 {
        return None;
    }
    text.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Round `value` to a byte if it is within 0..=max.
fn to_byte(value: f64, max: f64) -> Option<u8> {
    if !(0.0..=max).contains(&value) {
        return None;
    }
    // In 0..=255 after the check, so the cast cannot truncate.
    Some(value.round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_forms() {
        assert_eq!(parse("#ff8800"), Some([255, 136, 0, 255]));
        assert_eq!(parse("FF8800"), Some([255, 136, 0, 255]));
        assert_eq!(parse("#ff880080"), Some([255, 136, 0, 128]));
        assert_eq!(parse("#f80"), Some([255, 136, 0, 255]));
        assert_eq!(parse("#f808"), Some([255, 136, 0, 136]));
        assert_eq!(parse("  #ABCDEF  "), Some([0xAB, 0xCD, 0xEF, 255]));
    }

    #[test]
    fn rgb_and_rgba_forms() {
        assert_eq!(parse("rgb(255, 136, 0)"), Some([255, 136, 0, 255]));
        assert_eq!(parse("RGB(100%,50%,0%)"), Some([255, 128, 0, 255]));
        assert_eq!(parse("rgba(255, 136, 0, 0.5)"), Some([255, 136, 0, 128]));
        assert_eq!(parse("rgba(0,0,0,50%)"), Some([0, 0, 0, 128]));
        assert_eq!(parse("rgba (1, 2, 3, 1)"), Some([1, 2, 3, 255]));
    }

    #[test]
    fn rejects_everything_else() {
        let bad = [
            "",
            "#",
            "#12",
            "#12345",
            "#1234567",
            "#123456789",
            "#gg0000",
            "red",
            "rgb(256, 0, 0)",
            "rgb(-1, 0, 0)",
            "rgb(1, 2)",
            "rgb(1, 2, 3, 4)",
            "rgba(1, 2, 3)",
            "rgba(1, 2, 3, 2)",
            "rgb(nan, 0, 0)",
            "rgb(inf, 0, 0)",
            "rgb(1e400, 0, 0)",
            "rgb(101%, 0, 0)",
            "rgb(1, 2, 3",
            "\u{FF11}23456",
        ];
        for text in bad {
            assert_eq!(parse(text), None, "{text:?}");
        }
        assert_eq!(parse(&"f".repeat(10_000)), None);
        assert_eq!(parse(&format!("rgb({})", "1,".repeat(1000))), None);
    }

    #[test]
    fn formats_round_trip() {
        assert_eq!(format_hex([255, 136, 0, 255]), "#FF8800");
        assert_eq!(format_hex([255, 136, 0, 128]), "#FF880080");
        for rgba in [[0, 0, 0, 0], [1, 2, 3, 255], [200, 100, 50, 7]] {
            assert_eq!(parse(&format_hex(rgba)), Some(rgba));
        }
    }
}
