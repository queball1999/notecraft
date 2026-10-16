//! The TCP side of the control channel: a listener thread on 127.0.0.1 and one thread per
//! client. Clients never touch the UI; they hand [`Job`]s to the UI thread over a channel and
//! wait for the answer.

use super::proto::{self, Connection, Line, Reply, Request, RpcError, Step};
use serde_json::Value;
use std::io::{BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;

/// Most clients at once; more are dropped on accept.
const MAX_CLIENTS: usize = 8;
/// How long a client may take to send its first complete line (the `auth`).
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
/// The longest the connection thread waits for the UI thread, above `wait.idle`'s own cap.
const REPLY_TIMEOUT: Duration = Duration::from_secs(120);

/// A request for the UI thread, with where to send the answer.
pub struct Job {
    pub request: Request,
    pub reply: Sender<Result<Reply, RpcError>>,
}

/// A running listener. Dropping it does not stop the threads; they end with the process.
pub struct Server {
    jobs: Receiver<Job>,
    addr: SocketAddr,
}

impl Server {
    /// The address actually bound (useful with port 0).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The next waiting job, if any. Never blocks.
    pub fn try_job(&self) -> Option<Job> {
        self.jobs.try_recv().ok()
    }
}

/// Listen on `127.0.0.1:port` (0 picks a free port). `wake` is called after each job is
/// queued so the UI thread runs a frame.
pub fn start(
    port: u16,
    token: String,
    version: String,
    wake: Arc<dyn Fn() + Send + Sync>,
) -> std::io::Result<Server> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))?;
    let addr = listener.local_addr()?;
    let (tx, jobs) = mpsc::channel();
    let clients = Arc::new(AtomicUsize::new(0));
    std::thread::Builder::new()
        .name("control-listener".to_owned())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                if clients.fetch_add(1, Ordering::SeqCst) >= MAX_CLIENTS {
                    clients.fetch_sub(1, Ordering::SeqCst);
                    continue;
                }
                let (tx, token, version, wake) =
                    (tx.clone(), token.clone(), version.clone(), wake.clone());
                let slot = clients.clone();
                let spawned = std::thread::Builder::new()
                    .name("control-client".to_owned())
                    .spawn(move || {
                        serve(stream, Connection::new(token, version), &tx, &*wake);
                        slot.fetch_sub(1, Ordering::SeqCst);
                    });
                if spawned.is_err() {
                    tracing::warn!("control: could not start a client thread");
                    clients.fetch_sub(1, Ordering::SeqCst);
                }
            }
        })?;
    Ok(Server { jobs, addr })
}

fn send(stream: &mut TcpStream, line: &str) -> bool {
    stream
        .write_all(line.as_bytes())
        .and_then(|()| stream.write_all(b"\n"))
        .and_then(|()| stream.flush())
        .is_ok()
}

/// After refusing an oversize line, read and discard what the client is still sending (for a
/// second, at most 64 MiB) so closing does not reset the connection before it reads the error.
fn drain(stream: &mut TcpStream, reader: &mut BufReader<TcpStream>) {
    use std::io::Read;
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let mut sink = [0u8; 64 * 1024];
    let mut total = 0usize;
    while total < 64 * 1024 * 1024 {
        match reader.read(&mut sink) {
            Ok(0) | Err(_) => break,
            Ok(n) => total += n,
        }
    }
}

/// Run one client to the end of its connection.
fn serve(stream: TcpStream, mut conn: Connection, jobs: &Sender<Job>, wake: &(dyn Fn() + Sync)) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut stream = stream;
    let mut reader = BufReader::new(read_half);
    let _ = stream.set_read_timeout(Some(AUTH_TIMEOUT));
    loop {
        let line = match proto::read_line(&mut reader, proto::MAX_LINE) {
            Ok(Line::Data(line)) => line,
            Ok(Line::Eof) | Err(_) => return,
            Ok(Line::TooLong) => {
                let error = RpcError::new(
                    proto::INVALID_REQUEST,
                    "limit_exceeded",
                    "a line is at most 16 MiB",
                );
                send(&mut stream, &proto::error_line(&Value::Null, &error));
                drain(&mut stream, &mut reader);
                return;
            }
        };
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match conn.handle_line(&line) {
            Step::Reply(reply) => {
                if !send(&mut stream, &reply) {
                    return;
                }
                if conn.is_authed() {
                    let _ = stream.set_read_timeout(None);
                }
            }
            Step::Close(reply) => {
                send(&mut stream, &reply);
                return;
            }
            Step::Forward { id, request } => {
                let (reply_tx, reply_rx) = mpsc::channel();
                if jobs
                    .send(Job {
                        request,
                        reply: reply_tx,
                    })
                    .is_err()
                {
                    return;
                }
                wake();
                let answer = match reply_rx.recv_timeout(REPLY_TIMEOUT) {
                    Ok(answer) => answer,
                    Err(RecvTimeoutError::Timeout) => {
                        Err(RpcError::internal("the app did not answer in time"))
                    }
                    Err(RecvTimeoutError::Disconnected) => return,
                };
                if !send(&mut stream, &proto::finish(&id, answer)) {
                    return;
                }
            }
        }
    }
}
