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
//! That is the third time this shape has appeared: a stylesheet field
//! (since M32, a program's `.style(..)`) was added because a turnkey
//! window could not be styled, and
//! `Config::images` because it could not draw a picture. The capability
//! was never the missing part.
//!
//! # The thread rule
//!
//! **No thread but the loop's touches the tree.** Each request crosses
//! to the event loop through winit's proxy and is answered there, so the
//! tree is only ever read or driven from the thread that owns it. That
//! is inherited from the counter-window endpoint and is the only
//! arrangement that is sound without a lock.
//!
//! **Connections run in parallel since M39/T39.3; requests still do
//! not.** The transport gives each connection its own thread, up to
//! sixteen, so an idle client no longer locks every other one out. Each
//! of those threads only sends an [`Rpc`] and waits, and the loop
//! answers them one at a time — two clients' requests interleave on the
//! loop, and neither ever touches the tree.
//!
//! **Widened from "the socket thread" by M28/T28.1**, which is a
//! restatement rather than a change: the socket thread was the only
//! other thread that existed when this was written, and
//! [`crate::UserEvent`] now carries a
//! [`Wake`](crate::UserEvent::Wake) as well as an [`Rpc`], sent by any
//! worker holding a [`ProxyWaker`](crate::ProxyWaker). The rule was
//! always about the tree's ownership and never about sockets, and
//! stating it in terms of one producer is how a second producer gets
//! added without anyone checking it against the rule.
//!
//! The wake carries **no payload**, which is what keeps the widening
//! honest: a worker cannot hand the loop anything to apply, only ask it
//! to look at the inbox it already wrote to. `Rpc` is the exception and
//! pays for it by being answered on the loop's own thread.
//!
//! # The socket is the agent crate's (M39)
//!
//! The loop that reads lines lives in `lumen_static_agent::transport`,
//! and [`serve`] is its handler for this window. There used to be two
//! copies of the loop — this one and `examples/counter-window`'s — and
//! both skipped a line they could not parse, so a web page's HTTP request
//! body executed (R14-F1). The transport ends a connection on any line
//! that is not a JSON object instead, and `scripts/check_one_agent_server.sh`
//! refuses a second listener anywhere outside tests.

/// One request from the socket thread, and where its reply goes.
pub struct Rpc {
    pub req: serde_json::Value,
    pub reply: std::sync::mpsc::Sender<serde_json::Value>,
}

// The live window as the agent sees it moved to the shell with M37
// (`lumen_static_shell::session::Live`): the tree lives wherever the
// session does, which for a reloading window is inside the library. This
// module keeps the socket and the thread rule.

/// Serve the agent on `addr`, forwarding each request to the event loop
/// and writing back what it returns.
///
/// **The socket is `lumen_static_agent::transport`'s** since M39/T39.1:
/// this is its handler, and the handler is the thread rule — each request
/// crosses to the loop through the proxy and is answered there. A line
/// that is not a JSON object ends the connection, which is what keeps a
/// web page's HTTP request from running anything (R14-F1).
///
/// **A failed bind ends the process with status 3** (M42/T42.1,
/// `lumen_static_agent::transport::BIND_FAILED`): a window that was asked
/// for an endpoint and cannot serve it exits and names the address,
/// rather than running on while something else answers on its port.
pub fn serve(addr: String, proxy: winit::event_loop::EventLoopProxy<crate::UserEvent>) {
    lumen_static_agent::transport::serve_addr(&addr, move |req| {
        let (tx, rx) = std::sync::mpsc::channel();
        proxy.send_event(crate::UserEvent::Agent(Rpc { req, reply: tx })).ok()?;
        rx.recv().ok()
    });
}
