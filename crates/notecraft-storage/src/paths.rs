//! File name rules for pages and folders that are safe on Windows, macOS, and Linux
//! (docs/markdown.md section 2.3).
//!
//! A page's file name is its title with every character that is illegal on any of the three
//! systems replaced, so a notebook folder copied between systems keeps working. Nothing here
//! touches the disk: callers pass the names that already exist.

/// Longest file name, in bytes, that all three systems accept (most limit one name to 255).
pub const MAX_FILE_NAME_BYTES: usize = 255;
/// The name used when a title has nothing usable in it, as OneNote calls an untitled page.
pub const UNTITLED: &str = "Untitled Page";

/// Characters that are illegal in a file name on Windows (and `/` on every system).
const ILLEGAL: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Device names Windows reserves, with or without an extension, in any case.
const RESERVED_WORDS: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];

/// Whether `name` (a whole file name or a bare stem) is a Windows device name such as `CON`,
/// `nul.txt`, or `COM3`. Windows also reserves these with trailing spaces or dots.
pub fn is_reserved_windows_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    let stem = stem.trim_end_matches(' ');
    let upper = stem.to_uppercase();
    if RESERVED_WORDS.contains(&upper.as_str()) {
        return true;
    }
    let mut chars = upper.chars();
    let prefix: String = chars.by_ref().take(3).collect();
    if prefix != "COM" && prefix != "LPT" {
        return false;
    }
    // One digit, including the superscript digits Windows also reserves.
    match (chars.next(), chars.next()) {
        (Some(c), None) => c.is_ascii_digit() || matches!(c, '\u{b9}' | '\u{b2}' | '\u{b3}'),
        _ => false,
    }
}

/// Turn `name` into a name that is valid on Windows, macOS, and Linux:
///
/// - `<>:"/\|?*` and control characters become `-`;
/// - leading and trailing spaces are trimmed, and so are leading and trailing dots (a
///   trailing dot is illegal on Windows, a leading dot hides the file on macOS and Linux);
/// - a Windows device name (`CON`, `NUL`, `COM1`, ...) gets an underscore after the device
///   name, so `CON` becomes `CON_` and `nul.txt` becomes `nul_.txt`;
/// - the result is cut to [`MAX_FILE_NAME_BYTES`] bytes at a character boundary;
/// - an empty result becomes [`UNTITLED`].
pub fn sanitize_file_name(name: &str) -> String {
    sanitize_with_limit(name, MAX_FILE_NAME_BYTES)
}

/// The file name for a page titled `title` with extension `ext` (without the dot, such as
/// `md` or `notecraft`). The title part is sanitized and shortened so the whole name fits in
/// [`MAX_FILE_NAME_BYTES`].
pub fn page_file_name(title: &str, ext: &str) -> String {
    let budget = MAX_FILE_NAME_BYTES.saturating_sub(ext.len() + 1);
    format!("{}.{ext}", sanitize_with_limit(title, budget))
}

/// `stem.ext`, or `stem 2.ext`, `stem 3.ext`, ... for the first name not in `existing`.
/// Names are compared ignoring case, because Windows and macOS volumes usually do. The result
/// always fits in [`MAX_FILE_NAME_BYTES`] when `stem` came from [`page_file_name`] limits.
pub fn unique_file_name(stem: &str, ext: &str, existing: &[&str]) -> String {
    let taken = |candidate: &str| {
        existing
            .iter()
            .any(|name| name.to_lowercase() == candidate.to_lowercase())
    };
    let first = format!("{stem}.{ext}");
    if !taken(&first) {
        return first;
    }
    // At most `existing.len()` numbers can be taken, so this ends by then.
    let mut last = first;
    for n in 2..=existing.len().saturating_add(2) {
        let suffix = format!(" {n}");
        let room = MAX_FILE_NAME_BYTES.saturating_sub(ext.len() + 1 + suffix.len());
        let candidate = format!("{}{suffix}.{ext}", cut(stem, room));
        if !taken(&candidate) {
            return candidate;
        }
        last = candidate;
    }
    last
}

