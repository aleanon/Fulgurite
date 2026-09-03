//! The counter, in a window.
//!
//! This is the only crate in the workspace with an OS dependency, and it
//! is deliberately thin: everything above the seam — the tree, the frame
//! loop, layout, paint, the software renderer — is the same code
//! `examples/counter` runs headlessly.
//!
//! `crates/lumen-static-shell` stopped exactly here for three
//! milestones, because a window cannot be gated headlessly and every
//! other subsystem earned an acceptance test. The claim it stopped on
//! was that a front end adds only two things: window events into
//! `Host::post`, and `Canvas::pixels` blitted to the surface. That claim
//! is what this file tests — `window_event` and `redraw` below are
//! the whole binding.

use std::num::NonZeroU32;
use std::rc::Rc;

use lumen_static::prelude::*;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::Host;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WKey, NamedKey};
use winit::window::{Window, WindowId};

mod app;

use app::{build, stylesheet_for, Tally, PAGE};

// ---- the agent endpoint ----------------------------------------------

/// What the endpoint drives: the window's host and its last frame.
///
/// The shell's `Host` cannot implement `AgentHost` itself without
/// pulling `observe` into every binary that links the shell (F2.5), so
/// the *example* bridges them — which is fine, because this is the one
/// crate that has already opted into everything.
struct Live<'a> {
    host: &'a mut Host<Tally>,
    metrics: &'a mut Option<FontMetrics>,
    canvas: &'a mut Canvas,
    sign: &'a mut i64,
}

impl lumen_static_agent::AgentHost for Live<'_> {
    fn tree(&self) -> lumen_static_agent::TreeRef<'_> {
        lumen_static_agent::TreeRef::of_root(&self.host.app.ui, &self.host.styles, &self.host.states)
    }
    fn enqueue(&mut self, ev: Event) {
        self.host.post(ev);
    }
    fn pump(&mut self) -> u64 {
        match self.metrics.as_mut() {
            Some(m) => self.host.pump_with(m),
            None => self.host.pump(),
        };
        // Same sign-tracking the redraw path does, so a click that flips
        // the sign recolours before the agent's next query.
        let sign = self.host.app.count.signum();
        if sign != *self.sign {
            *self.sign = sign;
            self.host.set_stylesheet(&stylesheet_for(sign));
            match self.metrics.as_mut() {
                Some(m) => self.host.pump_with(m),
                None => self.host.pump(),
            };
        }
        self.canvas.clear(PAGE);
        self.host.render(self.canvas);
        self.host.frame_number()
    }
    fn frame(&self) -> Option<(u32, u32, &[u8])> {
        Some((self.canvas.width, self.canvas.height, &self.canvas.pixels))
    }
}

/// One request from the socket thread, and where its reply goes.
struct Rpc {
    req: serde_json::Value,
    reply: std::sync::mpsc::Sender<serde_json::Value>,
}

/// Serve newline-delimited JSON-RPC on `addr`, forwarding each request
/// to the event loop and writing back what it returns.
///
/// Lumen's `run-agent` protocol, so its client works unchanged: one JSON
/// object per line in, one per line out. The socket thread never touches
/// the tree — every request crosses to the main thread via the proxy,
/// and the tree is only ever read or driven from there.
fn serve_agent(addr: String, proxy: winit::event_loop::EventLoopProxy<Rpc>) {
    use std::io::{BufRead, BufReader, Write};
    let listener = match std::net::TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("agent: cannot bind {addr}: {e}");
            return;
        }
    };
    eprintln!("agent: listening on {addr}");
    for stream in listener.incoming().flatten() {
        let mut out = match stream.try_clone() {
            Ok(s) => s,
            Err(_) => continue,
        };
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            let req: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(e) => {
                    let _ = writeln!(
                        out,
                        r#"{{"jsonrpc":"2.0","id":null,"error":{{"code":-32700,"message":"parse error: {e}"}}}}"#
                    );
                    continue;
                }
            };
            let (tx, rx) = std::sync::mpsc::channel();
            if proxy.send_event(Rpc { req, reply: tx }).is_err() {
                return;
            }
            let Ok(resp) = rx.recv() else { return };
            if writeln!(out, "{resp}").is_err() {
                break;
            }
        }
    }
}

