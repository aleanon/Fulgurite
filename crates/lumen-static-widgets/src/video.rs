//! `Video<S>` — a picture that a feed keeps replacing, with transport
//! controls — M84/T84.8 (`docs/plan-images-in-motion.md`,
//! `docs/results-images.md`).
//!
//! # A controlled widget, not a codec
//!
//! The widget decodes nothing and owns no clock. It shows `(id, frame)`
//! from the application's image store — the store a
//! `lumen_static_shell::video::VideoFeed` fills through the application's
//! messages, `ImageStore::replace` answering the frame to paint — and it
//! draws a play/pause button, a seek track and the time. Everything it
//! shows is the **application's**, written back every frame the way a
//! [`Select`](crate::Select)'s `open` is (M33): `playing`, `position_ms`,
//! `duration_ms` and `frame`. The widget reports intent and never acts on
//! it:
//!
//! - **`on_toggle`** runs when the play button is activated (a click,
//!   Enter or Space on it, or a reader's default action on the video);
//!   the application flips its `playing` and starts or stops its feed.
//! - **`on_seek`** runs with the fraction of the track the user chose, in
//!   `0..=1`: a press on the track, the end of a drag along it, an arrow
//!   key on it, or a reader's `SetValue`. The handler reads it with
//!   [`seek_fraction`] and multiplies by its own `duration_ms`.
//!
//! **The widget computes the fraction**, from the press's position
//! against the track it laid out, because only it knows the track's
//! width: `EventCtx` carries no geometry of the receiving widget, and a
//! handler cannot reach the tree. It reaches the handler through
//! [`Node::take_action`], the pass's route for a value a consumed gesture
//! produced (`docs/plan-images-in-motion.md`, "What building found",
//! T84.8): the plan wrote `on_seek: fn(&mut S, f32)`, which no handler
//! table can return, since a handler is one `fn(&mut S, &EventCtx)` per
//! node and cannot capture the widget's pointer.
//!
//! # A drag previews, a release seeks
//!
//! A press on the track seeks at once and captures the pointer; the knob
//! follows the drag without seeking (a seek re-keys the application's
//! feed, and one per pointer move would restart it sixty times a second);
//! the release seeks to where it ended. The track is its own, not a nested
//! [`Slider`](crate::Slider), which owns its value and has no change
//! handler (M57: a change handler on `Slider` is recorded as not built).

#[cfg(feature = "a11y")]
use alloc::vec::Vec;

use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::{Color, ImageId, Painter};
use lumen_static::event::{Action, Event, EventCtx, Key, NodeSel};
use lumen_static::hit::{contains, Hit};
use lumen_static::ops::{record_painted, rect_of, resolve_self};
use lumen_static::style::{Edges, Fields, Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::LayoutCx;

use crate::image::Image;
use crate::ops::Button;

/// `#id` of the picture, for a stylesheet and the agent:
/// `#my_video > #video-picture`.
pub const PART_PICTURE: &str = "video-picture";
/// `#id` of the play/pause button.
pub const PART_PLAY: &str = "video-play";
/// `#id` of the seek track.
pub const PART_TRACK: &str = "video-track";

/// The controls row's height, in logical px: the track's 24 px target
/// (WCAG 2.5.8) and its padding.
pub const CONTROLS_H: f32 = 48.0;
/// The play button's fixed width: its label flips between "Play" and
/// "Pause", and a button that changed width would move the track under
/// the pointer (the Image demo's pause button found it, M84/T84.3).
pub const PLAY_W: f32 = 76.0;
/// The time text's box, "mm:ss / mm:ss" at the kind's font size.
const TIME_W: f32 = 104.0;
/// Between the controls, and inside the row's edges.
const GAP: f32 = 8.0;
/// The seek track's height, the pointer-target floor.
const TRACK_H: f32 = 24.0;

/// **The fraction a seek chose**, `0..=1`, from the `ctx` an `on_seek`
/// handler is handed — `None` for anything else.
///
/// The pass hands the handler the value as
/// `Event::Action { action: Action::SetValue(f), .. }`
/// ([`Node::take_action`]); this reads it, so a handler need not match on
/// the event itself.
pub fn seek_fraction(ctx: &EventCtx) -> Option<f32> {
    match ctx.event {
        Event::Action { action: Action::SetValue(f), .. } if f.is_finite() => Some(f.clamp(0.0, 1.0)),
        _ => None,
    }
}

/// `0..=1`, with NaN mapped to 0 (`Slider`'s rule, for the same reason:
/// a non-finite knob rect is never hit again).
fn clamp01(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        lumen_static::num::clamp(v, 0.0, 1.0)
    }
}

