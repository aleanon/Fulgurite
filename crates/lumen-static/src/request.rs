//! **Platform requests are data** — M88/T88.3, seam S1
//! (`docs/plan-platform-requests.md`, `docs/plan-iced-roadmap.md` §2.8).
//!
//! A screenshot, a clipboard write and a clipboard read are work only
//! the window can do: they need the renderer or the platform, which no
//! job body on a worker can reach and no handler can name. So they are
//! not [`Job`](crate::spawn::Job)s. An application asks by pushing a
//! [`Request`] onto its own [`Requests`] queue — from a handler, an
//! `apply` or its boot, reached through
//! [`App::requests`](crate::event::App::requests) as the
//! [`Outbox`](crate::spawn::Outbox) is reached through `App::outbox` —
//! and the window drains it, does the work, and answers by **tag**.
//!
//! The direction table, beside the two queues M28 built:
//!
//! | queue | written by | drained by | answered as |
//! |---|---|---|---|
//! | `Event` input | the window | the frame | handlers |
//! | [`Outbox`](crate::spawn::Outbox) | handlers, `apply` | the host, after dispatch | a job's `Msg`, through the inbox |
//! | [`Requests`] | handlers, `apply`, boot | the window, every frame and after every agent call | [`App::answered`](crate::event::App::answered), applied at once |
//!
//! **Rule 3 holds by construction** (`docs/events-decision.md`): what
//! travels is a `Copy` value carrying a tag the application chose; the
//! answer is turned into a message by `App::answered` **in the build
//! loaded when the answer arrives**, and applied inside that call, so no
//! code reference and no converted message is held across a swap.
//!
//! **What the application writes** for a clipboard write is not in the
//! request: the host asks [`App::clip_for`](crate::event::App::clip_for)
//! for it while it drains, synchronously, in the current build.
//!
//! **A request is not a job.** `App::take_cancel` bumps the inbox's epoch
//! and drops results in flight; it does not withdraw a request that has
//! already been taken, which the window answers before the drain that
//! took it returns.
//!
//! Behind `std`, as the outbox is: the firmware links none of it
//! (`docs/results-tasks.md`, T88.3's `nm` count).

/// Which clipboard a request names. `Primary` is the X11 selection
/// (select-to-copy), and carries text only; a platform without one
/// answers [`Refusal::Unsupported`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ClipKind {
    Clipboard,
    Primary,
}

/// What a clipboard read asks for. Append-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ClipFormat {
    Text,
    Html,
    /// RGBA8 pixels (M88/T88.5), answered [`Answer::Image`] — by a window
    /// built with `lumen-static-winit`'s default-off `clipboard-image`;
    /// without it, [`Refusal::Unsupported`]. The variant is here either
    /// way, so an application's request compiles against any build.
    Image,
}

/// **One platform request** — `Copy`, 16 bytes (asserted below), and
/// **append-only**: M89 appends its window variants after these, and a
/// variant is never reordered or removed (`docs/plan-iced-roadmap.md`
/// §2.8).
///
/// `tag` is the application's own name for the request; the answer comes
/// back to [`App::answered`](crate::event::App::answered) with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// What the renderer drew, answered [`Answer::Image`] in physical
    /// pixels — the window's `Backend::frame_rgba`, after the frame's
    /// present-or-skip decision, so it is the frame on screen.
    Screenshot { tag: u64 },
    /// Put what [`App::clip_for`](crate::event::App::clip_for) answers
    /// for `tag` on `kind`. Answered [`Answer::Written`], or refused.
    ClipboardWrite { tag: u64, kind: ClipKind },
    /// Read `kind` as `format`. Answered [`Answer::Text`],
    /// [`Answer::Html`] or [`Answer::Image`], or refused.
    ClipboardRead { tag: u64, kind: ClipKind, format: ClipFormat },
    /// **Open a window on program `program`** of this program's window
    /// table (M89/T89.3, `docs/plan-windows.md`): 1 and up are the table's
    /// entries, 0 is the program itself and is never opened again. The
    /// window is created, and answered [`Answer::WindowOpened`] with its
    /// key, **inside the drain that took this request** — so no open is
    /// pending at a hot swap — or [`Answer::NoSuchProgram`] when the
    /// table has no entry `program`.
    OpenWindow { program: u16, tag: u64 },
    /// Close the window with key `window`. Unanswered; a key no open
    /// window has does nothing. The window's opener is told
    /// ([`Answer::WindowClosed`]). Closing a non-daemon program's primary
    /// window (key 1) ends the process.
    CloseWindow { window: u32 },
    /// Close **the window this application runs in** — the session that
    /// drained it. Unanswered. The primary's ends the process.
    Close,
    /// **End the process** — every window. Unanswered.
    Quit,
}

const _: () = assert!(core::mem::size_of::<Request>() == 16, "Request is 16 B through M89/T89.3 (T89.4's Post makes it 32)");

