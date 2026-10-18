//! Picture and file commands: `insert.picture`, `insert.file_attachment`, `edit.paste_picture`,
//! and `attachment.open`. The insert commands take a `path` that is optional in the schema so a
//! ribbon button can send them bare; the front end then asks with a file picker (or reads the
//! clipboard for `edit.paste_picture`) and sends the command again with the path. Run without
//! a path (from the control channel), they fail with `invalid_args`.
//!
//! `attachment.open` only checks and resolves the file and returns its path; the front end
//! hands the path to the operating system, because the engine never calls the OS.

use std::path::Path;

use notecraft_markdown::image;
use notecraft_storage::attachments::{is_runnable_name, LINK_THRESHOLD_BYTES};
use serde_json::json;

use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::session::{AttachKind, Session};

fn invalid(id: CommandId, reason: impl Into<String>) -> CommandError {
    CommandError::InvalidArgs {
        id,
        reason: reason.into(),
    }
}

fn string_arg<'a>(
    id: CommandId,
    args: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, CommandError> {
    let value = args
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(id, format!("{name} is required")))?;
    if value.is_empty() || value.chars().count() > 4096 || value.chars().any(char::is_control) {
        return Err(invalid(
            id,
            format!("{name} must be 1 to 4096 characters with no control characters"),
        ));
    }
    Ok(value)
}

fn insert(
    id: CommandId,
    kind: AttachKind,
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let path = string_arg(id, args, "path")?;
    // A pasted picture's file is temporary, so it is always copied, never linked in place.
    let link_over = if id == ids::EDIT_PASTE_PICTURE {
        u64::MAX
    } else {
        LINK_THRESHOLD_BYTES
    };
    match session.insert_attachment_with(Path::new(path), kind, link_over) {
        // The commit marked the session changed.
        Ok(done) => Ok(CommandOutput {
            value: json!({ "link": done.link, "name": done.name, "linked": done.linked }),
        }),
        Err(err) => Err(invalid(id, err.0)),
    }
}

/// `insert.picture { path }`: copy a PNG, JPEG, WebP, or SVG file next to the page and show it on
/// the page. Returns `{ "link", "name", "linked" }`.
pub(super) fn insert_picture(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    insert(ids::INSERT_PICTURE, AttachKind::Picture, session, args)
}

/// `insert.file_attachment { path }`: copy any file next to the page and show it as an icon
/// with its name. Returns `{ "link", "name", "linked" }`.
pub(super) fn insert_file(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    insert(ids::INSERT_FILE_ATTACHMENT, AttachKind::File, session, args)
}

/// `edit.paste_picture { path }`: insert the picture the front end took from the clipboard
/// (saved as a PNG at `path`). Same as `insert.picture`, with its own undo label source.
pub(super) fn paste_picture(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    insert(ids::EDIT_PASTE_PICTURE, AttachKind::Picture, session, args)
}

/// `picture.resize { src, width, nth? }`: set the display width of a picture on the page, in
/// page units (16 to 4096; 0 means the natural size again). The height follows the picture's
/// own proportions. One undo step, and only a small comment after the image changes
/// (docs/markdown.md section 2.4). Returns `{ "width" }` (null for the natural size).
pub(super) fn resize_picture(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::PICTURE_RESIZE;
    let src = string_arg(id, args, "src")?;
    let width = args
        .get("width")
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid(id, "width is required"))?;
    if !(0..=i64::from(image::MAX_WIDTH)).contains(&width) {
        return Err(invalid(id, "width must be 0 or 16 to 4096"));
    }
    let nth = match args.get("nth") {
        None | Some(Value::Null) => 0,
        Some(value) => value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n < 100_000)
            .ok_or_else(|| invalid(id, "nth must be a small non-negative number"))?,
    };
    let width = match u16::try_from(width) {
        Ok(0) | Err(_) => None,
        Ok(w) => Some(w),
    };
    session
        .resize_picture(src, nth, width)
        .map_err(|err| invalid(id, err.0))?;
    Ok(CommandOutput {
        value: json!({ "width": width }),
    })
}

