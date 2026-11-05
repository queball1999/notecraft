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

/// Make saving the page fail: a folder takes the place of its file. (A file that is simply gone
/// is not a failed save: it raises the "moved or deleted" question.)
fn break_saving(fx: &Fixture) {
    std::fs::remove_file(fx.page_file()).expect("remove");
    std::fs::create_dir(fx.page_file()).expect("mkdir");
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

    // Once the file is back the page saves and the error goes away.
    std::fs::remove_dir(fx.page_file()).expect("remove");
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
fn a_save_that_changes_the_title_is_not_an_echo() {
    let fx = Fixture::new("---\ntitle: One\n---\nBody\n");
    let mut writes = own_writes::OwnWrites::default();
    let now = Instant::now();
    writes.record("p", &fx.page_file(), now);
    writes.record("p", &fx.page_file(), now);
    assert!(writes.is_echo(Some("p"), &fx.page_file(), now));
    std::fs::write(fx.page_file(), "---\ntitle: Two\n---\nBody\n").expect("write");
    writes.record("p", &fx.page_file(), now);
    assert!(!writes.is_echo(Some("p"), &fx.page_file(), now));
    let temp = fx.notebook.path().join("Sec/.A.md.tmp9");
    assert!(!writes.is_echo(None, &temp, now));
}

#[test]
fn a_save_that_changes_links_or_aliases_is_not_an_echo() {
    let fx = Fixture::new("Body\n");
    let mut writes = own_writes::OwnWrites::default();
    let now = Instant::now();
    writes.record("p", &fx.page_file(), now);
    writes.record("p", &fx.page_file(), now);
    assert!(writes.is_echo(Some("p"), &fx.page_file(), now));
    // A new link must reach the link index even though our own save wrote it.
    std::fs::write(fx.page_file(), "Body [[Other]]\n").expect("write");
    writes.record("p", &fx.page_file(), now);
    assert!(!writes.is_echo(Some("p"), &fx.page_file(), now));
    // The same links again: an echo.
    writes.record("p", &fx.page_file(), now);
    assert!(writes.is_echo(Some("p"), &fx.page_file(), now));
    // Text before the link moves its byte range, which the index stores.
    std::fs::write(fx.page_file(), "More Body [[Other]]\n").expect("write");
    writes.record("p", &fx.page_file(), now);
    assert!(!writes.is_echo(Some("p"), &fx.page_file(), now));
    // Aliases count too.
    writes.record("p", &fx.page_file(), now);
    std::fs::write(
        fx.page_file(),
        "---\naliases: [Nick]\n---\nMore Body [[Other]]\n",
    )
    .expect("write");
    writes.record("p", &fx.page_file(), now);
    assert!(!writes.is_echo(Some("p"), &fx.page_file(), now));
}

#[test]
fn links_typed_and_saved_reach_the_link_index() {
    let fx = Fixture::new("one\n");
    std::fs::write(fx.notebook.path().join("Sec/B.md"), "b\n").expect("write");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);
    settle(&mut io, &mut session, 600);
    let t = Instant::now();
    session.edit_page_text(&id, 0, 0, "[[B]] ", false);
    io.pump(&mut session, t, true);
    io.pump(&mut session, t + SAVE_AFTER_IDLE, true);
    assert!(!session.is_page_dirty(&id));
    let index_file = notecraft_storage::index_path(fx.notebook.path());
    let stored = |expected: usize| {
        notecraft_storage::index::Index::open(&index_file)
            .and_then(|index| index.all_links())
            .map(|links| links.len() == expected && links.iter().all(|l| l.resolved_page.is_some()))
            .unwrap_or(false)
    };
    let start = Instant::now();
    while !stored(1) {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "link not indexed"
        );
        io.pump(&mut session, Instant::now(), true);
        std::thread::sleep(Duration::from_millis(20));
    }
    // The canvas resolves it from the tree right away, with no index at all.
    assert_eq!(
        session.resolve_page_link("B").page.as_deref(),
        Some(id.replace("A.md", "B.md").as_str())
    );
    // Taking the link out again removes the row.
    let t = Instant::now();
    session.edit_page_text(&id, 0, 6, "", false);
    io.pump(&mut session, t, true);
    io.pump(&mut session, t + SAVE_AFTER_IDLE, true);
    let start = Instant::now();
    while !stored(0) {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "link not removed"
        );
        io.pump(&mut session, Instant::now(), true);
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Pump with the real clock for `ms` milliseconds so watcher events arrive and are handled.
fn settle(io: &mut SessionIo, session: &mut Session, ms: u64) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(ms) {
        io.pump(session, Instant::now(), true);
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn an_auto_save_that_keeps_the_title_does_not_start_an_index_refresh() {
    let fx = Fixture::new("---\ntitle: One\n---\nBody\n");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);
    settle(&mut io, &mut session, 600);
    let save = |io: &mut SessionIo, session: &mut Session, at: usize| {
        let t = Instant::now();
        session.edit_page_text(&id, at, 0, "x", false);
        io.pump(session, t, true);
        io.pump(session, t + SAVE_AFTER_IDLE, true);
        assert!(!session.is_page_dirty(&id));
    };
    // The first save of a page has no known title, so it may refresh the index.
    save(&mut io, &mut session, 22);
    settle(&mut io, &mut session, 800);
    let before = io.index_sync.started;
    save(&mut io, &mut session, 22);
    settle(&mut io, &mut session, 800);
    assert_eq!(io.index_sync.started, before, "no refresh for our own save");
    // An outside edit still does.
    std::fs::write(fx.page_file(), "---\ntitle: One\n---\nOutside\n").expect("write");
    settle(&mut io, &mut session, 800);
    assert!(io.index_sync.started > before, "outside edits refresh");
}

