//! **M28/T28.12 — use the thing.**
//!
//! `AGENT.md`, after M24: an end-to-end assembly is the only thing that
//! can find a capability which is present, correct in isolation, and
//! unreachable. Every one of M28's pieces has a passing test; none of
//! them had ever been used together, by an application, through a real
//! window.
//!
//! This application does all five: **spawn**, **batch**, **chain**,
//! **cancel** and **subscribe** — and, since M88/T88.1, a sixth: a
//! **sipper**, one job streaming 10,000 values through
//! `Sender::progress`. Since M88/T88.2 a 16×16 **swatch** sits at the
//! origin, and `#swatch` recolours it: the one control whose click
//! changes pixels, so `just tasks-drive`'s GPU leg can prove that
//! `ui.screenshot` reads the GPU back per call. Since M88/T88.3 it also
//! **asks the window for a screenshot** (`lumen_static::request`): `#shot`
//! recolours the swatch and pushes a `Screenshot` request from its
//! handler, `#shot-timer` does it from a timer tick, and `answered`
//! shows the pixel the answer holds at the swatch. Since M88/T88.4 it
//! **reaches the clipboard** through the same queue: `#copy-text`,
//! `#copy-primary` and `#copy-html` push `ClipboardWrite`s whose bytes
//! `clip_for` hands over at the drain, and `#read-text`, `#read-primary`
//! and `#read-html` push `ClipboardRead`s whose answers the two `#clip-*`
//! labels show. Every one is driven by a click, and
//! every one puts its consequence on screen where
//! `scripts/tasks_drive.sh` can read it back over the agent endpoint.
//!
//! Run it: `cargo run -p tasks-demo`, or `just tasks-drive` to have the
//! script click every button and check what happened.

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::spawn::{Descriptor, Failure, JobFn, Outbox, Subs, Timers};
use lumen_static::style::{Edges, Length, Patch, Rule, Selector};
use lumen_static::Color;
use lumen_static_winit::{application, Program, Run};

// ---- the job table. Stable, append-only ids (plan-tasks.md §2.1) -----

const KIND_ONE: u64 = 1;
const KIND_BATCH: u64 = 2;
const KIND_CHAIN: u64 = 3;
const KIND_SLOW: u64 = 4;
const KIND_SUB: u64 = 5;
const KIND_SIP: u64 = 6;
const KIND_SHOT: u64 = 7;

/// The tags this application's screenshot requests carry (M88/T88.3).
const TAG_SHOT: u64 = 1;
const TAG_SHOT_TIMER: u64 = 2;
/// The tags of its clipboard requests (M88/T88.4).
const TAG_COPY_TEXT: u64 = 3;
const TAG_COPY_PRIMARY: u64 = 4;
const TAG_COPY_HTML: u64 = 5;
const TAG_READ_TEXT: u64 = 6;
const TAG_READ_PRIMARY: u64 = 7;
const TAG_READ_HTML: u64 = 8;

/// What `#copy-html` writes, and the plain text a reader that cannot take
/// HTML gets. Markup, an entity, and a two-byte character, so a
/// round trip that re-encoded or trimmed anything would differ
/// (`just tasks-drive` compares it byte for byte).
const COPY_HTML: &str = "<p>caf\u{e9} <b>&amp;</b> <i>lumen</i></p>";
const COPY_HTML_ALT: &str = "caf\u{e9} & lumen";
/// What `#copy-primary` puts on the primary selection.
const COPY_PRIMARY: &str = "B";

/// A tag's name in the `#clip-*` labels.
fn clip_name(tag: u64) -> &'static str {
    match tag {
        TAG_COPY_TEXT | TAG_READ_TEXT => "text",
        TAG_COPY_PRIMARY | TAG_READ_PRIMARY => "primary",
        _ => "html",
    }
}

