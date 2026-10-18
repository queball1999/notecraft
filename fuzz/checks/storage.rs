//! Checks run by the storage fuzz targets and by their stable property-test twins in
//! `crates/notecraft-storage/tests/fuzz_props.rs`. Each takes arbitrary bytes, never panics
//! on any input, and asserts the limits in docs/markdown.md section 9.

#![allow(dead_code)]

use notecraft_storage::canvas_block::{MAX_COORD, MAX_OBJECTS, MAX_OBJECT_SRC, MAX_STROKES};
use notecraft_storage::notebook_toml::{MAX_ID_CHARS, MAX_LIST_ENTRIES, MAX_NAME_CHARS};
use notecraft_storage::recovery::MAX_ENTRIES;
use notecraft_storage::{
    find_canvas_block, read_canvas, strip_canvas_block, write_canvas_all, write_ink, Journal,
    NotebookConfig, SectionConfig,
};

fn text_of(data: &[u8]) -> String {
    String::from_utf8_lossy(data).into_owned()
}

/// The canvas block: ranges stay inside the page, at most 100,000 strokes come back, each
/// forced into range, and what is written reads back as clean.
pub fn check_canvas(data: &[u8]) {
    let text = text_of(data);
    if let Some(block) = find_canvas_block(&text) {
        assert!(block.block.start <= block.block.end && block.block.end <= text.len());
        assert!(block.json.start <= block.json.end && block.json.end <= text.len());
        assert!(text.get(block.block.clone()).is_some());
        assert!(text.get(block.json.clone()).is_some());
    }
    let read = read_canvas(&text);
    assert!(read.strokes.len() <= MAX_STROKES);
    let mut ids = std::collections::HashSet::new();
    for s in &read.strokes {
        assert!(s.points.len() <= notecraft_core::ink::MAX_STROKE_POINTS);
        assert!(s.width.is_finite() && s.width > 0.0 && s.width <= 200.0);
        assert!(s.id.chars().count() <= 64);
        assert!(ids.insert(s.id.clone()), "duplicate stroke id {:?}", s.id);
        for p in &s.points {
            assert!(p.x.is_finite() && p.y.is_finite());
            assert!(p.x.abs() <= MAX_COORD && p.y.abs() <= MAX_COORD);
        }
    }
    // Placed pictures and files: bounded, finite, and with unique ids.
    assert!(read.objects.len() <= 2 * MAX_OBJECTS);
    let mut object_ids = std::collections::HashSet::new();
    for o in &read.objects {
        assert!(!o.src.trim().is_empty() && o.src.chars().count() <= MAX_OBJECT_SRC);
        assert!(o.x.is_finite() && o.y.is_finite());
        assert!(o.x.abs() <= MAX_COORD && o.y.abs() <= MAX_COORD);
        assert!(
            object_ids.insert(o.id.clone()),
            "duplicate object id {:?}",
            o.id
        );
    }
    let _ = strip_canvas_block(&text);
    if let Ok(out) = write_canvas_all(&text, &read.strokes, &read.containers, &read.objects) {
        let again = read_canvas(&out);
        assert!(again.problem.is_none(), "written objects read back as a problem");
        assert_eq!(again.objects.len(), read.objects.len());
    }
    if let Ok(out) = write_ink(&text, &read.strokes) {
        let again = read_canvas(&out);
        assert!(
            again.problem.is_none(),
            "written ink reads back as a problem"
        );
        assert_eq!(again.strokes.len(), read.strokes.len());
    }
}

/// `notebook.toml`: any text parses or is refused, with ids, names, and lists bounded.
pub fn check_notebook_toml(data: &[u8]) {
    let text = text_of(data);
    let Some(config) = NotebookConfig::parse(&text) else {
        return;
    };
    if let Some(id) = &config.id {
        assert!(id.chars().count() <= MAX_ID_CHARS);
    }
    if let Some(name) = &config.name {
        assert!(name.chars().count() <= MAX_NAME_CHARS);
    }
    assert!(config.sections.len() <= MAX_LIST_ENTRIES);
}

/// `.section.toml`: same bounds, plus page levels from 0 to 2.
pub fn check_section_toml(data: &[u8]) {
    let text = text_of(data);
    let Some(config) = SectionConfig::parse(&text) else {
        return;
    };
    assert!(config.pages.len() <= MAX_LIST_ENTRIES);
    assert!(config.sections.len() <= MAX_LIST_ENTRIES);
    for page in &config.pages {
        assert!(page.level <= 2);
    }
}

/// The recovery journal reader: a journal file holding arbitrary bytes is skipped or read,
/// never a panic, and at most 2,000 entries come back. `dir` is a scratch folder.
pub fn check_journal(dir: &std::path::Path, data: &[u8]) {
    let journal_dir = dir.join("journal");
    if std::fs::create_dir_all(&journal_dir).is_err() {
        return;
    }
    let file = journal_dir.join("0000000000000001.json");
    if std::fs::write(&file, data).is_err() {
        return;
    }
    let journal = Journal::new(&journal_dir);
    let entries = journal.load_all();
    assert!(entries.len() <= MAX_ENTRIES);
    for entry in &entries {
        assert!(!entry.path.as_os_str().is_empty());
        journal.remove(&entry.path);
    }
    let _ = std::fs::remove_file(&file);
}
