//! **The turnkey window's agent endpoint, through a real event loop** —
//! M39/T39.0 (`docs/plan-agent-transport.md`).
//!
//! The assembly check: `fulgurite_winit::agent::serve`, with every
//! request crossing to a winit event loop through its proxy exactly as
//! `run` wires it, answered by a handler that counts what it executed.
//! The display-free half — the transport alone, byte for byte, with a
//! counting allocator — is `crates/fulgurite-agent/tests/transport.rs`;
//! this binary is the proof that the window really serves through it.
//!
//! `harness = false`, because winit wants the main thread and a display.
//! **Without a display it prints a line and exits 0** — a plain
//! `cargo test` may skip — unless `FULGURITE_REQUIRE_DISPLAY=1`, where a
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
//! Ports 7812 and 7813 (the milestone's range is 7810–7819), and 7862
//! and 7863 for M48/T48.4's and T48.12's windows (M48's range is
//! 7860–7869). T48.12's children end themselves with
//! `FULGURITE_FRAME_REPORT=3`, which prints how many frames were journaled.
//!
//! # A real window's journal (M48/T48.4, R13-F8)
//!
//! The handler above only counts methods. The last case runs a
//! **`Session`-backed window** — `fulgurite_winit::run`, in a child
//! process, because `run` owns its event loop until the window closes —
//! and asserts that after `input.click`, `ui.lastChange` with
//! `since = reply.frame - 1` is non-empty. That holds only if the redraw
//! `answer_agent` requests ran and fed the journal; deleting it (the
//! review's `1beed32`) left every live drive green.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;
use fulgurite_winit::agent::{self, Rpc};
use fulgurite_winit::UserEvent;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::WindowId;

const MAIN: &str = "127.0.0.1:7812";
const WILD_PORT: u16 = 7813;
/// M48/T48.4: the session-backed window's endpoint.
const SESSION: &str = "127.0.0.1:7862";

// ---- M48/T48.4: an application for a real window ----------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct SUi {
    #[widget_id("count")]
    count: Label<String>,
    #[widget_id("inc")]
    inc: Button<STally>,
}

#[derive(DeriveApp, Default)]
struct STally {
    #[tree]
    ui: SUi,
    #[bind(ui.count, "count {}")]
    n: i64,
}

/// M48/T48.12: an application that animates forever, so a window draws
/// frames with no input at all.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct AUi {
    l: Label<String>,
}

#[derive(Default)]
struct Spin {
    ui: AUi,
    t: f32,
}

impl fulgurite::event::App for Spin {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = AUi;
    type Msg = fulgurite::event::Never;
    fn apply(_: &mut Self, m: Self::Msg) -> bool {
        match m {}
    }
    fn tree(&self) -> &AUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut AUi {
        &mut self.ui
    }
    fn tick(&mut self, dt_ms: f32) -> bool {
        self.t += dt_ms;
        true
    }
}

/// Run the spinning window as a child with `FULGURITE_FRAME_REPORT=3` and
/// read back its "N frames drawn, M journaled" line.
fn frame_report(endpoint: bool) -> (Option<u64>, String) {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .env("FULGURITE_SPIN_CHILD", if endpoint { "endpoint" } else { "none" })
        .env("FULGURITE_FRAME_REPORT", "3")
        .env_remove("FULGURITE_AGENT_ADDR")
        .env_remove("FULGURITE_AGENT")
        .output()
        .expect("spawn the spin child");
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    let line = err.lines().find(|l| l.contains("frames drawn")).unwrap_or("").to_string();
    let journaled = line
        .split(", ")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .and_then(|n| n.parse().ok());
    (journaled, line)
}

/// One JSON-RPC call on its own connection, or `None` on any failure.
fn rpc(addr: &str, method: &str, params: &str) -> Option<serde_json::Value> {
    let mut s = TcpStream::connect(addr).ok()?;
    s.write_all(format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"{method}\",\"params\":{params}}}\n").as_bytes())
        .ok()?;
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut line = Vec::new();
    let mut b = [0u8; 1];
    loop {
        match s.read(&mut b) {
            Ok(1) if b[0] == b'\n' => break,
            Ok(1) => line.push(b[0]),
            _ => return None,
        }
    }
    serde_json::from_slice(&line).ok()
}

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
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fulgurite-agent/tests/browser");
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

