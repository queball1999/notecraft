//! Resolution order and ambiguity (docs/markdown.md section 5.3).

use crate::link_resolve::{fold, LinkIndex, LinkPage, Resolved, MAX_CANDIDATES};

fn page(path: &str) -> LinkPage {
    LinkPage::new(format!("nb/{path}"), path)
}

fn index(paths: &[&str]) -> LinkIndex {
    LinkIndex::new(paths.iter().map(|p| page(p)).collect())
}

fn id_of(found: Resolved<'_>) -> Option<String> {
    found.page().map(|p| p.id.clone())
}

fn candidates(found: Resolved<'_>) -> Vec<String> {
    match found {
        Resolved::Ambiguous { candidates, .. } => {
            candidates.iter().map(|p| p.path.clone()).collect()
        }
        _ => Vec::new(),
    }
}

#[test]
fn step_1_the_path_from_the_notebook_root_wins() {
    let idx = index(&["Work/Plan.md", "Home/Work/Plan.md", "Plan.md"]);
    // `Work/Plan` is the full path of one page, though another page ends the same way.
    assert_eq!(
        idx.resolve("Work/Plan", "Home/Notes.md"),
        Resolved::Page(&page("Work/Plan.md"))
    );
    // The root page `Plan` beats the closer `Home/Plan` (path before base name).
    let idx = index(&["Plan.md", "Home/Plan.md"]);
    assert_eq!(
        idx.resolve("Plan", "Home/Notes.md"),
        Resolved::Page(&page("Plan.md"))
    );
}

#[test]
fn step_1_ignores_the_extension_kind() {
    let idx = index(&["Sec/A.notecraft"]);
    assert_eq!(
        id_of(idx.resolve("Sec/A", "")),
        Some("nb/Sec/A.notecraft".into())
    );
    // A dot in the name is not an extension.
    let idx = index(&["Sec/v1.2 plan.md"]);
    assert_eq!(
        id_of(idx.resolve("Sec/v1.2 plan", "")),
        Some("nb/Sec/v1.2 plan.md".into())
    );
    assert!(idx.resolve("Sec/v1", "").page().is_none());
}

#[test]
fn step_2_the_path_from_the_linking_pages_folder() {
    let idx = index(&["A/Sub/Page.md", "B/Sub/Page.md", "A/Main.md"]);
    assert_eq!(
        idx.resolve("Sub/Page", "A/Main.md"),
        Resolved::Page(&page("A/Sub/Page.md"))
    );
    assert_eq!(
        idx.resolve("Sub/Page", "B/Other.md"),
        Resolved::Page(&page("B/Sub/Page.md"))
    );
}

#[test]
fn step_2_beats_step_3() {
    // `Page` is the base name of two pages, but one sits in the linking page's folder.
    let idx = index(&["A/Page.md", "B/Page.md", "B/Main.md"]);
    assert_eq!(
        idx.resolve("Page", "B/Main.md"),
        Resolved::Page(&page("B/Page.md"))
    );
}

#[test]
fn step_3_a_unique_base_name() {
    let idx = index(&["Work/Plan.md", "Home/Groceries.md"]);
    assert_eq!(
        idx.resolve("Groceries", "Work/Plan.md"),
        Resolved::Page(&page("Home/Groceries.md"))
    );
    assert_eq!(idx.resolve("Missing", "Work/Plan.md"), Resolved::Unresolved);
}

#[test]
fn step_3_more_than_one_page_is_ambiguous_and_the_closest_wins() {
    let idx = index(&["A/X/Notes.md", "B/Notes.md", "C/D/E/Notes.md", "A/Main.md"]);
    let found = idx.resolve("Notes", "A/Main.md");
    assert!(found.is_ambiguous());
    // Distance from `A`: A/X is one step, B is two, C/D/E is four.
    assert_eq!(
        candidates(found.clone()),
        ["A/X/Notes.md", "B/Notes.md", "C/D/E/Notes.md"]
    );
    assert_eq!(id_of(found), Some("nb/A/X/Notes.md".into()));
    // From another folder the order changes.
    let found = idx.resolve("Notes", "B/Other.md");
    assert_eq!(id_of(found), Some("nb/B/Notes.md".into()));
}

#[test]
fn ties_in_distance_break_by_path_length_then_name() {
    let idx = index(&["Z/Notes.md", "Y/Notes.md", "X/Notes.md", "Home/Main.md"]);
    let found = idx.resolve("Notes", "Home/Main.md");
    assert_eq!(
        candidates(found),
        ["X/Notes.md", "Y/Notes.md", "Z/Notes.md"]
    );
    // The order does not depend on the order the pages were listed in.
    let idx = index(&["X/Notes.md", "Z/Notes.md", "Y/Notes.md", "Home/Main.md"]);
    assert_eq!(
        id_of(idx.resolve("Notes", "Home/Main.md")),
        Some("nb/X/Notes.md".into())
    );
}

