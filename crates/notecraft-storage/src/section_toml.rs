//! Lenient reading of `.section.toml` (docs/markdown.md section 2.2).
//!
//! Same rules as `notebook.toml`: never fatal, unknown keys and unusable values are kept in
//! [`SectionConfig::extra`], bad entries in a list are skipped, and levels are clamped.

use std::path::Path;

use toml::{Table, Value};

use crate::notebook_dir::MAX_PAGE_LEVEL;
use crate::notebook_toml::{
    keep, name_list, parse_color, read_bounded, ReadOutcome, MAX_LIST_ENTRIES,
};

/// The name of the settings file inside a section or group folder.
pub const SECTION_FILE_NAME: &str = ".section.toml";

/// What a `.section.toml` says the folder is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionKind {
    Section,
    Group,
}

/// One entry of a section's `pages` list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageListing {
    /// The page file name, for example `Meeting notes.notecraft`.
    pub file: String,
    /// 0 for a page, 1 for a subpage, 2 for a sub-subpage.
    pub level: u8,
    /// True when the page's subpages are folded away in the page list.
    pub collapsed: bool,
}

/// The contents of `.section.toml`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SectionConfig {
    pub format: Option<i64>,
    pub kind: Option<SectionKind>,
    /// Tab color as 0xRRGGBB.
    pub color: Option<u32>,
    /// Page order and levels, for a section.
    pub pages: Vec<PageListing>,
    /// Order of child sections and groups by folder name, for a group.
    pub sections: Vec<String>,
    /// Unknown keys, and known keys whose value was unusable, as read.
    pub extra: Table,
}

impl SectionConfig {
    /// Parse `text`. `None` when it is not valid TOML.
    pub fn parse(text: &str) -> Option<Self> {
        let table: Table = toml::from_str(text).ok()?;
        Some(Self::from_table(table))
    }

    fn from_table(mut table: Table) -> Self {
        let mut config = Self::default();
        if let Some(value) = table.remove("format") {
            match value {
                Value::Integer(n) => config.format = Some(n),
                other => keep(&mut config.extra, "format", other),
            }
        }
        if let Some(value) = table.remove("kind") {
            match value.as_str().and_then(parse_kind) {
                Some(kind) => config.kind = Some(kind),
                None => keep(&mut config.extra, "kind", value),
            }
        }
        if let Some(value) = table.remove("color") {
            match value.as_str().and_then(parse_color) {
                Some(color) => config.color = Some(color),
                None => keep(&mut config.extra, "color", value),
            }
        }
        if let Some(value) = table.remove("pages") {
            match page_list(&value) {
                Some(list) => config.pages = list,
                None => keep(&mut config.extra, "pages", value),
            }
        }
        if let Some(value) = table.remove("sections") {
            match name_list(&value) {
                Some(list) => config.sections = list,
                None => keep(&mut config.extra, "sections", value),
            }
        }
        config.extra.extend(table);
        config
    }
}

/// Read `path` as `.section.toml`.
pub fn read_section_config(path: &Path) -> (SectionConfig, ReadOutcome) {
    match read_bounded(path) {
        Ok(None) => (SectionConfig::default(), ReadOutcome::Missing),
        Ok(Some(text)) => match SectionConfig::parse(&text) {
            Some(config) => (config, ReadOutcome::Read),
            None => (SectionConfig::default(), ReadOutcome::Bad),
        },
        Err(()) => (SectionConfig::default(), ReadOutcome::Bad),
    }
}

fn parse_kind(text: &str) -> Option<SectionKind> {
    match text.trim().to_ascii_lowercase().as_str() {
        "section" => Some(SectionKind::Section),
        "group" => Some(SectionKind::Group),
        _ => None,
    }
}

/// `pages` entries are tables with `file` and `level`; a bare string is a page at level 0.
/// `None` when the value is not an array.
fn page_list(value: &Value) -> Option<Vec<PageListing>> {
    let array = value.as_array()?;
    Some(
        array
            .iter()
            .filter_map(page_listing)
            .take(MAX_LIST_ENTRIES)
            .collect(),
    )
}

