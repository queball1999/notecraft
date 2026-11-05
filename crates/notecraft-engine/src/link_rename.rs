//! Keeping page links working when pages are renamed or moved (build_plan/04 PR 2.3,
//! docs/markdown.md section 5.4).
//!
//! A pure planner: given every page of a notebook with the path it had and the path it gets,
//! and the text of each page, it finds the links whose answer would change and says which
//! bytes to replace so each one still points at the same page. It touches no files and no
//! session, so the app (before a rename, text from memory or disk) and the refresh thread
//! (after an outside rename, text from disk) share it.
//!
//! Rules for one link, written on page `Q`:
//!
//! - It must resolve now (from `Q` at its old path, against the old page list). A link that
//!   points nowhere is left alone.
//! - If it resolves to the same page from `Q` at its new path against the new list, nothing
//!   changes. Otherwise the target is replaced by the shortest form that resolves to that page
//!   (`LinkIndex::shortest_form`).
//! - Shown text: a link with display text keeps it. A link without display text whose target
//!   was the plain name of the page updates to the new name (the visible text follows the
//!   rename, as in OneNote). Any other link without display text (`Section/Page`, an alias)
//!   keeps what it showed, so `[[New Section/Page]]` becomes `[[New Section/Page|Section/Page]]`.
//! - Only the replaced spans change; every other byte of the page stays as it was.

use notecraft_markdown::{fold_link_text, LinkIndex, LinkPage, Resolved};

/// Most pages read by one plan; beyond this the rest are skipped and reported.
pub const MAX_PLAN_PAGES: usize = 20_000;

/// One page of the notebook, as the planner sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanPage {
    /// The page id as it is now (a key; ids are paths in the app).
    pub id: String,
    /// Where the page's file is now, relative to the notebook folder.
    pub rel: String,
    /// The path links were written against.
    pub old: String,
    /// The path the page has after the change.
    pub new: String,
    pub aliases: Vec<String>,
}

/// Why a page with links to fix was not changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// Cut short or not UTF-8: NoteCraft never writes such a page.
    ReadOnly,
    /// The page has unsaved text that clashes with a newer file.
    Conflict,
    /// The page could not be read.
    Unreadable,
}

impl SkipReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::Conflict => "changed outside NoteCraft",
            Self::Unreadable => "cannot be read",
        }
    }
}

/// The text of a page for the planner.
pub struct PageSource {
    pub text: String,
    /// Set when the page must not be written.
    pub blocked: Option<SkipReason>,
}

/// One replacement in a page's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub at: usize,
    pub removed: String,
    pub inserted: String,
}

/// The edits for one page, highest offset first (so each offset is valid when applied in
/// order).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageEdits {
    pub id: String,
    pub rel: String,
    pub edits: Vec<Edit>,
}

/// What the planner decided.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RewritePlan {
    pub pages: Vec<PageEdits>,
    /// Links that will be rewritten.
    pub links: usize,
    /// Pages with links to fix that cannot be written (page id and why).
    pub skipped: Vec<(String, SkipReason)>,
    /// Links that could not be rewritten safely (no form resolves, or the name cannot be
    /// written as a link).
    pub unfixable: usize,
}

impl RewritePlan {
    pub fn is_empty(&self) -> bool {
        self.links == 0 && self.skipped.is_empty() && self.unfixable == 0
    }
}

fn link_pages(pages: &[PlanPage], new: bool) -> Vec<LinkPage> {
    pages
        .iter()
        .map(|p| {
            LinkPage::new(p.id.clone(), if new { &p.new } else { &p.old })
                .with_aliases(p.aliases.clone())
        })
        .collect()
}

/// A page name that can be written between `[[` and `]]` and read back the same.
fn writable(target: &str) -> bool {
    !target.is_empty()
        && target.len() <= notecraft_markdown::links::MAX_LINK_BYTES
        && !target.contains(['[', ']', '|', '\n', '\r', '\\'])
        && target.trim() == target
}

