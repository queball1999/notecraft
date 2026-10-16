//! Sidebar load for a 1,000-page notebook. The target is under 500 ms (SCRD section 11).
//! Run with `cargo bench -p notecraft-storage`.

use std::fs;
use std::path::Path;

use criterion::{criterion_group, criterion_main, Criterion};
use notecraft_storage::index::Index;

fn build_fixture(root: &Path) {
    for section in 0..20 {
        let dir = root.join(format!("Section {section:02}"));
        if fs::create_dir_all(&dir).is_err() {
            return;
        }
        for page in 0..50 {
            let text =
                format!("# Page {page}\n\nBody text for page {page} of section {section}.\n");
            let _ = fs::write(dir.join(format!("Page {page:02}.md")), text);
        }
    }
}

fn sidebar(c: &mut Criterion) {
    let Ok(dir) = tempfile::tempdir() else {
        return;
    };
    let root = dir.path().join("nb");
    build_fixture(&root);
    let db = root.join(".notecraft").join("index.sqlite");

    // Cold build of the whole index from the folder.
    c.bench_function("index_build_1000_pages", |b| {
        b.iter(|| {
            let _ = fs::remove_file(&db);
            if let Ok((mut index, _)) = Index::open_or_rebuild(&db) {
                let _ = index.refresh(&root);
            }
        })
    });

    // Warm refresh: nothing changed, so no page is read.
    if let Ok((mut index, _)) = Index::open_or_rebuild(&db) {
        let _ = index.refresh(&root);
        c.bench_function("index_refresh_unchanged_1000_pages", |b| {
            b.iter(|| {
                let _ = index.refresh(&root);
            })
        });
    }

    // What the sidebar needs at startup: open the database and list.
    c.bench_function("sidebar_open_and_list_1000_pages", |b| {
        b.iter(|| {
            if let Ok((index, _)) = Index::open_or_rebuild(&db) {
                let _ = index.sidebar();
            }
        })
    });
}

criterion_group!(benches, sidebar);
criterion_main!(benches);
