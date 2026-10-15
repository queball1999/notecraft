//! The ribbon: a tab strip over a body of groups, rendered from the data in [`spec`]. Every
//! button runs a command by id; the ribbon returns what was clicked and the shell dispatches
//! it (AGENTS.md section 4). A button whose command is Coming Soon says so in its tooltip, and
//! clicking it shows the Coming Soon toast.
//!
//! The chevron at the body's lower right opens Ribbon Display Options, as in OneNote: the
//! classic or the simplified layout ([`simplified`], `view.ribbon.layout`), and how much shows
//! (`view.ribbon.show`): always, tabs only, or nothing until the pointer reaches the top of
//! the window (full-screen mode). Ctrl+F1 and a double click on a tab switch between always
//! and tabs only. With tabs only, clicking a tab shows its groups over the page until a button
//! is used or the user clicks elsewhere. Key tips are in [`keytips`].

mod backstage;
mod button;
mod combo;
mod group;
pub(crate) mod keytips;
mod menus;
mod simplified;
pub mod spec;
mod tab;

pub(crate) use button::style as button_style;
pub(crate) use menus::{recent_colors, set_recent_colors};

use craft_ui::buttons::IconButton;
use egui::{
    Align, Id, Key, Layout, Order, Popup, PopupCloseBehavior, Rect, Sense, SetOpenCommand, Stroke,
    Ui, UiBuilder,
};
use notecraft_engine::cmd::{ids, CommandId, Registry, Value};
use notecraft_engine::recent::Recents;
use notecraft_engine::view::{RibbonLayout, RibbonShow};

use crate::themes::ThemeTokens;
use keytips::{Action, KeyTips, Mode};
use spec::{str_arg, Arg, Button, TabKind, TABS};

/// Height of the tab strip, in points.
pub const TAB_STRIP_HEIGHT: f32 = 26.0;
/// Height of the classic body under the tab strip, in points.
pub const BODY_HEIGHT: f32 = 96.0;
/// Total height of the classic ribbon.
pub const HEIGHT: f32 = TAB_STRIP_HEIGHT + BODY_HEIGHT;
/// Room kept at the right of the body for the display options chevron.
const CHEVRON_WIDTH: f32 = 28.0;
/// How close to the top of the window the pointer must come to show a full-screen ribbon.
const REVEAL_EDGE: f32 = 6.0;

/// Height of the body for `layout`.
pub fn body_height(layout: RibbonLayout) -> f32 {
    match layout {
        RibbonLayout::Classic => BODY_HEIGHT,
        RibbonLayout::Simplified => simplified::BODY_HEIGHT,
    }
}

/// Height the ribbon takes from the window for `layout` and `show`.
pub fn height(layout: RibbonLayout, show: RibbonShow) -> f32 {
    match show {
        RibbonShow::Always => TAB_STRIP_HEIGHT + body_height(layout),
        RibbonShow::TabsOnly => TAB_STRIP_HEIGHT,
        RibbonShow::FullScreen => 0.0,
    }
}

/// What the ribbon needs to draw itself.
#[derive(Clone, Copy)]
pub struct Look<'a> {
    pub tokens: &'a ThemeTokens,
    pub registry: &'a Registry,
    /// The installed font families for the font box; empty until they are known.
    pub fonts: &'a [String],
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

/// Key tip state for one frame of the body: the codes to draw and a button a code pressed.
struct KeyFrame {
    tips: Vec<(&'static Button, String)>,
    press: Option<&'static Button>,
}

impl KeyFrame {
    fn tip(&self, button: &'static Button) -> Option<&str> {
        self.tips
            .iter()
            .find(|(b, _)| std::ptr::eq(*b, button))
            .map(|(_, code)| code.as_str())
    }

    fn pressed(&self, button: &'static Button) -> bool {
        self.press.is_some_and(|b| std::ptr::eq(b, button))
    }
}

/// Ribbon state: the selected tab, the File backstage, the collapsed-ribbon peek, and key
/// tips.
#[derive(Debug)]
pub struct Ribbon {
    /// Index into [`TABS`] of the selected group tab.
    selected: usize,
    /// The highlighted backstage entry while the File backstage is open.
    backstage: Option<usize>,
    /// While collapsed: the selected tab's groups show over the page.
    peek: bool,
    /// In full-screen mode: the ribbon shows over the page.
    revealed: bool,
    /// The layout and show mode of the frame being drawn, for the display options menu.
    layout: RibbonLayout,
    show_mode: RibbonShow,
    keytips: KeyTips,
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
            peek: false,
            revealed: false,
            layout: RibbonLayout::default(),
            show_mode: RibbonShow::default(),
            keytips: KeyTips::default(),
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
        self.peek = false;
    }

    pub fn close_backstage(&mut self) {
        self.backstage = None;
    }

    /// Whether the groups show over the page while the ribbon is collapsed.
    pub fn peeking(&self) -> bool {
        self.peek
    }

    /// Whether key tips are showing.
    pub fn key_tips_showing(&self) -> bool {
        self.keytips.mode != Mode::Off
    }

