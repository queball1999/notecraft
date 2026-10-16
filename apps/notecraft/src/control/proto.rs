//! The control protocol without any window or socket: framing, JSON-RPC types, and the
//! per-connection auth state machine (docs/control-protocol.md). Everything here is plain
//! data in and out, so it is unit-tested without egui or a network.

use serde_json::{json, Map, Value};
use std::io::BufRead;
use std::time::Duration;

/// The `protocol` number `auth` reports.
pub const PROTOCOL: u32 = 1;
/// Longest accepted line, in bytes (docs/control-protocol.md section 2).
pub const MAX_LINE: usize = 16 * 1024 * 1024;
/// `wait.idle` timeout when the client gives none, and the most it may ask for.
pub const DEFAULT_IDLE_MS: u64 = 5_000;
pub const MAX_IDLE_MS: u64 = 60_000;

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;
/// Not authenticated (docs/control-protocol.md section 4).
pub const UNAUTHENTICATED: i64 = -32001;
/// A `wait.idle` that ran out of time.
pub const TIMEOUT: i64 = -32002;
/// A command that is not found, Coming Soon, and so on; the string code is in `data.code`.
pub const COMMAND_FAILED: i64 = -32000;

/// A JSON-RPC error, with NoteCraft's string code (docs/mcp.md section 5) in `data.code`.
#[derive(Clone, Debug, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub string_code: &'static str,
}

impl RpcError {
    pub fn new(code: i64, string_code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            string_code,
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS, "invalid_args", message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(INTERNAL_ERROR, "internal", message)
    }

    /// The JSON-RPC code for a command error's string code.
    pub fn from_command(string_code: &'static str, message: String) -> Self {
        let code = match string_code {
            "invalid_args" => INVALID_PARAMS,
            "internal" => INTERNAL_ERROR,
            _ => COMMAND_FAILED,
        };
        Self::new(code, string_code, message)
    }
}

/// What the UI thread is asked to do. `auth` and unknown methods never get this far.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    CommandRun { id: String, args: Value },
    Screenshot,
    WaitIdle { timeout: Duration },
}

/// What the UI thread answers.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    Json(Value),
    /// A captured frame as RGBA8, row by row. The connection thread encodes it, so PNG work
    /// stays off the UI thread.
    Frame {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
}

/// What a connection does with one line.
#[derive(Debug, PartialEq)]
pub enum Step {
    /// Send this line back and carry on.
    Reply(String),
    /// Send this line back, then close the connection.
    Close(String),
    /// Ask the UI thread; answer with [`finish`] when it replies.
    Forward { id: Value, request: Request },
}

/// One client connection's protocol state: it starts unauthenticated.
#[derive(Debug)]
pub struct Connection {
    token: String,
    version: String,
    authed: bool,
}

