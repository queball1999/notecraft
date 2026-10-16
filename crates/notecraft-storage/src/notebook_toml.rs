//! Lenient reading of `.notecraft/notebook.toml` (docs/markdown.md section 2.1).
//!
//! Reading never fails the notebook. A file that is not valid TOML, too big, or unreadable
//! gives the default config and a [`ReadOutcome::Bad`] flag. A key with the wrong type or an
//! unusable value falls back to its default; the original value is kept in
//! [`NotebookConfig::extra`] together with every key this version does not know, so a later
//! writer can put them back unchanged.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use toml::{Table, Value};

/// Settings files bigger than this are not read.
pub const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
/// Entries read from a `sections` or `pages` list; more are dropped.
pub const MAX_LIST_ENTRIES: usize = 100_000;
/// Longest accepted id, in characters.
pub const MAX_ID_CHARS: usize = 128;
/// Longest accepted display name, in characters.
pub const MAX_NAME_CHARS: usize = 256;

/// What reading a settings file found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadOutcome {
    /// The file is not there. Not a problem.
    Missing,
    /// The file was read (individual keys may still have been defaulted).
    Read,
    /// The file could not be used: unreadable, too big, or not TOML.
    Bad,
}

/// The contents of `notebook.toml`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NotebookConfig {
    /// The `format` number, when present and an integer.
    pub format: Option<i64>,
    /// Stable notebook id.
    pub id: Option<String>,
    /// Display name, which may differ from the folder name.
    pub name: Option<String>,
    /// Notebook color as 0xRRGGBB.
    pub color: Option<u32>,
    /// Display order of top-level sections and groups, by folder name.
    pub sections: Vec<String>,
    /// Unknown keys, and known keys whose value was unusable, as read.
    pub extra: Table,
}

