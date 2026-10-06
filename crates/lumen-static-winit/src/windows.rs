//! One window's state — M89/T89.1 (`docs/plan-windows.md`).
//!
//! **`Win` is shared state plus a `Vec<WindowSlot>`.** Everything a
//! window owns — its session, its winit window and renderer, its images,
//! its pointer and modifiers, its frame clock and deadline, its present
//! bookkeeping, its accessibility adapter and its render face — lives in
//! a [`WindowSlot`]; what the process has one of — the clipboard, the
//! reload state, the journal switch, `app.quit`, the frame report, the
//! proxy and the primary program's `Config` — stays on `Win`. A window
//! event finds its slot by `WindowId`, a worker's wake by the slot's key
//! ([`crate::UserEvent::Wake`] carries it, and [`crate::ProxyWaker`] is
//! built with it), so a wake asks *its own* window for a frame.
//!
//! **A `Vec` and a linear scan rather than a map**, the reasoning the
//! deleted `Panes` carried: an application with enough windows for a
//! hash to pay for itself is not one this is written for, and a `Vec`
//! keeps the common case — one window — to one comparison.
//!
//! **Why `Pane` and `Panes` were deleted rather than reused** (review A27):
//! a `Pane<A>` held a loader-side `Host<A>`, and M37 moved the host
//! behind [`Session`] because loader code cannot be reloaded
//! (`lumen_static_shell::session`'s module doc). A slot holds a
//! `Box<dyn Session>`, which is strategy-free and may live in a hot
//! library; the strategy is the session's type parameter, chosen by the
//! program (`tests/inval_param.rs`).
//!
//! **A key, never a winit type, names a window outside this crate**
//! (D1 move d, P21): keys are `u32`s from 1, never reused in a process;
//! key 0 is reserved for a daemon's windowless session (T89.6). At T89.1
//! there is exactly one slot, key 1, and a one-window program behaves as
//! it did; routing by id is observable only with two windows (T89.3's
//! XTest click).
//!
//! **More than one since M89/T89.3**: a `Request::OpenWindow` drained from
//! any slot's session pushes a slot for the window program it names, keyed
//! by the next unused key, and creates its window inside the same drain;
//! a closed window's slot is removed. Closing the primary (key 1) of a
//! program that is not a daemon ends the process (`docs/plan-windows.md`,
//! "Closing a window and quitting are different").

use std::sync::Arc;
use std::time::Instant;

use lumen_static_shell::session::Session;
use lumen_static_shell::text::FontStack;
use winit::event_loop::ControlFlow;
use winit::window::Window;

use crate::{translate, Backend, Cursor};

/// The primary window's key: the program's own window (program 0).
pub const PRIMARY: u32 = 1;