/// Work out the edits. `source` gives the text of a page (called once per page, in order).
pub fn plan(
    pages: &[PlanPage],
    source: &mut dyn FnMut(&PlanPage) -> Option<PageSource>,
) -> RewritePlan {
    let mut out = RewritePlan::default();
    // Nothing to do when no path changes.
    if pages.iter().all(|p| p.old == p.new) {
        return out;
    }
    let old_index = LinkIndex::new(link_pages(pages, false));
    let new_index = LinkIndex::new(link_pages(pages, true));
    for page in pages.iter().take(MAX_PLAN_PAGES) {
        let Some(found) = source(page) else {
            continue;
        };
        let text = found.text;
        let links = notecraft_markdown::page_links_of(&text);
        let mut edits = Vec::new();
        let mut unfixable = 0;
        for link in &links {
            match edit_for(&text, link, page, pages, &old_index, &new_index) {
                Ok(Some(edit)) => edits.push(edit),
                Ok(None) => {}
                Err(()) => unfixable += 1,
            }
        }
        if edits.is_empty() {
            out.unfixable += unfixable;
            continue;
        }
        if let Some(reason) = found.blocked {
            out.skipped.push((page.id.clone(), reason));
            continue;
        }
        out.unfixable += unfixable;
        out.links += edits.len();
        edits.sort_by_key(|e| std::cmp::Reverse(e.at));
        out.pages.push(PageEdits {
            id: page.id.clone(),
            rel: page.rel.clone(),
            edits,
        });
    }
    out
}

/// The edit for one link: `Ok(None)` when it needs none, `Err` when it needs one that cannot
/// be made.
fn edit_for(
    text: &str,
    link: &notecraft_markdown::PageLink,
    page: &PlanPage,
    pages: &[PlanPage],
    old_index: &LinkIndex,
    new_index: &LinkIndex,
) -> Result<Option<Edit>, ()> {
    let Some(was) = old_index.resolve(&link.target, &page.old).page() else {
        return Ok(None);
    };
    let Some(target_page) = pages.iter().find(|p| p.id == was.id) else {
        return Ok(None);
    };
    let now = new_index.resolve(&link.target, &page.new);
    if matches!(&now, Resolved::Page(p) | Resolved::Ambiguous { chosen: p, .. } if p.id == was.id) {
        return Ok(None);
    }
    let Some(new_page) = new_index.page(&target_page.id) else {
        return Err(());
    };
    let form = new_index.shortest_form(new_page, &page.new);
    let resolves = new_index
        .resolve(&form, &page.new)
        .page()
        .is_some_and(|p| p.id == new_page.id);
    if !writable(&form) || !resolves {
        return Err(());
    }
    // The raw target region (trimmed) inside the brackets.
    let inner_start = link.span.start + 2;
    let inner_end = link.sep.map_or(link.span.end - 2, |s| s.start);
    let raw = text.get(inner_start..inner_end).ok_or(())?;
    let lead = raw.len() - raw.trim_start().len();
    let trimmed = raw.trim();
    let at = inner_start + lead;
    let sep = if link.in_table { "\\|" } else { "|" };
    let inserted = if link.sep.is_some() {
        // Display text stays as written.
        form
    } else if is_plain_name(&link.target, was) {
        let base = new_page.base();
        if form == base {
            form
        } else {
            format!("{form}{sep}{base}")
        }
    } else {
        format!("{form}{sep}{trimmed}")
    };
    Ok(Some(Edit {
        at,
        removed: trimmed.to_owned(),
        inserted,
    }))
}

