//! Key tips: tap Alt (or press F10) to show a letter on every tab, type a tab's letter (or
//! press Alt plus the letter) to show codes on its buttons, and type a code to press that
//! button. Esc steps back one level; any mouse click turns key tips off.
//!
//! Tab letters are fixed in the spec. Button codes are assigned per tab from the labels:
//! one or two characters, and no code is a prefix of another, so a code is complete as soon
//! as it is typed.

use egui::{Color32, Context, Event, FontId, Id, Key, LayerId, Order, Pos2, Ui};

use super::spec::{Button, Group, TabKind, TABS};
use super::Look;

/// Which key tips show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    #[default]
    Off,
    /// A letter on every tab.
    Tabs,
    /// Codes on the selected tab's buttons; the string is what has been typed so far.
    Buttons(String),
}

/// What a key press asked the ribbon to do.
#[derive(Clone, Copy, Debug)]
pub(super) enum Action {
    OpenBackstage,
    SelectTab(usize),
    Press(&'static Button),
}

#[derive(Debug, Default)]
pub(super) struct KeyTips {
    pub(super) mode: Mode,
    alt_down: bool,
    /// Alt went down and nothing else was pressed while it was held.
    alt_alone: bool,
}

impl KeyTips {
    /// Read this frame's keyboard input. While key tips are showing, the key events they
    /// use are removed so nothing else reacts to them.
    pub(super) fn update(&mut self, ctx: &Context, selected: usize) -> Option<Action> {
        let (alt, keys, clicked) = ctx.input(|input| {
            let keys: Vec<(Key, egui::Modifiers)> = input
                .events
                .iter()
                .filter_map(|event| match event {
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => Some((*key, *modifiers)),
                    _ => None,
                })
                .collect();
            (input.modifiers.alt, keys, input.pointer.any_pressed())
        });
        if clicked {
            self.mode = Mode::Off;
        }
        if alt {
            if !self.alt_down {
                self.alt_alone = true;
            }
            if !keys.is_empty() {
                self.alt_alone = false;
            }
        } else if self.alt_down && self.alt_alone {
            self.toggle();
        }
        self.alt_down = alt;

        let mut action = None;
        let mut consumed = false;
        let mut tab = selected;
        for (key, modifiers) in keys {
            if key == Key::F10 && modifiers.is_none() {
                self.toggle();
                consumed = true;
                continue;
            }
            // Alt plus a tab letter goes straight to that tab's buttons.
            let alt_letter = modifiers.alt && !modifiers.ctrl && !modifiers.command;
            if alt_letter && self.mode == Mode::Off {
                self.mode = Mode::Tabs;
            }
            if self.mode == Mode::Off || modifiers.ctrl || modifiers.command {
                continue;
            }
            consumed = true;
            let mode = std::mem::take(&mut self.mode);
            self.mode = match (mode, key) {
                (Mode::Buttons(_), Key::Escape) => Mode::Tabs,
                (_, Key::Escape) => Mode::Off,
                (Mode::Tabs, key) => match tab_for_key(key.name()) {
                    Some((_, true)) => {
                        action = Some(Action::OpenBackstage);
                        Mode::Off
                    }
                    Some((index, false)) => {
                        tab = index;
                        action = Some(Action::SelectTab(index));
                        Mode::Buttons(String::new())
                    }
                    None => Mode::Tabs,
                },
                (Mode::Buttons(mut typed), key) => {
                    let name = key.name();
                    if name.len() == 1 {
                        typed.push_str(name);
                    }
                    let tips = tips_for_tab(tab);
                    if let Some((button, _)) = tips.iter().find(|(_, code)| *code == typed) {
                        action = Some(Action::Press(button));
                        Mode::Off
                    } else if tips
                        .iter()
                        .any(|(_, code)| code.starts_with(typed.as_str()))
                    {
                        Mode::Buttons(typed)
                    } else {
                        Mode::Buttons(String::new())
                    }
                }
                (Mode::Off, _) => Mode::Off,
            };
        }
        if consumed {
            ctx.input_mut(|input| {
                input
                    .events
                    .retain(|event| !matches!(event, Event::Key { .. } | Event::Text(_)));
            });
        }
        action
    }

