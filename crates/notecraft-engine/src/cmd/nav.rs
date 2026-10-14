//! Navigation commands: `nav.notebook.select`, `nav.section.select`, `nav.page.select`.
//! They change which notebook, section, and page the session shows.

use serde_json::json;

use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::session::{Selection, Session};

fn id_arg(args: &Map<String, Value>) -> &str {
    args.get("id").and_then(Value::as_str).unwrap_or_default()
}

fn not_found(command: CommandId, what: &str, id: &str) -> CommandError {
    let id: String = id.chars().take(128).collect();
    CommandError::InvalidArgs {
        id: command,
        reason: format!("no {what} with id {id:?}"),
    }
}

fn apply(session: &mut Session, selection: Selection) -> CommandOutput {
    let value = json!({
        "notebook": selection.notebook,
        "section": selection.section,
        "page": selection.page,
    });
    if *session.selection() != selection {
        *session.selection_mut() = selection;
        session.mark_changed();
    }
    CommandOutput { value }
}

/// `nav.notebook.select { id }`: show a notebook, at its first section's first page.
pub(super) fn notebook_select(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = id_arg(args);
    let Some(notebook) = session.notebooks().iter().find(|nb| nb.id == id) else {
        return Err(not_found(ids::NAV_NOTEBOOK_SELECT, "notebook", id));
    };
    let section = notebook.first_section();
    let selection = Selection {
        notebook: Some(notebook.id.clone()),
        section: section.map(|s| s.id.clone()),
        page: section.and_then(|s| s.pages.first()).map(|p| p.id.clone()),
    };
    Ok(apply(session, selection))
}

/// `nav.section.select { id }`: show a section, at its first page.
pub(super) fn section_select(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = id_arg(args);
    let found = session
        .notebooks()
        .iter()
        .find_map(|nb| nb.section(id).map(|section| (nb, section)));
    let Some((notebook, section)) = found else {
        return Err(not_found(ids::NAV_SECTION_SELECT, "section", id));
    };
    let selection = Selection {
        notebook: Some(notebook.id.clone()),
        section: Some(section.id.clone()),
        page: section.pages.first().map(|p| p.id.clone()),
    };
    Ok(apply(session, selection))
}

/// `nav.page.select { id }`: show a page.
pub(super) fn page_select(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = id_arg(args);
    let found = session
        .notebooks()
        .iter()
        .find_map(|nb| nb.section_of_page(id).map(|section| (nb, section)));
    let Some((notebook, section)) = found else {
        return Err(not_found(ids::NAV_PAGE_SELECT, "page", id));
    };
    let selection = Selection {
        notebook: Some(notebook.id.clone()),
        section: Some(section.id.clone()),
        page: Some(id.to_owned()),
    };
    Ok(apply(session, selection))
}

#[cfg(test)]
mod tests {
    use notecraft_model::{Notebook, PageRef, Section, SectionGroup, SectionNode};
    use serde_json::json;

    use crate::cmd::Registry;
    use crate::session::Session;

    fn section(id: &str, pages: &[&str]) -> SectionNode {
        SectionNode::Section(Section {
            id: id.into(),
            name: id.into(),
            color: 0,
            pages: pages.iter().map(|p| PageRef::new(*p, *p, 0)).collect(),
        })
    }

    fn session() -> Session {
        let mut session = Session::new();
        session.set_notebooks(vec![
            Notebook {
                id: "nb1".into(),
                name: "One".into(),
                color: 0,
                sections: vec![section("s1", &["p1", "p2"]), section("s2", &[])],
            },
            Notebook {
                id: "nb2".into(),
                name: "Two".into(),
                color: 0,
                sections: vec![SectionNode::Group(SectionGroup {
                    id: "g".into(),
                    name: "Group".into(),
                    children: vec![section("s3", &["p3", "p4"])],
                })],
            },
        ]);
        session
    }

    fn ids(session: &Session) -> (Option<&str>, Option<&str>, Option<&str>) {
        let s = session.selection();
        (
            s.notebook.as_deref(),
            s.section.as_deref(),
            s.page.as_deref(),
        )
    }

    #[test]
    fn loading_notebooks_selects_the_first_page() {
        let session = session();
        assert_eq!(ids(&session), (Some("nb1"), Some("s1"), Some("p1")));
        assert_eq!(session.current_page().map(|p| p.id.as_str()), Some("p1"));
    }

    #[test]
    fn select_notebook_section_and_page() {
        let registry = Registry::builtin();
        let mut s = session();
        registry
            .run(&mut s, "nav.notebook.select", json!({ "id": "nb2" }))
            .unwrap();
        assert_eq!(ids(&s), (Some("nb2"), Some("s3"), Some("p3")));
        registry
            .run(&mut s, "nav.page.select", json!({ "id": "p2" }))
            .unwrap();
        assert_eq!(ids(&s), (Some("nb1"), Some("s1"), Some("p2")));
        registry
            .run(&mut s, "nav.section.select", json!({ "id": "s2" }))
            .unwrap();
        assert_eq!(ids(&s), (Some("nb1"), Some("s2"), None));
        assert_eq!(s.revision(), 3);
    }

    #[test]
    fn unknown_ids_are_rejected_and_change_nothing() {
        let registry = Registry::builtin();
        let mut s = session();
        let before = s.clone();
        for (command, args) in [
            ("nav.notebook.select", json!({ "id": "nope" })),
            ("nav.section.select", json!({ "id": "nope" })),
            ("nav.page.select", json!({ "id": "x".repeat(10_000) })),
            ("nav.page.select", json!({})),
        ] {
            let err = registry.run(&mut s, command, args).unwrap_err();
            assert_eq!(err.code(), "invalid_args");
        }
        assert_eq!(s, before);
    }

    #[test]
    fn selecting_the_current_page_is_not_a_change() {
        let registry = Registry::builtin();
        let mut s = session();
        registry
            .run(&mut s, "nav.page.select", json!({ "id": "p1" }))
            .unwrap();
        assert_eq!(s.revision(), 0);
    }
}
