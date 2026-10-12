//! **M84/T84.8: `Video<S>`, `VideoFeed`, `GifFeed`** —
//! `docs/plan-images-in-motion.md`, `docs/results-images.md`.
//!
//! The headless half of the video's gates. The widget computes a seek's
//! fraction from the press against the track it laid out and hands it to
//! the application through `Node::take_action`; a press on the picture
//! does nothing; the play button and a reader's actions on the video reach
//! `on_toggle` and `on_seek`; and a `GifFeed` subscription paces its frames
//! to their `pts_ms`, so the inbox never refuses one. The live half is
//! `scripts/drive/widgets/video.py` (`just widget-drive Video`).

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use fulgurite::event::{Action, Event};
use fulgurite::prelude::*;
use fulgurite::spawn::{Descriptor, JobFn, Subs, Threads};
use fulgurite_shell::image::DecodedImage;
use fulgurite_shell::video::{video_job, GifFeed, GifLibrary, VideoFeed, VideoFrame, VideoKey};
use fulgurite_shell::Host;
use fulgurite_widgets::prelude::*;
use fulgurite_widgets::seek_fraction;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("v")]
    v: Video<St>,
}

/// What the handlers record.
#[derive(Default)]
struct St {
    toggles: u32,
    seeks: Vec<f32>,
    /// Calls to `on_seek` that carried no fraction.
    bare: u32,
    subs: Subs,
    /// When each frame was applied, and from which feed.
    arrived: Vec<(Instant, u64)>,
}

#[derive(DeriveApp, Default)]
#[app(msg = VideoFrame, apply = take_frame, subscriptions = subs, jobs = jobs)]
struct Player {
    #[tree]
    ui: Ui,
    #[state]
    st: St,
}

fn take_frame(s: &mut St, f: VideoFrame) -> bool {
    s.arrived.push((Instant::now(), f.key));
    true
}

fn subs(p: &mut Player) -> Option<&mut Subs> {
    Some(&mut p.st.subs)
}

const KIND: u64 = 8;

fn jobs(d: Descriptor) -> Option<JobFn<VideoFrame>> {
    (d.kind == KIND).then_some(video_job::<GifFeed<Lib>, VideoFrame> as JobFn<VideoFrame>)
}

/// One source: four 8 px frames at 100 ms, a colour each.
struct Lib;

impl GifLibrary for Lib {
    fn gif(index: u16) -> Option<&'static DecodedImage> {
        static GIF: OnceLock<DecodedImage> = OnceLock::new();
        (index == 0).then(|| {
            GIF.get_or_init(|| DecodedImage {
                w: 8,
                h: 8,
                frames: (0..4u8).map(|k| [k * 60, 40, 200 - k * 40, 255].repeat(64)).collect(),
                delays_ms: vec![100; 4],
            })
        })
    }
}

fn toggle(s: &mut St, _: &EventCtx) {
    s.toggles += 1;
}

fn seek(s: &mut St, cx: &EventCtx) {
    match seek_fraction(cx) {
        Some(f) => s.seeks.push(f),
        None => s.bare += 1,
    }
}

const W: u32 = 480;
const H: u32 = 420;

fn host() -> Host<Player> {
    let mut p = Player::default();
    p.ui.v = Video::new(fulgurite::display::ImageId(5), Size { w: 96.0, h: 96.0 });
    p.ui.v.on_toggle = toggle;
    p.ui.v.on_seek = seek;
    p.ui.v.duration_ms = 400;
    let mut h = Host::new(p, W, H);
    h.pump();
    h
}

/// The table index of a part, and its laid-out rect.
fn part(h: &Host<Player>, id: &str) -> (usize, [f32; 4]) {
    let table = <Ui as fulgurite::widget::Widget>::SUB_TABLE;
    let v = fulgurite::style::index_of_wid(table, "v").expect("the video");
    let i = (v..v + 4).find(|&i| table[i].wid == id).unwrap_or_else(|| panic!("no part {id}"));
    let s = &h.states[i];
    (i, [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h])
}

fn click(h: &mut Host<Player>, x: f32, y: f32) -> bool {
    h.post(Event::down(x, y));
    h.post(Event::up(x, y));
    h.pump()
}

