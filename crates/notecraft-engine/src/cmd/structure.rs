//! Structure commands: create, rename, move, color, and delete notebooks, section groups,
//! sections, and pages (`page.*`, `section.*`, `notebook.*`, `file.close_notebook`). Each one is
//! a single undo step that includes its effect on the files; see `Session::create_page` and the
//! rest of `session/structure.rs` for what happens on disk.
//!
//! Where an id is optional, the thing shown now is used. A name that is optional in the schema
//! lets a menu send the command without one; the front end then asks for it and sends the
//! command again. Run without a name (from the control channel), it fails with `invalid_args`.

use notecraft_storage::notebook_toml::parse_color;
use serde_json::json;

use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::session::{EditError, Session};

fn invalid(command: CommandId, reason: impl Into<String>) -> CommandError {
    CommandError::InvalidArgs {
        id: command,
        reason: reason.into(),
    }
}

fn edit(command: CommandId, err: EditError) -> CommandError {
    invalid(command, err.0)
}

fn text<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

fn index(args: &Map<String, Value>) -> usize {
    args.get("index")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(usize::MAX)
}

fn name_arg(command: CommandId, args: &Map<String, Value>) -> Result<&str, CommandError> {
    text(args, "name").ok_or_else(|| invalid(command, "name is required; the app asks for it"))
}

fn color_arg(command: CommandId, args: &Map<String, Value>) -> Result<u32, CommandError> {
    text(args, "color")
        .and_then(parse_color)
        .ok_or_else(|| invalid(command, "color must look like #RRGGBB"))
}

fn page_id(session: &Session, args: &Map<String, Value>) -> Option<String> {
    text(args, "id")
        .map(str::to_owned)
        .or_else(|| session.selection().page.clone())
}

fn section_id(session: &Session, args: &Map<String, Value>) -> Option<String> {
    text(args, "id")
        .map(str::to_owned)
        .or_else(|| session.selection().section.clone())
}

fn notebook_id(session: &Session, args: &Map<String, Value>, key: &str) -> Option<String> {
    text(args, key)
        .map(str::to_owned)
        .or_else(|| session.selection().notebook.clone())
}

fn missing(command: CommandId, what: &str) -> CommandError {
    invalid(command, format!("no {what} is shown; pass its id"))
}

fn done(id: Option<String>) -> Result<CommandOutput, CommandError> {
    Ok(CommandOutput {
        value: json!({ "id": id }),
    })
}

/// `file.close_notebook { id? }`: close a notebook; its folder stays as it is.
pub(super) fn close_notebook(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let command = ids::FILE_CLOSE_NOTEBOOK;
    let id = notebook_id(session, args, "id").ok_or_else(|| missing(command, "notebook"))?;
    session.close_notebook(&id).map_err(|e| edit(command, e))?;
    done(Some(id))
}

/// `page.new { title?, section? }`: add a page at the end of a section and show it.
pub(super) fn page_new(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let command = ids::PAGE_NEW;
    let section = text(args, "section")
        .map(str::to_owned)
        .or_else(|| session.selection().section.clone())
        .ok_or_else(|| missing(command, "section"))?;
    let id = session
        .create_page(&section, text(args, "title"))
        .map_err(|e| edit(command, e))?;
    done(Some(id))
}

/// `page.rename { id?, name }`: rename a page (its file is renamed).
pub(super) fn page_rename(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let command = ids::PAGE_RENAME;
    let name = name_arg(command, args)?;
    let id = page_id(session, args).ok_or_else(|| missing(command, "page"))?;
    let new = session
        .rename_page(&id, name)
        .map_err(|e| edit(command, e))?;
    done(Some(new))
}

/// `page.delete { id? }`: move a page to the notebook's trash folder.
pub(super) fn page_delete(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let command = ids::PAGE_DELETE;
    let id = page_id(session, args).ok_or_else(|| missing(command, "page"))?;
    session.delete_page(&id).map_err(|e| edit(command, e))?;
    done(Some(id))
}

/// `page.move { id?, section?, index? }`: move a page within its section or to another.
pub(super) fn page_move(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let command = ids::PAGE_MOVE;
    let id = page_id(session, args).ok_or_else(|| missing(command, "page"))?;
    let section = match text(args, "section") {
        Some(section) => section.to_owned(),
        None => session
            .notebooks()
            .iter()
            .find_map(|nb| nb.section_of_page(&id))
            .map(|s| s.id.clone())
            .ok_or_else(|| invalid(command, "no page with that id"))?,
    };
    let new = session
        .move_page_to(&id, &section, index(args))
        .map_err(|e| edit(command, e))?;
    done(Some(new))
}

