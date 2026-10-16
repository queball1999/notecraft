//! Keyboard shortcuts: the engine's default map (`cmd::shortcuts`) as egui shortcuts. Each
//! frame the shell takes the pressed ones and dispatches their commands. "Ctrl" in the map is
//! egui's command modifier, so it is Cmd on macOS.

use egui::{Key, KeyboardShortcut, Modifiers};
use notecraft_engine::cmd::shortcuts::{self, Chord};

use crate::ribbon::Clicked;

/// The shortcut map, ready to match against input.
#[derive(Debug)]
pub struct Shortcuts {
    /// Most modifiers first, as the engine sorts them: egui's matching ignores extra Shift
    /// and Alt, so Ctrl+Shift+N must be tried before Ctrl+N.
    bindings: Vec<(KeyboardShortcut, Clicked)>,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self::new()
    }
}

impl Shortcuts {
    pub fn new() -> Self {
        let bindings = shortcuts::default_map()
            .into_iter()
            .filter_map(|binding| {
                let shortcut = to_egui(binding.chord)?;
                let clicked = Clicked {
                    command: binding.command,
                    args: binding.args,
                };
                Some((shortcut, clicked))
            })
            .collect();
        Self { bindings }
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// The commands whose shortcuts were pressed this frame. Their key presses are consumed.
    /// Nothing is taken while a text field has focus, so typing and text editing keys work.
    pub fn pressed(&self, ctx: &egui::Context) -> Vec<Clicked> {
        if ctx.egui_wants_keyboard_input() {
            return Vec::new();
        }
        ctx.input_mut(|input| {
            self.bindings
                .iter()
                .filter(|(shortcut, _)| input.consume_shortcut(shortcut))
                .map(|(_, clicked)| clicked.clone())
                .collect()
        })
    }
}

/// The egui shortcut for a chord, or `None` for a key egui does not name.
fn to_egui(chord: Chord) -> Option<KeyboardShortcut> {
    let key = Key::from_name(chord.key)?;
    let modifiers = Modifiers {
        alt: chord.alt,
        ctrl: false,
        shift: chord.shift,
        mac_cmd: false,
        command: chord.ctrl,
    };
    Some(KeyboardShortcut::new(modifiers, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_engine_shortcut_has_an_egui_key() {
        assert_eq!(Shortcuts::new().len(), shortcuts::default_map().len());
    }

    #[test]
    fn chords_map_to_command_modifiers() {
        let chord = Chord::parse("Ctrl+Alt+D").unwrap();
        let shortcut = to_egui(chord).unwrap();
        assert!(shortcut.modifiers.command && shortcut.modifiers.alt && !shortcut.modifiers.shift);
        assert_eq!(shortcut.logical_key, Key::D);
        assert_eq!(
            to_egui(Chord::parse("Ctrl+-").unwrap()).map(|s| s.logical_key),
            Some(Key::Minus)
        );
        assert_eq!(
            to_egui(Chord::parse("F11").unwrap()).map(|s| s.logical_key),
            Some(Key::F11)
        );
    }
}
