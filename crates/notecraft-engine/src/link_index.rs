//! Filling the `links` table of the notebook index (build_plan/04 PR 2.2).
//!
//! Storage cannot parse markdown, so this reads the pages the index says changed, finds their
//! page links and frontmatter aliases, and hands them to `notecraft_storage`. Resolving a link
//! needs every page of the notebook, so a second step resolves all stored links against the
//! current page list and stores the answer; it runs when pages were added, removed, renamed,
//! or edited. Both steps follow the search text update (`search::update_text_index`) and run
//! from the same places: the background refresh and the command line `index`.

use std::io::Read;
use std::path::Path;

use notecraft_markdown::{LinkIndex, LinkPage};
use notecraft_storage::index::{Index, LinkRow, PageLinks};

/// Most of a page file read to find its links.
const MAX_LINK_READ: u64 = 4 * 1024 * 1024;

/// The links and frontmatter aliases of a page file's text, with link spans as byte offsets
/// into that text.
pub fn page_link_facts(source: &str) -> (Vec<LinkRow>, Vec<String>) {
    let links = notecraft_markdown::page_links_of(source)
        .into_iter()
        .map(|link| LinkRow {
            target_text: link.target,
            start_byte: link.span.start,
            end_byte: link.span.end,
        })
        .collect();
    let aliases = notecraft_markdown::parse_frontmatter(source)
        .map(|front| {
            front
                .aliases()
                .into_iter()
                .map(|alias| alias.trim().to_owned())
                .filter(|alias| !alias.is_empty())
                .collect()
        })
        .unwrap_or_default();
    (links, aliases)
}

/// Read the links of every page whose links are missing or out of date, then resolve all
/// links if anything changed or `pages_changed` says pages came or went. Returns how many
/// pages were read. A page that cannot be read gets no links, so it is not tried again until
/// it changes.
pub fn update_link_index(
    root: &Path,
    index: &mut Index,
    pages_changed: bool,
) -> Result<usize, String> {
    let pending = index.pages_needing_links().map_err(|err| err.to_string())?;
    let mut rows = Vec::with_capacity(pending.len());
    for (id, path) in &pending {
        let mut bytes = Vec::new();
        let read = std::fs::File::open(root.join(path))
            .and_then(|file| file.take(MAX_LINK_READ).read_to_end(&mut bytes));
        let (links, aliases) = match read {
            Ok(_) => page_link_facts(&String::from_utf8_lossy(&bytes)),
            Err(_) => (Vec::new(), Vec::new()),
        };
        rows.push(PageLinks {
            id: id.clone(),
            links,
            aliases,
        });
    }
    index.set_page_links(&rows).map_err(|err| err.to_string())?;
    if !pending.is_empty() || pages_changed {
        resolve_stored_links(index)?;
    }
    Ok(pending.len())
}

/// The frontmatter aliases of every page in the index, by page id (pages without aliases are
/// left out). The navigation keeps them to resolve links on the canvas.
pub fn stored_aliases(
    index: &Index,
) -> Result<std::collections::BTreeMap<String, Vec<String>>, String> {
    Ok(index
        .link_pages()
        .map_err(|err| err.to_string())?
        .into_iter()
        .filter(|(_, _, aliases)| !aliases.is_empty())
        .map(|(id, _, aliases)| (id, aliases))
        .collect())
}

