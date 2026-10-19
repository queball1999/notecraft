//! `info`: what a notebook holds, from the folder reader.

use std::process::ExitCode;

use notecraft_storage::TreeNode;

use crate::{bad_args, fail, notebook, out, Globals};

fn count_groups(nodes: &[TreeNode]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            TreeNode::Section(_) => 0,
            TreeNode::Group(group) => 1 + count_groups(&group.children),
        })
        .sum()
}

/// `info`: the notebook's name, id, folder, format version, and section, group, and page
/// counts, plus anything the scan wanted to say.
pub fn info(globals: &Globals, rest: &[String]) -> ExitCode {
    if let Some(other) = rest.first() {
        return bad_args(&format!("unknown option `{other}` for info"));
    }
    let notebook = match notebook::open(globals.notebook.as_deref()) {
        Ok(notebook) => notebook,
        Err(failure) => return fail(globals, &failure),
    };
    let tree = &notebook.tree;
    let sections = tree.sections().len();
    let groups = count_groups(&tree.nodes);
    let pages = tree.page_count();
    let warnings: Vec<String> = tree.warnings.iter().map(ToString::to_string).collect();
    if globals.json {
        out(&serde_json::json!({
            "name": tree.name,
            "id": tree.id,
            "path": notebook.root.display().to_string(),
            "format": tree.config.format,
            "sections": sections,
            "groups": groups,
            "pages": pages,
            "warnings": warnings,
        })
        .to_string());
    } else if !globals.quiet {
        out(&format!("Notebook: {}", tree.name));
        out(&format!("Id: {}", tree.id));
        out(&format!("Path: {}", notebook.root.display()));
        match tree.config.format {
            Some(format) => out(&format!("Format version: {format}")),
            None => out("Format version: not set (notebook.toml is missing or has no format)"),
        }
        out(&format!("Sections: {sections}"));
        out(&format!("Section groups: {groups}"));
        out(&format!("Pages: {pages}"));
        for warning in &warnings {
            out(&format!("Warning: {warning}"));
        }
    }
    ExitCode::SUCCESS
}