/// One window and everything that is its own (module doc).
///
/// Fields are the crate's; outside it a slot is named only for its
/// [`key`](Self::key) and its strategy-free [`session`](Self::session).
pub struct WindowSlot {
    /// This window's key (module doc): 1 for the primary.
    pub(crate) key: u32,
    /// The program this window runs: 0 for the program itself, 1 and up
    /// for its window table's entries (M89/T89.3). `app.windows` reports
    /// it (M89/T89.2).
    #[cfg_attr(not(feature = "agent"), allow(dead_code))]
    pub(crate) program: u16,
    /// Who opened this window and with which tag: the opener's key and
    /// its `Request::OpenWindow` tag (M89/T89.3). `None` for the primary.
    /// Closing this window tells the opener, `Answer::WindowClosed`.
    pub(crate) opener: Option<(u32, u64)>,
    /// The window's size in logical pixels, its page and its renderer
    /// preference: the primary's from the program's `Config`, a window
    /// program's from its `WindowSpec` (M89/T89.3).
    pub(crate) spec: (u32, u32, lumen_static::Color, crate::Prefer),
    /// The application this window runs, whichever strategy it chose.
    pub(crate) session: Box<dyn Session>,
    /// `None` until `resumed`; winit owns window creation.
    pub(crate) window: Option<Arc<Window>>,
    /// The renderer, chosen once the window exists — a GPU surface
    /// cannot be made without one, and neither can a softbuffer one.
    pub(crate) backend: Option<Box<dyn Backend>>,
    /// Where `Cmd::Image` gets its pixels: handed to each backend
    /// `resumed` makes and taken back by `suspended` (M47/T47.7).
    pub(crate) images: Option<Box<dyn lumen_static::ImageSource>>,
    /// The face this window's renderer rasterises with, **in its
    /// session's mode** (`Session::shaping`, M81/T81.4; second review X2):
    /// per slot, because each window program is its own `Program`.
    pub(crate) font: Option<FontStack>,
    /// This window's keyed waker ([`crate::ProxyWaker`]), installed on
    /// its session and on every session a swap puts here.
    pub(crate) waker: Option<Arc<dyn lumen_static::inbox::Waker>>,
    pub(crate) cursor: Cursor,
    /// Single/double/triple click counting (T14.3).
    pub(crate) clicks: translate::ClickCounter,
    /// Modifiers as of this window's last `ModifiersChanged` (T14.2).
    pub(crate) mods: lumen_static::Mods,
    /// The cursor the window is showing, so an unchanged frame does not
    /// ask the platform again.
    pub(crate) cursor_shown: lumen_static::style::Cursor,
    /// The title the window is showing, so an unchanged one is not set
    /// again.
    pub(crate) title_shown: String,
    /// When this window's last frame was drawn, for its animation tick;
    /// cleared while a panic is shown (M47/T47.1).
    pub(crate) last_frame: Option<Instant>,
    /// What this window waits on (`control_flow_for`, M47/T47.1); the
    /// loop waits on the soonest of every slot's.
    pub(crate) wait: ControlFlow,
    /// Dropped presents in a row (`present_retry`, M47/T47.4).
    pub(crate) present_drops: u32,
    /// The pending redraw was this window's own request (M53/T53.4,
    /// `present_needed`). **Per slot and cleared on the slot's frame**:
    /// kept anywhere else, one window's requests would decide another's
    /// presents (T89.1's injection).
    pub(crate) own_redraw: bool,
    /// The next frame presents whatever its damage says: a resize, or a
    /// dropped present's retry.
    pub(crate) must_present: bool,
    /// Whether the last frame ended on M36's screen (M84/T84.6).
    pub(crate) was_failed: bool,
    /// Frames presented, and redraws that presented nothing —
    /// `app.presents` (M53/T53.4).
    pub(crate) presents: (u64, u64),
    /// When the oldest input not yet on screen arrived (M60/T60.3).
    pub(crate) input_at: Option<Instant>,
    /// The IME cursor area last passed to this window's
    /// `set_ime_cursor_area`, physical pixels (M80/T80.5) — `app.imeArea`.
    pub(crate) ime_area: Option<[f32; 4]>,
    /// Whether this window's last frame drew the reload bar (M37).
    #[cfg(feature = "hot")]
    pub(crate) bar_shown: bool,
    /// This window's platform accessibility adapter (M54/T54.2).
    #[cfg(feature = "a11y")]
    pub(crate) a11y: Option<crate::a11y::Accessible>,
}

impl WindowSlot {
    /// A slot for `session`, with no window yet: `resumed` opens it.
    /// Its face is built in the session's mode before any backend exists
    /// (M81/T81.4, second review T24).
    pub(crate) fn new(key: u32, session: Box<dyn Session>, images: Option<Box<dyn lumen_static::ImageSource>>) -> Self {
        let font = FontStack::system().map(|f| f.with_shaping(session.shaping()));
        if font.is_none() {
            // A machine with no font we recognise still gets a window; it
            // just gets the block stand-in, which is honest about it.
            eprintln!("no system font found — falling back to block glyphs");
        }
        WindowSlot {
            key,
            program: 0,
            opener: None,
            spec: (640, 480, lumen_static::Color(255, 255, 255, 255), crate::Prefer::Auto),
            session,
            window: None,
            backend: None,
            images,
            font,
            waker: None,
            cursor: Cursor::default(),
            clicks: Default::default(),
            mods: lumen_static::Mods::NONE,
            cursor_shown: lumen_static::style::Cursor::Default,
            title_shown: String::new(),
            last_frame: None,
            wait: ControlFlow::Wait,
            present_drops: 0,
            own_redraw: false,
            must_present: false,
            was_failed: false,
            presents: (0, 0),
            input_at: None,
            ime_area: None,
            #[cfg(feature = "hot")]
            bar_shown: false,
            #[cfg(feature = "a11y")]
            a11y: None,
        }
    }

    /// This window's key.
    pub fn key(&self) -> u32 {
        self.key
    }

    /// This window's session — strategy-free: a `Local<A, P, Everything>`
    /// and a `Local<A, P>` are both a `Box<dyn Session>` here.
    pub fn session(&mut self) -> &mut dyn Session {
        &mut *self.session
    }

    /// Install this window's keyed waker on it and on its session.
    pub(crate) fn set_waker(&mut self, waker: Arc<dyn lumen_static::inbox::Waker>) {
        self.session.set_waker(waker.clone());
        self.waker = Some(waker);
    }

    /// Ask for a frame of this window's own — M53/T53.4. The frame it
    /// gets presents only if it changed something (`present_needed`).
    pub(crate) fn ask_redraw(&mut self) {
        if let Some(w) = &self.window {
            self.own_redraw = true;
            w.request_redraw();
        }
    }
}
