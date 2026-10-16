//! `notecraft-cli`: NoteCraft from the command line, as the related craft apps have one. It
//! runs the same engine and command registry as the desktop app, without a window, for
//! scripts, CI, and agents (docs/cli.md). Subcommands that need notebooks on disk (info, text,
//! check, render, export, search) arrive with the engine work they use.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]

use std::io::Write;
use std::process::ExitCode;

use notecraft_engine::cmd::{ArgKind, CommandError, Registry, Status, Value};
use notecraft_engine::Session;

const USAGE: &str = "\
notecraft-cli - NoteCraft from the command line

USAGE:
  notecraft-cli commands [--json]           list every command: id, label, status, arguments
  notecraft-cli run [--sample] --cmd 'id={json}' [--cmd ...]
                                            run commands headlessly, in order, on an empty
                                            session (or the sample notebooks with --sample);
                                            prints one JSON result per command
  notecraft-cli [<command>] -h | --help     this help
  notecraft-cli --version

EXIT CODES:
  0 success, 1 error, 2 bad arguments, 3 unknown command, 4 command is Coming Soon,
  6 input too large
";

/// The subcommands, in `USAGE` order.
const SUBCOMMANDS: &[&str] = &["commands", "run"];

/// Longest `--cmd` value accepted, in bytes (Windows caps a whole command line near 32 KB).
const MAX_CMD_BYTES: usize = 16 * 1024;
/// Most `--cmd` values in one run.
const MAX_CMDS: usize = 1000;

/// Process exit codes (docs/cli.md).
mod code {
    pub const ERROR: u8 = 1;
    pub const BAD_ARGS: u8 = 2;
    pub const NOT_FOUND: u8 = 3;
    pub const COMING_SOON: u8 = 4;
    pub const TOO_LARGE: u8 = 6;
}

/// "notecraft-cli <version> (<sha>, <date>)", as printed by --version.
fn version_line() -> String {
    format!(
        "notecraft-cli {} ({}, {})",
        env!("CARGO_PKG_VERSION"),
        env!("NOTECRAFT_BUILD_SHA"),
        env!("NOTECRAFT_BUILD_DATE")
    )
}

// Write errors (a closed pipe) are not worth crashing over; exit codes still tell the story.
fn out(text: &str) {
    let _ = writeln!(std::io::stdout(), "{text}");
}

fn err(text: &str) {
    let _ = writeln!(std::io::stderr(), "{text}");
}

/// A usage error: the message, then the help, and exit code 2.
fn bad_args(message: &str) -> ExitCode {
    err(&format!("notecraft-cli: {message}\n"));
    let _ = write!(std::io::stderr(), "{USAGE}");
    ExitCode::from(code::BAD_ARGS)
}

fn main() -> ExitCode {
    // Arguments that are not valid UTF-8 are reported, not a crash.
    let mut args = Vec::new();
    for arg in std::env::args_os().skip(1) {
        match arg.into_string() {
            Ok(arg) => args.push(arg),
            Err(arg) => return bad_args(&format!("argument is not valid text: {arg:?}")),
        }
    }
    let Some((first, rest)) = args.split_first() else {
        let _ = write!(std::io::stdout(), "{USAGE}");
        return ExitCode::SUCCESS;
    };
    let wants_help = rest.iter().any(|a| a == "-h" || a == "--help");
    match first.as_str() {
        "--version" | "-V" => {
            out(&version_line());
            ExitCode::SUCCESS
        }
        "--help" | "-h" => {
            let _ = write!(std::io::stdout(), "{USAGE}");
            ExitCode::SUCCESS
        }
        cmd if SUBCOMMANDS.contains(&cmd) && wants_help => {
            let _ = write!(std::io::stdout(), "{USAGE}");
            ExitCode::SUCCESS
        }
        "commands" => commands(rest),
        "run" => run(rest),
        other => bad_args(&format!("unknown command `{other}`")),
    }
}

fn kind_name(kind: ArgKind) -> &'static str {
    match kind {
        ArgKind::Bool => "bool",
        ArgKind::Integer => "integer",
        ArgKind::Number => "number",
        ArgKind::String => "string",
        ArgKind::Array => "array",
    }
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Ready => "ready",
        Status::ComingSoon => "coming_soon",
    }
}

