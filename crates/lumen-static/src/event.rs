//! Input events and the one queue — plan E0 (`docs/plan-events.md`).
//!
//! **One queue.** OS input, agent-synthesized input, and test input all
//! enter [`InputQueue`], so anything an agent does replays as a test.
//! The queue is host-owned and exists before any shell does: a winit (or
//! headless, or agent) front end *feeds* it, it does not own it.
//!
//! Events are plain data and serialize under `observe` — recording a
//! session is a dev concern, but the vocabulary itself is not, so the
//! types are always present.


use crate::desc::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "observe", derive(serde::Serialize, serde::Deserialize))]
///
/// **Deliberately not exhaustive.** Media and browser keys, numpad
/// distinctions, and the modifier keys themselves are excluded
/// (`docs/plan-desktop.md` T14.2). Modifiers are *state* — [`Mods`] —
/// not keys: a `Key::Shift` would let a widget see a shortcut
/// half-pressed.
pub enum Key {
    Enter,
    /// Activates the focused control. Added M11/T11.1: every desktop
    /// toolkit toggles a checkbox with Space, and until then `dispatch`
    /// carried a comment claiming it already did.
    Space,
    Escape,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    /// Added M14/T14.2, with the rest of this block.
    PageUp,
    PageDown,
    /// Completes the editing set beside Delete and Backspace.
    Insert,
    /// A printable key *as a shortcut*: Ctrl+C is
    /// `Key::Char('c')` with [`Mods::CTRL`], not [`Event::Text`].
    ///
    /// Typing still arrives as `Event::Text`; the windowing layer
    /// suppresses that when [`Mods::is_shortcut`] holds, or Ctrl+C would
    /// both fire the shortcut and type a "c" — a bug invisible until a
    /// text field has focus.
    ///
    /// Lower-cased by the windowing layer, so a shortcut table needs one
    /// entry rather than two: Shift is reported in the modifiers.
    Char(char),
    /// F1 to F12, as `F(1)` to `F(12)`. One variant rather than twelve:
    /// F1 help, F2 rename, F5 refresh and F11 fullscreen are the common
    /// ones, and a table is a worse fit than a number.
    F(u8),
}

/// Which modifier keys are held — `docs/plan-desktop.md` T14.1.
///
/// A bitflag rather than four `bool`s so it fits in one byte and
/// `Event` stays 16 B, which is what keeps [`InputQueue`] at 1 KB.
///
/// **`META`, not `CMD` or `SUPER`.** One name for the platform's own
/// modifier — Command on macOS, Super elsewhere — mapped per platform
/// by the windowing layer, so an application writes one shortcut table.
/// Whether `CTRL` or `META` is "the" shortcut modifier is the
/// *application's* choice; this reports both truthfully and decides
/// nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "observe", derive(serde::Serialize, serde::Deserialize))]
pub struct Mods(pub u8);

impl Mods {
    pub const NONE: Mods = Mods(0);
    pub const SHIFT: Mods = Mods(1 << 0);
    pub const CTRL: Mods = Mods(1 << 1);
    pub const ALT: Mods = Mods(1 << 2);
    pub const META: Mods = Mods(1 << 3);

    /// Whether every bit of `m` is held. `has(Mods::NONE)` is always
    /// true, which is the right answer to "does this need no modifiers".
    pub const fn has(self, m: Mods) -> bool {
        self.0 & m.0 == m.0
    }

    /// Whether *no* modifier is held. A shortcut table wants this to
    /// tell a bare Enter from Ctrl+Enter.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether any modifier but Shift is held.
    ///
    /// The rule the windowing layer applies to suppress [`Event::Text`]:
    /// Shift is excluded because Shift+A *is* how a capital arrives,
    /// while Ctrl+C must not also type a "c" (T14.2).
    pub const fn is_shortcut(self) -> bool {
        self.0 & !Mods::SHIFT.0 != 0
    }

    /// Add `m`.
    pub const fn with(self, m: Mods) -> Mods {
        Mods(self.0 | m.0)
    }
}