/// "m:ss / m:ss" in a fixed buffer: written in layout whenever the time
/// moves, which a playing video does every frame, so a `String` would be
/// one allocation per video frame (`alloc_gate.rs`'s video arm bounds it
/// at 0).
#[derive(Clone, Copy)]
struct TimeText {
    buf: [u8; 24],
    len: u8,
}

impl TimeText {
    const fn new() -> Self {
        TimeText { buf: [0; 24], len: 0 }
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len as usize]).unwrap_or("")
    }

    fn set(&mut self, position_ms: u32, duration_ms: u32) {
        use core::fmt::Write;
        self.len = 0;
        let (p, d) = (position_ms / 1000, duration_ms / 1000);
        let _ = write!(self, "{}:{:02} / {}:{:02}", p / 60, p % 60, d / 60, d % 60);
    }
}

impl core::fmt::Write for TimeText {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let at = self.len as usize;
        let end = at + s.len();
        if end > self.buf.len() {
            return Err(core::fmt::Error);
        }
        self.buf[at..end].copy_from_slice(s.as_bytes());
        self.len = end as u8;
        Ok(())
    }
}

/// The seek track: a bar, the played part, and a knob.
///
/// A part of [`Video`], not a widget on its own: its handler is the
/// video's `on_seek`, and its value is the video's `position_ms` over
/// `duration_ms`, written by the video's layout.
pub struct SeekTrack {
    /// The played fraction, from the video's controlled position.
    value: f32,
    /// Where a drag in flight has the knob: drawn instead of `value`
    /// until the release seeks there.
    preview: Option<f32>,
    /// The fraction a consumed event chose, for [`Node::take_action`].
    pending: Option<f32>,
    /// How far an arrow key moves it.
    pub step: f32,
    /// The track's width from the last layout: the press's `local`
    /// against it is the fraction (P19).
    track_w: f32,
}

impl Default for SeekTrack {
    fn default() -> Self {
        SeekTrack { value: 0.0, preview: None, pending: None, step: 0.05, track_w: 0.0 }
    }
}

impl SeekTrack {
    /// The fraction a pointer at `local_x` picks: the track's own width,
    /// from its own left edge (`cx.local` is the receiving node's).
    fn at(&self, local_x: f32) -> f32 {
        if self.track_w <= 0.0 {
            return self.value;
        }
        clamp01(local_x / self.track_w)
    }

    /// A seek to `f`, for the pass to hand to `on_seek`.
    fn seek(&mut self, f: f32) -> bool {
        self.pending = Some(clamp01(f));
        true
    }
}

impl Widget for SeekTrack {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];

    const KIND: &'static str = "videotrack";

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::Fixed(TRACK_H),
            radius: 3.0,
            // The unplayed bar, light on the player's dark ground; the
            // played part and the knob in the accent.
            background: Color(100, 116, 139, 255),
            color: Color(96, 165, 250, 255),
            cursor: lumen_static::style::Cursor::Pointer,
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] = &[Seed::new(Self::KIND, Self::type_defaults, Fields::of(&[
        Fields::WIDTH,
        Fields::HEIGHT,
        Fields::CURSOR,
        Fields::FOCUS_RING,
        Fields::VISIBLE,
        Fields::COLOR,
        Fields::BACKGROUND,
        Fields::RADIUS,
    ]))];
}

impl Node for SeekTrack {
    lumen_static::node_count!();