// ---- the seam --------------------------------------------------------

struct WinApp {
    host: Host<Tally>,
    canvas: Canvas,
    /// The *same* face the canvas rasterizes with. Layout must measure
    /// with the metrics paint will draw with, or the two disagree —
    /// the first window measured with the block stand-in and painted
    /// with fontdue, so every box was too narrow and "TALLY" wrapped
    /// to "TAL / LY". The headless path had it right; the window did
    /// not, and only a screenshot of the live window showed it.
    metrics: Option<FontMetrics>,
    /// `None` until `resumed`; winit owns window creation.
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    cursor: (f32, f32),
    /// Last sign the stylesheet was cascaded for.
    sign: i64,
    agent: lumen_static_agent::Agent,
}

impl WinApp {
    fn new() -> Self {
        let (w, h) = (440, 520);
        let mut host = Host::new(build(), w, h);
        host.set_stylesheet(&stylesheet_for(0));
        let font = FontStack::system();
        let canvas = match &font {
            Some(f) => Canvas::with_font(w, h, f.clone()),
            // A machine with no font we recognise still gets a window;
            // it just gets the block stand-in in it, which is honest
            // about what happened.
            None => {
                eprintln!("no system font found — falling back to block glyphs");
                Canvas::new(w, h)
            }
        };
        let metrics = font.map(FontMetrics::new);
        WinApp {
            host,
            canvas,
            metrics,
            window: None,
            surface: None,
            cursor: (0.0, 0.0),
            sign: 0,
            agent: lumen_static_agent::Agent::new(),
        }
    }

    /// One frame, measured with the real face when there is one.
    fn pump(&mut self) {
        match self.metrics.as_mut() {
            Some(m) => {
                self.host.pump_with(m);
            }
            None => {
                self.host.pump();
            }
        }
    }

    /// Re-cascade when the sign changes — Lumen's counter toggles a
    /// `.lss` class here; this swaps the sign-dependent rules and lets
    /// the cascade run again. Either way it is not per-frame work.
    fn sync_sign(&mut self) {
        let sign = self.host.app.count.signum();
        if sign != self.sign {
            self.sign = sign;
            self.host.set_stylesheet(&stylesheet_for(sign));
        }
    }

    /// Drain input, lay out, paint, and blit. The second half of the
    /// binding: `Canvas::pixels` is RGBA8, softbuffer wants `0RGB` in a
    /// `u32`.
    fn redraw(&mut self) {
        let Some(window) = self.window.clone() else { return };
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };

        if (self.canvas.width, self.canvas.height) != (size.width, size.height) {
            let font = self.canvas.font.clone();
            self.canvas = match font {
                Some(f) => Canvas::with_font(size.width, size.height, f),
                None => Canvas::new(size.width, size.height),
            };
            self.host.resize(size.width, size.height);
        }

        self.pump();
        self.sync_sign();
        self.pump();

        let Some(surface) = self.surface.as_mut() else { return };
        self.canvas.clear(PAGE);
        self.host.render(&mut self.canvas);

        surface.resize(w, h).expect("resize surface");
        let mut buf = surface.buffer_mut().expect("acquire surface buffer");
        for (dst, px) in buf.iter_mut().zip(self.canvas.pixels.chunks_exact(4)) {
            *dst = (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32;
        }
        let dark = buf.iter().filter(|v| **v != 0x00FF_FFFF).count();
        let blen = buf.len();
        buf.present().expect("present");
        if std::env::var_os("LUMEN_TRACE").is_some() {
            eprintln!("  buf len={blen} non-white={dark} surface={}x{}", w, h);
            eprintln!(
                "redraw {}x{} cmds={} ink={:.3}",
                size.width,
                size.height,
                self.host.list.len(),
                self.canvas.ink_fraction(PAGE)
            );
        }
    }
}