/// One step of an input-method composition — `docs/plan-desktop.md`
/// T14.8.
///
/// **Not an `Event`.** A composition step carries a string, and `Event`
/// is `Copy` and 16 bytes; a borrowed payload would put a lifetime on
/// the whole enum and a borrow in the queue. It reaches the focused node
/// through `Host::compose` instead, the same shape `Host::paste` uses
/// for the same reason.
///
/// The sequence mirrors the platform's: `Preedit` repeatedly as the user
/// composes, then `Commit` with the final text — or `Cancel` when the
/// composition is abandoned. Provisional text is *in* the buffer, so it
/// measures, positions and scrolls like real text; what makes it
/// provisional is that the next `Preedit` replaces it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Composition<'a> {
    /// Replace the provisional text with this. Empty clears it.
    Preedit(&'a str),
    /// The composition finished: drop the provisional text, insert this.
    Commit(&'a str),
    /// Abandon it, leaving the buffer as it was before composing.
    Cancel,
}

/// Where a touch is in its life — T15.5.
///
/// `Cancel` is not `End`: the system took the touch away (a gesture was
/// claimed by the OS, a call arrived), and a widget that treats it as a
/// release commits an action the user did not finish. Distinguishing
/// them is the whole reason this is four variants and not a `bool`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "observe", derive(serde::Serialize, serde::Deserialize))]
pub enum TouchPhase {
    Begin,
    Move,
    End,
    Cancel,
}

/// Which pointer button — `docs/plan-desktop.md` T14.3.
///
/// Three, not five: the back and forward buttons are excluded for the
/// same reason as media keys, and an application that wants them wants
/// a different vocabulary than "the buttons every mouse has".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "observe", derive(serde::Serialize, serde::Deserialize))]
pub enum PointerButton {
    #[default]
    Left,
    Right,
    Middle,
}

/// How long after the last wheel event a gesture counts as ended —
/// [`Event::ScrollEnd`]. Long enough to span the gaps between a touchpad's
/// events, short enough that a spring-back feels like a release.
pub const SCROLL_END_MS: f32 = 150.0;

/// How long the pointer rests on a node before [`Event::Linger`] — the
/// half second toolkits wait before a tooltip.
pub const LINGER_MS: f32 = 500.0;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "observe", derive(serde::Serialize, serde::Deserialize))]
pub enum Event {
    /// `clicks` is 1, 2 or 3 — a single, double or triple click.
    ///
    /// Counted by the **host**, not by `dispatch`: it needs a clock, and
    /// the core is `no_std`. Carried on the event rather than tracked
    /// beside it so that a recorded session replays as the same
    /// double-click it was recorded as (E0).
    PointerDown { x: f32, y: f32, button: PointerButton, clicks: u8 },
    PointerUp { x: f32, y: f32, button: PointerButton },
    PointerMove { x: f32, y: f32 },
    /// A pointer gesture ended **without a release** — M44/T44.4
    /// (`docs/plan-input-semantics.md` §2.4).
    ///
    /// What a platform sends when a touch is taken away (a palm, a
    /// system gesture, a window losing the pointer). Delivered to the
    /// node holding the capture, like a release, so a widget that keeps
    /// drag state of its own learns the gesture ended; it releases the
    /// capture, clears the press, and **activates nothing** — which is
    /// the whole difference from `PointerUp`. The host synthesises it
    /// from the primary touch's `Cancel`.
    PointerCancel { x: f32, y: f32 },
    /// The pointer is **over** this node, and nothing is captured.
    ///
    /// **A separate event, not a flag on the move** (M27/T27.11,
    /// finding 3). A move was delivered only under pointer capture,
    /// which is granted only to a node that consumed a `PointerDown` —
    /// so a widget that paints *rows* could never learn which row the
    /// pointer was over. `Tree` and `OptionList` both shipped with the
    /// code for a hot row and neither could ever receive the event that
    /// sets it; screenshots before and after a hover were byte-identical.
    ///
    /// Routing the move itself was tried first and is wrong: `Slider`
    /// reads a move as a drag, correctly, and would have followed the
    /// pointer without anyone pressing it —
    /// `tests/slider.rs::a_move_with_nothing_captured_is_not_routed`
    /// pins that and went red. So the two questions get two events: "you
    /// are being dragged" is a `PointerMove`, "the pointer is over you"
    /// is this, and a widget that does not match it is unaffected —
    /// which is the safe default for the hundred widgets that will never
    /// care.
    ///
    /// Synthesized by `dispatch`, never enqueued by a host: it *is* the
    /// move, asked differently.
    Hover { x: f32, y: f32 },
    Scroll { x: f32, y: f32, dy: f32 },
    Key { key: Key, down: bool },
    Text { ch: char },
    /// Focus arrived at (`gained`) or left this node — M57/A7
    /// (`docs/plan-widget-fixes.md`).
    ///
    /// Synthesized by `dispatch` once an event has **finished** moving
    /// focus, and offered to the old node and then the new one, bubbling
    /// like any event (so `ctx.target` names the node focus moved to or
    /// from, and a container can tell its own part apart).
    ///
    /// **A transition, not a state.** A chooser opened its list from
    /// `layout` whenever its field was focused, so a choice that closed
    /// the list reopened it in the same pass (`docs/widget-audit.md` X5),
    /// and a tooltip had no way to open on keyboard focus at all. An
    /// event fires once, when focus moves, which is what "opens on focus"
    /// means. Never enqueued by a host.
    Focus { gained: bool },
    /// The pointer left this node — M57/B50 (`docs/plan-widget-fixes.md`).
    ///
    /// Synthesized by `dispatch` when a move changes which node the pointer
    /// is over, and sent to the node it left. A widget that tracks a hovered
    /// *row* from [`Event::Hover`] learned when the row changed, never when
    /// the pointer left altogether: a tree kept its highlight after the
    /// pointer moved away (`docs/widget-audit.md`, Tree). Never enqueued by
    /// a host.
    Leave,
    /// A wheel gesture on this node ended: no wheel event has arrived for
    /// a moment — M57/B40 (`docs/plan-widget-fixes.md`).
    ///
    /// The browser's scroll-end event. A wheel has no release, so a widget that
    /// acts when a scroll *finishes* — a pull-to-refresh springing back and
    /// firing — had nothing to act on, and stayed armed with its content
    /// held down until an unrelated click (`docs/widget-audit.md`,
    /// PullToRefresh). The host sends it to the node that took the last
    /// wheel, bubbling, once none has come for [`SCROLL_END_MS`]. Never
    /// enqueued by a host's input path.
    ScrollEnd,
    /// The pointer has rested on this node for [`LINGER_MS`] — hover
    /// intent — M57/B35 (`docs/plan-widget-fixes.md`).
    ///
    /// Sent by the host to the node under a pointer that has stopped,
    /// bubbling. A tooltip opened on the very first hover frame and closed
    /// on the next, which is how a pointer passing over a toolbar strobes
    /// every tip on the way (`docs/widget-audit.md`, Tooltip); toolkits
    /// wait for the pointer to rest. Widgets have no clock, so the host
    /// keeps this one, as it keeps [`Event::ScrollEnd`]'s. Never enqueued
    /// by a host's input path.
    Linger,
    /// One finger, at one moment — `docs/plan-capabilities.md` T15.5.
    ///
    /// **`id` is a `u32`, and that was measured rather than chosen.**
    /// `Event` is 16 bytes and asserted; a `u64` id makes this variant
    /// 20 and the enum 24, which would take [`InputQueue`] from 1 048
    /// bytes to 1 560. The windowing layer truncates the platform's id,
    /// which is a per-session sequence and never approaches 2³².
    ///
    /// **The primary touch also drives the pointer** (M44/T44.4). A host
    /// dispatches this event as it is, so a recogniser still sees every
    /// finger, and then — for the first finger down, until it lifts —
    /// synthesises the pointer events a mouse would have sent: `Begin` is
    /// a move and a left press, `Move` a move, `End` a release, and
    /// `Cancel` a [`Event::PointerCancel`]. Other fingers never drive the
    /// pointer. That is what makes a tap on a touchscreen press a button:
    /// no stock widget handles `Touch` itself.
    ///
    /// Gesture *recognition* — pinch, rotate, swipe, long-press — is
    /// not here. Those are stateful, and a framework pass that
    /// recognised them would cost every tree that wants none. They are
    /// types an application owns: see [`crate::gesture`].
    Touch { id: u32, x: f32, y: f32, phase: TouchPhase },
    /// The modifier keys changed — T14.1.
    ///
    /// **In the queue, not beside it.** `docs/plan-events.md` E0 is
    /// "one queue, so anything an agent does replays as a test", and
    /// modifier state held outside the queue makes a recorded session
    /// unreplayable exactly when modifiers matter. `dispatch` tracks
    /// the current value and puts it in every [`EventCtx`], so a
    /// handler reads `ctx.mods` and never sees this event.
    Modifiers(Mods),
    /// **An assistive technology acted on a node** — M54/T54.4
    /// (`docs/plan-a11y-platform.md` §2.4, R15-F2).
    ///
    /// Delivered like a key, but to `node` rather than to the focused
    /// node: offered to `node` and then to each ancestor through
    /// `on_event` until one consumes it, and — for
    /// [`Action::Default`] — then to the application's activation
    /// handler, exactly as Enter would be. So a screen reader's "press"
    /// on a label inside a button reaches the button, and runs the same
    /// handler a click does, without synthesising pointer coordinates.
    /// [`Action::Focus`] is dispatch's own: it moves focus to a focusable
    /// node, as Tab does.
    ///
    /// In the queue, so a session an assistive technology drove replays
    /// as a test (E0). `node` is a main-table index; `Event` stays 16 B
    /// (`tests/modifiers.rs`).
    Action { node: u32, action: Action },
}

