//! Starts the real app with `--control`, switches through the four theme presets, and saves a
//! screenshot of each to `target/control-screenshots/<preset>.png` (docs/control-protocol.md).
//!
//! Ignored by default: it opens a window, so it needs a display and a GPU adapter, which CI
//! runners do not have. The same dispatcher is covered without a window by the tests in
//! `src/control/channel_tests.rs`. Run it by hand with:
//!
//! `cargo test -p notecraft --test control_window -- --ignored --nocapture`
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const PRESETS: [&str; 4] = ["onenote2016", "onenote2019", "onenote2021", "m365"];

/// Kills the app when the test ends, pass or fail.
struct App(Child);

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn decode_base64(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for v in text.bytes().filter_map(value) {
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

struct Client {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    next_id: u64,
}

impl Client {
    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(self.stream, "{line}").unwrap();
        let mut reply = String::new();
        self.reader.read_line(&mut reply).unwrap();
        let reply: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(reply["id"], id, "{reply}");
        reply
    }
}

#[test]
#[ignore = "opens a real window: needs a display and a GPU adapter"]
fn every_theme_screenshots_through_the_control_channel() {
    let config = std::env::temp_dir().join(format!("notecraft-control-{}", std::process::id()));
    std::fs::create_dir_all(&config).unwrap();
    let port = free_port();
    let _app = App(Command::new(env!("CARGO_BIN_EXE_notecraft"))
        .args(["--control", &port.to_string()])
        .env("NOTECRAFT_CONFIG_DIR", &config)
        .env_remove("NOTECRAFT_NO_PREFS")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap());

    // The token appears once the app has started listening; connect as soon as it does.
    let token_path = config.join("control.token");
    let started = Instant::now();
    let stream = loop {
        let token = std::fs::read_to_string(&token_path).unwrap_or_default();
        if token.len() == 64 {
            if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
                break (stream, token);
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "the app did not start listening"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    let (stream, token) = stream;
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let reader = BufReader::new(stream.try_clone().unwrap());
    let mut client = Client {
        stream,
        reader,
        next_id: 1,
    };

    let auth = client.call("auth", json!({ "token": token.trim() }));
    assert_eq!(auth["result"]["ok"], true, "{auth}");
    assert_eq!(auth["result"]["protocol"], 1);

    let out: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "target",
        "control-screenshots",
    ]
    .iter()
    .collect();
    std::fs::create_dir_all(&out).unwrap();

    let mut shots: Vec<Vec<u8>> = Vec::new();
    for preset in PRESETS {
        let ran = client.call(
            "command.run",
            json!({ "id": "view.theme.set", "args": { "preset": preset } }),
        );
        assert_eq!(ran["result"], preset, "{ran}");
        let idle = client.call("wait.idle", json!({ "timeout_ms": 20000 }));
        assert_eq!(idle["result"]["ok"], true, "{idle}");
        let shot = client.call("screenshot", json!({}));
        let png = decode_base64(
            shot["result"]["png"]
                .as_str()
                .unwrap_or_else(|| panic!("{shot}")),
        );
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
        std::fs::write(out.join(format!("{preset}.png")), &png).unwrap();
        shots.push(png);
    }
    for (i, a) in shots.iter().enumerate() {
        for b in shots.iter().skip(i + 1) {
            assert_ne!(a, b, "two presets rendered identically");
        }
    }
    let _ = std::fs::remove_dir_all(&config);
}
