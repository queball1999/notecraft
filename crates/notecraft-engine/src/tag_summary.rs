//! The Tags Summary (build_plan/04 PR 2.6): the tagged items of a scope, grouped by tag name,
//! section, page title, or date, and the text of the summary page made from them.
//!
//! The query reads each notebook's `.notecraft/index.sqlite` (the `tags` table of PR 2.5), so
//! it never touches the page files. It takes plain inputs (folders, the current selection,
//! a scope), the way [`crate::search::search`] does, so the command line can call it too.
//!
//! The index keeps one modified time per page, not one per tag, so the Date grouping uses the
//! day the page was last modified (OneNote groups by the day each tag was applied, which a
//! plain text file cannot record).

#[cfg(not(target_arch = "wasm32"))]
use crate::search::SearchRoot;
use crate::search::{SearchContext, SearchScope};

/// How the items are grouped, as in OneNote's "Group tags by" list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SummaryGroup {
    /// By tag name, in alphabetical order.
    #[default]
    Tag,
    /// By section, in notebook order.
    Section,
    /// By page title, in alphabetical order.
    Title,
    /// By the day the page was modified, newest first.
    Date,
}

impl SummaryGroup {
    pub const LIST: [SummaryGroup; 4] = [Self::Tag, Self::Section, Self::Title, Self::Date];

    /// The id used in command arguments.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Tag => "tag",
            Self::Section => "section",
            Self::Title => "title",
            Self::Date => "date",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::LIST.into_iter().find(|group| group.id() == id)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Tag => "Tag name",
            Self::Section => "Section",
            Self::Title => "Page title",
            Self::Date => "Date",
        }
    }
}

/// One tagged item: a paragraph, heading, or list item that holds a tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryItem {
    pub notebook_id: String,
    pub notebook_name: String,
    /// The section id (`<notebook id>/<folder path>`), as the navigation uses it.
    pub section_id: String,
    pub section_name: String,
    pub page_id: String,
    pub page_title: String,
    /// Page file modified time, milliseconds since the Unix epoch.
    pub modified: i64,
    /// The tag id (what is stored after the `#`, `todo` for a task list item).
    pub tag: String,
    /// The tag name shown to the user.
    pub tag_name: String,
    /// For a To Do: whether it is checked. `None` for other tags.
    pub checked: Option<bool>,
    /// The paragraph text without markdown syntax or tag names.
    pub text: String,
    /// Where the paragraph starts in the page source (bytes), as the index last read it.
    pub at: usize,
}

/// A run of items under one heading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemGroup {
    pub label: String,
    pub items: Vec<SummaryItem>,
}

/// What a tags query found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TagSummary {
    pub scope: SearchScope,
    pub group: SummaryGroup,
    pub only_unchecked: bool,
    pub groups: Vec<ItemGroup>,
    /// More items matched than the list holds.
    pub truncated: bool,
}

impl TagSummary {
    /// How many items there are in all groups.
    pub fn count(&self) -> usize {
        self.groups.iter().map(|g| g.items.len()).sum()
    }
}

/// What the Tags Summary pane shows and remembers (the session holds one).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagSummaryState {
    /// Whether the pane is shown.
    pub open: bool,
    pub scope: SearchScope,
    pub group: SummaryGroup,
    /// "Show only unchecked items".
    pub only_unchecked: bool,
    pub summary: Option<TagSummary>,
}

impl Default for TagSummaryState {
    fn default() -> Self {
        Self {
            open: false,
            scope: SearchScope::Notebook,
            group: SummaryGroup::Tag,
            only_unchecked: false,
            summary: None,
        }
    }
}

/// Most items a query returns in all.
pub const MAX_ITEMS: usize = 2_000;

