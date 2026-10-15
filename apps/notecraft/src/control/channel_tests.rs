//! The control channel end to end over a real TCP socket, with the shell running in a
//! headless egui_kittest window. kittest has no GPU, so a real `ViewportCommand::Screenshot`
//! is never answered here; the test injects the `Event::Screenshot` eframe would deliver,
//! which still covers the request, the PNG and base64 path, and the reply. The real-window
//! run is `tests/control_window.rs` (ignored by default).

use super::{server, Control};
use notecraft_engine::view::ThemePreset;
use notecraft_ui_egui::AppShell;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct State {
    shell: AppShell,
    control: Control,
}

struct Client {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    next_id: u64,
}

impl Client {
    fn connect(addr: std::net::SocketAddr) -> Self {
        let stream = TcpStream::connect(addr).unwrap_or_else(|e| panic!("connect: {e}"));
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap_or_else(|e| panic!("timeout: {e}"));
        let reader = BufReader::new(stream.try_clone().unwrap_or_else(|e| panic!("clone: {e}")));
        Self {
            stream,
            reader,
            next_id: 1,
        }
    }

    fn send_raw(&mut self, line: &str) {
        self.stream
            .write_all(format!("{line}\n").as_bytes())
            .unwrap_or_else(|e| panic!("write: {e}"));
    }

    fn read(&mut self) -> Option<Value> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => serde_json::from_str(&line).ok(),
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send_raw(
            &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string(),
        );
        let reply = self
            .read()
            .unwrap_or_else(|| panic!("no reply to {method}"));
        assert_eq!(reply["id"], id);
        reply
    }
}

/// Step the headless window until `done` yields a value. While `want_shot` is set (the client
/// is waiting on a `screenshot`), every step injects the fake frame eframe would deliver.
/// Events are injected only then, because any input makes egui ask for another repaint.
fn run_until<T>(
    harness: &mut egui_kittest::Harness<'static, State>,
    done: &mpsc::Receiver<T>,
    want_shot: &AtomicBool,
    shot_color: egui::Color32,
) -> T {
    for _ in 0..2000 {
        if want_shot.load(Ordering::SeqCst) {
            harness.input_mut().events.push(egui::Event::Screenshot {
                viewport_id: egui::ViewportId::ROOT,
                user_data: egui::UserData::default(),
                image: Arc::new(egui::ColorImage::filled([48, 32], shot_color)),
            });
        }
        harness.step();
        if let Ok(value) = done.try_recv() {
            return value;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the client did not finish");
}

fn harness() -> egui_kittest::Harness<'static, State> {
    let ctx = egui::Context::default();
    let wake = Arc::new(move || ctx.request_repaint());
    let server = server::start(0, TOKEN.to_owned(), "9.9.9".to_owned(), wake)
        .unwrap_or_else(|e| panic!("start: {e}"));
    let state = State {
        shell: AppShell::new(),
        control: Control::with_server(server),
    };
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, state: &mut State| {
                state.control.poll(ui.ctx(), &mut state.shell);
                state.shell.logic(ui.ctx());
                state.shell.ui(ui);
                state.control.end_frame(ui.ctx());
            },
            state,
        )
}

