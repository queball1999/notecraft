//! Outside edits and crash recovery for loose pages (`io/loose.rs`).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::*;

struct Fixture {
    folder: tempfile::TempDir,
    journal: tempfile::TempDir,
}

impl Fixture {
    fn new(text: &str) -> Self {
        let folder = tempfile::TempDir::new().expect("temp dir");
        std::fs::write(folder.path().join("Loose.md"), text).expect("write");
        // A neighbor that must stay out of everything.
        std::fs::write(folder.path().join("Neighbor.md"), "neighbor\n").expect("write");
        Self {
            folder,
            journal: tempfile::TempDir::new().expect("temp dir"),
        }
    }

    fn file(&self) -> PathBuf {
        self.folder.path().join("Loose.md")
    }

    fn on_disk(&self) -> String {
        std::fs::read_to_string(self.file()).expect("read")
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
        session.open_loose_page(&self.file()).expect("loose page");
        session
    }
}

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

fn page_id(session: &Session) -> String {
    session.selection().page.clone().expect("page")
}

#[test]
fn an_outside_edit_of_a_loose_page_reloads_it_or_flags_a_conflict() {
    let fx = Fixture::new("one\n");
    let mut session = fx.session();
    let mut io = fx.io();
    pump_until(&mut io, &mut session, "load", |s| {
        s.current_page_source().is_some()
    });
    let id = page_id(&session);

    std::fs::write(fx.file(), "outside\n").expect("outside edit");
    pump_until(&mut io, &mut session, "reload", |s| {
        s.page_text(&page_id(s)) == Some("outside\n")
    });
    assert!(!session.is_page_dirty(&id));

    session.edit_page_text(&id, 0, 0, "mine ", false);
    std::fs::write(fx.file(), "outside again\n").expect("outside edit");
    pump_until(&mut io, &mut session, "conflict", |s| {
        s.current_conflict().is_some()
    });
    assert_eq!(session.current_conflict(), Some("outside again\n"));
    assert!(!fx.folder.path().join(".notecraft").exists());
}

#[test]
fn a_neighbor_file_is_not_watched_or_indexed() {
    let fx = Fixture::new("one\n");
    let mut session = fx.session();
    let mut io = fx.io();
    pump_until(&mut io, &mut session, "load", |s| {
        s.current_page_source().is_some()
    });
    assert_eq!(io.loose.watchers.len(), 1);
    assert!(io.watchers.is_empty(), "no recursive notebook watcher");
    std::fs::write(fx.folder.path().join("Neighbor.md"), "changed\n").expect("write");
    for _ in 0..15 {
        io.pump(&mut session, Instant::now(), true);
        std::thread::sleep(Duration::from_millis(40));
    }
    assert_eq!(session.notebooks().len(), 1);
    assert!(!fx.folder.path().join(".notecraft").exists());
}

#[test]
fn a_crash_leaves_a_journal_that_the_next_run_offers_back_for_a_loose_page() {
    let fx = Fixture::new("saved\n");
    {
        let mut session = fx.session();
        let mut io = fx
            .io()
            .with_save_delays(Duration::from_secs(3600), Duration::from_secs(3600));
        pump_until(&mut io, &mut session, "load", |s| {
            s.current_page_source().is_some()
        });
        let id = page_id(&session);
        session.edit_page_text(&id, 0, 0, "unsaved ", false);
        let now = Instant::now();
        io.pump(&mut session, now, true);
        io.pump(&mut session, now + JOURNAL_EVERY, true);
        // Crash: no flush, nothing saved.
    }
    assert_eq!(fx.on_disk(), "saved\n");
    assert_eq!(
        fx.journal().load_all().len(),
        1,
        "journal is in the settings folder"
    );

    // The next run opens the page again; the offer follows.
    let mut session = fx.session();
    let mut io = fx.io();
    pump_until(&mut io, &mut session, "offer", |s| {
        !s.pending_recovery().is_empty()
    });
    assert_eq!(session.pending_recovery()[0].text, "unsaved saved\n");
    session.restore_recovery();
    io.flush(&mut session);
    assert_eq!(fx.on_disk(), "unsaved saved\n");
    assert!(fx.journal().load_all().is_empty());
    assert!(!fx.folder.path().join(".notecraft").exists());
}

#[test]
fn recovery_text_for_a_neighbor_file_is_not_offered_for_a_loose_page() {
    let fx = Fixture::new("saved\n");
    let neighbor = fx.folder.path().join("Neighbor.md");
    fx.journal()
        .write(&neighbor, "stale neighbor text\n")
        .expect("journal");
    let mut session = fx.session();
    let mut io = fx.io();
    for _ in 0..10 {
        io.pump(&mut session, Instant::now(), true);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(session.pending_recovery().is_empty());
}
