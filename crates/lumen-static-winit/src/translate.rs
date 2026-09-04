//! winit's input vocabulary to this framework's — `docs/plan-desktop.md`
//! T14.0.
//!
//! Split out from the event loop because translation is the half worth
//! testing: it is pure, it is where a key silently becomes nothing, and
//! it is what T14.1–T14.3 extend. The loop around it is untestable
//! without a display; this is not.

use lumen_static::{Event, Key};
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
pub fn window_event(
    event: &WindowEvent,
    cursor: &mut Cursor,
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

        WindowEvent::KeyboardInput { event, .. } => {
            let down = event.state == ElementState::Pressed;
            let key = named_key(&event.logical_key).map(|key| Event::Key { key, down });
            // Text only on the way down, and never for a control
            // character: winit reports "\r" as the text of Enter, and
            // typing it into a field would insert a carriage return
            // beside the Key event that already means "activate".
            let text = event
                .text
                .as_ref()
                .filter(|_| down)
                .and_then(|t| t.chars().find(|c| !c.is_control()))
                .map(|ch| Event::Text { ch });
            Translated { events: [key, text], is_input: true }
        }

        _ => Translated::NONE,
    }
}

/// winit's named keys to the framework's, for the keys [`Key`] names.
///
/// `None` for a key this framework has no name for. That is a real
/// dropped keystroke, which is why T14.2 exists: the set is small on
/// purpose, but it should be small *deliberately*.
pub fn named_key(k: &WKey) -> Option<Key> {
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
        _ => return None,
    })
}
