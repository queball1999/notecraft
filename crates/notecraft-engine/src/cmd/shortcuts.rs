//! The default shortcut map: OneNote's keyboard shortcuts, as key chords bound to commands.
//!
//! Most bindings come from each command's `default_shortcut`. Shortcuts that run a command
//! with arguments (Ctrl+2 to Ctrl+9 apply note tags) are listed in [`WITH_ARGS`]. "Ctrl" means
//! the platform command key: the front end maps it to Cmd on macOS.

use serde_json::Value;

use super::{builtin, ids, CommandId, Map};

/// A key chord such as `Ctrl+Alt+D`: modifiers and one key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// The key name: `A` to `Z`, `0` to `9`, `F1` to `F12`, or one of [`NAMED_KEYS`].
    pub key: &'static str,
}

/// Keys other than letters, digits, and function keys that a chord may use.
pub const NAMED_KEYS: [&str; 8] = ["=", "-", ".", "/", "Delete", "Left", "Right", "Escape"];

impl Chord {
    /// Parse `Ctrl+Shift+N`. Modifiers come first, in any order, each at most once; the
    /// key comes last. Anything else is `None`.
    pub fn parse(text: &'static str) -> Option<Self> {
        let (mods, key) = match text.rsplit_once('+') {
            // "Ctrl+-" splits as ("Ctrl", "-"); a trailing "+" key is not supported.
            Some((mods, key)) => (Some(mods), key),
            None => (None, text),
        };
        if !is_key(key) {
            return None;
        }
        let mut chord = Self {
            ctrl: false,
            alt: false,
            shift: false,
            key,
        };
        for part in mods.into_iter().flat_map(|mods| mods.split('+')) {
            let flag = match part {
                "Ctrl" => &mut chord.ctrl,
                "Alt" => &mut chord.alt,
                "Shift" => &mut chord.shift,
                _ => return None,
            };
            if *flag {
                return None;
            }
            *flag = true;
        }
        Some(chord)
    }

    /// How many modifiers the chord uses. The front end tries chords with more modifiers
    /// first, because its key matching ignores extra Shift and Alt.
    pub fn modifier_count(&self) -> usize {
        usize::from(self.ctrl) + usize::from(self.alt) + usize::from(self.shift)
    }
}

fn is_key(key: &str) -> bool {
    let single = |pred: fn(&u8) -> bool| key.len() == 1 && key.as_bytes().first().is_some_and(pred);
    let function = key
        .strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (1..=12).contains(&n));
    single(u8::is_ascii_uppercase)
        || single(u8::is_ascii_digit)
        || function
        || NAMED_KEYS.contains(&key)
}

/// One shortcut: the chord, as written, and the command (with arguments) it runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub text: &'static str,
    pub chord: Chord,
    pub command: CommandId,
    pub args: Value,
}

/// Shortcuts that pass arguments: OneNote's Ctrl+2 to Ctrl+9 note tags (Ctrl+1 is To Do,
/// a command of its own). Tag ids match the ribbon's Tags gallery.
pub const WITH_ARGS: [(&str, CommandId, &str, &str); 8] = [
    ("Ctrl+2", ids::TAG_APPLY, "tag", "important"),
    ("Ctrl+3", ids::TAG_APPLY, "tag", "question"),
    ("Ctrl+4", ids::TAG_APPLY, "tag", "remember"),
    ("Ctrl+5", ids::TAG_APPLY, "tag", "definition"),
    ("Ctrl+6", ids::TAG_APPLY, "tag", "highlight"),
    ("Ctrl+7", ids::TAG_APPLY, "tag", "contact"),
    ("Ctrl+8", ids::TAG_APPLY, "tag", "address"),
    ("Ctrl+9", ids::TAG_APPLY, "tag", "phone"),
];