#[test]
fn step_4_ignores_case() {
    let idx = index(&["Work/Project Plan.md"]);
    assert_eq!(
        idx.resolve("project plan", "Home/Main.md"),
        Resolved::Page(&page("Work/Project Plan.md"))
    );
    assert_eq!(
        idx.resolve("PROJECT PLAN", "Home/Main.md"),
        Resolved::Page(&page("Work/Project Plan.md"))
    );
}

#[test]
fn step_4_ignores_unicode_normalization() {
    // "Cafe" with an acute accent: precomposed in the file name, decomposed in the link.
    let idx = index(&["Notes/Caf\u{e9}.md"]);
    assert_eq!(
        id_of(idx.resolve("Cafe\u{301}", "")),
        Some("nb/Notes/Caf\u{e9}.md".into())
    );
    assert_eq!(
        id_of(idx.resolve("CAFE\u{301}", "")),
        Some("nb/Notes/Caf\u{e9}.md".into())
    );
    // The other way around.
    let idx = index(&["Notes/Cafe\u{301}.md"]);
    assert_eq!(
        id_of(idx.resolve("Caf\u{e9}", "")),
        Some("nb/Notes/Cafe\u{301}.md".into())
    );
    assert_eq!(fold("Caf\u{e9}"), fold("Cafe\u{301}"));
    // A different letter is still different.
    assert!(idx.resolve("Cafa\u{301}", "").page().is_none());
}

#[test]
fn step_4_matches_a_path_ignoring_case() {
    let idx = index(&["Work/Plan.md"]);
    assert_eq!(
        idx.resolve("work/plan", "Home/Main.md"),
        Resolved::Page(&page("Work/Plan.md"))
    );
    // From the same folder, relative to it.
    let idx = index(&["Work/Sub/Plan.md"]);
    assert_eq!(
        idx.resolve("sub/plan", "Work/Main.md"),
        Resolved::Page(&page("Work/Sub/Plan.md"))
    );
}

#[test]
fn step_4_two_pages_differing_only_in_case_are_ambiguous() {
    let idx = index(&["A/Todo.md", "B/todo.md", "A/Main.md"]);
    // Step 3 matches the exact spelling first, so `Todo` is not ambiguous.
    assert_eq!(
        idx.resolve("Todo", "A/Main.md"),
        Resolved::Page(&page("A/Todo.md"))
    );
    // `TODO` matches neither exactly; step 4 finds both.
    let found = idx.resolve("TODO", "A/Main.md");
    assert!(found.is_ambiguous());
    assert_eq!(candidates(found), ["A/Todo.md", "B/todo.md"]);
}

#[test]
fn step_3_beats_step_4() {
    let idx = index(&["Far/Away/plan.md", "Near/Plan.md"]);
    // `plan` is exact for one page; the case-insensitive match is never reached.
    assert_eq!(
        idx.resolve("plan", "Near/Main.md"),
        Resolved::Page(&page("Far/Away/plan.md"))
    );
}

#[test]
fn step_5_aliases() {
    let pages = vec![
        page("Work/Quarterly review.md").with_aliases(["Q3 sync", "QBR"]),
        page("Home/Main.md"),
    ];
    let idx = LinkIndex::new(pages);
    assert_eq!(
        id_of(idx.resolve("Q3 sync", "Home/Main.md")),
        Some("nb/Work/Quarterly review.md".into())
    );
    assert_eq!(
        id_of(idx.resolve("QBR", "")),
        Some("nb/Work/Quarterly review.md".into())
    );
    // Aliases are exact.
    assert_eq!(idx.resolve("qbr", ""), Resolved::Unresolved);
}

#[test]
fn step_5_is_last() {
    // A page named like another page's alias wins by name.
    let idx = LinkIndex::new(vec![
        page("A/Other.md").with_aliases(["Plan"]),
        page("B/Plan.md"),
    ]);
    assert_eq!(id_of(idx.resolve("Plan", "")), Some("nb/B/Plan.md".into()));
    // Two pages with the same alias are ambiguous.
    let idx = LinkIndex::new(vec![
        page("A/One.md").with_aliases(["Shared"]),
        page("B/Two.md").with_aliases(["Shared"]),
        page("B/Main.md"),
    ]);
    let found = idx.resolve("Shared", "B/Main.md");
    assert!(found.is_ambiguous());
    assert_eq!(id_of(found), Some("nb/B/Two.md".into()));
}