/// `attachment.open { src }`: resolve the link `src` of the page shown to a file on disk, for
/// the front end to open in the default app. Fails if the link is not a file link, the file is
/// missing, or opening it could run code (programs and scripts). Returns `{ "path" }`.
pub(super) fn open_attachment(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::ATTACHMENT_OPEN;
    let src = string_arg(id, args, "src")?;
    let Some(path) = session.attachment_path(src) else {
        return Err(invalid(
            id,
            "that link does not point to a file in the notebook",
        ));
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !path.is_file() {
        return Err(invalid(id, format!("{name} is missing")));
    }
    if is_runnable_name(&name) {
        return Err(invalid(
            id,
            format!("{name} is a program, so it is not opened from a note"),
        ));
    }
    Ok(CommandOutput {
        value: json!({ "path": path.to_string_lossy() }),
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::{json, Value};

    use crate::cmd::{CommandError, Registry};
    use crate::open::open_folder;
    use crate::session::Session;

    fn setup() -> (tempfile::TempDir, Session) {
        let dir = tempfile::TempDir::new().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("Sec")).expect("mkdir");
        std::fs::write(dir.path().join("Sec/A.md"), "# A\n").expect("page");
        std::fs::write(dir.path().join("pic.png"), b"png").expect("pic");
        std::fs::write(dir.path().join("doc.pdf"), b"pdf").expect("doc");
        std::fs::write(dir.path().join("tool.exe"), b"exe").expect("exe");
        let mut session = Session::new();
        let opened = open_folder(dir.path()).expect("scan");
        let id = opened.notebook.id.clone();
        session.open_notebook(opened.notebook);
        session.set_notebook_root(&id, dir.path());
        session.load_current_page();
        (dir, session)
    }

    fn run(
        session: &mut Session,
        id: &str,
        args: serde_json::Value,
    ) -> Result<crate::cmd::CommandOutput, CommandError> {
        Registry::builtin().run(session, id, args)
    }

    fn text(session: &Session) -> String {
        session
            .current_page_source()
            .map(|s| s.text.to_owned())
            .unwrap_or_default()
    }

    #[test]
    fn insert_picture_adds_an_image_and_undoes() {
        let (dir, mut session) = setup();
        let path = dir.path().join("pic.png").to_string_lossy().into_owned();
        let out = run(&mut session, "insert.picture", json!({ "path": path })).expect("insert");
        let link = out.value["link"].as_str().expect("link").to_owned();
        assert!(text(&session).contains(&format!("![pic]({link})")));
        assert!(session.can_undo());
        run(&mut session, "edit.undo", Value::Null).expect("undo");
        assert_eq!(text(&session), "# A\n");
        run(&mut session, "edit.redo", Value::Null).expect("redo");
        assert!(text(&session).contains(&format!("![pic]({link})")));
    }

    #[test]
    fn insert_file_and_paste_picture_add_links() {
        let (dir, mut session) = setup();
        let doc = dir.path().join("doc.pdf").to_string_lossy().into_owned();
        run(
            &mut session,
            "insert.file_attachment",
            json!({ "path": doc }),
        )
        .expect("file");
        assert!(text(&session).contains("[doc.pdf](attachments/"));
        let pic = dir.path().join("pic.png").to_string_lossy().into_owned();
        run(&mut session, "edit.paste_picture", json!({ "path": pic })).expect("paste");
        assert!(text(&session).contains("![pic](attachments/"));
        // Two inserts, two steps.
        assert!(session.undo().is_some());
        assert!(session.undo().is_some());
        assert!(!session.can_undo());
        assert_eq!(text(&session), "# A\n");
    }

    #[test]
    fn resize_picture_writes_a_small_comment_and_undoes() {
        let (dir, mut session) = setup();
        let path = dir.path().join("pic.png").to_string_lossy().into_owned();
        let out = run(&mut session, "insert.picture", json!({ "path": path })).expect("insert");
        let link = out.value["link"].as_str().expect("link").to_owned();
        let before = text(&session);
        let out = run(
            &mut session,
            "picture.resize",
            json!({ "src": link, "width": 320 }),
        )
        .expect("resize");
        assert_eq!(out.value["width"], 320);
        assert_eq!(
            text(&session),
            before.replacen(
                &format!("![pic]({link})"),
                &format!("![pic]({link})<!-- notecraft:width=320 -->"),
                1
            )
        );
        // A second resize replaces the comment; the same width again changes nothing.
        run(
            &mut session,
            "picture.resize",
            json!({ "src": link, "width": 500 }),
        )
        .expect("again");
        assert!(text(&session).contains("width=500 -->") && !text(&session).contains("320"));
        let steps = session
            .history()
            .undo_label(session.selection().page.as_deref());
        assert_eq!(steps, Some("Resize picture"));
        run(
            &mut session,
            "picture.resize",
            json!({ "src": link, "width": 500 }),
        )
        .expect("same");
        assert!(session.undo().is_some());
        assert!(text(&session).contains("width=320 -->"));
        assert!(session.undo().is_some());
        assert_eq!(text(&session), before);
        run(&mut session, "edit.redo", Value::Null).expect("redo");
        assert!(text(&session).contains("width=320 -->"));
        // Width 0 goes back to the natural size.
        run(
            &mut session,
            "picture.resize",
            json!({ "src": link, "width": 0 }),
        )
        .expect("reset");
        assert_eq!(text(&session), before);
    }

    #[test]
    fn resize_picture_rejects_bad_arguments() {
        let (_dir, mut session) = setup();
        for args in [
            json!({}),
            json!({ "src": "attachments/none.png", "width": 100 }),
            json!({ "src": "x.png", "width": 5000 }),
            json!({ "src": "x.png", "width": -1 }),
            json!({ "src": "x.png", "width": 100, "nth": -1 }),
            json!({ "width": 100 }),
        ] {
            let err = run(&mut session, "picture.resize", args.clone()).expect_err("fails");
            assert_eq!(err.code(), "invalid_args", "{args}");
        }
        assert!(!session.can_undo());
    }

    #[test]
    fn bad_arguments_are_invalid_args() {
        let (dir, mut session) = setup();
        let notes = dir.path().join("Sec/A.md").to_string_lossy().into_owned();
        for (id, args) in [
            ("insert.picture", json!({})),
            ("insert.picture", json!({ "path": "" })),
            ("insert.picture", json!({ "path": notes })),
            ("insert.file_attachment", json!({ "path": "nope.bin" })),
            ("attachment.open", json!({})),
            ("attachment.open", json!({ "src": "https://example.org/x" })),
            (
                "attachment.open",
                json!({ "src": "attachments/missing.pdf" }),
            ),
        ] {
            let err = run(&mut session, id, args.clone()).expect_err("fails");
            assert_eq!(err.code(), "invalid_args", "{id} {args}");
        }
        assert_eq!(text(&session), "# A\n");
        assert!(!session.can_undo());
    }

    #[test]
    fn open_attachment_resolves_a_file_and_refuses_programs() {
        let (dir, mut session) = setup();
        let doc = dir.path().join("doc.pdf").to_string_lossy().into_owned();
        let out = run(
            &mut session,
            "insert.file_attachment",
            json!({ "path": doc }),
        )
        .expect("file");
        let link = out.value["link"].as_str().expect("link").to_owned();
        let opened = run(&mut session, "attachment.open", json!({ "src": link })).expect("open");
        let path = opened.value["path"].as_str().expect("path");
        assert!(
            Path::new(path).is_file() && path.contains("attachments"),
            "{path}"
        );
        // A program is refused even when it is stored in the notebook.
        let exe = dir.path().join("tool.exe").to_string_lossy().into_owned();
        let out = run(
            &mut session,
            "insert.file_attachment",
            json!({ "path": exe }),
        )
        .expect("exe");
        let link = out.value["link"].as_str().expect("link").to_owned();
        let err =
            run(&mut session, "attachment.open", json!({ "src": link })).expect_err("blocked");
        assert!(err.to_string().contains("is a program"), "{err}");
    }
}