/// `commands [--json]`: every command, sorted by id.
fn commands(rest: &[String]) -> ExitCode {
    let mut json = false;
    for arg in rest {
        match arg.as_str() {
            "--json" => json = true,
            other => return bad_args(&format!("unknown option `{other}` for commands")),
        }
    }
    let registry = Registry::builtin();
    if json {
        let list: Vec<Value> = registry
            .specs()
            .map(|spec| {
                let args: Vec<Value> = spec
                    .args_schema
                    .iter()
                    .map(|arg| {
                        serde_json::json!({
                            "name": arg.name,
                            "type": kind_name(arg.kind),
                            "required": arg.required,
                            "description": arg.description,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "id": spec.id.as_str(),
                    "label": spec.label,
                    "description": spec.description,
                    "status": status_name(spec.status),
                    "shortcut": spec.default_shortcut,
                    "undoable": spec.undoable,
                    "args": args,
                })
            })
            .collect();
        out(&Value::Array(list).to_string());
    } else {
        for spec in registry.specs() {
            let args: Vec<String> = spec
                .args_schema
                .iter()
                .map(|arg| {
                    let name = format!("{}: {}", arg.name, kind_name(arg.kind));
                    if arg.required {
                        name
                    } else {
                        format!("[{name}]")
                    }
                })
                .collect();
            out(&format!(
                "{:<28} {:<12} {}{}",
                spec.id.as_str(),
                status_name(spec.status),
                spec.label,
                if args.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", args.join(", "))
                }
            ));
        }
    }
    ExitCode::SUCCESS
}

/// Split `id={json}` (or a bare `id`) into the id and its arguments.
fn parse_cmd(text: &str) -> Result<(String, Value), String> {
    let (id, args) = match text.split_once('=') {
        Some((id, json)) => {
            let args: Value = serde_json::from_str(json)
                .map_err(|e| format!("arguments of `{id}` are not JSON: {e}"))?;
            (id, args)
        }
        None => (text, Value::Null),
    };
    let id = id.trim();
    if id.is_empty() {
        return Err("a --cmd has no command id".to_owned());
    }
    Ok((id.to_owned(), args))
}

/// The exit code for a command that did not run.
fn error_code(error: &CommandError) -> u8 {
    match error {
        CommandError::NotFound { .. } => code::NOT_FOUND,
        CommandError::ComingSoon { .. } => code::COMING_SOON,
        CommandError::InvalidArgs { .. } => code::BAD_ARGS,
        CommandError::Internal { .. } => code::ERROR,
    }
}

/// `run [--sample] --cmd 'id={json}' ...`: run each command in order and print its result
/// as one JSON line. Stops at the first command that does not run.
fn run(rest: &[String]) -> ExitCode {
    let mut sample = false;
    let mut cmds = Vec::new();
    let mut args = rest.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sample" => sample = true,
            "--cmd" => {
                let Some(value) = args.next() else {
                    return bad_args("--cmd needs a value: 'id={json}'");
                };
                if value.len() > MAX_CMD_BYTES {
                    err(&format!(
                        "notecraft-cli: a --cmd is over {MAX_CMD_BYTES} bytes"
                    ));
                    return ExitCode::from(code::TOO_LARGE);
                }
                if cmds.len() >= MAX_CMDS {
                    err(&format!("notecraft-cli: more than {MAX_CMDS} --cmd values"));
                    return ExitCode::from(code::TOO_LARGE);
                }
                match parse_cmd(value) {
                    Ok(cmd) => cmds.push(cmd),
                    Err(message) => return bad_args(&message),
                }
            }
            other => return bad_args(&format!("unknown option `{other}` for run")),
        }
    }
    if cmds.is_empty() {
        return bad_args("run needs at least one --cmd");
    }
    let registry = Registry::builtin();
    let mut session = Session::new();
    if sample {
        session.set_notebooks(notecraft_engine::mock::notebooks());
    }
    for (id, args) in cmds {
        match registry.run(&mut session, &id, args) {
            Ok(output) => {
                out(&serde_json::json!({ "id": id, "ok": true, "value": output.value }).to_string())
            }
            Err(error) => {
                out(&serde_json::json!({
                    "id": id,
                    "ok": false,
                    "error": error.code(),
                    "message": error.to_string(),
                })
                .to_string());
                return ExitCode::from(error_code(&error));
            }
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_values_split_into_id_and_json() {
        assert_eq!(
            parse_cmd(r#"view.zoom={"percent":150}"#),
            Ok((
                "view.zoom".to_owned(),
                serde_json::json!({ "percent": 150 })
            ))
        );
        assert_eq!(
            parse_cmd("view.zoom_in"),
            Ok(("view.zoom_in".to_owned(), Value::Null))
        );
        assert!(parse_cmd("view.zoom={").is_err());
        assert!(parse_cmd("={}").is_err());
    }
}
