use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::*;
use crate::open::open_folder;

struct Fixture {
    notebook: tempfile::TempDir,
    journal: tempfile::TempDir,
}

impl Fixture {
    fn new(text: &str) -> Self {
        let notebook = tempfile::TempDir::new().expect("temp dir");
        let path = notebook.path().join("Sec/A.md");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, text).expect("write");
        Self {
            notebook,
            journal: tempfile::TempDir::new().expect("temp dir"),
        }
    }

    fn page_file(&self) -> PathBuf {
        self.notebook.path().join("Sec/A.md")
    }

    fn on_disk(&self) -> String {
        std::fs::read_to_string(self.page_file()).expect("read")
    }

    fn journal(&self) -> Journal {
        Journal::new(self.journal.path().join("recovery"))
    }

    fn io(&self) -> SessionIo {
        SessionIo::new(Some(self.journal.path().join("recovery")), || {})
            .with_debounce(Duration::from_millis(100))
    }

    fn session(&self) -> Session {
        let mut session = Session::new();
        let opened = open_folder(self.notebook.path()).expect("scan");
        let id = opened.notebook.id.clone();
        session.open_notebook(opened.notebook);
        session.set_notebook_root(&id, self.notebook.path());
        session
    }
}

/// Pump until `done` holds, or fail after 10 seconds.
fn pump_until(
    io: &mut SessionIo,
    session: &mut Session,
    what: &str,
    done: impl Fn(&Session) -> bool,
) {
    let start = Instant::now();
    while !done(session) {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out: {what}"
        );
        io.pump(session, Instant::now(), true);
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn loaded(io: &mut SessionIo, session: &mut Session) {
    pump_until(io, session, "load", |s| s.current_page_source().is_some());
}

fn page_id(session: &Session) -> String {
    session.selection().page.clone().expect("page")
}

#[test]
fn pages_load_on_the_worker_and_save_on_the_timer() {
    let fx = Fixture::new("one\n");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);
    session.edit_page_text(&id, 0, 0, "x", false);

    let t0 = Instant::now();
    io.pump(&mut session, t0, true);
    assert!(session.is_page_dirty(&id), "not due yet");
    io.pump(&mut session, t0 + Duration::from_secs(29), true);
    assert!(session.is_page_dirty(&id));
    let next = io.pump(&mut session, t0 + Duration::from_secs(31), true);
    assert!(!session.is_page_dirty(&id), "30 s passed");
    assert_eq!(fx.on_disk(), "xone\n");
    assert!(next.is_some());
}

#[test]
fn losing_focus_and_switching_pages_save() {
    let fx = Fixture::new("one\n");
    std::fs::write(fx.notebook.path().join("Sec/B.md"), "two\n").expect("write");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);
    let now = Instant::now();
    session.edit_page_text(&id, 0, 0, "a", false);
    io.pump(&mut session, now, true);
    assert!(session.is_page_dirty(&id));
    io.pump(&mut session, now, false);
    assert!(!session.is_page_dirty(&id), "focus loss saves");

    session.edit_page_text(&id, 0, 0, "b", false);
    let other = session
        .current_section()
        .and_then(|s| s.pages.iter().find(|p| p.id != id))
        .map(|p| p.id.clone())
        .expect("second page");
    session.selection_mut().page = Some(other);
    io.pump(&mut session, now, true);
    assert!(!session.is_page_dirty(&id), "page switch saves");
    assert_eq!(fx.on_disk(), "baone\n");
}

#[test]
fn a_crash_leaves_a_journal_that_the_next_run_offers_back() {
    let fx = Fixture::new("saved\n");
    {
        let mut session = fx.session();
        let mut io = fx.io();
        loaded(&mut io, &mut session);
        let id = page_id(&session);
        session.edit_page_text(&id, 0, 0, "unsaved ", false);
        let now = Instant::now();
        io.pump(&mut session, now, true);
        io.pump(&mut session, now + JOURNAL_EVERY, true);
        // Crash: no flush, nothing saved.
    }
    assert_eq!(fx.on_disk(), "saved\n");

    let mut session = fx.session();
    let mut io = fx.io();
    pump_until(&mut io, &mut session, "offer", |s| {
        !s.pending_recovery().is_empty()
    });
    assert_eq!(session.pending_recovery()[0].text, "unsaved saved\n");
    session.restore_recovery();
    io.flush(&mut session);
    assert_eq!(fx.on_disk(), "unsaved saved\n");
    assert!(
        fx.journal().load_all().is_empty(),
        "saved text clears the journal"
    );
}

#[test]
fn flush_on_quit_saves_and_leaves_no_journal() {
    let fx = Fixture::new("one\n");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);
    session.edit_page_text(&id, 3, 0, "!", false);
    io.flush(&mut session);
    assert_eq!(fx.on_disk(), "one!\n");
    assert!(fx.journal().load_all().is_empty());
}

#[test]
fn a_page_that_cannot_be_saved_stays_in_the_journal_on_quit() {
    let fx = Fixture::new("one\n");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);
    session.edit_page_text(&id, 0, 0, "mine ", false);
    std::fs::write(fx.page_file(), "theirs\n").expect("outside edit");
    io.flush(&mut session);
    assert_eq!(fx.on_disk(), "theirs\n");
    assert_eq!(fx.journal().load_all().len(), 1);
}

#[test]
fn the_watcher_reloads_a_clean_page_and_flags_a_dirty_one() {
    let fx = Fixture::new("one\n");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);

    std::fs::write(fx.page_file(), "outside\n").expect("outside edit");
    pump_until(&mut io, &mut session, "reload", |s| {
        s.page_text(&page_id(s)) == Some("outside\n")
    });
    assert!(!session.is_page_dirty(&id));

    session.edit_page_text(&id, 0, 0, "mine ", false);
    std::fs::write(fx.page_file(), "outside again\n").expect("outside edit");
    pump_until(&mut io, &mut session, "conflict", |s| {
        s.current_conflict().is_some()
    });
    assert_eq!(session.current_conflict(), Some("outside again\n"));
    assert_eq!(session.page_text(&id), Some("mine outside\n"));
}
