//! **An animated image wakes the window at its next frame** — M84/T84.3
//! (`docs/plan-images-in-motion.md`, `docs/results-images.md`).
//!
//! An animated image's tick answers `true` only when its `Playback`
//! changes frame, so after one frame the window has nothing to draw and
//! idles. What wakes it is the deadline: `App::wake_in_ms`, folded into
//! `Host::next_deadline_ms` beside the timers, and asked **guarded** by a
//! session (`Local::next_deadline_ms`), because it is application code.

use fulgurite::anim::Playback;
use fulgurite::display::{Color, ImageId};
use fulgurite::prelude::*;
use fulgurite_shell::session::{Crashing, Local, Recovering, Session};
use fulgurite_shell::Host;
use fulgurite_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("img")]
    img: Image,
}

/// The showcase's shape, in miniature: the clock in the application, the
/// frame on the image, and the wake answered from the clock.
#[derive(DeriveApp, Default)]
#[app(tick = advance, wake = next_frame)]
struct Gif {
    #[tree]
    ui: Ui,
    spin: Playback,
    /// Site 9's shape (`fixtures/panic-app`): a wake that panics once.
    bad_wake: core::cell::Cell<bool>,
}

fn advance(g: &mut Gif, dt: f32) -> bool {
    match g.spin.tick(dt) {
        Some(f) => {
            g.ui.img.frame = f;
            true
        }
        None => false,
    }
}

fn next_frame(g: &Gif) -> Option<f32> {
    if g.bad_wake.replace(false) {
        panic!("a wake_in_ms that panics");
    }
    g.spin.next_change_ms()
}

fn gif() -> Gif {
    let mut g = Gif { spin: Playback::new(&[100; 4]), ..Default::default() };
    g.ui.img = Image::new(ImageId(1), fulgurite::desc::Size { w: 8.0, h: 8.0 });
    g
}

/// **Deadline** — T84.3's second gate. While playing, the host's next
/// deadline is the playback's next change (± 0.01 ms), through
/// `Host::next_deadline_ms` and through the session's guarded
/// `Local::next_deadline_ms`; paused, it is `None` in both.
/// *Injection:* `Host::next_deadline_ms` ignores `wake_in_ms` -- it reads
/// `None` while playing.
#[test]
fn the_deadline_is_the_next_frame_change_while_playing() {
    let mut h: Host<Gif> = Host::new(gif(), 120, 100);
    h.pump();
    // Setup landed: a tick at the deadline changes the frame and reports it.
    let first = h.next_deadline_ms();
    let ticked = h.tick(30.0);
    assert!(!ticked, "setup: 30 ms into a 100 ms frame changed nothing, and the tick says so");
    let (want, got) = (h.app.spin.next_change_ms(), h.next_deadline_ms());
    println!("host: first deadline {first:?}; after 30 ms: next_deadline_ms {got:?}, Playback::next_change_ms {want:?} (bound ±0.01)");
    assert_eq!(want, Some(70.0), "setup: the playback is 30 ms into its first frame");
    let close = |a: Option<f32>, b: Option<f32>| match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() <= 0.01,
        (a, b) => a == b,
    };
    assert!(close(got, want), "Host::next_deadline_ms is {got:?} while the image plays; its next change is in {want:?} ms -- the wake is not folded in");

    let mut s = Local::from_host(h, "image-wake", Color(255, 255, 255, 255), Crashing);
    let via = s.next_deadline_ms();
    println!("session: Local::next_deadline_ms {via:?} (want {want:?})");
    assert!(close(via, want), "Local::next_deadline_ms is {via:?}, the playback's next change {want:?}");

    // At the deadline the frame moves, and the tick reports it.
    assert!(s.host.tick(70.0), "a tick at the deadline did not report the frame change");
    assert_eq!(s.host.app.ui.img.frame, 1, "the tick did not write the new frame into the image");

    // Paused: no frame will change, so nothing asks for one.
    s.host.app.spin.playing = false;
    let (paused_host, paused_session) = (s.host.next_deadline_ms(), s.next_deadline_ms());
    println!("paused: host {paused_host:?}, session {paused_session:?} (want None, None)");
    assert_eq!((paused_host, paused_session), (None, None), "a paused image still asks for a frame");
}

/// **A panicking wake raises M36's screen and does not unwind** — second
/// review A3. The session asks it guarded: the failure is recorded (so a
/// window draws the screen and `app.failure` names the site), and the
/// deadline answers what the host alone knows. *Injection:*
/// `Local::next_deadline_ms` asks `wake_in_ms` outside the guard -- the
/// panic escapes this call and the test dies on it.
#[test]
fn a_panicking_wake_is_caught_and_recorded() {
    let mut h: Host<Gif> = Host::new(gif(), 120, 100);
    h.pump();
    let mut s = Local::from_host(h, "image-wake", Color(255, 255, 255, 255), Recovering);
    assert!(!s.failed(), "setup: nothing has failed yet");
    s.host.app.bad_wake.set(true);
    let d = s.next_deadline_ms();
    let failed = s.failed();
    println!("a wake that panicked: deadline {d:?}, failed {failed} (want None, true)");
    assert!(failed, "the wake's panic was not recorded: no screen would be drawn");
    assert_eq!(d, None, "a failed session still answered the image's deadline");
    // A failed host asks no application code until the retry.
    assert!(s.retry(), "this build recovers");
    assert_eq!(s.next_deadline_ms(), Some(100.0), "after the retry the wake answers again");
}