/// **A press at the track's midpoint seeks half** — the plan's "Seek
/// fraction" gate (P19). The widget computes the fraction against the track
/// it laid out, not its own box. Prints the value beside the bound
/// `0.5 ± 1/track-width`, and the width.
///
/// *Injection:* the video measures against its own box (the track's
/// `track_w` set to the video's width) — the fraction is off by the
/// controls' inset, printed against the bound.
#[test]
fn a_press_at_the_tracks_midpoint_seeks_half() {
    let mut h = host();
    let (_, t) = part(&h, "video-track");
    let w = t[2] - t[0];
    assert!(w > 50.0, "setup: the track is {w} px wide");
    let handled = click(&mut h, (t[0] + t[2]) / 2.0, (t[1] + t[3]) / 2.0);
    let seeks = h.app.st.seeks.clone();
    println!("press at the track's midpoint: handled {handled}, on_seek ran {} time(s) with {seeks:?}; \
              track {w:.1} px wide, bound 0.5 +- {:.4}", seeks.len(), 1.0 / w);
    assert!(handled, "setup: the press was not handled");
    // A press seeks, and its release seeks again at the same point.
    assert_eq!(seeks.len(), 2, "setup: on_seek ran {} times, want 2 (press and release)", seeks.len());
    for f in &seeks {
        assert!((f - 0.5).abs() <= 1.0 / w, "on_seek got {f}, want 0.5 +- {:.4}", 1.0 / w);
    }
    assert_eq!(h.app.st.toggles, 0, "a press on the track toggled play");
}

/// **A drag previews, and its release seeks where it ended** — the scrub's
/// headless half. A press at 25% seeks 0.25; the moves seek nothing; the
/// release at 50% seeks 0.5. Prints each.
///
/// *Injection:* the track's release does not seek — the drag ends at 0.25.
#[test]
fn a_drag_seeks_at_its_press_and_its_release_only() {
    let mut h = host();
    let (_, t) = part(&h, "video-track");
    let (w, y) = (t[2] - t[0], (t[1] + t[3]) / 2.0);
    let at = |f: f32| t[0] + w * f;
    h.post(Event::down(at(0.25), y));
    h.pump();
    let after_press = h.app.st.seeks.clone();
    for k in 1..=5 {
        h.post(Event::PointerMove { x: at(0.25 + 0.05 * k as f32), y });
        h.pump();
    }
    let after_moves = h.app.st.seeks.clone();
    h.post(Event::up(at(0.5), y));
    h.pump();
    let all = h.app.st.seeks.clone();
    println!("drag 25% -> 50%: after the press {after_press:?}, after 5 moves {after_moves:?}, \
              after the release {all:?} (bound: [0.25], [0.25], [0.25, 0.5] +- {:.4})", 1.0 / w);
    assert_eq!(after_press.len(), 1, "setup: the press did not seek");
    assert!((after_press[0] - 0.25).abs() <= 1.0 / w, "the press sought {}", after_press[0]);
    assert_eq!(after_moves.len(), 1, "a move sought: a drag re-keyed the feed per pointer move");
    assert_eq!(all.len(), 2, "the release did not seek: the drag ends where it started");
    assert!((all[1] - 0.5).abs() <= 1.0 / w, "the release sought {}, want 0.5", all[1]);
}