impl ApplicationHandler<Rpc> for WinApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("lumen-static — tally")
            .with_inner_size(winit::dpi::LogicalSize::new(440.0, 520.0));
        let window = Rc::new(event_loop.create_window(attrs).expect("create window"));
        let ctx = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        self.surface =
            Some(softbuffer::Surface::new(&ctx, window.clone()).expect("softbuffer surface"));
        // Ask for the first frame explicitly: nothing else will.
        window.request_redraw();
        self.window = Some(window);
    }

    /// A request from the agent socket. Answered on this thread, against
    /// the live tree, then the window is asked to redraw so what the
    /// agent did is what the user sees.
    fn user_event(&mut self, event_loop: &ActiveEventLoop, rpc: Rpc) {
        let method = rpc.req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        if method == "app.quit" {
            let _ = rpc.reply.send(serde_json::json!({ "jsonrpc": "2.0", "id": rpc.req.get("id"), "result": { "ok": true } }));
            event_loop.exit();
            return;
        }
        let resp = {
            let mut live = Live {
                host: &mut self.host,
                metrics: &mut self.metrics,
                canvas: &mut self.canvas,
                sign: &mut self.sign,
            };
            self.agent.dispatch_input(&mut live, &rpc.req)
        };
        let _ = rpc.reply.send(resp);
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        // The first half of the binding: every one of these becomes an
        // `Event` in the host's single queue — the same queue the agent
        // protocol's `input.*` methods post into, which is what makes an
        // agent session replay as a test.
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
                self.host.post(Event::PointerMove { x: self.cursor.0, y: self.cursor.1 });
            }

            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let (x, y) = self.cursor;
                self.host.post(match state {
                    ElementState::Pressed => Event::PointerDown { x, y },
                    ElementState::Released => Event::PointerUp { x, y },
                });
                self.window.as_ref().unwrap().request_redraw();
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y * 20.0,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                self.host.post(Event::Scroll { x: self.cursor.0, y: self.cursor.1, dy });
                self.window.as_ref().unwrap().request_redraw();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                let down = event.state == ElementState::Pressed;
                // Up and down nudge the counter. `dispatch` only
                // activates handlers on pointer-up today (routing for
                // keys is M9), so this reaches the state directly — and
                // the binding still puts it on screen.
                if down {
                    match event.logical_key {
                        WKey::Named(NamedKey::ArrowUp) => self.host.app.count += 1,
                        WKey::Named(NamedKey::ArrowDown) => self.host.app.count -= 1,
                        WKey::Named(NamedKey::Escape) => event_loop.exit(),
                        _ => {}
                    }
                }
                if let Some(k) = named_key(&event.logical_key) {
                    self.host.post(Event::Key { key: k, down });
                }
                if let Some(t) = event.text.as_ref().filter(|_| down) {
                    for ch in t.chars().filter(|c| !c.is_control()) {
                        self.host.post(Event::Text { ch });
                    }
                }
                self.window.as_ref().unwrap().request_redraw();
            }

            WindowEvent::Resized(_) | WindowEvent::RedrawRequested => self.redraw(),

            _ => {}
        }
    }
}

/// winit's key vocabulary to the framework's, for the keys `Key` names.
fn named_key(k: &WKey) -> Option<Key> {
    let WKey::Named(n) = k else { return None };
    Some(match n {
        NamedKey::Enter => Key::Enter,
        NamedKey::Escape => Key::Escape,
        NamedKey::Tab => Key::Tab,
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Delete => Key::Delete,
        NamedKey::ArrowLeft => Key::Left,
        NamedKey::ArrowRight => Key::Right,
        NamedKey::ArrowUp => Key::Up,
        NamedKey::ArrowDown => Key::Down,
        NamedKey::Home => Key::Home,
        NamedKey::End => Key::End,
        _ => return None,
    })
}

/// `--headless <path.ppm> [count]` renders one frame and exits.
///
/// Rung 1 of Lumen's verification ladder: write an image, then *look at
/// it*. A window you cannot screenshot in CI is not a thing you can
/// assert on, and this is the same tree, layout, paint and renderer the
/// window uses — only the event loop differs.
fn headless(path: &str, count: i64) {
    let (w, h) = (440u32, 520u32);
    let mut host = Host::new(build(), w, h);
    host.app.count = count;
    host.set_stylesheet(&stylesheet_for(count.signum()));
    let mut canvas = match FontStack::system() {
        Some(f) => Canvas::with_font(w, h, f),
        None => Canvas::new(w, h),
    };
    let mut metrics = FontStack::system().map(FontMetrics::new);
    match metrics.as_mut() {
        Some(m) => {
            host.pump_with(m);
        }
        None => {
            host.pump();
        }
    }
    canvas.clear(PAGE);
    host.render(&mut canvas);
    std::fs::write(path, canvas.to_ppm()).expect("write frame");
    println!(
        "tally: count={count} nodes={} cmds={} ink={:.1}% -> {path}",
        <app::Page as Widget>::NODE_COUNT,
        host.list.len(),
        canvas.ink_fraction(PAGE) * 100.0,
    );
}

