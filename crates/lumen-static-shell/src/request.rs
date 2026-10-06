//! **A platform request on its way to the window** — M88/T88.3, seam S1
//! (`docs/plan-platform-requests.md`).
//!
//! The core's [`Request`] is `Copy` and holds a tag; what a clipboard
//! write puts on the clipboard is asked of the application
//! ([`App::clip_for`](lumen_static::event::App::clip_for)) while the host
//! drains, and copied here into an owned [`Payload`], so nothing the
//! window holds borrows the application. These two cross the session
//! seam by value (`Session::take_requests`), as `String` and `Vec<u8>`
//! already do in `copy` and `save` (`docs/step4-hot-reload.md`).

use lumen_static::request::{Clip, Request};

/// What the application wrote for a request, owned. `None` for every
/// request but a clipboard write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Payload {
    None,
    Text(String),
    Html { html: String, alt: String },
}

impl From<Clip<'_>> for Payload {
    fn from(c: Clip<'_>) -> Payload {
        match c {
            Clip::Text(t) => Payload::Text(t.to_owned()),
            Clip::Html { html, alt } => Payload::Html { html: html.to_owned(), alt: alt.to_owned() },
        }
    }
}

/// One request the window must answer before the call that drained it
/// returns (`Host::take_requests`).
#[derive(Clone, Debug, PartialEq)]
pub struct Outgoing {
    pub request: Request,
    pub payload: Payload,
}
