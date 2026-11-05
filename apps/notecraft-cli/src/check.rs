//! `check`: validate a notebook. Reports pages that cannot be read cleanly, canvas blocks
//! that cannot be parsed, page links that point nowhere, and attachments that are missing.

use std::collections::BTreeSet;
use std::process::ExitCode;

use notecraft_engine::cmd::Registry;
use notecraft_engine::open::notebook_from_tree;
use notecraft_engine::Session;
use notecraft_markdown::{
    page_links_of, parse, parse_frontmatter, Block, BlockKind, LinkIndex, LinkPage, Resolved, Run,
};
use notecraft_storage::{
    find_canvas_block, read_canvas, read_page_file, PageEncoding, PageReadError, WarningKind,
};

use crate::notebook::{self, Notebook, PageInfo};
use crate::{bad_args, code, fail, out, Globals};

/// Most of a page file read to find its frontmatter aliases.
const ALIAS_READ_LIMIT: u64 = 64 * 1024;

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

/// Where a page links to besides other pages: every markdown link or image target.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Links {
    pub targets: Vec<String>,
}

fn collect_runs(runs: &[Run], links: &mut Links) {
    for run in runs {
        if let Some(target) = &run.link {
            if !links.targets.contains(target) {
                links.targets.push(target.clone());
            }
        }
    }
}

/// Gather the file links of `blocks` (Html and code blocks are skipped).
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

/// The frontmatter `aliases` of the page file at `path` (none if it cannot be read).
fn aliases_of(path: &std::path::Path) -> Vec<String> {
    use std::io::Read;

    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let mut bytes = Vec::new();
    if file.take(ALIAS_READ_LIMIT).read_to_end(&mut bytes).is_err() {
        return Vec::new();
    }
    parse_frontmatter(&String::from_utf8_lossy(&bytes))
        .map(|front| {
            front
                .aliases()
                .into_iter()
                .map(|alias| alias.trim().to_owned())
                .filter(|alias| !alias.is_empty())
                .collect()
        })
        .unwrap_or_default()
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
    // Page links resolve the way the app resolves them (docs/markdown.md section 5.3).
    let resolver = LinkIndex::new(
        pages
            .iter()
            .map(|page| {
                LinkPage::new(page.id.clone(), page.entry.rel_path.clone())
                    .with_aliases(aliases_of(&notebook.root.join(&page.entry.rel_path)))
            })
            .collect::<Vec<_>>(),
    );
    for page in &pages {
        check_page(
            notebook,
            &resolver,
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
    resolver: &LinkIndex,
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
    if read.encoding != PageEncoding::Utf8 {
        problem("bad_text", "the page is not saved as UTF-8".to_owned());
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

    // Page links, `[[target]]`: one answer per target and page.
    let mut seen = BTreeSet::new();
    for link in page_links_of(visible) {
        if !seen.insert(link.target.clone()) {
            continue;
        }
        let target = &link.target;
        match resolver.resolve(target, &page.entry.rel_path) {
            Resolved::Page(_) => {}
            Resolved::Unresolved => problem(
                "broken_link",
                format!("the link [[{target}]] points at no page"),
            ),
            Resolved::Ambiguous { candidates, .. } => {
                let names: Vec<&str> = candidates.iter().map(|p| p.path.as_str()).collect();
                warnings.push(format!(
                    "{}: the link [[{target}]] matches more than one page: {}",
                    page.entry.rel_path,
                    names.join(", ")
                ));
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