    /// Whether a full-screen ribbon is showing over the page.
    pub fn revealed(&self) -> bool {
        self.revealed
    }

    /// Draw the ribbon into `ui`, [`height`] tall. In full-screen mode it takes no room: it
    /// shows over the page while the pointer is at the top of `window` or over the ribbon.
    /// Returns the commands clicked this frame.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        look: Look<'_>,
        layout: RibbonLayout,
        show: RibbonShow,
    ) -> Vec<Clicked> {
        // More Colors opens the Colors dialog, which stays up after its menu closes.
        let mut dialog = Vec::new();
        menus::colors_dialog(ui.ctx(), look, &mut dialog);
        let mut out = self.show_bar(ui, look, layout, show);
        out.extend(dialog);
        out
    }

    fn show_bar(
        &mut self,
        ui: &mut Ui,
        look: Look<'_>,
        layout: RibbonLayout,
        show: RibbonShow,
    ) -> Vec<Clicked> {
        self.layout = layout;
        self.show_mode = show;
        if show != RibbonShow::FullScreen {
            self.revealed = false;
            return self.draw(ui, look, show == RibbonShow::TabsOnly);
        }
        self.peek = false;
        let ctx = ui.ctx().clone();
        let window = ctx.content_rect();
        let pointer = ctx.input(|input| input.pointer.latest_pos());
        if pointer.is_some_and(|pos| pos.y <= window.min.y + REVEAL_EDGE) {
            self.revealed = true;
        }
        if !self.revealed {
            return Vec::new();
        }
        let size = egui::vec2(window.width(), TAB_STRIP_HEIGHT + body_height(layout));
        let mut out = Vec::new();
        let area = egui::Area::new(Id::new("ribbon_reveal"))
            .order(Order::Foreground)
            .fixed_pos(window.min)
            .show(&ctx, |ui| {
                let rect = Rect::from_min_size(ui.max_rect().min, size);
                ui.painter().rect_filled(rect, 0.0, look.tokens.ribbon_bg);
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    out = self.draw(ui, look, false);
                });
                ui.allocate_rect(rect, Sense::hover());
            });
        // It stays while the pointer is over it or one of its menus is open, and goes once a
        // command runs or on Esc.
        let over = pointer.is_some_and(|pos| area.response.rect.contains(pos));
        let menu_open = Popup::is_any_open(&ctx);
        let escape = ctx.input(|input| input.key_pressed(Key::Escape));
        if escape || !(over || menu_open) || (!out.is_empty() && !menu_open) {
            self.revealed = false;
        }
        out
    }

    /// The tab strip and, unless `collapsed`, the body; with `collapsed`, a peek over the page.
    fn draw(&mut self, ui: &mut Ui, look: Look<'_>, collapsed: bool) -> Vec<Clicked> {
        let mut out = Vec::new();
        let ctx = ui.ctx().clone();
        if !collapsed {
            self.peek = false;
        }
        let mut press = None;
        match self.keytips.update(&ctx, self.selected) {
            Some(Action::OpenBackstage) => self.open_backstage(),
            Some(Action::SelectTab(index)) => {
                self.selected = index;
                self.peek = collapsed;
            }
            Some(Action::Press(button)) => press = Some(button),
            None => {}
        }

        let full = ui.max_rect();
        let strip = Rect::from_min_size(full.min, egui::vec2(full.width(), TAB_STRIP_HEIGHT));
        let selected = (!collapsed || self.peek).then_some(self.selected);
        let tab_tips = self.keytips.mode == Mode::Tabs;
        if let Some(event) = tab::show_strip(ui, strip, selected, tab_tips, look) {
            if event.double_clicked {
                out.push(Clicked::new(ids::VIEW_RIBBON_TOGGLE, &[]));
                self.peek = false;
            } else if event.file {
                self.open_backstage();
            } else {
                // Clicking the open tab of a collapsed ribbon closes the peek.
                self.peek = collapsed && !(self.peek && event.index == self.selected);
                self.selected = event.index;
            }
        }

        let keys = KeyFrame {
            tips: match self.keytips.mode {
                Mode::Buttons(_) => keytips::tips_for_tab(self.selected),
                _ => Vec::new(),
            },
            press,
        };
        if !collapsed {
            let body = Rect::from_min_max(egui::pos2(full.min.x, strip.max.y), full.max);
            self.body(ui, body, look, &keys, &mut out);
        } else if self.peek {
            let area = egui::Area::new(Id::new("ribbon_peek"))
                .order(Order::Foreground)
                .fixed_pos(strip.left_bottom())
                .show(&ctx, |ui| {
                    let body = Rect::from_min_size(
                        ui.max_rect().min,
                        egui::vec2(strip.width(), body_height(self.layout)),
                    );
                    self.body(ui, body, look, &keys, &mut out);
                    ui.allocate_rect(body, Sense::hover());
                });
            let inside = area.response.rect.union(strip);
            let clicked_outside = ctx.input(|input| {
                input.pointer.any_pressed()
                    && input
                        .pointer
                        .interact_pos()
                        .is_some_and(|pos| !inside.contains(pos))
            });
            let escape = ctx.input(|input| input.key_pressed(Key::Escape));
            if !out.is_empty() || clicked_outside || escape {
                self.peek = false;
            }
        }
        ui.advance_cursor_after_rect(if collapsed { strip } else { full });
        out
    }

    /// The selected tab's groups and the collapse chevron, drawn into `body`.
    fn body(
        &mut self,
        ui: &mut Ui,
        body: Rect,
        look: Look<'_>,
        keys: &KeyFrame,
        out: &mut Vec<Clicked>,
    ) {
        let tokens = look.tokens;
        ui.painter().rect_filled(body, 0.0, tokens.ribbon_bg);
        ui.painter().hline(
            body.x_range(),
            body.max.y - 0.5,
            Stroke::new(1.0, tokens.border),
        );
        let groups = match TABS.get(self.selected).map(|tab| tab.kind) {
            Some(TabKind::Groups(groups)) => groups,
            _ => &[],
        };
        let mut inner = body.shrink2(egui::vec2(6.0, 3.0));
        inner.max.x -= CHEVRON_WIDTH;
        match self.layout {
            RibbonLayout::Classic => {
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
                                        group::show(ui, group, look, keys, out);
                                    }
                                });
                            });
                    },
                );
            }
            RibbonLayout::Simplified => simplified::show(ui, inner, groups, look, keys, out),
        }

        // While peeking, pin the ribbon open; otherwise open Ribbon Display Options.
        let peeking = self.peek;
        let label = if peeking {
            "Pin the Ribbon"
        } else {
            "Ribbon Display Options"
        };
        let chevron = Rect::from_min_size(
            egui::pos2(body.max.x - CHEVRON_WIDTH + 2.0, body.max.y - 24.0),
            egui::vec2(22.0, 20.0),
        );
        let style = button::style(tokens);
        let response = ui
            .scope_builder(UiBuilder::new().max_rect(chevron), |ui| {
                IconButton::new(label, chevron.size(), &style).show(ui, |ui, rect, color| {
                    let c = rect.center();
                    let points = if peeking {
                        // A pin: head and point.
                        vec![
                            c + egui::vec2(-4.0, -2.0),
                            c + egui::vec2(4.0, -2.0),
                            c + egui::vec2(0.0, -2.0),
                            c + egui::vec2(0.0, 5.0),
                        ]
                    } else {
                        // A down chevron, as in OneNote.
                        vec![
                            c + egui::vec2(-4.0, -2.0),
                            c + egui::vec2(0.0, 2.0),
                            c + egui::vec2(4.0, -2.0),
                        ]
                    };
                    ui.painter()
                        .add(egui::Shape::line(points, Stroke::new(1.5, color)));
                })
            })
            .inner;
        let response = crate::a11y::plain_button(response);
        if peeking {
            if response
                .on_hover_text(format!("{label} (Ctrl+F1)"))
                .clicked()
            {
                out.push(Clicked::new(
                    ids::VIEW_RIBBON_SHOW,
                    &[str_arg("mode", "always")],
                ));
                self.peek = false;
            }
            return;
        }
        let toggle = response.clicked().then_some(SetOpenCommand::Toggle);
        let response = response.on_hover_text(label);
        let (layout, show) = (self.layout, self.show_mode);
        Popup::menu(&response)
            .open_memory(toggle)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| display_options(ui, layout, show, look, out));
    }

    /// Draw the File backstage over the whole window. Returns the commands clicked.
    pub fn show_backstage(
        &mut self,
        ui: &mut Ui,
        look: Look<'_>,
        recents: &Recents,
    ) -> Vec<Clicked> {
        let mut out = Vec::new();
        backstage::show(self, ui, look, recents, &mut out);
        out
    }
}