/// `YYYY-MM-DD` for milliseconds since the Unix epoch (UTC).
pub fn date_text(millis: i64) -> String {
    let days = millis.div_euclid(86_400_000);
    // Civil date from a day count (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Put `items` (in notebook order) under headings. `multi` says several notebooks are shown,
/// so a section heading names its notebook too.
pub fn group_items(items: Vec<SummaryItem>, group: SummaryGroup, multi: bool) -> Vec<ItemGroup> {
    // (sort key, group key, heading), in first-seen order of the group key.
    let mut order: Vec<(String, String, String)> = Vec::new();
    let mut members: Vec<Vec<SummaryItem>> = Vec::new();
    for item in items {
        let (key, sort, label) = match group {
            SummaryGroup::Tag => (
                item.tag.clone(),
                item.tag_name.to_lowercase(),
                item.tag_name.clone(),
            ),
            SummaryGroup::Section => {
                let label = if multi {
                    format!("{} > {}", item.notebook_name, item.section_name)
                } else {
                    item.section_name.clone()
                };
                (item.section_id.clone(), String::new(), label)
            }
            SummaryGroup::Title => (
                item.page_id.clone(),
                item.page_title.to_lowercase(),
                item.page_title.clone(),
            ),
            SummaryGroup::Date => {
                let label = date_text(item.modified);
                (label.clone(), label.clone(), label)
            }
        };
        let at = match order.iter().position(|(_, k, _)| *k == key) {
            Some(at) => at,
            None => {
                order.push((sort, key, label));
                members.push(Vec::new());
                order.len() - 1
            }
        };
        if let Some(list) = members.get_mut(at) {
            list.push(item);
        }
    }
    let mut groups: Vec<((String, String), ItemGroup)> = order
        .into_iter()
        .zip(members)
        .map(|((sort, key, label), items)| ((sort, key), ItemGroup { label, items }))
        .collect();
    match group {
        SummaryGroup::Tag | SummaryGroup::Title => groups.sort_by(|a, b| a.0.cmp(&b.0)),
        SummaryGroup::Date => groups.sort_by(|a, b| b.0.cmp(&a.0)),
        SummaryGroup::Section => {}
    }
    groups.into_iter().map(|(_, group)| group).collect()
}

/// The items of `roots` in `scope`, relative to `context`, grouped by `group`.
///
/// `tag_name` gives the display name of a tag id. A notebook whose index cannot be read is
/// skipped. A scope that needs a page or section that is not selected finds nothing.
#[cfg(not(target_arch = "wasm32"))]
pub fn tag_summary(
    roots: &[SearchRoot],
    context: &SearchContext,
    scope: SearchScope,
    group: SummaryGroup,
    only_unchecked: bool,
    tag_name: &dyn Fn(&str) -> String,
) -> TagSummary {
    use notecraft_storage::index::{Index, TagScope};
    use notecraft_storage::index_path;

    let mut summary = TagSummary {
        scope,
        group,
        only_unchecked,
        ..TagSummary::default()
    };
    let mut items: Vec<SummaryItem> = Vec::new();
    for root in roots {
        let current = context.notebook.as_deref() == Some(root.id.as_str());
        let section = context
            .section
            .as_deref()
            .and_then(|id| crate::search::section_path(&root.id, id));
        let tag_scope = match scope {
            SearchScope::All => Some(TagScope::Notebook),
            SearchScope::Notebook => current.then_some(TagScope::Notebook),
            SearchScope::Page => context
                .page
                .as_deref()
                .filter(|_| current)
                .map(|id| TagScope::Page(id.to_owned())),
            SearchScope::Section => section
                .filter(|_| current)
                .map(|path| TagScope::Folder(path.to_owned())),
            SearchScope::Group => section.filter(|_| current).map(|path| {
                path.rsplit_once('/')
                    .map_or(TagScope::Notebook, |(parent, _)| {
                        TagScope::Folder(parent.to_owned())
                    })
            }),
        };
        let Some(tag_scope) = tag_scope else {
            continue;
        };
        let db = index_path(&root.path);
        if !db.is_file() {
            continue;
        }
        let rows = match Index::open(&db).and_then(|index| index.tagged_items(&tag_scope, &[])) {
            Ok(rows) => rows,
            Err(err) => {
                tracing::warn!("tags in {} failed: {err}", root.name);
                continue;
            }
        };
        for row in rows {
            if only_unchecked && row.checked == Some(true) {
                continue;
            }
            let section_name = row
                .section
                .rsplit('/')
                .next()
                .unwrap_or(&row.section)
                .to_owned();
            items.push(SummaryItem {
                notebook_id: root.id.clone(),
                notebook_name: root.name.clone(),
                section_id: format!("{}/{}", root.id, row.section),
                section_name,
                page_id: row.page_id,
                page_title: row.page_title,
                modified: row.modified,
                tag_name: tag_name(&row.tag),
                tag: row.tag,
                checked: row.checked,
                text: row.text,
                at: row.holder_start,
            });
        }
    }
    summary.truncated = items.len() > MAX_ITEMS;
    items.truncate(MAX_ITEMS);
    let multi = roots.len() > 1 && scope == SearchScope::All;
    summary.groups = group_items(items, group, multi);
    summary
}

/// An item's text made safe to write as markdown on one line: white space is collapsed, a
/// word that would become a tag and a `[[` that would become a page link are escaped.
pub fn escape_item_text(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = String::with_capacity(flat.len() + 8);
    let mut prev_space = true;
    let mut chars = flat.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '#' if prev_space && chars.peek().is_some_and(char::is_ascii_alphabetic) => {
                out.push_str("\\#");
            }
            '[' if chars.peek() == Some(&'[') => out.push_str("\\["),
            c => out.push(c),
        }
        prev_space = c == ' ';
    }
    out
}