/// **The tag an unasked answer carries** — `u64::MAX`, reserved by M89
/// (`docs/plan-iced-roadmap.md` §2.8): [`Answer::WindowClosed`] arrives
/// at a window's opener with it, since nothing the opener pushed is being
/// answered. An application chooses its own tags below it.
pub const UNSOLICITED: u64 = u64::MAX;

impl Request {
    /// The application's tag, for the variants that carry one; `None` for
    /// the window requests that are never answered (M89/T89.3).
    pub fn tag(&self) -> Option<u64> {
        match *self {
            Request::Screenshot { tag }
            | Request::ClipboardWrite { tag, .. }
            | Request::ClipboardRead { tag, .. }
            | Request::OpenWindow { tag, .. } => Some(tag),
            Request::CloseWindow { .. } | Request::Close | Request::Quit => None,
        }
    }
}

/// **The application's queue of requests out to the window** — a copy of
/// [`Outbox`](crate::spawn::Outbox)'s shape: fixed capacity, refuses when
/// full and counts the refusal, allocates nothing.
///
/// It lives on the application's state, which is what a handler can
/// reach. An application that issues none answers `None` from
/// [`App::requests`](crate::event::App::requests) and carries no field.
#[derive(Debug)]
pub struct Requests {
    q: [Request; Requests::CAPACITY],
    len: usize,
    refused: usize,
}

impl Default for Requests {
    fn default() -> Self {
        Requests::new()
    }
}

impl Requests {
    /// How many requests one frame may hold. The window drains to empty
    /// on every frame and after every agent call, so this bounds a single
    /// dispatch's asks, not a backlog.
    pub const CAPACITY: usize = 8;

    pub const fn new() -> Requests {
        Requests { q: [Request::Screenshot { tag: 0 }; Requests::CAPACITY], len: 0, refused: 0 }
    }

    /// Ask the window for something. Whether it was queued: **refuses
    /// rather than overwrites** when full, and counts it
    /// ([`Requests::refused`]).
    pub fn push(&mut self, r: Request) -> bool {
        if self.len == Requests::CAPACITY {
            self.refused += 1;
            return false;
        }
        self.q[self.len] = r;
        self.len += 1;
        true
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Pushes refused for want of room. Monotonic.
    pub fn refused(&self) -> usize {
        self.refused
    }

    /// Take everything queued, oldest first, leaving it empty. Writes
    /// into `out`, so the take allocates nothing.
    pub fn drain_into(&mut self, out: &mut [Request; Requests::CAPACITY]) -> usize {
        let n = self.len;
        out[..n].copy_from_slice(&self.q[..n]);
        self.len = 0;
        n
    }
}

/// **What an application puts on the clipboard** — answered by
/// [`App::clip_for`](crate::event::App::clip_for) while the host drains a
/// [`Request::ClipboardWrite`]. Borrowed from the application, and copied
/// into an owned payload before the host returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clip<'a> {
    Text(&'a str),
    /// HTML, with the plain-text alternative a reader that cannot take
    /// HTML gets.
    Html { html: &'a str, alt: &'a str },
    /// An image: RGBA8, `width × height` pixels, row-major from the top
    /// left, so `rgba.len()` is `4 × width × height` (M88/T88.5). Put on
    /// the clipboard by a window built with `clipboard-image`; without
    /// it, the write is answered [`Refusal::Unsupported`].
    Image { width: u32, height: u32, rgba: &'a [u8] },
}

/// Why a request was not done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// This window, build or platform cannot do it (a build without the
    /// window's `clipboard` feature, an image without its
    /// `clipboard-image`, the primary selection off X11 or as anything
    /// but text, a backend that cannot read back).
    Unsupported,
    /// Nothing to do it with: `App::clip_for` answered `None`, or the
    /// clipboard held nothing in the asked format.
    Empty,
    /// The platform was asked and failed.
    Failed,
}

/// **What comes back for a request**, borrowed for the
/// [`App::answered`](crate::event::App::answered) call and no longer: a
/// message that wants the pixels or the text copies what it keeps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Answer<'a> {
    /// A screenshot: RGBA8, `width × height` **physical** pixels, `scale`
    /// physical per logical. Or, since M88/T88.5, a clipboard image read
    /// as [`ClipFormat::Image`]: its own pixels, `scale` 1.
    Image { width: u32, height: u32, scale: f32, rgba: &'a [u8] },
    Text(&'a str),
    Html(&'a str),
    /// A clipboard write landed.
    Written,
    Refused(Refusal),
    /// [`Request::OpenWindow`] opened window `window` (its key, as the
    /// agent's `app.windows` reports it) — answered with the request's
    /// tag, inside the drain that took it (M89/T89.3).
    WindowOpened { window: u32 },
    /// Window `window`, which this application opened, was closed — by
    /// the user, by `Request::CloseWindow`, by its own `Request::Close` or
    /// key hook, or by the agent's `app.close`. **Unasked: it carries
    /// [`UNSOLICITED`]** as its tag (M89/T89.3).
    WindowClosed { window: u32 },
    /// [`Request::OpenWindow`] named a program the window table has no
    /// entry for — answered with the request's tag (M89/T89.3).
    NoSuchProgram { program: u16 },
}
