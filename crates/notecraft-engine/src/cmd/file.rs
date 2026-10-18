//! File commands: `file.new_notebook`, `file.open_notebook`, `file.open`, and
//! `file.recent.clear`. Each open records the path in the recent list. Opening a notebook
//! folder reads it from disk; the other commands, and a notebook path that does not exist,
//! show the mock notebooks until the rest of Phase 1 lands.
//!
//! The `path` argument is optional in the schema so a ribbon button can send the command
//! without one; the front end then asks with a file picker and sends it again with the path.
//! Run without a path (from the control channel), the command fails with `invalid_args`.

use std::path::Path;

use serde_json::json;

use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::mock;
use crate::open::{open_folder_for_navigation, NavOpened};
use crate::recent::{RecentItem, RecentKind, MAX_PATH_CHARS};
use crate::session::Session;

/// File extensions `file.open` accepts, lowercase.
pub const PAGE_EXTENSIONS: [&str; 2] = ["md", "notecraft"];

fn path_arg(id: CommandId, args: &Map<String, Value>) -> Result<&str, CommandError> {
    args.get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| CommandError::InvalidArgs {
            id,
            reason: "path is required; the app asks with a picker when it is omitted".to_owned(),
        })
}

fn bad_path(id: CommandId, reason: &str) -> CommandError {
    CommandError::InvalidArgs {
        id,
        reason: reason.to_owned(),
    }
}

/// Whether `path` names a page file NoteCraft opens (`.md` or `.notecraft`, any case).
pub fn is_page_file(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty()
            && !stem.ends_with(['/', '\\'])
            && PAGE_EXTENSIONS
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
    })
}

/// Record `item` as most recent and show the mock notebooks if nothing is open yet.
fn open(session: &mut Session, item: RecentItem) -> CommandOutput {
    let value = json!({ "kind": item.kind.id(), "path": item.path });
    session.recents_mut().push(item);
    if session.notebooks().is_empty() {
        session.set_notebooks(mock::notebooks());
    }
    session.mark_changed();
    CommandOutput { value }
}

fn open_kind(
    id: CommandId,
    kind: RecentKind,
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let path = path_arg(id, args)?;
    let Some(item) = RecentItem::new(kind, path) else {
        return Err(bad_path(
            id,
            &format!("path must be 1 to {MAX_PATH_CHARS} characters with no control characters"),
        ));
    };
    if kind == RecentKind::Page && !is_page_file(&item.path) {
        return Err(bad_path(id, "only .md and .notecraft files can be opened"));
    }
    Ok(open(session, item))
}

/// `file.new_notebook { path }`: create a notebook in a folder and open it. The folder is made
/// when it does not exist (its parent must), and gets `.notecraft/notebook.toml`; files already
/// in it stay as they are. A path whose parent does not exist keeps the Phase 0 behavior (record
/// it, show the mock notebooks if nothing is open).
pub(super) fn new_notebook(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::FILE_NEW_NOTEBOOK;
    let path = path_arg(id, args)?;
    let Some(item) = RecentItem::new(RecentKind::Notebook, path) else {
        return Err(bad_path(
            id,
            &format!("path must be 1 to {MAX_PATH_CHARS} characters with no control characters"),
        ));
    };
    let folder = Path::new(&item.path);
    let can_create = folder.is_dir() || folder.parent().is_some_and(Path::is_dir);
    if !can_create {
        tracing::warn!(path = %item.path, "parent folder not found; showing the sample notebooks");
        return Ok(open(session, item));
    }
    let notebook_id = session
        .create_notebook(folder)
        .map_err(|err| bad_path(id, &err.to_string()))?;
    let value = json!({ "kind": item.kind.id(), "path": item.path, "id": notebook_id });
    session.recents_mut().push(item);
    session.mark_changed();
    Ok(CommandOutput { value })
}

