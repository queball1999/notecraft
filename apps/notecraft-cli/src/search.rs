//! `search`: ranked full-text hits, or tagged items, from the notebook index. It calls the
//! same `notecraft_engine::search::search` the desktop app does.

use std::collections::BTreeMap;
use std::process::ExitCode;

use notecraft_engine::search::{
    index_notebook, search, HitKind, SearchHit, SearchRoot, SearchScope, SearchSort, MAX_RESULTS,
};
use notecraft_engine::session::Selection;
use notecraft_storage::index::{Index, TagScope, TaggedRow};
use notecraft_storage::index_path;

use crate::notebook::{self, folder_arg, Failure, Notebook};
use crate::{bad_args, code, err, fail, out, Globals};

/// Hits printed when `--limit` is not given.
const DEFAULT_LIMIT: usize = 20;
/// Most `--tag` options accepted.
const MAX_TAGS: usize = 32;

/// What the command line asked for.
#[derive(Debug, Default, PartialEq, Eq)]
struct Request {
    query: String,
    tags: Vec<String>,
    limit: usize,
    scope: Option<String>,
    within: Option<String>,
    folder: Option<String>,
}

/// A note tag as typed: without the `#`, lowercase.
fn clean_tag(text: &str) -> String {
    text.trim().trim_start_matches('#').to_lowercase()
}

fn parse(rest: &[String]) -> Result<Request, String> {
    let mut request = Request {
        limit: DEFAULT_LIMIT,
        ..Request::default()
    };
    let mut positional: Vec<&str> = Vec::new();
    let mut args = rest.iter();
    while let Some(arg) = args.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) if name.starts_with("--") => (name, Some(value.to_owned())),
            _ => (arg.as_str(), None),
        };
        match name {
            "--tag" | "--limit" | "--scope" | "--in" => {
                let Some(value) = inline.or_else(|| args.next().cloned()) else {
                    return Err(format!("{name} needs a value"));
                };
                match name {
                    "--tag" => {
                        let tag = clean_tag(&value);
                        if tag.is_empty() {
                            return Err("--tag needs a tag name".to_owned());
                        }
                        if request.tags.len() >= MAX_TAGS {
                            return Err(format!("at most {MAX_TAGS} --tag options"));
                        }
                        if !request.tags.contains(&tag) {
                            request.tags.push(tag);
                        }
                    }
                    "--limit" => {
                        request.limit = match value.trim().parse::<usize>() {
                            Ok(n) if n >= 1 => n.min(MAX_RESULTS),
                            _ => {
                                return Err(format!(
                                    "--limit must be a whole number from 1 to {MAX_RESULTS}"
                                ))
                            }
                        };
                    }
                    "--scope" => request.scope = Some(value),
                    _ => request.within = Some(value),
                }
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown option `{other}` for search"));
            }
            _ => positional.push(arg),
        }
    }
    if positional.len() > 2 {
        return Err("search takes a query and at most one notebook folder".to_owned());
    }
    request.query = positional
        .first()
        .map(|q| (*q).to_owned())
        .unwrap_or_default();
    request.folder = positional.get(1).map(|f| (*f).to_owned());
    if request.query.trim().is_empty() && request.tags.is_empty() {
        return Err("search needs a query or at least one --tag".to_owned());
    }
    Ok(request)
}

/// The scope options, resolved against the notebook.
struct Where {
    scope: SearchScope,
    selection: Selection,
    tags: TagScope,
}

