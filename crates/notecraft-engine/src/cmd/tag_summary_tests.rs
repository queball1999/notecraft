//! Tags Summary commands: scopes, grouping, opening an item, and the summary page.

use serde_json::{json, Value};

use crate::cmd::Registry;
use crate::session::Session;

fn write(root: &std::path::Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
}

fn build_index(root: &std::path::Path) {
    let db = notecraft_storage::index_path(root);
    let (mut index, _) = notecraft_storage::index::refresh_or_rebuild(root, &db).expect("index");
    crate::tags_index::update_tag_index(root, &mut index).expect("tags");
}

/// Notebook "One" (Grp/Alpha, Grp/Beta, Loose) and "Two" (Camp), tags built.
fn session() -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut notebooks = Vec::new();
    let mut roots = Vec::new();
    for (name, pages) in [
        (
            "One",
            &[
                (
                    "Grp/Alpha/Tasks.md",
                    "- [ ] call Sam\n- [x] pay rent\n\n#idea new logo\n",
                ),
                (
                    "Grp/Beta/Notes.md",
                    "#idea second idea\n\n- [ ] water plants\n",
                ),
                ("Loose/Misc.md", "#question why\n"),
            ][..],
        ),
        ("Two", &[("Camp/Gear.md", "- [ ] buy stove\n")][..]),
    ] {
        let root = dir.path().join(name);
        for (rel, text) in pages {
            write(&root, rel, text);
        }
        let opened = crate::open::open_folder(&root).expect("open");
        build_index(&root);
        roots.push((opened.notebook.id.clone(), root));
        notebooks.push(opened.notebook);
    }
    let mut session = Session::new();
    session.set_notebooks(notebooks);
    for (id, root) in &roots {
        session.set_notebook_root(id, root);
    }
    (dir, session)
}

fn run(session: &mut Session, id: &str, args: Value) -> Value {
    Registry::builtin()
        .run(session, id, args)
        .expect("run")
        .value
}

fn select(session: &mut Session, page: &str) {
    let id = session
        .notebooks()
        .iter()
        .flat_map(|nb| nb.all_sections())
        .flat_map(|s| s.pages.iter())
        .find(|p| p.id.ends_with(page))
        .map(|p| p.id.clone())
        .expect("page");
    run(session, "nav.page.select", json!({ "id": id }));
}

