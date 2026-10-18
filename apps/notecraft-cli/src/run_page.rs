//! `run --page <page> --cmd ...`: run commands against one page of a notebook through the
//! engine, then save. Nothing is written when a command fails, and a page whose file changed
//! on disk since it was read is never written over (exit code 5).

use std::process::ExitCode;

use notecraft_engine::cmd::{Registry, Value};
use notecraft_engine::open::notebook_from_tree;
use notecraft_engine::{SaveOutcome, Session};

use crate::notebook::{self, Failure};
use crate::{code, fail, out, run_commands, Globals};

/// What saving did for one page, for the report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub page: String,
    pub outcome: &'static str,
}

/// Save every changed page of `session`. `Err` carries the exit code and message of the worst
/// outcome: 5 if a file changed on disk, else 1.
pub fn save_all(session: &mut Session) -> Result<Vec<Saved>, Failure> {
    let mut saved = Vec::new();
    let mut failure: Option<Failure> = None;
    for (page, outcome) in session.save_dirty() {
        let (label, problem) = match outcome {
            SaveOutcome::Saved | SaveOutcome::Promoted { .. } => ("saved", None),
            SaveOutcome::Unchanged | SaveOutcome::NotDirty => ("unchanged", None),
            SaveOutcome::Conflict => (
                "conflict",
                Some(Failure::new(
                    code::CONFLICT,
                    format!("{page} changed on disk since it was read; nothing was written"),
                )),
            ),
            SaveOutcome::ReadOnly => (
                "read_only",
                Some(Failure::new(
                    code::ERROR,
                    format!("{page} is read-only; nothing was written"),
                )),
            ),
            SaveOutcome::NoFile => (
                "no_file",
                Some(Failure::new(code::ERROR, format!("{page} has no file"))),
            ),
            SaveOutcome::Failed(reason) => (
                "failed",
                Some(Failure::new(
                    code::ERROR,
                    format!("cannot save {page}: {reason}"),
                )),
            ),
        };
        saved.push(Saved {
            page,
            outcome: label,
        });
        // A conflict outranks the other failures; otherwise the first one is kept.
        match (&failure, problem) {
            (None, Some(new)) => failure = Some(new),
            (Some(old), Some(new)) if old.code != code::CONFLICT && new.code == code::CONFLICT => {
                failure = Some(new);
            }
            _ => {}
        }
    }
    match failure {
        Some(failure) => Err(failure),
        None => Ok(saved),
    }
}

/// `run --page`: open the notebook, show the page, run `cmds`, and save.
pub fn run_on_page(globals: &Globals, query: &str, cmds: Vec<(String, Value)>) -> ExitCode {
    let notebook = match notebook::open(globals.notebook.as_deref()) {
        Ok(notebook) => notebook,
        Err(failure) => return fail(globals, &failure),
    };
    let pages = notebook.pages();
    let page = match notebook::resolve(&pages, query) {
        Ok(page) => page,
        Err(failure) => return fail(globals, &failure),
    };

    let registry = Registry::builtin();
    let mut session = Session::new();
    session.open_notebook(notebook_from_tree(&notebook.tree));
    session.set_notebook_root(&notebook.tree.id, &notebook.root);
    if let Err(error) = registry.run(
        &mut session,
        "nav.page.select",
        serde_json::json!({ "id": page.id }),
    ) {
        return fail(globals, &Failure::new(code::ERROR, error.to_string()));
    }
    session.load_current_page();
    if let Some(notice) = session.current_page_notice() {
        if notice.read_only {
            let message = format!("cannot edit {}: {}", page.entry.rel_path, notice.text);
            return fail(globals, &Failure::new(code::ERROR, message));
        }
    }

    if let Err(code) = run_commands(&registry, &mut session, cmds, globals.quiet) {
        return ExitCode::from(code);
    }
    match save_all(&mut session) {
        Ok(saved) => {
            if !globals.quiet {
                let saved: Vec<Value> = saved
                    .iter()
                    .map(|s| serde_json::json!({ "page": s.page, "outcome": s.outcome }))
                    .collect();
                out(&serde_json::json!({ "ok": true, "saved": saved }).to_string());
            }
            ExitCode::SUCCESS
        }
        Err(failure) => fail(globals, &failure),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sample() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let section = dir.path().join("S");
        std::fs::create_dir_all(&section).unwrap();
        std::fs::write(section.join("P.md"), "# Title\n\nBody\n").unwrap();
        dir
    }

    fn open(dir: &std::path::Path) -> (Session, Registry, String) {
        let root = dir.to_str().unwrap();
        let notebook = notebook::open(Some(root)).unwrap();
        let pages = notebook.pages();
        let id = pages[0].id.clone();
        let registry = Registry::builtin();
        let mut session = Session::new();
        session.open_notebook(notebook_from_tree(&notebook.tree));
        session.set_notebook_root(&notebook.tree.id, &notebook.root);
        registry
            .run(
                &mut session,
                "nav.page.select",
                serde_json::json!({ "id": id }),
            )
            .unwrap();
        session.load_current_page();
        (session, registry, id)
    }

    fn type_text(session: &mut Session, registry: &Registry, at: u64, text: &str) {
        registry
            .run(session, "text.set_caret", serde_json::json!({ "at": at }))
            .unwrap();
        registry
            .run(session, "text.insert", serde_json::json!({ "text": text }))
            .unwrap();
    }

    #[test]
    fn a_changed_page_is_saved() {
        let dir = sample();
        let (mut session, registry, _) = open(dir.path());
        type_text(&mut session, &registry, 7, " two");
        let saved = save_all(&mut session).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].outcome, "saved");
        let text = std::fs::read_to_string(dir.path().join("S").join("P.md")).unwrap();
        assert_eq!(text, "# Title two\n\nBody\n");
    }

    #[test]
    fn a_file_changed_on_disk_is_a_conflict_and_is_not_written_over() {
        let dir = sample();
        let (mut session, registry, _) = open(dir.path());
        type_text(&mut session, &registry, 7, " two");
        let file = dir.path().join("S").join("P.md");
        std::fs::write(&file, "# Changed elsewhere\n").unwrap();
        let failure = save_all(&mut session).unwrap_err();
        assert_eq!(failure.code, code::CONFLICT);
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "# Changed elsewhere\n"
        );
    }

    #[test]
    fn nothing_changed_means_nothing_to_save() {
        let dir = sample();
        let (mut session, _, _) = open(dir.path());
        assert_eq!(save_all(&mut session).unwrap(), Vec::new());
    }
}