impl Connection {
    pub fn new(token: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            version: version.into(),
            authed: false,
        }
    }

    pub fn is_authed(&self) -> bool {
        self.authed
    }

    /// Handle one request line (no newline). Anything wrong before a successful `auth` closes
    /// the connection.
    pub fn handle_line(&mut self, line: &[u8]) -> Step {
        let (id, method, params) = match parse_request(line) {
            Ok(parts) => parts,
            Err((id, error)) => return self.fail(&id, &error),
        };
        if method == "auth" {
            return self.auth(&id, &params);
        }
        if !self.authed {
            return Step::Close(error_line(
                &id,
                &RpcError::new(UNAUTHENTICATED, "unauthenticated", "send auth first"),
            ));
        }
        match method.as_str() {
            "command.run" => match command_run_params(&params) {
                Ok(request) => Step::Forward { id, request },
                Err(error) => Step::Reply(error_line(&id, &error)),
            },
            "screenshot" => Step::Forward {
                id,
                request: Request::Screenshot,
            },
            "wait.idle" => match idle_params(&params) {
                Ok(timeout) => Step::Forward {
                    id,
                    request: Request::WaitIdle { timeout },
                },
                Err(error) => Step::Reply(error_line(&id, &error)),
            },
            other => {
                let shown: String = other.chars().take(64).collect();
                Step::Reply(error_line(
                    &id,
                    &RpcError::new(
                        METHOD_NOT_FOUND,
                        "not_found",
                        format!("unknown method: {shown}"),
                    ),
                ))
            }
        }
    }

    /// An error reply that also closes the connection while unauthenticated.
    fn fail(&self, id: &Value, error: &RpcError) -> Step {
        let line = error_line(id, error);
        if self.authed {
            Step::Reply(line)
        } else {
            Step::Close(line)
        }
    }

    fn auth(&mut self, id: &Value, params: &Value) -> Step {
        let sent = params.get("token").and_then(Value::as_str);
        match sent {
            Some(sent) if constant_time_eq(sent.as_bytes(), self.token.as_bytes()) => {
                self.authed = true;
                Step::Reply(result_line(
                    id,
                    &json!({ "ok": true, "version": self.version, "protocol": PROTOCOL }),
                ))
            }
            _ => Step::Close(error_line(
                id,
                &RpcError::new(UNAUTHENTICATED, "unauthenticated", "bad token"),
            )),
        }
    }
}

/// The response line for the UI thread's answer to a forwarded request.
pub fn finish(id: &Value, reply: Result<Reply, RpcError>) -> String {
    match reply {
        Ok(Reply::Json(value)) => result_line(id, &value),
        Ok(Reply::Frame {
            width,
            height,
            rgba,
        }) => match encode_png_base64(width, height, &rgba) {
            Ok(png) => result_line(id, &json!({ "png": png })),
            Err(message) => error_line(id, &RpcError::internal(message)),
        },
        Err(error) => error_line(id, &error),
    }
}

type Parsed = (Value, String, Value);

fn parse_request(line: &[u8]) -> Result<Parsed, (Value, RpcError)> {
    let value: Value = serde_json::from_slice(line).map_err(|e| {
        (
            Value::Null,
            RpcError::new(PARSE_ERROR, "invalid_args", format!("bad JSON: {e}")),
        )
    })?;
    let Value::Object(mut object) = value else {
        return Err((
            Value::Null,
            RpcError::new(
                INVALID_REQUEST,
                "invalid_args",
                "a request is a JSON object",
            ),
        ));
    };
    // A response id is echoed only if it is a string, number, or null; anything else is
    // dropped rather than reflected.
    let id = match object.remove("id") {
        Some(id @ (Value::String(_) | Value::Number(_))) => id,
        _ => Value::Null,
    };
    let Some(Value::String(method)) = object.remove("method") else {
        return Err((
            id,
            RpcError::new(INVALID_REQUEST, "invalid_args", "missing method"),
        ));
    };
    let params = object.remove("params").unwrap_or(Value::Null);
    Ok((id, method, params))
}

fn command_run_params(params: &Value) -> Result<Request, RpcError> {
    let Some(id) = params.get("id").and_then(Value::as_str) else {
        return Err(RpcError::invalid_params("command.run needs a string id"));
    };
    let args = match params.get("args") {
        None => Value::Null,
        Some(args @ (Value::Null | Value::Object(_))) => args.clone(),
        Some(_) => return Err(RpcError::invalid_params("args must be an object")),
    };
    Ok(Request::CommandRun {
        id: id.to_owned(),
        args,
    })
}

fn idle_params(params: &Value) -> Result<Duration, RpcError> {
    let ms = match params.get("timeout_ms") {
        None | Some(Value::Null) => DEFAULT_IDLE_MS,
        Some(v) => v
            .as_u64()
            .ok_or_else(|| RpcError::invalid_params("timeout_ms must be a whole number"))?,
    };
    Ok(Duration::from_millis(ms.min(MAX_IDLE_MS)))
}