/// What an assistive technology asked a node to do — M54/T54.4.
///
/// The framework's own vocabulary, `no_std` and free of `accesskit`: the
/// window crate maps the platform's actions onto it. **`f32`, not
/// accesskit's `f64`**: `Event` is 16 B and asserted, and the
/// interactive firmware links no soft-`f64` code (M52/T52.2).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "observe", derive(serde::Serialize, serde::Deserialize))]
pub enum Action {
    /// Activate: what a click or Enter does.
    Default,
    /// One step up, as an arrow key would.
    Increment,
    /// One step down.
    Decrement,
    /// Set the value outright, in the widget's own units.
    SetValue(f32),
    /// Open a disclosure, a list, a popup.
    Expand,
    /// Close it.
    Collapse,
    /// Take keyboard focus.
    Focus,
}

impl Event {
    /// A plain left-button press: one click, no modifiers implied.
    ///
    /// The common case, and a constructor rather than a literal because
    /// T14.3 added two fields that almost every call site does not care
    /// about — a test that means "click here" should not have to name a
    /// button and a click count to say it.
    pub fn down(x: f32, y: f32) -> Event {
        Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 }
    }

    /// The matching release.
    pub fn up(x: f32, y: f32) -> Event {
        Event::PointerUp { x, y, button: PointerButton::Left }
    }

    /// Which button, for the events that carry one; [`PointerButton::Left`]
    /// otherwise — a key event has no button and no caller should ask.
    pub fn button(&self) -> PointerButton {
        match *self {
            Event::PointerDown { button, .. } | Event::PointerUp { button, .. } => button,
            _ => PointerButton::Left,
        }
    }

    /// How many clicks this press completes, or 0 for anything but a
    /// `PointerDown`.
    pub fn clicks(&self) -> u8 {
        match *self {
            Event::PointerDown { clicks, .. } => clicks,
            _ => 0,
        }
    }

    /// Pointer position, for the events that carry one.
    pub fn position(&self) -> Option<Point> {
        match *self {
            Event::PointerDown { x, y, .. }
            | Event::PointerUp { x, y, .. }
            | Event::PointerMove { x, y }
            | Event::PointerCancel { x, y }
            | Event::Hover { x, y }
            | Event::Touch { x, y, .. }
            | Event::Scroll { x, y, .. } => Some(Point { x, y }),
            Event::Key { .. }
            | Event::Text { .. }
            | Event::Modifiers(_)
            | Event::Action { .. }
            | Event::Focus { .. }
            | Event::Leave
            | Event::ScrollEnd
            | Event::Linger => None,
        }
    }
}

/// What dispatch learned about where an event landed. Handlers read
/// their payload from here (and from their widget's own fields) — never
/// from a capture, which is what keeps them `fn` pointers across a hot
/// reload (`docs/events-decision.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EventCtx {
    /// What happened (C2).
    ///
    /// A handler is one `fn` per node and every event now reaches it —
    /// keys, text and scroll as well as clicks (C1) — so it has to be
    /// able to tell them apart. The ctx already carries data *derived*
    /// from the event (`pos`, `local`, `row`); carrying the event itself
    /// is one field and removes the guessing.
    pub event: Event,
    /// Main-table index of the node that received the event.
    pub node: u32,
    /// Main-table index of the node the event was **aimed at**: the node
    /// hit by a pointer, or the focused node for a key — M57/A4
    /// (`docs/plan-widget-fixes.md`).
    ///
    /// Equal to `node` for the node itself, and different once the event
    /// bubbles: a press that a label ignored reaches its container with
    /// `target` still naming the label. A container that acts on presses
    /// on *its own* chrome asks `ctx.target == ctx.node`, or tests `local`
    /// against its chrome's rect. Before this, `Veil`, `SplitPane`,
    /// `Collapsible` and `PaneGrid` each read any press that reached them
    /// as a press on themselves: a click on a modal's panel dismissed it,
    /// a click inside a pane moved the divider (`docs/widget-audit.md` X2).
    pub target: u32,
    /// Whether [`EventCtx::target`] is a **control** — a focusable node —
    /// M57/B40 (`docs/plan-widget-fixes.md`).
    ///
    /// For a container that may take a press its content declined: a
    /// pull-to-refresh can start a drag from a label or from its own
    /// surface, and must leave a button's press to the button, since
    /// taking it would capture the pointer and the click would never
    /// activate.
    pub target_is_control: bool,
    /// For virtualized content: which row. `None` for ordinary nodes.
    ///
    /// This is the field that replaces `move |s| s.remove(idx)`: the row
    /// is recomputed by hit testing every time, so it can never go stale
    /// the way a captured index does.
    ///
    /// **Fresh as an index, not as an identity.** When the list changes
    /// between a press and its release, the index under the pointer is a
    /// different item; that click is cancelled by the press latch
    /// ([`InputState::press`], M44/T44.2) before a handler could receive
    /// this row, so a handler never sees an index for an item nobody
    /// pressed.
    pub row: Option<u64>,
    /// Node index *within the template*, when `row` is set.
    pub inner: u32,
    /// Absolute pointer position, when the event carries one.
    pub pos: Point,
    /// Pointer position relative to the **receiving** node's origin —
    /// `node`'s, re-derived at every node the event bubbles through
    /// (M57/A4). It used to stay relative to the node first hit, so a
    /// container read its child's coordinates as its own.
    pub local: Point,
    /// Which button, for a pointer event; [`PointerButton::Left`] otherwise.
    pub button: PointerButton,
    /// 1, 2 or 3 for a `PointerDown`; 0 for anything else (T14.3).
    ///
    /// A widget that does not care reads the same `PointerDown` it
    /// always did — which is why this is a field rather than a
    /// `DoubleClick` event.
    pub clicks: u8,
    /// Which modifiers were held when this event happened (T14.1).
    ///
    /// Tracked by `dispatch` from [`Event::Modifiers`] rather than
    /// carried on each event, so `Event` stays 16 B.
    pub mods: Mods,
}

impl EventCtx {
    pub fn at(node: u32) -> Self {
        EventCtx::for_event(node, Event::PointerUp {
            x: 0.0,
            y: 0.0,
            button: PointerButton::Left,
        })
    }

    /// The ctx a non-pointer event produces at `node`.
    pub fn for_event(node: u32, event: Event) -> Self {
        EventCtx {
            event,
            node,
            target: node,
            target_is_control: false,
            row: None,
            inner: 0,
            pos: Point::default(),
            local: Point::default(),
            button: event.button(),
            clicks: event.clicks(),
            mods: Mods::NONE,
        }
    }

    /// The same, with the modifiers that were held.
    pub fn for_event_with(node: u32, event: Event, mods: Mods) -> Self {
        EventCtx { mods, ..EventCtx::for_event(node, event) }
    }
}

