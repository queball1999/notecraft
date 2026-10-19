//! `check`: validate a notebook. Reports pages that cannot be read cleanly, canvas blocks
//! that cannot be parsed, page links that point nowhere, and attachments that are missing.

use std::collections::BTreeSet;
use std::process::ExitCode;

use notecraft_engine::cmd::Registry;
use notecraft_engine::open::notebook_from_tree;
use notecraft_engine::Session;
use notecraft_markdown::{parse, Block, BlockKind, Run};
use notecraft_storage::{
    find_canvas_block, read_canvas, read_page_file, PageReadError, WarningKind,
};

use crate::notebook::{self, resolve_page, Found, Notebook, PageInfo};
use crate::{bad_args, code, fail, out, Globals};

/// Most `[[...]]` links looked at on one page.
const MAX_LINKS_PER_PAGE: usize = 10_000;
/// Longest `[[...]]` target, in bytes.
const MAX_TARGET_BYTES: usize = 256;

/// One thing wrong with the notebook.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// `unreadable_page`, `bad_text`, `page_too_large`, `bad_canvas`, `broken_link`,
    /// `missing_attachment`, or `bad_settings`.
    pub kind: &'static str,
    /// The page (or file) it is about, relative to the notebook.
    pub path: String,
    pub message: String,
}

/// What `check` found.
#[derive(Debug, Default)]
pub struct Report {
    pub pages: usize,
    pub problems: Vec<Problem>,
    /// Things worth knowing that do not fail the check (ambiguous links, scan warnings).
    pub warnings: Vec<String>,
}

/// The targets of the `[[Page]]` and `[[Page|shown text]]` links in `text`. A pipe written
/// `\|` (the table-cell form) does not end the target.
pub fn link_targets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find("[[") {
        if out.len() >= MAX_LINKS_PER_PAGE {
            break;
        }
        let Some(after) = rest.get(open + 2..) else {
            break;
        };
        // Only a short stretch after the opener is searched, so a page of openers is not
        // quadratic.
        let mut end = (MAX_TARGET_BYTES + 2).min(after.len());
        while !after.is_char_boundary(end) {
            end -= 1;
        }
        let window = after.get(..end).unwrap_or("");
        let Some(close) = window.find("]]") else {
            rest = after;
            continue;
        };
        let inner = window.get(..close).unwrap_or("");
        if inner.contains("[[") {
            // A nested opener: the inner one is the link; start again just after this one.
            rest = after;
            continue;
        }
        if !inner.contains('\n') {
            let target = target_of(inner);
            if !target.is_empty() {
                out.push(target);
            }
        }
        rest = after.get(close + 2..).unwrap_or("");
    }
    out
}

fn target_of(inner: &str) -> String {
    let mut target = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                chars.next();
                target.push('|');
            }
            '|' => break,
            _ => target.push(c),
        }
    }
    target.trim().to_owned()
}

/// What a page links to: the text of each block outside code (for `[[...]]` page links) and
/// every markdown link or image target.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Links {
    pub texts: Vec<String>,
    pub targets: Vec<String>,
}

fn collect_runs(runs: &[Run], links: &mut Links) {
    let mut text = String::new();
    for run in runs {
        if !run.code {
            text.push_str(&run.text);
        }
        if let Some(target) = &run.link {
            if !links.targets.contains(target) {
                links.targets.push(target.clone());
            }
        }
    }
    links.texts.push(text);
}

/// Gather the links of `blocks` (Html and code blocks are skipped).
pub fn collect_links(blocks: &[Block], links: &mut Links) {
    for block in blocks {
        match &block.kind {
            BlockKind::Heading { runs, .. } | BlockKind::Paragraph { runs } => {
                collect_runs(runs, links);
            }
            BlockKind::List { items, .. } => {
                for item in items {
                    collect_links(&item.blocks, links);
                }
            }
            BlockKind::Table(table) => {
                for row in std::iter::once(&table.header).chain(&table.rows) {
                    for cell in row {
                        collect_runs(&cell.runs, links);
                    }
                }
            }
            BlockKind::Quote { blocks } => collect_links(blocks, links),
            BlockKind::Code { .. } | BlockKind::Html { .. } | BlockKind::Rule => {}
        }
    }
}

/// Check every page of the notebook.
pub fn check_notebook(notebook: &Notebook) -> Report {
    let mut report = Report::default();
    for warning in &notebook.tree.warnings {
        match warning.kind {
            WarningKind::BadConfig | WarningKind::Unreadable => report.problems.push(Problem {
                kind: "bad_settings",
                path: warning.path.clone(),
                message: warning.to_string(),
            }),
            _ => report.warnings.push(warning.to_string()),
        }
    }

    // A session over the notebook answers where an attachment link points on disk.
    let registry = Registry::builtin();
    let mut session = Session::new();
    session.open_notebook(notebook_from_tree(&notebook.tree));
    session.set_notebook_root(&notebook.tree.id, &notebook.root);

    let pages = notebook.pages();
    report.pages = pages.len();
    for page in &pages {
        check_page(
            notebook,
            &pages,
            page,
            &registry,
            &mut session,
            &mut report.problems,
            &mut report.warnings,
        );
    }
    report
}

