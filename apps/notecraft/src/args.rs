//! Command-line arguments: `notecraft [--version] [--help] [--control <port>] [<notebook folder or page file>]`.
//!
//! Parsing works on `OsString` so a path that is not valid UTF-8 is carried through intact.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Cli {
    /// Start the app, opening `open` first if given, and serving the control channel on
    /// `control` (a 127.0.0.1 port) if given.
    Launch {
        open: Option<PathBuf>,
        control: Option<u16>,
    },
    /// Print the version line and exit.
    Version,
    /// Print the usage text and exit.
    Help,
}

/// A command line the app does not accept.
#[derive(Debug, PartialEq, Eq)]
pub enum ArgError {
    UnknownFlag(String),
    TooManyPaths,
    /// `--control` was given no port.
    MissingPort,
    /// The `--control` value is not a port from 1 to 65535.
    BadPort(String),
}

impl std::fmt::Display for ArgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFlag(flag) => write!(f, "unknown option: {flag}"),
            Self::TooManyPaths => write!(f, "only one notebook folder or page file can be opened"),
            Self::MissingPort => write!(f, "--control needs a port number"),
            Self::BadPort(port) => write!(f, "not a port from 1 to 65535: {port}"),
        }
    }
}

/// The text printed by `--help` and after an argument error.
pub const USAGE: &str = "Usage: notecraft [OPTIONS] [PATH]

Open a notebook folder or a page file (.md, .notecraft) at launch.
With no PATH, the notebook or page open at the last exit reopens.

Options:
  -h, --help     Print this help and exit
  -V, --version  Print version and build info and exit
      --control PORT
                 Serve the UI test control channel on 127.0.0.1:PORT (off by default;
                 writes control.token in the settings folder)
";

/// Parse the arguments after the program name. `--help` and `--version` win over everything
/// else on the line; `--` ends the options so a path may start with a dash.
pub fn parse<I>(args: I) -> Result<Cli, ArgError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut path: Option<PathBuf> = None;
    let mut version = false;
    let mut help = false;
    let mut control = None;
    let mut options_done = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if !options_done {
            if arg == OsStr::new("--") {
                options_done = true;
                continue;
            }
            if arg == OsStr::new("--version") || arg == OsStr::new("-V") {
                version = true;
                continue;
            }
            if arg == OsStr::new("--help") || arg == OsStr::new("-h") {
                help = true;
                continue;
            }
            if arg == OsStr::new("--control") {
                let value = args.next().ok_or(ArgError::MissingPort)?;
                control = Some(parse_port(&value)?);
                continue;
            }
            if let Some(value) = arg.to_string_lossy().strip_prefix("--control=") {
                control = Some(parse_port(OsStr::new(value))?);
                continue;
            }
            if arg.to_string_lossy().starts_with('-') && arg != OsStr::new("-") {
                return Err(ArgError::UnknownFlag(arg.to_string_lossy().into_owned()));
            }
        }
        if path.is_some() {
            return Err(ArgError::TooManyPaths);
        }
        path = Some(PathBuf::from(arg));
    }
    Ok(if help {
        Cli::Help
    } else if version {
        Cli::Version
    } else {
        Cli::Launch {
            open: path,
            control,
        }
    })
}

/// A TCP port from 1 to 65535 (0 would mean "any", which a client could not find).
fn parse_port(value: &OsStr) -> Result<u16, ArgError> {
    let text = value.to_string_lossy();
    match text.parse::<u16>() {
        Ok(port) if port != 0 => Ok(port),
        _ => Err(ArgError::BadPort(text.chars().take(32).collect())),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Cli, ArgError> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn no_arguments_launches_plain() {
        assert_eq!(
            p(&[]),
            Ok(Cli::Launch {
                open: None,
                control: None
            })
        );
    }

    #[test]
    fn version_and_help_flags() {
        assert_eq!(p(&["--version"]), Ok(Cli::Version));
        assert_eq!(p(&["-V"]), Ok(Cli::Version));
        assert_eq!(p(&["--help"]), Ok(Cli::Help));
        assert_eq!(p(&["-h"]), Ok(Cli::Help));
        assert_eq!(p(&["-V", "--help"]), Ok(Cli::Help));
    }

    #[test]
    fn one_path_is_opened() {
        assert_eq!(
            p(&["Notes/Work"]),
            Ok(Cli::Launch {
                open: Some(PathBuf::from("Notes/Work")),
                control: None
            })
        );
    }

    #[test]
    fn double_dash_allows_a_dashed_path() {
        assert_eq!(
            p(&["--", "-odd"]),
            Ok(Cli::Launch {
                open: Some(PathBuf::from("-odd")),
                control: None
            })
        );
    }

    #[test]
    fn two_paths_are_an_error() {
        assert_eq!(p(&["a", "b"]), Err(ArgError::TooManyPaths));
    }

    #[test]
    fn unknown_flag_is_an_error() {
        assert_eq!(
            p(&["--wat"]),
            Err(ArgError::UnknownFlag("--wat".to_owned()))
        );
        assert!(p(&["a", "-x"]).is_err());
    }

    fn launch_control(args: &[&str]) -> Result<Option<u16>, ArgError> {
        match p(args)? {
            Cli::Launch { control, .. } => Ok(control),
            other => panic!("not a launch: {other:?}"),
        }
    }

    #[test]
    fn control_is_off_unless_given() {
        assert_eq!(launch_control(&[]), Ok(None));
        assert_eq!(launch_control(&["Notes"]), Ok(None));
    }

    #[test]
    fn control_takes_a_port() {
        assert_eq!(launch_control(&["--control", "47830"]), Ok(Some(47830)));
        assert_eq!(launch_control(&["--control=47830"]), Ok(Some(47830)));
        assert_eq!(launch_control(&["--control", "65535"]), Ok(Some(65535)));
        assert_eq!(launch_control(&["--control", "1"]), Ok(Some(1)));
        assert_eq!(
            p(&["--control", "9000", "Notes"]),
            Ok(Cli::Launch {
                open: Some(PathBuf::from("Notes")),
                control: Some(9000)
            })
        );
    }

    #[test]
    fn bad_control_ports_are_errors() {
        assert_eq!(p(&["--control"]), Err(ArgError::MissingPort));
        for bad in [
            "0",
            "65536",
            "-1",
            "abc",
            "",
            "80.5",
            "99999999999999999999",
        ] {
            assert_eq!(
                p(&["--control", bad]),
                Err(ArgError::BadPort(bad.to_owned())),
                "{bad}"
            );
        }
        assert_eq!(p(&["--control=x"]), Err(ArgError::BadPort("x".to_owned())));
    }

    #[cfg(windows)]
    #[test]
    fn non_utf8_path_survives() {
        use std::os::windows::ffi::OsStringExt;
        let odd = OsString::from_wide(&[0x4e, 0xD800, 0x6f]);
        assert_eq!(
            parse([odd.clone()]),
            Ok(Cli::Launch {
                open: Some(PathBuf::from(odd)),
                control: None
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_path_survives() {
        use std::os::unix::ffi::OsStringExt;
        let odd = OsString::from_vec(vec![b'N', 0xff, b'o']);
        assert_eq!(
            parse([odd.clone()]),
            Ok(Cli::Launch {
                open: Some(PathBuf::from(odd)),
                control: None
            })
        );
    }
}