/// The caller-owned input state `dispatch` reads and updates.
///
/// Bundled as of M14/T14.1, when modifier tracking would have made
/// `dispatch` a seven-argument function. It is also the thing a session
/// recording snapshots: three loose `&mut` were three things to
/// remember, and this is one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InputState {
    /// The focused node's main-table index, if any.
    pub focus: Option<u32>,
    /// Whether the next `PointerUp` ends a press that dismissed an
    /// overlay, and must be consumed with it — M33/T33.3.
    ///
    /// Activation happens on the *release*, and nothing requires the
    /// release to end a press that began on the same node. So when
    /// `pass::dispatch_overlaid` consumed a press outside every open
    /// overlay, the release still activated whatever it landed on: the
    /// click that closed a menu also pressed the button under it
    /// (`crates/lumen-static/tests/dismiss_click.rs`).
    pub swallow_release: bool,
    /// The node holding the pointer for a drag, if any (T11.2).
    ///
    /// Checked *before* the hit test: a drag that leaves its control
    /// must keep reaching it, and the first version checked after, so
    /// the slider stopped following the pointer the moment it left the
    /// track.
    pub capture: Option<u32>,
    /// Modifiers currently held, updated from [`Event::Modifiers`].
    pub mods: Mods,
    /// Whether the current focus came from the **keyboard** — T15.6.
    ///
    /// A focus ring after a mouse click is noise; a focus ring after Tab
    /// is the difference between a usable application and an unusable
    /// one. Every toolkit draws the distinction (CSS calls it
    /// `:focus-visible`) and so does this.
    ///
    /// Here rather than in `NodeState` because it is **not per node** —
    /// there is one focus. As a `NodeState` field it would have taken
    /// that struct from 76 bytes to 80, times every node of every tree,
    /// to say something about exactly one of them.
    pub focus_visible: bool,
    /// The node the pointer is over, as `dispatch` last hit-tested it —
    /// M25/T25.0.
    ///
    /// The same fact as the `hovered` bits in the state array, kept
    /// here as well so a host can tell that it **changed** without
    /// scanning every node. A hover runs no handler, so nothing else
    /// reports it, and a host that skips clean subtrees
    /// (`invalidate::Boundaries`) would otherwise keep painting the
    /// shade of the node the pointer left.
    pub hovered: Option<u32>,
    /// Which row, and which node of it, the pointer is over when
    /// [`InputState::hovered`] is a virtualizer — M44/T44.8. What the
    /// host's cursor asks the virtualizer about; the hit test already
    /// knew it and threw it away.
    pub hover_inner: Option<crate::hit::Inner>,
    /// The node a press latched on, until its release — M25/T25.0.
    /// Kept for the same reason as [`InputState::hovered`].
    pub pressed: Option<u32>,
    /// The click count of the press currently in progress (T14.3).
    ///
    /// Tracked rather than carried on `PointerUp`, because activation
    /// happens on the *release* and the count is known at the *press* —
    /// a handler that acts on a double click would otherwise always see
    /// zero. Tracking it here also means a host that synthesizes a
    /// release (the agent, a screen reader) does not have to know the
    /// count of the press it is completing.
    pub clicks: u8,
    /// How many `NodeState` fields the pass has written — a diagnostic,
    /// M44/T44.0 item 7 (`docs/plan-input-semantics.md` T44.6).
    ///
    /// Counted per loop rather than per write (a sweep adds the table's
    /// length once), so it costs one add per sweep. It exists so that
    /// "an event writes the nodes it changed" is a number a test can
    /// bound; a sweep that writes every node's `hovered` bit writes it
    /// whether or not the value changes, which no before/after diff can
    /// see. Compares equal whatever it holds, so two snapshots of an
    /// input state still compare by what they *mean*.
    #[doc(hidden)]
    pub state_writes: WriteCount,
    /// What the press in progress landed on — M44/T44.2
    /// (`docs/plan-input-semantics.md` §2.2).
    ///
    /// Set on `PointerDown` and taken on every `PointerUp`. Activation on
    /// the release runs only for a handler whose node contains the press
    /// too, and for a row only when the row and the virtualizer's
    /// [`row_generation`](crate::widget::Node::row_generation) match: a
    /// click is a press and a release on the same logical item. The
    /// `pressed` index above is the press *shade*; this is the press's
    /// identity, and it is separate because the shade is cleared by the
    /// release before activation asks.
    pub press: Option<PressLatch>,
    /// The node whose `focused` bit the pass last set — M44/T44.6, so a
    /// focus move clears that one node rather than every node.
    #[doc(hidden)]
    pub focus_published: Option<u32>,
    /// For each open overlay, the focus it opened over — M44/T44.9. A
    /// dismissal returns focus there (`pass::settle_overlays`).
    pub overlay_return: [Option<(u32, Option<u32>)>; 4],
    /// The activation keys now held down — bit 0 Enter, bit 1 Space —
    /// M57/A10. A down for a key already held is an auto-repeat, which
    /// does not activate again.
    pub held: u8,
    /// Where the pointer last was, in window coordinates — M57/B48. For
    /// [`crate::widget::Node::cursor_at`]: a node whose cursor depends on
    /// *where* on it the pointer is (a pane grid's two kinds of divider)
    /// needs the point, and the hover bit alone does not carry it.
    pub pointer: Option<Point>,
}

/// The item a press landed on — [`InputState::press`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PressLatch {
    /// The hit's main-table node: for a row, the virtualizer.
    pub node: u32,
    /// The row within it, when the press was on one.
    pub row: Option<u64>,
    /// The node's `row_generation` at the press.
    pub generation: u64,
}

/// [`InputState::state_writes`]: a wrapping count that is never part of
/// an input state's equality.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WriteCount(pub u32);

impl WriteCount {
    /// Record `n` writes.
    #[inline]
    pub fn add(&mut self, n: usize) {
        self.0 = self.0.wrapping_add(n as u32);
    }
}

