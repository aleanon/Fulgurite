//! **The turnkey window's agent endpoint, through a real event loop** —
//! M39/T39.0 (`docs/plan-agent-transport.md`).
//!
//! The assembly check: `lumen_static_winit::agent::serve`, with every
//! request crossing to a winit event loop through its proxy exactly as
//! `run` wires it, answered by a handler that counts what it executed.
//! The display-free half — the transport alone, byte for byte, with a
//! counting allocator — is `crates/lumen-static-agent/tests/transport.rs`;
//! this binary is the proof that the window really serves through it.
//!
//! `harness = false`, because winit wants the main thread and a display.
//! **Without a display it prints a line and exits 0** — a plain
//! `cargo test` may skip — unless `LUMEN_REQUIRE_DISPLAY=1`, where a
//! missing display is a failure (M43 §2.2's skip policy).
//!
//! # Cases
//!
//! Each prints its quantity beside its bound. A case can carry
//! `red_until`: it is then *expected* red until that task lands, prints
//! `XFAIL`, and **fails the run if it goes green** — so a fix that lands
//! without removing its marker is noticed, and a marker cannot hide a
//! regression of anything already green.
//!
//! Ports 7812 and 7813 (the milestone's range is 7810–7819).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lumen_static_winit::agent::{self, Rpc};
use lumen_static_winit::UserEvent;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::WindowId;

const MAIN: &str = "127.0.0.1:7812";
const WILD_PORT: u16 = 7813;

// ---- a counting allocator: live bytes, and the peak since a reset -----------

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        let p = unsafe { std::alloc::System.alloc(l) };
        if !p.is_null() {
            let now = LIVE.fetch_add(l.size(), Ordering::Relaxed) + l.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
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
                let now = LIVE.fetch_add(new - l.size(), Ordering::Relaxed) + (new - l.size());
                PEAK.fetch_max(now, Ordering::Relaxed);
            } else {
                LIVE.fetch_sub(l.size() - new, Ordering::Relaxed);
            }
        }
        q
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

// ---- the loop: count, answer, and exit on request ---------------------------

struct Loop {
    executed: Arc<AtomicUsize>,
}

impl ApplicationHandler<UserEvent> for Loop {
    fn resumed(&mut self, _: &ActiveEventLoop) {}
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    #[allow(unreachable_patterns)] // `hot` adds variants under workspace unification
    fn user_event(&mut self, el: &ActiveEventLoop, ev: UserEvent) {
        if let UserEvent::Agent(Rpc { req, reply }) = ev {
            let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
            if method == "test.exit" {
                el.exit();
            } else {
                self.executed.fetch_add(1, Ordering::SeqCst);
            }
            let _ = reply.send(serde_json::json!({
                "jsonrpc": "2.0", "id": req.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "result": { "executed": method }
            }));
        }
    }
}

// ---- the cases -------------------------------------------------------------

struct Outcome {
    name: String,
    green: bool,
    line: String,
    red_until: Option<&'static str>,
}

fn fixtures() -> Vec<(String, Vec<u8>)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../lumen-static-agent/tests/browser");
    let mut v: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "bin"))
        .map(|p| (p.file_stem().unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap()))
        .collect();
    v.sort();
    v
}

/// Write `bytes` and read to EOF or `timeout`. Returns the replies
/// (lines) and whether **the server** closed. The write side stays open,
/// as a browser's does while it waits for a response: a half-close here
/// would hand the server an EOF and make every loop look like it closed.
fn play(addr: &str, bytes: &[u8], timeout: Duration) -> (usize, bool) {
    let Ok(mut s) = TcpStream::connect(addr) else { return (0, true) };
    let _ = s.write_all(bytes);
    s.set_read_timeout(Some(timeout)).unwrap();
    let mut got = Vec::new();
    let mut buf = [0u8; 4096];
    let closed = loop {
        match s.read(&mut buf) {
            Ok(0) => break true,
            Ok(n) => got.extend_from_slice(&buf[..n]),
            Err(_) => break false,
        }
    };
    (got.iter().filter(|&&b| b == b'\n').count(), closed)
}

fn line_client(addr: &str, id: u64) -> Option<(TcpStream, Duration)> {
    let mut s = TcpStream::connect(addr).ok()?;
    let t = Instant::now();
    s.write_all(format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"ui.getTree\"}}\n").as_bytes()).ok()?;
    s.set_read_timeout(Some(Duration::from_millis(1000))).unwrap();
    let mut b = [0u8; 1];
    let mut line = Vec::new();
    loop {
        match s.read(&mut b) {
            Ok(1) if b[0] == b'\n' => return Some((s, t.elapsed())),
            Ok(1) => line.push(b[0]),
            _ => return None,
        }
    }
}

fn line_exit() -> Option<()> {
    let mut s = TcpStream::connect(MAIN).ok()?;
    s.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"test.exit\"}\n").ok()?;
    s.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    let _ = s.read(&mut [0u8; 256]);
    Some(())
}

