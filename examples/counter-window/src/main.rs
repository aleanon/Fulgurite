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
use std::time::Instant;
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

use app::{build, stylesheet_for, Tally, INK, PAGE, SURFACE};

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
    /// Borrowed so `ui.screenshot { source: "surface" }` can read the
    /// window back from the display server — the one view that sees the
    /// blit and the present.
    surface: &'a mut Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
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
            // Scoped to the card: the sign only changes `#value`'s
            // colour and `#pill`'s text and background, both inside it.
            // A full `set_stylesheet` re-resolves every node against
            // every rule — 37.8 us at 901 nodes against 2.3 us scoped
            // (`docs/results-cascade-cost.md`). Debug builds check the
            // claim against a full cascade.
            assert!(self.host.restyle_subtree(&stylesheet_for(sign), "card"), "no #card");
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

    /// Read the window back from X11. `softbuffer::Surface::fetch` is
    /// unimplemented on Wayland and macOS, and needs a mapped window on
    /// X11, so a failure here is reported as "not checkable" rather than
    /// guessed at.
    fn presented(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        let surface = self.surface.as_mut()?;
        let px = surface.fetch().ok()?;
        let (w, h) = (self.canvas.width, self.canvas.height);
        if px.len() < (w * h) as usize {
            return None;
        }
        // 0RGB back to RGBA8, which is what the PNG encoder wants.
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for v in &px[..(w * h) as usize] {
            rgba.extend_from_slice(&[(v >> 16) as u8, (v >> 8) as u8, *v as u8, 255]);
        }
        Some((w, h, rgba))
    }
}

/// One request from the socket thread, and where its reply goes.
struct Rpc {
    req: serde_json::Value,
    reply: std::sync::mpsc::Sender<serde_json::Value>,
}

/// Everything that reaches the event loop from somewhere other than the
/// window: an agent request, and — with `--features a11y` — the
/// platform accessibility adapter's callbacks, which arrive on their own
/// threads and must be answered on this one.
enum UserEvent {
    Agent(Rpc),
    #[cfg(feature = "a11y")]
    A11y(accesskit_winit::Event),
}

#[cfg(feature = "a11y")]
impl From<accesskit_winit::Event> for UserEvent {
    fn from(e: accesskit_winit::Event) -> Self {
        UserEvent::A11y(e)
    }
}

