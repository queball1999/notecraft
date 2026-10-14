//! Theme commands: `view.theme.set`, `view.mode.toggle`, `view.mode.set` (docs/themes.md
//! section 5). They change the session's view state; the front end turns it into colors.

use serde_json::json;

use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::session::Session;
use crate::view::{ThemeMode, ThemePreset};

/// The string argument `name`, already known to be present or absent by the schema check.
fn string_arg<'a>(args: &'a Map<String, Value>, name: &str) -> &'a str {
    args.get(name).and_then(Value::as_str).unwrap_or_default()
}

fn invalid(id: CommandId, what: &str, value: &str, expected: &[&str]) -> CommandError {
    let value: String = value.chars().take(64).collect();
    CommandError::InvalidArgs {
        id,
        reason: format!("unknown {what} {value:?}; expected {}", expected.join(", ")),
    }
}

/// `view.theme.set { preset }`
pub(super) fn theme_set(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = string_arg(args, "preset");
    let Some(preset) = ThemePreset::from_id(id) else {
        let expected = ThemePreset::ALL.map(ThemePreset::id);
        return Err(invalid(ids::VIEW_THEME_SET, "preset", id, &expected));
    };
    if session.view().theme_preset != preset {
        session.view_mut().theme_preset = preset;
        session.mark_changed();
    }
    Ok(CommandOutput {
        value: json!(preset.id()),
    })
}

/// `view.mode.set { mode }`
pub(super) fn mode_set(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = string_arg(args, "mode");
    let Some(mode) = ThemeMode::from_id(id) else {
        let expected = ThemeMode::ALL.map(ThemeMode::id);
        return Err(invalid(ids::VIEW_MODE_SET, "mode", id, &expected));
    };
    Ok(set_mode(session, mode))
}

/// `view.mode.toggle`: light becomes dark and dark becomes light. In system mode it switches
/// to the opposite of what the OS shows now.
pub(super) fn mode_toggle(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let mode = if session.view().is_dark() {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    };
    Ok(set_mode(session, mode))
}

fn set_mode(session: &mut Session, mode: ThemeMode) -> CommandOutput {
    if session.view().theme_mode != mode {
        session.view_mut().theme_mode = mode;
        session.mark_changed();
    }
    CommandOutput {
        value: json!(mode.id()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::cmd::{ids, CommandError, Registry};
    use crate::session::Session;
    use crate::view::{ThemeMode, ThemePreset};

    fn run(
        session: &mut Session,
        id: &str,
        args: serde_json::Value,
    ) -> Result<String, &'static str> {
        Registry::builtin()
            .run(session, id, args)
            .map(|out| out.value.as_str().unwrap_or_default().to_owned())
            .map_err(|err| err.code())
    }

    #[test]
    fn theme_set_switches_presets() {
        let mut session = Session::new();
        for preset in ThemePreset::ALL {
            let out = run(
                &mut session,
                "view.theme.set",
                json!({ "preset": preset.id() }),
            );
            assert_eq!(out.as_deref(), Ok(preset.id()));
            assert_eq!(session.view().theme_preset, preset);
        }
    }

    #[test]
    fn theme_set_rejects_unknown_presets() {
        let mut session = Session::new();
        let before = session.clone();
        for args in [
            json!({ "preset": "neon" }),
            json!({}),
            json!({ "preset": 2016 }),
        ] {
            assert_eq!(
                run(&mut session, "view.theme.set", args).as_deref(),
                Err(&"invalid_args")
            );
        }
        assert_eq!(session, before);

        let err = Registry::builtin().run(
            &mut session,
            ids::VIEW_THEME_SET.as_str(),
            json!({ "preset": "neon" }),
        );
        match err {
            Err(CommandError::InvalidArgs { reason, .. }) => {
                assert!(reason.contains("onenote2016"), "{reason}");
            }
            other => panic!("expected InvalidArgs, got {other:?}"),
        }
    }

    #[test]
    fn mode_set_and_toggle() {
        let mut session = Session::new();
        assert_eq!(
            run(&mut session, "view.mode.set", json!({ "mode": "dark" })).as_deref(),
            Ok("dark")
        );
        assert!(session.view().is_dark());
        assert_eq!(
            run(&mut session, "view.mode.toggle", json!(null)).as_deref(),
            Ok("light")
        );
        assert_eq!(session.view().theme_mode, ThemeMode::Light);
        assert_eq!(
            run(&mut session, "view.mode.toggle", json!(null)).as_deref(),
            Ok("dark")
        );
        assert_eq!(
            run(&mut session, "view.mode.set", json!({ "mode": "sepia" })).as_deref(),
            Err(&"invalid_args")
        );
        assert_eq!(session.view().theme_mode, ThemeMode::Dark);
    }

    #[test]
    fn toggle_from_system_goes_opposite_to_the_os() {
        for (os_dark, expected) in [(true, ThemeMode::Light), (false, ThemeMode::Dark)] {
            let mut session = Session::new();
            session.view_mut().system_dark = os_dark;
            run(&mut session, "view.mode.toggle", json!(null)).unwrap();
            assert_eq!(session.view().theme_mode, expected);
        }
    }

    #[test]
    fn setting_the_same_value_changes_nothing() {
        let mut session = Session::new();
        run(&mut session, "view.mode.set", json!({ "mode": "system" })).unwrap();
        run(&mut session, "view.theme.set", json!({ "preset": "m365" })).unwrap();
        assert_eq!(session.revision(), 0);
    }
}
