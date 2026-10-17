//! `text <page>`: a page's plain text.

use std::process::ExitCode;

use notecraft_markdown::{apply_edits, parse, remove_container_markers};
use notecraft_storage::{
    find_canvas_block, read_page_file, PageReadError, PageText, MAX_PAGE_BYTES,
};

use crate::notebook::{self, Failure, Notebook, PageInfo};
use crate::{bad_args, code, err, fail, out, Globals};

/// Read a page file through the storage layer.
pub fn read_page(notebook: &Notebook, page: &PageInfo<'_>) -> Result<PageText, Failure> {
    read_page_file(&notebook.root.join(&page.entry.rel_path)).map_err(|e| match e {
        PageReadError::NotFound | PageReadError::NotAFile => Failure::new(
            code::NOT_FOUND,
            format!("page file not found: {}", page.entry.rel_path),
        ),
        PageReadError::Io(e) => Failure::new(
            code::ERROR,
            format!("cannot read {}: {e}", page.entry.rel_path),
        ),
    })
}

/// The page's text without markdown syntax, its frontmatter, or its canvas block.
pub fn plain_text(source: &str) -> String {
    let visible = match find_canvas_block(source) {
        Some(block) => source.get(..block.block.start).unwrap_or(source),
        None => source,
    };
    // Container markers are layout, not text.
    let without_markers = apply_edits(visible, &remove_container_markers(visible, visible.len()))
        .unwrap_or_else(|_| visible.to_owned());
    parse(&without_markers).plain_text_up_to(usize::try_from(MAX_PAGE_BYTES).unwrap_or(usize::MAX))
}

/// `text <page>`: print the plain text of a page. `<page>` is a page id, a path relative to
/// the notebook, or a page link target. The text is printed even with `--quiet`.
pub fn text(globals: &Globals, rest: &[String]) -> ExitCode {
    let mut query = None;
    for arg in rest {
        if arg.starts_with("--") {
            return bad_args(&format!("unknown option `{arg}` for text"));
        }
        if query.replace(arg.as_str()).is_some() {
            return bad_args("text takes one <page>");
        }
    }
    let Some(query) = query else {
        return bad_args("text needs a <page>");
    };
    let notebook = match notebook::open(globals.notebook.as_deref()) {
        Ok(notebook) => notebook,
        Err(failure) => return fail(globals, &failure),
    };
    let pages = notebook.pages();
    let page = match notebook::resolve(&pages, query) {
        Ok(page) => page,
        Err(failure) => return fail(globals, &failure),
    };
    let read = match read_page(&notebook, page) {
        Ok(read) => read,
        Err(failure) => return fail(globals, &failure),
    };
    let text = plain_text(&read.text);
    let warning = read.warning();
    if globals.json {
        out(&serde_json::json!({
            "page": page.id,
            "path": page.entry.rel_path,
            "text": text,
            "warning": warning,
        })
        .to_string());
    } else {
        if let (Some(warning), false) = (&warning, globals.quiet) {
            err(&format!("notecraft-cli: {warning}"));
        }
        let _ = std::io::Write::write_all(&mut std::io::stdout(), text.as_bytes());
    }
    ExitCode::SUCCESS
}