/// `file.open_notebook { path }`: open a notebook folder. A folder that exists is scanned
/// and its sections and pages replace the mock notebooks; the output then also has the
/// notebook `id` and, when the scan skipped something, a `warnings` list of plain text. A
/// path that does not exist keeps the Phase 0 behavior (record it, show the mock notebooks if
/// nothing is open) so a stale recent entry still opens something; a path that exists but is
/// not a readable folder is `invalid_args`.
pub(super) fn open_notebook(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::FILE_OPEN_NOTEBOOK;
    let path = path_arg(id, args)?;
    let Some(item) = RecentItem::new(RecentKind::Notebook, path) else {
        return Err(bad_path(
            id,
            &format!("path must be 1 to {MAX_PATH_CHARS} characters with no control characters"),
        ));
    };
    let folder = Path::new(&item.path);
    if !folder.exists() {
        tracing::warn!(path = %item.path, "notebook folder not found; showing the sample notebooks");
        return Ok(open(session, item));
    }
    let NavOpened { opened, titles } =
        open_folder_for_navigation(folder).map_err(|err| bad_path(id, &err.to_string()))?;
    let mut fields = Map::new();
    fields.insert("kind".to_owned(), json!(item.kind.id()));
    fields.insert("path".to_owned(), json!(item.path));
    fields.insert("id".to_owned(), json!(opened.notebook.id));
    if !opened.warnings.is_empty() {
        fields.insert("warnings".to_owned(), json!(opened.warnings));
    }
    let value = Value::Object(fields);
    let notebook_id = opened.notebook.id.clone();
    session.open_notebook(opened.notebook);
    session.set_notebook_root(&notebook_id, folder);
    session.remember_frontmatter_titles(&notebook_id, titles);
    session.recents_mut().push(item);
    session.mark_changed();
    Ok(CommandOutput { value })
}

/// `file.open { path }`: open a `.md` or `.notecraft` page file.
pub(super) fn open_file(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    open_kind(ids::FILE_OPEN, RecentKind::Page, session, args)
}

/// `file.recent.clear`: empty the recent list.
pub(super) fn recent_clear(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    if !session.recents().is_empty() {
        session.recents_mut().clear();
        session.mark_changed();
    }
    Ok(CommandOutput::default())
}

/// `file.about`: show the About page. The engine has nothing to change; the front end opens
/// the page (it alone knows the build details), so this succeeds and does no more.
pub(super) fn about(
    _session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    Ok(CommandOutput::default())
}

/// `file.copy_about_info`: copy the About details. The front end builds the text and owns the
/// clipboard, so this succeeds and does no more.
pub(super) fn copy_about_info(
    _session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    Ok(CommandOutput::default())
}

/// `file.restore_unsaved`: put back the unsaved text found from the last session. The output
/// has the number of pages restored (`restored`).
pub(super) fn restore_unsaved(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let restored = session.restore_recovery();
    Ok(CommandOutput {
        value: json!({ "restored": restored }),
    })
}

/// `file.discard_unsaved`: throw away the unsaved text found from the last session. The
/// output has the number of pages discarded (`discarded`).
pub(super) fn discard_unsaved(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let discarded = session.discard_recovery();
    Ok(CommandOutput {
        value: json!({ "discarded": discarded }),
    })
}

fn settle_conflict(
    id: CommandId,
    session: &mut Session,
    keep_mine: bool,
) -> Result<CommandOutput, CommandError> {
    if session.resolve_current_conflict(keep_mine) {
        Ok(CommandOutput::default())
    } else {
        Err(bad_path(id, "the page has no outside change to settle"))
    }
}

/// `file.keep_my_version`: keep the page text as it is; the next save replaces the file.
pub(super) fn keep_my_version(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    settle_conflict(ids::FILE_KEEP_MY_VERSION, session, true)
}