fn drive(executed: Arc<AtomicUsize>) -> Vec<Outcome> {
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
            // Green since T39.1: the request line ends the connection.
            red_until: None,
        });
    }

    // R14-F2: a wildcard address is bound and answered.
    //
    // **In a child** since M42/T42.1: a refused bind now ends the process
    // with status 3 (`docs/plan-drive-identity.md` §2.2), so serving
    // `0.0.0.0` in this process would end the test. The child is this
    // binary with `FULGURITE_WILD_CHILD` set, serving through `agent::serve`
    // on its own event loop's proxy.
    {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .env("FULGURITE_WILD_CHILD", "1")
            .env_remove("FULGURITE_AGENT_ALLOW_REMOTE")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn the wildcard child");
        let deadline = Instant::now() + Duration::from_millis(2000);
        let mut answered = false;
        let mut status = None;
        while Instant::now() < deadline && !answered && status.is_none() {
            answered = line_client(&format!("127.0.0.1:{WILD_PORT}"), 2).is_some();
            status = child.try_wait().ok().flatten();
            if !answered {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        // Reaped on every path; a kill after the child exited is a no-op.
        let _ = child.kill();
        let _ = child.wait();
        let code = status.and_then(|s| s.code());
        out.push(Outcome {
            name: "a 0.0.0.0 bind is refused, and the window exits 3".into(),
            green: !answered && code == Some(3),
            line: format!(
                "0.0.0.0:{WILD_PORT} bound and answered: {answered} / expected false; \
                 child exit {code:?} / expected Some(3)"
            ),
            red_until: None,
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
            red_until: None,
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
            red_until: None,
        });
    }

    // M48/T48.4 (R13-F8): a real window's redraw feeds the journal.
    {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .env("FULGURITE_SESSION_CHILD", "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn the session child");
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut up = false;
        while Instant::now() < deadline && !up {
            up = rpc(SESSION, "ui.getTree", "{}").is_some();
            if !up {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        let (mut landed, mut changes, mut frame) = (false, 0usize, 0u64);
        if up {
            // The window's first frame seeds the journal; give it one.
            std::thread::sleep(Duration::from_millis(200));
            if let Some(r) = rpc(SESSION, "input.click", r##"{"selector":"#inc"}"##) {
                landed = r["result"]["handled"] == true;
                frame = r["result"]["frame"].as_u64().unwrap_or(0);
            }
            // The redraw the click asked for runs on the loop; one wait,
            // not a poll -- a redraw that never comes must fail here.
            std::thread::sleep(Duration::from_millis(300));
            if let Some(lc) = rpc(SESSION, "ui.lastChange", &format!(r#"{{"since":{}}}"#, frame.saturating_sub(1))) {
                changes = lc["result"]["changed"].as_array().map_or(0, |a| a.len());
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        out.push(Outcome {
            name: "a session window's redraw feeds the journal".into(),
            green: up && landed && changes >= 1,
            line: format!(
                "window up {up}, click landed {landed}; lastChange after click: {changes} changes \
                 (must be >= 1), since {}",
                frame.saturating_sub(1)
            ),
            red_until: None,
        });
    }

    // M48/T48.12 (R14-F8): no endpoint, no journal -- and with one,
    // every frame journaled (the accept direction).
    {
        let (none, line_none) = frame_report(false);
        out.push(Outcome {
            name: "a window with no endpoint journals nothing".into(),
            green: none == Some(0),
            line: format!("journaled {none:?} / bound Some(0) -- \"{line_none}\""),
            red_until: None,
        });
        let (some, line_some) = frame_report(true);
        out.push(Outcome {
            name: "a window with an endpoint journals every frame".into(),
            green: some == Some(3),
            line: format!("journaled {some:?} / expected Some(3) -- \"{line_some}\""),
            red_until: None,
        });
    }

    // Stop the loop through the endpoint itself.
    let _ = line_exit();
    out
}

fn main() {
    // `harness = false` means libtest's flags arrive here. `--list` is
    // how scripts/check_test_counts.sh (M43/T43.9) counts a binary's tests,
    // so answer it in libtest's terse format and stop: the cases are one
    // test with many vectors, and listing must never open a window.
    if std::env::args().any(|a| a == "--list") {
        println!("agent_transport: test");
        return;
    }
    if let Ok(which) = std::env::var("FULGURITE_SPIN_CHILD") {
        // M48/T48.12's child: a spinning window, with or without an
        // endpoint; `FULGURITE_FRAME_REPORT` ends it after three frames.
        let agent = (which == "endpoint").then(|| "127.0.0.1:7863".to_string());
        fulgurite_winit::run(
            Spin::default(),
            fulgurite_winit::Config { agent, width: 120, height: 80, ..Default::default() },
        );
        return;
    }
    if std::env::var("FULGURITE_SESSION_CHILD").is_ok() {
        // M48/T48.4's child: the turnkey window, with its endpoint.
        let mut app = STally::default();
        app.ui.inc.on_click = |t: &mut STally, _: &fulgurite::EventCtx| t.n += 1;
        fulgurite_winit::run(
            app,
            fulgurite_winit::Config { agent: Some(SESSION.into()), width: 240, height: 160, ..Default::default() },
        );
        return;
    }
    if std::env::var("FULGURITE_WILD_CHILD").is_ok() {
        // The wildcard case's child: serve 0.0.0.0 on this loop's proxy.
        // A refused bind exits 3 inside `serve`; one that binds serves
        // until the parent kills it.
        let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("display");
        let proxy = event_loop.create_proxy();
        std::thread::spawn(move || agent::serve(format!("0.0.0.0:{WILD_PORT}"), proxy));
        event_loop.run_app(&mut Loop { executed: Arc::new(AtomicUsize::new(0)) }).expect("run");
        return;
    }
    let require = std::env::var("FULGURITE_REQUIRE_DISPLAY").is_ok_and(|v| v == "1");
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
            let _ = tx.send(drive(executed));
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