fn resolve_where(request: &Request, notebook: &Notebook) -> Result<Where, Failure> {
    let id = notebook.tree.id.clone();
    let scope = match request.scope.as_deref() {
        None => SearchScope::Notebook,
        Some(text) => SearchScope::from_id(text).ok_or_else(|| {
            Failure::new(
                code::BAD_ARGS,
                "--scope must be one of notebook, section, group, page",
            )
        })?,
    };
    let mut selection = Selection {
        notebook: Some(id.clone()),
        ..Selection::default()
    };
    let needs_in = matches!(
        scope,
        SearchScope::Page | SearchScope::Section | SearchScope::Group
    );
    let within = request
        .within
        .as_deref()
        .map(|w| w.trim().trim_matches('/'));
    if needs_in != within.is_some() {
        return Err(Failure::new(
            code::BAD_ARGS,
            if needs_in {
                "--scope page, section, or group needs --in <page or section path>"
            } else {
                "--in only goes with --scope page, section, or group"
            },
        ));
    }
    let mut tags = TagScope::Notebook;
    if let Some(within) = within {
        if scope == SearchScope::Page {
            let pages = notebook.pages();
            let page = notebook::resolve(&pages, within)?;
            selection.page = Some(page.id.clone());
            selection.section = Some(format!("{id}/{}", page.section));
            tags = TagScope::Page(page.id.clone());
        } else {
            let known = notebook
                .tree
                .sections()
                .iter()
                .any(|section| section.rel_path == within);
            if !known {
                return Err(Failure::new(
                    code::NOT_FOUND,
                    format!("no section `{within}` in the notebook"),
                ));
            }
            selection.section = Some(format!("{id}/{within}"));
            tags = if scope == SearchScope::Section {
                TagScope::Folder(within.to_owned())
            } else {
                // The group that holds the section; a top-level section means the notebook.
                within
                    .rsplit_once('/')
                    .map_or(TagScope::Notebook, |(group, _)| {
                        TagScope::Folder(group.to_owned())
                    })
            };
        }
    }
    Ok(Where {
        scope,
        selection,
        tags,
    })
}

fn fail_msg(globals: &Globals, code: u8, message: String) -> ExitCode {
    fail(globals, &Failure::new(code, message))
}

/// `search <query> [--tag <t>]... [--limit <n>] [--scope <s> --in <path>] [<folder>]`.
pub fn search_command(globals: &Globals, rest: &[String]) -> ExitCode {
    let request = match parse(rest) {
        Ok(request) => request,
        Err(message) => return bad_args(&message),
    };
    let explicit = match folder_arg(globals.notebook.as_deref(), request.folder.as_deref()) {
        Ok(explicit) => explicit,
        Err(failure) => return fail(globals, &failure),
    };
    let notebook = match notebook::open(explicit.as_deref()) {
        Ok(notebook) => notebook,
        Err(failure) => return fail(globals, &failure),
    };
    let place = match resolve_where(&request, &notebook) {
        Ok(place) => place,
        Err(failure) => return fail(globals, &failure),
    };
    // The index is brought up to date first, as the app's refresh would. If that fails but an
    // index exists (a read-only notebook), search what is there.
    if let Err(message) = index_notebook(&notebook.root, false) {
        if !index_path(&notebook.root).is_file() {
            return fail_msg(
                globals,
                code::ERROR,
                format!("cannot update the index: {message}"),
            );
        }
        err(&format!(
            "notecraft-cli: warning: the index was not updated: {message}"
        ));
    }
    if request.query.trim().is_empty() {
        tagged_only(globals, &request, &place, &notebook)
    } else {
        ranked(globals, &request, &place, &notebook)
    }
}

/// The tagged items in `scope` carrying any of `tags`, in notebook order.
fn tagged(
    notebook: &Notebook,
    scope: &TagScope,
    tags: &[String],
) -> Result<Vec<TaggedRow>, String> {
    let index = Index::open(&index_path(&notebook.root)).map_err(|e| e.to_string())?;
    index.tagged_items(scope, tags).map_err(|e| e.to_string())
}

