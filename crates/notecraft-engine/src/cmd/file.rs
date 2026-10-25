//! File commands: `file.new_notebook`, `file.open_notebook`, `file.open`,
//! `file.recent.clear`, and `file.recent.remove`. Each open records the path in the recent
//! list. Opening a notebook folder reads it from disk. `file.open` finds the notebook a page
//! file belongs to and shows the page there, or shows the file on its own as a loose page
//! (`session/loose.rs`). A path that does not exist is an error, never sample data.
//!
//! The `path` argument is optional in the schema so a ribbon button can send the command
//! without one; the front end then asks with a file picker and sends it again with the path.
//! Run without a path (from the control channel), the command fails with `invalid_args`.

use std::path::Path;

use serde_json::json;

use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::open::{find_notebook_root, open_folder_for_navigation, NavOpened};
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

/// The message for a path that no longer exists (a stale recent entry, a moved folder).
fn not_found(what: &str, path: &str) -> String {
    format!("the {what} was not found: {path}. It may have been moved, renamed, or deleted.")
}

/// `file.new_notebook { path }`: create a notebook in a folder and open it. The folder is made
/// when it does not exist (its parent must), and gets `.notecraft/notebook.toml`; files already
/// in it stay as they are. A path whose parent folder does not exist is `invalid_args` with a
/// message that says so; nothing is recorded and no sample data is shown.
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
    let absolute = std::path::absolute(folder).unwrap_or_else(|_| folder.to_path_buf());
    let can_create = absolute.is_dir() || absolute.parent().is_some_and(Path::is_dir);
    if !can_create {
        tracing::warn!(path = %item.path, "parent folder not found; no notebook created");
        return Err(bad_path(
            id,
            &format!(
                "the folder for the new notebook cannot be created because its parent folder was not found: {}. Choose a folder inside one that exists.",
                item.path
            ),
        ));
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
/// path that does not exist is `invalid_args` with a message that says so (nothing is
/// recorded, and no sample data is shown), as is a path that exists but is not a readable
/// folder.
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
        tracing::warn!(path = %item.path, "notebook folder not found");
        return Err(bad_path(id, &not_found("notebook folder", &item.path)));
    }
    let nav = open_folder_for_navigation(folder).map_err(|err| bad_path(id, &err.to_string()))?;
    let notebook_id = nav.opened.notebook.id.clone();
    let warnings = register_notebook(session, folder, nav);
    let mut fields = Map::new();
    fields.insert("kind".to_owned(), json!(item.kind.id()));
    fields.insert("path".to_owned(), json!(item.path));
    fields.insert("id".to_owned(), json!(notebook_id));
    if !warnings.is_empty() {
        fields.insert("warnings".to_owned(), json!(warnings));
    }
    session.recents_mut().push(item);
    session.mark_changed();
    Ok(CommandOutput {
        value: Value::Object(fields),
    })
}

/// Show a scanned notebook (replacing the sample notebooks) and remember its folder. Returns
/// the scan warnings.
fn register_notebook(session: &mut Session, folder: &Path, nav: NavOpened) -> Vec<String> {
    let NavOpened { opened, titles } = nav;
    let notebook_id = opened.notebook.id.clone();
    session.open_notebook(opened.notebook);
    session.set_notebook_root(&notebook_id, folder);
    session.remember_frontmatter_titles(&notebook_id, titles);
    opened.warnings
}

/// The id a page file has inside the notebook folder `root` (`<notebook id>/<path>`).
fn page_id_in(notebook: &str, root: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(root).ok()?;
    let mut id = notebook.to_owned();
    for part in rel.components() {
        let std::path::Component::Normal(part) = part else {
            return None;
        };
        id.push('/');
        id.push_str(part.to_str()?);
    }
    Some(id)
}

/// What opening a page file showed: its notebook (open already or just opened) and page.
struct InNotebook {
    notebook: String,
    page: String,
    warnings: Vec<String>,
}