    /// Arrows seek, so they never leave the track ([`Node::owns_arrows`]).
    fn owns_arrows(&self) -> bool {
        true
    }

    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
    }

    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn layout(&mut self, _cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        let me = style[i];
        let size = resolve_self(limits, &me, Size { w: TRACK_H * 4.0, h: TRACK_H });
        self.track_w = size.w;
        record_painted(st, i, limits, size, &me)
    }

    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let t = 6.0_f32.min(h);
        let mid = r[1] + h / 2.0;
        out.rounded_rect([r[0], mid - t / 2.0, r[2], mid + t / 2.0], s.bg, t / 2.0);
        let f = self.preview.unwrap_or(self.value);
        let x = r[0] + w * f;
        if f > 0.0 {
            out.rounded_rect([r[0], mid - t / 2.0, x, mid + t / 2.0], s.fg, t / 2.0);
        }
        let d = 14.0_f32.min(h);
        out.rounded_rect([x - d / 2.0, mid - d / 2.0, x + d / 2.0, mid + d / 2.0], s.fg, d / 2.0);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        contains(&st[i], x, y).then(|| Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        })
    }

    /// The gesture, and what it chose for `on_seek`.
    ///
    /// Consuming the press captures the pointer, so the drag keeps
    /// arriving after it leaves the track, and the release is the
    /// track's too. Enter and Space are consumed and do nothing: the
    /// track's handler is `on_seek`, and an activation reaching it would
    /// hand it no fraction.
    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            // ANCHOR: seek
            Event::PointerDown { .. } => {
                let f = self.at(cx.local.x);
                self.preview = Some(f);
                self.seek(f)
            }
            Event::PointerMove { .. } => {
                let f = self.at(cx.local.x);
                let moved = self.preview != Some(f);
                self.preview = Some(f);
                moved
            }
            Event::PointerUp { .. } => {
                let f = self.at(cx.local.x);
                self.preview = None;
                self.seek(f)
            }
            // ANCHOR_END: seek
            Event::Key { key: Key::Left | Key::Down, down: true } => self.seek(self.value - self.step),
            Event::Key { key: Key::Right | Key::Up, down: true } => self.seek(self.value + self.step),
            Event::Key { key: Key::PageDown, down: true } => self.seek(self.value - self.step * 10.0),
            Event::Key { key: Key::PageUp, down: true } => self.seek(self.value + self.step * 10.0),
            Event::Key { key: Key::Home, down: true } => self.seek(0.0),
            Event::Key { key: Key::End, down: true } => self.seek(1.0),
            Event::Key { key: Key::Enter | Key::Space, down: true } => true,
            Event::Action { action: Action::Default, .. } => true,
            Event::Action { action: Action::Increment, .. } => self.seek(self.value + self.step),
            Event::Action { action: Action::Decrement, .. } => self.seek(self.value - self.step),
            Event::Action { action: Action::SetValue(v), .. } => self.seek(*v),
            _ => false,
        }
    }

    fn take_action(&mut self) -> Option<(u32, Action)> {
        self.pending.take().map(|f| (0, Action::SetValue(f)))
    }

    fn node(&self, _i: usize) -> &dyn Node {
        self
    }

    fn node_mut(&mut self, _i: usize) -> &mut dyn Node {
        self
    }

    /// A slider over the played fraction, with the actions `on_event`
    /// handles (M54/T54.4).
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        use lumen_static::observe::accesskit::{Action as Ak, Role};
        node.set_role(Role::Slider);
        node.set_label("Seek");
        node.add_action(Ak::Increment);
        node.add_action(Ak::Decrement);
        node.add_action(Ak::SetValue);
        node.set_numeric_value(self.value as f64);
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(1.0);
        node.set_numeric_value_step(self.step as f64);
    }
}

impl<S> lumen_static::event::Handlers<S> for SeekTrack {
    /// None: a track alone has no application to tell. Inside a
    /// [`Video`], the video answers for it with `on_seek`.
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

/// A video: the picture, a play/pause button, a seek track and the time.
///
/// See the module docs for what it owns (nothing) and what the
/// application does.
pub struct Video<S: 'static> {
    /// The picture: `id` and `source` are the stream's, and `frame` is
    /// written from [`Video::frame`] at every layout.
    pub picture: Image,
    /// The play/pause button. Its label is written from
    /// [`Video::playing`] at every layout; its handler is the video's
    /// `on_toggle`.
    pub play: Button<S>,
    /// The seek track.
    pub track: SeekTrack,
    /// **Controlled**: whether the application's feed is running. Decides
    /// the button's label ("Pause" while playing).
    pub playing: bool,
    /// **Controlled**: where the stream is, in ms.
    pub position_ms: u32,
    /// **Controlled**: how long it is, in ms; 0 for a stream with no end,
    /// which draws the track empty.
    pub duration_ms: u32,
    /// **Controlled**: the version of the picture to draw — what the
    /// application's `ImageStore::replace` answered for the last frame it
    /// took. Writing it is what repaints the picture's rect
    /// ([`Cmd::Image`](lumen_static::Cmd::Image)'s `frame`, M84/T84.2).
    pub frame: u32,
    /// Run when the play button is activated.
    pub on_toggle: fn(&mut S, &EventCtx),
    /// Run with the chosen fraction; read it with [`seek_fraction`].
    pub on_seek: fn(&mut S, &EventCtx),
    /// What a screen reader announces the video as — M54/T54.3. Empty by
    /// default and not published when empty. Only with `a11y`; write it
    /// through `set_label`, which every build has.
    #[cfg(feature = "a11y")]
    pub label: &'static str,
    time: TimeText,
    /// What the root's own consumed event asked for, for
    /// [`Node::take_action`]: a reader's default action toggles, its
    /// `SetValue` seeks.
    pending: Option<(u32, Action)>,
}