/// A message is data. Nothing here is a callback, and nothing crossing
/// the frame boundary holds a code reference.
#[derive(Clone, Debug)]
enum Msg {
    One(u64),
    Batch(u64),
    Chain { step: u64, value: u64 },
    Slow(u64),
    Tick(u64),
    /// One value of the sipper's stream (M88/T88.1), reported through
    /// `Sender::progress`.
    Sip(u64),
    /// A tick of `#shot-timer`'s timer (M88/T88.3).
    ShotTick,
    /// A screenshot's answer, as `answered` read it: its size and the
    /// pixel at physical (4, 4), inside the swatch.
    Shot { w: u32, h: u32, rgb: [u8; 3] },
    /// A screenshot the window refused.
    ShotRefused,
    /// A clipboard request's answer (M88/T88.4): a write's `Written` or
    /// refusal, or what a read found — owned, because the answer's text
    /// is lent for the `answered` call only.
    Clip { tag: u64, said: String },
    Failed(u64, Failure),
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    /// At the origin: physical (0, 0)..(16, 16) at scale 1.
    #[widget_id("swatch-box")]
    swatch_box: Swatch,
    #[widget_id("title")]
    title: Label<String>,
    #[widget_id("spawn-one")]
    spawn_one: Button<Tasks>,
    #[widget_id("spawn-batch")]
    spawn_batch: Button<Tasks>,
    #[widget_id("start-chain")]
    start_chain: Button<Tasks>,
    #[widget_id("start-slow")]
    start_slow: Button<Tasks>,
    #[widget_id("cancel")]
    cancel: Button<Tasks>,
    #[widget_id("toggle-sub")]
    toggle_sub: Button<Tasks>,
    #[widget_id("sip")]
    sip: Button<Tasks>,
    #[widget_id("swatch")]
    swatch: Button<Tasks>,
    #[widget_id("shot")]
    shot: Button<Tasks>,
    #[widget_id("shot-timer")]
    shot_timer: Button<Tasks>,
    #[widget_id("shot-value")]
    shot_value: Label<String>,
    #[widget_id("shot-log")]
    shot_log: Label<String>,
    copies: CopyRow,
    reads: ReadRow,
    #[widget_id("clip-write")]
    clip_write: Label<String>,
    #[widget_id("clip-read")]
    clip_read: Label<String>,
    #[widget_id("sip-value")]
    sip_value: Label<String>,
    #[widget_id("status")]
    status: Label<String>,
    #[widget_id("counts")]
    counts: Label<String>,
}

/// M88/T88.4: the clipboard writes, one row.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct CopyRow {
    #[widget_id("copy-text")]
    copy_text: Button<Tasks>,
    #[widget_id("copy-primary")]
    copy_primary: Button<Tasks>,
    #[widget_id("copy-html")]
    copy_html: Button<Tasks>,
}

/// M88/T88.4: the clipboard reads, one row.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct ReadRow {
    #[widget_id("read-text")]
    read_text: Button<Tasks>,
    #[widget_id("read-primary")]
    read_primary: Button<Tasks>,
    #[widget_id("read-html")]
    read_html: Button<Tasks>,
}

/// The tree, and the state its handlers may touch — split in
/// M33/T33.4. Hand-written rather than derived because the derive
/// cannot express a real `Msg`, which is why `STATE_IS_SPLIT` is set by
/// hand below: a promise nothing checks, which M33 §2.2 names.
#[derive(Default)]
struct Demo {
    ui: Ui,
    state: Tasks,
}

/// What a handler and `apply` may touch. The tree is not in here.
#[derive(Default)]
struct Tasks {
    outbox: Outbox,
    subs: Subs,
    timers: Timers,

    /// What the last completed thing was — read by the drive script.
    last: String,
    /// A cancel asked for by a handler, acted on by the next frame —
    /// a handler reaches the app and nothing else, so this is how the
    /// request travels, exactly like a descriptor in the outbox.
    cancel_requested: bool,
    /// How many results of each kind have landed. The drive script
    /// asserts on these, because "something changed" is not a claim.
    one: u32,
    batch: u32,
    chain_steps: u32,
    slow: u32,
    ticks: u32,
    failures: u32,
    sub_on: bool,
    /// The sipper's latest value shown, `None` before the first.
    sip: Option<u64>,
    /// How many times the swatch was recoloured — by `#swatch`, `#shot`
    /// and `#shot-timer`'s first tick; its colour is derived from it
    /// ([`swatch_color`]).
    swatch_clicks: u32,
    /// The platform requests out to the window (M88/T88.3).
    requests: lumen_static::request::Requests,
    /// `#shot-timer`'s ticks since it was armed.
    shot_ticks: u32,
    /// Screenshot answers applied, and what the last one showed:
    /// `shot N WxH #rrggbb`, `shot pending` before the first.
    shots: u32,
    shot_value: String,
    /// Every answer's swatch colour, oldest first: the drive reads which
    /// frame each answer came from.
    shot_log: String,
    /// What `#copy-text` writes: `A` on its first click, then `C`, `E`, …
    /// — two letters on per click, so its text never meets `#copy-primary`'s
    /// `B` (M88/T88.4).
    copy_text: String,
    copy_clicks: u32,
    /// Clipboard answers applied, writes and reads, and what the last of
    /// each said: `write N text Written`, `read N primary B`.
    writes: u32,
    reads: u32,
    clip_write: String,
    clip_read: String,
}

