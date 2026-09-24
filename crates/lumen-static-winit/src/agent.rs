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

/// One request from the socket thread, and where its reply goes.
pub struct Rpc {
    pub req: serde_json::Value,
    pub reply: std::sync::mpsc::Sender<serde_json::Value>,
}

// The live window as the agent sees it moved to the shell with M37
// (`lumen_static_shell::session::Live`): the tree lives wherever the
// session does, which for a reloading window is inside the library. This
// module keeps the socket and the thread rule.

/// Serve newline-delimited JSON-RPC on `addr`, forwarding each request
/// to the event loop and writing back what it returns.
///
/// Lumen's `run-agent` protocol, so its client works unchanged: one JSON
/// object per line in, one per line out.
pub fn serve(addr: String, proxy: winit::event_loop::EventLoopProxy<crate::UserEvent>) {
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
            if proxy.send_event(crate::UserEvent::Agent(Rpc { req, reply: tx })).is_err() {
                return;
            }
            let Ok(resp) = rx.recv() else { return };
            if writeln!(out, "{resp}").is_err() {
                break;
            }
        }
    }
}