impl<S: 'static> Default for Video<S> {
    fn default() -> Self {
        Video::new(ImageId(0), Size { w: 0.0, h: 0.0 })
    }
}

impl<S: 'static> Video<S> {
    /// A video showing image `id`, whose frames are `source` px.
    pub fn new(id: ImageId, source: Size) -> Self {
        let mut play = Button::<S>::default();
        play.label = "Play";
        Video {
            picture: Image::new(id, source),
            play,
            track: SeekTrack::default(),
            playing: false,
            position_ms: 0,
            duration_ms: 0,
            frame: 0,
            on_toggle: crate::ops::ignore_click::<S>,
            on_seek: crate::ops::ignore_click::<S>,
            #[cfg(feature = "a11y")]
            label: "",
            time: TimeText::new(),
            pending: None,
        }
    }

    /// The time as it is drawn: "m:ss / m:ss".
    pub fn time_text(&self) -> &str {
        self.time.as_str()
    }

    /// The played fraction the track draws.
    pub fn fraction(&self) -> f32 {
        if self.duration_ms == 0 {
            0.0
        } else {
            clamp01(self.position_ms as f32 / self.duration_ms as f32)
        }
    }

    /// Name this video for a screen reader (M54/T54.3). A no-op in a
    /// build without `a11y`, which carries no name.
    #[allow(unused_variables)]
    pub fn set_label(&mut self, label: &'static str) {
        #[cfg(feature = "a11y")]
        {
            self.label = label;
        }
    }

    const PICTURE: usize = 1;
    const PLAY: usize = 2;
    const TRACK: usize = 3;
}

impl<S: 'static> Widget for Video<S> {
    const NODE_COUNT: usize = 4;
    const DEPTH: usize = 2;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = {
        assert!(
            <Image as Widget>::NODE_COUNT == 1
                && <Button<S> as Widget>::NODE_COUNT == 1
                && <SeekTrack as Widget>::NODE_COUNT == 1,
            "Video's table assumes three one-node parts"
        );
        &[
            NodeDesc::new(1, NO_PARENT, 0, 3, NodeDesc::CONTAINER),
            NodeDesc::new(0, 0, 2, 0, 0).with_id(PART_PICTURE),
            NodeDesc::new(0, 0, 3, 0, NodeDesc::FOCUSABLE).with_id(PART_PLAY),
            NodeDesc::new(0, 0, 0, 0, NodeDesc::FOCUSABLE).with_id(PART_TRACK),
        ]
    };

    const KIND: &'static str = "video";

    fn type_defaults() -> Style {
        Style {
            // A player is dark in either palette: the picture is
            // letterboxed on it, and the time is light on it.
            background: Color(15, 23, 42, 255),
            color: Color(226, 232, 240, 255),
            radius: 10.0,
            font_size: 13.0,
            padding: Edges::all(0.0),
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] = &[
        Seed::new(Self::KIND, Self::type_defaults, Fields::of(&[
            Fields::WIDTH,
            Fields::HEIGHT,
            Fields::VISIBLE,
            Fields::COLOR,
            Fields::BACKGROUND,
            Fields::RADIUS,
            Fields::FONT_SIZE,
            Fields::BORDER,
            Fields::BACKGROUND_FILL,
        ])),
        <Image as Widget>::SEEDS[0],
        <Button<S> as Widget>::SEEDS[0],
        <SeekTrack as Widget>::SEEDS[0],
    ];
}

