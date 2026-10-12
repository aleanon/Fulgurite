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
//! is `crates/fulgurite-winit/tests/agent_transport.rs`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use fulgurite_agent::transport;
use serde_json::{json, Value};

// ---- a counting allocator: live bytes, and the peak since a reset --------
//
// For `a_line_without_newline_is_capped`: the transport runs in-process,
// so the test can count what a flood costs. Other tests in this binary
// run beside it and allocate kilobytes; the bound is megabytes.

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grew(n: usize) {
    let now = LIVE.fetch_add(n, Ordering::Relaxed) + n;
    PEAK.fetch_max(now, Ordering::Relaxed);
}

unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        let p = unsafe { std::alloc::System.alloc(l) };
        if !p.is_null() {
            grew(l.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(p, l) };
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, new: usize) -> *mut u8 {
        let q = unsafe { std::alloc::System.realloc(p, l, new) };
        if !q.is_null() {
            if new >= l.size() {
                grew(new - l.size());
            } else {
                LIVE.fetch_sub(l.size() - new, Ordering::Relaxed);
            }
        }
        q
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

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

/// **A reply costs a round trip, not a delayed ACK.** Twenty requests in
/// turn on one connection; the median is the figure. A reply written in
/// pieces -- `writeln!` on the bare stream sends a `Value`'s `Display`
/// output piece by piece and the newline apart -- leaves its tail to
/// Nagle, which holds it until the client's delayed ACK: about 40 ms on
/// every request, whatever the request did. The client side writes each
/// request in one `write_all` so only the server's writes are measured.
#[test]
fn a_reply_is_not_held_for_an_ack() {
    let (addr, _) = server();
    let s = TcpStream::connect(addr).unwrap();
    let mut w = s.try_clone().unwrap();
    let mut r = BufReader::new(s);
    let mut ms = Vec::new();
    for id in 0..20 {
        let req = format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"ui.getTree\"}}\n");
        let t = Instant::now();
        w.write_all(req.as_bytes()).unwrap();
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        ms.push(t.elapsed().as_secs_f64() * 1e3);
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["id"], id, "{v}");
    }
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = ms[ms.len() / 2];
    println!("round trip: median {median:.2} ms, max {:.2} ms / bound 10 ms", ms[ms.len() - 1]);
    assert!(median < 10.0, "a reply waited {median:.2} ms: it is written in pieces and Nagle holds the last one");
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

/// One well-formed request on its own connection; the reply, or `None`
/// within `timeout`.
fn ask(addr: SocketAddr, id: u64, timeout: Duration) -> (TcpStream, Option<Value>) {
    let s = TcpStream::connect(addr).unwrap();
    let mut w = s.try_clone().unwrap();
    s.set_read_timeout(Some(timeout)).unwrap();
    let _ = writeln!(w, r#"{{"jsonrpc":"2.0","id":{id},"method":"ui.getTree"}}"#);
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut line = String::new();
    let v = match r.read_line(&mut line) {
        Ok(n) if n > 0 => serde_json::from_str(&line).ok(),
        _ => None,
    };
    (s, v)
}

/// **R14-F3.** A client that connects and says nothing does not lock a
/// second one out.
#[test]
fn an_idle_client_does_not_block_a_second() {
    let (addr, _) = server();
    let _idle = TcpStream::connect(addr).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let t = Instant::now();
    let (_b, reply) = ask(addr, 21, Duration::from_millis(1000));
    let ms = t.elapsed().as_millis();
    println!("answered: {} in {ms} ms / bound 1000 ms", reply.is_some());
    assert_eq!(reply.map(|r| r["id"].clone()), Some(json!(21)), "the second client was not answered");
    assert!(ms < 1000);
}

/// **R14-F3.** A line with no newline is refused at the cap rather than
/// grown by every byte sent. 8 MiB is sent (the plan's 2 MiB would sit
/// at the bound under the old `lines()`, whose buffer doubles to exactly
/// 2 MiB); growth is the peak of live heap bytes over the flood.
#[test]
fn a_line_without_newline_is_capped() {
    const FLOOD: usize = 8 << 20;
    let (addr, executed) = server();
    let base = LIVE.load(Ordering::SeqCst);
    PEAK.store(base, Ordering::SeqCst);
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    let chunk = vec![b'x'; 1 << 16];
    let mut sent = 0;
    while sent < FLOOD {
        match s.write(&chunk) {
            Ok(n) => sent += n,
            Err(_) => break,
        }
    }
    s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut rest = Vec::new();
    let closed = match s.read_to_end(&mut rest) {
        Ok(_) => true,
        Err(e) => e.kind() == std::io::ErrorKind::ConnectionReset,
    };
    let grew = PEAK.load(Ordering::SeqCst).saturating_sub(base);
    println!(
        "sent {} KiB of one line; closed {closed}; grew {} KiB / bound 2048 KiB",
        sent >> 10,
        grew >> 10
    );
    assert!(closed, "the connection stayed open after {} KiB with no newline", sent >> 10);
    assert!(grew < 2 << 20, "grew {} KiB", grew >> 10);
    assert_eq!(executed.load(Ordering::SeqCst), 0);
    // Accept: a fresh connection is still answered.
    let (_c, reply) = ask(addr, 22, Duration::from_millis(1000));
    assert_eq!(reply.map(|r| r["id"].clone()), Some(json!(22)), "a fresh client after the flood");
}

/// The accept direction of the connection limit: sixteen at once are
/// all answered, and the seventeenth gets one `-32000` line.
#[test]
fn a_seventeenth_connection_is_refused_and_sixteen_answer() {
    let (addr, _) = server();
    let held: Vec<TcpStream> = (0..transport::MAX_CONNECTIONS).map(|_| TcpStream::connect(addr).unwrap()).collect();
    std::thread::sleep(Duration::from_millis(100));
    let (_extra, reply) = ask(addr, 99, Duration::from_millis(1000));
    let code = reply.as_ref().map(|r| r["error"]["code"].clone());
    println!("17th connection -> {reply:?}");
    assert_eq!(code, Some(json!(-32000)), "the 17th connection was not refused");
    let mut answered = 0;
    for (k, s) in held.iter().enumerate() {
        let mut w = s.try_clone().unwrap();
        writeln!(w, r#"{{"jsonrpc":"2.0","id":{k},"method":"ui.getTree"}}"#).unwrap();
        s.set_read_timeout(Some(Duration::from_millis(1000))).unwrap();
        let mut line = String::new();
        if BufReader::new(s.try_clone().unwrap()).read_line(&mut line).is_ok_and(|n| n > 0) {
            answered += 1;
        }
    }
    println!("held connections answered: {answered} / {}", transport::MAX_CONNECTIONS);
    assert_eq!(answered, transport::MAX_CONNECTIONS);
    // And a slot comes back when one closes.
    drop(held);
    std::thread::sleep(Duration::from_millis(100));
    let (_d, again) = ask(addr, 100, Duration::from_millis(1000));
    assert_eq!(again.map(|r| r["id"].clone()), Some(json!(100)), "slots were not given back");
}

/// **A failed bind ends the process with status 3** — M42/T42.1
/// (`docs/plan-drive-identity.md` §2.2, R18-F1). Until then the socket
/// thread logged `agent: cannot bind` and returned, the window kept
/// running without an endpoint, and whatever already held the port was
/// driven in its place.
///
/// Run in a child: the test binary re-executes itself with
/// `FULGURITE_BIND_CHILD` naming a port this test already holds. The child
/// models a window — `serve_addr` on a thread, the main thread carrying
/// on — so a bind that only returns shows as a child still running.
#[test]
fn a_failed_bind_exits_3() {
    if let Ok(addr) = std::env::var("FULGURITE_BIND_CHILD") {
        std::thread::spawn(move || transport::serve_addr(&addr, |_req: Value| None));
        std::thread::sleep(Duration::from_secs(10));
        return;
    }
    let held = transport::bind("127.0.0.1:0").expect("bind");
    let addr = held.local_addr().unwrap().to_string();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "a_failed_bind_exits_3", "--nocapture", "--test-threads=1"])
        .env("FULGURITE_BIND_CHILD", &addr)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn the child");
    let t0 = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if t0.elapsed() > Duration::from_secs(2) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child still running after 2 s: a bind on {addr}, which this test holds, did not exit");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut err = String::new();
    child.stderr.take().unwrap().read_to_string(&mut err).unwrap();
    println!("child on held {addr}: {status}, stderr {:?}", err.trim());
    assert_eq!(status.code(), Some(transport::BIND_FAILED), "{status}: {err}");
    assert!(err.contains(&format!("cannot bind {addr}")), "the address is not named: {err}");
    drop(held);
}