/// Serve newline-delimited JSON-RPC on `addr`, forwarding each request
/// to the event loop and writing back what it returns.
///
/// Lumen's `run-agent` protocol, so its client works unchanged: one JSON
/// object per line in, one per line out. The socket thread never touches
/// the tree — every request crosses to the main thread via the proxy,
/// and the tree is only ever read or driven from there.
fn serve_agent(addr: String, proxy: winit::event_loop::EventLoopProxy<UserEvent>) {
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
            if proxy.send_event(UserEvent::Agent(Rpc { req, reply: tx })).is_err() {
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
    /// When the oldest not-yet-presented input arrived. Set by the
    /// first input after a frame, cleared when the frame that consumed
    /// it is on screen: the interval is `FrameTiming::input_latency_ns`.
    input_at: Option<Instant>,
    /// The platform accessibility adapter (T7.5). `observe` has built a
    /// `TreeUpdate` since M5; this is the thing that hands it to the
    /// screen reader. `None` until the window exists, and absent
    /// entirely without the `a11y` feature.
    #[cfg(feature = "a11y")]
    a11y: Option<accesskit_winit::Adapter>,
    /// Kept so the adapter, created in `resumed`, can post its callbacks
    /// back to this loop.
    #[cfg(feature = "a11y")]
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
}

impl WinApp {
    fn new(#[allow(unused)] proxy: winit::event_loop::EventLoopProxy<UserEvent>) -> Self {
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
        let metrics = font.clone().map(FontMetrics::new);
        let mut agent = lumen_static_agent::Agent::new();
        if let Some(f) = font {
            // The audit must measure with the face the window lays out
            // with, or `ui.lint` reports the block stand-in's opinion.
            agent.set_text_measure(Box::new(FontMetrics::new(f)));
        }
        WinApp {
            host,
            canvas,
            metrics,
            window: None,
            surface: None,
            cursor: (0.0, 0.0),
            sign: 0,
            agent,
            input_at: None,
            #[cfg(feature = "a11y")]
            a11y: None,
            #[cfg(feature = "a11y")]
            proxy,
        }
    }

    /// The accessibility tree as the platform wants it: the same
    /// `observe` snapshot the agent reads, with focus filled in from the
    /// host. One source, two consumers — a screen reader and an agent
    /// cannot see different trees, which is the property `observe::dump`
    /// exists to keep.
    #[cfg(feature = "a11y")]
    fn a11y_tree(&self) -> accesskit::TreeUpdate {
        lumen_static::observe::snapshot_focused(
            &self.host.app.ui,
            &self.host.states,
            self.host.focus,
        )
    }

    /// A request from the agent socket. Answered on this thread, against
    /// the live tree, then the window is asked to redraw so what the
    /// agent did is what the user sees.
    fn agent_request(&mut self, event_loop: &ActiveEventLoop, rpc: Rpc) {
        let method = rpc.req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        if method == "app.quit" {
            let _ = rpc.reply.send(serde_json::json!({ "jsonrpc": "2.0", "id": rpc.req.get("id"), "result": { "ok": true } }));
            event_loop.exit();
            return;
        }
        if method.starts_with("input.") {
            self.input_arrived();
        }
        let resp = {
            let mut live = Live {
                host: &mut self.host,
                metrics: &mut self.metrics,
                canvas: &mut self.canvas,
                sign: &mut self.sign,
                surface: &mut self.surface,
            };
            self.agent.dispatch_input(&mut live, &rpc.req)
        };
        let _ = rpc.reply.send(resp);
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// A callback from the platform accessibility adapter. It runs on
    /// this thread, and every arm ends by handing the platform a tree
    /// or an input the rest of the app cannot tell from a click.
    #[cfg(feature = "a11y")]
    fn a11y_event(&mut self, e: accesskit_winit::Event) {
        use accesskit_winit::WindowEvent as AkWindowEvent;
        match e.window_event {
            // The screen reader just attached: it has no tree yet.
            AkWindowEvent::InitialTreeRequested => {
                let update = self.a11y_tree();
                if let Some(a) = self.a11y.as_mut() {
                    a.update_if_active(|| update);
                }
            }
            // A screen-reader activation. It becomes a click at the
            // node's centre — the same two queued events a mouse
            // produces, so it routes, dispatches and shows up in the
            // agent's journal identically. Synthesizing the input rather
            // than calling the handler directly is what keeps one input
            // path (`docs/events-decision.md`).
            AkWindowEvent::ActionRequested(req) => {
                let Some(i) =
                    lumen_static::observe::index_of_node::<app::Page>(req.target_node)
                else {
                    return;
                };
                let st = &self.host.states[i];
                let (x, y) = (st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0);
                self.input_arrived();
                self.host.click(x, y);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            AkWindowEvent::AccessibilityDeactivated => {}
        }
    }

    /// Stamp an input's arrival, keeping the oldest unpresented one.
    fn input_arrived(&mut self) {
        self.input_at.get_or_insert_with(Instant::now);
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
            // Scoped to the card: the sign only changes `#value`'s
            // colour and `#pill`'s text and background, both inside it.
            // A full `set_stylesheet` re-resolves every node against
            // every rule — 37.8 us at 901 nodes against 2.3 us scoped
            // (`docs/results-cascade-cost.md`). Debug builds check the
            // claim against a full cascade.
            assert!(self.host.restyle_subtree(&stylesheet_for(sign), "card"), "no #card");
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

        let frame_start = Instant::now();
        self.pump();
        self.sync_sign();
        self.pump();
        let (layout_ns, paint_ns) = self.host.timings();

        let Some(surface) = self.surface.as_mut() else { return };
        surface.resize(w, h).expect("resize surface");
        let mut buf = surface.buffer_mut().expect("acquire surface buffer");

        // What this buffer needs redrawn. Not `host.damage()`: the
        // surface is multi-buffered, and the one just handed back may
        // hold the frame from two presents ago or nothing at all. It
        // says which through `age`, and drawing one frame's difference
        // onto a two-frame-old base is the classic double-buffered
        // smear (`crates/lumen-static/src/retain.rs`).
        let viewport = [0.0, 0.0, size.width as f32, size.height as f32];
        let area = self.host.take_damage_for_age(buf.age()).clip_to(viewport);

        if std::env::var_os("LUMEN_TRACE").is_some() {
            eprintln!(
                "redraw: age={} area={:?} canvas={}x{} win={}x{}",
                buf.age(),
                area,
                self.canvas.width,
                self.canvas.height,
                size.width,
                size.height
            );
        }

        // Render only what changed — but *always* present.
        //
        // T8b.4 said "no damage, no frame", and the first implementation
        // took that literally and returned without presenting. That is
        // wrong, and it emptied the window: `RedrawRequested` is not
        // only ours. X11 sends it on **expose** — when the window is
        // mapped, uncovered or moved — and an expose means the *server*
        // lost the content, which the app's own damage knows nothing
        // about. Skipping the present left whatever was in the shared
        // memory segment on screen, which is another window's pixels.
        //
        // winit 0.30 does not say which kind of `RedrawRequested` this
        // is, and guessing wrong is invisible in every headless test, so
        // the frame is always presented. What T8b.4 actually saves is
        // the *raster*, which is the expensive half: render p95
        // 0.089 ms against present p95 0.044 ms
        // (`docs/results-frame-cost.md`).
        let t = Instant::now();
        let render_ns = match area {
            Some(area) => {
                self.canvas.clear_within(area, PAGE);
                self.canvas.render_within(&self.host.list, area);
                t.elapsed().as_nanos() as u64
            }
            None => 0,
        };

        let t = Instant::now();
        // Blit only what changed, but present all of it.
        //
        // The two halves answer different questions. The *buffer* only
        // needs the damage written into it, because at `age >= 1` it
        // already holds the frame we presented last — and at age 0 the
        // damage is `All` by construction, so this is a full blit
        // anyway. The *present* has to be whole, because this redraw may
        // be an expose and the display server has lost everything.
        if let Some(area) = area {
            self.canvas.blit_0rgb_within(&mut buf, area);
        }
        // `present`, not `present_with_damage`: the partial form would
        // tell the server to copy only the damaged rows, which is
        // exactly wrong when the redraw is an expose. (`present` is
        // itself `present_with_damage` over the full rect in every
        // softbuffer backend, so only the hint is given up.)
        buf.present().expect("present");
        let present_ns = t.elapsed().as_nanos() as u64;

        // The frame is on screen: this is where the journal, the audit
        // and the timings are recorded, so `ui.lastChange`, `ui.lint`
        // and `app.perf` describe what a person can see.
        let timing = lumen_static_agent::FrameTiming {
            layout_ns,
            paint_ns,
            render_ns,
            present_ns,
            frame_ns: frame_start.elapsed().as_nanos() as u64,
            input_latency_ns: self.input_at.take().map(|t| t.elapsed().as_nanos() as u64),
            damaged_px: Some(area.map_or(0, |a| {
                ((a[2] - a[0]) as f64 * (a[3] - a[1]) as f64).max(0.0) as u64
            })),
        };
        let mut tree = lumen_static_agent::TreeMut::of_root(
            &mut self.host.app.ui,
            &self.host.styles,
            &self.host.states,
        );
        self.agent.frame_done(&mut tree, timing);

        // The screen reader's view of the frame that was just presented.
        // Pushed here, beside `frame_done`, so the accessibility tree,
        // the agent's journal and the pixels are all the same frame —
        // the drift a separate update site would introduce is exactly
        // what makes a screen reader read a stale label.
        #[cfg(feature = "a11y")]
        {
            let update = self.a11y_tree();
            if let Some(a) = self.a11y.as_mut() {
                a.update_if_active(|| update);
            }
        }
        if std::env::var_os("LUMEN_TRACE").is_some() {
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

impl ApplicationHandler<UserEvent> for WinApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("lumen-static — tally")
            .with_inner_size(winit::dpi::LogicalSize::new(440.0, 520.0));
        let window = Rc::new(event_loop.create_window(attrs).expect("create window"));
        // Before the window is shown, per the adapter's contract.
        #[cfg(feature = "a11y")]
        {
            self.a11y = Some(accesskit_winit::Adapter::with_event_loop_proxy(
                event_loop,
                &window,
                self.proxy.clone(),
            ));
        }
        let ctx = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        self.surface =
            Some(softbuffer::Surface::new(&ctx, window.clone()).expect("softbuffer surface"));
        // Ask for the first frame explicitly: nothing else will.
        window.request_redraw();
        self.window = Some(window);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, ev: UserEvent) {
        match ev {
            UserEvent::Agent(rpc) => self.agent_request(event_loop, rpc),
            #[cfg(feature = "a11y")]
            UserEvent::A11y(e) => self.a11y_event(e),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        // The adapter watches for focus and window-geometry changes it
        // must report; it never consumes an event.
        #[cfg(feature = "a11y")]
        if let (Some(a), Some(w)) = (self.a11y.as_mut(), self.window.as_ref()) {
            a.process_event(w, &event);
        }

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
                self.input_arrived();
                let (x, y) = self.cursor;
                self.host.post(match state {
                    ElementState::Pressed => Event::PointerDown { x, y },
                    ElementState::Released => Event::PointerUp { x, y },
                });
                self.window.as_ref().unwrap().request_redraw();
            }

            WindowEvent::MouseWheel { delta, .. } => {
                self.input_arrived();
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y * 20.0,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                self.host.post(Event::Scroll { x: self.cursor.0, y: self.cursor.1, dy });
                self.window.as_ref().unwrap().request_redraw();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                self.input_arrived();
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
    if std::env::var_os("LUMEN_PROFILE").is_some() {
        profile(&mut host, &mut canvas, metrics.as_mut());
    }
    println!(
        "tally: count={count} nodes={} cmds={} ink={:.1}% -> {path}",
        <app::Page as Widget>::NODE_COUNT,
        host.list.len(),
        canvas.ink_fraction(PAGE) * 100.0,
    );
}

/// Per-stage frame cost, 50 frames each, median. Temporary harness for
/// the 2026-09-03 profiling round; the numbers go in
/// `docs/results-frame-cost.md`.
fn profile(host: &mut Host<Tally>, canvas: &mut Canvas, mut metrics: Option<&mut FontMetrics>) {
    let med = |name: &str, mut f: Box<dyn FnMut() + '_>| {
        let mut v: Vec<u128> = (0..50)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed().as_micros()
            })
            .collect();
        v.sort();
        eprintln!("{name:<22} {:>8} us", v[25]);
    };
    let (w, h) = (canvas.width, canvas.height);
    med(
        "pump(layout+paint)",
        Box::new(|| match metrics.as_mut() {
            Some(m) => {
                host.pump_with(*m);
            }
            None => {
                host.pump();
            }
        }),
    );
    med("clear", Box::new(|| canvas.clear(PAGE)));
    med("render(all)", Box::new(|| host.render(canvas)));
    // Per command class.
    let mut only = |name: &str, keep: Box<dyn Fn(&lumen_static::Cmd) -> bool>| {
        let mut sub = lumen_static::DisplayList::new();
        for c in &host.list.cmds {
            if keep(c) {
                sub.cmds.push(*c);
            }
        }
        sub.text = host.list.text.clone();
        let mut v: Vec<u128> = (0..50)
            .map(|_| {
                let t = Instant::now();
                lumen_static::Renderer::render(canvas, &sub, w, h);
                t.elapsed().as_micros()
            })
            .collect();
        v.sort();
        eprintln!("{name:<22} {:>8} us  ({} cmds)", v[25], sub.cmds.len());
    };
    use lumen_static::Cmd;
    only("  rect", Box::new(|c| matches!(c, Cmd::Rect { .. })));
    only("  rounded_rect", Box::new(|c| matches!(c, Cmd::RoundedRect { .. })));
    only("  text", Box::new(|c| matches!(c, Cmd::Text { .. })));
    let mut buf = vec![0u32; (w * h) as usize];
    med("blit rgba->0rgb", Box::new(|| canvas.blit_0rgb(&mut buf)));

    // The partial path (M8b), against the damage a count change makes.
    // Bank the seed frame first, or the damage is the whole viewport.
    host.take_damage_for_age(1);
    host.app.count += 1;
    match metrics.as_mut() {
        Some(m) => {
            host.pump_with(*m);
        }
        None => {
            host.pump();
        }
    }
    if let Some(area) =
        host.take_damage_for_age(1).clip_to([0.0, 0.0, w as f32, h as f32])
    {
        let px = ((area[2] - area[0]) * (area[3] - area[1])) as u64;
        eprintln!("{:<22} {:>8} px  {area:?}", "damage", px);
        med("  clear_within", Box::new(|| canvas.clear_within(area, PAGE)));
        med("  render_within", Box::new(|| canvas.render_within(&host.list, area)));
        med("  blit_within", Box::new(|| canvas.blit_0rgb_within(&mut buf, area)));
        // What each command class costs inside that rect.
        let mut only_in = |name: &str, keep: Box<dyn Fn(&lumen_static::Cmd) -> bool>| {
            let mut sub = lumen_static::DisplayList::new();
            for c in &host.list.cmds {
                if keep(c) {
                    sub.cmds.push(*c);
                }
            }
            sub.text = host.list.text.clone();
            let mut v: Vec<u128> = (0..50)
                .map(|_| {
                    let t = Instant::now();
                    canvas.render_within(&sub, area);
                    t.elapsed().as_micros()
                })
                .collect();
            v.sort();
            eprintln!("{name:<22} {:>8} us  ({} cmds)", v[25], sub.cmds.len());
        };
        only_in("    rounded_rect", Box::new(|c| matches!(c, Cmd::RoundedRect { .. })));
        only_in("    shadow", Box::new(|c| matches!(c, Cmd::Shadow { .. })));
        only_in("    text", Box::new(|c| matches!(c, Cmd::Text { .. })));
    }
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

// ---- `--form`: the M8/M9 widgets through the real rasteriser --------

/// A scrollable form: a field, and more rows than fit.
#[derive(lumen_static::StaticTree, Default)]
#[widget(column, spacing = 10.0)]
struct FormBody {
    #[widget_id("name")]
    name: lumen_static::input::TextInput,
    #[widget_id("r1")]
    r1: Label,
    #[widget_id("r2")]
    r2: Label,
    #[widget_id("r3")]
    r3: Label,
    #[widget_id("r4")]
    r4: Label,
    #[widget_id("r5")]
    r5: Label,
}

#[derive(lumen_static::StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct FormUi {
    #[widget_id("scroll")]
    scroll: lumen_static::scroll::ScrollView<FormBody>,
}

#[derive(lumen_static::App, Default)]
struct Form {
    #[tree]
    ui: FormUi,
}

/// Render the form headlessly and assert the properties only a real
/// rasteriser can settle.
///
/// `ScrollView` and `TextInput` are covered by unit tests that count
/// display-list commands. That is not the same as knowing they *paint*:
/// this repository has twice shipped a construct whose commands were
/// right and whose pixels were not — a repeater that drew 44 px outside
/// its viewport for three milestones, and a renderer that ignored clip
/// scopes entirely. So the new widgets go through `Canvas` here, in
/// `just fixtures`, and the frame is asserted.
fn form(path: &str) {
    let (w, h) = (300u32, 200u32);
    let mut host: Host<Form> = Host::new(Form::default(), w, h);
    {
        let b = &mut host.app.ui.scroll.content;
        b.name.set_str("typed into a real field");
        for (i, l) in [&mut b.r1, &mut b.r2, &mut b.r3, &mut b.r4, &mut b.r5].into_iter().enumerate()
        {
            l.set(match i {
                0 => "row one",
                1 => "row two",
                2 => "row three",
                3 => "row four",
                _ => "row five — below the fold",
            });
        }
    }
    host.set_stylesheet(&[
        Rule::new(Selector::Kind("FormUi"), Patch::new().background(PAGE).padding(Edges::all(12.0))),
        Rule::new(Selector::Id("name"), Patch::new().background(SURFACE).radius(8.0).width(Length::FILL)),
        Rule::new(Selector::Kind("label"), Patch::new().color(INK)),
        Rule::new(Selector::Id("name"), Patch::new().color(INK)),
    ]);
    let mut metrics = FontStack::system().map(FontMetrics::new);
    let mut canvas = match FontStack::system() {
        Some(f) => Canvas::with_font(w, h, f),
        None => Canvas::new(w, h),
    };
    let pump = |host: &mut Host<Form>, m: &mut Option<FontMetrics>| match m.as_mut() {
        Some(m) => {
            host.pump_with(m);
        }
        None => {
            host.pump();
        }
    };
    pump(&mut host, &mut metrics);

    let content_h = host.states[2].size.h;
    let viewport_h = host.states[1].size.h;
    assert!(content_h > viewport_h, "the form must overflow: {content_h} vs {viewport_h}");

    // Scroll to the bottom through the real event path, not by writing
    // the field: this asserts C3's `on_event` reaches the scroller.
    for _ in 0..20 {
        host.post(Event::Scroll { x: 150.0, y: 100.0, dy: -20.0 });
        pump(&mut host, &mut metrics);
    }
    let at_end = host.app.ui.scroll.scroll;
    assert!(at_end > 0.0, "the wheel scrolled it");
    host.post(Event::Scroll { x: 150.0, y: 100.0, dy: -20.0 });
    pump(&mut host, &mut metrics);
    assert_eq!(host.app.ui.scroll.scroll, at_end, "and it stops at the end");

    canvas.clear(PAGE);
    host.render(&mut canvas);
    let ink = canvas.ink_fraction(PAGE);
    assert!(ink > 0.01, "the form is blank ({ink:.3} inked)");

    // Scrolled to the bottom, the *first* row is above the viewport and
    // the last is inside it. The scroller's clip is what keeps the one
    // that has left from painting there anyway — the property a
    // repeater got wrong for three milestones, and only geometry plus
    // pixels together can show it.
    let top = host.states[1].pos.y;
    let field_y = host.states[3].pos.y;
    let last = &host.states[8];
    assert!(field_y < top, "the field has scrolled above the viewport (y={field_y}, top={top})");
    assert!(
        last.pos.y >= top && last.pos.y + last.size.h <= top + viewport_h + 0.001,
        "the last row is in view at the bottom (y={}, h={})",
        last.pos.y,
        last.size.h
    );

    std::fs::write(path, canvas.to_ppm()).expect("write frame");
    println!(
        "form: nodes={} cmds={} scroll={:.0}/{:.0} ink={:.1}% -> {path}",
        <FormUi as Widget>::NODE_COUNT,
        host.list.len(),
        host.app.ui.scroll.scroll,
        content_h - viewport_h,
        ink * 100.0,
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    if let Some(flag) = args.next() {
        if flag == "--drive" {
            drive();
            return;
        }
        if flag == "--form" {
            let path = args.next().unwrap_or_else(|| "form.ppm".into());
            form(&path);
            return;
        }
        if flag == "--headless" {
            let path = args.next().unwrap_or_else(|| "tally.ppm".into());
            let count = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            headless(&path, count);
            return;
        }
    }
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    // Wait, don't spin: this UI changes only when something happens.
    event_loop.set_control_flow(ControlFlow::Wait);

    // The agent endpoint, on Lumen's default address unless overridden.
    let addr = std::env::var("LUMEN_AGENT_ADDR").unwrap_or_else(|_| "127.0.0.1:9230".into());
    let proxy = event_loop.create_proxy();
    std::thread::spawn(move || serve_agent(addr, proxy));

    let mut app = WinApp::new(event_loop.create_proxy());
    event_loop.run_app(&mut app).expect("run");
}
