//! The What's New notes: one entry per released version, compiled in from `WHATS_NEW.md` at
//! the repository root, and the rule for when the dialog opens by itself.
//!
//! The reader is lenient and bounded: unknown lines are plain paragraphs, long lines are cut,
//! and the number of entries and lines is capped, so a damaged file never fails or grows
//! without limit.

/// The notes compiled into the binary.
pub const BUNDLED: &str = include_str!("../../../WHATS_NEW.md");

/// Most bytes of source read.
pub const MAX_SOURCE_BYTES: usize = 64 * 1024;
/// Most versions kept; later ones are dropped.
pub const MAX_ENTRIES: usize = 50;
/// Most lines kept for one version.
pub const MAX_ITEMS: usize = 40;
/// Most characters kept from one line; the rest is cut.
pub const MAX_LINE_CHARS: usize = 300;
/// Most characters kept from a version label.
pub const MAX_VERSION_CHARS: usize = 40;

/// One line of an entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// A `### ` line.
    Heading(String),
    /// A `- ` line.
    Bullet(String),
    /// Any other non-empty line.
    Text(String),
}

/// The notes for one version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub version: String,
    pub items: Vec<Item>,
}

/// All entries, newest first (the order of the file).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReleaseNotes {
    entries: Vec<Entry>,
}

fn clip(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

impl ReleaseNotes {
    /// The notes compiled into the binary.
    pub fn bundled() -> Self {
        Self::parse(BUNDLED)
    }

    /// Read notes from `source`: `## <version>` starts an entry, `- ` is a bullet, `### `
    /// a heading, anything else non-empty a paragraph. Text before the first `## ` is
    /// ignored.
    pub fn parse(source: &str) -> Self {
        let mut end = source.len().min(MAX_SOURCE_BYTES);
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        let mut entries: Vec<Entry> = Vec::new();
        for line in source[..end].lines() {
            let line = line.trim_end();
            if let Some(version) = line.strip_prefix("## ").map(str::trim) {
                if version.is_empty() {
                    continue;
                }
                if entries.len() >= MAX_ENTRIES {
                    break;
                }
                entries.push(Entry {
                    version: clip(version, MAX_VERSION_CHARS),
                    items: Vec::new(),
                });
                continue;
            }
            let Some(entry) = entries.last_mut() else {
                continue;
            };
            let text = line.trim_start();
            if text.is_empty() || entry.items.len() >= MAX_ITEMS {
                continue;
            }
            let item = if let Some(rest) = text.strip_prefix("### ") {
                Item::Heading(clip(rest, MAX_LINE_CHARS))
            } else if let Some(rest) = text.strip_prefix("- ") {
                Item::Bullet(clip(rest, MAX_LINE_CHARS))
            } else {
                Item::Text(clip(text, MAX_LINE_CHARS))
            };
            entry.items.push(item);
        }
        Self { entries }
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry for exactly `version`, if there is one.
    pub fn entry(&self, version: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.version == version.trim())
    }
}

/// Whether the dialog should open by itself at launch: the stored version is not the running
/// one. This covers the first launch ever (nothing stored), an update, a downgrade, and a
/// stored value that makes no sense, because in all of them the user has not been shown this
/// version.
pub fn should_show_at_launch(stored: &str, current: &str) -> bool {
    stored.trim() != current.trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_file_parses_and_has_the_current_version() {
        let notes = ReleaseNotes::bundled();
        assert!(BUNDLED.is_ascii());
        assert!(notes.entries().len() >= 2);
        assert!(!notes.entries()[0].items.is_empty());
        assert!(notes.entry("0.0.1").is_some());
        // Between releases Cargo.toml says 0.0.0; the release commit sets the real version
        // and this catches a release made without its notes (docs/releasing.md).
        let version = env!("CARGO_PKG_VERSION");
        assert!(
            version == "0.0.0" || notes.entry(version).is_some(),
            "WHATS_NEW.md needs a section for the version in Cargo.toml"
        );
    }

    #[test]
    fn lines_become_items_and_the_preface_is_ignored() {
        let notes = ReleaseNotes::parse(
            "# T\nignored\n## 1.0\nhello\n### Part\n- one\n\n- two\n## 0.9\n- old\n",
        );
        assert_eq!(notes.entries().len(), 2);
        assert_eq!(
            notes.entries()[0].items,
            vec![
                Item::Text("hello".into()),
                Item::Heading("Part".into()),
                Item::Bullet("one".into()),
                Item::Bullet("two".into()),
            ]
        );
        assert_eq!(notes.entry("0.9").map(|e| e.items.len()), Some(1));
    }

    #[test]
    fn size_and_counts_are_bounded() {
        let mut src = String::new();
        for v in 0..(MAX_ENTRIES + 20) {
            src.push_str(&format!("## 1.{v}\n"));
            for i in 0..(MAX_ITEMS + 10) {
                src.push_str(&format!("- item {i}\n"));
            }
        }
        let notes = ReleaseNotes::parse(&src);
        assert_eq!(notes.entries().len(), MAX_ENTRIES);
        assert!(notes.entries().iter().all(|e| e.items.len() == MAX_ITEMS));
        let long = format!("## {}\n- {}\n", "v".repeat(500), "x".repeat(5000));
        let notes = ReleaseNotes::parse(&long);
        assert_eq!(
            notes.entries()[0].version.chars().count(),
            MAX_VERSION_CHARS
        );
        assert_eq!(
            notes.entries()[0].items,
            vec![Item::Bullet("x".repeat(MAX_LINE_CHARS))]
        );
        // A huge source of multi-byte text is cut on a character boundary.
        let big = format!("## 1\n{}", "\u{e9}".repeat(MAX_SOURCE_BYTES));
        assert_eq!(ReleaseNotes::parse(&big).entries().len(), 1);
    }

    #[test]
    fn junk_input_gives_no_entries_and_no_panic() {
        assert!(ReleaseNotes::parse("").is_empty());
        assert!(ReleaseNotes::parse("just text\n- bullet\n").is_empty());
        assert!(ReleaseNotes::parse("##\n## \n### x\n").is_empty());
    }

    #[test]
    fn launch_rule() {
        assert!(should_show_at_launch("", "0.0.2"));
        assert!(!should_show_at_launch("0.0.2", "0.0.2"));
        assert!(should_show_at_launch("0.0.1", "0.0.2"));
        assert!(should_show_at_launch("0.0.3", "0.0.2"));
        assert!(should_show_at_launch("garbage", "0.0.2"));
    }
}