fn sanitize_with_limit(name: &str, max_bytes: usize) -> String {
    let replaced: String = name
        .chars()
        .map(|c| {
            if c.is_control() || ILLEGAL.contains(&c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    let mut out = trim_edges(&replaced).to_owned();
    if out.is_empty() {
        return UNTITLED.to_owned();
    }
    if is_reserved_windows_name(&out) {
        let at = out.find('.').unwrap_or(out.len());
        out.insert(at, '_');
    }
    let cut_name = cut(&out, max_bytes);
    let trimmed = trim_edges(cut_name);
    if trimmed.is_empty() {
        UNTITLED.to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Trim spaces and dots from both ends (and other whitespace, which is just as invisible).
fn trim_edges(text: &str) -> &str {
    text.trim_matches(|c: char| c == '.' || c.is_whitespace())
}

/// The longest prefix of `text` that is at most `max_bytes` bytes, on a character boundary.
fn cut(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.get(..end).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_names_are_unchanged() {
        for name in [
            "Meeting notes",
            "Q3 budget (draft)",
            "a.b",
            "caf\u{e9} \u{65e5}\u{672c}",
        ] {
            assert_eq!(sanitize_file_name(name), name);
        }
    }

    #[test]
    fn illegal_and_control_characters_become_hyphens() {
        assert_eq!(
            sanitize_file_name("a<b>c:d\"e/f\\g|h?i*j"),
            "a-b-c-d-e-f-g-h-i-j"
        );
        assert_eq!(
            sanitize_file_name("tab\there\nnew\u{0}nul\u{7f}del"),
            "tab-here-new-nul-del"
        );
        assert_eq!(sanitize_file_name("a\u{85}b"), "a-b");
    }

    #[test]
    fn trailing_dots_and_spaces_are_trimmed() {
        assert_eq!(sanitize_file_name("Notes. . "), "Notes");
        assert_eq!(sanitize_file_name("  Notes  "), "Notes");
        assert_eq!(sanitize_file_name("Notes..."), "Notes");
        assert_eq!(sanitize_file_name(".hidden"), "hidden");
        assert_eq!(sanitize_file_name("a . b"), "a . b");
    }

    #[test]
    fn empty_results_become_untitled() {
        for name in ["", " ", "...", ". .", "\u{a0}"] {
            assert_eq!(sanitize_file_name(name), UNTITLED, "{name:?}");
        }
        assert_eq!(page_file_name("", "md"), "Untitled Page.md");
    }

    #[test]
    fn windows_device_names_are_made_safe() {
        for (name, want) in [
            ("CON", "CON_"),
            ("con", "con_"),
            ("Nul", "Nul_"),
            ("PRN", "PRN_"),
            ("AUX", "AUX_"),
            ("COM1", "COM1_"),
            ("com9", "com9_"),
            ("LPT3", "LPT3_"),
            ("LPT\u{b9}", "LPT\u{b9}_"),
            ("nul.txt", "nul_.txt"),
            ("CON.tar.gz", "CON_.tar.gz"),
            ("CON ", "CON_"),
        ] {
            assert_eq!(sanitize_file_name(name), want, "{name:?}");
            assert!(is_reserved_windows_name(name), "{name:?}");
        }
        for name in [
            "CONSOLE", "COM10", "COM", "LPT", "Aux1", "my CON", "NULL", "CO",
        ] {
            assert!(!is_reserved_windows_name(name), "{name:?}");
            assert_eq!(sanitize_file_name(name), name, "{name:?}");
        }
    }

    #[test]
    fn page_names_get_their_extension_after_sanitizing() {
        assert_eq!(page_file_name("Plan: v2?", "md"), "Plan- v2-.md");
        assert_eq!(page_file_name("CON", "notecraft"), "CON_.notecraft");
        assert_eq!(page_file_name("Ends with dot.", "md"), "Ends with dot.md");
    }

    #[test]
    fn long_names_are_cut_on_a_character_boundary() {
        let long = "\u{65e5}".repeat(200); // 3 bytes each
        let name = sanitize_file_name(&long);
        assert!(name.len() <= MAX_FILE_NAME_BYTES);
        assert_eq!(name.chars().count(), 85);
        let file = page_file_name(&long, "notecraft");
        assert!(file.len() <= MAX_FILE_NAME_BYTES, "{}", file.len());
        assert!(file.ends_with(".notecraft"));
        let spaced = format!("{} {}", "a".repeat(254), "b");
        assert_eq!(sanitize_file_name(&spaced).len(), 254);
    }

    #[test]
    fn unique_names_count_up_and_ignore_case() {
        assert_eq!(unique_file_name("Plan", "md", &[]), "Plan.md");
        assert_eq!(unique_file_name("Plan", "md", &["Other.md"]), "Plan.md");
        assert_eq!(unique_file_name("Plan", "md", &["plan.MD"]), "Plan 2.md");
        assert_eq!(
            unique_file_name("Plan", "md", &["Plan.md", "Plan 2.md", "plan 3.md"]),
            "Plan 4.md"
        );
    }

    #[test]
    fn unique_names_stay_within_the_length_limit() {
        let stem = "x".repeat(MAX_FILE_NAME_BYTES - 3);
        let first = format!("{stem}.md");
        let name = unique_file_name(&stem, "md", &[first.as_str()]);
        assert!(name.len() <= MAX_FILE_NAME_BYTES);
        assert!(name.ends_with(" 2.md"));
    }

    #[test]
    fn sanitized_names_are_idempotent() {
        for name in [
            "CON",
            "a:b",
            " x. ",
            "",
            "\u{65e5}".repeat(300).as_str(),
            "nul.txt",
        ] {
            let once = sanitize_file_name(name);
            assert_eq!(sanitize_file_name(&once), once, "{name:?}");
        }
    }
}
