//! What a keystroke becomes before any widget sees it: a clipboard
//! shortcut, or text for the focused field — `docs/plan-text-correctness.md`
//! T45.0 items 10 and 11.
//!
//! Both decisions used to live inline in the window's event handler,
//! where a test could not reach them without a display. They are
//! functions here so `tests/key_text.rs` and `tests/clipboard_shortcut.rs`
//! drive the same code the window runs.

use lumen_static::{Key, Mods};
use lumen_static_shell::session::Session;
use winit::keyboard::Key as WKey;

use crate::translate;

/// The platform whose shortcut modifier applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    /// Cmd is the shortcut modifier.
    MacOs,
    /// Ctrl is.
    Other,
}

impl Os {
    /// The platform this build targets.
    pub const CURRENT: Os = if cfg!(target_os = "macos") { Os::MacOs } else { Os::Other };
}

/// What a clipboard shortcut does, when it is one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clip {
    /// Put the selection on the clipboard.
    Copy,
    /// Copy, then delete the selection.
    Cut,
    /// Type the clipboard into the focused node.
    Paste,
    /// Consume the key and do nothing.
    Swallow,
}

/// Everything the shortcut decision depends on.
#[derive(Clone, Copy, Debug)]
pub struct Shortcut {
    pub mods: Mods,
    /// The key as [`translate::key_of`] reports it.
    pub key: Option<Key>,
    /// Whether any node has focus.
    pub focused: bool,
    /// Whether the focused node has a selection to copy.
    pub selection: bool,
    /// Whether the clipboard holds text.
    pub clipboard: bool,
    pub os: Os,
}

/// Whether a keystroke is a clipboard shortcut, and what it does — T14.5.
///
/// `None` routes the key on to the application, as any other key.
pub fn clipboard_decision(s: Shortcut) -> Option<Clip> {
    if !(s.mods.has(Mods::CTRL) || s.mods.has(Mods::META)) {
        return None;
    }
    match s.key? {
        Key::Char('c') if s.selection => Some(Clip::Copy),
        Key::Char('x') if s.selection => Some(Clip::Cut),
        Key::Char('c') | Key::Char('x') => Some(Clip::Swallow),
        Key::Char('v') if s.clipboard => Some(Clip::Paste),
        Key::Char('v') => Some(Clip::Swallow),
        _ => None,
    }
}

/// Deliver one keyboard event to `session`, as the window does.
///
/// **Key text arrives whole** (M45/T45.8, R10-F8). A single character is
/// an `Event::Text`, queued, as it always was. Text of several characters
/// — a dead key followed by a consonant it cannot combine with (`"^a"` on
/// a US-International layout), a decomposed accent, a flag, an Indic
/// conjunct a layout commits per key — is committed to the focused node
/// in one step through [`Session::compose`], which the widgets accept and
/// `Host::compose` marks. `translate` stays one event out; the routing is
/// here, where a test can call it.
///
/// Returns whether anything reached the session, which is whether the
/// window asks for a redraw.
pub fn deliver_key(session: &mut dyn Session, logical: &WKey, text: Option<&str>, down: bool, held: Mods) -> bool {
    let t = translate::keyboard(logical, text, down, held);
    let mut any = false;
    for ev in t.iter() {
        session.post(ev);
        any = true;
    }
    if let translate::KeyText::Whole(s) = translate::key_text(text, down, held) {
        any |= session.compose(lumen_static::event::Composition::Commit(s));
    }
    any
}