impl<S: 'static> Node for Video<S> {
    lumen_static::node_count!();

    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
    }

    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn layout(&mut self, cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        let me = style[i];
        if !me.visible {
            return lumen_static::ops::hide_subtree(st, i, limits, <Self as Widget>::NODE_COUNT);
        }
        // The controlled fields, onto the parts. Compared before written
        // by the caller's binding; here they are copies, so writing them
        // every layout costs nothing and allocates nothing.
        self.picture.frame = self.frame;
        self.play.label = if self.playing { "Pause" } else { "Play" };
        self.track.value = self.fraction();
        self.time.set(self.position_ms, self.duration_ms);

        // The picture's own aspect, at the width offered; 16:9 with no
        // source size.
        let src = self.picture.source;
        let natural_w = if src.w > 0.0 { src.w.max(320.0) } else { 320.0 };
        let aspect = if src.w > 0.0 && src.h > 0.0 { src.h / src.w } else { 9.0 / 16.0 };
        let want = Size { w: natural_w, h: natural_w * aspect + CONTROLS_H };
        let size = resolve_self(limits, &me, want);
        let pic_h = (size.h - CONTROLS_H).max(0.0);
        self.picture.layout(cx, Constraints::tight(size.w, pic_h), style, st, i + Self::PICTURE);
        let btn_h = 32.0_f32.min(CONTROLS_H);
        self.play.layout(cx, Constraints::tight(PLAY_W, btn_h), style, st, i + Self::PLAY);
        let track_w = (size.w - 2.0 * GAP - PLAY_W - 2.0 * GAP - TIME_W).max(0.0);
        self.track.layout(cx, Constraints::tight(track_w, TRACK_H), style, st, i + Self::TRACK);
        record_painted(st, i, limits, size, &me)
    }

    fn place(&mut self, at: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        self.picture.place(at, style, st, i + Self::PICTURE);
        let row_y = at.y + st[i + Self::PICTURE].size.h;
        let btn_h = st[i + Self::PLAY].size.h;
        self.play.place(Point { x: at.x + GAP, y: row_y + (CONTROLS_H - btn_h) / 2.0 }, style, st, i + Self::PLAY);
        self.track.place(
            Point { x: at.x + GAP + PLAY_W + GAP, y: row_y + (CONTROLS_H - TRACK_H) / 2.0 },
            style,
            st,
            i + Self::TRACK,
        );
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let r = rect_of(&st[i]);
        lumen_static::ops::fill(st, i, out);
        out.rounded_rect(r, st[i].bg, st[i].radius);
        lumen_static::ops::border(st, i, out);
        self.picture.paint(st, i + Self::PICTURE, out);
        self.play.paint(st, i + Self::PLAY, out);
        self.track.paint(st, i + Self::TRACK, out);
        // The time, in its own box at the row's end, centred on the row.
        let px = st[i].font_size;
        let row_y = r[1] + st[i + Self::PICTURE].size.h;
        let x0 = r[2] - GAP - TIME_W;
        let y0 = row_y + (CONTROLS_H - px * 1.3) / 2.0;
        out.text_weighted([x0, y0, r[2] - GAP, y0 + px * 1.3], self.time.as_str(), px, st[i].font_weight, st[i].fg);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        if !contains(&st[i], x, y) {
            return None;
        }
        self.play
            .hit(st, i + Self::PLAY, x, y)
            .or_else(|| self.track.hit(st, i + Self::TRACK, x, y))
            .or_else(|| self.picture.hit(st, i + Self::PICTURE, x, y))
            .or(Some(Hit { node: i as u32, inner: None, local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y } }))
    }

    /// A reader's actions on the video itself: its default action toggles
    /// play, as the button would, and `SetValue` seeks, as the track would
    /// (M54/T54.4). Only an action **aimed at this node** — a press on the
    /// picture that nothing consumed bubbles here too, and must do nothing.
    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        if cx.target != cx.node {
            return false;
        }
        match ev {
            Event::Action { action: Action::Default, .. } => {
                self.pending = Some(((Self::PLAY) as u32, Action::Default));
                true
            }
            Event::Action { action: Action::SetValue(v), .. } => {
                self.pending = Some(((Self::TRACK) as u32, Action::SetValue(clamp01(*v))));
                true
            }
            _ => false,
        }
    }

    fn take_action(&mut self) -> Option<(u32, Action)> {
        self.pending.take()
    }

    fn node(&self, i: usize) -> &dyn Node {
        match i {
            0 => self,
            Self::PICTURE => &self.picture,
            Self::PLAY => &self.play,
            _ => &self.track,
        }
    }

    fn node_mut(&mut self, i: usize) -> &mut dyn Node {
        match i {
            0 => self,
            Self::PICTURE => &mut self.picture,
            Self::PLAY => &mut self.play,
            _ => &mut self.track,
        }
    }

    /// A video: its name, the time as its value, and the two actions it
    /// handles — toggle and seek (M54/T54.3, `docs/plan-images-in-motion.md`
    /// T84.8).
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        use lumen_static::observe::accesskit::{Action as Ak, Role};
        node.set_role(Role::Video);
        if !self.label.is_empty() {
            node.set_label(self.label);
        }
        node.set_value(self.time.as_str());
        node.add_action(Ak::Click);
        node.add_action(Ak::SetValue);
    }
}

impl<S: 'static> lumen_static::event::Handlers<S> for Video<S> {
    fn handler(&self, sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        match sel.i {
            Self::PLAY => Some(self.on_toggle),
            Self::TRACK => Some(self.on_seek),
            _ => None,
        }
    }
}