/// Show `file` as a page of the notebook folder `root`: switch to the notebook if it is open,
/// otherwise open it. `None` when the notebook cannot be read or does not list the file as a
/// page (a file directly in the notebook folder, or one the scan skipped); the caller then
/// shows the file as a loose page, and nothing has changed.
fn open_in_notebook(session: &mut Session, root: &Path, file: &Path) -> Option<InNotebook> {
    if let Some(notebook) = session.notebook_id_for_root(root) {
        let page = page_id_in(&notebook, root, file)?;
        return session.select_page_by_id(&page).then_some(InNotebook {
            notebook,
            page,
            warnings: Vec::new(),
        });
    }
    let nav = open_folder_for_navigation(root).ok()?;
    let notebook = nav.opened.notebook.id.clone();
    let page = page_id_in(&notebook, root, file)?;
    nav.opened.notebook.section_of_page(&page)?;
    let warnings = register_notebook(session, root, nav);
    session.select_page_by_id(&page);
    Some(InNotebook {
        notebook,
        page,
        warnings,
    })
}

/// `file.open { path }`: open a `.md` or `.notecraft` page file. The nearest ancestor folder
/// with a `.notecraft/` folder is its notebook: the notebook is opened (or switched to) and
/// the page selected; the output has `notebook` and `page`, and `warnings` from a fresh scan.
/// A file outside any notebook opens as a loose page (`loose: true`, with `page`), and no
/// folder is created next to it. A file that does not exist is `invalid_args` with a message
/// that says so; nothing is recorded and no sample data is shown.
pub(super) fn open_file(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::FILE_OPEN;
    let path = path_arg(id, args)?;
    let Some(item) = RecentItem::new(RecentKind::Page, path) else {
        return Err(bad_path(
            id,
            &format!("path must be 1 to {MAX_PATH_CHARS} characters with no control characters"),
        ));
    };
    if !is_page_file(&item.path) {
        return Err(bad_path(id, "only .md and .notecraft files can be opened"));
    }
    let given = Path::new(&item.path);
    let file = std::path::absolute(given).unwrap_or_else(|_| given.to_path_buf());
    if !file.exists() {
        tracing::warn!(path = %item.path, "page file not found");
        return Err(bad_path(id, &not_found("page file", &item.path)));
    }
    if !file.is_file() {
        return Err(bad_path(
            id,
            "that path is a folder; use Open Notebook to open it",
        ));
    }
    let mut fields = Map::new();
    fields.insert("kind".to_owned(), json!(item.kind.id()));
    fields.insert("path".to_owned(), json!(item.path));
    let in_notebook =
        find_notebook_root(&file).and_then(|root| open_in_notebook(session, &root, &file));
    if let Some(found) = in_notebook {
        fields.insert("notebook".to_owned(), json!(found.notebook));
        fields.insert("page".to_owned(), json!(found.page));
        if !found.warnings.is_empty() {
            fields.insert("warnings".to_owned(), json!(found.warnings));
        }
    } else {
        let Some(page) = session.open_loose_page(&file) else {
            return Err(bad_path(id, "that file cannot be opened"));
        };
        fields.insert("loose".to_owned(), json!(true));
        fields.insert("page".to_owned(), json!(page));
    }
    session.recents_mut().push(item);
    session.mark_changed();
    Ok(CommandOutput {
        value: Value::Object(fields),
    })
}

