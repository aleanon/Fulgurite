//! **The agent endpoint's one transport** — M39/T39.1
//! (`docs/plan-agent-transport.md`).
//!
//! Newline-delimited JSON-RPC 2.0 over TCP: one JSON object per line in,
//! one per line out. Lumen's `run-agent` protocol, so its client works
//! unchanged. Every server in the workspace — the turnkey window's
//! (`lumen_static_winit::agent::serve`) and `examples/counter-window`'s —
//! calls [`serve_addr`](crate::transport::serve_addr), and `scripts/check_one_agent_server.sh` refuses a
//! second `TcpListener::bind` anywhere outside tests.
//!
//! # What it refuses, and why that is framing and not policy
//!
//! **A line that is not a JSON object ends the connection**, after one
//! error reply. Nothing reads an `Origin` or a `Host` header, because
//! nothing reads headers at all: every way a web page can reach a local
//! port — `fetch`, a form, a beacon, a WebSocket upgrade, DNS rebinding —
//! begins with an HTTP request line, and an HTTP method token cannot
//! begin a JSON object. So nothing after that line runs.
//!
//! Until M39 both servers answered an unparsable line with `-32700` and
//! **kept reading**: an HTTP request's request line and headers each drew
//! an error, and the body a page chose then executed. A `text/plain`
//! `no-cors` POST is a CORS simple request, so no preflight stopped it
//! (R14-F1, `docs/results-deep-review.md`). What a real browser sends,
//! and what each vector executed before and after, is
//! `docs/results-agent-transport.md`.
//!
//! When the refused line looks like HTTP the reply says so
//! (`-32600`, "this is a JSON-RPC line protocol, not HTTP"). That check is
//! **only a message** — close-on-error alone carries the refusal, which
//! T39.1's injections showed by removing each mechanism separately.
//!
//! **Out of scope:** another process running as the same user, which can
//! already `ptrace` the application. A per-session token is deferred, not
//! rejected — the plan's §4 names what reopens it.
//!
//! # What it does not do
//!
//! It never sees a tree. The handler a server passes owns the step that
//! reaches one, and the thread rule that goes with it: the window's
//! handler sends each request to the event loop and waits for the reply,
//! so no thread but the loop's touches the tree. Envelope validation —
//! `jsonrpc`, `id`, `method` — is the dispatcher's, so an in-process
//! caller gets it too.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

use serde_json::{json, Value};

/// Answer one request. `None` when whatever answers requests is gone —
/// the event loop has exited — which closes the connection.
///
/// `Clone`, because each connection gets its own copy.
pub trait Handler: FnMut(Value) -> Option<Value> + Clone + Send + 'static {}
impl<F: FnMut(Value) -> Option<Value> + Clone + Send + 'static> Handler for F {}

/// Bind the agent's listener.
pub fn bind(addr: &str) -> std::io::Result<TcpListener> {
    TcpListener::bind(addr)
}

/// Bind `addr` and serve on it until the process ends, reporting on
/// stderr. A failed bind is reported and returns: the application keeps
/// running without an endpoint.
pub fn serve_addr(addr: &str, handler: impl Handler) {
    let listener = match bind(addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("agent: cannot bind {addr}: {e}");
            return;
        }
    };
    eprintln!("agent: listening on {addr}");
    serve(listener, handler);
}

/// Serve an already-bound listener: each connection in turn.
pub fn serve(listener: TcpListener, handler: impl Handler) {
    for stream in listener.incoming().flatten() {
        connection(stream, handler.clone());
    }
}

/// One connection: a line at a time until EOF, a refused line, or a
/// handler that has gone away.
fn connection(stream: TcpStream, mut handler: impl Handler) {
    let Ok(mut out) = stream.try_clone() else { return };
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let text = line.trim_ascii();
        if text.is_empty() {
            continue;
        }
        let req = match serde_json::from_slice::<Value>(text) {
            Ok(v @ Value::Object(_)) => v,
            parsed => {
                // Only a message: the `return` below refuses it anyway.
                if looks_like_http(text) {
                    let _ = writeln!(out, "{}", refusal(-32600, HTTP_REFUSAL));
                    return;
                }
                let reply = match parsed {
                    Ok(Value::Array(_)) => refusal(-32600, "batches are not supported: send one request per line"),
                    Ok(_) => refusal(-32600, "a request is a JSON object, one per line"),
                    Err(e) => refusal(-32700, &format!("parse error: {e}")),
                };
                let _ = writeln!(out, "{reply}");
                // **Close on error.** This, not the HTTP check above, is
                // what keeps a browser's body from running (R14-F1).
                return;
            }
        };
        let Some(resp) = handler(req) else { return };
        if writeln!(out, "{resp}").is_err() {
            return;
        }
    }
}

const HTTP_REFUSAL: &str = "this is a JSON-RPC line protocol, not HTTP: send one JSON object per line \
                            (see scripts/agent_client.py); the connection is closed";

fn refusal(code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": null, "error": { "code": code, "message": message } })
}

/// Whether a refused line is an HTTP request line — it starts with a
/// method token, or names an HTTP version.
fn looks_like_http(line: &[u8]) -> bool {
    const METHODS: [&[u8]; 10] =
        [b"GET ", b"POST ", b"PUT ", b"DELETE ", b"HEAD ", b"OPTIONS ", b"PATCH ", b"CONNECT ", b"TRACE ", b"PRI "];
    METHODS.iter().any(|m| line.starts_with(m)) || line.windows(6).any(|w| w == b" HTTP/")
}
