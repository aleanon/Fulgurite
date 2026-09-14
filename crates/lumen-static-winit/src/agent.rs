//! The agent endpoint: a JSON-RPC socket that drives the **running**
//! window — M23/T23.1c.
//!
//! # Why this is here and not in an example
//!
//! `lumen-static-agent` has had the whole command surface since M15 —
//! `input.click`, `input.type`, `ui.getTree`, `ui.resolve`,
//! `ui.screenshot`, `ui.waitSettled` — and `examples/counter-window` has
//! had a socket for it since M14. What has never existed is a way to
//! reach either from [`crate::run`], which is what every other example
//! uses. So steering a live window meant copying 154 lines into your own
//! binary, and the practical consequence was that a review of
//! `examples/showcase` **restarted the process once per widget** rather
//! than opening the dropdown and picking the next one.
//!
//! That is the third time this shape has appeared: `Config::stylesheet`
//! was added because a turnkey window could not be styled, and
//! `Config::images` because it could not draw a picture. The capability
//! was never the missing part.
//!
//! # The thread rule
//!
//! The socket thread **never touches the tree**. Each request crosses to
//! the event loop through winit's proxy and is answered there, so the
//! tree is only ever read or driven from the thread that owns it. That
//! is inherited from the counter-window endpoint and is the only
//! arrangement that is sound without a lock.

use lumen_static::event::App;
use lumen_static_agent::{AgentHost, TreeRef};
use lumen_static_shell::text::FontMetrics;
use lumen_static_shell::Host;
use std::time::Instant;

use crate::backend::Backend;

/// One request from the socket thread, and where its reply goes.
pub struct Rpc {
    pub req: serde_json::Value,
    pub reply: std::sync::mpsc::Sender<serde_json::Value>,
}

/// What reaches the event loop from somewhere other than the window.
pub enum UserEvent {
    Agent(Rpc),
}

/// The live window, as the agent protocol sees it.
///
/// Generic over the application, which is what makes it reusable where
/// the example's was not: `App::tree` reaches the tree without knowing
/// its field name, and `Backend` reaches the pixels without knowing
/// which renderer drew them.
pub struct Live<'a, A: App> {
    pub host: &'a mut Host<A>,
    pub metrics: &'a mut Option<FontMetrics>,
    pub backend: &'a mut dyn Backend<A>,
    pub page: lumen_static::display::Color,
}

impl<A: App> AgentHost for Live<'_, A> {
    fn tree(&self) -> TreeRef<'_> {
        TreeRef::of_root(self.host.app.tree(), &self.host.styles, &self.host.states)
            .with_sheet(self.host.stylesheet(), self.host.sheet_findings())
    }

    fn enqueue(&mut self, ev: lumen_static::Event) {
        self.host.post(ev);
    }

    /// `app.setPalette` (M25/T25.3): the host owns the palette.
    fn set_palette(&mut self, palette: lumen_static::style::Palette) -> bool {
        self.host.set_palette(palette);
        true
    }

    /// Pump **and present**, then report the frame number.
    ///
    /// Presenting matters: `ui.screenshot` and every pixel assertion
    /// after an `input.click` read the frame this produced, and a pump
    /// alone would hand back the one before the click.
    fn pump(&mut self) -> u64 {
        match self.metrics.as_mut() {
            Some(m) => {
                self.host.pump_with(m);
            }
            None => {
                self.host.pump();
            }
        };
        self.backend.present(self.host, self.page, Instant::now());
        self.host.frame_number()
    }

    fn frame(&self) -> Option<(u32, u32, &[u8])> {
        self.backend.frame_rgba()
    }

    fn scale(&self) -> f32 {
        self.host.scale()
    }

    /// The window read back from the display server — the one view that
    /// sees the blit and the present. `None` where that is not possible
    /// (Wayland, macOS, an unmapped window), which the protocol reports
    /// as "not checkable" rather than guessing.
    fn presented(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        self.backend.fetch_rgba()
    }
}

/// Serve newline-delimited JSON-RPC on `addr`, forwarding each request
/// to the event loop and writing back what it returns.
///
/// Lumen's `run-agent` protocol, so its client works unchanged: one JSON
/// object per line in, one per line out.
pub fn serve(addr: String, proxy: winit::event_loop::EventLoopProxy<UserEvent>) {
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
        let Ok(mut out) = stream.try_clone() else { continue };
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
