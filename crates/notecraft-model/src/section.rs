//! Sections and section groups.

use crate::page::PageRef;

/// Deepest nesting of section groups that is walked. Deeper groups are ignored, so a
/// malformed or hostile tree cannot exhaust the stack.
pub const MAX_GROUP_DEPTH: usize = 16;

/// A section: a colored tab holding a list of pages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    /// Stable within its notebook.
    pub id: String,
    pub name: String,
    /// Tab color as 0xRRGGBB.
    pub color: u32,
    pub pages: Vec<PageRef>,
}

/// A named folder of sections and further groups.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionGroup {
    pub id: String,
    pub name: String,
    pub children: Vec<SectionNode>,
}

/// One entry in a notebook's section list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SectionNode {
    Section(Section),
    Group(SectionGroup),
}

impl Section {
    pub fn page(&self, id: &str) -> Option<&PageRef> {
        self.pages.iter().find(|page| page.id == id)
    }
}

/// Every section in `nodes`, depth first, groups included, up to [`MAX_GROUP_DEPTH`].
pub fn sections(nodes: &[SectionNode]) -> Vec<&Section> {
    let mut out = Vec::new();
    collect(nodes, 0, &mut out);
    out
}

fn collect<'a>(nodes: &'a [SectionNode], depth: usize, out: &mut Vec<&'a Section>) {
    if depth > MAX_GROUP_DEPTH {
        return;
    }
    for node in nodes {
        match node {
            SectionNode::Section(section) => out.push(section),
            SectionNode::Group(group) => collect(&group.children, depth + 1, out),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(id: &str) -> SectionNode {
        SectionNode::Section(Section {
            id: id.into(),
            name: id.into(),
            color: 0,
            pages: Vec::new(),
        })
    }

    #[test]
    fn sections_walk_groups_depth_first() {
        let nodes = vec![
            section("a"),
            SectionNode::Group(SectionGroup {
                id: "g".into(),
                name: "Group".into(),
                children: vec![section("b"), section("c")],
            }),
            section("d"),
        ];
        let ids: Vec<&str> = sections(&nodes).iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c", "d"]);
    }

    #[test]
    fn deep_nesting_is_bounded() {
        let mut node = section("deep");
        for i in 0..1000 {
            node = SectionNode::Group(SectionGroup {
                id: format!("g{i}"),
                name: String::new(),
                children: vec![node],
            });
        }
        assert!(sections(std::slice::from_ref(&node)).is_empty());
    }
}
