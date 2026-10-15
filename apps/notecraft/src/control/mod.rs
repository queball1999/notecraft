//! The minimal control channel for UI tests (docs/control-protocol.md, build_plan PR 0.11).
//!
//! `notecraft --control <port>` listens on 127.0.0.1, writes a random token to `control.token`
//! in the settings folder, and serves `auth`, `command.run`, `screenshot`, and `wait.idle` as
//! newline-delimited JSON-RPC. Commands run on the UI thread through the same
//! [`AppShell::run_command`] path the ribbon uses; there is no second action path.
//!
//! - [`proto`] is the protocol with no window or socket (framing, auth state machine).
//! - [`server`] is the TCP threads.
//! - [`Control`] (here) is the UI-thread half: it drains jobs once per frame and answers them.

pub mod proto;
pub mod server;

use notecraft_ui_egui::AppShell;
use proto::{Reply, Request, RpcError};
use serde_json::Value;
use server::Job;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Name of the token file in the settings folder.
pub const TOKEN_FILE: &str = "control.token";
/// How often a waiting `wait.idle` wakes the UI to check its deadline.
const IDLE_POLL: Duration = Duration::from_millis(50);

type Answer = Sender<Result<Reply, RpcError>>;

struct IdleWait {
    deadline: Instant,
    reply: Answer,
}

/// The UI-thread half of the control channel.
pub struct Control {
    server: server::Server,
    /// Screenshot requests waiting for their frame, oldest first.
    shots: VecDeque<Answer>,
    idles: Vec<IdleWait>,
    token_path: Option<PathBuf>,
}

impl Control {
    /// Create the token, write it to `dir`, and start listening on `127.0.0.1:port`.
    pub fn start(port: u16, dir: &Path, ctx: &egui::Context) -> Result<Self, String> {
        let token = new_token().map_err(|e| format!("could not make a control token: {e}"))?;
        let path = dir.join(TOKEN_FILE);
        write_token(&path, &token)
            .map_err(|e| format!("could not write {}: {e}", path.display()))?;
        let wake_ctx = ctx.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || wake_ctx.request_repaint());
        let server = server::start(port, token, env!("CARGO_PKG_VERSION").to_owned(), wake)
            .map_err(|e| format!("could not listen on 127.0.0.1:{port}: {e}"))?;
        let mut control = Self::with_server(server);
        control.token_path = Some(path);
        Ok(control)
    }

    /// Wrap an already running server (tests).
    pub fn with_server(server: server::Server) -> Self {
        Self {
            server,
            shots: VecDeque::new(),
            idles: Vec::new(),
            token_path: None,
        }
    }

    /// Where the server listens.
    pub fn addr(&self) -> std::net::SocketAddr {
        self.server.addr()
    }

    /// Per frame, before the shell's own `logic`: run queued requests and collect screenshots.
    pub fn poll(&mut self, ctx: &egui::Context, shell: &mut AppShell) {
        while let Some(Job { request, reply }) = self.server.try_job() {
            match request {
                Request::CommandRun { id, args } => {
                    let _ = reply.send(run_command(shell, &id, args));
                    // Whatever the command changed needs a frame to show up.
                    ctx.request_repaint();
                }
                Request::Screenshot => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                        egui::UserData::default(),
                    ));
                    ctx.request_repaint();
                    self.shots.push_back(reply);
                }
                Request::WaitIdle { timeout } => self.idles.push(IdleWait {
                    deadline: Instant::now() + timeout,
                    reply,
                }),
            }
        }
        let frames: Vec<Arc<egui::ColorImage>> = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
                .collect()
        });
        for image in frames {
            // A frame nobody asked for (for example one a test injected) is ignored.
            if let Some(reply) = self.shots.pop_front() {
                let _ = reply.send(Ok(frame_reply(&image)));
            }
        }
    }

    /// Per frame, after the UI is drawn: answer `wait.idle` once nothing is pending, or fail
    /// it at its deadline.
    pub fn end_frame(&mut self, ctx: &egui::Context) {
        if self.idles.is_empty() {
            return;
        }
        let busy = ctx.has_requested_repaint() || !self.shots.is_empty();
        let now = Instant::now();
        let mut waiting = Vec::new();
        for wait in std::mem::take(&mut self.idles) {
            if !busy {
                let _ = wait
                    .reply
                    .send(Ok(Reply::Json(serde_json::json!({ "ok": true }))));
            } else if now >= wait.deadline {
                let _ = wait.reply.send(Err(RpcError::new(
                    proto::TIMEOUT,
                    "limit_exceeded",
                    "the app was still repainting when the timeout passed",
                )));
            } else {
                waiting.push(wait);
            }
        }
        self.idles = waiting;
        if !self.idles.is_empty() {
            // A delayed request does not count as pending, so this cannot keep itself busy.
            ctx.request_repaint_after(IDLE_POLL);
        }
    }

    /// Remove the token file (best effort) when the app exits.
    pub fn shutdown(&self) {
        if let Some(path) = &self.token_path {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn run_command(shell: &mut AppShell, id: &str, args: Value) -> Result<Reply, RpcError> {
    match shell.run_command(id, args) {
        Some(Ok(output)) => Ok(Reply::Json(output.value)),
        Some(Err(error)) => Err(RpcError::from_command(error.code(), error.to_string())),
        // The user canceled a file picker; nothing ran.
        None => Ok(Reply::Json(serde_json::json!({ "canceled": true }))),
    }
}

fn frame_reply(image: &egui::ColorImage) -> Reply {
    let [width, height] = image.size;
    let rgba = image.pixels.iter().flat_map(|p| p.to_array()).collect();
    Reply::Frame {
        width: u32::try_from(width).unwrap_or(0),
        height: u32::try_from(height).unwrap_or(0),
        rgba,
    }
}

/// 32 random bytes as 64 hex characters.
pub fn new_token() -> Result<String, getrandom::Error> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)?;
    Ok(hex(&bytes))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(
            DIGITS.get(usize::from(b >> 4)).copied().unwrap_or(b'0'),
        ));
        out.push(char::from(
            DIGITS.get(usize::from(b & 15)).copied().unwrap_or(b'0'),
        ));
    }
    out
}

/// Write the token so only the current user can read it: mode 0600 on Unix. On Windows the
/// file keeps the permissions of the settings folder (`%APPDATA%\NoteCraft`), which only the
/// current user, administrators, and SYSTEM can read by default; no ACL is changed.
pub fn write_token(path: &Path, token: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        // An older file keeps its old mode through `open`, so set it explicitly.
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(token.as_bytes())?;
    file.flush()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_64_hex_chars_and_differ() {
        let a = new_token().unwrap();
        let b = new_token().unwrap();
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn hex_encodes_bytes() {
        assert_eq!(hex(&[0x00, 0xab, 0xff]), "00abff");
    }

    #[test]
    fn token_file_is_written_and_replaced() {
        let dir = std::env::temp_dir().join(format!("notecraft-token-{}", std::process::id()));
        let path = dir.join(TOKEN_FILE);
        write_token(&path, "first-token-value").unwrap();
        write_token(&path, "second").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
