//! Mock notebooks for Phase 0: two notebooks, five sections (one inside a section group),
//! and twenty pages with subpages. Opening any notebook or page shows them until Phase 1 reads
//! notebooks from disk.

use notecraft_model::{Notebook, PageRef, Section, SectionGroup, SectionNode};

/// The date and time every mock page shows under its title. Phase 1 reads the real ones
/// from the page file.
pub const CREATED: (&str, &str) = ("Wednesday, October 14, 2026", "3:27 PM");

fn section(id: &str, name: &str, color: u32, pages: &[(&str, &str, u8)]) -> SectionNode {
    SectionNode::Section(Section {
        id: id.to_owned(),
        name: name.to_owned(),
        color,
        pages: pages
            .iter()
            .map(|&(id, title, level)| PageRef::new(id, title, level))
            .collect(),
    })
}

/// The mock notebooks, in notebook list order.
pub fn notebooks() -> Vec<Notebook> {
    vec![
        Notebook {
            id: "nb-personal".to_owned(),
            name: "My Notebook".to_owned(),
            color: 0x7E57C2,
            sections: vec![
                section(
                    "s-quick",
                    "Quick Notes",
                    0xF6C28B,
                    &[
                        ("p-welcome", "Welcome", 0),
                        ("p-grocery", "Grocery list", 0),
                        ("p-ideas", "Ideas", 0),
                        ("p-weekend", "Weekend project", 1),
                        ("p-books", "Book recommendations", 0),
                    ],
                ),
                section(
                    "s-personal",
                    "Personal",
                    0x9FD3E8,
                    &[
                        ("p-travel", "Travel plans", 0),
                        ("p-packing", "Packing list", 1),
                        ("p-toiletries", "Toiletries", 2),
                        ("p-recipes", "Recipes", 0),
                        ("p-breakfast", "Breakfast", 1),
                        ("p-dinner", "Dinner", 1),
                    ],
                ),
                SectionNode::Group(SectionGroup {
                    id: "g-archive".to_owned(),
                    name: "Archive".to_owned(),
                    children: vec![section(
                        "s-old",
                        "Old Projects",
                        0xC9B6E4,
                        &[("p-garden", "Garden plan", 0), ("p-bike", "Bike repair", 0)],
                    )],
                }),
            ],
        },
        Notebook {
            id: "nb-work".to_owned(),
            name: "Work Notebook".to_owned(),
            color: 0x26A69A,
            sections: vec![
                section(
                    "s-meetings",
                    "Meetings",
                    0xB5DCA6,
                    &[
                        ("p-sync", "Weekly sync", 0),
                        ("p-actions", "Action items", 1),
                        ("p-planning", "Planning session", 0),
                        ("p-one-on-one", "One on one", 0),
                    ],
                ),
                section(
                    "s-research",
                    "Research",
                    0xF3A9B8,
                    &[
                        ("p-reading", "Reading list", 0),
                        ("p-interviews", "Interview notes", 0),
                        ("p-follow-ups", "Follow-ups", 1),
                    ],
                ),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_matches_the_build_plan() {
        let notebooks = notebooks();
        assert_eq!(notebooks.len(), 2);
        let sections: Vec<&Section> = notebooks.iter().flat_map(|nb| nb.all_sections()).collect();
        assert_eq!(sections.len(), 5);
        let pages: usize = sections.iter().map(|s| s.pages.len()).sum();
        assert_eq!(pages, 20);
        assert!(sections.iter().flat_map(|s| &s.pages).any(|p| p.level == 2));
        let groups = notebooks
            .iter()
            .flat_map(|nb| &nb.sections)
            .filter(|node| matches!(node, SectionNode::Group(_)))
            .count();
        assert_eq!(groups, 1);
    }

    #[test]
    fn ids_are_unique() {
        let notebooks = notebooks();
        let mut ids = std::collections::BTreeSet::new();
        for nb in &notebooks {
            assert!(ids.insert(nb.id.clone()));
            for section in nb.all_sections() {
                assert!(ids.insert(section.id.clone()));
                for page in &section.pages {
                    assert!(ids.insert(page.id.clone()), "{}", page.id);
                }
            }
        }
    }
}