/// `page.set_level { id?, level }`: indent a page as a subpage (0 to 2).
pub(super) fn page_set_level(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let command = ids::PAGE_SET_LEVEL;
    let id = page_id(session, args).ok_or_else(|| missing(command, "page"))?;
    let level = args
        .get("level")
        .and_then(Value::as_u64)
        .and_then(|n| u8::try_from(n).ok())
        .ok_or_else(|| invalid(command, "level must be 0, 1, or 2"))?;
    let set = session
        .set_page_level(&id, level)
        .map_err(|e| edit(command, e))?;
    Ok(CommandOutput {
        value: json!({ "id": id, "level": set }),
    })
}

fn shift_level(
    session: &mut Session,
    args: &Map<String, Value>,
    command: CommandId,
    up: bool,
) -> Result<CommandOutput, CommandError> {
    let id = page_id(session, args).ok_or_else(|| missing(command, "page"))?;
    let current = session
        .notebooks()
        .iter()
        .find_map(|nb| nb.section_of_page(&id))
        .and_then(|s| s.page(&id))
        .map(|p| p.level)
        .ok_or_else(|| invalid(command, "no page with that id"))?;
    let level = if up {
        current.saturating_add(1)
    } else {
        current.saturating_sub(1)
    };
    let set = session
        .set_page_level(&id, level)
        .map_err(|e| edit(command, e))?;
    Ok(CommandOutput {
        value: json!({ "id": id, "level": set }),
    })
}

/// `page.make_subpage { id? }`: indent a page one level.
pub(super) fn page_make_subpage(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    shift_level(session, args, ids::PAGE_MAKE_SUBPAGE, true)
}

/// `page.promote_subpage { id? }`: outdent a page one level.
pub(super) fn page_promote_subpage(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    shift_level(session, args, ids::PAGE_PROMOTE_SUBPAGE, false)
}

fn new_node(
    session: &mut Session,
    args: &Map<String, Value>,
    command: CommandId,
    group: bool,
) -> Result<CommandOutput, CommandError> {
    let notebook =
        notebook_id(session, args, "notebook").ok_or_else(|| missing(command, "notebook"))?;
    let parent = text(args, "parent");
    let name = text(args, "name");
    let made = if group {
        session.create_section_group(&notebook, parent, name)
    } else {
        session.create_section(&notebook, parent, name)
    };
    done(Some(made.map_err(|e| edit(command, e))?))
}

/// `section.new { name?, parent?, notebook? }`: add a section and show it.
pub(super) fn section_new(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    new_node(session, args, ids::SECTION_NEW, false)
}

/// `section.group_new { name?, parent?, notebook? }`: add a section group.
pub(super) fn section_group_new(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    new_node(session, args, ids::SECTION_GROUP_NEW, true)
}

/// `section.rename { id?, name }`: rename a section or group (its folder is renamed).
pub(super) fn section_rename(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let command = ids::SECTION_RENAME;
    let name = name_arg(command, args)?;
    let id = section_id(session, args).ok_or_else(|| missing(command, "section"))?;
    let new = session
        .rename_section(&id, name)
        .map_err(|e| edit(command, e))?;
    done(Some(new))
}

/// `section.delete { id? }`: move a section or group to the notebook's trash folder.
pub(super) fn section_delete(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let command = ids::SECTION_DELETE;
    let id = section_id(session, args).ok_or_else(|| missing(command, "section"))?;
    session.delete_section(&id).map_err(|e| edit(command, e))?;
    done(Some(id))
}

/// `section.move { id?, parent?, index? }`: move a section or group into a group (or to the
/// top level when `parent` is left out).
pub(super) fn section_move(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let command = ids::SECTION_MOVE;
    let id = section_id(session, args).ok_or_else(|| missing(command, "section"))?;
    let new = session
        .move_section_to(&id, text(args, "parent"), index(args))
        .map_err(|e| edit(command, e))?;
    done(Some(new))
}

/// `section.set_color { id?, color }`: set a section's tab color.
pub(super) fn section_set_color(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let command = ids::SECTION_SET_COLOR;
    let color = color_arg(command, args)?;
    let id = section_id(session, args).ok_or_else(|| missing(command, "section"))?;
    session
        .set_section_color(&id, color)
        .map_err(|e| edit(command, e))?;
    done(Some(id))
}

/// `notebook.rename { id?, name }`: change a notebook's display name.
pub(super) fn notebook_rename(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let command = ids::NOTEBOOK_RENAME;
    let name = name_arg(command, args)?;
    let id = notebook_id(session, args, "id").ok_or_else(|| missing(command, "notebook"))?;
    session
        .rename_notebook(&id, name)
        .map_err(|e| edit(command, e))?;
    done(Some(id))
}

