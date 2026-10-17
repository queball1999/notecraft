//! Keyboard and IME input for text editing, mapped to commands (PR 1.5). egui only reports
//! events; what each key does is a command with a stable id (AGENTS.md section 4), so the map
//! here is a plain function from events to actions and has no state.
//!
//! Keys that need the laid-out text (Up, Down, Home, End follow wrapped rows) come back as
//! [`Action`]s the canvas finishes with the raw region it knows.

use egui::{Event, ImeEvent, Key, Modifiers};
use notecraft_engine::cmd::{ids, CommandId, Value};

use crate::ribbon::Clicked;

/// What an input event asks for.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Action {
    Command(Clicked),
    /// Up or Down by one visual row.
    Vertical {
        down: bool,
        extend: bool,
    },
    /// Home or End on the visual row.
    Edge {
        end: bool,
        extend: bool,
    },
    /// The IME composition changed; empty ends it.
    Preedit(String),
}

fn command(id: CommandId, args: &[(&str, Value)]) -> Action {
    let map = args
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect();
    Action::Command(Clicked {
        command: id,
        args: Value::Object(map),
    })
}

fn word_modifier(modifiers: Modifiers) -> bool {
    modifiers.command || (cfg!(target_os = "macos") && modifiers.alt)
}

fn movement(motion: &str, extend: bool) -> Action {
    command(
        ids::TEXT_MOVE,
        &[
            ("motion", Value::from(motion)),
            ("extend", Value::from(extend)),
        ],
    )
}

fn key_action(key: Key, modifiers: Modifiers) -> Option<Action> {
    let shift = modifiers.shift;
    let word = word_modifier(modifiers);
    Some(match key {
        Key::Enter => command(ids::TEXT_ENTER, &[("soft", Value::from(shift))]),
        Key::Tab => command(ids::TEXT_TAB, &[("shift", Value::from(shift))]),
        Key::Backspace => delete("backward", word),
        Key::Delete => delete("forward", word),
        Key::ArrowLeft => movement(if word { "word_left" } else { "left" }, shift),
        Key::ArrowRight => movement(if word { "word_right" } else { "right" }, shift),
        Key::ArrowUp => Action::Vertical {
            down: false,
            extend: shift,
        },
        Key::ArrowDown => Action::Vertical {
            down: true,
            extend: shift,
        },
        Key::Home if modifiers.command => movement("start", shift),
        Key::End if modifiers.command => movement("end", shift),
        Key::Home => Action::Edge {
            end: false,
            extend: shift,
        },
        Key::End => Action::Edge {
            end: true,
            extend: shift,
        },
        _ => return None,
    })
}

fn delete(direction: &str, word: bool) -> Action {
    let unit = if word { "word" } else { "char" };
    command(
        ids::TEXT_DELETE,
        &[
            ("direction", Value::from(direction)),
            ("unit", Value::from(unit)),
        ],
    )
}

