//! The tally's window: the parts a turnkey `lumen_static_winit::run`
//! cannot supply.
//!
//! Everything generic — window creation, the software raster, damage,
//! buffer age, the scale factor, the font, event translation — is
//! `lumen-static-winit` as of M14/T14.0. What is left here is what makes
//! *this* application not the minimal one:
//!
//! - an **agent endpoint** on a socket, answered on this thread;
//! - a **platform accessibility adapter**, behind `a11y`;
//! - a **sign-dependent stylesheet** re-cascaded between pumps;
//! - **Up/Down nudging the counter**, which is app behaviour.
//!
//! `examples/minimal` is the other end of the same seam: 24 lines, no
//! event loop of its own.

use std::rc::Rc;
use std::time::Instant;

use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::Host;
use lumen_static_winit::{translate, Cursor, Surface};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WKey, NamedKey};
use winit::window::{Window, WindowId};

use crate::agent_socket::{serve_agent, Live, Rpc, UserEvent};
use crate::app::{build, stylesheet_for, Tally, PAGE};

pub struct WinApp {
    pub host: Host<Tally>,
    metrics: Option<FontMetrics>,
    font: Option<FontStack>,
    /// Owned here, not by `Surface`: the agent endpoint answers
    /// `ui.screenshot` before `resumed` has created a surface, and a
    /// canvas that lived inside one would make that a `None`.
    canvas: lumen_static_shell::software::Canvas,
    /// `None` until `resumed`; winit owns window creation.
    window: Option<Rc<Window>>,
    surface: Option<Surface>,
    cursor: Cursor,
    /// Single/double/triple click counting (T14.3): it needs a clock,
    /// which the core does not have.
    clicks: translate::ClickCounter,
    /// The cursor the window is showing, so an unchanged frame does not
    /// ask the platform to set it again (T14.6).
    cursor_shown: lumen_static::style::Cursor,
    /// Modifiers as of the last `ModifiersChanged`. Needed before the
    /// event is queued: it decides whether a keystroke types (T14.2).
    mods: lumen_static::Mods,
    /// Last sign the stylesheet was cascaded for.
    sign: i64,
    agent: lumen_static_agent::Agent,
    /// When the oldest not-yet-presented input arrived. Set by the first
    /// input after a frame, cleared when the frame that consumed it is
    /// on screen: the interval is `FrameTiming::input_latency_ns`.
    input_at: Option<Instant>,
    /// The platform accessibility adapter (T7.5). `observe` has built a
    /// `TreeUpdate` since M5; this is the thing that hands it to the
    /// screen reader.
    #[cfg(feature = "a11y")]
    a11y: Option<accesskit_winit::Adapter>,
    #[cfg(feature = "a11y")]
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
}

impl WinApp {
    fn new(#[allow(unused)] proxy: winit::event_loop::EventLoopProxy<UserEvent>) -> Self {
        let (w, h) = (440, 520);
        let mut host = Host::new(build(), w, h);
        host.set_stylesheet(&stylesheet_for(0));
        let font = FontStack::system();
        if font.is_none() {
            eprintln!("no system font found — falling back to block glyphs");
        }
        let metrics = font.clone().map(FontMetrics::new);
        let mut agent = lumen_static_agent::Agent::new();
        if let Some(f) = font.clone() {
            // The audit must measure with the face the window lays out
            // with, or `ui.lint` reports the block stand-in's opinion.
            agent.set_text_measure(Box::new(FontMetrics::new(f)));
        }
        let canvas = lumen_static_winit::canvas(w, h, font.as_ref());
        WinApp {
            host,
            metrics,
            font,
            canvas,
            window: None,
            surface: None,
            cursor: Cursor::default(),
            clicks: Default::default(),
            cursor_shown: lumen_static::style::Cursor::Default,
            mods: lumen_static::Mods::NONE,
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
            self.host.input.focus,
        )
    }