/// `file.use_outside_version`: replace the page text with the version from disk.
pub(super) fn use_outside_version(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    settle_conflict(ids::FILE_USE_OUTSIDE_VERSION, session, false)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::is_page_file;
    use crate::cmd::Registry;
    use crate::recent::RecentKind;
    use crate::session::Session;

    fn run(session: &mut Session, id: &str, args: Value) -> Result<Value, &'static str> {
        Registry::builtin()
            .run(session, id, args)
            .map(|out| out.value)
            .map_err(|err| err.code())
    }

    fn recent(session: &Session) -> Vec<(RecentKind, String)> {
        session
            .recents()
            .items()
            .iter()
            .map(|item| (item.kind, item.path.clone()))
            .collect()
    }

    #[test]
    fn opening_a_notebook_records_it_and_shows_the_mock() {
        let mut session = Session::new();
        assert!(session.notebooks().is_empty());
        let out = run(
            &mut session,
            "file.open_notebook",
            json!({ "path": "C:\\Notes\\Work" }),
        );
        assert_eq!(
            out,
            Ok(json!({ "kind": "notebook", "path": "C:\\Notes\\Work" }))
        );
        assert_eq!(session.notebooks().len(), 2);
        assert!(session.current_page().is_some());
        assert_eq!(
            recent(&session),
            [(RecentKind::Notebook, "C:\\Notes\\Work".to_owned())]
        );
    }

    fn sample_path() -> String {
        let path: std::path::PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            "testdata",
            "notebooks",
            "basic",
        ]
        .iter()
        .collect();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn opening_a_real_folder_shows_its_sections_and_pages() {
        let mut session = Session::new();
        // The sample notebooks are open first, as at app start.
        session.set_notebooks(crate::mock::notebooks());
        let path = sample_path();
        let out = run(&mut session, "file.open_notebook", json!({ "path": path })).unwrap();
        assert_eq!(out["id"], "nb_basic_sample");
        assert!(out.get("warnings").is_none());
        let names: Vec<&str> = session
            .notebooks()
            .iter()
            .map(|nb| nb.name.as_str())
            .collect();
        assert_eq!(names, ["Basic Sample"], "the mock notebooks are replaced");
        let sections: Vec<&str> = session
            .current_notebook()
            .unwrap()
            .all_sections()
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(sections, ["Journal", "Beta", "Alpha", "Reference"]);
        assert_eq!(session.current_section().unwrap().name, "Journal");
        assert_eq!(session.current_page().unwrap().title, "Welcome");
        assert_eq!(recent(&session), [(RecentKind::Notebook, path)]);
    }

    #[test]
    fn reopening_a_real_folder_keeps_the_selection() {
        let mut session = Session::new();
        let path = sample_path();
        run(&mut session, "file.open_notebook", json!({ "path": path })).unwrap();
        let page = "nb_basic_sample/Journal/Tuesday.md";
        run(&mut session, "nav.page.select", json!({ "id": page })).unwrap();
        run(&mut session, "file.open_notebook", json!({ "path": path })).unwrap();
        assert_eq!(session.notebooks().len(), 1);
        assert_eq!(session.selection().page.as_deref(), Some(page));
    }

    #[test]
    fn a_real_folder_is_added_beside_another_real_one() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("Only")).unwrap();
        std::fs::write(dir.path().join("Only").join("Note.md"), "x").unwrap();
        let mut session = Session::new();
        run(
            &mut session,
            "file.open_notebook",
            json!({ "path": sample_path() }),
        )
        .unwrap();
        let other = dir.path().to_string_lossy().into_owned();
        run(&mut session, "file.open_notebook", json!({ "path": other })).unwrap();
        assert_eq!(session.notebooks().len(), 2);
        assert_eq!(session.current_page().unwrap().title, "Note");
    }

    #[test]
    fn a_file_path_is_not_a_notebook() {
        let mut session = Session::new();
        let file = format!("{}/Journal/Welcome.md", sample_path());
        let before = session.clone();
        assert_eq!(
            run(&mut session, "file.open_notebook", json!({ "path": file })),
            Err("invalid_args")
        );
        assert_eq!(session, before);
    }

    #[test]
    fn scan_warnings_are_reported() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".notecraft")).unwrap();
        std::fs::write(
            dir.path().join(".notecraft").join("notebook.toml"),
            "= broken",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("S")).unwrap();
        std::fs::write(dir.path().join("S").join("a.md"), "x").unwrap();
        let mut session = Session::new();
        let path = dir.path().to_string_lossy().into_owned();
        let out = run(&mut session, "file.open_notebook", json!({ "path": path })).unwrap();
        assert_eq!(out["warnings"].as_array().map(Vec::len), Some(1));
        assert_eq!(session.current_page().unwrap().title, "a");
    }

    #[test]
    fn opening_again_keeps_the_selection() {
        let mut session = Session::new();
        run(
            &mut session,
            "file.new_notebook",
            json!({ "path": "/no-such-parent/a" }),
        )
        .unwrap();
        run(&mut session, "nav.page.select", json!({ "id": "p-ideas" })).unwrap();
        run(&mut session, "file.open", json!({ "path": "/b/Page.MD" })).unwrap();
        assert_eq!(session.selection().page.as_deref(), Some("p-ideas"));
        assert_eq!(
            recent(&session),
            [
                (RecentKind::Page, "/b/Page.MD".to_owned()),
                (RecentKind::Notebook, "/no-such-parent/a".to_owned()),
            ]
        );
    }

    #[test]
    fn bad_paths_are_rejected_and_change_nothing() {
        let mut session = Session::new();
        let before = session.clone();
        for (id, args) in [
            ("file.open_notebook", json!(null)),
            ("file.open_notebook", json!({ "path": "  " })),
            ("file.open_notebook", json!({ "path": "a\u{0}b" })),
            ("file.new_notebook", json!({ "path": "x".repeat(5000) })),
            ("file.open", json!({ "path": "notes.txt" })),
            ("file.open", json!({ "path": "/notes/.md" })),
            ("file.open", json!({ "path": "md" })),
            ("file.open", json!({ "path": 7 })),
        ] {
            assert_eq!(
                run(&mut session, id, args.clone()),
                Err("invalid_args"),
                "{id} {args}"
            );
        }
        assert_eq!(session, before);
    }

    #[test]
    fn clear_empties_the_list() {
        let mut session = Session::new();
        run(&mut session, "file.recent.clear", json!(null)).unwrap();
        assert_eq!(
            session.revision(),
            0,
            "clearing an empty list is not a change"
        );
        run(&mut session, "file.open", json!({ "path": "a.notecraft" })).unwrap();
        run(&mut session, "file.recent.clear", json!(null)).unwrap();
        assert!(session.recents().is_empty());
        assert_eq!(
            session.notebooks().len(),
            2,
            "clearing does not close anything"
        );
    }

    #[test]
    fn a_new_notebook_is_created_on_disk_and_opened() {
        let dir = tempfile::TempDir::new().unwrap();
        let folder = dir.path().join("Fresh");
        let path = folder.to_string_lossy().into_owned();
        let mut session = Session::new();
        let out = run(&mut session, "file.new_notebook", json!({ "path": path })).unwrap();
        assert!(folder.join(".notecraft/notebook.toml").is_file());
        assert_eq!(out["id"], session.current_notebook().unwrap().id.as_str());
        assert_eq!(session.current_notebook().unwrap().name, "Fresh");
        assert_eq!(recent(&session), [(RecentKind::Notebook, path)]);
    }

    #[test]
    fn page_file_extensions() {
        for good in ["a.md", "A.MD", "x/y.notecraft", "C:\\n\\p.NoteCraft"] {
            assert!(is_page_file(good), "{good}");
        }
        for bad in ["a.txt", "a", ".md", "dir/.md", "a.md.bak", "a."] {
            assert!(!is_page_file(bad), "{bad}");
        }
    }
}