// ---- the swatch (M88/T88.2) ------------------------------------------

/// A distinct opaque colour per count, so consecutive clicks never
/// repaint the same pixels.
fn swatch_color(n: u32) -> Color {
    Color(
        (40 + n.wrapping_mul(67)) as u8,
        (90 + n.wrapping_mul(131)) as u8,
        (160 + n.wrapping_mul(29)) as u8,
        255,
    )
}

/// **A 16×16 square of one colour**, written by [`Demo::update`] from the
/// click count. No widget in the set paints a colour its application
/// chooses at run time (`Fixed` paints its style's), so the demo owns
/// this one: a rigid leaf, never a click target.
struct Swatch {
    color: Color,
}

impl Default for Swatch {
    fn default() -> Self {
        Swatch { color: swatch_color(0) }
    }
}

impl lumen_static::widget::Widget for Swatch {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [lumen_static::desc::NodeDesc] = &[lumen_static::desc::NodeDesc::new(
        0,
        lumen_static::desc::NO_PARENT,
        0,
        0,
        lumen_static::desc::NodeDesc::RIGID,
    )];
    const KIND: &'static str = "swatch";
    const SEEDS: &'static [lumen_static::style::Seed] = &[lumen_static::style::Seed::new(
        Self::KIND,
        Self::type_defaults,
        lumen_static::style::Fields::of(&[lumen_static::style::Fields::VISIBLE]),
    )];
}

impl lumen_static::widget::Node for Swatch {
    fn layout(
        &mut self,
        _cx: &mut lumen_static::cx::LayoutCx,
        limits: lumen_static::desc::Constraints,
        style: &[lumen_static::Style],
        st: &mut [lumen_static::NodeState],
        i: usize,
    ) -> lumen_static::desc::Size {
        let content = lumen_static::desc::Size { w: 16.0, h: 16.0 };
        lumen_static::ops::record_painted(
            st,
            i,
            limits,
            lumen_static::ops::resolve_self(limits, &style[i], content),
            &style[i],
        )
    }
    fn paint(&self, st: &[lumen_static::NodeState], i: usize, out: &mut dyn lumen_static::display::Painter) {
        out.rect(lumen_static::ops::rect_of(&st[i]), self.color);
    }
    lumen_static::leaf_common!(Swatch, lumen_static::desc::NodeDesc::RIGID, nohit);