fn ranked(globals: &Globals, request: &Request, place: &Where, notebook: &Notebook) -> ExitCode {
    let root = SearchRoot {
        id: notebook.tree.id.clone(),
        name: notebook.tree.name.clone(),
        path: notebook.root.clone(),
    };
    let results = search(
        std::slice::from_ref(&root),
        &place.selection,
        &request.query,
        place.scope,
        SearchSort::Relevance,
    );
    let mut truncated = results.truncated;
    // With --tag, only pages that carry one of the tags stay, with the tags they carry.
    let mut carried: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if !request.tags.is_empty() {
        match tagged(notebook, &TagScope::Notebook, &request.tags) {
            Ok(rows) => {
                for row in rows {
                    let list = carried.entry(row.page_id).or_default();
                    if !list.contains(&row.tag) {
                        list.push(row.tag);
                    }
                }
            }
            Err(message) => {
                return fail_msg(globals, code::ERROR, format!("cannot read tags: {message}"))
            }
        }
    }
    let mut hits: Vec<SearchHit> = results
        .hits
        .into_iter()
        .filter(|hit| {
            request.tags.is_empty()
                || (hit.kind == HitKind::Page && carried.contains_key(&hit.page_id))
        })
        .collect();
    if hits.len() > request.limit {
        hits.truncate(request.limit);
        truncated = true;
    }
    let prefix = format!("{}/", notebook.tree.id);
    let path_of = |hit: &SearchHit| match &hit.attachment {
        Some(file) => file.path.clone(),
        None => hit
            .page_id
            .strip_prefix(&prefix)
            .unwrap_or(&hit.page_id)
            .to_owned(),
    };
    if globals.json {
        let list: Vec<serde_json::Value> = hits
            .iter()
            .map(|hit| {
                let mut value = serde_json::json!({
                    "kind": if hit.kind == HitKind::Page { "page" } else { "attachment" },
                    "path": path_of(hit),
                    "page_id": hit.page_id,
                    "title": hit.title,
                    "section": hit.section_path,
                    "rank": hit.rank,
                    "modified": hit.modified,
                    "snippet": hit.snippet,
                    "highlights": hit.highlights.iter().map(|(a, b)| [a, b]).collect::<Vec<_>>(),
                });
                if let (Some(tags), Some(map)) = (carried.get(&hit.page_id), value.as_object_mut())
                {
                    map.insert("tags".to_owned(), serde_json::json!(tags));
                }
                value
            })
            .collect();
        out(&serde_json::json!({
            "ok": true,
            "query": request.query,
            "tags": request.tags,
            "scope": place.scope.id(),
            "count": list.len(),
            "truncated": truncated,
            "hits": list,
        })
        .to_string());
    } else {
        for hit in &hits {
            out(&format!("{}  {}", path_of(hit), hit.title));
            if !hit.snippet.trim().is_empty() {
                out(&format!("    {}", hit.snippet.trim()));
            }
        }
        if hits.is_empty() && !globals.quiet {
            out("No matches.");
        }
        if truncated {
            err(&format!(
                "notecraft-cli: more pages matched; showing the first {}",
                hits.len()
            ));
        }
    }
    ExitCode::SUCCESS
}

fn tagged_only(
    globals: &Globals,
    request: &Request,
    place: &Where,
    notebook: &Notebook,
) -> ExitCode {
    let mut rows = match tagged(notebook, &place.tags, &request.tags) {
        Ok(rows) => rows,
        Err(message) => {
            return fail_msg(globals, code::ERROR, format!("cannot read tags: {message}"))
        }
    };
    let truncated = rows.len() > request.limit;
    rows.truncate(request.limit);
    if globals.json {
        let list: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "path": row.page_path,
                    "page_id": row.page_id,
                    "title": row.page_title,
                    "section": row.section,
                    "tag": row.tag,
                    "text": row.text,
                    "checked": row.checked,
                    "modified": row.modified,
                })
            })
            .collect();
        out(&serde_json::json!({
            "ok": true,
            "tags": request.tags,
            "scope": place.scope.id(),
            "count": list.len(),
            "truncated": truncated,
            "items": list,
        })
        .to_string());
    } else {
        for row in &rows {
            let mark = match row.checked {
                Some(true) => " [x]",
                Some(false) => " [ ]",
                None => "",
            };
            out(&format!(
                "{}  {}  #{}{} {}",
                row.page_path,
                row.page_title,
                row.tag,
                mark,
                row.text.trim()
            ));
        }
        if rows.is_empty() && !globals.quiet {
            out("No tagged items.");
        }
        if truncated {
            err(&format!(
                "notecraft-cli: more items matched; showing the first {}",
                rows.len()
            ));
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn options_are_parsed() {
        let r = parse(&args(&[
            "budget",
            "--tag",
            "#Todo",
            "--tag=idea",
            "--limit",
            "5",
            "/nb",
        ]))
        .unwrap_or_default();
        assert_eq!(r.query, "budget");
        assert_eq!(r.tags, ["todo", "idea"]);
        assert_eq!(r.limit, 5);
        assert_eq!(r.folder.as_deref(), Some("/nb"));
        let r = parse(&args(&["--tag", "todo"])).unwrap_or_default();
        assert!(r.query.is_empty());
        assert_eq!(r.limit, DEFAULT_LIMIT);
        let r = parse(&args(&["x", "--limit", "99999"])).unwrap_or_default();
        assert_eq!(r.limit, MAX_RESULTS);
    }

    #[test]
    fn bad_options_are_errors() {
        for bad in [
            &["--limit", "3"][..],
            &["x", "--limit", "0"],
            &["x", "--limit", "many"],
            &["x", "--tag"],
            &["x", "--tag", "#"],
            &["x", "--nope"],
            &["a", "b", "c"],
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad:?}");
        }
    }
}
