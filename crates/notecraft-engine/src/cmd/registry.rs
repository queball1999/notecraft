//! The command registry: looks commands up by id, checks their arguments, and runs them.

use std::collections::BTreeMap;
use std::fmt;

use super::{
    builtin, guard, CommandError, CommandId, CommandOutput, CommandSpec, Handler, Map, Status,
    Value,
};
use crate::session::Session;

/// Longest unknown id echoed back in [`CommandError::NotFound`]; ids come from outside.
const MAX_ECHOED_ID: usize = 128;

/// Why [`Registry::register`] refused a command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegisterError {
    /// The id does not match `^[a-z]+(\.[a-z_]+)+$`.
    InvalidId(CommandId),
    /// The label is empty.
    MissingLabel(CommandId),
    /// Another command already has this id.
    Duplicate(CommandId),
    /// The command is Ready but has no handler.
    MissingHandler(CommandId),
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(id) => write!(f, "invalid command id: {id}"),
            Self::MissingLabel(id) => write!(f, "command has no label: {id}"),
            Self::Duplicate(id) => write!(f, "command registered twice: {id}"),
            Self::MissingHandler(id) => write!(f, "ready command has no handler: {id}"),
        }
    }
}

impl std::error::Error for RegisterError {}

#[derive(Debug)]
struct Entry {
    spec: CommandSpec,
    handler: Option<Handler>,
}

/// Every command the app knows, by id.
#[derive(Debug, Default)]
pub struct Registry {
    entries: BTreeMap<&'static str, Entry>,
}

impl Registry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry with every built-in command.
    pub fn builtin() -> Self {
        let mut registry = Self::new();
        for spec in builtin::SPECS {
            if let Err(err) = registry.register(*spec, builtin::handler(spec.id)) {
                tracing::error!("built-in command skipped: {err}");
            }
        }
        registry
    }

    /// Add a command. A Ready command needs a handler; a Coming Soon command's handler is
    /// ignored.
    pub fn register(
        &mut self,
        spec: CommandSpec,
        handler: Option<Handler>,
    ) -> Result<(), RegisterError> {
        if !is_valid_id(spec.id.as_str()) {
            return Err(RegisterError::InvalidId(spec.id));
        }
        if spec.label.trim().is_empty() {
            return Err(RegisterError::MissingLabel(spec.id));
        }
        if self.entries.contains_key(spec.id.as_str()) {
            return Err(RegisterError::Duplicate(spec.id));
        }
        if spec.status == Status::Ready && handler.is_none() {
            return Err(RegisterError::MissingHandler(spec.id));
        }
        self.entries
            .insert(spec.id.as_str(), Entry { spec, handler });
        Ok(())
    }

    /// The spec of a command, if one has this id.
    pub fn get(&self, id: &str) -> Option<&CommandSpec> {
        self.entries.get(id).map(|entry| &entry.spec)
    }

    /// Every spec, sorted by id.
    pub fn specs(&self) -> impl Iterator<Item = &CommandSpec> {
        self.entries.values().map(|entry| &entry.spec)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Check that `id` exists and `args` fit its schema, without running it. A Coming Soon
    /// command is reported as such, after the argument check.
    pub fn validate(&self, id: &str, args: &Value) -> Result<(), CommandError> {
        let Some(entry) = self.entries.get(id) else {
            return Err(CommandError::NotFound {
                id: id.chars().take(MAX_ECHOED_ID).collect(),
            });
        };
        check_args(&entry.spec, args.clone())?;
        match (entry.spec.status, entry.handler) {
            (Status::Ready, Some(_)) => Ok(()),
            _ => Err(CommandError::ComingSoon {
                id: entry.spec.id,
                label: entry.spec.label,
            }),
        }
    }

    /// Run a command. `args` is a JSON object (or null for no arguments). Unknown ids, Coming
    /// Soon commands, and bad arguments are errors. The handler runs inside the command
    /// guard, so a panic or failure leaves `session` as it was.
    pub fn run(
        &self,
        session: &mut Session,
        id: &str,
        args: Value,
    ) -> Result<CommandOutput, CommandError> {
        let Some(entry) = self.entries.get(id) else {
            return Err(CommandError::NotFound {
                id: id.chars().take(MAX_ECHOED_ID).collect(),
            });
        };
        let spec = &entry.spec;
        let handler = match (spec.status, entry.handler) {
            (Status::Ready, Some(handler)) => handler,
            _ => {
                return Err(CommandError::ComingSoon {
                    id: spec.id,
                    label: spec.label,
                })
            }
        };
        let args = check_args(spec, args)?;
        guard::run_guarded(spec.id, session, |session| {
            let output = handler(session, &args)?;
            // Whatever the command changed (a page picked, a notebook opened), the page
            // shown now gets its text from disk if it has not been read yet.
            session.load_current_page();
            Ok(output)
        })
    }
}

/// Check `args` against the spec: an object (or null), no unknown names, every required
/// argument present, every value of the declared kind.
fn check_args(spec: &CommandSpec, args: Value) -> Result<Map<String, Value>, CommandError> {
    let invalid = |reason: String| CommandError::InvalidArgs {
        id: spec.id,
        reason,
    };
    let args = match args {
        Value::Null => Map::new(),
        Value::Object(map) => map,
        _ => return Err(invalid("arguments must be a JSON object".to_owned())),
    };
    for (name, value) in &args {
        let Some(arg) = spec.args_schema.iter().find(|arg| arg.name == name) else {
            let name: String = name.chars().take(MAX_ECHOED_ID).collect();
            return Err(invalid(format!("unknown argument: {name}")));
        };
        if !arg.kind.accepts(value) {
            return Err(invalid(format!(
                "argument {} must be {:?}",
                arg.name, arg.kind
            )));
        }
    }
    for arg in spec.args_schema {
        if arg.required && !args.contains_key(arg.name) {
            return Err(invalid(format!("missing argument: {}", arg.name)));
        }
    }
    Ok(args)
}

/// Whether `id` matches `^[a-z]+(\.[a-z_]+)+$`: a lowercase area, then one or more dotted
/// parts of lowercase letters and underscores.
pub fn is_valid_id(id: &str) -> bool {
    let mut parts = id.split('.');
    let area_ok = parts
        .next()
        .is_some_and(|area| !area.is_empty() && area.bytes().all(|b| b.is_ascii_lowercase()));
    let mut rest = 0;
    let rest_ok = parts.all(|part| {
        rest += 1;
        !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
    });
    area_ok && rest_ok && rest > 0
}