/// The link text was just the page's name (no folder, not an alias).
fn is_plain_name(target: &str, page: &LinkPage) -> bool {
    !target.contains('/') && fold_link_text(target) == fold_link_text(page.base())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(id: &str, old: &str, new: &str) -> PlanPage {
        PlanPage {
            id: id.to_owned(),
            rel: old.to_owned(),
            old: old.to_owned(),
            new: new.to_owned(),
            aliases: Vec::new(),
        }
    }

    fn run(pages: &[PlanPage], texts: &[(&str, &str)]) -> RewritePlan {
        plan(pages, &mut |p| {
            texts
                .iter()
                .find(|(id, _)| *id == p.id)
                .map(|(_, t)| PageSource {
                    text: (*t).to_owned(),
                    blocked: None,
                })
        })
    }

    fn apply(text: &str, edits: &PageEdits) -> String {
        let mut out = text.to_owned();
        for e in &edits.edits {
            out.replace_range(e.at..e.at + e.removed.len(), &e.inserted);
        }
        out
    }

    #[test]
    fn a_renamed_page_updates_plain_links_and_shown_text() {
        let pages = [
            page("a", "S/A.md", "S/A.md"),
            page("b", "S/Old.md", "S/New.md"),
        ];
        let src = "x [[Old]] y [[Old|the old one]] z [[S/Old]]\n";
        let result = run(&pages, &[("a", src)]);
        assert_eq!(result.links, 3);
        assert_eq!(
            apply(src, &result.pages[0]),
            "x [[New]] y [[New|the old one]] z [[New|S/Old]]\n"
        );
    }

    #[test]
    fn a_moved_page_gets_a_longer_form_when_the_name_is_taken() {
        let pages = [
            page("a", "S/A.md", "S/A.md"),
            page("b", "S/Dup.md", "V/Dup.md"),
            page("c", "U/Dup.md", "U/Dup.md"),
            page("d", "S/B.md", "S/B.md"),
        ];
        let src = "[[Dup]] [[B]]\n";
        let result = run(&pages, &[("a", src)]);
        // From S the Dup in S was the closest. Now both are one folder away; U/Dup wins the
        // tie by path order, so the link is rewritten to keep pointing at the moved page.
        assert_eq!(result.links, 1);
        assert_eq!(apply(src, &result.pages[0]), "[[V/Dup|Dup]] [[B]]\n");
    }

    #[test]
    fn a_form_gets_shorter_when_the_clash_is_gone() {
        let pages = [
            page("a", "S/A.md", "S/A.md"),
            page("b", "T/Dup.md", "T/Solo.md"),
            page("c", "U/Dup.md", "U/Dup.md"),
        ];
        let src = "[[T/Dup]] [[U/Dup]]\n";
        let result = run(&pages, &[("a", src)]);
        assert_eq!(apply(src, &result.pages[0]), "[[Solo|T/Dup]] [[U/Dup]]\n");
    }

    #[test]
    fn table_cells_keep_their_escaped_pipe() {
        let pages = [page("a", "S/A.md", "S/A.md"), page("b", "S/B.md", "S/C.md")];
        let src = "| x |\n|---|\n| [[B\\|b]] [[S/B]] |\n";
        let result = run(&pages, &[("a", src)]);
        assert_eq!(
            apply(src, &result.pages[0]),
            "| x |\n|---|\n| [[C\\|b]] [[C\\|S/B]] |\n"
        );
    }

    #[test]
    fn unresolved_and_unaffected_links_are_left_alone() {
        let pages = [page("a", "S/A.md", "S/A.md"), page("b", "S/B.md", "T/B.md")];
        let src = "[[Missing]] [[B]] `[[B]]`\n";
        assert!(run(&pages, &[("a", src)]).is_empty());
    }

    #[test]
    fn a_moved_page_keeps_its_own_links() {
        let pages = [
            page("a", "S/A.md", "T/A.md"),
            page("b", "S/B.md", "S/B.md"),
            page("c", "T/B.md", "T/B.md"),
        ];
        // From S, [[B]] means S/B; from T it would mean T/B.
        let src = "[[B]]\n";
        let result = run(&pages, &[("a", src)]);
        assert_eq!(apply(src, &result.pages[0]), "[[S/B|B]]\n");
    }

    #[test]
    fn blocked_pages_are_reported_not_edited() {
        let pages = [page("a", "S/A.md", "S/A.md"), page("b", "S/B.md", "S/C.md")];
        let result = plan(&pages, &mut |p| {
            (p.id == "a").then(|| PageSource {
                text: "[[B]]".to_owned(),
                blocked: Some(SkipReason::ReadOnly),
            })
        });
        assert!(result.pages.is_empty());
        assert_eq!(result.skipped, [("a".to_owned(), SkipReason::ReadOnly)]);
    }

    #[test]
    fn a_name_that_cannot_be_written_is_counted_not_forced() {
        let pages = [
            page("a", "S/A.md", "S/A.md"),
            page("b", "S/B.md", "S/C[1].md"),
        ];
        let result = run(&pages, &[("a", "[[B]]")]);
        assert_eq!(result.unfixable, 1);
        assert!(result.pages.is_empty());
    }
}