    fn observe(
        &self,
        _st: &[lumen_static::NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        node.set_role(lumen_static::observe::accesskit::Role::Image);
        node.set_label("swatch");
    }
}

impl<S> lumen_static::event::Handlers<S> for Swatch {
    fn handler(&self, _sel: lumen_static::event::NodeSel) -> Option<fn(&mut S, &lumen_static::EventCtx)> {
        None
    }
}

// ---- job bodies. Non-capturing `fn` items, by type ------------------

fn one(d: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    let _ = tx.send(Msg::One(d.payload));
}
fn batch(d: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    let _ = tx.send(Msg::Batch(d.payload));
}
fn chain(d: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    let _ = tx.send(Msg::Chain { step: d.step, value: d.payload * 2 });
}
/// Long enough that a cancel can land while it is still running — which
/// is the only way to demonstrate what cancel actually does.
fn slow(d: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    std::thread::sleep(std::time::Duration::from_millis(400));
    let _ = tx.send(Msg::Slow(d.payload));
}
fn tick(d: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    let _ = tx.send(Msg::Tick(d.payload));
}
fn shot_tick(_: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    let _ = tx.send(Msg::ShotTick);
}

// ANCHOR: sip
/// **A sipper** — M88/T88.1. Ten thousand values, each reported with
/// `progress`, which replaces the job's one-value slot instead of
/// queueing: it never blocks and is never refused, the window applies
/// the newest value at most once a frame, and the last one is always
/// applied. Through `send` this stream lost 9,744 values to the bounded
/// inbox, its last one among them (`docs/results-tasks.md`).
fn sip(d: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    for i in 0..d.payload {
        tx.progress(Msg::Sip(i));
    }
}
// ANCHOR_END: sip

impl App for Demo {
    type State = Tasks;
    fn state_mut(&mut self) -> &mut Tasks {
        &mut self.state
    }
    const STATE_IS_SPLIT: bool = true;
    type Tree = Ui;
    type Msg = Msg;

    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }

    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.state.outbox)
    }
    fn subscriptions(&mut self) -> Option<&mut Subs> {
        Some(&mut self.state.subs)
    }
    fn timers(&mut self) -> Option<&mut Timers> {
        Some(&mut self.state.timers)
    }
    fn requests(&mut self) -> Option<&mut lumen_static::request::Requests> {
        Some(&mut self.state.requests)
    }

    // ANCHOR: shot
    /// **A screenshot's answer, in the build loaded now** — M88/T88.3.
    /// The window answered `Screenshot` from what its renderer drew; this
    /// reads the pixel at physical (4, 4), inside the 16×16 swatch, and
    /// the message is applied inside the same call (`Host::answer`), so
    /// nothing converted waits for a frame a swap could pre-empt.
    fn answered(tag: u64, a: lumen_static::request::Answer<'_>) -> Option<Msg> {
        use lumen_static::request::Answer;
        match (tag, a) {
            (TAG_SHOT | TAG_SHOT_TIMER, Answer::Image { width, height, rgba, .. }) => {
                let i = ((4 * width + 4) * 4) as usize;
                let px = rgba.get(i..i + 3)?;
                Some(Msg::Shot { w: width, h: height, rgb: [px[0], px[1], px[2]] })
            }
            (TAG_SHOT | TAG_SHOT_TIMER, Answer::Refused(_)) => Some(Msg::ShotRefused),
            (TAG_COPY_TEXT..=TAG_READ_HTML, a) => Some(Msg::Clip {
                tag,
                said: match a {
                    Answer::Text(t) | Answer::Html(t) => t.to_owned(),
                    other => format!("{other:?}"),
                },
            }),
            _ => None,
        }
    }
    // ANCHOR_END: shot

    // ANCHOR: clip
    /// **What a clipboard write puts there**, asked by the host while it
    /// drains the request — synchronously, in the build loaded now — so
    /// the bytes never live in the request (M88/T88.4).
    fn clip_for(&mut self, tag: u64) -> Option<lumen_static::request::Clip<'_>> {
        use lumen_static::request::Clip;
        match tag {
            TAG_COPY_TEXT => Some(Clip::Text(&self.state.copy_text)),
            TAG_COPY_PRIMARY => Some(Clip::Text(COPY_PRIMARY)),
            TAG_COPY_HTML => Some(Clip::Html { html: COPY_HTML, alt: COPY_HTML_ALT }),
            _ => None,
        }
    }
    // ANCHOR_END: clip

    fn job_for(d: Descriptor) -> Option<JobFn<Msg>> {
        match (d.kind, d.step) {
            (KIND_ONE, 0) => Some(one),
            (KIND_BATCH, 0) => Some(batch),
            // The chain's three steps share a body and differ by index —
            // that is the point of §2.1: the step is data.
            (KIND_CHAIN, 0..=2) => Some(chain),
            (KIND_SLOW, 0) => Some(slow),
            (KIND_SUB, 0) => Some(tick),
            (KIND_SIP, 0) => Some(sip),
            (KIND_SHOT, 0) => Some(shot_tick),
            _ => None,
        }
    }

    fn take_cancel(&mut self) -> bool {
        core::mem::take(&mut self.state.cancel_requested)
    }

    fn failed(d: Descriptor, why: Failure) -> Option<Msg> {
        Some(Msg::Failed(d.kind, why))
    }

    /// The step function, in the currently loaded build.
    ///
    /// Receives the state (M33/T33.2), so it cannot write the labels: the
    /// binding step below does, from what this changed.
    fn apply(this: &mut Tasks, m: Msg) -> bool {
        match m {
            Msg::One(v) => {
                this.one += 1;
                this.last = format!("one={v}");
            }
            Msg::Batch(v) => {
                this.batch += 1;
                this.last = format!("batch={v}");
            }
            Msg::Chain { step, value } => {
                this.chain_steps += 1;
                this.last = format!("chain step={step} value={value}");
                // What happens next is data: another descriptor, or
                // nothing. "Nothing" is the absence of a push.
                if step < 2 {
                    this.outbox.push(Descriptor {
                        kind: KIND_CHAIN,
                        step: step + 1,
                        payload: value,
                    });
                }
            }
            Msg::Slow(v) => {
                this.slow += 1;
                this.last = format!("slow={v}");
            }
            Msg::Tick(k) => {
                this.ticks += 1;
                this.last = format!("tick={k}");
            }
            Msg::Sip(v) => this.sip = Some(v),
            // The first tick recolours the swatch and asks for a
            // screenshot; the second asks again and changes nothing, so its
            // frame has no damage and presents nothing — the request is
            // answered from the frame still on screen — then stops.
            Msg::ShotTick => {
                this.shot_ticks += 1;
                // A tick already in flight when the second cancelled the
                // timer — a window whose presents are slow — asks nothing.
                if this.shot_ticks > 2 {
                    return false;
                }
                this.requests.push(lumen_static::request::Request::Screenshot { tag: TAG_SHOT_TIMER });
                if this.shot_ticks == 1 {
                    this.swatch_clicks += 1;
                } else {
                    this.timers.cancel(KIND_SHOT, 1);
                    return false;
                }
            }
            Msg::Shot { w, h, rgb } => {
                this.shots += 1;
                let c = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
                this.shot_value = format!("shot {} {w}x{h} {c}", this.shots);
                this.shot_log.push(' ');
                this.shot_log.push_str(&c);
            }
            Msg::ShotRefused => {
                this.shots += 1;
                this.shot_value = format!("shot {} refused", this.shots);
            }
            Msg::Clip { tag, said } if tag <= TAG_COPY_HTML => {
                this.writes += 1;
                this.clip_write = format!("write {} {} {said}", this.writes, clip_name(tag));
            }
            Msg::Clip { tag, said } => {
                this.reads += 1;
                this.clip_read = format!("read {} {} {said}", this.reads, clip_name(tag));
            }
            Msg::Failed(kind, why) => {
                this.failures += 1;
                this.last = format!("failed kind={kind} why={why:?}");
            }
        }
        true
    }

    /// **The binding step, hand-written** because this impl is.
    ///
    /// The labels used to be written by `apply` and two handlers calling
    /// a `refresh()`, so that the drive script's "the consequence landed"
    /// assertion was about `apply`. After the split neither can reach the
    /// tree: `apply` changes the state and this renders it, so the drive
    /// checks the same consequence, reached through the path that can be
    /// narrowed. `format_into` compares before it writes, so this reports
    /// only what changed.
    fn update(&mut self) -> bool {
        let s = &self.state;
        let a = lumen_static::bind::format_into(&mut self.ui.status, format_args!("last: {}", s.last));
        let b = lumen_static::bind::format_into(
            &mut self.ui.counts,
            format_args!(
                "one={} batch={} chain={} slow={} ticks={} failed={} sub={}",
                s.one,
                s.batch,
                s.chain_steps,
                s.slow,
                s.ticks,
                s.failures,
                if s.sub_on { "on" } else { "off" },
            ),
        );
        let c = match s.sip {
            Some(v) => lumen_static::bind::format_into(&mut self.ui.sip_value, format_args!("sip {v}")),
            None => lumen_static::bind::format_into(&mut self.ui.sip_value, format_args!("sip -")),
        };
        let want = swatch_color(s.swatch_clicks);
        let d = self.ui.swatch_box.color != want;
        self.ui.swatch_box.color = want;
        let e = match s.shots {
            0 => lumen_static::bind::format_into(&mut self.ui.shot_value, format_args!("shot pending")),
            _ => lumen_static::bind::set_if_changed(&mut self.ui.shot_value, &s.shot_value),
        };
        let f = lumen_static::bind::format_into(&mut self.ui.shot_log, format_args!("log{}", s.shot_log));
        let g = match s.writes {
            0 => lumen_static::bind::format_into(&mut self.ui.clip_write, format_args!("write -")),
            _ => lumen_static::bind::set_if_changed(&mut self.ui.clip_write, &s.clip_write),
        };
        let h = match s.reads {
            0 => lumen_static::bind::format_into(&mut self.ui.clip_read, format_args!("read -")),
            _ => lumen_static::bind::set_if_changed(&mut self.ui.clip_read, &s.clip_read),
        };
        a | b | c | d | e | f | g | h
    }
}

