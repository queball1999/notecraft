//! The writing style of a page: which line ending the file already uses, so new text matches
//! the text around it.

/// A line ending.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineEnding {
    #[default]
    Lf,
    Crlf,
}

impl LineEnding {
    /// The dominant line ending of `text`: CRLF only if more lines end in CRLF than in a bare
    /// LF. A text with no line ending, or a tie, is LF (the canonical style).
    pub fn detect(text: &str) -> Self {
        let (mut crlf, mut lf) = (0usize, 0usize);
        let mut previous = 0u8;
        for byte in text.bytes() {
            if byte == b'\n' {
                if previous == b'\r' {
                    crlf += 1;
                } else {
                    lf += 1;
                }
            }
            previous = byte;
        }
        if crlf > lf {
            Self::Crlf
        } else {
            Self::Lf
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }

    /// Rewrite every line ending in `text` (`\n`, `\r\n`, or a lone `\r`) to this one.
    pub fn normalize(self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    out.push_str(self.as_str());
                }
                '\n' => out.push_str(self.as_str()),
                other => out.push(other),
            }
        }
        out
    }
}

/// The style new text is written in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub eol: LineEnding,
}

impl Style {
    /// Detect the style of an existing page.
    pub fn detect(text: &str) -> Self {
        Self {
            eol: LineEnding::detect(text),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_dominant_line_ending() {
        assert_eq!(LineEnding::detect("a\nb\n"), LineEnding::Lf);
        assert_eq!(LineEnding::detect("a\r\nb\r\n"), LineEnding::Crlf);
        assert_eq!(LineEnding::detect("a\r\nb\nc\n"), LineEnding::Lf);
        assert_eq!(LineEnding::detect("a\r\nb\r\nc\n"), LineEnding::Crlf);
        assert_eq!(LineEnding::detect("no newline"), LineEnding::Lf);
        assert_eq!(LineEnding::detect(""), LineEnding::Lf);
    }

    #[test]
    fn normalize_rewrites_every_ending() {
        assert_eq!(
            LineEnding::Crlf.normalize("a\nb\r\nc\rd"),
            "a\r\nb\r\nc\r\nd"
        );
        assert_eq!(LineEnding::Lf.normalize("a\r\nb\r\n"), "a\nb\n");
    }
}