pub fn result_line(id: &Value, result: &Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

pub fn error_line(id: &Value, error: &RpcError) -> String {
    let mut data = Map::new();
    data.insert("code".to_owned(), Value::from(error.string_code));
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": error.code, "message": error.message, "data": data },
    })
    .to_string()
}

/// Equal bytes, in time that depends only on the length of the inputs.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for (i, x) in a.iter().enumerate() {
        let y = b.get(i).copied().unwrap_or(0);
        diff |= usize::from(x ^ y);
    }
    diff == 0
}

/// One read from a connection.
#[derive(Debug, PartialEq, Eq)]
pub enum Line {
    /// A line without its newline (and without a trailing carriage return). May be empty.
    Data(Vec<u8>),
    /// The peer closed the connection.
    Eof,
    /// The line passed `max` bytes. Nothing more is read.
    TooLong,
}

/// Read one newline-terminated line of at most `max` bytes, never buffering more than that.
/// A final line without a newline counts as a line.
pub fn read_line<R: BufRead>(reader: &mut R, max: usize) -> std::io::Result<Line> {
    let mut line = Vec::new();
    loop {
        let (used, done) = {
            let chunk = reader.fill_buf()?;
            if chunk.is_empty() {
                return Ok(if line.is_empty() {
                    Line::Eof
                } else {
                    Line::Data(trim_cr(line))
                });
            }
            match chunk.iter().position(|&b| b == b'\n') {
                Some(end) => {
                    if line.len() + end > max {
                        return Ok(Line::TooLong);
                    }
                    line.extend_from_slice(chunk.get(..end).unwrap_or_default());
                    (end + 1, true)
                }
                None => {
                    if line.len() + chunk.len() > max {
                        return Ok(Line::TooLong);
                    }
                    line.extend_from_slice(chunk);
                    (chunk.len(), false)
                }
            }
        };
        reader.consume(used);
        if done {
            return Ok(Line::Data(trim_cr(line)));
        }
    }
}

fn trim_cr(mut line: Vec<u8>) -> Vec<u8> {
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    line
}