fn check_page(
    notebook: &Notebook,
    pages: &[PageInfo<'_>],
    page: &PageInfo<'_>,
    registry: &Registry,
    session: &mut Session,
    problems: &mut Vec<Problem>,
    warnings: &mut Vec<String>,
) {
    let path = page.entry.rel_path.clone();
    let mut problem = |kind: &'static str, message: String| {
        problems.push(Problem {
            kind,
            path: path.clone(),
            message,
        });
    };
    let read = match read_page_file(&notebook.root.join(&page.entry.rel_path)) {
        Ok(read) => read,
        Err(PageReadError::NotFound | PageReadError::NotAFile) => {
            problem("unreadable_page", "the page file is missing".to_owned());
            return;
        }
        Err(PageReadError::Io(e)) => {
            problem("unreadable_page", format!("the page cannot be read: {e}"));
            return;
        }
    };
    if read.truncated {
        problem(
            "page_too_large",
            "the page is over the size limit".to_owned(),
        );
    }
    if read.lossy {
        problem(
            "bad_text",
            "the page has bytes that are not valid text".to_owned(),
        );
    }
    if let Some(issue) = read_canvas(&read.text).problem {
        problem("bad_canvas", issue.message());
    }

    let visible = match find_canvas_block(&read.text) {
        Some(block) => read.text.get(..block.block.start).unwrap_or(&read.text),
        None => &read.text,
    };
    let mut links = Links::default();
    collect_links(&parse(visible).blocks, &mut links);

    // Page links, `[[target]]`.
    let mut seen = BTreeSet::new();
    for text in &links.texts {
        for target in link_targets(text) {
            if !seen.insert(target.clone()) {
                continue;
            }
            match resolve_page(pages, &target, Some(page.section)) {
                Found::One(_) => {}
                Found::None => problem(
                    "broken_link",
                    format!("the link [[{target}]] points at no page"),
                ),
                Found::Many(list) => {
                    let names: Vec<&str> = list.iter().map(|p| p.entry.rel_path.as_str()).collect();
                    warnings.push(format!(
                        "{}: the link [[{target}]] matches more than one page: {}",
                        page.entry.rel_path,
                        names.join(", ")
                    ));
                }
            }
        }
    }

    // Files the page links to or shows.
    if registry
        .run(
            session,
            "nav.page.select",
            serde_json::json!({ "id": page.id }),
        )
        .is_err()
    {
        return;
    }
    for target in &links.targets {
        if let Some(file) = session.attachment_path(target) {
            if !file.exists() {
                problem(
                    "missing_attachment",
                    format!("the file {target} does not exist"),
                );
            }
        }
    }
}

/// `check`: print what is wrong with the notebook and exit 1 if anything is.
pub fn check(globals: &Globals, rest: &[String]) -> ExitCode {
    if let Some(other) = rest.first() {
        return bad_args(&format!("unknown option `{other}` for check"));
    }
    let notebook = match notebook::open(globals.notebook.as_deref()) {
        Ok(notebook) => notebook,
        Err(failure) => return fail(globals, &failure),
    };
    let report = check_notebook(&notebook);
    let ok = report.problems.is_empty();
    if globals.json {
        let problems: Vec<serde_json::Value> = report
            .problems
            .iter()
            .map(|p| serde_json::json!({ "kind": p.kind, "path": p.path, "message": p.message }))
            .collect();
        out(&serde_json::json!({
            "ok": ok,
            "pages": report.pages,
            "problems": problems,
            "warnings": report.warnings,
        })
        .to_string());
    } else {
        for problem in &report.problems {
            out(&format!(
                "{}: {}: {}",
                problem.path, problem.kind, problem.message
            ));
        }
        if !globals.quiet {
            for warning in &report.warnings {
                out(&format!("warning: {warning}"));
            }
            out(&format!(
                "Checked {} pages: {} problems, {} warnings.",
                report.pages,
                report.problems.len(),
                report.warnings.len()
            ));
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(code::ERROR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_targets_follow_the_page_link_forms() {
        assert_eq!(link_targets("See [[Plan]] now"), ["Plan"]);
        assert_eq!(
            link_targets("[[Plan|the plan]] and [[Journal/Monday]]"),
            ["Plan", "Journal/Monday"]
        );
        assert_eq!(link_targets(r"[[A\|B|shown]]"), ["A|B"]);
        assert_eq!(link_targets("[[ spaced ]]"), ["spaced"]);
        assert!(link_targets("[[]] [[ ]] [[unclosed").is_empty());
    }

    #[test]
    fn hostile_brackets_are_bounded() {
        let text = "[[".repeat(50_000);
        assert!(link_targets(&text).is_empty());
        let many = "[[a]]".repeat(MAX_LINKS_PER_PAGE + 100);
        assert_eq!(link_targets(&many).len(), MAX_LINKS_PER_PAGE);
        assert_eq!(link_targets("[[[[x]]"), ["x"]);
    }
}