impl NotebookConfig {
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
        if let Some(value) = table.remove("id") {
            match value.as_str().and_then(clean_id) {
                Some(id) => config.id = Some(id),
                None => keep(&mut config.extra, "id", value),
            }
        }
        if let Some(value) = table.remove("name") {
            match value.as_str().and_then(clean_name) {
                Some(name) => config.name = Some(name),
                None => keep(&mut config.extra, "name", value),
            }
        }
        if let Some(value) = table.remove("color") {
            match value.as_str().and_then(parse_color) {
                Some(color) => config.color = Some(color),
                None => keep(&mut config.extra, "color", value),
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

/// Read `path` as `notebook.toml`.
pub fn read_notebook_config(path: &Path) -> (NotebookConfig, ReadOutcome) {
    match read_bounded(path) {
        Ok(None) => (NotebookConfig::default(), ReadOutcome::Missing),
        Ok(Some(text)) => match NotebookConfig::parse(&text) {
            Some(config) => (config, ReadOutcome::Read),
            None => (NotebookConfig::default(), ReadOutcome::Bad),
        },
        Err(()) => (NotebookConfig::default(), ReadOutcome::Bad),
    }
}

pub(crate) fn keep(extra: &mut Table, key: &str, value: Value) {
    extra.insert(key.to_owned(), value);
}

/// The text of `path`: `Ok(None)` when it does not exist, `Err` when it is unreadable, over
/// [`MAX_CONFIG_BYTES`], or not UTF-8.
pub(crate) fn read_bounded(path: &Path) -> Result<Option<String>, ()> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(());
    }
    let text = String::from_utf8(bytes).map_err(|_| ())?;
    // A byte order mark from some editors is not part of the TOML.
    Ok(Some(text.trim_start_matches('\u{feff}').to_owned()))
}

/// A color written `#RRGGBB` or `#RGB` (any case) as 0xRRGGBB.
pub fn parse_color(text: &str) -> Option<u32> {
    let digits = text.trim().strip_prefix('#')?;
    if !digits.is_ascii() || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match digits.len() {
        6 => u32::from_str_radix(digits, 16).ok(),
        3 => {
            let short = u32::from_str_radix(digits, 16).ok()?;
            let (r, g, b) = ((short >> 8) & 0xF, (short >> 4) & 0xF, short & 0xF);
            Some(((r * 0x11) << 16) | ((g * 0x11) << 8) | (b * 0x11))
        }
        _ => None,
    }
}

fn clean_id(text: &str) -> Option<String> {
    let text = text.trim();
    let ok = !text.is_empty()
        && text.chars().count() <= MAX_ID_CHARS
        && !text.chars().any(char::is_control);
    ok.then(|| text.to_owned())
}

fn clean_name(text: &str) -> Option<String> {
    let text = text.trim();
    let ok = !text.is_empty()
        && text.chars().count() <= MAX_NAME_CHARS
        && !text.chars().any(char::is_control);
    ok.then(|| text.to_owned())
}

/// A TOML array of strings, bounded. `None` when the value is not an array; entries that are
/// not strings are skipped.
pub(crate) fn name_list(value: &Value) -> Option<Vec<String>> {
    let array = value.as_array()?;
    Some(
        array
            .iter()
            .filter_map(Value::as_str)
            .take(MAX_LIST_ENTRIES)
            .map(str::to_owned)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_full_config() {
        let config = NotebookConfig::parse(
            r##"
format = 1
id = "nb_01J9Z3K8V6Q4"
name = "My Notebook"
color = "#7A4FB5"
sections = ["Work", "Projects"]
"##,
        )
        .unwrap();
        assert_eq!(config.format, Some(1));
        assert_eq!(config.id.as_deref(), Some("nb_01J9Z3K8V6Q4"));
        assert_eq!(config.name.as_deref(), Some("My Notebook"));
        assert_eq!(config.color, Some(0x7A4FB5));
        assert_eq!(config.sections, ["Work", "Projects"]);
        assert!(config.extra.is_empty());
    }

    #[test]
    fn unknown_keys_are_kept_and_bad_values_defaulted() {
        let config = NotebookConfig::parse(
            r##"
format = "one"
id = 7
name = "   "
color = "purple"
sections = ["A", 3, "B"]
future = { nested = true }
[table]
x = 1.5
"##,
        )
        .unwrap();
        assert_eq!(config.format, None);
        assert_eq!(config.id, None);
        assert_eq!(config.name, None);
        assert_eq!(config.color, None);
        assert_eq!(config.sections, ["A", "B"]);
        for key in ["format", "id", "name", "color", "future", "table"] {
            assert!(config.extra.contains_key(key), "{key} should be kept");
        }
        assert!(!config.extra.contains_key("sections"));
    }

    #[test]
    fn a_wrong_type_for_sections_is_kept_and_ignored() {
        let config = NotebookConfig::parse("sections = \"Work\"").unwrap();
        assert!(config.sections.is_empty());
        assert!(config.extra.contains_key("sections"));
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#7a4fb5"), Some(0x7A4FB5));
        assert_eq!(parse_color(" #FFF "), Some(0xFFFFFF));
        assert_eq!(parse_color("#abc"), Some(0xAABBCC));
        for bad in [
            "",
            "#",
            "7A4FB5",
            "#12345",
            "#GGGGGG",
            "#1234567",
            "#\u{e9}\u{e9}\u{e9}",
        ] {
            assert_eq!(parse_color(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn bad_files_give_defaults() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("notebook.toml");
        assert_eq!(read_notebook_config(&path).1, ReadOutcome::Missing);

        std::fs::write(&path, "this is = = not toml [").unwrap();
        let (config, outcome) = read_notebook_config(&path);
        assert_eq!(outcome, ReadOutcome::Bad);
        assert_eq!(config, NotebookConfig::default());

        std::fs::write(&path, [0xff, 0xfe, 0x00, 0x80]).unwrap();
        assert_eq!(read_notebook_config(&path).1, ReadOutcome::Bad);

        std::fs::write(&path, vec![b'#'; MAX_CONFIG_BYTES as usize + 1]).unwrap();
        assert_eq!(read_notebook_config(&path).1, ReadOutcome::Bad);

        std::fs::write(&path, "\u{feff}name = \"With BOM\"").unwrap();
        let (config, outcome) = read_notebook_config(&path);
        assert_eq!(outcome, ReadOutcome::Read);
        assert_eq!(config.name.as_deref(), Some("With BOM"));
    }

    #[test]
    fn long_lists_are_bounded() {
        let items: Vec<String> = (0..MAX_LIST_ENTRIES + 10)
            .map(|i| format!("\"s{i}\""))
            .collect();
        let text = format!("sections = [{}]", items.join(","));
        let config = NotebookConfig::parse(&text).unwrap();
        assert_eq!(config.sections.len(), MAX_LIST_ENTRIES);
    }
}
