//! Search commands: `search.open`, `search.all`, `search.run`, `search.set_scope`,
//! `search.set_sort`, `search.close` (build_plan/04 PR 2.4). The session keeps the query,
//! scope, sort, and last results; the front end shows them and opens a hit with
//! `nav.page.select`.

use serde_json::json;

use super::theme::{invalid, string_arg};
use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::search::{HitKind, SearchResults, SearchRoot, SearchScope, SearchSort};
use crate::session::Session;

/// Longest query kept, in characters.
const MAX_QUERY_CHARS: usize = 512;

fn open_box(session: &mut Session) {
    session.tag_summary_mut().open = false;
    let state = session.search_mut();
    state.open = true;
    state.open_serial = state.open_serial.wrapping_add(1);
    session.mark_changed();
}

/// The open notebooks that were read from a folder.
fn roots_of(session: &Session) -> Vec<SearchRoot> {
    session
        .notebooks()
        .iter()
        .filter_map(|nb| {
            let path = session
                .notebook_roots()
                .find_map(|(id, path)| (id == nb.id).then(|| path.to_path_buf()))?;
            Some(SearchRoot {
                id: nb.id.clone(),
                name: nb.name.clone(),
                path,
            })
        })
        .collect()
}

/// Run the search the state describes and keep the results.
fn rerun(session: &mut Session) {
    let roots = roots_of(session);
    let (query, scope, sort) = {
        let state = session.search();
        (state.query.clone(), state.scope, state.sort)
    };
    #[cfg(not(target_arch = "wasm32"))]
    let results = crate::search::search(&roots, session.selection(), &query, scope, sort);
    #[cfg(target_arch = "wasm32")]
    let results = {
        let _ = &roots;
        SearchResults {
            query,
            scope,
            sort,
            ..SearchResults::default()
        }
    };
    session.search_mut().results = Some(results);
    session.mark_changed();
}

fn summary(results: &SearchResults) -> Value {
    let hits: Vec<Value> = results
        .hits
        .iter()
        .map(|hit| {
            json!({
                "kind": match hit.kind {
                    HitKind::Page => "page",
                    HitKind::Attachment => "attachment",
                },
                "attachment": hit.attachment.as_ref().map(|a| json!({
                    "path": a.path,
                    "mime": a.mime,
                    "size": a.size,
                })),
                "notebook": hit.notebook_id,
                "section": hit.section_id,
                "page": hit.page_id,
                "title": hit.title,
                "modified": hit.modified,
                "snippet": hit.snippet,
            })
        })
        .collect();
    json!({
        "query": results.query,
        "scope": results.scope.id(),
        "sort": results.sort.id(),
        "count": results.hits.len(),
        "truncated": results.truncated,
        "hits": hits,
    })
}

fn scope_arg(id: CommandId, args: &Map<String, Value>) -> Result<SearchScope, CommandError> {
    let text = string_arg(args, "scope");
    SearchScope::from_id(text).ok_or_else(|| {
        let expected = SearchScope::LIST.map(SearchScope::id);
        invalid(id, "scope", text, &expected)
    })
}

fn sort_arg(id: CommandId, args: &Map<String, Value>) -> Result<SearchSort, CommandError> {
    let text = string_arg(args, "sort");
    SearchSort::from_id(text).ok_or_else(|| {
        let expected = SearchSort::LIST.map(SearchSort::id);
        invalid(id, "sort", text, &expected)
    })
}

/// `search.open`: show the search box, ready to type in.
pub(super) fn open(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    open_box(session);
    Ok(CommandOutput::default())
}

/// `search.all`: show the search box and look in every open notebook.
pub(super) fn all(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    session.search_mut().scope = SearchScope::All;
    open_box(session);
    if !session.search().query.trim().is_empty() {
        rerun(session);
    }
    Ok(CommandOutput::default())
}

/// `search.run { query, scope?, sort? }`: search and keep the results. Returns them as JSON.
pub(super) fn run(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let scope = if args.contains_key("scope") {
        Some(scope_arg(ids::SEARCH_RUN, args)?)
    } else {
        None
    };
    let sort = if args.contains_key("sort") {
        Some(sort_arg(ids::SEARCH_RUN, args)?)
    } else {
        None
    };
    let query: String = string_arg(args, "query")
        .chars()
        .take(MAX_QUERY_CHARS)
        .collect();
    let state = session.search_mut();
    state.query = query;
    if let Some(scope) = scope {
        state.scope = scope;
    }
    if let Some(sort) = sort {
        state.sort = sort;
    }
    state.open = true;
    rerun(session);
    let value = session
        .search()
        .results
        .as_ref()
        .map(summary)
        .unwrap_or_default();
    Ok(CommandOutput { value })
}

