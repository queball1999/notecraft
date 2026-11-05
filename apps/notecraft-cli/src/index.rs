//! `index`: bring a notebook's `.notecraft/index.sqlite` up to date, or build it again.

use std::process::ExitCode;

use notecraft_engine::search::index_notebook;

use crate::notebook::{self, folder_arg, Failure};
use crate::{bad_args, code, fail, out, Globals};

/// `index [--rebuild] [<notebook folder>]`: run the full refresh plus the text, link, and tag
/// passes the app runs, then print what the index holds.
pub fn index(globals: &Globals, rest: &[String]) -> ExitCode {
    let mut rebuild = false;
    let mut folder = None;
    for arg in rest {
        match arg.as_str() {
            "--rebuild" => rebuild = true,
            other if other.starts_with('-') => {
                return bad_args(&format!("unknown option `{other}` for index"));
            }
            other => {
                if folder.replace(other).is_some() {
                    return bad_args("index takes at most one notebook folder");
                }
            }
        }
    }
    let explicit = match folder_arg(globals.notebook.as_deref(), folder) {
        Ok(explicit) => explicit,
        Err(failure) => return fail(globals, &failure),
    };
    let notebook = match notebook::open(explicit.as_deref()) {
        Ok(notebook) => notebook,
        Err(failure) => return fail(globals, &failure),
    };
    let summary = match index_notebook(&notebook.root, rebuild) {
        Ok(summary) => summary,
        Err(message) => {
            return fail(
                globals,
                &Failure::new(code::ERROR, format!("cannot update the index: {message}")),
            )
        }
    };
    if globals.json {
        out(&serde_json::json!({
            "ok": true,
            "rebuilt": summary.rebuilt,
            "pages": summary.pages,
            "indexed_text": summary.text_built,
            "links": summary.links,
            "unresolved_links": summary.unresolved_links,
            "tagged_items": summary.tagged_items,
            "tags": summary.tags,
        })
        .to_string());
    } else if !globals.quiet {
        out(&format!(
            "{} the index of {}: {} pages, {} links ({} unresolved), {} tagged items in {} tags.",
            if summary.rebuilt {
                "Rebuilt"
            } else {
                "Updated"
            },
            notebook.tree.name,
            summary.pages,
            summary.links,
            summary.unresolved_links,
            summary.tagged_items,
            summary.tags
        ));
    }
    ExitCode::SUCCESS
}