impl PartialEq for WriteCount {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The single input queue (see the module note).
///
/// **Fixed capacity, in every build.** This was a `VecDeque` until
/// M13/T13.4, which needed the core to compile with no allocator at
/// all. A second, embedded-only queue would have forked the one thing
/// `docs/plan-events.md` E0 set out to unify, so instead there is one
/// ring and every build uses it — which is also the better-tested
/// arrangement, since the existing suite exercises the embedded path
/// without knowing it.
///
/// **`CAP` is a type parameter, and that is the budget that matters.**
///
/// Until 2026-09-07 the capacity was a fixed 64 and `Event`'s 16 bytes
/// were treated as the constraint — every vocabulary decision was
/// negotiated against a number nobody had chosen, because 16 is simply
/// what `Scroll { x, y, dy }` happened to need. The real constraint is
/// this struct: `CAP * size_of::<Event>()` plus bookkeeping, which at
/// the default 64 is 1 048 bytes, 0.8% of the M13 device's 128 KB
/// (`docs/results-embedded.md`).
///
/// Decoupling them means an embedded host that wants 384 bytes writes
/// `InputQueue<16>` and a desktop host that expects bursts writes
/// `InputQueue<256>`, and neither has to argue with the size of an
/// event. `Event` staying small is still worth something — it is
/// multiplied by `CAP` — but it is a cost to weigh rather than a wall.
#[derive(Debug)]
pub struct InputQueue<const CAP: usize = 64> {
    q: [Event; CAP],
    head: usize,
    len: usize,
    dropped: u32,
}

impl<const CAP: usize> Default for InputQueue<CAP> {
    fn default() -> Self {
        InputQueue {
            // Any value works: `len` is what makes a slot live.
            q: [Event::Key { key: Key::Enter, down: false }; CAP],
            head: 0,
            len: 0,
            dropped: 0,
        }
    }
}

impl<const CAP: usize> InputQueue<CAP> {
    /// Queue depth. At the default 64, 60 fps and human input rates,
    /// this is about a second of continuous input without a drain —
    /// and the frame loop drains every frame.
    pub const CAP: usize = CAP;

    pub fn new() -> Self {
        Self::default()
    }

    /// Queue `ev`, or count it as dropped if the queue is full.
    ///
    /// Overflow rejects the **newest** event rather than evicting the
    /// oldest. Either choice breaks gesture pairing, but evicting the
    /// oldest can strand a `PointerUp` whose `PointerDown` is gone —
    /// a click that was never pressed. Rejecting the newest leaves the
    /// queued prefix a coherent history.
    ///
    /// Returns whether it was queued. The drop is *counted*, not
    /// silent: see [`dropped`](Self::dropped). Same rule as the change
    /// journal's `truncated` flag — a gate that hides its own overflow
    /// is worse than no gate. **And something reads the count now**
    /// (M44/T44.5): a `Host` publishes it as `dropped_inputs` and the
    /// agent reports it in `app.diagnostics`; until then the sentence
    /// above was true of the counter and false of the system.
    ///
    /// A host that must not lose a release uses
    /// [`push_merged`](Self::push_merged), which merges moves and evicts
    /// one to make room, instead of this.
    pub fn push(&mut self, ev: Event) -> bool {
        if self.len == CAP {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        }
        self.q[(self.head + self.len) % CAP] = ev;
        self.len += 1;
        true
    }

    /// [`push`](Self::push), **merging moves and keeping releases** —
    /// M44/T44.5 (`docs/plan-input-semantics.md` §2.5).
    ///
    /// Two rules, both about which event is worth a slot:
    ///
    /// - a `PointerMove` **replaces** a `PointerMove` at the back of the
    ///   queue rather than taking a slot of its own. Only the latest
    ///   position of an unbroken run of moves matters, and unmerged moves
    ///   are what filled the queue: a window posts every `CursorMoved`.
    /// - when the queue is full, an event that **ends** something — a
    ///   `PointerUp`, a `PointerCancel`, a key release, a touch's `End`
    ///   or `Cancel` — evicts the newest queued move instead of being
    ///   refused. Refusing it would strand the capture: the drag would
    ///   go on with no button held.
    ///
    /// A bigger queue would only move the cliff, and the embedded budget
    /// pays for every slot. Returns whether `ev` is now queued; an
    /// eviction counts the evicted move as dropped.
    pub fn push_merged(&mut self, ev: Event) -> bool {
        if let Event::PointerMove { .. } = ev {
            if self.len > 0 {
                let last = (self.head + self.len - 1) % CAP;
                if let Event::PointerMove { .. } = self.q[last] {
                    self.q[last] = ev;
                    return true;
                }
            }
        }
        if self.len == CAP && ends_a_gesture(&ev) && self.evict_newest_move() {
            self.dropped = self.dropped.saturating_add(1);
        }
        self.push(ev)
    }

    /// Remove the newest queued `PointerMove`, keeping the order of the
    /// rest. `false` when there is none.
    fn evict_newest_move(&mut self) -> bool {
        let Some(k) = (0..self.len)
            .rev()
            .find(|&k| matches!(self.q[(self.head + k) % CAP], Event::PointerMove { .. }))
        else {
            return false;
        };
        for j in k..self.len - 1 {
            self.q[(self.head + j) % CAP] = self.q[(self.head + j + 1) % CAP];
        }
        self.len -= 1;
        true
    }

    pub fn pop(&mut self) -> Option<Event> {
        if self.len == 0 {
            return None;
        }
        let ev = self.q[self.head];
        self.head = (self.head + 1) % CAP;
        self.len -= 1;
        Some(ev)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Free slots: how many more events [`push`](Self::push) will take.
    pub fn room(&self) -> usize {
        CAP - self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// How many events this queue has ever refused for lack of room.
    /// Non-zero means input was lost and the app should be told, not
    /// that the queue is currently full.
    pub fn dropped(&self) -> u32 {
        self.dropped
    }

    /// Everything queued, oldest first — the recording seed.
    pub fn iter(&self) -> impl Iterator<Item = &Event> {
        (0..self.len).map(|i| &self.q[(self.head + i) % CAP])
    }
}

/// A node selector for handler lookup: a main-table index, plus the
/// off-table row a hit resolved to, if any.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeSel {
    pub i: usize,
    pub inner: Option<(u64, u32)>,
}

impl NodeSel {
    /// The `inner` a dismissal query carries — a row no repeater can have.
    const DISMISSED: (u64, u32) = (u64::MAX, u32::MAX);

    /// **"This node's overlay was just dismissed — who handles that?"** —
    /// M33/T33.3.
    ///
    /// A controlled widget whose popup the *pass* closes (Escape, a press
    /// outside) has to tell the application, or its state would still
    /// say "open" and the next binding would reopen it. Asked through
    /// [`Handlers::handler`] with a sentinel `inner`, so every container
    /// forwards it the way it already forwards a row's selection, and no
    /// activation can ever ask it: activation passes `inner` only at the
    /// node it hit, and a real row index is never `u64::MAX`.
    pub const fn dismissed(owner: usize) -> NodeSel {
        NodeSel { i: owner, inner: Some(Self::DISMISSED) }
    }

