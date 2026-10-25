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
fn a_page_saves_after_a_pause_in_editing() {
    let fx = Fixture::new("one\n");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);
    session.edit_page_text(&id, 0, 0, "x", false);

    let t0 = Instant::now();
    io.pump(&mut session, t0, true);
    assert!(session.is_page_dirty(&id), "not due yet");
    io.pump(
        &mut session,
        t0 + SAVE_AFTER_IDLE - Duration::from_millis(100),
        true,
    );
    assert!(session.is_page_dirty(&id), "still inside the quiet time");
    io.pump(&mut session, t0 + SAVE_AFTER_IDLE, true);
    assert!(!session.is_page_dirty(&id), "saved after the pause");
    assert_eq!(fx.on_disk(), "xone\n");
}

#[test]
fn each_edit_restarts_the_quiet_time_until_the_cap() {
    let fx = Fixture::new("one\n");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);
    let t0 = Instant::now();
    // Typing every second: the page never sits idle for 1.5 s, so only the cap saves it.
    let mut second = 0;
    while second < SAVE_MAX_WAIT.as_secs() {
        session.edit_page_text(&id, 0, 0, "k", true);
        io.pump(&mut session, t0 + Duration::from_secs(second), true);
        assert!(session.is_page_dirty(&id), "second {second}");
        second += 1;
    }
    session.edit_page_text(&id, 0, 0, "k", true);
    io.pump(&mut session, t0 + SAVE_MAX_WAIT, true);
    assert!(
        !session.is_page_dirty(&id),
        "the cap saves while typing goes on"
    );
}

#[test]
fn the_pump_reports_when_the_save_falls_due() {
    let fx = Fixture::new("one\n");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);
    session.edit_page_text(&id, 0, 0, "x", false);
    let t0 = Instant::now();
    let wait = io.pump(&mut session, t0, true).expect("a wake time");
    assert!(wait <= SAVE_AFTER_IDLE, "{wait:?}");
}

/// Make saving the page fail: a file takes the place of its section folder.
fn break_saving(fx: &Fixture) {
    std::fs::remove_dir_all(fx.notebook.path().join("Sec")).expect("remove");
    std::fs::write(fx.notebook.path().join("Sec"), "in the way").expect("write");
}

#[test]
fn a_failed_save_is_kept_per_page_and_reported_once() {
    let fx = Fixture::new("one\n");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);
    session.edit_page_text(&id, 0, 0, "mine ", false);
    break_saving(&fx);

    let t0 = Instant::now();
    io.pump(&mut session, t0, true);
    io.pump(&mut session, t0 + SAVE_AFTER_IDLE, true);
    assert!(session.is_page_dirty(&id));
    let reason = io.save_error(&id).expect("the failure is kept").to_owned();
    assert!(!reason.is_empty());
    assert_eq!(io.take_new_save_failures(), vec![(id.clone(), reason)]);

    // The retry after the cap fails again, but it is not new.
    io.pump(&mut session, t0 + SAVE_AFTER_IDLE + SAVE_MAX_WAIT, true);
    assert!(io.save_error(&id).is_some());
    assert!(io.take_new_save_failures().is_empty());

    // Once the folder is back the page saves and the error goes away.
    std::fs::remove_file(fx.notebook.path().join("Sec")).expect("remove");
    std::fs::create_dir_all(fx.notebook.path().join("Sec")).expect("mkdir");
    std::fs::write(fx.page_file(), "one\n").expect("restore file");
    io.pump(&mut session, t0 + SAVE_AFTER_IDLE + SAVE_MAX_WAIT * 2, true);
    assert_eq!(io.save_error(&id), None);
}

#[test]
fn a_failure_found_on_quit_is_reported_and_journaled() {
    let fx = Fixture::new("one\n");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);
    session.edit_page_text(&id, 0, 0, "mine ", false);
    break_saving(&fx);
    let kept = io.flush(&mut session);
    assert_eq!(kept, vec![id.clone()]);
    assert!(io.save_error(&id).is_some());
    assert_eq!(fx.journal().load_all().len(), 1);
}

#[test]
fn a_clean_quit_keeps_nothing_for_recovery() {
    let fx = Fixture::new("one\n");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);
    session.edit_page_text(&id, 0, 0, "x", false);
    assert!(io.flush(&mut session).is_empty());
}

#[test]
fn the_echo_of_our_own_save_does_not_count_as_an_outside_change() {
    let fx = Fixture::new("one\n");
    let mut writes = own_writes::OwnWrites::default();
    let now = Instant::now();
    writes.record("nb/Sec/A.md", &fx.page_file(), now);
    let soon = now + Duration::from_secs(1);
    assert!(writes.is_echo(Some("nb/Sec/A.md"), &fx.page_file(), soon));
    // A temporary sibling made by the atomic write.
    let temp = fx.notebook.path().join("Sec/.A.md.tmp123");
    assert!(writes.is_echo(None, &temp, soon));
    // Another file in the same folder is not ours.
    let other = fx.notebook.path().join("Sec/B.md");
    assert!(!writes.is_echo(None, &other, soon));
    // An outside edit changes the bytes, so it counts.
    std::fs::write(fx.page_file(), "outside\n").expect("write");
    assert!(!writes.is_echo(Some("nb/Sec/A.md"), &fx.page_file(), soon));
    // And the window ends.
    std::fs::write(fx.page_file(), "one\n").expect("write");
    let late = now + own_writes::OWN_WRITE_WINDOW + Duration::from_secs(1);
    assert!(!writes.is_echo(Some("nb/Sec/A.md"), &fx.page_file(), late));
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
        session.edit_page_text(&id, 0, 0, "un", false);
        let now = Instant::now();
        io.pump(&mut session, now, true);
        // A second edit keeps the page from sitting idle when the journal falls due.
        session.edit_page_text(&id, 2, 0, "saved ", false);
        io.pump(&mut session, now + Duration::from_millis(1400), true);
        io.pump(&mut session, now + JOURNAL_EVERY, true);
        assert!(session.is_page_dirty(&id), "the quiet time has not passed");
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
