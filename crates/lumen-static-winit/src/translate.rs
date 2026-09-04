//! winit's input vocabulary to this framework's — `docs/plan-desktop.md`
//! T14.0.
//!
//! Split out from the event loop because translation is the half worth
//! testing: it is pure, it is where a key silently becomes nothing, and
//! it is what T14.1–T14.3 extend. The loop around it is untestable
//! without a display; this is not.

use lumen_static::{Event, Key, Mods};
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key as WKey, NamedKey};

/// How far one wheel *line* scrolls, in logical px.
///
/// winit reports lines on X11 and pixels on a trackpad, and a line has
/// no inherent size. 20 px is what the tally has always used.
pub const LINE_SCROLL: f32 = 20.0;

/// Pointer position, kept between events.
///
/// winit reports a move and a click separately, and the click carries no
/// coordinates — so a host that does not remember the last move cannot
/// say where a click landed.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cursor {
    pub x: f32,
    pub y: f32,
}

/// What a translated event needs from its host, so translation stays a
/// pure function of the event plus this.
pub struct Translated {
    /// The events to post, in order. A keystroke can produce two — a
    /// `Key` and the `Text` it types.
    pub events: [Option<Event>; 2],
    /// Whether this was user input, as opposed to a resize or an
    /// expose. The host stamps input arrival from this, for the
    /// click-to-presented latency in `docs/results-frame-cost.md`.
    pub is_input: bool,
}

impl Translated {
    const NONE: Translated = Translated { events: [None, None], is_input: false };

    fn one(e: Event) -> Self {
        Translated { events: [Some(e), None], is_input: true }
    }

    /// Everything this produced, in order.
    pub fn iter(&self) -> impl Iterator<Item = Event> + '_ {
        self.events.iter().flatten().copied()
    }
}

/// Translate one `WindowEvent`.
///
/// `to_logical` converts winit's physical coordinates to the logical
/// ones everything above the renderer speaks (M10b) — it is
/// `Host::to_logical`, taken as a closure so this function does not need
/// the host's type parameter.
///
/// `held` is the modifier state as of the last `ModifiersChanged`. It is
/// needed here, not just by the host, because it decides whether a
/// keystroke types (T14.2).
pub fn window_event(
    event: &WindowEvent,
    cursor: &mut Cursor,
    held: Mods,
    to_logical: impl Fn(f32, f32) -> (f32, f32),
) -> Translated {
    match event {
        WindowEvent::CursorMoved { position, .. } => {
            let (x, y) = to_logical(position.x as f32, position.y as f32);
            *cursor = Cursor { x, y };
            // A move is not "input" for latency purposes: it fires
            // continuously and would reset the stamp every frame,
            // reporting a latency of nearly zero for every click.
            Translated {
                events: [Some(Event::PointerMove { x, y }), None],
                is_input: false,
            }
        }

        WindowEvent::MouseInput { state, button: winit::event::MouseButton::Left, .. } => {
            let (x, y) = (cursor.x, cursor.y);
            Translated::one(match state {
                ElementState::Pressed => Event::PointerDown { x, y },
                ElementState::Released => Event::PointerUp { x, y },
            })
        }

        WindowEvent::MouseWheel { delta, .. } => {
            let dy = match delta {
                MouseScrollDelta::LineDelta(_, y) => y * LINE_SCROLL,
                MouseScrollDelta::PixelDelta(p) => p.y as f32,
            };
            Translated::one(Event::Scroll { x: cursor.x, y: cursor.y, dy })
        }

        WindowEvent::ModifiersChanged(m) => {
            // Into the queue, not beside it: modifier state held outside
            // the queue makes a recorded session unreplayable exactly
            // when modifiers matter (T14.1).
            Translated {
                events: [Some(Event::Modifiers(mods(m.state()))), None],
                is_input: false,
            }
        }

        WindowEvent::KeyboardInput { event, .. } => keyboard(
            &event.logical_key,
            event.text.as_deref(),
            event.state == ElementState::Pressed,
            held,
        ),

        _ => Translated::NONE,
    }
}