fn page_listing(value: &Value) -> Option<PageListing> {
    match value {
        Value::String(file) => Some(PageListing {
            file: file.clone(),
            level: 0,
            collapsed: false,
        }),
        Value::Table(table) => {
            let file = table.get("file")?.as_str()?.to_owned();
            let level = table
                .get("level")
                .and_then(Value::as_integer)
                .map_or(0, clamp_level);
            let collapsed = table
                .get("collapsed")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Some(PageListing {
                file,
                level,
                collapsed,
            })
        }
        _ => None,
    }
}

fn clamp_level(level: i64) -> u8 {
    u8::try_from(level.clamp(0, i64::from(MAX_PAGE_LEVEL))).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_section() {
        let config = SectionConfig::parse(
            r##"
format = 1
kind = "section"
color = "#3C8DBC"
pages = [
  { file = "Meeting notes.notecraft", level = 0 },
  { file = "Action items.md",     level = 1 },
]
"##,
        )
        .unwrap();
        assert_eq!(config.kind, Some(SectionKind::Section));
        assert_eq!(config.color, Some(0x3C8DBC));
        assert_eq!(
            config.pages,
            [
                PageListing {
                    file: "Meeting notes.notecraft".into(),
                    level: 0,
                    collapsed: false
                },
                PageListing {
                    file: "Action items.md".into(),
                    level: 1,
                    collapsed: false
                },
            ]
        );
    }

    #[test]
    fn collapsed_flag_is_read_leniently() {
        let config = SectionConfig::parse(
            r##"
pages = [
  { file = "a.md", collapsed = true },
  { file = "b.md", collapsed = "yes" },
  { file = "c.md", collapsed = 1 },
  { file = "d.md", collapsed = false },
  "e.md",
]
"##,
        )
        .unwrap();
        let flags: Vec<bool> = config.pages.iter().map(|p| p.collapsed).collect();
        assert_eq!(flags, [true, false, false, false, false]);
    }

    #[test]
    fn reads_a_group() {
        let config = SectionConfig::parse("kind = \"Group\"\nsections = [\"B\", \"A\"]").unwrap();
        assert_eq!(config.kind, Some(SectionKind::Group));
        assert_eq!(config.sections, ["B", "A"]);
    }

    #[test]
    fn bad_values_are_defaulted_and_kept() {
        let config = SectionConfig::parse(
            r##"
kind = "folder"
color = 12
pages = [
  "bare.md",
  { file = "deep.md", level = 9 },
  { file = "neg.md", level = -4 },
  { file = "text-level.md", level = "two" },
  { file = 5 },
  { level = 1 },
  7,
]
mood = "calm"
"##,
        )
        .unwrap();
        assert_eq!(config.kind, None);
        assert_eq!(config.color, None);
        let levels: Vec<(&str, u8)> = config
            .pages
            .iter()
            .map(|p| (p.file.as_str(), p.level))
            .collect();
        assert_eq!(
            levels,
            [
                ("bare.md", 0),
                ("deep.md", 2),
                ("neg.md", 0),
                ("text-level.md", 0)
            ]
        );
        for key in ["kind", "color", "mood"] {
            assert!(config.extra.contains_key(key), "{key}");
        }
    }

    #[test]
    fn pages_that_is_not_a_list_is_kept() {
        let config = SectionConfig::parse("pages = \"all\"").unwrap();
        assert!(config.pages.is_empty());
        assert!(config.extra.contains_key("pages"));
    }

    #[test]
    fn hostile_input_gives_defaults() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join(SECTION_FILE_NAME);
        assert_eq!(read_section_config(&path).1, ReadOutcome::Missing);
        std::fs::write(&path, "pages = [[[[[[").unwrap();
        let (config, outcome) = read_section_config(&path);
        assert_eq!(outcome, ReadOutcome::Bad);
        assert_eq!(config, SectionConfig::default());
        let deep = format!("pages = {}1{}", "[".repeat(5000), "]".repeat(5000));
        std::fs::write(&path, deep).unwrap();
        // Either parsed (nothing usable inside) or rejected; never a panic.
        let _ = read_section_config(&path);
    }
}
