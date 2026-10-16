//! Tests for the command ids, the registry, and the panic guard.

use std::collections::BTreeSet;

use serde_json::json;

use super::*;

// Ids

#[test]
fn every_id_is_unique() {
    let mut seen = BTreeSet::new();
    for id in ids::ALL {
        assert!(seen.insert(id.as_str()), "duplicate id {id}");
    }
    assert_eq!(Registry::builtin().len(), ids::ALL.len());
}

#[test]
fn every_id_matches_the_pattern() {
    for id in ids::ALL {
        assert!(is_valid_id(id.as_str()), "bad id {id}");
    }
}

#[test]
fn every_id_has_a_spec_and_every_spec_has_an_id() {
    let declared: BTreeSet<&str> = ids::ALL.iter().map(|id| id.as_str()).collect();
    let specced: BTreeSet<&str> = builtin::SPECS.iter().map(|spec| spec.id.as_str()).collect();
    assert_eq!(declared, specced);
    assert_eq!(
        builtin::SPECS.len(),
        specced.len(),
        "a spec is listed twice"
    );
}

#[test]
fn every_spec_has_a_label_and_description() {
    for spec in builtin::SPECS {
        assert!(!spec.label.trim().is_empty(), "{} has no label", spec.id);
        assert!(
            spec.description.ends_with('.'),
            "{} needs a one-sentence description",
            spec.id
        );
        for arg in spec.args_schema {
            assert!(!arg.name.is_empty() && !arg.description.is_empty());
        }
    }
}

#[test]
fn ready_commands_and_handlers_match() {
    let ready: BTreeSet<&str> = builtin::SPECS
        .iter()
        .filter(|spec| spec.status == Status::Ready)
        .map(|spec| spec.id.as_str())
        .collect();
    let handled: BTreeSet<&str> = builtin::handled_ids().map(CommandId::as_str).collect();
    assert_eq!(ready, handled);
    assert!(ready.contains("view.theme.set"));
}

#[test]
fn default_shortcuts_do_not_clash() {
    let mut seen = BTreeSet::new();
    for spec in builtin::SPECS {
        if let Some(shortcut) = spec.default_shortcut {
            assert!(seen.insert(shortcut), "{shortcut} is bound twice");
        }
    }
}

#[test]
fn ribbon_tabs_are_covered() {
    // Every OneNote ribbon tab NoteCraft has, and nothing from tabs it does not.
    let areas: BTreeSet<&str> = ids::ALL
        .iter()
        .filter_map(|id| id.as_str().split('.').next())
        .collect();
    for area in [
        "file", "edit", "format", "insert", "draw", "history", "review", "view", "help",
    ] {
        assert!(areas.contains(area), "no commands for {area}");
    }
    assert!(Registry::builtin().len() >= 100);
}

#[test]
fn id_pattern_edge_cases() {
    for good in [
        "a.b",
        "edit.paste",
        "view.theme.set",
        "draw.ink_to_text",
        "a._",
    ] {
        assert!(is_valid_id(good), "{good} should be valid");
    }
    for bad in [
        "",
        "edit",
        "edit.",
        ".paste",
        "edit..paste",
        "Edit.paste",
        "edit.Paste",
        "edit.paste1",
        "edit_x.paste",
        "edit.paste ",
        "edit-paste.x",
        "\u{e9}dit.paste",
    ] {
        assert!(!is_valid_id(bad), "{bad:?} should be invalid");
    }
}

// Registry

const TEST_BUMP: CommandId = CommandId::new("test.bump");
const TEST_PANIC: CommandId = CommandId::new("test.panic");
const TEST_FAIL: CommandId = CommandId::new("test.fail");
const TEST_ARGS: CommandId = CommandId::new("test.args");

const BUMP_ARGS: &[ArgSpec] = &[
    ArgSpec::required("times", ArgKind::Integer, "How many changes."),
    ArgSpec::optional("note", ArgKind::String, "Ignored."),
];

fn bump(session: &mut Session, args: &Map<String, Value>) -> Result<CommandOutput, CommandError> {
    let times = args.get("times").and_then(Value::as_u64).unwrap_or(1);
    for _ in 0..times {
        session.mark_changed();
    }
    Ok(CommandOutput {
        value: json!(session.revision()),
    })
}

fn change_then_panic(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    session.mark_changed();
    panic!("test command panicked on purpose");
}

fn change_then_fail(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    session.mark_changed();
    Err(CommandError::InvalidArgs {
        id: TEST_FAIL,
        reason: "refused".to_owned(),
    })
}

fn test_registry() -> Registry {
    let mut registry = Registry::builtin();
    let specs: [(CommandSpec, Handler); 4] = [
        (
            CommandSpec::new(TEST_BUMP, "Bump", "Change the session.").ready(),
            bump,
        ),
        (
            CommandSpec::new(TEST_PANIC, "Panic", "Change the session, then panic.").ready(),
            change_then_panic,
        ),
        (
            CommandSpec::new(TEST_FAIL, "Fail", "Change the session, then fail.").ready(),
            change_then_fail,
        ),
        (
            CommandSpec::new(TEST_ARGS, "Args", "Change the session with arguments.")
                .args(BUMP_ARGS)
                .ready(),
            bump,
        ),
    ];
    for (spec, handler) in specs {
        assert_eq!(registry.register(spec, Some(handler)), Ok(()));
    }
    registry
}

