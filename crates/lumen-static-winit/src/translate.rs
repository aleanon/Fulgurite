//! winit's input vocabulary to this framework's — `docs/plan-desktop.md`
//! T14.0.
//!
//! Split out from the event loop because translation is the half worth
//! testing: it is pure, it is where a key silently becomes nothing, and
//! it is what T14.1–T14.3 extend. The loop around it is untestable
//! without a display; this is not.

use std::time::{Duration, Instant};

use lumen_static::{Event, Key, Mods, PointerButton, TouchPhase};
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key as WKey, NamedKey};

/// How far one wheel *line* scrolls, in logical px.
///
/// winit reports lines on X11 and pixels on a trackpad, and a line has
/// no inherent size. 20 px is what the tally has always used.
pub const LINE_SCROLL: f32 = 20.0;

/// How long after a press a second one still counts as a double click.
///
/// 500 ms is the platform-conventional value. It is a constant so an
/// application can say what it means rather than discovering it.
pub const MULTI_CLICK_TIME: Duration = Duration::from_millis(500);

/// How far a second press may land and still count as a double click,
/// in **logical** px — so a scale factor does not change the feel
/// (M10b).
pub const MULTI_CLICK_SLOP: f32 = 5.0;

/// Counts single, double and triple clicks — `docs/plan-desktop.md`
/// T14.3.
///
/// Lives here rather than in `dispatch` because it needs a clock, and
/// the core is `no_std`. The count is then carried *on the event*, so a
/// recorded session replays as the same double click it was recorded as.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClickCounter {
    last: Option<(Instant, f32, f32, PointerButton)>,
    count: u8,
}

impl ClickCounter {
    /// Register a press and return whether it is the 1st, 2nd or 3rd.
    ///
    /// Wraps back to 1 after a triple: a fourth click starts a new
    /// sequence, which is what every toolkit does and what a
    /// select-word-then-select-line gesture expects.
    pub fn press(&mut self, x: f32, y: f32, button: PointerButton) -> u8 {
        let now = Instant::now();
        let continues = self.last.is_some_and(|(t, lx, ly, b)| {
            b == button
                && now.duration_since(t) <= MULTI_CLICK_TIME
                && (x - lx).abs() <= MULTI_CLICK_SLOP
                && (y - ly).abs() <= MULTI_CLICK_SLOP
        });
        self.count = if continues && self.count < 3 { self.count + 1 } else { 1 };
        self.last = Some((now, x, y, button));
        self.count
    }
}

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
    clicks: &mut ClickCounter,
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

        WindowEvent::MouseInput { state, button, .. } => {
            // Back and forward produce nothing rather than a wrong
            // button — T14.3 excludes them deliberately.
            let Some(button) = pointer_button(*button) else { return Translated::NONE };
            let (x, y) = (cursor.x, cursor.y);
            Translated::one(match state {
                ElementState::Pressed => {
                    Event::PointerDown { x, y, button, clicks: clicks.press(x, y, button) }
                }
                ElementState::Released => Event::PointerUp { x, y, button },
            })
        }

        WindowEvent::Touch(t) => {
            let (x, y) = to_logical(t.location.x as f32, t.location.y as f32);
            // winit's id is a u64 per-session sequence; `Event` is 16
            // bytes and a u64 id would make it 24, taking the queue from
            // 1 048 to 1 560 (T15.5, measured before the design). It
            // never approaches 2^32, so the low half is the id.
            Translated::one(Event::Touch { id: t.id as u32, x, y, phase: touch_phase(t.phase) })
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

/// A file the platform dropped on the window — T15.7's other half.
///
/// Behind the default-off `dnd` feature: platform drag and drop is a
/// stack, and an application that does not accept drops should not link
/// one. The *within-application* half is `lumen_static::Drag` and needs
/// no feature and no platform at all.
#[cfg(feature = "dnd")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dropped {
    /// The pointer is over the window carrying files.
    Hovering,
    /// Files were released on it.
    Files(alloc_vec::Vec<std::path::PathBuf>),
    /// The pointer left without dropping.
    Cancelled,
}

#[cfg(feature = "dnd")]
mod alloc_vec {
    pub use std::vec::Vec;
}

/// Translate a platform drag-and-drop event, if this is one.
#[cfg(feature = "dnd")]
pub fn dropped(event: &WindowEvent) -> Option<Dropped> {
    match event {
        WindowEvent::HoveredFile(_) => Some(Dropped::Hovering),
        WindowEvent::HoveredFileCancelled => Some(Dropped::Cancelled),
        WindowEvent::DroppedFile(p) => Some(Dropped::Files(alloc_vec::Vec::from([p.clone()]))),
        _ => None,
    }
}

/// winit's touch phases to the framework's.
///
/// `Cancelled` maps to `Cancel` and not to `End`: the system took the
/// touch away, and a widget that treats that as a release commits an
/// action the user did not finish.
pub fn touch_phase(p: winit::event::TouchPhase) -> TouchPhase {
    use winit::event::TouchPhase as P;
    match p {
        P::Started => TouchPhase::Begin,
        P::Moved => TouchPhase::Move,
        P::Ended => TouchPhase::End,
        P::Cancelled => TouchPhase::Cancel,
    }
}

/// winit's buttons to the framework's three.
///
/// `None` for the back and forward buttons, which are excluded for the
/// same reason as media keys — an application that wants them wants a
/// different vocabulary than "the buttons every mouse has" (T14.3).
pub fn pointer_button(b: winit::event::MouseButton) -> Option<PointerButton> {
    use winit::event::MouseButton as B;
    Some(match b {
        B::Left => PointerButton::Left,
        B::Right => PointerButton::Right,
        B::Middle => PointerButton::Middle,
        _ => return None,
    })
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