    /// Whether this selection is [`NodeSel::dismissed`]'s question.
    pub const fn is_dismissal(&self) -> bool {
        matches!(self.inner, Some(Self::DISMISSED))
    }
}

/// Which handler, if any, a node carries.
///
/// Generic over the application state so that a `Button<A>` placed in a
/// `B`-rooted tree is a **compile error**: the state-type contract that
/// used to be a safety comment is now checked by the type system.
pub trait Handlers<S> {
    fn handler(&self, sel: NodeSel) -> Option<fn(&mut S, &EventCtx)>;
}

/// What [`App::update_marked`] wrote — M29/T29.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrote {
    /// No binding wrote anything: the frame may be still.
    Nothing,
    /// Every write was reported as a range.
    Marked,
    /// Something wrote where it cannot say: mark the whole tree.
    Everything,
}

impl Wrote {
    /// From "did anything change" and "was any of it unlocatable".
    pub const fn of(changed: bool, everywhere: bool) -> Wrote {
        match (changed, everywhere) {
            (false, _) => Wrote::Nothing,
            (true, false) => Wrote::Marked,
            (true, true) => Wrote::Everything,
        }
    }
}

/// The message type of an application that defers no work — M28/T28.2.
///
/// Uninhabited, which is stronger than an empty struct in the way that
/// matters here: a `Sender<Never>` exists but nothing can be sent
/// through it, so "this application has no deferred work" is enforced by
/// the type rather than by everyone remembering. It is also why
/// [`App::apply`]'s default body is total without being reachable.
#[derive(Debug)]
pub enum Never {}

/// The application root: state plus the tree that presents it.
///
/// Splitting the tree out of `S` is what lets dispatch hold `&mut S`
/// while a handler runs: the handler `fn` pointer is copied out through
/// `&*self` first, that borrow ends, and only then does `&mut S` exist.
/// The borrow checker enforces the sequence that used to be a documented
/// rule (`docs/plan-events.md` E2).
///
/// **Construction is not this trait's job, nor any trait's.** An
/// application is built by the boot function its program is described
/// with — `lumen_static_shell::program::application(boot)` — and every
/// path that builds one asks that description: the window, every
/// headless host, `Host::reload`, and a hot window's swap, which boots
/// the new library's `program()` (M32, M37). M31's `Application::build`
/// was a second, trait-shaped path beside it that only the reload derive
/// called; M49/T49.4 retired it (`docs/plan-reload-hardening.md` §2.2).
pub trait App: Sized {
    /// **What a handler is allowed to touch** — M33
    /// (`docs/plan-state-split.md`).
    ///
    /// `Self` unless the application names a `#[state]` field, and then
    /// something stronger than tidiness: a handler is
    /// `fn(&mut Self::State, &EventCtx)`, the tree is a *sibling* of the
    /// state, and so **a handler cannot name a widget on the tree**:
    ///
    /// ```compile_fail
    /// use lumen_static::prelude::*;
    /// use lumen_static_widgets::prelude::*;
    /// #[derive(StaticTree, Default)]
    /// #[widget(column)]
    /// struct Ui { title: Label, inc: Button<Tally> }
    /// #[derive(Default)]
    /// struct Tally { count: i64 }
    /// #[derive(DeriveApp)]
    /// struct Counter { #[tree] ui: Ui, #[state] state: Tally }
    /// fn on_inc(t: &mut Tally, _: &EventCtx) { t.ui.title.set("reached the tree"); }
    /// # fn main() { let _ = on_inc; }
    /// ```
    ///
    /// and the same program, differing only in that handler's body,
    /// compiles — which is what makes the refusal above specific. Stable
    /// rustdoc does not check a `compile_fail` block's error code, so
    /// without this twin any unrelated error would pass it. (Observed on
    /// the day: `error[E0609]: no field 'ui' on type '&mut Tally'`.)
    ///
    /// ```
    /// use lumen_static::prelude::*;
    /// use lumen_static_widgets::prelude::*;
    /// #[derive(StaticTree, Default)]
    /// #[widget(column)]
    /// struct Ui { title: Label, inc: Button<Tally> }
    /// #[derive(Default)]
    /// struct Tally { count: i64 }
    /// #[derive(DeriveApp)]
    /// struct Counter { #[tree] ui: Ui, #[state] state: Tally }
    /// fn on_inc(t: &mut Tally, _: &EventCtx) { t.count += 1; }
    /// # fn main() { let _ = on_inc; }
    /// ```
    ///
    /// **What it does not promise:** interior mutability shared between
    /// the two. An `Rc<Cell<_>>` held by both a widget and the state lets
    /// a handler change what a widget shows with no binding involved.
    /// Nothing in the workspace does that; the invalidation oracle is
    /// what would catch it.
    ///
    /// Not defaulted, because associated type defaults are unstable
    /// (M28/T28.6 paid this for `Msg`): a derived application gets
    /// `type State = Self` emitted for it, and every hand-written impl
    /// says it out loud — thirty-two of them when this landed, counted
    /// with `git grep` across both workspaces.
    type State;
    /// The state a handler receives. `self` unless the application
    /// names a separate state type.
    fn state_mut(&mut self) -> &mut Self::State;

    /// **Whether a handler's state excludes the tree** — M33 §2.2.
    ///
    /// The derive answers `true` exactly when a `#[state]` field exists,
    /// and refuses the two ways that could lie: `#[state]` on the
    /// `#[tree]` field, and a second `#[state]` field. A hand-written
    /// impl gets `false`, the conservative answer. One that sets `true`
    /// is making a promise nothing checks — the invalidation oracle is
    /// the backstop — and M29/T29.6 narrows the mark for a consumed event
    /// only when this is `true`.
    const STATE_IS_SPLIT: bool = false;

    type Tree: crate::widget::Widget + Handlers<Self::State>;
    fn tree(&self) -> &Self::Tree;
    fn tree_mut(&mut self) -> &mut Self::Tree;

    /// What deferred work sends back — M28/T28.2.
    ///
    /// `Host` is generic over the application alone, so an inbox of `M`
    /// had no `M` in scope until this existed. Declaring it here rather
    /// than as a fourth host parameter keeps the message with the code
    /// that interprets it, which is the whole point of
    /// `docs/events-decision.md`'s rule: **work in flight carries a
    /// value, and whatever build is loaded when it lands decides what
    /// the value means.**
    ///
    /// The default is [`Never`], which is uninhabited. An application
    /// that defers nothing writes nothing, cannot construct a message,
    /// and — because a `Sender<Never>` has no `send` anyone can call —
    /// cannot accidentally acquire deferred work by forgetting to opt
    /// out. The ~27 derived apps and 13 hand-written impls that defer
    /// nothing write `type Msg = Never;` — which the derive emits, so
    /// only the 20 hand-written impls say it out loud.
    ///
    /// **Required, not defaulted**, because associated type defaults are
    /// unstable (rust-lang#29661). The plan assumed otherwise and said
    /// the hand-written impls would be untouched; they are not, and two
    /// lines each is the honest price.
    type Msg: Send + 'static;

