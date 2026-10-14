//! The ribbon: a tab strip over a body of groups, rendered from the data in [`spec`]. Every
//! button runs a command by id; the ribbon returns what was clicked and the shell dispatches
//! it (AGENTS.md section 4). A button whose command is Coming Soon says so in its tooltip, and clicking it shows the
//! Coming Soon toast.

mod backstage;
mod button;
mod group;
pub mod spec;
mod tab;

use egui::{Align, Layout, Rect, Ui, UiBuilder};
use notecraft_engine::cmd::{CommandId, Registry, Value};

use crate::themes::ThemeTokens;
use spec::{Arg, TabKind, TABS};

/// Height of the tab strip, in points.
pub const TAB_STRIP_HEIGHT: f32 = 26.0;
/// Height of the body under the tab strip, in points.
pub const BODY_HEIGHT: f32 = 96.0;
/// Total ribbon height.
pub const HEIGHT: f32 = TAB_STRIP_HEIGHT + BODY_HEIGHT;

/// What the ribbon needs to draw itself.
#[derive(Clone, Copy)]
pub struct Look<'a> {
    pub tokens: &'a ThemeTokens,
    pub registry: &'a Registry,
}

/// A command a ribbon button or menu item asked to run.
#[derive(Clone, Debug, PartialEq)]
pub struct Clicked {
    pub command: CommandId,
    pub args: Value,
}

impl Clicked {
    fn new(command: CommandId, args: &[Arg]) -> Self {
        Self {
            command,
            args: spec::args_value(args),
        }
    }
}

/// Ribbon state: which tab is selected and whether the File backstage is open.
#[derive(Debug)]
pub struct Ribbon {
    /// Index into [`TABS`] of the selected group tab.
    selected: usize,
    /// The highlighted backstage entry while the File backstage is open.
    backstage: Option<usize>,
}

impl Default for Ribbon {
    fn default() -> Self {
        let home = TABS
            .iter()
            .position(|tab| matches!(tab.kind, TabKind::Groups(_)))
            .unwrap_or(0);
        Self {
            selected: home,
            backstage: None,
        }
    }
}

impl Ribbon {
    pub fn new() -> Self {
        Self::default()
    }

    /// The name of the selected tab.
    pub fn selected(&self) -> &'static str {
        TABS.get(self.selected).map_or("", |tab| tab.name)
    }

    /// Select a group tab by name. Returns false for unknown names and for File.
    pub fn select(&mut self, name: &str) -> bool {
        match TABS
            .iter()
            .position(|tab| tab.name == name && matches!(tab.kind, TabKind::Groups(_)))
        {
            Some(index) => {
                self.selected = index;
                true
            }
            None => false,
        }
    }

    pub fn backstage_open(&self) -> bool {
        self.backstage.is_some()
    }

    pub fn open_backstage(&mut self) {
        self.backstage = Some(0);
    }

    pub fn close_backstage(&mut self) {
        self.backstage = None;
    }

    /// Draw the tab strip and the selected tab's groups into `ui`, which should be
    /// [`HEIGHT`] tall. Returns the commands clicked this frame.
    pub fn show(&mut self, ui: &mut Ui, look: Look<'_>) -> Vec<Clicked> {
        let mut out = Vec::new();
        let full = ui.max_rect();
        let strip = Rect::from_min_size(full.min, egui::vec2(full.width(), TAB_STRIP_HEIGHT));
        tab::show_strip(ui, strip, self, look);

        let body = Rect::from_min_max(egui::pos2(full.min.x, strip.max.y), full.max);
        let tokens = look.tokens;
        ui.painter().rect_filled(body, 0.0, tokens.ribbon_bg);
        ui.painter().hline(
            body.x_range(),
            body.max.y - 0.5,
            egui::Stroke::new(1.0, tokens.border),
        );
        let groups = match TABS.get(self.selected).map(|tab| tab.kind) {
            Some(TabKind::Groups(groups)) => groups,
            _ => &[],
        };
        let inner = body.shrink2(egui::vec2(6.0, 3.0));
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(inner)
                .layout(Layout::left_to_right(Align::Min)),
            |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("ribbon_body")
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            for group in groups {
                                group::show(ui, group, look, &mut out);
                            }
                        });
                    });
            },
        );
        ui.advance_cursor_after_rect(full);
        out
    }

    /// Draw the File backstage over the whole window. Returns the commands clicked.
    pub fn show_backstage(&mut self, ui: &mut Ui, look: Look<'_>) -> Vec<Clicked> {
        let mut out = Vec::new();
        backstage::show(self, ui, look, &mut out);
        out
    }
}
