//! **The stepped oracle for M86's transitions** — T86.0
//! (`docs/plan-transitions.md`).
//!
//! One script, two hosts in one process: a `Host<_, Incremental,
//! Everything>` and a `Host<_, Incremental, Boundaries<256>>`, each on
//! its own tree of a `Button`, a `Switch` and a `TextInput`. The script
//! hovers `#go`, presses it, releases it, hovers the switch, then Tabs to
//! the input; every move is followed by twelve frames of `tick(16.0)` and
//! `pump()`. **On every frame** the two `NodeState` arrays must agree
//! field for field, and the two display lists' `hash_from(0)` must be
//! equal. A divergence names the step, the frame, the node and the field.
//!
//! It was written before anything in M86 animates, deliberately — the
//! shape `invalidation_oracle.rs` took for T16.7. Today a pointer bit
//! changes the shade in one frame and the ticks change nothing, so the
//! harness agrees trivially except where T25.0's marks matter; T86.1–T86.3
//! are the commits that can break it, and they find it standing.
//!
//! **The setup asserts that it happened** (AGENT.md, "the setup silently
//! did not happen"): after each move the intended node is the one in
//! `input.hovered`, `input.pressed` or `input.focus`, printed beside its
//! selector, and after the Tab `focus_visible` is `true`. Two hosts
//! agreeing about a pointer that landed nowhere would be agreement about
//! nothing.

use lumen_static::event::{Key, PointerButton};
use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Event, Incremental, NodeState, StaticTree};
use lumen_static_shell::Host;
use lumen_static_widgets::{Button, Switch, TextInput};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
struct Ui {
    #[widget_id("go")]
    go: Button<App>,
    /// Before the switch in Tab order, so one Tab from the button a
    /// press focused lands on the field: the script's fifth move is one
    /// event, and the frame count stays 5 x 13.
    #[widget_id("field")]
    field: TextInput,
    #[widget_id("sw")]
    sw: Switch<App>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    clicks: u32,
}

const W: u32 = 320;
const H: u32 = 240;

/// Ticks after each move, and the moves in the script.
const TICKS: usize = 12;
const STEPS: usize = 5;

fn sheet() -> Vec<Rule> {
    vec![
        Rule::new(Selector::Kind("Ui"), Patch::new().padding(lumen_static::style::Edges::all(16.0))),
        Rule::new(Selector::Id("field"), Patch::new().width(Length::Fixed(200.0))),
    ]
}

fn index(wid: &str) -> u32 {
    lumen_static::style::index_of_wid(<Ui as lumen_static::Widget>::SUB_TABLE, wid).expect("id exists") as u32
}

/// A recorded frame: the state array, the list's hash, its length.
type Frame = (Vec<NodeState>, u64, usize);