fn on_one(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.outbox.push(Descriptor::new(KIND_ONE, 7));
}

/// A batch: several descriptors pushed together. There is no batch type.
fn on_batch(s: &mut Tasks, _: &lumen_static::EventCtx) {
    for i in 1..=3 {
        s.outbox.push(Descriptor::new(KIND_BATCH, i));
    }
}

fn on_chain(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.outbox.push(Descriptor::new(KIND_CHAIN, 1));
}

fn on_slow(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.outbox.push(Descriptor::new(KIND_SLOW, 99));
}

/// Cancel is a *request*, pushed like any other — the handler cannot
/// reach the host, and `EventCtx` carries no inbox. The host's
/// `pump` sees the flag on the next frame.
fn on_cancel(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.cancel_requested = true;
    s.last = "cancel requested".into();
}

fn on_sip(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.outbox.push(Descriptor::new(KIND_SIP, 10_000));
}

/// M88/T88.2: recolour the swatch. The binding step paints it.
fn on_swatch(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.swatch_clicks += 1;
}

/// M88/T88.3: recolour the swatch and ask the window for a screenshot.
/// The click arrives through the agent in a drive, so the window answers
/// it after that call, from the frame the call presented.
fn on_shot(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.swatch_clicks += 1;
    s.requests.push(lumen_static::request::Request::Screenshot { tag: TAG_SHOT });
}