/// **A press on the picture does nothing** — the plan's isolate: `handled`
/// false, no handler ran. Accept direction: a press on the play button, on
/// the same host, is handled and toggles once.
///
/// *Injection:* `Video`'s handler answers `on_toggle` for the picture —
/// handled true and one toggle, by name.
#[test]
fn a_press_on_the_picture_does_nothing() {
    let mut h = host();
    let (_, p) = part(&h, "video-picture");
    let handled = click(&mut h, (p[0] + p[2]) / 2.0, (p[1] + p[3]) / 2.0);
    let (t0, s0) = (h.app.st.toggles, h.app.st.seeks.len());
    let (_, b) = part(&h, "video-play");
    let play = click(&mut h, (b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    println!("press on the picture: handled {handled}, toggles {t0}, seeks {s0} (bound false, 0, 0); \
              on the play button: handled {play}, toggles {}", h.app.st.toggles);
    assert!(!handled && t0 == 0 && s0 == 0, "a press on the picture was handled ({handled}) or ran a handler");
    assert!(play && h.app.st.toggles == 1, "accept: the play button did not toggle once");
}

/// **A reader's actions on the video reach the application** — M54/T54.4's
/// rule, through the same route: the default action runs `on_toggle`, and
/// `SetValue(0.3)` runs `on_seek` with 0.3. And the play button's label
/// says what it will do: "Play" while paused, "Pause" while playing.
///
/// *Injection:* the pass ignores `Node::take_action` — the action is
/// consumed and no handler runs: toggles 0, seeks [].
#[test]
fn a_readers_actions_toggle_and_seek() {
    let mut h = host();
    let table = <Ui as fulgurite::widget::Widget>::SUB_TABLE;
    let v = fulgurite::style::index_of_wid(table, "v").expect("the video") as u32;
    h.post(Event::Action { node: v, action: Action::Default });
    let a = h.pump();
    h.post(Event::Action { node: v, action: Action::SetValue(0.3) });
    let b = h.pump();
    println!("default action: handled {a}, toggles {}; SetValue(0.3): handled {b}, seeks {:?}, bare {}",
             h.app.st.toggles, h.app.st.seeks, h.app.st.bare);
    assert!(a && b, "setup: a reader's action was not handled");
    assert_eq!(h.app.st.toggles, 1, "the default action did not toggle");
    assert_eq!(h.app.st.seeks, vec![0.3], "SetValue did not seek 0.3");
    assert_eq!(h.app.st.bare, 0, "on_seek ran with no fraction");

    let paused = h.app.ui.v.play.label;
    h.app.ui.v.playing = true;
    h.invalidate_all();
    h.pump();
    let playing = h.app.ui.v.play.label;
    println!("the play button reads {paused:?} paused and {playing:?} playing");
    assert_eq!((paused, playing), ("Play", "Pause"), "the button's label ignores `playing`");
}

/// **Frames arrive at their `pts_ms`** — the plan's pacing gate. A
/// `GifFeed` at 100 ms runs as a subscription on a real worker; 20 frames
/// arrive 100 ± 10 ms apart, and the inbox refuses none. Prints the
/// smallest and largest gap and `Inbox::refused`.
///
/// *Injection:* `video_job` does not sleep to `pts_ms` — the feed fills the
/// inbox at once and the rest are refused, counted.
#[test]
fn a_feed_is_paced_to_its_pts() {
    let mut h = host();
    h.set_spawner(Box::new(Threads::new()));
    let key = VideoKey { source: 0, seek: 1, start_ms: 0 }.pack();
    h.app.st.subs.declare(KIND, key);
    let end = Instant::now() + Duration::from_secs(4);
    while h.app.st.arrived.len() < 21 && Instant::now() < end {
        h.pump();
        std::thread::sleep(Duration::from_millis(1));
    }
    h.app.st.subs.clear();
    h.pump();
    let at = &h.app.st.arrived;
    let gaps: Vec<f64> = at.windows(2).take(20).map(|w| (w[1].0 - w[0].0).as_secs_f64() * 1000.0).collect();
    let (lo, hi) = gaps.iter().fold((f64::MAX, 0.0f64), |(l, u), &g| (l.min(g), u.max(g)));
    let refused = h.inbox.refused();
    println!("{} frames applied; gaps over the first 20: min {lo:.1} ms, max {hi:.1} ms \
              (bound 100 +- 10); inbox refused {refused} (bound 0); keys all {key:#x}: {}",
             at.len(), at.iter().all(|a| a.1 == key));
    assert!(at.len() >= 21, "setup: only {} frames arrived in 4 s", at.len());
    assert_eq!(refused, 0, "the inbox refused {refused} frames: the feed is not paced");
    assert!(lo >= 90.0 && hi <= 110.0, "frame gaps {lo:.1}..{hi:.1} ms, want 100 +- 10");
}

/// **The feed opens where its key says, and loops** — `GifFeed`'s own
/// arithmetic, which the live seek's "picture shows frame ⌊0.75·n⌋" reads.
#[test]
fn a_gif_feed_starts_at_its_key_and_loops() {
    let mut f = GifFeed::<Lib>::open(VideoKey { source: 0, seek: 0, start_ms: 300 }.pack()).expect("opens");
    let got: Vec<(u32, u32, u64)> = (0..6).filter_map(|_| f.next_frame()).map(|x| (x.index, x.position_ms, x.pts_ms)).collect();
    println!("from 300 ms: (index, position, pts) {got:?}");
    assert_eq!(got.first().map(|g| g.0), Some(3), "a start at 75% shows frame floor(0.75 * 4) = 3");
    assert_eq!(got.iter().map(|g| g.0).collect::<Vec<_>>(), vec![3, 0, 1, 2, 3, 0], "it loops in order");
    assert_eq!(got.iter().map(|g| g.2).collect::<Vec<_>>(), vec![300, 400, 500, 600, 700, 800]);
    let mid = GifFeed::<Lib>::open(VideoKey { source: 0, seek: 0, start_ms: 250 }.pack()).map(|mut f| {
        let a = f.next_frame().unwrap();
        let b = f.next_frame().unwrap();
        ((a.index, a.position_ms), (b.index, b.pts_ms))
    });
    println!("from 250 ms: {mid:?}");
    assert_eq!(mid, Some(((2, 250), (3, 300))), "a mid-frame start waits out the rest of its frame");
    assert!(GifFeed::<Lib>::open(VideoKey { source: 1, seek: 0, start_ms: 0 }.pack()).is_none(), "an unknown source opens nothing");
}