/// `notebook.set_color { id?, color }`: set a notebook's color.
pub(super) fn notebook_set_color(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let command = ids::NOTEBOOK_SET_COLOR;
    let color = color_arg(command, args)?;
    let id = notebook_id(session, args, "id").ok_or_else(|| missing(command, "notebook"))?;
    session
        .set_notebook_color(&id, color)
        .map_err(|e| edit(command, e))?;
    done(Some(id))
}

/// How to run each Ready undoable structure command in a test, with `{nb}` standing for the
/// notebook id. These commands need a notebook folder, so `undo/tests.rs` (which uses the mock
/// notebooks) leaves them to `every_structure_command_undoes_and_redoes_exactly` below.
#[cfg(test)]
pub(crate) const UNDO_CASES: &[(&str, &str)] = &[
    ("file.close_notebook", "{}"),
    ("page.new", r#"{ "title": "Fresh" }"#),
    ("page.rename", r#"{ "name": "Renamed" }"#),
    ("page.delete", "{}"),
    ("page.move", r#"{ "section": "{nb}/Home", "index": 0 }"#),
    (
        "page.set_level",
        r#"{ "id": "{nb}/Work/C.md", "level": 2 }"#,
    ),
    ("page.make_subpage", r#"{ "id": "{nb}/Work/C.md" }"#),
    ("page.promote_subpage", r#"{ "id": "{nb}/Work/B.md" }"#),
    ("section.new", r#"{ "name": "Extra" }"#),
    ("section.group_new", r#"{ "name": "Folder" }"#),
    ("section.rename", r#"{ "name": "Zed" }"#),
    ("section.delete", "{}"),
    (
        "section.move",
        r#"{ "id": "{nb}/Home", "parent": "{nb}/Projects", "index": 0 }"#,
    ),
    ("section.set_color", r##"{ "color": "#102030" }"##),
    ("notebook.rename", r#"{ "name": "Book" }"#),
    ("notebook.set_color", r##"{ "color": "#A0B0C0" }"##),
];

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::UNDO_CASES;
    use crate::cmd::Registry;
    use crate::open::open_folder;
    use crate::session::Session;

    fn session(files: &[(&str, &str)]) -> (tempfile::TempDir, Session, String) {
        let dir = tempfile::TempDir::new().unwrap();
        for (rel, text) in files {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let mut session = Session::new();
        let opened = open_folder(dir.path()).unwrap();
        let id = opened.notebook.id.clone();
        session.open_notebook(opened.notebook);
        session.set_notebook_root(&id, dir.path());
        (dir, session, id)
    }

    fn run(
        s: &mut Session,
        id: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, &'static str> {
        Registry::builtin()
            .run(s, id, args)
            .map(|o| o.value)
            .map_err(|e| e.code())
    }

    #[test]
    fn structure_commands_edit_the_notebook_and_edit_undo_reverses_them() {
        let (dir, mut s, nb) = session(&[("Work/A.md", "a"), ("Home/B.md", "b")]);
        let out = run(&mut s, "page.new", json!({ "title": "Fresh" })).unwrap();
        let page = format!("{nb}/Home/Fresh.md");
        assert_eq!(out["id"], page.as_str(), "the first section is shown");
        run(&mut s, "page.rename", json!({ "name": "Renamed" })).unwrap();
        assert!(dir.path().join("Home/Renamed.md").is_file());
        run(
            &mut s,
            "page.move",
            json!({ "section": format!("{nb}/Work"), "index": 0 }),
        )
        .unwrap();
        assert!(dir.path().join("Work/Renamed.md").is_file());
        run(&mut s, "page.make_subpage", json!({})).unwrap();
        run(&mut s, "section.new", json!({ "name": "Extra" })).unwrap();
        run(&mut s, "section.set_color", json!({ "color": "#102030" })).unwrap();
        run(&mut s, "notebook.rename", json!({ "name": "Book" })).unwrap();
        run(&mut s, "notebook.set_color", json!({ "color": "#A0B0C0" })).unwrap();
        run(&mut s, "section.delete", json!({})).unwrap();
        assert!(!dir.path().join("Extra").exists());
        while s.can_undo() {
            run(&mut s, "edit.undo", json!(null)).unwrap();
        }
        assert!(dir.path().join("Home/B.md").is_file());
        assert!(!dir.path().join("Work/Renamed.md").exists());
        assert!(!dir.path().join("Home/Fresh.md").exists());
        assert!(!dir.path().join("Home/Renamed.md").exists());
        let fresh = open_folder(dir.path()).unwrap().notebook;
        assert_eq!(s.current_notebook().unwrap(), &fresh);
    }

    /// Every file under `dir` with its bytes, for comparing the disk before and after.
    fn files(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let mut todo = vec![dir.to_path_buf()];
        while let Some(path) = todo.pop() {
            for entry in std::fs::read_dir(&path).unwrap() {
                let entry = entry.unwrap().path();
                if entry.is_dir() {
                    todo.push(entry);
                } else {
                    let rel = entry
                        .strip_prefix(dir)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    // Undoing a create keeps the made entry in the trash for a redo.
                    if !rel.starts_with(".notecraft/trash/") {
                        out.push((rel, std::fs::read(&entry).unwrap()));
                    }
                }
            }
        }
        out.sort();
        out
    }

    #[test]
    fn every_structure_command_undoes_and_redoes_exactly() {
        let fixture = [
            ("Work/A.md", "a"),
            ("Work/B.md", "b"),
            ("Work/C.md", "c"),
            (
                "Work/.section.toml",
                "kind = \"section\"
pages = [\"A.md\", { file = \"B.md\", level = 1 }, \"C.md\"]
",
            ),
            ("Home/D.md", "d"),
            (
                "Projects/.section.toml",
                "kind = \"group\"
",
            ),
            ("Projects/Alpha/E.md", "e"),
        ];
        for (id, args) in UNDO_CASES {
            let (dir, mut s, nb) = session(&fixture);
            let args: serde_json::Value = serde_json::from_str(&args.replace("{nb}", &nb)).unwrap();
            let (tree, disk) = (s.notebooks().to_vec(), files(dir.path()));
            run(&mut s, id, args.clone()).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(
                s.can_undo() || s.notebooks().is_empty(),
                "{id} left no undo step"
            );
            let (tree_after, disk_after) = (s.notebooks().to_vec(), files(dir.path()));
            assert!(
                tree_after != tree || disk_after != disk,
                "{id} changed nothing"
            );
            run(&mut s, "edit.undo", json!(null)).unwrap();
            assert_eq!(s.notebooks(), tree, "{id}: undo restores the tree");
            assert_eq!(files(dir.path()), disk, "{id}: undo restores the files");
            run(&mut s, "edit.redo", json!(null)).unwrap();
            assert_eq!(s.notebooks(), tree_after, "{id}: redo restores the tree");
            assert_eq!(
                files(dir.path()),
                disk_after,
                "{id}: redo restores the files"
            );
            // The tree in memory is what the folder holds.
            if !s.notebooks().is_empty() {
                let fresh = open_folder(dir.path()).unwrap().notebook;
                assert_eq!(s.current_notebook().unwrap(), &fresh, "{id}");
            }
        }
    }

    #[test]
    fn bad_arguments_are_refused_and_change_nothing() {
        let (dir, mut s, nb) = session(&[("Work/A.md", "a")]);
        let before = s.clone();
        for (id, args) in [
            ("page.rename", json!({})),
            ("page.rename", json!({ "id": "nope", "name": "x" })),
            ("page.delete", json!({ "id": "nope" })),
            ("page.move", json!({ "section": "nope" })),
            ("page.set_level", json!({ "level": "deep" })),
            ("section.set_color", json!({ "color": "red" })),
            ("section.move", json!({ "parent": format!("{nb}/Work") })),
            ("notebook.rename", json!({ "name": "" })),
            ("section.new", json!({ "parent": "nope" })),
        ] {
            assert_eq!(
                run(&mut s, id, args.clone()),
                Err("invalid_args"),
                "{id} {args}"
            );
        }
        assert_eq!(s, before);
        assert!(dir.path().join("Work/A.md").is_file());
    }

    #[test]
    fn closing_a_notebook_is_undoable() {
        let (_dir, mut s, nb) = session(&[("Work/A.md", "a")]);
        run(&mut s, "file.close_notebook", json!({})).unwrap();
        assert!(s.notebooks().is_empty());
        run(&mut s, "edit.undo", json!(null)).unwrap();
        assert_eq!(s.current_notebook().unwrap().id, nb);
    }

    #[test]
    fn structure_commands_are_flagged_undoable_and_deletes_destructive() {
        for id in ["page.delete", "section.delete"] {
            let registry = Registry::builtin();
            let spec = registry.get(id).unwrap();
            assert!(spec.undoable && spec.destructive, "{id}");
        }
        for id in [
            "page.new",
            "section.new",
            "page.move",
            "section.move",
            "notebook.rename",
        ] {
            assert!(Registry::builtin().get(id).unwrap().undoable, "{id}");
        }
    }
}
