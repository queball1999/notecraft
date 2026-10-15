//! Command-line arguments: `notecraft [--version] [--help] [<notebook folder or page file>]`.
//!
//! Parsing works on `OsString` so a path that is not valid UTF-8 is carried through intact.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Cli {
    /// Start the app, opening `open` first if given.
    Launch { open: Option<PathBuf> },
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
}

impl std::fmt::Display for ArgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFlag(flag) => write!(f, "unknown option: {flag}"),
            Self::TooManyPaths => write!(f, "only one notebook folder or page file can be opened"),
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
    let mut options_done = false;
    for arg in args {
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
        Cli::Launch { open: path }
    })
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
        assert_eq!(p(&[]), Ok(Cli::Launch { open: None }));
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
                open: Some(PathBuf::from("Notes/Work"))
            })
        );
    }

    #[test]
    fn double_dash_allows_a_dashed_path() {
        assert_eq!(
            p(&["--", "-odd"]),
            Ok(Cli::Launch {
                open: Some(PathBuf::from("-odd"))
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

    #[cfg(windows)]
    #[test]
    fn non_utf8_path_survives() {
        use std::os::windows::ffi::OsStringExt;
        let odd = OsString::from_wide(&[0x4e, 0xD800, 0x6f]);
        assert_eq!(
            parse([odd.clone()]),
            Ok(Cli::Launch {
                open: Some(PathBuf::from(odd))
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
                open: Some(PathBuf::from(odd))
            })
        );
    }
}