/// M88/T88.3: arm a 500 ms timer whose ticks ask for screenshots — so the
/// window answers from its redraw, with no agent call in flight.
fn on_shot_timer(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.shot_ticks = 0;
    s.timers.cancel(KIND_SHOT, 1);
    s.timers.every(KIND_SHOT, 1, 500.0);
}

// ---- the clipboard (M88/T88.4): each button pushes one request ------

fn on_copy_text(s: &mut Tasks, _: &lumen_static::EventCtx) {
    use lumen_static::request::{ClipKind, Request};
    s.copy_clicks += 1;
    s.copy_text = char::from(b'A' + ((2 * (s.copy_clicks - 1)) % 26) as u8).to_string();
    s.requests.push(Request::ClipboardWrite { tag: TAG_COPY_TEXT, kind: ClipKind::Clipboard });
}
fn on_copy_primary(s: &mut Tasks, _: &lumen_static::EventCtx) {
    use lumen_static::request::{ClipKind, Request};
    s.requests.push(Request::ClipboardWrite { tag: TAG_COPY_PRIMARY, kind: ClipKind::Primary });
}
fn on_copy_html(s: &mut Tasks, _: &lumen_static::EventCtx) {
    use lumen_static::request::{ClipKind, Request};
    s.requests.push(Request::ClipboardWrite { tag: TAG_COPY_HTML, kind: ClipKind::Clipboard });
}
fn on_read_text(s: &mut Tasks, _: &lumen_static::EventCtx) {
    use lumen_static::request::{ClipFormat, ClipKind, Request};
    s.requests.push(Request::ClipboardRead { tag: TAG_READ_TEXT, kind: ClipKind::Clipboard, format: ClipFormat::Text });
}
fn on_read_primary(s: &mut Tasks, _: &lumen_static::EventCtx) {
    use lumen_static::request::{ClipFormat, ClipKind, Request};
    s.requests.push(Request::ClipboardRead { tag: TAG_READ_PRIMARY, kind: ClipKind::Primary, format: ClipFormat::Text });
}
fn on_read_html(s: &mut Tasks, _: &lumen_static::EventCtx) {
    use lumen_static::request::{ClipFormat, ClipKind, Request};
    s.requests.push(Request::ClipboardRead { tag: TAG_READ_HTML, kind: ClipKind::Clipboard, format: ClipFormat::Html });
}

fn on_toggle_sub(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.sub_on = !s.sub_on;
    s.subs.clear();
    s.timers.cancel(KIND_SUB, 1);
    if s.sub_on {
        s.subs.declare(KIND_SUB, 1);
        // A subscription that keeps producing is a timer plus a stream:
        // the declare starts it once, the timer keeps it going.
        s.timers.every(KIND_SUB, 1, 120.0);
    }
    s.last = format!("subscription {}", if s.sub_on { "on" } else { "off" });
}

