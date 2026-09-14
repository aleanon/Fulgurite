//! The agent endpoint: a JSON-RPC socket driving the live window.
//!
//! Split out of `main.rs` by M14/T14.0 — it is not windowing code, and
//! leaving it beside the event loop is part of why that file was 1160
//! lines.


use lumen_static::prelude::*;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::FontMetrics;
use lumen_static_shell::Host;
use lumen_static_winit::Surface;

use crate::app::{stylesheet_for, Tally, PAGE};

/// What the endpoint drives: the window's host and its last frame.
///
/// The shell's `Host` cannot implement `AgentHost` itself without
/// pulling `observe` into every binary that links the shell (F2.5), so
/// the *example* bridges them — which is fine, because this is the one
/// crate that has already opted into everything.
pub struct Live<'a> {
    pub host: &'a mut Host<Tally>,
    pub metrics: &'a mut Option<FontMetrics>,
    pub canvas: &'a mut Canvas,
    pub sign: &'a mut i64,
    /// Borrowed so `ui.screenshot { source: "surface" }` can read the
    /// window back from the display server — the one view that sees the
    /// blit and the present.
    pub surface: &'a mut Option<Surface>,
}

impl lumen_static_agent::AgentHost for Live<'_> {
    fn tree(&self) -> lumen_static_agent::TreeRef<'_> {
        lumen_static_agent::TreeRef::of_root(&self.host.app.ui, &self.host.styles, &self.host.states)
            .with_sheet(self.host.stylesheet(), self.host.sheet_findings())
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

    fn scale(&self) -> f32 {
        self.host.scale()
    }

    /// Read the window back from X11. `softbuffer::Surface::fetch` is
    /// unimplemented on Wayland and macOS, and needs a mapped window on
    /// X11, so a failure here is reported as "not checkable" rather than
    /// guessed at.
    fn presented(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        // The 0RGB-to-RGBA readback is `Surface::fetch_rgba` since
        // M14/T14.0: it is the same for every window, not this app's.
        self.surface.as_mut()?.fetch_rgba(self.canvas.width, self.canvas.height)
    }
}

/// One request from the socket thread, and where its reply goes.
pub struct Rpc {
    pub req: serde_json::Value,
    pub reply: std::sync::mpsc::Sender<serde_json::Value>,
}

/// Everything that reaches the event loop from somewhere other than the
/// window: an agent request, and — with `--features a11y` — the
/// platform accessibility adapter's callbacks, which arrive on their own
/// threads and must be answered on this one.
pub enum UserEvent {
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
pub fn serve_agent(addr: String, proxy: winit::event_loop::EventLoopProxy<UserEvent>) {
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