/// `search.set_scope { scope }`
pub(super) fn set_scope(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let scope = scope_arg(ids::SEARCH_SET_SCOPE, args)?;
    if session.search().scope != scope {
        session.search_mut().scope = scope;
        if session.search().results.is_some() {
            rerun(session);
        }
        session.mark_changed();
    }
    Ok(CommandOutput {
        value: json!(scope.id()),
    })
}

/// `search.set_sort { sort }`
pub(super) fn set_sort(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let sort = sort_arg(ids::SEARCH_SET_SORT, args)?;
    if session.search().sort != sort {
        session.search_mut().sort = sort;
        if session.search().results.is_some() {
            rerun(session);
        }
        session.mark_changed();
    }
    Ok(CommandOutput {
        value: json!(sort.id()),
    })
}

/// `search.close`: hide the search box and results. The query is kept for next time.
pub(super) fn close(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    if session.search().open {
        session.search_mut().open = false;
        session.mark_changed();
    }
    Ok(CommandOutput::default())
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use serde_json::json;

    use crate::cmd::Registry;
    use crate::session::Session;

    fn write(root: &std::path::Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, text).expect("write");
    }

    fn build_index(root: &std::path::Path) {
        let db = notecraft_storage::index_path(root);
        let (mut index, _) =
            notecraft_storage::index::refresh_or_rebuild(root, &db).expect("index");
        crate::search::update_text_index(root, &mut index).expect("text");
    }

    /// Two notebooks on disk, opened in a session, with their indexes built.
    fn session() -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut notebooks = Vec::new();
        let mut roots = Vec::new();
        for (name, pages) in [
            (
                "One",
                &[
                    ("Grp/Alpha/Tent.md", "pack the tent for the trip"),
                    ("Grp/Alpha/Plan.md", "plan the budget"),
                    ("Grp/Beta/Notes.md", "the tent budget is here"),
                    ("Loose/Idea.md", "a tent idea"),
                ][..],
            ),
            ("Two", &[("Camp/Gear.md", "tent poles and a stove")][..]),
        ] {
            let root = dir.path().join(name);
            for (rel, text) in pages {
                write(&root, rel, text);
            }
            let opened = crate::open::open_folder(&root).expect("open");
            build_index(&root);
            roots.push((opened.notebook.id.clone(), root));
            notebooks.push(opened.notebook);
        }
        let mut session = Session::new();
        session.set_notebooks(notebooks);
        for (id, root) in &roots {
            session.set_notebook_root(id, root);
        }
        (dir, session)
    }

    fn run(session: &mut Session, args: serde_json::Value) -> serde_json::Value {
        Registry::builtin()
            .run(session, "search.run", args)
            .expect("run")
            .value
    }

    fn pages(value: &serde_json::Value) -> Vec<String> {
        value["hits"]
            .as_array()
            .map(|hits| {
                hits.iter()
                    .filter_map(|h| h["page"].as_str())
                    .map(|p| p.split_once('/').map_or(p, |(_, rest)| rest).to_owned())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn select(session: &mut Session, page: &str) {
        let id = session
            .notebooks()
            .iter()
            .flat_map(|nb| nb.all_sections())
            .flat_map(|s| s.pages.iter())
            .find(|p| p.id.ends_with(page))
            .map(|p| p.id.clone())
            .expect("page");
        Registry::builtin()
            .run(session, "nav.page.select", json!({ "id": id }))
            .expect("select");
    }

    #[test]
    fn scopes_narrow_from_all_notebooks_to_one_page() {
        let (_dir, mut session) = session();
        select(&mut session, "Grp/Alpha/Tent.md");
        let all = run(&mut session, json!({ "query": "tent", "scope": "all" }));
        assert_eq!(all["count"], 4);
        let notebook = run(
            &mut session,
            json!({ "query": "tent", "scope": "notebook" }),
        );
        assert_eq!(notebook["count"], 3);
        let group = run(&mut session, json!({ "query": "tent", "scope": "group" }));
        let mut found = pages(&group);
        found.sort();
        assert_eq!(found, ["Grp/Alpha/Tent.md", "Grp/Beta/Notes.md"]);
        let section = run(&mut session, json!({ "query": "tent", "scope": "section" }));
        assert_eq!(pages(&section), ["Grp/Alpha/Tent.md"]);
        let page = run(&mut session, json!({ "query": "tent", "scope": "page" }));
        assert_eq!(pages(&page), ["Grp/Alpha/Tent.md"]);
        let none = run(&mut session, json!({ "query": "stove", "scope": "page" }));
        assert_eq!(none["count"], 0);
    }

    #[test]
    fn a_section_outside_a_group_searches_the_notebook_as_its_group() {
        let (_dir, mut session) = session();
        select(&mut session, "Loose/Idea.md");
        let group = run(&mut session, json!({ "query": "tent", "scope": "group" }));
        assert_eq!(group["count"], 3);
    }

    #[test]
    fn sorting_orders_the_same_hits() {
        let (_dir, mut session) = session();
        let by_section = run(
            &mut session,
            json!({ "query": "tent", "scope": "all", "sort": "section" }),
        );
        assert_eq!(
            pages(&by_section),
            [
                "Grp/Alpha/Tent.md",
                "Grp/Beta/Notes.md",
                "Loose/Idea.md",
                "Camp/Gear.md"
            ]
        );
        let by_title = run(
            &mut session,
            json!({ "query": "tent", "scope": "all", "sort": "title" }),
        );
        assert_eq!(
            pages(&by_title),
            [
                "Camp/Gear.md",
                "Loose/Idea.md",
                "Grp/Beta/Notes.md",
                "Grp/Alpha/Tent.md"
            ]
        );
        // The state keeps the choice; changing it reruns the search.
        let reg = Registry::builtin();
        reg.run(
            &mut session,
            "search.set_sort",
            json!({ "sort": "relevance" }),
        )
        .expect("sort");
        let kept = session.search().results.as_ref().expect("results");
        assert_eq!(kept.hits.len(), 4);
        assert_eq!(kept.sort.id(), "relevance");
    }

    #[test]
    fn the_title_outranks_the_body() {
        let (_dir, mut session) = session();
        let hits = run(&mut session, json!({ "query": "tent", "scope": "all" }));
        assert_eq!(pages(&hits)[0], "Grp/Alpha/Tent.md");
    }

    #[test]
    fn hits_carry_a_snippet_with_highlights() {
        let (_dir, mut session) = session();
        run(&mut session, json!({ "query": "stove", "scope": "all" }));
        let results = session.search().results.as_ref().expect("results");
        let hit = &results.hits[0];
        assert!(hit.snippet.contains("stove"));
        let (from, to) = hit.highlights[0];
        assert_eq!(&hit.snippet[from..to], "stove");
        assert_eq!(hit.notebook_name, "Two");
        assert_eq!(hit.section_name, "Camp");
    }

    #[test]
    fn open_close_and_bad_arguments() {
        let (_dir, mut session) = session();
        let reg = Registry::builtin();
        reg.run(&mut session, "search.open", json!({}))
            .expect("open");
        assert!(session.search().open);
        let serial = session.search().open_serial;
        reg.run(&mut session, "search.all", json!({})).expect("all");
        assert!(session.search().open_serial > serial);
        reg.run(&mut session, "search.close", json!({}))
            .expect("close");
        assert!(!session.search().open);
        assert!(reg
            .run(
                &mut session,
                "search.set_scope",
                json!({ "scope": "galaxy" })
            )
            .is_err());
        assert!(reg
            .run(
                &mut session,
                "search.run",
                json!({ "query": "x", "sort": "nope" })
            )
            .is_err());
        // A blank query finds nothing and is not an error.
        let blank = run(&mut session, json!({ "query": "  " }));
        assert_eq!(blank["count"], 0);
    }

    #[test]
    fn edits_reach_the_results_after_the_text_index_updates() {
        let (dir, mut session) = session();
        let hits = run(&mut session, json!({ "query": "kayak", "scope": "all" }));
        assert_eq!(hits["count"], 0);
        let root = dir.path().join("Two");
        write(&root, "Camp/Gear.md", "a kayak and a paddle, longer text");
        build_index(&root);
        let hits = run(&mut session, json!({ "query": "kayak", "scope": "all" }));
        assert_eq!(hits["count"], 1);
        let gone = run(&mut session, json!({ "query": "stove", "scope": "all" }));
        assert_eq!(gone["count"], 0);
    }
}