/// The application, built once — the boot of the program below.
fn build() -> Demo {
    let mut app = Demo::default();
    app.ui.title.set("M28: spawn, batch, chain, cancel, subscribe".into());
    app.ui.spawn_one.label = "spawn one";
    app.ui.spawn_one.on_click = on_one;
    app.ui.spawn_batch.label = "spawn batch";
    app.ui.spawn_batch.on_click = on_batch;
    app.ui.start_chain.label = "start chain";
    app.ui.start_chain.on_click = on_chain;
    app.ui.start_slow.label = "start slow";
    app.ui.start_slow.on_click = on_slow;
    app.ui.cancel.label = "cancel";
    app.ui.cancel.on_click = on_cancel;
    app.ui.toggle_sub.label = "toggle subscription";
    app.ui.toggle_sub.on_click = on_toggle_sub;
    app.ui.sip.label = "start sipper";
    app.ui.sip.on_click = on_sip;
    app.ui.swatch.label = "recolour swatch";
    app.ui.swatch.on_click = on_swatch;
    app.ui.shot.label = "screenshot";
    app.ui.shot.on_click = on_shot;
    app.ui.shot_timer.label = "screenshot by timer";
    app.ui.shot_timer.on_click = on_shot_timer;
    app.ui.copies.copy_text.label = "copy text";
    app.ui.copies.copy_text.on_click = on_copy_text;
    app.ui.copies.copy_primary.label = "copy primary";
    app.ui.copies.copy_primary.on_click = on_copy_primary;
    app.ui.copies.copy_html.label = "copy html";
    app.ui.copies.copy_html.on_click = on_copy_html;
    app.ui.reads.read_text.label = "read text";
    app.ui.reads.read_text.on_click = on_read_text;
    app.ui.reads.read_primary.label = "read primary";
    app.ui.reads.read_primary.on_click = on_read_primary;
    app.ui.reads.read_html.label = "read html";
    app.ui.reads.read_html.on_click = on_read_html;
    app
}

/// The sheet. **Kinds are lowercase** (`button`, `label`): this said
/// `"Button"` and `"Label"` until M32/T32.1's migration found that
/// neither rule had ever matched anything — the demo had been unstyled
/// since its sheet was written. The test below asserts the sheet lint
/// finds nothing, which would have caught it on the day.
fn stylesheet() -> Vec<Rule> {
    vec![
        Rule::new(
            Selector::Kind("button"),
            Patch::new()
                .background(Color(226, 232, 240, 255))
                .padding(Edges::all(8.0))
                .radius(6.0)
                .height(Length::Fixed(32.0)),
        ),
        Rule::new(Selector::Kind("label"), Patch::new().font_size(14.0)),
    ]
}

/// **The program, described once** — M32 (`docs/plan-program.md`). The
/// window and `--shot` are both built from it.
fn program() -> Program<Demo> {
    application(build).title("tasks-demo").size(520, 760).style(|_: &Demo| stylesheet())
}

fn main() {
    // The endpoint's address is `LUMEN_AGENT_ADDR`, read by the window
    // (M48/T48.11); unset, this window serves none.
    // M55/T55.4: the strategy is this application's choice, made here.
    #[cfg(feature = "everything")]
    program().invalidation::<lumen_static::Everything>().run();
    #[cfg(not(feature = "everything"))]
    program().run();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M25/T25.2's lint over this program's own sheet: every rule matches
    /// a node and changes something.
    #[test]
    fn every_stylesheet_rule_changes_something() {
        let host = program().host::<lumen_static::retain::Incremental, lumen_static::Everything>();
        let sheet = host.stylesheet();
        let found: Vec<String> = host
            .sheet_findings()
            .iter()
            .map(|f| format!("rule {} {:?}: {:?}", f.rule, sheet[f.rule].selector, f.issue))
            .collect();
        println!("{} rules, {} findings", sheet.len(), found.len());
        assert!(!sheet.is_empty(), "setup: the program installed its sheet");
        assert!(found.is_empty(), "stylesheet rules that change nothing:\n{}", found.join("\n"));
    }
}