#[test]
fn unresolved_and_blank_targets() {
    let idx = index(&["A.md"]);
    assert_eq!(idx.resolve("", ""), Resolved::Unresolved);
    assert_eq!(idx.resolve("   ", ""), Resolved::Unresolved);
    assert_eq!(idx.resolve("Nope", "A.md"), Resolved::Unresolved);
    assert_eq!(LinkIndex::default().resolve("A", ""), Resolved::Unresolved);
    assert!(LinkIndex::default().is_empty());
}

#[test]
fn the_target_is_trimmed() {
    let idx = index(&["A.md"]);
    assert_eq!(idx.resolve("  A ", ""), Resolved::Page(&page("A.md")));
}

#[test]
fn a_page_resolves_to_itself() {
    let idx = index(&["Sec/Self.md", "Sec/Other.md"]);
    assert_eq!(
        idx.resolve("Self", "Sec/Self.md"),
        Resolved::Page(&page("Sec/Self.md"))
    );
}

#[test]
fn hostile_targets_do_not_panic() {
    let idx = index(&["A.md", "B/C.md"]);
    for target in [
        "/", "//", "../A", "./A", "A/", "/A", ".", "..", "\u{0}", "\u{301}",
    ] {
        let _ = idx.resolve(target, "B/C.md");
        let _ = idx.resolve(target, "weird/");
        let _ = idx.resolve(target, "/");
    }
    let odd = LinkIndex::new(vec![
        page(".md"),
        page("dir/.hidden"),
        page("noext"),
        page(""),
    ]);
    let _ = odd.resolve("noext", "");
    let _ = odd.resolve("", "");
}

#[test]
fn the_candidate_list_is_capped() {
    let paths: Vec<String> = (0..100).map(|n| format!("F{n}/Same.md")).collect();
    let idx = LinkIndex::new(paths.iter().map(|p| page(p)).collect());
    match idx.resolve("Same", "") {
        Resolved::Ambiguous { candidates, .. } => assert_eq!(candidates.len(), MAX_CANDIDATES),
        other => panic!("expected ambiguous, got {other:?}"),
    }
}

#[test]
fn the_shortest_form_is_the_base_name_when_it_is_enough() {
    let idx = index(&["Work/Plan.md", "Home/Groceries.md", "Home/Main.md"]);
    let plan = page("Work/Plan.md");
    assert_eq!(idx.shortest_form(&plan, "Home/Main.md"), "Plan");
    assert_eq!(idx.shortest_form(&plan, ""), "Plan");
}

#[test]
fn the_shortest_form_names_the_path_when_the_base_name_is_shared() {
    let idx = index(&[
        "A/Notes.md",
        "B/Notes.md",
        "A/Main.md",
        "B/Main.md",
        "C/Main.md",
    ]);
    let a = page("A/Notes.md");
    let form = idx.shortest_form(&a, "C/Main.md");
    assert_eq!(form, "A/Notes");
    assert_eq!(idx.resolve(&form, "C/Main.md"), Resolved::Page(&a));
    // From A, `Notes` finds A/Notes by folder (step 2).
    assert_eq!(idx.shortest_form(&a, "A/Main.md"), "Notes");
    let b = page("B/Notes.md");
    let form = idx.shortest_form(&b, "A/Main.md");
    assert_eq!(form, "B/Notes");
    assert_eq!(idx.resolve(&form, "A/Main.md"), Resolved::Page(&b));
}

#[test]
fn the_shortest_form_uses_the_folder_relative_path() {
    // `Sub/Page` from inside `A` is shorter than `A/Sub/Page`, and `Page` is shared.
    let idx = index(&["A/Sub/Page.md", "B/Page.md", "A/Main.md"]);
    let target = page("A/Sub/Page.md");
    assert_eq!(idx.shortest_form(&target, "A/Main.md"), "Sub/Page");
    assert_eq!(idx.shortest_form(&target, "B/Other.md"), "A/Sub/Page");
}

#[test]
fn every_shortest_form_resolves_back_to_its_page() {
    let paths = [
        "A/Notes.md",
        "B/Notes.md",
        "A/Sub/Notes.md",
        "Notes.md",
        "C/notes.md",
        "C/Plan.md",
        "D/E/Plan.md",
    ];
    let idx = index(&paths);
    for from in ["", "A/Notes.md", "C/Plan.md", "D/E/Plan.md", "Z/Q.md"] {
        for target in idx.pages() {
            let form = idx.shortest_form(target, from);
            assert_eq!(
                idx.resolve(&form, from).page().map(|p| &p.id),
                Some(&target.id),
                "form {form:?} from {from:?}"
            );
        }
    }
}