/// The actions for this frame's input events, in order.
pub(super) fn actions(events: &[Event]) -> Vec<Action> {
    let mut out = Vec::new();
    for event in events {
        match event {
            Event::Text(text) => {
                let typed: String = text.chars().filter(|c| !c.is_control()).collect();
                if !typed.is_empty() {
                    out.push(command(
                        ids::TEXT_INSERT,
                        &[("text", Value::from(typed)), ("typing", Value::from(true))],
                    ));
                }
            }
            Event::Ime(ImeEvent::Commit(text)) => {
                out.push(Action::Preedit(String::new()));
                if !text.is_empty() {
                    out.push(command(
                        ids::TEXT_INSERT,
                        &[("text", Value::from(text.as_str()))],
                    ));
                }
            }
            Event::Ime(ImeEvent::Preedit { text, .. }) => out.push(Action::Preedit(text.clone())),
            Event::Copy => out.push(command(ids::EDIT_COPY, &[])),
            Event::Cut => out.push(command(ids::EDIT_CUT, &[])),
            Event::Paste(text) => out.push(command(
                ids::EDIT_PASTE,
                &[("text", Value::from(text.as_str()))],
            )),
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => out.extend(key_action(*key, *modifiers)),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn only(events: &[Event]) -> Action {
        let mut found = actions(events);
        assert_eq!(found.len(), 1, "{found:?}");
        found.remove(0)
    }

    fn id_and_args(action: &Action) -> (&'static str, &Value) {
        match action {
            Action::Command(c) => (c.command.as_str(), &c.args),
            other => panic!("not a command: {other:?}"),
        }
    }

    #[test]
    fn typed_text_becomes_a_typing_insert_without_control_characters() {
        let action = only(&[Event::Text("a\u{7}b".to_owned())]);
        let (id, args) = id_and_args(&action);
        assert_eq!(id, "text.insert");
        assert_eq!(args["text"], "ab");
        assert_eq!(args["typing"], true);
        assert!(actions(&[Event::Text("\r".to_owned())]).is_empty());
    }

    #[test]
    fn editing_keys_map_to_commands() {
        let none = Modifiers::NONE;
        let shift = Modifiers::SHIFT;
        let ctrl = Modifiers::COMMAND;
        let cases = [
            (Key::Enter, none, "text.enter", "soft", Value::from(false)),
            (Key::Enter, shift, "text.enter", "soft", Value::from(true)),
            (Key::Tab, shift, "text.tab", "shift", Value::from(true)),
            (
                Key::Backspace,
                none,
                "text.delete",
                "unit",
                Value::from("char"),
            ),
            (
                Key::Backspace,
                ctrl,
                "text.delete",
                "unit",
                Value::from("word"),
            ),
            (
                Key::Delete,
                none,
                "text.delete",
                "direction",
                Value::from("forward"),
            ),
            (
                Key::ArrowLeft,
                none,
                "text.move",
                "motion",
                Value::from("left"),
            ),
            (
                Key::ArrowRight,
                ctrl,
                "text.move",
                "motion",
                Value::from("word_right"),
            ),
            (
                Key::ArrowLeft,
                shift,
                "text.move",
                "extend",
                Value::from(true),
            ),
            (Key::Home, ctrl, "text.move", "motion", Value::from("start")),
            (
                Key::End,
                ctrl | shift,
                "text.move",
                "motion",
                Value::from("end"),
            ),
        ];
        for (k, m, id, arg, expected) in cases {
            let action = only(&[key(k, m)]);
            let (got_id, args) = id_and_args(&action);
            assert_eq!(got_id, id, "{k:?}");
            assert_eq!(args[arg], expected, "{k:?} {arg}");
        }
    }

    #[test]
    fn row_keys_are_left_for_the_canvas() {
        assert_eq!(
            only(&[key(Key::ArrowDown, Modifiers::SHIFT)]),
            Action::Vertical {
                down: true,
                extend: true
            }
        );
        assert_eq!(
            only(&[key(Key::Home, Modifiers::NONE)]),
            Action::Edge {
                end: false,
                extend: false
            }
        );
        assert!(actions(&[key(Key::F5, Modifiers::NONE)]).is_empty());
    }

    #[test]
    fn clipboard_events_map_to_the_edit_commands() {
        let (id, _) = id_and_args(&only(&[Event::Copy]));
        assert_eq!(id, "edit.copy");
        let (id, _) = id_and_args(&only(&[Event::Cut]));
        assert_eq!(id, "edit.cut");
        let action = only(&[Event::Paste("hi".to_owned())]);
        let (id, args) = id_and_args(&action);
        assert_eq!((id, &args["text"]), ("edit.paste", &Value::from("hi")));
    }

    #[test]
    fn ime_composition_previews_then_commits_as_one_insert() {
        let steps = actions(&[
            Event::Ime(ImeEvent::Preedit {
                text: "ni".to_owned(),
                active_range_chars: None,
            }),
            Event::Ime(ImeEvent::Commit("\u{4f60}".to_owned())),
        ]);
        assert_eq!(steps.len(), 3);
        assert_eq!(steps.first(), Some(&Action::Preedit("ni".to_owned())));
        assert_eq!(steps.get(1), Some(&Action::Preedit(String::new())));
        let (id, args) = id_and_args(steps.get(2).expect("insert"));
        assert_eq!(
            (id, &args["text"]),
            ("text.insert", &Value::from("\u{4f60}"))
        );
        // An empty commit only ends the composition.
        assert_eq!(
            actions(&[Event::Ime(ImeEvent::Commit(String::new()))]),
            vec![Action::Preedit(String::new())]
        );
    }

    #[test]
    fn key_releases_do_nothing() {
        let release = Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert!(actions(&[release]).is_empty());
    }
}