/// Ribbon Display Options: Ribbon Layout and Show Ribbon, the current choices checked.
fn display_options(
    ui: &mut Ui,
    layout: RibbonLayout,
    show: RibbonShow,
    look: Look<'_>,
    out: &mut Vec<Clicked>,
) {
    ui.spacing_mut().item_spacing.y = 1.0;
    menus::heading(ui, "Ribbon Layout");
    for (choice, label) in [
        (RibbonLayout::Classic, "Classic Ribbon"),
        (RibbonLayout::Simplified, "Simplified Ribbon"),
    ] {
        if menus::choice(ui, label, layout == choice, look).clicked() {
            out.push(Clicked::new(
                ids::VIEW_RIBBON_LAYOUT,
                &[str_arg("layout", choice.id())],
            ));
            ui.close();
        }
    }
    ui.separator();
    menus::heading(ui, "Show Ribbon");
    for (choice, label) in [
        (RibbonShow::FullScreen, "Full-screen mode"),
        (RibbonShow::TabsOnly, "Show tabs only"),
        (RibbonShow::Always, "Always show Ribbon"),
    ] {
        if menus::choice(ui, label, show == choice, look).clicked() {
            out.push(Clicked::new(
                ids::VIEW_RIBBON_SHOW,
                &[str_arg("mode", choice.id())],
            ));
            ui.close();
        }
    }
}
