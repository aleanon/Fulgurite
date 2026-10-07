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
/// `None` routes the key on to the application, as any other key — and
/// since M45/T45.9 (R10-F13) that is the answer whenever the shortcut has
/// nothing to act on: nothing focused, nothing selected for `c`/`x`,
/// nothing on the clipboard for `v`. It used to consume the key in every
/// one of those cases, so an application never saw Ctrl+C or Ctrl+V at
/// all, not even with nothing focused.
///
/// **The modifier is the platform's**: Cmd on macOS, Ctrl everywhere
/// else. It accepted either on every OS, so Super+C copied on Linux —
/// where Super belongs to the desktop — and Ctrl+C did on macOS, where it
/// is a terminal's interrupt.
///
/// [`Clip::Swallow`] is no longer an answer this gives; it stays for a
/// caller that wants to consume a key without acting.
pub fn clipboard_decision(s: Shortcut) -> Option<Clip> {
    let shortcut = match s.os {
        Os::MacOs => Mods::META,
        Os::Other => Mods::CTRL,
    };
    if !s.mods.has(shortcut) || !s.focused {
        return None;
    }
    match s.key? {
        Key::Char('c') if s.selection => Some(Clip::Copy),
        Key::Char('x') if s.selection => Some(Clip::Cut),
        Key::Char('v') if s.clipboard => Some(Clip::Paste),
        _ => None,
    }
}

/// The platform clipboard's text, as [`clipboard_shortcut`] uses it: a
/// trait so a test can hand it a clipboard that is not the desktop's.
pub trait ClipText {
    /// The clipboard's text, if it has any.
    fn read(&mut self) -> Option<String>;
    /// Put `text` on it.
    fn write(&mut self, text: &str);
}

impl ClipText for crate::Clipboard {
    fn read(&mut self) -> Option<String> {
        self.get()
    }
    fn write(&mut self, text: &str) {
        self.set(text);
    }
}

/// **Ctrl+C, Ctrl+X and Ctrl+V (Cmd on macOS) in `session`** — T14.5, a
/// free function since M90/T90.5 so a test drives what the window runs
/// (as [`deliver_key`] is, M45/T45.0 item 10). `key` is the press as
/// [`translate::key_of`] reports it, `mods` the modifiers held. Returns
/// whether the keystroke was **consumed** as a clipboard shortcut; `false`
/// routes it on through [`deliver_key`] like any key, to a widget, the
/// program's accelerators (`Program::menu`) and its key hook.
///
/// [`clipboard_decision`] decides whether it is a shortcut at all. A copy
/// or a cut consumes the key whenever the focused node answers a
/// selection. **A paste consumes it only when the focused node took the
/// paste** ([`Session::paste`] answered `Some`; `docs/plan-desktop-integration.md`,
/// X3): before, Ctrl+V with a `Button` or a menu bar focused and text on
/// the clipboard was swallowed, so an Edit menu's Paste accelerator never
/// fired, and no agent input passes through here to have seen it.
pub fn clipboard_shortcut(session: &mut dyn Session, key: Option<Key>, mods: Mods, os: Os, clip: &mut dyn ClipText) -> bool {
    // Only what the decision needs is read: the selection for c/x, the
    // clipboard for v.
    let copied = match key {
        Some(Key::Char('c' | 'x')) => session.copy(),
        _ => None,
    };
    let pasted = match key {
        Some(Key::Char('v')) => clip.read().filter(|t| !t.is_empty()),
        _ => None,
    };
    let decision = clipboard_decision(Shortcut {
        mods,
        key,
        focused: session.has_focus(),
        selection: copied.is_some(),
        clipboard: pasted.is_some(),
        os,
    });
    match decision {
        None => false,
        Some(Clip::Swallow) => true,
        Some(Clip::Copy) | Some(Clip::Cut) => {
            if let Some(text) = copied {
                clip.write(&text);
            }
            if decision == Some(Clip::Cut) {
                // Cut is copy plus the delete the field already does: one
                // edit path, so the caret and the shaped cache cannot
                // disagree with a bespoke one.
                session.post(lumen_static::Event::Key { key: Key::Backspace, down: true });
            }
            true
        }
        // The focused node took it, or the key goes on (X3).
        Some(Clip::Paste) => pasted.is_some_and(|text| session.paste(&text).is_some()),
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