/// Every default shortcut, most modifiers first. A shortcut that does not parse is left out
/// (and fails the tests).
pub fn default_map() -> Vec<Binding> {
    let plain = builtin::SPECS.iter().filter_map(|spec| {
        let text = spec.default_shortcut?;
        Some(Binding {
            text,
            chord: Chord::parse(text)?,
            command: spec.id,
            args: Value::Null,
        })
    });
    let with_args = WITH_ARGS
        .iter()
        .filter_map(|&(text, command, name, value)| {
            let mut args = Map::new();
            args.insert(name.to_owned(), Value::from(value));
            Some(Binding {
                text,
                chord: Chord::parse(text)?,
                command,
                args: Value::Object(args),
            })
        });
    let mut map: Vec<Binding> = plain.chain(with_args).collect();
    map.sort_by_key(|binding| std::cmp::Reverse(binding.chord.modifier_count()));
    map
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::cmd::{CommandError, Registry};

    #[test]
    fn every_shortcut_parses() {
        let declared = builtin::SPECS
            .iter()
            .filter_map(|spec| spec.default_shortcut)
            .chain(WITH_ARGS.iter().map(|(text, ..)| *text));
        for text in declared {
            assert!(Chord::parse(text).is_some(), "{text} does not parse");
        }
        let declared_count = builtin::SPECS
            .iter()
            .filter(|spec| spec.default_shortcut.is_some())
            .count();
        assert_eq!(default_map().len(), declared_count + WITH_ARGS.len());
    }

    #[test]
    fn no_two_commands_share_a_shortcut() {
        let mut seen: BTreeMap<Chord, &str> = BTreeMap::new();
        for binding in default_map() {
            if let Some(other) = seen.insert(binding.chord, binding.text) {
                panic!("{} and {other} are the same chord", binding.text);
            }
        }
    }

    #[test]
    fn every_binding_runs_a_known_command_with_valid_arguments() {
        let registry = Registry::builtin();
        for binding in default_map() {
            match registry.validate(binding.command.as_str(), &binding.args) {
                Ok(()) => {}
                Err(err @ (CommandError::NotFound { .. } | CommandError::InvalidArgs { .. })) => {
                    panic!("{}: {err}", binding.text)
                }
                Err(_) => {}
            }
        }
    }

    #[test]
    fn onenote_defaults_are_bound() {
        let map = default_map();
        let command = |text: &str| {
            map.iter()
                .find(|b| b.text == text)
                .map(|b| b.command.as_str())
        };
        for (text, id) in [
            ("Ctrl+N", "page.new"),
            ("Ctrl+T", "section.new"),
            ("Ctrl+E", "search.all"),
            ("Ctrl+1", "tag.todo"),
            ("Ctrl+9", "tag.apply"),
            ("Ctrl+Alt+D", "view.dock"),
            ("Ctrl+F1", "view.ribbon.toggle"),
            ("Alt+Shift+D", "insert.date"),
            ("Alt+Shift+T", "insert.time"),
            ("F11", "view.full_page"),
            ("Ctrl+O", "file.open_notebook"),
        ] {
            assert_eq!(command(text), Some(id), "{text}");
        }
    }

    #[test]
    fn more_modifiers_come_first() {
        let counts: Vec<usize> = default_map()
            .iter()
            .map(|b| b.chord.modifier_count())
            .collect();
        assert!(counts.windows(2).all(|w| w[0] >= w[1]), "{counts:?}");
    }

    #[test]
    fn chord_parsing_edge_cases() {
        let chord = Chord::parse("Ctrl+Alt+-").unwrap();
        assert!(chord.ctrl && chord.alt && !chord.shift);
        assert_eq!(chord.key, "-");
        assert_eq!(Chord::parse("F11").map(|c| c.modifier_count()), Some(0));
        for bad in [
            "",
            "Ctrl+",
            "Ctrl+Ctrl+A",
            "Meta+A",
            "Ctrl+a",
            "Ctrl+F13",
            "Ctrl+F0",
            "Ctrl+Space",
            "A+Ctrl",
            "Ctrl++",
            "Ctrl+AB",
        ] {
            assert_eq!(Chord::parse(bad), None, "{bad:?}");
        }
    }
}
