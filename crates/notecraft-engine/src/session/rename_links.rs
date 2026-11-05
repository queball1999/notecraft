//! Page links through renames and moves (build_plan/04 PR 2.3, docs/markdown.md section 5.4):
//! the setting, the planning of link edits for a structure step, the report of what was done,
//! and the offer to fix links after a rename made outside NoteCraft.

use super::Session;
use crate::link_rename::{plan, PageSource, PlanPage, RewritePlan, SkipReason};
use crate::links::page_path;
use crate::undo::{FsOp, Op, Scope, Transaction};

/// What the last structure step did to page links.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkUpdateReport {
    /// Links rewritten.
    pub links: usize,
    /// Pages changed.
    pub pages: usize,
    /// Pages with links to fix that were not changed, with the reason.
    pub skipped: Vec<(String, String)>,
    /// Links that could not be rewritten.
    pub unfixable: usize,
    /// The step was made without the link edits because a page changed while it ran.
    pub failed: bool,
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

impl LinkUpdateReport {
    /// A sentence for a toast, or `None` when there is nothing to tell.
    pub fn message(&self) -> Option<String> {
        let mut parts = Vec::new();
        if self.failed {
            parts.push("Links were not updated because a page changed.".to_owned());
        } else if self.links > 0 {
            parts.push(format!(
                "Updated {} on {}.",
                plural(self.links, "link", "links"),
                plural(self.pages, "page", "pages")
            ));
        }
        if !self.skipped.is_empty() {
            parts.push(format!(
                "{} with links could not be changed (read-only or in conflict).",
                plural(self.skipped.len(), "page", "pages")
            ));
        }
        if self.unfixable > 0 {
            parts.push(format!(
                "{} could not be updated.",
                plural(self.unfixable, "link", "links")
            ));
        }
        (!parts.is_empty()).then(|| parts.join(" "))
    }
}

/// A rename made outside NoteCraft, found by page id, with links that can be fixed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkFixOffer {
    pub notebook: String,
    /// `(old path, new path)` from the notebook folder.
    pub moves: Vec<(String, String)>,
    /// Links that would be rewritten.
    pub links: usize,
}

/// The edits a structure step needs, ready to put in front of its other operations.
pub(crate) struct LinkFix {
    pub ops: Vec<Op>,
    pub report: LinkUpdateReport,
}

/// The path after `renames` (each an exact path or a folder prefix).
pub(crate) fn mapped(path: &str, renames: &[(String, String)]) -> String {
    for (from, to) in renames {
        if path == from {
            return to.clone();
        }
        if let Some(rest) = path
            .strip_prefix(from.as_str())
            .and_then(|r| r.strip_prefix('/'))
        {
            return format!("{to}/{rest}");
        }
    }
    path.to_owned()
}

impl Session {
    /// Whether renaming or moving a page rewrites the links that point to it
    /// (`links.update_on_rename`, on by default).
    pub fn link_update_on_rename(&self) -> bool {
        !self.no_link_updates
    }

    /// Set `links.update_on_rename` (restoring saved settings, or the command).
    pub fn set_link_update_on_rename(&mut self, on: bool) {
        if self.no_link_updates == on {
            self.no_link_updates = !on;
            self.mark_changed();
        }
    }

    /// Whether page `id` has unsaved text that clashes with a newer file.
    pub(crate) fn page_has_conflict(&self, id: &str) -> bool {
        self.saving.conflicts.contains_key(id)
    }

    /// What the last rename or move did to links, handed out once.
    pub fn take_link_update_report(&mut self) -> Option<LinkUpdateReport> {
        self.link_update_report.take()
    }

    /// The text of a page for the planner: from memory when open, else from its file.
    fn plan_source(&self, notebook: &str, page: &PlanPage) -> Option<PageSource> {
        if let Some(text) = self.page_text(&page.id) {
            let blocked = if self.page_is_read_only(&page.id) {
                Some(SkipReason::ReadOnly)
            } else if self.page_has_conflict(&page.id) {
                Some(SkipReason::Conflict)
            } else {
                None
            };
            return Some(PageSource {
                text: text.to_owned(),
                blocked,
            });
        }
        let root = self.notebook_roots.get(notebook)?;
        let found = notecraft_storage::read_page_file(&root.join(&page.rel)).ok()?;
        Some(PageSource {
            blocked: found.read_only().then_some(SkipReason::ReadOnly),
            text: found.text,
        })
    }

