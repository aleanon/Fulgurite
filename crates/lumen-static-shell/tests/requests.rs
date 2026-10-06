//! **M88/T88.3: the request seam (S1)** — `docs/plan-platform-requests.md`.
//!
//! A handler pushes a [`Request`] onto its state's [`Requests`]; the host
//! hands it out through `Host::take_requests`, and an answer comes back
//! through `Host::answer`, which converts it with `App::answered` and
//! applies the message **inside the call**. A headless host answers
//! nothing by itself, so these tests stand in for the window.
//!
//! | claim | bound |
//! |---|---|
//! | a click's `Screenshot { tag: 7 }` is taken once | 1, then 0 |
//! | `answer` applies inside the call | `shots` = 1 before any pump |
//! | an **unsplit** app under `Boundaries<256>` shows the answer | the label reads `x` |
//! | an unmodelled tag is counted | `unmodelled_answers` = 1 |
//! | a ninth push is refused | 8 queued, 1 refused |
//! | a clipboard write with no `clip_for` answers itself | `Refused(Empty)`, nothing taken |
//! | an HTML clip is handed out whole (M88/T88.4) | `Payload::Html`, both halves |
//! | an image clip keeps its shape (M88/T88.5) | `Payload::Image`, 8×4, 128 B |
//! | an app with no queue | `Vec::new()`, nothing counted |

use lumen_static::display::Cmd;
use lumen_static::event::{App, EventCtx};
use lumen_static::prelude::*;
use lumen_static::request::{Answer, Clip, ClipFormat, ClipKind, Refusal, Request, Requests};
use lumen_static_shell::request::{Outgoing, Payload};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

// ---- a split application: a button that asks for a screenshot --------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct ShotUi {
    #[widget_id("shoot")]
    shoot: Button<Asks>,
    #[widget_id("shown")]
    shown: Label<String>,
}

#[derive(Default)]
struct Asks {
    requests: Requests,
    shots: u32,
    last: String,
    /// What `clip_for` answers; `None` exercises `Refused(Empty)`.
    clip: Option<String>,
    /// When set, `clip_for` answers HTML (with its plain-text
    /// alternative) instead (M88/T88.4).
    html: Option<(String, String)>,
    refusals: Vec<Refusal>,
}

#[derive(Default)]
struct Shots {
    ui: ShotUi,
    state: Asks,
}

#[derive(Debug)]
enum Msg {
    Shot { w: u32, h: u32 },
    Text(String),
    Refused(Refusal),
}

impl App for Shots {
    type State = Asks;
    fn state_mut(&mut self) -> &mut Asks {
        &mut self.state
    }
    const STATE_IS_SPLIT: bool = true;
    type Tree = ShotUi;
    type Msg = Msg;
    fn tree(&self) -> &ShotUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut ShotUi {
        &mut self.ui
    }
    fn requests(&mut self) -> Option<&mut Requests> {
        Some(&mut self.state.requests)
    }
    fn clip_for(&mut self, _tag: u64) -> Option<Clip<'_>> {
        if let Some((html, alt)) = &self.state.html {
            return Some(Clip::Html { html, alt });
        }
        self.state.clip.as_deref().map(Clip::Text)
    }
    fn answered(tag: u64, a: Answer<'_>) -> Option<Msg> {
        match (tag, a) {
            (7, Answer::Image { width, height, .. }) => Some(Msg::Shot { w: width, h: height }),
            (7, Answer::Text(t)) => Some(Msg::Text(t.to_owned())),
            (_, Answer::Refused(r)) if tag < 99 => Some(Msg::Refused(r)),
            _ => None,
        }
    }
    fn apply(s: &mut Asks, m: Msg) -> bool {
        match m {
            Msg::Shot { w, h } => {
                s.shots += 1;
                s.last = format!("shot {} {w}x{h}", s.shots);
            }
            Msg::Text(t) => s.last = t,
            Msg::Refused(r) => s.refusals.push(r),
        }
        true
    }
    fn update(&mut self) -> bool {
        lumen_static::bind::set_if_changed(&mut self.ui.shown, &self.state.last)
    }
}

fn on_shoot(s: &mut Asks, _: &EventCtx) {
    s.requests.push(Request::Screenshot { tag: 7 });
}

fn shots_host() -> Host<Shots> {
    let mut app = Shots::default();
    app.ui.shoot.label = "shoot";
    app.ui.shoot.on_click = on_shoot;
    app.state.last = "none".into();
    let mut h: Host<Shots> = Host::new(app, 200, 80);
    h.pump();
    h
}