/// Encode RGBA8 pixels as a PNG, then as standard base64.
pub fn encode_png_base64(width: u32, height: u32, rgba: &[u8]) -> Result<String, String> {
    let image = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| "the frame size does not match its pixels".to_owned())?;
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| format!("PNG encoding failed: {e}"))?;
    Ok(base64(&png))
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding.
pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let byte = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let n = (byte(0) << 16) | (byte(1) << 8) | byte(2);
        let sextet = |shift: u32| {
            let index = usize::try_from((n >> shift) & 63).unwrap_or(0);
            char::from(ALPHABET.get(index).copied().unwrap_or(b'A'))
        };
        out.push(sextet(18));
        out.push(sextet(12));
        out.push(if chunk.len() > 1 { sextet(6) } else { '=' });
        out.push(if chunk.len() > 2 { sextet(0) } else { '=' });
    }
    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const TOKEN: &str = "abc123";

    fn conn() -> Connection {
        Connection::new(TOKEN, "9.9.9")
    }

    fn authed() -> Connection {
        let mut c = conn();
        let step = c.handle_line(
            br#"{"jsonrpc":"2.0","id":1,"method":"auth","params":{"token":"abc123"}}"#,
        );
        assert!(matches!(step, Step::Reply(_)), "{step:?}");
        c
    }

    fn parse(line: &str) -> Value {
        serde_json::from_str(line).unwrap()
    }

    #[test]
    fn auth_succeeds_with_the_token() {
        let mut c = conn();
        let Step::Reply(line) = c.handle_line(
            br#"{"jsonrpc":"2.0","id":7,"method":"auth","params":{"token":"abc123"}}"#,
        ) else {
            panic!("expected a reply");
        };
        let v = parse(&line);
        assert_eq!(v["id"], 7);
        assert_eq!(v["result"]["ok"], true);
        assert_eq!(v["result"]["version"], "9.9.9");
        assert_eq!(v["result"]["protocol"], 1);
        assert!(c.is_authed());
    }

    #[test]
    fn wrong_or_missing_token_closes() {
        for params in [r#"{"token":"nope"}"#, r#"{}"#, r#"{"token":5}"#, "null"] {
            let mut c = conn();
            let line = format!(r#"{{"id":1,"method":"auth","params":{params}}}"#);
            let Step::Close(reply) = c.handle_line(line.as_bytes()) else {
                panic!("expected close for {params}");
            };
            let v = parse(&reply);
            assert_eq!(v["error"]["code"], UNAUTHENTICATED);
            assert_eq!(v["error"]["data"]["code"], "unauthenticated");
            assert!(!c.is_authed());
        }
    }

    #[test]
    fn requests_before_auth_close() {
        for method in ["command.run", "screenshot", "wait.idle", "nope"] {
            let mut c = conn();
            let line = format!(r#"{{"id":2,"method":"{method}"}}"#);
            let Step::Close(reply) = c.handle_line(line.as_bytes()) else {
                panic!("expected close for {method}");
            };
            assert_eq!(parse(&reply)["error"]["code"], UNAUTHENTICATED);
        }
    }

    #[test]
    fn bad_json_before_auth_closes_and_after_auth_replies() {
        let Step::Close(reply) = conn().handle_line(b"{nope") else {
            panic!("expected close");
        };
        let v = parse(&reply);
        assert_eq!(v["error"]["code"], PARSE_ERROR);
        assert_eq!(v["id"], Value::Null);

        let mut c = authed();
        let Step::Reply(reply) = c.handle_line(b"{nope") else {
            panic!("expected reply");
        };
        assert_eq!(parse(&reply)["error"]["code"], PARSE_ERROR);
        assert!(matches!(c.handle_line(b"[1,2]"), Step::Reply(_)));
        assert!(matches!(c.handle_line(br#"{"id":1}"#), Step::Reply(_)));
    }

    #[test]
    fn unknown_method_is_32601() {
        let mut c = authed();
        let Step::Reply(reply) = c.handle_line(br#"{"id":3,"method":"state.get"}"#) else {
            panic!("expected reply");
        };
        let v = parse(&reply);
        assert_eq!(v["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(v["id"], 3);
    }

    #[test]
    fn command_run_is_forwarded() {
        let mut c = authed();
        let step = c.handle_line(
            br#"{"id":"a","method":"command.run","params":{"id":"view.zoom","args":{"percent":150}}}"#,
        );
        assert_eq!(
            step,
            Step::Forward {
                id: json!("a"),
                request: Request::CommandRun {
                    id: "view.zoom".to_owned(),
                    args: json!({ "percent": 150 }),
                },
            }
        );
        let step = c.handle_line(br#"{"id":1,"method":"command.run","params":{"id":"x.y"}}"#);
        assert!(matches!(
            step,
            Step::Forward {
                request: Request::CommandRun {
                    args: Value::Null,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn command_run_checks_its_params() {
        let mut c = authed();
        for line in [
            r#"{"id":1,"method":"command.run"}"#,
            r#"{"id":1,"method":"command.run","params":{"id":5}}"#,
            r#"{"id":1,"method":"command.run","params":{"id":"a.b","args":[1]}}"#,
        ] {
            let Step::Reply(reply) = c.handle_line(line.as_bytes()) else {
                panic!("expected reply for {line}");
            };
            let v = parse(&reply);
            assert_eq!(v["error"]["code"], INVALID_PARAMS);
            assert_eq!(v["error"]["data"]["code"], "invalid_args");
        }
    }

    #[test]
    fn wait_idle_timeout_defaults_and_caps() {
        let mut c = authed();
        let timeout = |c: &mut Connection, params: &str| {
            let line = format!(r#"{{"id":1,"method":"wait.idle","params":{params}}}"#);
            match c.handle_line(line.as_bytes()) {
                Step::Forward {
                    request: Request::WaitIdle { timeout },
                    ..
                } => Some(timeout),
                _ => None,
            }
        };
        assert_eq!(timeout(&mut c, "{}"), Some(Duration::from_millis(5_000)));
        assert_eq!(
            timeout(&mut c, r#"{"timeout_ms":250}"#),
            Some(Duration::from_millis(250))
        );
        assert_eq!(
            timeout(&mut c, r#"{"timeout_ms":99999999}"#),
            Some(Duration::from_millis(60_000))
        );
        assert_eq!(timeout(&mut c, r#"{"timeout_ms":"x"}"#), None);
    }

    #[test]
    fn strange_ids_are_not_echoed() {
        let mut c = authed();
        let Step::Reply(reply) = c.handle_line(br#"{"id":{"a":1},"method":"nope"}"#) else {
            panic!("expected reply");
        };
        assert_eq!(parse(&reply)["id"], Value::Null);
    }

    #[test]
    fn finish_wraps_results_and_errors() {
        let ok = parse(&finish(&json!(1), Ok(Reply::Json(json!({"a": 1})))));
        assert_eq!(ok["result"]["a"], 1);
        let err = parse(&finish(
            &json!(1),
            Err(RpcError::from_command("coming_soon", "later".to_owned())),
        ));
        assert_eq!(err["error"]["code"], COMMAND_FAILED);
        assert_eq!(err["error"]["data"]["code"], "coming_soon");
        let bad = parse(&finish(
            &json!(1),
            Ok(Reply::Frame {
                width: 4,
                height: 4,
                rgba: vec![0; 3],
            }),
        ));
        assert_eq!(bad["error"]["data"]["code"], "internal");
    }

    #[test]
    fn constant_time_eq_compares() {
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"abcd", b"abc"));
    }

    #[test]
    fn lines_are_split_and_trimmed() {
        let mut r = Cursor::new(b"one\r\ntwo\n\nlast".to_vec());
        assert_eq!(read_line(&mut r, 100).unwrap(), Line::Data(b"one".to_vec()));
        assert_eq!(read_line(&mut r, 100).unwrap(), Line::Data(b"two".to_vec()));
        assert_eq!(read_line(&mut r, 100).unwrap(), Line::Data(Vec::new()));
        assert_eq!(
            read_line(&mut r, 100).unwrap(),
            Line::Data(b"last".to_vec())
        );
        assert_eq!(read_line(&mut r, 100).unwrap(), Line::Eof);
    }

    #[test]
    fn oversize_lines_stop_reading() {
        // A tiny buffer makes the cap bite across several fill_buf calls.
        let data = b"0123456789\nrest\n".to_vec();
        let mut r = std::io::BufReader::with_capacity(4, Cursor::new(data.clone()));
        assert_eq!(read_line(&mut r, 9).unwrap(), Line::TooLong);
        let mut r = std::io::BufReader::with_capacity(4, Cursor::new(data));
        assert_eq!(
            read_line(&mut r, 10).unwrap(),
            Line::Data(b"0123456789".to_vec())
        );
        // No newline at all: still capped.
        let mut r = std::io::BufReader::with_capacity(4, Cursor::new(vec![b'x'; 1000]));
        assert_eq!(read_line(&mut r, 100).unwrap(), Line::TooLong);
    }

    #[test]
    fn base64_matches_the_rfc_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn png_round_trips() {
        let rgba: Vec<u8> = (0..3 * 2)
            .flat_map(|i| [i as u8 * 10, 50, 200, 255])
            .collect();
        let text = encode_png_base64(3, 2, &rgba).unwrap();
        assert!(text.starts_with("iVBORw0KGgo"), "PNG signature in base64");
        assert!(encode_png_base64(3, 3, &rgba).is_err());
    }
}
