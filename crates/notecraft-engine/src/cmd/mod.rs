//! Commands. Every user action is a command with a stable id (AGENTS.md section 4,
//! docs/architecture.md section 4). The ribbon, shortcuts, and the UI test control channel
//! all dispatch by id through [`Registry::run`].

mod builtin;
mod guard;
pub mod ids;
mod layout;
mod registry;
#[cfg(test)]
mod tests;
mod theme;

use std::fmt;

pub use registry::{is_valid_id, RegisterError, Registry};
pub use serde_json::{Map, Value};

use crate::session::Session;

/// A stable command id such as `edit.paste`. The constants live in [`ids`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandId(&'static str);

impl CommandId {
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Whether a command does anything yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Implemented; running it calls its handler.
    Ready,
    /// Not implemented yet. The UI shows a badge and a "Coming Soon" toast, and
    /// [`Registry::run`] returns [`CommandError::ComingSoon`].
    ComingSoon,
}

/// The JSON type of a command argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    Bool,
    Integer,
    Number,
    String,
}

impl ArgKind {
    fn accepts(self, value: &Value) -> bool {
        match self {
            Self::Bool => value.is_boolean(),
            Self::Integer => value.is_i64() || value.is_u64(),
            Self::Number => value.is_number(),
            Self::String => value.is_string(),
        }
    }
}

/// One named argument of a command. Arguments are passed as a JSON object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArgSpec {
    pub name: &'static str,
    pub kind: ArgKind,
    pub required: bool,
    pub description: &'static str,
}

impl ArgSpec {
    pub const fn required(name: &'static str, kind: ArgKind, description: &'static str) -> Self {
        Self {
            name,
            kind,
            required: true,
            description,
        }
    }

    pub const fn optional(name: &'static str, kind: ArgKind, description: &'static str) -> Self {
        Self {
            name,
            kind,
            required: false,
            description,
        }
    }
}

/// Everything the UI and the control channel need to know about a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    pub id: CommandId,
    /// Short UI label, as on the ribbon button.
    pub label: &'static str,
    /// One sentence for tooltips.
    pub description: &'static str,
    pub args_schema: &'static [ArgSpec],
    /// Whether running it adds an undo step.
    pub undoable: bool,
    /// Whether it deletes user content.
    pub destructive: bool,
    /// Default key binding, for example `Ctrl+Shift+C`.
    pub default_shortcut: Option<&'static str>,
    pub status: Status,
}

impl CommandSpec {
    /// A Coming Soon command with no arguments, not undoable, not destructive, no shortcut.
    pub const fn new(id: CommandId, label: &'static str, description: &'static str) -> Self {
        Self {
            id,
            label,
            description,
            args_schema: &[],
            undoable: false,
            destructive: false,
            default_shortcut: None,
            status: Status::ComingSoon,
        }
    }

    pub const fn args(mut self, args_schema: &'static [ArgSpec]) -> Self {
        self.args_schema = args_schema;
        self
    }

    pub const fn undoable(mut self) -> Self {
        self.undoable = true;
        self
    }

    pub const fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    pub const fn shortcut(mut self, shortcut: &'static str) -> Self {
        self.default_shortcut = Some(shortcut);
        self
    }

    pub const fn ready(mut self) -> Self {
        self.status = Status::Ready;
        self
    }
}

/// What a command returns on success. Most commands return `Value::Null`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandOutput {
    pub value: Value,
}

/// Runs a Ready command. Arguments have already been checked against the spec.
pub type Handler = fn(&mut Session, &Map<String, Value>) -> Result<CommandOutput, CommandError>;

/// Why a command did not run. [`CommandError::code`] gives the stable string code used by the
/// control channel (docs/control-protocol.md section 4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandError {
    /// No command has this id.
    NotFound { id: String },
    /// The command is registered but not implemented yet.
    ComingSoon { id: CommandId, label: &'static str },
    /// The arguments do not match the command's schema.
    InvalidArgs { id: CommandId, reason: String },
    /// The command panicked. The session was left as it was before the command.
    Internal { id: CommandId, message: String },
}

impl CommandError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "not_found",
            Self::ComingSoon { .. } => "coming_soon",
            Self::InvalidArgs { .. } => "invalid_args",
            Self::Internal { .. } => "internal",
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { id } => write!(f, "unknown command: {id}"),
            Self::ComingSoon { id, label } => write!(f, "not implemented yet: {label} ({id})"),
            Self::InvalidArgs { id, reason } => write!(f, "invalid arguments for {id}: {reason}"),
            Self::Internal { id, message } => write!(f, "internal error in {id}: {message}"),
        }
    }
}

impl std::error::Error for CommandError {}
