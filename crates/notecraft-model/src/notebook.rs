//! A notebook: a named, colored list of sections and section groups.

use crate::section::{self, Section, SectionNode};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notebook {
    /// Stable across runs.
    pub id: String,
    pub name: String,
    /// Swatch color in the notebook list, as 0xRRGGBB.
    pub color: u32,
    pub sections: Vec<SectionNode>,
}

impl Notebook {
    /// Every section, depth first through groups.
    pub fn all_sections(&self) -> Vec<&Section> {
        section::sections(&self.sections)
    }

    pub fn section(&self, id: &str) -> Option<&Section> {
        self.all_sections().into_iter().find(|s| s.id == id)
    }

    pub fn first_section(&self) -> Option<&Section> {
        self.all_sections().into_iter().next()
    }

    /// The section holding page `id`.
    pub fn section_of_page(&self, id: &str) -> Option<&Section> {
        self.all_sections()
            .into_iter()
            .find(|s| s.page(id).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::PageRef;
    use crate::section::SectionGroup;

    fn notebook() -> Notebook {
        let section = |id: &str, pages: &[&str]| {
            SectionNode::Section(Section {
                id: id.into(),
                name: id.into(),
                color: 0x88_99_AA,
                pages: pages.iter().map(|p| PageRef::new(*p, *p, 0)).collect(),
            })
        };
        Notebook {
            id: "nb".into(),
            name: "Notebook".into(),
            color: 0,
            sections: vec![
                section("s1", &["p1", "p2"]),
                SectionNode::Group(SectionGroup {
                    id: "g".into(),
                    name: "Group".into(),
                    children: vec![section("s2", &["p3"])],
                }),
            ],
        }
    }

    #[test]
    fn finds_sections_and_pages_through_groups() {
        let nb = notebook();
        assert_eq!(nb.first_section().map(|s| s.id.as_str()), Some("s1"));
        assert_eq!(nb.section("s2").map(|s| s.name.as_str()), Some("s2"));
        assert_eq!(nb.section_of_page("p3").map(|s| s.id.as_str()), Some("s2"));
        assert!(nb.section("nope").is_none());
        assert!(nb.section_of_page("nope").is_none());
    }
}