/// What each move was for, and the node it had to land on.
#[derive(Clone, Copy)]
enum Want {
    Hovered(&'static str),
    Pressed(&'static str),
    Released(&'static str),
    Focused(&'static str),
}

/// The script, run on one host. Returns every frame, in order, labelled.
fn drive<I: Invalidation>(name: &str) -> Vec<(String, Frame)> {
    let mut app = App::default();
    app.ui.go.label = "Go";
    app.ui.go.on_click = |a: &mut App, _| a.clicks += 1;
    app.ui.sw.on_toggle = |_, _| {};
    let mut h: Host<App, Incremental, I> = Host::new(app, W, H);
    h.set_stylesheet(&sheet());
    h.pump();

    let centre = |h: &Host<App, Incremental, I>, wid: &str| {
        let s = &h.states[index(wid) as usize];
        (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
    };
    let record = |h: &Host<App, Incremental, I>| (h.states.clone(), h.list.hash_from(0), h.list.len());

    let mut out = Vec::new();
    for step in 0..STEPS {
        let want = match step {
            0 => {
                let (x, y) = centre(&h, "go");
                h.post(Event::PointerMove { x, y });
                Want::Hovered("go")
            }
            1 => {
                let (x, y) = centre(&h, "go");
                h.post(Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 });
                Want::Pressed("go")
            }
            2 => {
                let (x, y) = centre(&h, "go");
                h.post(Event::PointerUp { x, y, button: PointerButton::Left });
                Want::Released("go")
            }
            3 => {
                let (x, y) = centre(&h, "sw");
                h.post(Event::PointerMove { x, y });
                Want::Hovered("sw")
            }
            _ => {
                h.post(Event::Key { key: Key::Tab, down: true });
                Want::Focused("field")
            }
        };
        h.pump();
        // **The setup landed**, printed with the selector.
        match want {
            Want::Hovered(w) => {
                println!("{name} step {step}: hovered {:?}, #{w} is {}", h.input.hovered, index(w));
                assert_eq!(h.input.hovered, Some(index(w)), "{name}: the move never hovered #{w}");
            }
            Want::Pressed(w) => {
                println!("{name} step {step}: pressed {:?}, #{w} is {}", h.input.pressed, index(w));
                assert_eq!(h.input.pressed, Some(index(w)), "{name}: the press never landed on #{w}");
            }
            Want::Released(w) => {
                println!(
                    "{name} step {step}: pressed {:?} after release, clicks {}, hovered {:?} (#{w} is {})",
                    h.input.pressed,
                    h.app.clicks,
                    h.input.hovered,
                    index(w)
                );
                assert_eq!(h.input.pressed, None, "{name}: the release left #{w} pressed");
                assert_eq!(h.app.clicks, 1, "{name}: the release never clicked #{w}");
            }
            Want::Focused(w) => {
                println!(
                    "{name} step {step}: focus {:?}, #{w} is {}, focus_visible {}",
                    h.input.focus,
                    index(w),
                    h.input.focus_visible
                );
                assert_eq!(h.input.focus, Some(index(w)), "{name}: Tab never reached #{w}");
                assert!(h.input.focus_visible, "{name}: a Tab must show the ring");
            }
        }
        out.push((format!("step {step} frame 0"), record(&h)));
        for f in 1..=TICKS {
            h.tick(16.0);
            h.pump();
            out.push((format!("step {step} frame {f}"), record(&h)));
        }
    }
    out
}

/// The first field two states disagree on, by name.
fn differs(a: &NodeState, b: &NodeState) -> Option<&'static str> {
    macro_rules! cmp {
        ($($f:ident),*) => {
            $( if format!("{:?}", a.$f) != format!("{:?}", b.$f) { return Some(stringify!($f)); } )*
        };
    }
    cmp!(
        constraints,
        size,
        pos,
        fg,
        bg,
        radius,
        font_size,
        font_weight,
        shadow,
        pad,
        pressed,
        hovered,
        focused,
        focus_ring,
        border_color,
        border_width,
        disabled,
        line_height,
        fill
    );
    None
}

#[test]
fn both_strategies_draw_every_frame_of_the_script_identically() {
    let all = drive::<Everything>("Everything");
    let bounded = drive::<Boundaries<256>>("Boundaries<256>");
    assert_eq!(all.len(), bounded.len());
    for ((label, (sa, ha, la)), (_, (sb, hb, lb))) in all.iter().zip(&bounded) {
        for (i, (a, b)) in sa.iter().zip(sb).enumerate() {
            if let Some(field) = differs(a, b) {
                panic!(
                    "{label}: node {i} differs under Boundaries on the field `{field}`: \
                     Everything {a:?}\n  Boundaries {b:?}"
                );
            }
        }
        assert_eq!((ha, la), (hb, lb), "{label}: the display lists differ");
    }
    println!(
        "frames compared: {} ({STEPS} steps x {} frames), 0 differ",
        all.len(),
        TICKS + 1
    );
    assert_eq!(all.len(), STEPS * (TICKS + 1), "every move and every tick was compared");
}