/// The markdown of a summary page: one heading per group and one list item per tagged item,
/// each keeping its tag (so it shows the same icon) and ending with a link to its page.
/// `link` gives the text between `[[` and `]]` for an item's page, or `None` when the page
/// cannot be linked to (it is in another notebook); the page title is written instead.
pub fn summary_page_text(
    summary: &TagSummary,
    link: &dyn Fn(&SummaryItem) -> Option<String>,
) -> String {
    let mut out = String::new();
    for group in &summary.groups {
        if group.items.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("## ");
        out.push_str(&escape_item_text(&group.label));
        out.push_str("\n\n");
        for item in &group.items {
            out.push_str("- ");
            if item.tag == "todo" {
                out.push_str(if item.checked == Some(true) {
                    "[x]"
                } else {
                    "[ ]"
                });
            } else {
                out.push('#');
                out.push_str(&item.tag);
            }
            let text = escape_item_text(&item.text);
            if !text.is_empty() {
                out.push(' ');
                out.push_str(&text);
            }
            out.push(' ');
            match link(item) {
                Some(target) => {
                    out.push_str("[[");
                    out.push_str(&target);
                    out.push_str("]]");
                }
                None => out.push_str(&escape_item_text(&item.page_title)),
            }
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(tag: &str, section: &str, page: &str, modified: i64, text: &str) -> SummaryItem {
        SummaryItem {
            notebook_id: "nb".into(),
            notebook_name: "Notes".into(),
            section_id: format!("nb/{section}"),
            section_name: section.into(),
            page_id: format!("nb/{section}/{page}.md"),
            page_title: page.into(),
            modified,
            tag: tag.into(),
            tag_name: match tag {
                "todo" => "To Do".into(),
                "idea" => "Idea".into(),
                other => other.into(),
            },
            checked: (tag == "todo").then_some(false),
            text: text.into(),
            at: 0,
        }
    }

    fn sample() -> Vec<SummaryItem> {
        vec![
            item("todo", "Work", "Alpha", 86_400_000 * 3, "call Sam"),
            item("idea", "Work", "Alpha", 86_400_000 * 3, "new logo"),
            item("todo", "Home", "Beta", 86_400_000, "buy milk"),
        ]
    }

    fn labels(groups: &[ItemGroup]) -> Vec<(&str, usize)> {
        groups
            .iter()
            .map(|g| (g.label.as_str(), g.items.len()))
            .collect()
    }

    #[test]
    fn group_ids_round_trip() {
        for group in SummaryGroup::LIST {
            assert_eq!(SummaryGroup::from_id(group.id()), Some(group));
        }
        assert_eq!(SummaryGroup::from_id("nope"), None);
    }

    #[test]
    fn grouping_orders_each_way() {
        let by_tag = group_items(sample(), SummaryGroup::Tag, false);
        assert_eq!(labels(&by_tag), [("Idea", 1), ("To Do", 2)]);
        let by_section = group_items(sample(), SummaryGroup::Section, false);
        assert_eq!(labels(&by_section), [("Work", 2), ("Home", 1)]);
        let by_title = group_items(sample(), SummaryGroup::Title, false);
        assert_eq!(labels(&by_title), [("Alpha", 2), ("Beta", 1)]);
        let by_date = group_items(sample(), SummaryGroup::Date, false);
        assert_eq!(labels(&by_date), [("1970-01-04", 2), ("1970-01-02", 1)]);
        let multi = group_items(sample(), SummaryGroup::Section, true);
        assert_eq!(multi[0].label, "Notes > Work");
    }

    #[test]
    fn dates_are_utc_calendar_days() {
        assert_eq!(date_text(0), "1970-01-01");
        assert_eq!(date_text(1_709_208_000_000), "2024-02-29");
        assert_eq!(date_text(-1), "1969-12-31");
    }

    #[test]
    fn item_text_is_escaped_for_one_line() {
        assert_eq!(escape_item_text("a\n  b"), "a b");
        assert_eq!(escape_item_text("#idea and a#b"), "\\#idea and a#b");
        assert_eq!(escape_item_text("see [[Page]]"), "see \\[[Page]]");
        assert_eq!(escape_item_text("# 5 items"), "# 5 items");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_summary_page_keeps_tags_and_links_back() {
        let mut checked = item("todo", "Home", "Beta", 0, "done thing");
        checked.checked = Some(true);
        let mut items = sample();
        items.push(checked);
        let summary = TagSummary {
            groups: group_items(items, SummaryGroup::Tag, false),
            ..TagSummary::default()
        };
        let text = summary_page_text(&summary, &|it| {
            (it.page_title != "Beta").then(|| it.page_title.clone())
        });
        assert_eq!(
            text,
            "## Idea\n\n- #idea new logo [[Alpha]]\n\n\
             ## To Do\n\n- [ ] call Sam [[Alpha]]\n- [ ] buy milk Beta\n- [x] done thing Beta\n"
        );
        // The text reads back as the same tags.
        let rows = crate::tags_index::rows_for_text(&text);
        let tags: Vec<(&str, Option<bool>)> =
            rows.iter().map(|r| (r.tag.as_str(), r.checked)).collect();
        assert_eq!(
            tags,
            [
                ("idea", None),
                ("todo", Some(false)),
                ("todo", Some(false)),
                ("todo", Some(true))
            ]
        );
    }
}
