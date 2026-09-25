//! **The agent transport answers only what speaks its protocol** — M39
//! (`docs/plan-agent-transport.md`), display-free.
//!
//! Every fixture in `tests/browser/` is replayed byte for byte on its own
//! connection against [`transport::serve`] and a handler that counts what
//! it executed. The captured ones are what a real browser sent — Firefox
//! 156.0 and Chrome for Testing 145.0.7632.6, recorded by
//! `scripts/capture_browser_requests.py`; the file name says which. The
//! two `hand-` ones are written by hand and say so.
//!
//! The assembly check that the turnkey window really serves through this
//! is `crates/lumen-static-winit/tests/agent_transport.rs`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lumen_static_agent::transport;
use serde_json::{json, Value};

/// A server on an ephemeral loopback port whose handler counts.
fn server() -> (SocketAddr, Arc<AtomicUsize>) {
    let listener = transport::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();
    let executed = Arc::new(AtomicUsize::new(0));
    let n = executed.clone();
    std::thread::spawn(move || {
        transport::serve(listener, move |req: Value| {
            n.fetch_add(1, Ordering::SeqCst);
            Some(json!({ "jsonrpc": "2.0", "id": req["id"], "result": { "method": req["method"] } }))
        })
    });
    (addr, executed)
}

fn fixtures() -> Vec<(String, Vec<u8>)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/browser");
    let mut v: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "bin"))
        .map(|p| (p.file_stem().unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap()))
        .collect();
    v.sort();
    v
}

/// Write `bytes` with the write side left open — a browser's is, while
/// it waits — and read until the server closes or 2 s pass.
fn play(addr: SocketAddr, bytes: &[u8]) -> (Vec<Value>, bool, Duration) {
    let mut s = TcpStream::connect(addr).unwrap();
    let t = Instant::now();
    s.write_all(bytes).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut got = Vec::new();
    let mut buf = [0u8; 4096];
    let closed = loop {
        match s.read(&mut buf) {
            Ok(0) => break true,
            Ok(n) => got.extend_from_slice(&buf[..n]),
            // A reset is the server closing with our bytes unread.
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => break true,
            Err(_) => break false,
        }
    };
    let replies = got
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_slice(l).expect("every reply is a JSON line"))
        .collect();
    (replies, closed, t.elapsed())
}

/// **R14-F1.** Nothing a browser sends executes, and the connection
/// closes on its first line.
#[test]
fn a_browser_request_executes_nothing() {
    let (addr, executed) = server();
    let all = fixtures();
    assert!(all.len() >= 10, "the fixtures are missing: {} found", all.len());
    let mut failed = Vec::new();
    for (name, bytes) in all {
        let before = executed.load(Ordering::SeqCst);
        let (replies, closed, took) = play(addr, &bytes);
        let n = executed.load(Ordering::SeqCst) - before;
        let first = replies.first().map(|r| r["error"]["message"].to_string()).unwrap_or_default();
        println!(
            "{name}: methods executed {n} / bound 0; replies before close {}; closed {closed} after {} ms; {first}",
            replies.len(),
            took.as_millis()
        );
        if n != 0 || !closed || replies.len() > 1 {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "a browser's request was served: {failed:?}");
}

/// The accept direction: two well-formed lines on one connection are two
/// executions and two replies, ids echoed.
#[test]
fn a_line_client_is_answered() {
    let (addr, executed) = server();
    let s = TcpStream::connect(addr).unwrap();
    let mut w = s.try_clone().unwrap();
    let mut r = BufReader::new(s);
    for id in [11, 12] {
        writeln!(w, r#"{{"jsonrpc":"2.0","id":{id},"method":"ui.getTree"}}"#).unwrap();
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        println!("id {id} -> {v}");
        assert_eq!(v["id"], id, "the id is not echoed: {v}");
        assert_eq!(v["result"]["method"], "ui.getTree", "{v}");
    }
    assert_eq!(executed.load(Ordering::SeqCst), 2, "two lines, two executions");
}

/// A malformed line ends the session, and says why — including to a
/// person who pointed `curl` at the port.
#[test]
fn a_refused_line_is_named() {
    let (addr, executed) = server();
    for (bytes, code, says) in [
        (&b"GET / HTTP/1.1\r\n\r\n"[..], -32600, "not HTTP"),
        (&b"[{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ui.getTree\"}]\n"[..], -32600, "batches"),
        (&b"5\n"[..], -32600, "JSON object"),
        (&b"{not json\n"[..], -32700, "parse error"),
    ] {
        let (replies, closed, _) = play(addr, bytes);
        let r = replies.first().cloned().unwrap_or_default();
        println!("{:?} -> {r}; closed {closed}", String::from_utf8_lossy(bytes));
        assert_eq!(replies.len(), 1, "one reply, then close: {replies:?}");
        assert!(closed, "the connection stayed open");
        assert_eq!(r["error"]["code"], code, "{r}");
        assert!(r["error"]["message"].as_str().unwrap().contains(says), "{r}");
    }
    assert_eq!(executed.load(Ordering::SeqCst), 0);
}

/// **R14-F2.** A non-loopback address is refused unless overridden, and
/// every loopback spelling is accepted — both directions in one table.
#[test]
fn a_remote_bind_is_refused() {
    let mut failed = Vec::new();
    for (addr, allow, want_ok) in [
        ("0.0.0.0:0", false, false),
        ("[::]:0", false, false),
        ("127.0.0.1:0", false, true),
        ("[::1]:0", false, true),
        ("localhost:0", false, true),
        ("0.0.0.0:0", true, true),
    ] {
        let r = transport::bind_with(addr, allow);
        let verdict = match &r {
            Ok(l) => format!("accepted, bound {}", l.local_addr().unwrap()),
            Err(e) => format!("refused: {e}"),
        };
        let over = if allow { " (with the override)" } else { "" };
        println!("{addr}{over} -> {verdict}");
        // `[::1]` may be absent on a host without IPv6; that is not this
        // gate's question, so only a *policy* refusal counts against it.
        let policy_refused =
            matches!(&r, Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied);
        if want_ok == policy_refused || (!want_ok && r.is_ok()) {
            failed.push(addr);
        }
    }
    assert!(failed.is_empty(), "wrong verdicts: {failed:?}");
}