#[test]
fn themes_switch_and_screenshot_over_tcp() {
    let mut harness = harness();
    let addr = harness.state().control.addr();
    let (tx, rx) = mpsc::channel();
    let want_shot = Arc::new(AtomicBool::new(false));
    let flag = want_shot.clone();
    let client = std::thread::spawn(move || {
        let mut c = Client::connect(addr);
        let auth = c.call("auth", json!({ "token": TOKEN }));
        assert_eq!(auth["result"]["ok"], true);
        assert_eq!(auth["result"]["version"], "9.9.9");
        assert_eq!(auth["result"]["protocol"], 1);

        for preset in ThemePreset::ALL {
            let ran = c.call(
                "command.run",
                json!({ "id": "view.theme.set", "args": { "preset": preset.id() } }),
            );
            assert_eq!(ran["result"], preset.id(), "{ran}");
            let idle = c.call("wait.idle", json!({ "timeout_ms": 20000 }));
            assert_eq!(idle["result"]["ok"], true, "{idle}");
            flag.store(true, Ordering::SeqCst);
            let shot = c.call("screenshot", json!({}));
            flag.store(false, Ordering::SeqCst);
            let png = shot["result"]["png"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            assert!(png.starts_with("iVBORw0KGgo"), "{shot}");
        }

        // Errors carry NoteCraft's string code.
        let bad = c.call("command.run", json!({ "id": "no.such_command" }));
        assert_eq!(bad["error"]["data"]["code"], "not_found");
        let bad = c.call(
            "command.run",
            json!({ "id": "view.theme.set", "args": { "preset": "nope" } }),
        );
        assert_eq!(bad["error"]["data"]["code"], "invalid_args");
        let unknown = c.call("state.get", json!({}));
        assert_eq!(unknown["error"]["code"], -32601);
        tx.send(()).ok();
    });
    run_until(
        &mut harness,
        &rx,
        &want_shot,
        egui::Color32::from_rgb(200, 30, 30),
    );
    client
        .join()
        .unwrap_or_else(|_| panic!("client thread panicked"));

    let last = ThemePreset::ALL.last().copied();
    assert_eq!(
        Some(harness.state().shell.session().view().theme_preset),
        last
    );
}

#[test]
fn screenshot_png_has_the_frame_size_and_pixels() {
    let mut harness = harness();
    let addr = harness.state().control.addr();
    let (tx, rx) = mpsc::channel();
    let want_shot = Arc::new(AtomicBool::new(true));
    let client = std::thread::spawn(move || {
        let mut c = Client::connect(addr);
        c.call("auth", json!({ "token": TOKEN }));
        let shot = c.call("screenshot", json!({}));
        tx.send(
            shot["result"]["png"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        )
        .ok();
    });
    let png = run_until(
        &mut harness,
        &rx,
        &want_shot,
        egui::Color32::from_rgb(10, 200, 30),
    );
    client
        .join()
        .unwrap_or_else(|_| panic!("client thread panicked"));
    let bytes = decode_base64(&png);
    let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .unwrap_or_else(|e| panic!("not a PNG: {e}"))
        .to_rgba8();
    assert_eq!((image.width(), image.height()), (48, 32));
    assert_eq!(image.get_pixel(5, 5).0, [10, 200, 30, 255]);
}

#[test]
fn requests_before_auth_and_bad_tokens_close_the_connection() {
    let harness = harness();
    let addr = harness.state().control.addr();
    let mut c = Client::connect(addr);
    let reply = c.call("command.run", json!({ "id": "view.theme.set" }));
    assert_eq!(reply["error"]["code"], -32001);
    assert!(c.read().is_none(), "the connection is closed");

    let mut c = Client::connect(addr);
    let reply = c.call("auth", json!({ "token": "wrong" }));
    assert_eq!(reply["error"]["code"], -32001);
    assert!(c.read().is_none());
}

#[test]
fn oversize_lines_close_the_connection() {
    let harness = harness();
    let mut c = Client::connect(harness.state().control.addr());
    c.call("auth", json!({ "token": TOKEN }));
    // 16 MiB and a bit, with no newline until the end.
    let chunk = vec![b'x'; 1024 * 1024];
    for _ in 0..17 {
        if c.stream.write_all(&chunk).is_err() {
            break;
        }
    }
    let _ = c.stream.write_all(b"\n");
    let reply = c.read().unwrap_or_else(|| panic!("no error reply"));
    assert_eq!(reply["error"]["data"]["code"], "limit_exceeded");
    assert!(c.read().is_none());
}

#[test]
fn wait_idle_times_out_while_the_app_keeps_repainting() {
    let mut harness = harness();
    let addr = harness.state().control.addr();
    let (tx, rx) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let mut c = Client::connect(addr);
        c.call("auth", json!({ "token": TOKEN }));
        let idle = c.call("wait.idle", json!({ "timeout_ms": 100 }));
        tx.send(idle).ok();
    });
    // Ask for an immediate repaint on every step, so the app is never idle.
    for _ in 0..2000 {
        harness.ctx.request_repaint();
        harness.step();
        if let Ok(idle) = rx.try_recv() {
            assert_eq!(idle["error"]["code"], -32002, "{idle}");
            client.join().unwrap_or_else(|_| panic!("client panicked"));
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("wait.idle never timed out");
}

/// Standard base64 decoding, enough for the test.
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
    let mut acc = 0u32;
    let mut bits = 0;
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