/// Resolve every stored link against the pages in the index and store the pages they point
/// to. Only rows whose answer changed are written.
fn resolve_stored_links(index: &mut Index) -> Result<(), String> {
    let pages: Vec<LinkPage> = index
        .link_pages()
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(|(id, path, aliases)| LinkPage::new(id, path).with_aliases(aliases))
        .collect();
    let links = index.all_links().map_err(|err| err.to_string())?;
    if links.is_empty() {
        return Ok(());
    }
    let resolver = LinkIndex::new(pages);
    let mut changes = Vec::new();
    for link in &links {
        let from = resolver
            .page(&link.source_page)
            .map_or("", |page| page.path.as_str());
        let now = resolver
            .resolve(&link.target_text, from)
            .page()
            .map(|page| page.id.clone());
        if now != link.resolved_page {
            changes.push((link.row, now));
        }
    }
    if changes.is_empty() {
        return Ok(());
    }
    index
        .set_resolved_links(&changes)
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use notecraft_storage::index::refresh_or_rebuild;
    use notecraft_storage::index_path;

    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, text).expect("write");
    }

    fn refresh(root: &Path) -> Index {
        let (mut index, report) = refresh_or_rebuild(root, &index_path(root)).expect("index");
        let changed = !(report.stats.added.is_empty()
            && report.stats.changed.is_empty()
            && report.stats.removed.is_empty());
        update_link_index(root, &mut index, changed).expect("links");
        index
    }

    fn id_of(index: &Index, path: &str) -> String {
        index
            .link_pages()
            .expect("pages")
            .into_iter()
            .find(|(_, p, _)| p == path)
            .map(|(id, ..)| id)
            .expect("page")
    }

    /// Each link of the page at `path` as its target text and the path of the page it
    /// resolved to.
    fn resolved(index: &Index, path: &str) -> Vec<(String, Option<String>)> {
        let id = id_of(index, path);
        let pages = index.link_pages().expect("pages");
        index
            .page_links(&id)
            .expect("links")
            .into_iter()
            .map(|l| {
                let target = l.resolved_page.and_then(|p| {
                    pages
                        .iter()
                        .find(|(pid, ..)| *pid == p)
                        .map(|(_, path, _)| path.clone())
                });
                (l.target_text, target)
            })
            .collect()
    }

    fn pair(target: &str, page: Option<&str>) -> (String, Option<String>) {
        (target.to_owned(), page.map(str::to_owned))
    }

    #[test]
    fn facts_hold_spans_into_the_whole_file_and_the_aliases() {
        let source = "---\ntitle: T\naliases: [One, Two]\n---\nSee [[Plan|the plan]] now.\n";
        let (links, aliases) = page_link_facts(source);
        assert_eq!(aliases, ["One", "Two"]);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target_text, "Plan");
        assert_eq!(
            &source[links[0].start_byte..links[0].end_byte],
            "[[Plan|the plan]]"
        );
        // Links in the frontmatter are not page links.
        let (links, _) = page_link_facts("---\nnote: \"[[x]]\"\n---\nbody\n");
        assert!(links.is_empty());
    }

    #[test]
    fn links_are_stored_and_resolved_by_the_section_5_3_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write(root, "S/A.md", "[[B]] [[Missing]] [[Q3 sync]] [[Dup]]\n");
        write(root, "S/B.md", "b");
        write(root, "T/Review.md", "---\naliases: [Q3 sync]\n---\nr");
        write(root, "S/Dup.md", "d");
        write(root, "T/Dup.md", "d2");
        let index = refresh(root);
        assert_eq!(
            resolved(&index, "S/A.md"),
            [
                pair("B", Some("S/B.md")),
                pair("Missing", None),
                pair("Q3 sync", Some("T/Review.md")),
                // Ambiguous: the closest one (same section) is stored.
                pair("Dup", Some("S/Dup.md")),
            ]
        );
    }

    #[test]
    fn links_follow_edits_creations_removals_and_renames() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write(root, "S/A.md", "[[B]]");
        let index = refresh(root);
        assert_eq!(resolved(&index, "S/A.md"), [pair("B", None)]);
        drop(index);
        // A page created later resolves the link even though A did not change.
        write(root, "S/B.md", "b");
        let index = refresh(root);
        assert_eq!(resolved(&index, "S/A.md"), [pair("B", Some("S/B.md"))]);
        drop(index);
        // A rename outside the app: the link no longer resolves.
        std::fs::rename(root.join("S/B.md"), root.join("S/C.md")).expect("rename");
        let index = refresh(root);
        assert_eq!(resolved(&index, "S/A.md"), [pair("B", None)]);
        drop(index);
        // Editing the page changes its rows, with new spans.
        write(root, "S/A.md", "intro [[C]] and [[C|again]]");
        let index = refresh(root);
        let id = id_of(&index, "S/A.md");
        let stored = index.page_links(&id).expect("links");
        assert_eq!(stored.len(), 2);
        assert_eq!((stored[0].start_byte, stored[0].end_byte), (6, 11));
        assert_eq!(stored[1].resolved_page, stored[0].resolved_page);
        assert!(stored[0].resolved_page.is_some());
        drop(index);
        // Removing the source page removes its rows.
        std::fs::remove_file(root.join("S/A.md")).expect("rm");
        let index = refresh(root);
        assert!(index.all_links().expect("all").is_empty());
    }

    #[test]
    fn an_unchanged_notebook_reads_no_pages() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "S/A.md", "[[B]]");
        write(dir.path(), "S/B.md", "b");
        let db = index_path(dir.path());
        let (mut index, _) = refresh_or_rebuild(dir.path(), &db).expect("index");
        assert_eq!(update_link_index(dir.path(), &mut index, true), Ok(2));
        let (mut index, _) = refresh_or_rebuild(dir.path(), &db).expect("index");
        assert_eq!(update_link_index(dir.path(), &mut index, false), Ok(0));
    }

    #[test]
    fn aliases_are_listed_by_page() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "S/A.md", "---\naliases: [X, Y]\n---\n");
        write(dir.path(), "S/B.md", "b");
        let index = refresh(dir.path());
        let found = stored_aliases(&index).expect("aliases");
        assert_eq!(found.len(), 1);
        assert_eq!(
            found.values().next().map(Vec::as_slice),
            Some(&["X".to_owned(), "Y".to_owned()][..])
        );
    }

    #[test]
    fn odd_page_text_is_read_leniently() {
        let (links, aliases) = page_link_facts("\u{0}\u{0}[[ok]]\u{fffd}");
        assert_eq!(links.len(), 1);
        assert!(aliases.is_empty());
    }
}