/// The accept direction: a bind on a free port serves.
///
/// The port is found by binding `:0` and dropping it, so something else
/// on the machine can take it before `serve_addr` binds -- and a failed
/// bind *exits the process* with 3 (that is the contract the test above
/// pins). Served in-process, that race killed this whole binary with a
/// bare "exit status: 3" (CI at 89e3f7d). So the server is a child, as in
/// `a_failed_bind_exits_3`, and a child that exits 3 means the port was
/// lost: a fresh one is tried, up to five, and each loss is printed.
#[test]
fn a_bind_on_a_free_port_serves() {
    if let Ok(addr) = std::env::var("FULGURITE_SERVE_CHILD") {
        std::thread::spawn(move || {
            transport::serve_addr(&addr, |req: Value| Some(json!({ "jsonrpc": "2.0", "id": req["id"], "result": 1 })))
        });
        std::thread::sleep(Duration::from_secs(10));
        return;
    }
    for attempt in 1..=5 {
        let port = transport::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let addr = format!("127.0.0.1:{port}");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "a_bind_on_a_free_port_serves", "--nocapture", "--test-threads=1"])
            .env("FULGURITE_SERVE_CHILD", &addr)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn the child");
        let t0 = Instant::now();
        let s = loop {
            if let Some(st) = child.try_wait().unwrap() {
                assert_eq!(st.code(), Some(transport::BIND_FAILED), "the server child died: {st}");
                break None;
            }
            match TcpStream::connect(&addr) {
                Ok(s) => break Some(s),
                Err(_) if t0.elapsed() < Duration::from_secs(2) => std::thread::sleep(Duration::from_millis(10)),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("nothing served on {addr}: {e}");
                }
            }
        };
        let Some(mut s) = s else {
            println!("attempt {attempt}: {addr} was taken before the child bound it (exit 3); trying another");
            continue;
        };
        // Whatever took the port may accept and never answer: a reply
        // must come within 2 s, and none from a child that exited 3 is a
        // lost port, not a failure.
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        s.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"x\"}\n").unwrap();
        let mut line = String::new();
        let read = BufReader::new(s).read_line(&mut line);
        let _ = child.kill();
        let st = child.wait().unwrap();
        if read.is_err() || line.is_empty() {
            assert_eq!(st.code(), Some(transport::BIND_FAILED), "no answer on {addr}, and the server child {st}");
            println!("attempt {attempt}: {addr} answered nothing and the child exited 3 -- the port was taken; trying another");
            continue;
        }
        println!("served on {addr} (attempt {attempt}): {}", line.trim());
        assert!(line.contains("\"result\":1"), "{line}");
        return;
    }
    panic!("five free ports were all taken before the child could bind one");
}

