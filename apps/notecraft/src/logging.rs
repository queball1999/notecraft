//! Logging for the desktop app (SCRD section 12).
//!
//! - Writes to stderr and to `logs/notecraft.log` in the settings folder.
//! - Each launch rotates the previous log to `notecraft.1.log` and `notecraft.1.log` to
//!   `notecraft.2.log`.
//! - The file stops growing at 16 MiB (one line says so), so a runaway loop cannot fill a disk.
//! - Default level: `info` for NoteCraft crates, `warn` for dependencies. `RUST_LOG` overrides.
//! - `NOTECRAFT_NO_PREFS=1` disables file logging (agent and test runs).
//!
//! Never log note content: page text, titles, file names inside notebooks, or ink. Log ids,
//! counts, sizes, timings, and error kinds instead.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

/// Largest log file, in bytes.
pub const MAX_LOG_BYTES: u64 = 16 * 1024 * 1024;

const DEFAULT_FILTER: &str = "warn,notecraft=info,notecraft_core=info,notecraft_storage=info,\
notecraft_model=info,notecraft_markdown=info,notecraft_canvas=info,notecraft_engine=info,\
notecraft_ui_egui=info";

/// The platform settings folder: `%APPDATA%\NoteCraft`, `~/Library/Application Support/NoteCraft`,
/// or `$XDG_CONFIG_HOME/notecraft` (falling back to `~/.config/notecraft`).
pub fn settings_dir() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(target_os = "windows") {
        env("APPDATA").map(|d| d.join("NoteCraft"))
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support/NoteCraft"))
    } else {
        env("XDG_CONFIG_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".config")))
            .map(|d| d.join("notecraft"))
    }
}

/// True when `NOTECRAFT_NO_PREFS` is set to anything but empty or "0".
pub fn no_prefs() -> bool {
    std::env::var("NOTECRAFT_NO_PREFS").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// Shift `notecraft.log` to `.1` and `.1` to `.2` in `dir`. Missing files are fine.
pub fn rotate(dir: &Path) -> io::Result<()> {
    let log = dir.join("notecraft.log");
    let one = dir.join("notecraft.1.log");
    let two = dir.join("notecraft.2.log");
    if one.exists() {
        if two.exists() {
            std::fs::remove_file(&two)?;
        }
        std::fs::rename(&one, &two)?;
    }
    if log.exists() {
        std::fs::rename(&log, &one)?;
    }
    Ok(())
}

/// A log file that stops accepting bytes once it reaches its cap.
pub struct CappedFile {
    file: File,
    written: u64,
    cap: u64,
    capped: bool,
}

impl CappedFile {
    pub fn new(file: File, cap: u64) -> Self {
        Self {
            file,
            written: 0,
            cap,
            capped: false,
        }
    }
}

impl Write for CappedFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.capped {
            return Ok(buf.len());
        }
        let len = u64::try_from(buf.len()).unwrap_or(u64::MAX);
        if self.written.saturating_add(len) > self.cap {
            self.capped = true;
            let note = b"log file reached its size cap; further lines go to stderr only\n";
            self.file.write_all(note)?;
            return Ok(buf.len());
        }
        self.file.write_all(buf)?;
        self.written = self.written.saturating_add(len);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

/// Shares one `CappedFile` between tracing's writers. A poisoned lock drops the line instead of
/// panicking.
#[derive(Clone)]
struct SharedFile(Arc<Mutex<CappedFile>>);

struct SharedGuard(Arc<Mutex<CappedFile>>);

impl Write for SharedGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.0.lock() {
            Ok(mut f) => f.write(buf),
            Err(_) => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.0.lock() {
            Ok(mut f) => f.flush(),
            Err(_) => Ok(()),
        }
    }
}

impl<'a> MakeWriter<'a> for SharedFile {
    type Writer = SharedGuard;

    fn make_writer(&'a self) -> Self::Writer {
        SharedGuard(Arc::clone(&self.0))
    }
}

fn open_log_file() -> Option<(SharedFile, PathBuf)> {
    if no_prefs() {
        return None;
    }
    let dir = settings_dir()?.join("logs");
    std::fs::create_dir_all(&dir).ok()?;
    // Rotation failing (for example, a log held open by another instance) is not fatal.
    let _ = rotate(&dir);
    let path = dir.join("notecraft.log");
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .ok()?;
    Some((
        SharedFile(Arc::new(Mutex::new(CappedFile::new(file, MAX_LOG_BYTES)))),
        path,
    ))
}

/// Install the global logger. Returns the log file path when file logging is on.
/// Never fails: if the file cannot be opened, logging continues on stderr only.
pub fn init() -> Option<PathBuf> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    let stderr = tracing_subscriber::fmt::layer().with_writer(io::stderr);
    let file = open_log_file();
    let path = file.as_ref().map(|(_, p)| p.clone());
    let file_layer = file.map(|(w, _)| {
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(w)
    });
    // Another subscriber already installed (only possible in tests) is not an error.
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stderr)
        .with(file_layer)
        .try_init();
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("notecraft-logging-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rotation_keeps_two_previous_logs() {
        let dir = temp_dir("rotate");
        for run in ["first", "second", "third"] {
            rotate(&dir).unwrap();
            std::fs::write(dir.join("notecraft.log"), run).unwrap();
        }
        let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap();
        assert_eq!(read("notecraft.log"), "third");
        assert_eq!(read("notecraft.1.log"), "second");
        assert_eq!(read("notecraft.2.log"), "first");
        rotate(&dir).unwrap();
        assert_eq!(read("notecraft.2.log"), "second");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotation_with_no_logs_is_fine() {
        let dir = temp_dir("empty");
        rotate(&dir).unwrap();
        assert!(!dir.join("notecraft.1.log").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn capped_file_stops_at_the_cap() {
        let dir = temp_dir("cap");
        let path = dir.join("notecraft.log");
        let mut f = CappedFile::new(File::create(&path).unwrap(), 10);
        f.write_all(b"12345").unwrap();
        f.write_all(b"67890").unwrap();
        f.write_all(b"overflow").unwrap();
        f.write_all(b"more").unwrap();
        f.flush().unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("1234567890log file reached its size cap"));
        assert!(!text.contains("overflow"));
        assert!(!text.contains("more"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
