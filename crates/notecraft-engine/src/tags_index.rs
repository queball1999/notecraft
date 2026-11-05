//! Filling the index `tags` table (build_plan/04 PR 2.5). Storage cannot parse markdown, so the
//! engine reads the pages the index says are out of date, finds their tags with
//! `notecraft_markdown::tags`, and hands the rows back. This runs on the refresh thread right
//! after the index refresh, and the command line calls [`update_tag_index`] the same way.

use std::io::Read;
use std::path::Path;

use notecraft_markdown::tags::{tagged_items, TaggedItem};
use notecraft_storage::find_canvas_block;
use notecraft_storage::index::{Index, TagRow};

/// Most bytes of a page read for its tags; the rest is not scanned.
const MAX_PAGE_BYTES: u64 = 16 * 1024 * 1024;

/// The index rows for the tagged items of a page, in source order. A task list item is a
/// `todo` row at its check box; an inline `#todo` in the same paragraph adds nothing; a tag
/// repeated in one paragraph counts once.
pub fn rows_for(items: &[TaggedItem]) -> Vec<TagRow> {
    let mut rows = Vec::new();
    for item in items {
        let mut seen: Vec<&str> = Vec::new();
        let row = |tag: &str, start: usize, end: usize, checked: Option<bool>| TagRow {
            tag: tag.to_owned(),
            start_byte: start,
            end_byte: end,
            holder_start: item.holder.start,
            holder_end: item.holder.end,
            text: item.text.clone(),
            checked,
        };
        if let Some(task) = &item.task {
            rows.push(row(
                "todo",
                task.span.start,
                task.span.end,
                Some(task.checked),
            ));
            seen.push("todo");
        }
        for tag in &item.tags {
            if seen.contains(&tag.name.as_str()) {
                continue;
            }
            seen.push(&tag.name);
            let checked = (tag.name == "todo").then_some(false);
            rows.push(row(&tag.name, tag.span.start, tag.span.end, checked));
        }
    }
    rows
}

/// The tag rows of page text (the part before a canvas block).
pub fn rows_for_text(text: &str) -> Vec<TagRow> {
    let end = find_canvas_block(text).map_or(text.len(), |b| b.block.start);
    let shown = text.get(..end.min(text.len())).unwrap_or(text);
    let doc = notecraft_markdown::parse(shown);
    rows_for(&tagged_items(shown, &doc))
}

/// Read the pages whose tags are out of date and record their tags. Returns how many pages
/// were read. A page that cannot be read is skipped and tried again at the next refresh.
pub fn update_tag_index(root: &Path, index: &mut Index) -> Result<usize, String> {
    let pending = index.pages_needing_tags().map_err(|err| err.to_string())?;
    let mut done = 0;
    for (id, rel) in pending {
        let Ok(file) = std::fs::File::open(root.join(&rel)) else {
            continue;
        };
        let mut bytes = Vec::new();
        if file.take(MAX_PAGE_BYTES).read_to_end(&mut bytes).is_err() {
            continue;
        }
        let rows = rows_for_text(&String::from_utf8_lossy(&bytes));
        index
            .set_page_tags(&id, &rows)
            .map_err(|err| err.to_string())?;
        done += 1;
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use notecraft_storage::index::{refresh_or_rebuild, TagScope};

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, text).expect("write");
    }

    #[test]
    fn rows_cover_tasks_inline_tags_and_dedupe() {
        let text = "#idea one #idea\n\n- [x] done #todo\n- [ ] open\n\n#todo alone\n";
        let rows = rows_for_text(text);
        let found: Vec<(&str, Option<bool>)> =
            rows.iter().map(|r| (r.tag.as_str(), r.checked)).collect();
        assert_eq!(
            found,
            [
                ("idea", None),
                ("todo", Some(true)),
                ("todo", Some(false)),
                ("todo", Some(false))
            ]
        );
        assert_eq!(rows[0].text, "one");
        assert_eq!(&text[rows[0].start_byte..rows[0].end_byte], "#idea");
        assert_eq!(&text[rows[1].start_byte..rows[1].end_byte], "[x]");
        assert!(rows_for_text("nothing here\n").is_empty());
    }

    #[test]
    fn the_refresh_fills_changed_pages_and_drops_removed_ones() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let root = dir.path();
        write(root, "Sec/A.md", "#important call\n");
        write(root, "Sec/B.md", "- [ ] buy milk\n");
        write(root, "Sec/C.md", "no tags\n");
        let db = root.join(".notecraft/index.sqlite");
        let (mut index, _) = refresh_or_rebuild(root, &db).expect("refresh");
        assert_eq!(update_tag_index(root, &mut index).expect("tags"), 3);
        assert_eq!(update_tag_index(root, &mut index).expect("again"), 0);
        let all = index.tagged_items(&TagScope::Notebook, &[]).expect("q");
        let tags: Vec<&str> = all.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(tags, ["important", "todo"]);
        assert_eq!(all[1].text, "buy milk");

        write(root, "Sec/A.md", "#question call again, longer\n");
        std::fs::remove_file(root.join("Sec/B.md")).expect("rm");
        drop(index);
        let (mut index, _) = refresh_or_rebuild(root, &db).expect("refresh");
        assert_eq!(update_tag_index(root, &mut index).expect("tags"), 1);
        let all = index.tagged_items(&TagScope::Notebook, &[]).expect("q");
        let tags: Vec<&str> = all.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(tags, ["question"]);
    }

    #[test]
    fn a_canvas_block_and_bad_utf8_are_not_a_problem() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let root = dir.path();
        let bytes = [b"#idea ok \xff\xfe\n".as_slice(), b"\n"].concat();
        std::fs::create_dir_all(root.join("Sec")).expect("mkdir");
        std::fs::write(root.join("Sec/A.md"), bytes).expect("write");
        let db = root.join(".notecraft/index.sqlite");
        let (mut index, _) = refresh_or_rebuild(root, &db).expect("refresh");
        assert_eq!(update_tag_index(root, &mut index).expect("tags"), 1);
        assert_eq!(
            index
                .tagged_items(&TagScope::Notebook, &[])
                .expect("q")
                .len(),
            1
        );
    }
}