/// `--drive` runs the app through the agent surface and asserts.
///
/// Lumen's `verifying-apps` skill puts this above screenshots: assert on
/// state, semantics and bounds first, pixels second. Every click here
/// goes in by *author id* through `ui.getLayout` — the agent asks where
/// `#inc` is and clicks its centre — so the same path an agent driving
/// this app would take is the path the test takes.
fn drive() {
    use lumen_static_agent::{Agent, TreeRef};
    use serde_json::json;

    let (w, h) = (440u32, 520u32);
    let mut host = Host::new(build(), w, h);
    host.set_stylesheet(&stylesheet_for(0));
    host.pump();

    let agent = Agent::new();
    let centre = |host: &Host<Tally>, id: &str| -> (f32, f32) {
        let tree = TreeRef::of_root(&host.app.ui, &host.styles, &host.states);
        let r = agent.dispatch(
            &tree,
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.getLayout",
                     "params": { "selector": id } }),
        );
        let l = &r["result"];
        let (x, y) = (l["pos"][0].as_f64().unwrap(), l["pos"][1].as_f64().unwrap());
        let (bw, bh) = (l["size"][0].as_f64().unwrap(), l["size"][1].as_f64().unwrap());
        ((x + bw / 2.0) as f32, (y + bh / 2.0) as f32)
    };

    let click = |host: &mut Host<Tally>, id: &str| {
        let (x, y) = centre(host, id);
        host.click(x, y);
        host.pump();
    };

    click(&mut host, "#inc");
    click(&mut host, "#inc");
    click(&mut host, "#inc10");
    assert_eq!(host.app.count, 12, "+1 +1 +10");
    assert_eq!(host.app.ui.card.value.as_str(), "12", "and the binding put it on screen");
    assert_eq!(host.app.ui.card.pill.as_str(), "POSITIVE");

    click(&mut host, "#dec10");
    click(&mut host, "#dec10");
    click(&mut host, "#dec");
    assert_eq!(host.app.count, -9);
    assert_eq!(host.app.ui.card.pill.as_str(), "NEGATIVE");

    click(&mut host, "#reset");
    assert_eq!(host.app.count, 0);
    assert_eq!(host.app.ui.card.value.as_str(), "0");
    assert_eq!(host.app.ui.card.pill.as_str(), "AT ZERO");

    // Bounds, not just state: every control the agent can name is on
    // screen and has area.
    for id in ["#dec10", "#dec", "#inc", "#inc10", "#reset", "#value", "#pill", "#caption"] {
        let (x, y) = centre(&host, id);
        assert!(
            x > 0.0 && y > 0.0 && x < w as f32 && y < h as f32,
            "{id} centre ({x}, {y}) is off screen"
        );
    }

    println!("drive: 7 clicks by author id through ui.getLayout — all assertions passed");
}

fn main() {
    let mut args = std::env::args().skip(1);
    if let Some(flag) = args.next() {
        if flag == "--drive" {
            drive();
            return;
        }
        if flag == "--headless" {
            let path = args.next().unwrap_or_else(|| "tally.ppm".into());
            let count = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            headless(&path, count);
            return;
        }
    }
    let event_loop = EventLoop::<Rpc>::with_user_event().build().expect("event loop");
    // Wait, don't spin: this UI changes only when something happens.
    event_loop.set_control_flow(ControlFlow::Wait);

    // The agent endpoint, on Lumen's default address unless overridden.
    let addr = std::env::var("LUMEN_AGENT_ADDR").unwrap_or_else(|_| "127.0.0.1:9230".into());
    let proxy = event_loop.create_proxy();
    std::thread::spawn(move || serve_agent(addr, proxy));

    event_loop.run_app(&mut WinApp::new()).expect("run");
}