    /// Interpret one message that arrived since the last frame.
    ///
    /// Returns **whether anything changed**, for the same reason
    /// [`App::update`] does: under
    /// [`Boundaries`](crate::invalidate::Boundaries) a `false` here is
    /// the difference between skipping the tree and walking it.
    ///
    /// Called from the frame loop on the thread that owns the tree —
    /// never from the worker that sent it — with **the state, not the
    /// application** (M33/T33.2). A deferred message is handler-shaped
    /// work, so it gets what a handler gets: for a split application
    /// (`#[state]`) the tree is not reachable from here, which is what
    /// lets a message frame be narrowed like a handled one. For an
    /// unsplit one, `State = Self` and nothing changes but the spelling:
    /// `fn apply(_: &mut Self, m: …)`.
    ///
    /// **No default body, deliberately.** A default that ignored the
    /// message and returned `false` would compile for an application
    /// that declares a real `Msg` and forgets to interpret it — and the
    /// symptom would be deferred work that silently never lands, which
    /// is the hardest class of bug this repository keeps finding. An
    /// application with nothing to defer writes
    /// `fn apply(_: &mut Self::State, m: Never) -> bool { match m {} }`, which the
    /// derive emits; `match` on an uninhabited type is how that says
    /// "unreachable" to the compiler rather than to the reader.
    fn apply(state: &mut Self::State, m: Self::Msg) -> bool;

    /// This application's queue **out** of the frame — M28/T28.4.
    ///
    /// A handler is `fn(&mut S, &EventCtx)`: it reaches the app and
    /// nothing else, so it cannot be handed a spawner. It pushes a
    /// [`Descriptor`](crate::spawn::Descriptor) here instead, and the
    /// host drains it after dispatch.
    ///
    /// `None` by default, which is what an application that defers
    /// nothing answers — and it costs such an application no field.
    #[cfg(feature = "std")]
    fn outbox(&mut self) -> Option<&mut crate::spawn::Outbox> {
        None
    }

    /// The job body for a descriptor, **from the currently loaded
    /// build** — M28/T28.4, keyed on `(kind, step)` since M28/T28.6.
    ///
    /// This is the seam that keeps a code pointer out of flight: the
    /// host calls it at spawn time, in whatever build is loaded then,
    /// and what travels is the descriptor.
    ///
    /// **It takes the whole descriptor, not just the kind**, because the
    /// step *is* the chain. `docs/plan-tasks.md` §2.1: iced chains with
    /// `.then(|out| next)`, a closure held across an await, which puts a
    /// code pointer in flight and breaks
    /// `docs/events-decision.md`'s rule 3 on the first reload. The way
    /// out is the move this architecture already made for handlers —
    /// identity became *positional* — so **the next step is an index,
    /// not a function**, and dispatching on it is what makes it one.
    ///
    /// An unrecognised `(kind, step)` answers `None` and the host counts
    /// the drop rather than guessing, which is §2.1's stability rule and
    /// the same treatment the reload model gives an enum variant a new
    /// build removed. Guessing is what makes a renumbering silently run
    /// the wrong step.
    #[cfg(feature = "std")]
    fn job_for(_d: crate::spawn::Descriptor) -> Option<crate::spawn::JobFn<Self::Msg>>
    where
        Self: Sized,
    {
        None
    }

    /// Whether the application has asked to cancel everything in
    /// flight — M28/T28.12.
    ///
    /// **Takes it**, so a request is acted on once. A handler is
    /// `fn(&mut S, &EventCtx)` and cannot reach the host, so a cancel
    /// travels the way a spawn request does: the handler sets a flag,
    /// and the host asks on the next frame. There is no second path for
    /// an application to reach the host, deliberately.
    ///
    /// Default `false`: an application that never cancels pays one
    /// inlined `false` per frame.
    #[cfg(feature = "std")]
    fn take_cancel(&mut self) -> bool {
        false
    }

    /// This application's periodic work — M28/T28.11.
    ///
    /// Advanced by `Host::tick`, which pushes a descriptor for each
    /// timer that came due into the outbox — so a tick is spawned by
    /// exactly the same path a handler's request is, and **no thread is
    /// created for a timer**.
    ///
    /// `None` by default, which costs an application with no timers no
    /// field.
    #[cfg(feature = "std")]
    fn timers(&mut self) -> Option<&mut crate::spawn::Timers> {
        None
    }

    /// What should be running — M28/T28.10.
    ///
    /// The host clears this, asks the application to re-declare into it,
    /// and diffs the result against what it already started. Identity is
    /// `(kind, key)`: values, not a hash of the code that produces the
    /// stream.
    ///
    /// `None` by default, which is what an application with no
    /// subscriptions answers — and it costs such an application no
    /// field.
    #[cfg(feature = "std")]
    fn subscriptions(&mut self) -> Option<&mut crate::spawn::Subs> {
        None
    }

    /// Turn a job's failure into one of this application's messages —
    /// M28/T28.9.
    ///
    /// A job that panics, and a spawn the OS refuses, both produce a
    /// **failure result applied like any other** — data, not a callback.
    /// Without it, a panicking job under the thread default unwinds, its
    /// sender drops, and a chain waiting on that completion stalls
    /// forever with nothing observing it.
    ///
    /// `None` — the default — means this application does not model that
    /// failure, and the result is simply absent. That is a choice an
    /// application is allowed to make; it is not a choice the framework
    /// should make silently on its behalf, which is why the hook exists
    /// even though its default does nothing.
    #[cfg(feature = "std")]
    fn failed(_d: crate::spawn::Descriptor, _why: crate::spawn::Failure) -> Option<Self::Msg>
    where
        Self: Sized,
    {
        None
    }

    /// Refresh the tree's *presentation copies* from the state they are
    /// derived from. Called by the frame loop after input is drained and
    /// before layout, so a handler that changes state cannot leave a
    /// stale value on screen.
    ///
    /// Widget fields hold the presentation copy and sibling fields are
    /// the source of truth (`docs/layout-and-storage-decisions.md` §1);
    /// this is the seam where the copy is brought back into agreement.
    /// `#[derive(App)]` writes the body from `#[bind(...)]` declarations
    /// — see `crate::bind`. The default does nothing, for trees that
    /// present no derived state.
    /// Returns **whether anything actually changed** —
    /// `docs/plan-invalidation.md` T16.10.
    ///
    /// A binding already compares before it writes
    /// ([`crate::bind::format_into`] returns whether the text differed),
    /// so this costs nothing to report and is what lets a host know a
    /// frame is genuinely still. Under
    /// [`crate::invalidate::Boundaries`] a `false` here is the
    /// difference between skipping the tree and walking it.
    ///
    /// The default is `false`: a tree that presents no derived state
    /// changes nothing here, which is true rather than merely safe.
    fn update(&mut self) -> bool {
        false
    }

    /// Run the bindings and **say where they wrote** — M29/T29.3
    /// (`docs/plan-damage-precision.md`).
    ///
    /// `mark` receives one table range per binding that wrote. The
    /// derive answers precisely: each binding's node is found by address
    /// once ([`crate::bind::node_of`]) and cached per site. A `with =`
    /// hook, an `after_update` hook that reports a change, and a target
    /// outside the table answer [`Wrote::Everything`], because none of
    /// them can say where.
    ///
    /// **The default is the conservative answer**: a hand-written
    /// `update` that returned `true` wrote somewhere this cannot know, so
    /// it is everything.
    fn update_marked(&mut self, mark: &mut dyn FnMut(core::ops::Range<usize>)) -> Wrote {
        let _ = mark;
        if self.update() {
            Wrote::Everything
        } else {
            Wrote::Nothing
        }
    }