fn drive(executed: Arc<AtomicUsize>, proxy: EventLoopProxy<UserEvent>) -> Vec<Outcome> {
    let mut out = Vec::new();
    let ex = || executed.load(Ordering::SeqCst);

    // Accept direction first: a line client is answered at all.
    let before = ex();
    let answered = line_client(MAIN, 1).is_some();
    out.push(Outcome {
        name: "a line client is answered".into(),
        green: answered && ex() == before + 1,
        line: format!("answered {answered}; executed {} / expected 1", ex() - before),
        red_until: None,
    });

    // Every browser vector, each on its own connection.
    for (name, bytes) in fixtures() {
        let before = ex();
        let t = Instant::now();
        let (replies, closed) = play(MAIN, &bytes, Duration::from_secs(2));
        let n = ex() - before;
        out.push(Outcome {
            name: format!("browser vector {name}"),
            green: n == 0 && closed,
            line: format!(
                "methods executed {n} / bound 0; replies before close {replies}; closed {closed} after {} ms",
                t.elapsed().as_millis()
            ),
            // A WebSocket upgrade carries no body, so it executes nothing
            // even before T39.1 — but the connection stays open for the
            // next line, which is the red half.
            red_until: Some("T39.1"),
        });
    }

    // R14-F2: a wildcard address is bound and answered.
    {
        let p = proxy.clone();
        std::thread::spawn(move || agent::serve(format!("0.0.0.0:{WILD_PORT}"), p));
        let deadline = Instant::now() + Duration::from_millis(1000);
        let mut answered = false;
        while Instant::now() < deadline && !answered {
            answered = line_client(&format!("127.0.0.1:{WILD_PORT}"), 2).is_some();
            if !answered {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        out.push(Outcome {
            name: "a 0.0.0.0 bind is refused".into(),
            green: !answered,
            line: format!("0.0.0.0:{WILD_PORT} bound and answered: {answered} / expected false"),
            red_until: Some("T39.2"),
        });
    }

    // R14-F3: an idle client locks every other client out.
    {
        let idle = TcpStream::connect(MAIN).expect("idle client connects");
        std::thread::sleep(Duration::from_millis(100));
        let second = line_client(MAIN, 3);
        let ms = second.as_ref().map(|(_, d)| d.as_millis());
        drop(idle);
        drop(second);
        out.push(Outcome {
            name: "an idle client does not block a second".into(),
            green: ms.is_some(),
            line: format!("second client answered in {ms:?} ms / bound 1000 ms"),
            red_until: Some("T39.3"),
        });
    }

    // R14-F3: a line with no newline grows the process by what was sent.
    {
        const FLOOD: usize = 32 << 20;
        let base = LIVE.load(Ordering::SeqCst);
        PEAK.store(base, Ordering::SeqCst);
        let mut s = TcpStream::connect(MAIN).expect("flood connects");
        s.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
        let chunk = vec![b'x'; 1 << 16];
        let mut sent = 0;
        while sent < FLOOD {
            match s.write(&chunk) {
                Ok(n) => sent += n,
                Err(_) => break,
            }
        }
        std::thread::sleep(Duration::from_millis(300));
        let grew = PEAK.load(Ordering::SeqCst).saturating_sub(base);
        drop(s);
        std::thread::sleep(Duration::from_millis(200));
        out.push(Outcome {
            name: "a line without a newline is capped".into(),
            green: grew < 2 << 20,
            line: format!("sent {} KiB; grew {} KiB / bound 2048 KiB", sent >> 10, grew >> 10),
            red_until: Some("T39.3"),
        });
    }

    // Stop the loop through the endpoint itself.
    let _ = line_exit();
    out
}

fn main() {
    let require = std::env::var("LUMEN_REQUIRE_DISPLAY").is_ok_and(|v| v == "1");
    let event_loop = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(el) => el,
        Err(e) => {
            println!("agent_transport: no display ({e}), skipped");
            std::process::exit(if require { 1 } else { 0 });
        }
    };
    let executed = Arc::new(AtomicUsize::new(0));
    let proxy = event_loop.create_proxy();
    {
        let p = proxy.clone();
        std::thread::spawn(move || agent::serve(MAIN.to_string(), p));
    }
    // Wait for the port, then drive from a thread while the loop runs here.
    let deadline = Instant::now() + Duration::from_secs(10);
    while TcpStream::connect(MAIN).is_err() {
        assert!(Instant::now() < deadline, "{MAIN} never came up");
        std::thread::sleep(Duration::from_millis(20));
    }
    let (tx, rx) = std::sync::mpsc::channel();
    {
        let executed = executed.clone();
        std::thread::spawn(move || {
            let _ = tx.send(drive(executed, proxy));
        });
    }
    event_loop.run_app(&mut Loop { executed }).expect("run");
    let outcomes = rx.recv_timeout(Duration::from_secs(30)).expect("the driver finished");

    let mut bad = 0;
    for o in &outcomes {
        let verdict = match (o.green, o.red_until) {
            (true, None) => "ok",
            (false, None) => {
                bad += 1;
                "FAIL"
            }
            (false, Some(_)) => "XFAIL",
            (true, Some(_)) => {
                bad += 1;
                "XPASS"
            }
        };
        let until = o.red_until.map(|t| format!(" (red until {t})")).unwrap_or_default();
        println!("{verdict:5} {}: {}{until}", o.name, o.line);
    }
    if bad > 0 {
        println!("agent_transport: {bad} unexpected; an XPASS means a fix landed: remove its `red_until`");
        std::process::exit(1);
    }
    println!("agent_transport: {} cases, all as expected", outcomes.len());
}