    /// The pages of `notebook` as the planner sees them. `back` is true when the pages are
    /// already at their new paths and `renames` say how to find the old ones.
    fn plan_pages(
        &self,
        notebook: &str,
        renames: &[(String, String)],
        back: bool,
    ) -> Vec<PlanPage> {
        let Some(nb) = self.notebooks.iter().find(|nb| nb.id == notebook) else {
            return Vec::new();
        };
        crate::links::link_pages(nb, &|id| {
            self.link_aliases.get(id).cloned().unwrap_or_default()
        })
        .into_iter()
        .map(|page| {
            let (old, new) = if back {
                let old = renames
                    .iter()
                    .find(|(_, new)| *new == page.path)
                    .map_or_else(|| page.path.clone(), |(old, _)| old.clone());
                (old, page.path.clone())
            } else {
                (page.path.clone(), mapped(&page.path, renames))
            };
            PlanPage {
                rel: page.path,
                old,
                new,
                id: page.id,
                aliases: page.aliases,
            }
        })
        .collect()
    }

    /// The link edits for a step that renames or moves files (`renames`: old path to new
    /// path, a file or a folder). `None` when the setting is off or no link is affected.
    pub(crate) fn link_fix(&self, notebook: &str, renames: &[(String, String)]) -> Option<LinkFix> {
        if self.no_link_updates || renames.is_empty() {
            return None;
        }
        let pages = self.plan_pages(notebook, renames, false);
        let found = plan(&pages, &mut |page| self.plan_source(notebook, page));
        if found.is_empty() {
            return None;
        }
        Some(fix_of(notebook, &found))
    }

    /// The offer to fix links after an outside rename, while one is open.
    pub fn link_fix_offer(&self) -> Option<&LinkFixOffer> {
        self.link_fix_offer.as_ref()
    }

    /// Offer to fix links after outside renames. Replaces an earlier offer.
    pub fn offer_link_fix(&mut self, offer: LinkFixOffer) {
        if self.link_fix_offer.as_ref() != Some(&offer) {
            self.link_fix_offer = Some(offer);
            self.mark_changed();
        }
    }

    /// Pages found renamed or moved outside NoteCraft (same `notecraft.id` at a new path;
    /// `(old path, new path)` from the notebook folder). Offers to fix the links to them when
    /// some link would stop working, counting the links as the open pages and files hold
    /// them now (so a rename NoteCraft made itself, whose links are fixed, offers nothing).
    /// Does nothing without ids: no moves are passed.
    pub fn consider_outside_renames(&mut self, notebook: &str, moves: Vec<(String, String)>) {
        if moves.is_empty() {
            return;
        }
        let pages = self.plan_pages(notebook, &moves, true);
        let found = plan(&pages, &mut |page| self.plan_source(notebook, page));
        if found.links > 0 {
            self.offer_link_fix(LinkFixOffer {
                notebook: notebook.to_owned(),
                moves,
                links: found.links,
            });
        }
    }

    /// Close the offer without fixing. Returns whether one was open.
    pub fn dismiss_link_fix(&mut self) -> bool {
        let was = self.link_fix_offer.take().is_some();
        if was {
            self.mark_changed();
        }
        was
    }

    /// Rewrite the links to the pages that were renamed outside NoteCraft, as one undo step.
    /// The pages are already at their new paths, so the old answer of each link is worked out
    /// with the renames taken back. Returns what was done, or `None` without an offer.
    pub fn fix_links_after_rename(&mut self) -> Option<LinkUpdateReport> {
        let offer = self.link_fix_offer.take()?;
        self.mark_changed();
        let pages = self.plan_pages(&offer.notebook, &offer.moves, true);
        let found = plan(&pages, &mut |page| self.plan_source(&offer.notebook, page));
        let fix = fix_of(&offer.notebook, &found);
        let mut report = fix.report;
        if !fix.ops.is_empty() {
            let tx = fix.ops.into_iter().fold(
                Transaction::new(Scope::Notebook, "Update links"),
                Transaction::with,
            );
            if !self.commit(tx) {
                report.failed = true;
                report.links = 0;
                report.pages = 0;
            }
        }
        Some(report)
    }
}

/// Turn a plan into operations and a report.
fn fix_of(notebook: &str, found: &RewritePlan) -> LinkFix {
    let mut ops = Vec::new();
    for page in &found.pages {
        for edit in &page.edits {
            ops.push(Op::Fs(FsOp::PatchText {
                notebook: notebook.to_owned(),
                rel: page.rel.clone(),
                at: edit.at,
                removed: edit.removed.clone(),
                inserted: edit.inserted.clone(),
            }));
        }
    }
    let report = LinkUpdateReport {
        links: found.links,
        pages: found.pages.len(),
        skipped: found
            .skipped
            .iter()
            .map(|(id, why)| {
                (
                    page_path(notebook, id).unwrap_or(id).to_owned(),
                    why.label().to_owned(),
                )
            })
            .collect(),
        unfixable: found.unfixable,
        failed: false,
    };
    LinkFix { ops, report }
}