    fn toggle(&mut self) {
        self.mode = if self.mode == Mode::Off {
            Mode::Tabs
        } else {
            Mode::Off
        };
    }
}

/// The tab whose letter is `name`, and whether it is the File tab.
fn tab_for_key(name: &str) -> Option<(usize, bool)> {
    TABS.iter()
        .enumerate()
        .find(|(_, tab)| tab.key == name)
        .map(|(index, tab)| (index, matches!(tab.kind, TabKind::Backstage(_))))
}

/// The key tip code of every button on tab `index`, in drawing order.
pub(crate) fn tips_for_tab(index: usize) -> Vec<(&'static Button, String)> {
    match TABS.get(index).map(|tab| tab.kind) {
        Some(TabKind::Groups(groups)) => assign(groups),
        _ => Vec::new(),
    }
}

/// Assign prefix-free codes to the buttons of `groups`, preferring letters from the label.
fn assign(groups: &'static [Group]) -> Vec<(&'static Button, String)> {
    let mut out: Vec<(&'static Button, String)> = Vec::new();
    for button in groups
        .iter()
        .flat_map(|group| group.buttons().collect::<Vec<_>>())
    {
        let code = candidates(button.label)
            .into_iter()
            .find(|code| {
                !out.iter().any(|(_, used)| {
                    used.starts_with(code.as_str()) || code.starts_with(used.as_str())
                })
            })
            .unwrap_or_default();
        out.push((button, code));
    }
    out
}

/// Codes to try for a label, best first.
fn candidates(label: &str) -> Vec<String> {
    let letters: Vec<char> = label
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let initials: Vec<char> = label
        .split_whitespace()
        .filter_map(|word| word.chars().find(char::is_ascii_alphanumeric))
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let mut out = Vec::new();
    if let Some(first) = letters.first() {
        out.push(first.to_string());
        if let Some(second) = initials.get(1) {
            out.push(format!("{first}{second}"));
        }
        for next in letters.iter().skip(1) {
            out.push(format!("{first}{next}"));
        }
    }
    for a in ('A'..='Z').chain('0'..='9') {
        for b in ('A'..='Z').chain('0'..='9') {
            out.push(format!("{a}{b}"));
        }
    }
    out
}

/// Draw a key tip label centered on `pos`, above everything else.
pub(super) fn paint_tip(ui: &Ui, pos: Pos2, code: &str, look: Look<'_>) {
    let painter = ui
        .ctx()
        .layer_painter(LayerId::new(Order::Tooltip, Id::new("ribbon_key_tips")));
    let t = look.tokens;
    let galley = painter.layout_no_wrap(
        code.to_owned(),
        FontId::proportional(11.0),
        Color32::PLACEHOLDER,
    );
    let rect = egui::Rect::from_center_size(pos, galley.size() + egui::vec2(8.0, 2.0));
    painter.rect_filled(rect, 2.0, t.ribbon_text);
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, t.border),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.center() - galley.size() / 2.0, galley, t.ribbon_bg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tab_has_a_unique_letter() {
        let mut seen = std::collections::BTreeSet::new();
        for tab in TABS {
            assert_eq!(tab.key.len(), 1, "{}", tab.name);
            assert!(seen.insert(tab.key), "{} used twice", tab.key);
        }
    }

    #[test]
    fn button_codes_are_short_and_prefix_free() {
        for (index, tab) in TABS.iter().enumerate() {
            let tips = tips_for_tab(index);
            for (i, (button, code)) in tips.iter().enumerate() {
                assert!(
                    (1..=2).contains(&code.len()),
                    "{} on {}",
                    button.label,
                    tab.name
                );
                for (other, other_code) in tips.iter().skip(i + 1) {
                    assert!(
                        !code.starts_with(other_code.as_str())
                            && !other_code.starts_with(code.as_str()),
                        "{} ({code}) and {} ({other_code}) on {}",
                        button.label,
                        other.label,
                        tab.name
                    );
                }
            }
        }
    }

    #[test]
    fn codes_start_with_the_label() {
        let tips = tips_for_tab(1);
        let paste = tips.iter().find(|(b, _)| b.label == "Paste");
        assert_eq!(paste.map(|(_, code)| code.as_str()), Some("P"));
    }
}