#[test]
fn the_navigation_shows_a_new_title_right_after_the_save() {
    let fx = Fixture::new("---\ntitle: One\n---\nBody\n");
    let mut session = fx.session();
    let mut io = fx.io();
    loaded(&mut io, &mut session);
    let id = page_id(&session);
    let shown = |s: &Session| {
        s.current_section()
            .and_then(|sec| sec.pages.iter().find(|p| p.id == id))
            .map(|p| p.title.clone())
    };
    pump_until(&mut io, &mut session, "first index", |s| {
        shown(s).as_deref() == Some("One")
    });
    // A body edit saves first, so the page's saved title is known.
    let t0 = Instant::now();
    session.edit_page_text(&id, 22, 0, "more ", false);
    io.pump(&mut session, t0, true);
    io.pump(&mut session, t0 + SAVE_AFTER_IDLE, true);
    assert!(!session.is_page_dirty(&id));
    // Let the watcher events of that save pass, then change the title.
    std::thread::sleep(Duration::from_millis(500));
    for _ in 0..10 {
        io.pump(&mut session, Instant::now(), true);
        std::thread::sleep(Duration::from_millis(30));
    }
    session.edit_page_text(&id, 11, 3, "Two", false);
    let t1 = Instant::now();
    io.pump(&mut session, t1, true);
    io.pump(&mut session, t1 + SAVE_AFTER_IDLE, true);
    assert!(!session.is_page_dirty(&id));
    assert!(fx.on_disk().contains("title: Two"));
    pump_until(&mut io, &mut session, "new title", |s| {
        shown(s).as_deref() == Some("Two")
    });
}

#[test]
fn the_echo_of_our_own_save_does_not_count_as_an_outside_change() {
    let fx = Fixture::new("one\n");
    let mut writes = own_writes::OwnWrites::default();
    let now = Instant::now();
    // The first save of a page has no known title yet, so it is not skipped; the second is.
    writes.record("nb/Sec/A.md", &fx.page_file(), now);
    assert!(!writes.is_echo(Some("nb/Sec/A.md"), &fx.page_file(), now));
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

#[test]
fn opening_a_notebook_prunes_dead_journal_files_but_keeps_real_entries() {
    let fx = Fixture::new("one\n");
    let dir = fx.journal.path().join("recovery");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("junk.json"), b"{not json").expect("junk");
    // A page whose file is gone keeps its entry (the "moved or deleted" question).
    let gone = fx.notebook.path().join("Sec/Gone.md");
    fx.journal().write(&gone, "unsaved\n").expect("journal");
    // An old-style name for the real page, stored by path.
    let stored = serde_json::json!({
        "version": 1,
        "path": fx.page_file().to_string_lossy(),
        "text": "old style\n",
    });
    std::fs::write(dir.join("0123456789abcdef.json"), stored.to_string()).expect("old");
    let mut session = fx.session();
    let mut io = fx.io();
    pump_until(&mut io, &mut session, "offer", |s| {
        !s.pending_recovery().is_empty()
    });
    assert!(!dir.join("junk.json").exists());
    assert_eq!(session.pending_recovery()[0].text, "old style\n");
    assert!(fx.journal().load_all().iter().any(|e| e.path == gone));
    // Discarding clears the old-style file by its stored path.
    session.discard_recovery();
    io.flush(&mut session);
    assert!(!dir.join("0123456789abcdef.json").exists());
    assert!(fx.journal().load_all().iter().any(|e| e.path == gone));
}