#[test]
fn ready_command_runs() {
    let registry = test_registry();
    let mut session = Session::new();
    let out = registry.run(&mut session, "test.bump", Value::Null);
    assert_eq!(out, Ok(CommandOutput { value: json!(1) }));
    assert_eq!(session.revision(), 1);
}

#[test]
fn unknown_id_is_an_error() {
    let registry = test_registry();
    let mut session = Session::new();
    let err = registry.run(&mut session, "no.such_command", Value::Null);
    assert_eq!(
        err,
        Err(CommandError::NotFound {
            id: "no.such_command".to_owned()
        })
    );
    assert_eq!(err.map_err(|e| e.code()), Err("not_found"));
    assert_eq!(session, Session::new());
}

#[test]
fn unknown_id_is_truncated_in_the_error() {
    let registry = test_registry();
    let long = "x".repeat(10_000);
    match registry.run(&mut Session::new(), &long, Value::Null) {
        Err(CommandError::NotFound { id }) => assert_eq!(id.len(), 128),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn coming_soon_is_an_error_with_the_label() {
    let registry = test_registry();
    let mut session = Session::new();
    let coming_soon = builtin::SPECS
        .iter()
        .filter(|spec| spec.status == Status::ComingSoon);
    for spec in coming_soon {
        let err = registry.run(&mut session, spec.id.as_str(), Value::Null);
        assert_eq!(
            err,
            Err(CommandError::ComingSoon {
                id: spec.id,
                label: spec.label
            })
        );
    }
    assert_eq!(session, Session::new());
}

#[test]
fn arguments_are_checked() {
    let registry = test_registry();
    let mut session = Session::new();
    let bad = [
        json!([1, 2]),
        json!("times"),
        json!({}),
        json!({ "times": "two" }),
        json!({ "times": 1.5 }),
        json!({ "times": 1, "extra": true }),
        json!({ "times": 1, "note": 7 }),
    ];
    for args in bad {
        let err = registry.run(&mut session, "test.args", args.clone());
        assert!(
            matches!(err, Err(CommandError::InvalidArgs { .. })),
            "{args} should be rejected, got {err:?}"
        );
    }
    assert_eq!(session.revision(), 0);

    let ok = registry.run(
        &mut session,
        "test.args",
        json!({ "times": 2, "note": "hi" }),
    );
    assert_eq!(ok, Ok(CommandOutput { value: json!(2) }));
}

#[test]
fn register_rejects_bad_specs() {
    let mut registry = Registry::new();
    let bad_id = CommandSpec::new(CommandId::new("Bad.id"), "Bad", "Bad.");
    assert_eq!(
        registry.register(bad_id, None),
        Err(RegisterError::InvalidId(bad_id.id))
    );
    let no_label = CommandSpec::new(TEST_BUMP, " ", "No label.");
    assert_eq!(
        registry.register(no_label, None),
        Err(RegisterError::MissingLabel(TEST_BUMP))
    );
    let ready = CommandSpec::new(TEST_BUMP, "Bump", "Bump.").ready();
    assert_eq!(
        registry.register(ready, None),
        Err(RegisterError::MissingHandler(TEST_BUMP))
    );
    assert_eq!(registry.register(ready, Some(bump)), Ok(()));
    assert_eq!(
        registry.register(ready, Some(bump)),
        Err(RegisterError::Duplicate(TEST_BUMP))
    );
    assert_eq!(registry.len(), 1);
}

// Panic guard

#[test]
fn panicking_command_is_caught_and_the_session_is_unchanged() {
    let registry = test_registry();
    let mut session = Session::new();
    registry
        .run(&mut session, "test.bump", Value::Null)
        .unwrap();
    let before = session.clone();

    let err = registry.run(&mut session, "test.panic", Value::Null);
    match err {
        Err(CommandError::Internal { id, message }) => {
            assert_eq!(id, TEST_PANIC);
            assert_eq!(message, "test command panicked on purpose");
        }
        other => panic!("expected Internal, got {other:?}"),
    }
    assert_eq!(session, before);

    // The registry and session keep working after a caught panic.
    registry
        .run(&mut session, "test.bump", Value::Null)
        .unwrap();
    assert_eq!(session.revision(), 2);
}

#[test]
fn failing_command_is_rolled_back() {
    let registry = test_registry();
    let mut session = Session::new();
    let err = registry.run(&mut session, "test.fail", Value::Null);
    assert!(matches!(err, Err(CommandError::InvalidArgs { .. })));
    assert_eq!(session.revision(), 0);
}

#[test]
fn error_codes_are_stable() {
    let cases = [
        (CommandError::NotFound { id: String::new() }, "not_found"),
        (
            CommandError::ComingSoon {
                id: TEST_BUMP,
                label: "Bump",
            },
            "coming_soon",
        ),
        (
            CommandError::InvalidArgs {
                id: TEST_BUMP,
                reason: String::new(),
            },
            "invalid_args",
        ),
        (
            CommandError::Internal {
                id: TEST_BUMP,
                message: String::new(),
            },
            "internal",
        ),
    ];
    for (err, code) in cases {
        assert_eq!(err.code(), code);
        assert!(!err.to_string().is_empty());
    }
}