    /// Advance whatever this application animates by `dt` milliseconds,
    /// and say whether anything changed — `docs/plan-capabilities.md`
    /// T15.4.
    ///
    /// **Here rather than as a per-node pass**, deliberately. A
    /// `Node::tick` walked by the framework would cost a virtual call
    /// per node per frame on trees that animate nothing — 3 000 of them
    /// on the benchmark's list — which the M15 gating policy forbids.
    /// The application knows which of its fields are `Animated<T>`;
    /// nothing else needs to.
    ///
    /// The default returns `false`, so an application that animates
    /// nothing has no per-frame cost and no code to write.
    ///
    /// A frame loop that gets `true` should schedule another frame; one
    /// that gets `false` can idle, which is what keeps a still UI at the
    /// idle numbers in `docs/results-frame-cost.md`.
    ///
    /// **A `dt_ms` of `0.0` must advance nothing and answer whether
    /// anything is still in motion** — M22/T22.4. A frame loop ticks
    /// before it pumps, because the bindings the pump runs are what put
    /// a ticked value on screen; but a press dispatched *during* the
    /// pump starts an animation that tick could not have seen, so the
    /// loop asks again afterwards with a zero `dt`. Forwarding to
    /// [`crate::anim::Animated::tick`] satisfies this without a thought:
    /// it adds `clamp(0, 0, duration)` and returns `!settled()`.
    ///
    /// **The contract under `Boundaries`** — M41/T41.0
    /// (`docs/plan-boundaries-repair.md` §1.1). `tick` takes `&mut self`
    /// and may write any widget, and it reports a `bool`, not ranges, so
    /// the host marks the whole tree when a tick with a positive `dt`
    /// answers `true` (M34/T34.0) — and marks nothing otherwise. Two
    /// things therefore follow, and a tick that breaks either paints a
    /// stale frame under the default strategy while `Everything` hides it:
    ///
    /// - **a tick that writes the tree returns `true`**, and
    /// - **a tick never writes on a zero `dt`**, which is a pure query
    ///   (above) and is not marked.
    ///
    /// Writing state here and presenting it through `update`'s bindings
    /// is always safe: the bindings mark what they write.
    fn tick(&mut self, dt_ms: f32) -> bool {
        let _ = dt_ms;
        false
    }
}

/// Whether `ev` ends a gesture, so that losing it would strand one —
/// [`InputQueue::push_merged`].
fn ends_a_gesture(ev: &Event) -> bool {
    matches!(
        ev,
        Event::PointerUp { .. }
            | Event::PointerCancel { .. }
            | Event::Key { down: false, .. }
            | Event::Touch { phase: TouchPhase::End | TouchPhase::Cancel, .. }
    )
}

impl<const CAP: usize> Extend<Event> for InputQueue<CAP> {
    /// Events past this queue's own `CAP` are counted as dropped, exactly
    /// as [`InputQueue::push`] does — `Extend` cannot report a refusal,
    /// so the count is the only channel it has.
    fn extend<T: IntoIterator<Item = Event>>(&mut self, iter: T) {
        for ev in iter {
            self.push(ev);
        }
    }
}

#[cfg(test)]
mod queue_tests {
    use super::*;

    fn ev(n: u8) -> Event {
        Event::Text { ch: n as char }
    }

    #[test]
    fn fifo_and_wraparound() {
        let mut q: InputQueue = InputQueue::new();
        // Push and drain past CAP so head wraps the backing array — the
        // case a VecDeque handled for free and this ring must not get
        // wrong.
        for round in 0..5u32 {
            for i in 0..40u8 {
                assert!(q.push(ev(i)), "round {round} had room");
            }
            for i in 0..40u8 {
                assert_eq!(q.pop(), Some(ev(i)), "FIFO order, round {round}");
            }
            assert!(q.is_empty());
        }
        assert_eq!(q.dropped(), 0, "never over capacity, so never dropped");
    }

    #[test]
    fn overflow_rejects_the_newest_and_counts_it() {
        let mut q: InputQueue = InputQueue::new();
        for i in 0..InputQueue::<64>::CAP {
            assert!(q.push(ev(i as u8)));
        }
        assert!(!q.push(ev(200)), "full queue refuses");
        assert!(!q.push(ev(201)));
        assert_eq!(q.dropped(), 2, "and says how much it lost");
        assert_eq!(q.len(), InputQueue::<64>::CAP);

        // The queued prefix is intact: rejecting the newest is what
        // keeps it a coherent history rather than one with a hole.
        assert_eq!(q.pop(), Some(ev(0)));
        assert_eq!(q.iter().count(), InputQueue::<64>::CAP - 1);
    }

    #[test]
    fn iter_is_oldest_first_across_a_wrap() {
        let mut q: InputQueue = InputQueue::new();
        for i in 0..50u8 {
            q.push(ev(i));
        }
        for _ in 0..40 {
            q.pop();
        }
        for i in 50..70u8 {
            q.push(ev(i));
        }
        let seen: Vec<u8> = q.iter().map(|e| match e {
            Event::Text { ch } => *ch as u8,
            _ => unreachable!(),
        }).collect();
        assert_eq!(seen, (40..70u8).collect::<Vec<_>>(), "oldest first, wrap included");
    }

    /// A capacity other than the default behaves identically — the
    /// generic changed the budget, not the semantics.
    #[test]
    fn a_small_queue_is_the_same_queue() {
        let mut q: InputQueue<4> = InputQueue::new();
        for i in 0..4u8 {
            assert!(q.push(ev(i)), "room for {i}");
        }
        assert!(!q.push(ev(9)), "and full at 4");
        assert_eq!(q.dropped(), 1);
        for i in 0..4u8 {
            assert_eq!(q.pop(), Some(ev(i)), "still FIFO");
        }
        // And it wraps, which is where a hand-rolled ring usually
        // breaks and where a small `CAP` reaches soonest.
        for round in 0..5u8 {
            for i in 0..3u8 {
                q.push(ev(round * 3 + i));
            }
            for i in 0..3u8 {
                assert_eq!(q.pop(), Some(ev(round * 3 + i)), "wrap round {round}");
            }
        }
    }

    #[test]
    fn extend_counts_what_it_could_not_take() {
        let mut q: InputQueue = InputQueue::new();
        q.extend((0..70u8).map(ev));
        assert_eq!(q.len(), InputQueue::<64>::CAP);
        assert_eq!(q.dropped(), 70 - InputQueue::<64>::CAP as u32);
    }
}