fn labels(value: &Value) -> Vec<(String, usize)> {
    value["groups"]
        .as_array()
        .map(|groups| {
            groups
                .iter()
                .map(|g| {
                    (
                        g["label"].as_str().unwrap_or_default().to_owned(),
                        g["items"].as_array().map_or(0, Vec::len),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn named(list: &[(&str, usize)]) -> Vec<(String, usize)> {
    list.iter().map(|(n, c)| ((*n).to_owned(), *c)).collect()
}

#[test]
fn scopes_narrow_from_all_notebooks_to_one_page() {
    let (_dir, mut session) = session();
    select(&mut session, "Grp/Alpha/Tasks.md");
    let all = run(&mut session, "tag.find", json!({ "scope": "all" }));
    assert_eq!(all["count"], 7);
    assert_eq!(
        labels(&all),
        named(&[("Idea", 2), ("Question", 1), ("To Do", 4)])
    );
    assert!(session.tag_summary().open);
    let notebook = run(
        &mut session,
        "tag.summary.set_scope",
        json!({ "scope": "notebook" }),
    );
    assert_eq!(notebook, "notebook");
    let summary = session.tag_summary().summary.clone().expect("summary");
    assert_eq!(summary.count(), 6);
    run(
        &mut session,
        "tag.summary.set_scope",
        json!({ "scope": "group" }),
    );
    assert_eq!(
        session.tag_summary().summary.as_ref().expect("s").count(),
        5
    );
    run(
        &mut session,
        "tag.summary.set_scope",
        json!({ "scope": "section" }),
    );
    assert_eq!(
        session.tag_summary().summary.as_ref().expect("s").count(),
        3
    );
    run(
        &mut session,
        "tag.summary.set_scope",
        json!({ "scope": "page" }),
    );
    assert_eq!(
        session.tag_summary().summary.as_ref().expect("s").count(),
        3
    );
    select(&mut session, "Loose/Misc.md");
    run(&mut session, "tag.summary.refresh", json!({}));
    assert_eq!(
        session.tag_summary().summary.as_ref().expect("s").count(),
        1
    );
    // A section outside a group has the notebook as its group.
    run(
        &mut session,
        "tag.summary.set_scope",
        json!({ "scope": "group" }),
    );
    assert_eq!(
        session.tag_summary().summary.as_ref().expect("s").count(),
        6
    );
}

#[test]
fn grouping_and_unchecked_only() {
    let (_dir, mut session) = session();
    select(&mut session, "Grp/Alpha/Tasks.md");
    run(&mut session, "tag.find", json!({ "scope": "notebook" }));
    run(
        &mut session,
        "tag.summary.set_group",
        json!({ "group": "section" }),
    );
    let summary = session.tag_summary().summary.clone().expect("summary");
    let by_section: Vec<(&str, usize)> = summary
        .groups
        .iter()
        .map(|g| (g.label.as_str(), g.items.len()))
        .collect();
    assert_eq!(by_section, [("Alpha", 3), ("Beta", 2), ("Loose", 1)]);
    run(
        &mut session,
        "tag.summary.set_group",
        json!({ "group": "title" }),
    );
    let summary = session.tag_summary().summary.clone().expect("summary");
    let by_title: Vec<&str> = summary.groups.iter().map(|g| g.label.as_str()).collect();
    assert_eq!(by_title, ["Misc", "Notes", "Tasks"]);
    run(
        &mut session,
        "tag.summary.set_group",
        json!({ "group": "date" }),
    );
    let summary = session.tag_summary().summary.clone().expect("summary");
    assert_eq!(
        summary.groups.len(),
        1,
        "files written together share a day"
    );
    run(
        &mut session,
        "tag.summary.set_group",
        json!({ "group": "tag" }),
    );
    run(
        &mut session,
        "tag.summary.set_unchecked",
        json!({ "value": true }),
    );
    let summary = session.tag_summary().summary.clone().expect("summary");
    assert_eq!(summary.count(), 5);
    assert!(summary
        .groups
        .iter()
        .flat_map(|g| &g.items)
        .all(|i| i.checked != Some(true)));
    assert!(Registry::builtin()
        .run(
            &mut session,
            "tag.summary.set_group",
            json!({ "group": "x" })
        )
        .is_err());
    assert!(Registry::builtin()
        .run(
            &mut session,
            "tag.summary.set_scope",
            json!({ "scope": "x" })
        )
        .is_err());
}

#[test]
fn opening_an_item_shows_the_page_with_the_caret_at_the_paragraph() {
    let (_dir, mut session) = session();
    select(&mut session, "Loose/Misc.md");
    run(&mut session, "tag.find", json!({ "scope": "notebook" }));
    let item = session
        .tag_summary()
        .summary
        .as_ref()
        .expect("summary")
        .groups
        .iter()
        .flat_map(|g| &g.items)
        .find(|i| i.page_title == "Notes" && i.tag == "idea")
        .cloned()
        .expect("item");
    let source = "#idea second idea\n\n- [ ] water plants\n";
    session.set_page_source(&item.page_id, source);
    run(
        &mut session,
        "tag.summary.open_item",
        json!({ "page": item.page_id, "at": item.at, "tag": item.tag, "text": item.text }),
    );
    assert_eq!(
        session.selection().page.as_deref(),
        Some(item.page_id.as_str())
    );
    let cursor = session.text_cursor().expect("caret");
    assert_eq!(cursor.head, 0);
    // A page that changed since the index was read is looked up again.
    session.set_page_source(&item.page_id, format!("new first line\n\n{source}"));
    run(
        &mut session,
        "tag.summary.open_item",
        json!({ "page": item.page_id, "at": item.at, "tag": item.tag, "text": item.text }),
    );
    assert_eq!(session.text_cursor().expect("caret").head, 16);
    assert!(Registry::builtin()
        .run(
            &mut session,
            "tag.summary.open_item",
            json!({ "page": "nope", "at": 0 })
        )
        .is_err());
}

#[test]
fn the_summary_page_lists_the_items_and_is_one_undo_step() {
    let (dir, mut session) = session();
    select(&mut session, "Grp/Alpha/Tasks.md");
    run(&mut session, "tag.find", json!({ "scope": "notebook" }));
    let section = session.selection().section.clone().expect("section");
    let before = session
        .notebooks()
        .iter()
        .find_map(|nb| nb.section(&section))
        .map_or(0, |s| s.pages.len());
    let made = run(&mut session, "tag.summary.create_page", json!({}));
    assert_eq!(made["created"], true);
    let page = made["page"].as_str().expect("id").to_owned();
    assert!(page.ends_with("Grp/Alpha/Tags Summary.md"), "{page}");
    assert_eq!(session.selection().page.as_deref(), Some(page.as_str()));
    let text =
        std::fs::read_to_string(dir.path().join("One/Grp/Alpha/Tags Summary.md")).expect("file");
    assert!(
        text.contains("## Idea\n\n- #idea new logo [[Tasks]]\n"),
        "{text}"
    );
    assert!(text.contains("- #idea second idea [[Notes]]\n"), "{text}");
    assert!(text.contains("- [x] pay rent [[Tasks]]\n"), "{text}");
    assert!(text.contains("- [ ] water plants [[Notes]]\n"), "{text}");
    assert!(
        text.contains("## Question\n\n- #question why [[Misc]]\n"),
        "{text}"
    );
    // A second summary gets a number.
    let again = run(&mut session, "tag.summary.create_page", json!({}));
    assert!(again["page"]
        .as_str()
        .expect("id")
        .ends_with("Tags Summary 2.md"));
    // Undo removes the second page in one step.
    run(&mut session, "edit.undo", json!({}));
    let count = |s: &Session| {
        s.notebooks()
            .iter()
            .find_map(|nb| nb.section(&section))
            .map_or(0, |sec| sec.pages.len())
    };
    assert_eq!(count(&session), before + 1);
    run(&mut session, "edit.undo", json!({}));
    assert_eq!(count(&session), before);
}

#[test]
fn nothing_shown_makes_no_page_and_search_closes_the_pane() {
    let (_dir, mut session) = session();
    select(&mut session, "Grp/Alpha/Tasks.md");
    let none = run(&mut session, "tag.summary.create_page", json!({}));
    assert_eq!(none["created"], false);
    run(&mut session, "tag.find", json!({}));
    assert!(session.tag_summary().open);
    run(&mut session, "search.open", json!({}));
    assert!(!session.tag_summary().open);
    assert!(session.search().open);
    run(&mut session, "tag.find", json!({}));
    assert!(!session.search().open);
    run(&mut session, "tag.summary.close", json!({}));
    assert!(!session.tag_summary().open);
}