/// The keyboard half of [`window_event`], as a function of what a key
/// event *says* rather than of winit's `KeyEvent`.
///
/// Split out because `KeyEvent::platform_specific` has no public
/// constructor, so a test cannot build one — and this is the branch most
/// worth testing, since it is where the shortcut/typing rule lives.
pub fn keyboard(logical: &WKey, text: Option<&str>, down: bool, held: Mods) -> Translated {
    let key = key_of(logical).map(|key| Event::Key { key, down });
            // Text only on the way down, never for a control character,
            // and **never while a shortcut modifier is held**.
            //
            // The control-character rule: winit reports "\r" as the text
            // of Enter, and typing it into a field would insert a
            // carriage return beside the `Key` that already means
            // "activate".
            //
            // The modifier rule (T14.2): without it Ctrl+C both fires
            // the shortcut and types a "c". Shift is excluded from
            // "shortcut" because Shift+A *is* how a capital arrives —
            // which is why `Mods::is_shortcut` masks it off rather than
            // testing for an empty set.
    let text = text
        .filter(|_| down && !held.is_shortcut())
        .and_then(|t| t.chars().find(|c| !c.is_control()))
        .map(|ch| Event::Text { ch });
    Translated { events: [key, text], is_input: true }
}

/// winit's modifier state to [`Mods`].
///
/// `META` is winit's `SUPER`, which is Command on macOS and the Windows
/// or Super key elsewhere — one name for the platform's own modifier, so
/// an application writes one shortcut table (T14.1).
pub fn mods(m: winit::keyboard::ModifiersState) -> Mods {
    let mut out = Mods::NONE;
    if m.shift_key() {
        out = out.with(Mods::SHIFT);
    }
    if m.control_key() {
        out = out.with(Mods::CTRL);
    }
    if m.alt_key() {
        out = out.with(Mods::ALT);
    }
    if m.super_key() {
        out = out.with(Mods::META);
    }
    out
}

/// winit's named keys to the framework's, for the keys [`Key`] names.
///
/// `None` for a key this framework has no name for. That is a real
/// dropped keystroke, which is why T14.2 exists: the set is small on
/// purpose, but it should be small *deliberately*.
pub fn key_of(k: &WKey) -> Option<Key> {
    // A printable key becomes `Key::Char`, lower-cased so a shortcut
    // table needs one entry rather than two — Shift is reported in the
    // modifiers, not folded into the letter (T14.2).
    if let WKey::Character(s) = k {
        let mut it = s.chars();
        return match (it.next(), it.next()) {
            (Some(c), None) => Some(Key::Char(c.to_ascii_lowercase())),
            // A multi-character key label is a dead-key or IME artefact,
            // not a shortcut. T14.8 is where those get an answer.
            _ => None,
        };
    }
    let WKey::Named(n) = k else { return None };
    Some(match n {
        NamedKey::Enter => Key::Enter,
        NamedKey::Space => Key::Space,
        NamedKey::Escape => Key::Escape,
        NamedKey::Tab => Key::Tab,
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Delete => Key::Delete,
        NamedKey::ArrowLeft => Key::Left,
        NamedKey::ArrowRight => Key::Right,
        NamedKey::ArrowUp => Key::Up,
        NamedKey::ArrowDown => Key::Down,
        NamedKey::Home => Key::Home,
        NamedKey::End => Key::End,
        NamedKey::PageUp => Key::PageUp,
        NamedKey::PageDown => Key::PageDown,
        NamedKey::Insert => Key::Insert,
        NamedKey::F1 => Key::F(1),
        NamedKey::F2 => Key::F(2),
        NamedKey::F3 => Key::F(3),
        NamedKey::F4 => Key::F(4),
        NamedKey::F5 => Key::F(5),
        NamedKey::F6 => Key::F(6),
        NamedKey::F7 => Key::F(7),
        NamedKey::F8 => Key::F(8),
        NamedKey::F9 => Key::F(9),
        NamedKey::F10 => Key::F(10),
        NamedKey::F11 => Key::F(11),
        NamedKey::F12 => Key::F(12),
        _ => return None,
    })
}