/// `file.recent.remove { path, kind? }`: drop a notebook or page from the recent list, as
/// offered when its path no longer exists. The output has `removed` (whether anything went).
pub(super) fn recent_remove(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::FILE_RECENT_REMOVE;
    let path = path_arg(id, args)?;
    let kind = match args.get("kind").and_then(Value::as_str) {
        None => None,
        Some(text) => Some(
            RecentKind::from_id(text)
                .ok_or_else(|| bad_path(id, "kind must be notebook or page"))?,
        ),
    };
    let removed = session.recents_mut().remove(kind, path.trim());
    if removed {
        session.mark_changed();
    }
    Ok(CommandOutput {
        value: json!({ "removed": removed }),
    })
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

/// `help.whats_new`: show the What's New dialog. The front end owns the dialog, so this
/// succeeds and does no more.
pub(super) fn whats_new(
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
    fn a_missing_notebook_path_is_an_error_and_shows_no_sample_data() {
        let mut session = Session::new();
        let before = session.clone();
        let out = Registry::builtin().run(
            &mut session,
            "file.open_notebook",
            json!({ "path": "C:\\Notes\\Work" }),
        );
        let message = out.expect_err("missing").to_string();
        assert!(message.contains("was not found"), "{message}");
        assert!(message.contains("C:\\Notes\\Work"), "{message}");
        assert_eq!(session, before, "no sample notebooks, no recent entry");
        assert!(session.notebooks().is_empty());
    }

    /// A notebook folder: `.notecraft/`, a section with a page, and a group holding a section.
    fn notebook_dir() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        for file in [
            "Work/Plan.md",
            "Group/Inner/Deep.md",
            "Group/Inner/Other.md",
        ] {
            let path = dir.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, format!("# {file}\n")).unwrap();
        }
        std::fs::create_dir_all(dir.path().join(".notecraft")).unwrap();
        std::fs::write(dir.path().join("Root.md"), "# Root\n").unwrap();
        dir
    }

    fn text(path: &std::path::Path) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn a_page_in_a_section_opens_its_notebook_and_selects_the_page() {
        let dir = notebook_dir();
        let file = text(&dir.path().join("Work/Plan.md"));
        let mut session = Session::new();
        session.set_notebooks(crate::mock::notebooks());
        let out = run(&mut session, "file.open", json!({ "path": file })).unwrap();
        assert_eq!(out["kind"], "page");
        assert_eq!(out.get("loose"), None);
        let notebook = session.current_notebook().unwrap();
        assert_eq!(
            session.notebooks().len(),
            1,
            "the sample notebooks are gone"
        );
        assert_eq!(out["notebook"], notebook.id.as_str());
        assert!(!session.is_loose_notebook(&notebook.id));
        assert_eq!(session.current_section().unwrap().name, "Work");
        assert_eq!(session.current_page().unwrap().title, "Plan");
        assert_eq!(out["page"], session.current_page().unwrap().id.as_str());
        session.load_current_page();
        assert_eq!(
            session.current_page_source().unwrap().text,
            "# Work/Plan.md\n"
        );
        assert_eq!(recent(&session), [(RecentKind::Page, file)]);
    }

    #[test]
    fn a_page_in_a_section_group_selects_that_page_not_the_first() {
        let dir = notebook_dir();
        let file = text(&dir.path().join("Group/Inner/Other.md"));
        let mut session = Session::new();
        run(&mut session, "file.open", json!({ "path": file })).unwrap();
        assert_eq!(session.current_section().unwrap().name, "Inner");
        assert_eq!(session.current_page().unwrap().title, "Other");
        assert!(!session.is_loose_notebook(&session.current_notebook().unwrap().id));
    }

    #[test]
    fn a_page_of_an_open_notebook_switches_to_it_without_reading_it_again() {
        let dir = notebook_dir();
        let root = text(dir.path());
        let mut session = Session::new();
        run(&mut session, "file.open_notebook", json!({ "path": root })).unwrap();
        let notebook = session.current_notebook().unwrap().id.clone();
        let generation = session.tree_generation(&notebook);
        let file = text(&dir.path().join("Group/Inner/Deep.md"));
        let out = run(&mut session, "file.open", json!({ "path": file })).unwrap();
        assert_eq!(out["notebook"], notebook.as_str());
        assert_eq!(session.notebooks().len(), 1);
        assert_eq!(session.tree_generation(&notebook), generation);
        assert_eq!(session.current_page().unwrap().title, "Deep");
    }

    #[test]
    fn a_page_in_another_open_notebook_is_found_among_several() {
        let first = notebook_dir();
        let second = notebook_dir();
        let mut session = Session::new();
        let first_root = text(first.path());
        run(
            &mut session,
            "file.open_notebook",
            json!({ "path": first_root }),
        )
        .unwrap();
        let file = text(&second.path().join("Work/Plan.md"));
        run(&mut session, "file.open", json!({ "path": file })).unwrap();
        assert_eq!(session.notebooks().len(), 2);
        let other = text(&first.path().join("Work/Plan.md"));
        run(&mut session, "file.open", json!({ "path": other })).unwrap();
        assert_eq!(session.notebooks().len(), 2, "switching adds nothing");
        assert_eq!(
            session.page_file_path(&session.selection().page.clone().unwrap()),
            Some(first.path().join("Work/Plan.md"))
        );
    }

    #[test]
    fn a_file_outside_any_notebook_opens_as_a_loose_page_and_saves_in_place() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("Loose note.md");
        std::fs::write(&file, "# Loose\n").unwrap();
        let mut session = Session::new();
        session.set_notebooks(crate::mock::notebooks());
        let out = run(&mut session, "file.open", json!({ "path": text(&file) })).unwrap();
        assert_eq!(out["loose"], true);
        let notebook = session.current_notebook().unwrap().clone();
        assert!(session.is_loose_notebook(&notebook.id));
        assert_eq!(
            session.notebooks().len(),
            1,
            "the sample notebooks are gone"
        );
        assert_eq!(session.current_page().unwrap().title, "Loose note");
        session.load_current_page();
        let page = session.selection().page.clone().unwrap();
        assert_eq!(session.current_page_source().unwrap().text, "# Loose\n");
        assert!(session.page_is_editable(&page));
        // Edit and save through the normal path: the one file changes.
        session.set_page_source(&page, "# Loose\nmore\n");
        session.mark_page_dirty(&page);
        session.save_page(&page);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "# Loose\nmore\n");
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["Loose note.md"], "no .notecraft folder, no extras");
        assert_eq!(recent(&session), [(RecentKind::Page, text(&file))]);
        // Not a notebook: nothing watches or indexes its folder, and it cannot be organized.
        assert_eq!(session.notebook_roots().count(), 0);
        assert_eq!(session.notebook_folders().count(), 0);
        assert!(run(&mut session, "page.new", json!(null)).is_err());
        // Opening it again shows the same page, not a second notebook.
        run(&mut session, "file.open", json!({ "path": text(&file) })).unwrap();
        assert_eq!(session.notebooks().len(), 1);
    }

    #[test]
    fn closing_a_loose_page_can_be_undone_and_reopens_it_as_a_loose_page() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("Loose.md");
        std::fs::write(&file, "# Loose\n").unwrap();
        let other = notebook_dir();
        let mut session = Session::new();
        run(
            &mut session,
            "file.open_notebook",
            json!({ "path": text(other.path()) }),
        )
        .unwrap();
        run(&mut session, "file.open", json!({ "path": text(&file) })).unwrap();
        let loose = session.current_notebook().unwrap().id.clone();
        assert_eq!(session.notebooks().len(), 2);
        run(&mut session, "file.close_notebook", json!({ "id": loose })).unwrap();
        assert_eq!(session.notebooks().len(), 1);
        assert_eq!(session.notebook_folders().count(), 1);
        run(&mut session, "edit.undo", json!(null)).unwrap();
        assert_eq!(session.notebooks().len(), 2);
        assert_eq!(session.notebooks()[1].id, loose, "back in its place");
        assert!(session.is_loose_notebook(&loose));
        assert_eq!(session.current_page().unwrap().title, "Loose");
        assert_eq!(session.loose_files(), std::slice::from_ref(&file));
        // Redo closes it again; the file is untouched, and no notebook folder appeared.
        run(&mut session, "edit.redo", json!(null)).unwrap();
        assert_eq!(session.notebooks().len(), 1);
        assert!(
            session.loose_files().is_empty(),
            "a closed page is not watched"
        );
        assert!(!dir.path().join(".notecraft").exists());
    }

    #[test]
    fn a_file_in_the_notebook_folder_itself_opens_as_a_loose_page() {
        let dir = notebook_dir();
        let file = text(&dir.path().join("Root.md"));
        let mut session = Session::new();
        let out = run(&mut session, "file.open", json!({ "path": file })).unwrap();
        assert_eq!(
            out["loose"], true,
            "it is not in any section of the notebook"
        );
        assert!(session.is_loose_notebook(&session.current_notebook().unwrap().id));
        session.load_current_page();
        assert_eq!(session.current_page_source().unwrap().text, "# Root\n");
    }

    #[test]
    fn a_missing_page_file_is_an_error_and_shows_no_sample_data() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = text(&dir.path().join("gone.md"));
        let mut session = Session::new();
        let before = session.clone();
        let message = Registry::builtin()
            .run(&mut session, "file.open", json!({ "path": file }))
            .expect_err("missing")
            .to_string();
        assert!(message.contains("was not found"), "{message}");
        assert!(message.contains("gone.md"), "{message}");
        assert_eq!(session, before);
        // A folder with a page-like name is not a file either.
        let folder = dir.path().join("dir.md");
        std::fs::create_dir_all(&folder).unwrap();
        assert_eq!(
            run(&mut session, "file.open", json!({ "path": text(&folder) })),
            Err("invalid_args")
        );
        assert_eq!(session, before);
    }

    #[test]
    fn a_page_recent_reopens_and_a_stale_one_can_be_removed() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("Recent.md");
        std::fs::write(&file, "x").unwrap();
        let mut first = Session::new();
        run(&mut first, "file.open", json!({ "path": text(&file) })).unwrap();
        // A new run restores the list, then reopens the newest entry.
        let mut session = Session::new();
        session.set_recents(first.recents().clone());
        let item = session.recents().items()[0].clone();
        run(&mut session, "file.open", json!({ "path": item.path })).unwrap();
        assert_eq!(session.current_page().unwrap().title, "Recent");
        // The file goes away: reopening fails, and the entry can be removed.
        std::fs::remove_file(&file).unwrap();
        let mut later = Session::new();
        later.set_recents(first.recents().clone());
        assert_eq!(
            run(&mut later, "file.open", json!({ "path": item.path })),
            Err("invalid_args")
        );
        assert!(later.notebooks().is_empty());
        assert_eq!(
            later.recents().items().len(),
            1,
            "removal is the user's call"
        );
        let out = run(
            &mut later,
            "file.recent.remove",
            json!({ "path": item.path, "kind": "page" }),
        )
        .unwrap();
        assert_eq!(out["removed"], true);
        assert!(later.recents().is_empty());
        let again = run(
            &mut later,
            "file.recent.remove",
            json!({ "path": item.path }),
        )
        .unwrap();
        assert_eq!(again["removed"], false);
        assert_eq!(
            run(
                &mut later,
                "file.recent.remove",
                json!({ "path": "a", "kind": "folder" })
            ),
            Err("invalid_args")
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
    fn the_newest_open_is_first_in_the_recent_list() {
        let dir = notebook_dir();
        let mut session = Session::new();
        let root = text(dir.path());
        let page = text(&dir.path().join("Work/Plan.md"));
        run(&mut session, "file.open_notebook", json!({ "path": root })).unwrap();
        run(&mut session, "file.open", json!({ "path": page })).unwrap();
        assert_eq!(
            recent(&session),
            [(RecentKind::Page, page), (RecentKind::Notebook, root)]
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
        let dir = notebook_dir();
        let root = text(dir.path());
        run(&mut session, "file.open_notebook", json!({ "path": root })).unwrap();
        run(&mut session, "file.recent.clear", json!(null)).unwrap();
        assert!(session.recents().is_empty());
        assert_eq!(
            session.notebooks().len(),
            1,
            "clearing does not close anything"
        );
    }

    #[test]
    fn a_new_notebook_with_a_missing_parent_is_an_error_and_shows_no_sample_data() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = text(&dir.path().join("no-such-parent").join("Fresh"));
        let mut session = Session::new();
        let before = session.clone();
        let message = Registry::builtin()
            .run(&mut session, "file.new_notebook", json!({ "path": path }))
            .expect_err("missing parent")
            .to_string();
        assert!(message.contains("parent folder was not found"), "{message}");
        assert_eq!(session, before, "nothing recorded, no sample notebooks");
        assert!(!dir.path().join("no-such-parent").exists());
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