fn text<A: App>(h: &Host<A>) -> String {
    h.list
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Text { text, .. } => Some(h.list.str_of(*text).to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn click(h: &mut Host<Shots>, wid: &str) -> bool {
    let i = lumen_static::style::index_of_wid(<ShotUi as lumen_static::Widget>::SUB_TABLE, wid).unwrap();
    let s = h.states[i];
    h.click(s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.pump()
}

#[test]
fn a_click_pushes_one_screenshot_and_take_empties_it() {
    let mut h = shots_host();
    let handled = click(&mut h, "shoot");
    assert!(handled, "setup: the click on #shoot was not handled");
    let first = h.take_requests();
    let second = h.take_requests();
    println!("take after the click: {} (bound 1) {first:?}; second take: {} (bound 0)", first.len(), second.len());
    assert_eq!(first, vec![Outgoing { request: Request::Screenshot { tag: 7 }, payload: Payload::None }]);
    assert_eq!(second.len(), 0, "a second take returned the request again: take copies without clearing");
}

#[test]
fn an_answer_is_applied_inside_the_call_and_the_next_pump_shows_it() {
    let mut h = shots_host();
    assert!(click(&mut h, "shoot"), "setup: the click was not handled");
    assert_eq!(h.take_requests().len(), 1, "setup: the click asked for nothing");
    let px = [10u8, 20, 30, 255, 10, 20, 30, 255, 10, 20, 30, 255, 10, 20, 30, 255];
    h.answer(7, Answer::Image { width: 2, height: 2, scale: 1.0, rgba: &px });
    let shots = h.app.state.shots;
    let owed = (h.answer_changed(), h.more_deferred());
    println!("after answer(7, 2x2), before any pump: shots {shots} (want 1); answer_changed/more_deferred {owed:?}");
    assert_eq!(shots, 1, "the answer was not applied inside `answer`");
    assert_eq!(owed, (true, true), "an applied answer did not ask for the frame that shows it");
    h.pump();
    let shown = text(&h);
    println!("after one pump: {shown:?} (want it to contain \"shot 1 2x2\")");
    assert!(shown.contains("shot 1 2x2"), "the next pump does not show the answer: {shown:?}");
    assert!(!h.answer_changed(), "the pump did not take the answer's change flag");
}

#[test]
fn an_unmodelled_answer_is_counted() {
    let mut h = shots_host();
    assert_eq!(h.unmodelled_answers(), 0, "setup: nothing counted yet");
    h.answer(99, Answer::Written);
    println!("unmodelled_answers after answer(99): {} (want 1)", h.unmodelled_answers());
    assert_eq!(h.unmodelled_answers(), 1);
    assert_eq!(h.app.state.shots, 0, "an unmodelled answer applied something");
}

#[test]
fn a_ninth_push_is_refused_and_counted() {
    let mut h = shots_host();
    let pushed: Vec<bool> = (0..9).map(|i| h.app.state.requests.push(Request::Screenshot { tag: i })).collect();
    let refused = h.app.state.requests.refused();
    let taken = h.take_requests();
    println!("9 pushes: {pushed:?}; refused {refused} (want 1); taken {} (want {})", taken.len(), Requests::CAPACITY);
    assert_eq!(pushed.iter().filter(|&&p| !p).count(), 1, "the ninth push was not refused");
    assert_eq!(refused, 1);
    assert_eq!(taken.len(), Requests::CAPACITY);
    assert_eq!(taken.iter().map(|o| o.request.tag().unwrap()).collect::<Vec<_>>(), (0..8).collect::<Vec<_>>(), "oldest first");
}

#[test]
fn a_clipboard_write_asks_clip_for_and_refuses_empty_by_itself() {
    let mut h = shots_host();
    h.app.state.requests.push(Request::ClipboardWrite { tag: 5, kind: ClipKind::Clipboard });
    let none = h.take_requests();
    println!("write with clip_for None: taken {} (want 0), refusals {:?} (want [Empty])", none.len(), h.app.state.refusals);
    assert!(none.is_empty(), "a write with nothing to write was handed to the window");
    assert_eq!(h.app.state.refusals, vec![Refusal::Empty]);

    h.app.state.clip = Some("hello".into());
    h.app.state.requests.push(Request::ClipboardWrite { tag: 5, kind: ClipKind::Primary });
    h.app.state.requests.push(Request::ClipboardRead { tag: 6, kind: ClipKind::Clipboard, format: ClipFormat::Text });
    let some = h.take_requests();
    println!("write with clip_for Some(\"hello\"), then a read: {some:?}");
    assert_eq!(
        some,
        vec![
            Outgoing { request: Request::ClipboardWrite { tag: 5, kind: ClipKind::Primary }, payload: Payload::Text("hello".into()) },
            Outgoing {
                request: Request::ClipboardRead { tag: 6, kind: ClipKind::Clipboard, format: ClipFormat::Text },
                payload: Payload::None
            },
        ]
    );
}

/// **M88/T88.4: an HTML clip crosses as an owned `Payload::Html`**, both
/// halves, byte for byte — the window then puts it on the clipboard. A
/// platform clipboard cannot be exercised headless (the reason
/// `clipboard.rs` lives in the winit crate), so this pins only what the
/// shell builds from `clip_for`; `just tasks-drive` round-trips it
/// through the real one.
#[test]
fn an_html_clip_is_handed_out_whole() {
    let mut h = shots_host();
    let (html, alt) = ("<p>caf\u{e9} <b>&amp;</b></p>", "caf\u{e9} &");
    h.app.state.html = Some((html.into(), alt.into()));
    h.app.state.requests.push(Request::ClipboardWrite { tag: 9, kind: ClipKind::Clipboard });
    let got = h.take_requests();
    println!("an HTML write: {got:?} (html {} B, alt {} B)", html.len(), alt.len());
    assert_eq!(
        got,
        vec![Outgoing {
            request: Request::ClipboardWrite { tag: 9, kind: ClipKind::Clipboard },
            payload: Payload::Html { html: html.into(), alt: alt.into() },
        }],
        "an HTML clip must reach the window as Payload::Html with both halves"
    );
    assert!(h.app.state.refusals.is_empty(), "setup: the write was refused {:?}", h.app.state.refusals);
}

/// **M88/T88.5: an image clip crosses as an owned `Payload::Image`**, its
/// width and height in their own fields and its bytes copied whole, so
/// the window hands arboard what the application wrote.
#[test]
fn an_image_clip_is_owned_with_its_shape() {
    let rgba: Vec<u8> = (0..8 * 4 * 4).map(|i| i as u8).collect();
    let p = Payload::from(Clip::Image { width: 8, height: 4, rgba: &rgba });
    println!("an 8x4 image clip -> {:?}", match &p { Payload::Image { width, height, rgba } => format!("Image {width}x{height}, {} B", rgba.len()), other => format!("{other:?}") });
    assert_eq!(p, Payload::Image { width: 8, height: 4, rgba: rgba.clone() }, "an image clip must keep its width, height and bytes");
}

#[test]
fn an_application_with_no_queue_takes_nothing() {
    // `LabelUi`'s app below answers the default `requests() = None`.
    let mut h: Host<Unsplit> = Host::new(Unsplit::default(), 100, 40);
    h.pump();
    let taken = h.take_requests();
    println!("no queue: taken {} (want 0), capacity {} (want 0), unmodelled {}", taken.len(), taken.capacity(), h.unmodelled_answers());
    assert!(taken.is_empty() && taken.capacity() == 0, "an empty take allocated");
    assert_eq!(h.unmodelled_answers(), 0);
}

// ---- an unsplit application sees its answer (second review A2) ---------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct LabelUi {
    value: Label<String>,
}

/// `STATE_IS_SPLIT = false` (the hand-written default), and `apply`
/// writes the label straight into the tree — so nothing marks it but the
/// host's `mark_all` on an applied change, which for an answer is the
/// `answer_changed` flag the next pump takes.
#[derive(Default)]
struct Unsplit {
    ui: LabelUi,
}

impl App for Unsplit {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = LabelUi;
    type Msg = String;
    fn tree(&self) -> &LabelUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut LabelUi {
        &mut self.ui
    }
    fn answered(tag: u64, a: Answer<'_>) -> Option<String> {
        match (tag, a) {
            (7, Answer::Text(t)) => Some(t.to_owned()),
            _ => None,
        }
    }
    fn apply(this: &mut Self, m: String) -> bool {
        this.ui.value.set(m);
        true
    }
}

#[test]
fn an_unsplit_application_sees_its_answer_under_boundaries() {
    let mut app = Unsplit::default();
    app.ui.value.set("0".into());
    // `Host<Unsplit>`: `Incremental` retention, `Boundaries<256>`.
    let mut h: Host<Unsplit> = Host::new(app, 200, 40);
    h.pump();
    h.pump();
    let before = text(&h);
    assert_eq!(before, "0", "setup: the label reads 0 before the answer");
    h.answer(7, Answer::Text("x"));
    h.pump();
    let after = text(&h);
    println!("unsplit Boundaries<256>: label {before:?} -> {after:?} (want \"x\")");
    assert_eq!(after, "x", "the frame shows a stale label: the answer's change never reached mark_all (the idle skip)");
}