    /// A request from the agent socket. Answered on this thread, against
    /// the live tree, then the window is asked to redraw so what the
    /// agent did is what the user sees.
    fn agent_request(&mut self, event_loop: &ActiveEventLoop, rpc: Rpc) {
        let method = rpc.req.get("method").and_then(serde_json::Value::as_str).unwrap_or("");
        if method == "app.quit" {
            let _ = rpc.reply.send(serde_json::json!({
                "jsonrpc": "2.0", "id": rpc.req.get("id"), "result": { "ok": true }
            }));
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
    /// this thread, and every arm ends by handing the platform a tree or
    /// an input the rest of the app cannot tell from a click.
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
                let Some(i) = lumen_static::observe::index_of_node::<crate::app::Page>(req.target_node)
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

    fn redraw(&mut self) {
        let Some(window) = self.window.clone() else { return };
        if self.surface.is_none() {
            return;
        }
        let size = window.inner_size();
        lumen_static_winit::sync_size(
            &mut self.canvas,
            &mut self.host,
            self.font.as_ref(),
            size.width,
            size.height,
        );

        let start = Instant::now();
        // Twice, with the cascade between: a handler may have flipped
        // the sign, and the stylesheet that follows from it must be
        // resolved before the frame that shows it. Both pumps happen
        // before the surface is borrowed, because they need `&mut self`.
        self.pump();
        self.sync_sign();
        self.pump();
        let f = {
            let surface = self.surface.as_mut().expect("checked above");
            surface.present(&mut self.host, &mut self.canvas, PAGE, start)
        };

        lumen_static_winit::cursor::apply(&window, self.host.cursor(), &mut self.cursor_shown);

        // The frame is on screen: this is where the journal, the audit
        // and the timings are recorded, so `ui.lastChange`, `ui.lint`
        // and `app.perf` describe what a person can see.
        let timing = lumen_static_agent::FrameTiming {
            layout_ns: f.layout_ns,
            paint_ns: f.paint_ns,
            render_ns: f.render_ns,
            present_ns: f.present_ns,
            frame_ns: f.frame_ns,
            input_latency_ns: self.input_at.take().map(|t| t.elapsed().as_nanos() as u64),
            damaged_px: Some(f.damaged_px),
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
    }

    /// The tally's own keys: Up and Down nudge the counter, Escape
    /// quits. App behaviour, not the window's — which is why it is here
    /// and not in `lumen-static-winit`.
    fn app_keys(&mut self, event_loop: &ActiveEventLoop, key: &WKey) {
        match key {
            WKey::Named(NamedKey::ArrowUp) => self.host.app.count += 1,
            WKey::Named(NamedKey::ArrowDown) => self.host.app.count -= 1,
            WKey::Named(NamedKey::Escape) => event_loop.exit(),
            _ => {}
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
        // Before the first frame: layout is logical, the surface is
        // physical, and this is the only thing that relates them (M10b).
        let scale = window.scale_factor() as f32;
        self.host.set_scale(scale);
        if let Some(m) = self.metrics.as_mut() {
            m.set_scale(scale);
        }
        self.surface = Some(Surface::new(window.clone()));
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

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                // A window can be dragged between monitors.
                self.host.set_scale(scale_factor as f32);
                if let Some(m) = self.metrics.as_mut() {
                    m.set_scale(scale_factor as f32);
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::Resized(_) | WindowEvent::RedrawRequested => self.redraw(),

            ref ev => {
                if let WindowEvent::KeyboardInput { event, .. } = ev {
                    if event.state == ElementState::Pressed {
                        self.app_keys(event_loop, &event.logical_key);
                    }
                }
                // Every one of these becomes an `Event` in the host's
                // single queue — the same queue the agent protocol's
                // `input.*` methods post into, which is what makes an
                // agent session replay as a test.
                let host = &self.host;
                let t =
                    translate::window_event(ev, &mut self.cursor, &mut self.clicks, self.mods, |x, y| {
                        host.to_logical(x, y)
                    });
                if let WindowEvent::ModifiersChanged(m) = ev {
                    self.mods = translate::mods(m.state());
                }
                if t.is_input {
                    self.input_arrived();
                }
                let any = t.events[0].is_some();
                for e in t.iter() {
                    self.host.post(e);
                }
                if any {
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
        }
    }
}

/// Open the tally's window, with the agent endpoint listening.
pub fn run() {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    // The agent endpoint, on Lumen's default address unless overridden.
    let addr = std::env::var("LUMEN_AGENT_ADDR").unwrap_or_else(|_| "127.0.0.1:9230".into());
    let proxy = event_loop.create_proxy();
    std::thread::spawn(move || serve_agent(addr, proxy));

    let mut win = WinApp::new(event_loop.create_proxy());
    event_loop.run_app(&mut win).expect("run");
}
