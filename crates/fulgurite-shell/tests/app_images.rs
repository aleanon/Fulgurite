//! **The application's images reach the renderer through the frame** —
//! M84/T84.6 (`docs/plan-images-in-motion.md`, `docs/results-images.md`).
//!
//! Until T84.6 a renderer's only image source was the window's
//! (`Program::images`, moved into the backend once), so nothing an
//! application did in `apply` or `tick` could change pixels, and a hot
//! build's assets never reached the window. Now `App::images` answers the
//! application's store, a session hands it over on every present through
//! `FrameSource::images`, and every renderer resolves a command through it
//! before the window's (`ImageChain`).
//!
//! - **Headless write**: `apply` replaces a stream's pixels, paints the
//!   frame `replace` answered, and the next `Offscreen` present shows them,
//!   damaging the image's box and nothing else.
//! - **Order**: one id in both stores draws the application's; an id only
//!   the window holds still draws.
//! - **Panic**: a session asks `App::images` inside its frame's guarded
//!   entry, so a panic there raises M36's screen; while it is up the
//!   session answers `None`.

use fulgurite::desc::Size;
use fulgurite::display::{Color, ImageId, ImageSource};
use fulgurite::event::App;
use fulgurite::prelude::*;
use fulgurite_shell::image::ImageStore;
use fulgurite_shell::session::{FrameSource, Local, Offscreen, Recovering, Screen, Session};
use fulgurite_shell::Host;
use fulgurite_widgets::prelude::*;

const W: u32 = 120;
const H: u32 = 100;
const SIDE: u32 = 8;
const PAGE: Color = Color(250, 250, 250, 255);
const RED: [u8; 4] = [220, 30, 30, 255];
const BLUE: [u8; 4] = [30, 60, 220, 255];
const GREEN: [u8; 4] = [30, 160, 60, 255];
const PAGE_PX: [u8; 4] = [250, 250, 250, 255];

fn fill(c: [u8; 4]) -> Vec<u8> {
    c.iter().copied().cycle().take((SIDE * SIDE * 4) as usize).collect()
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("a")]
    a: Image,
    #[widget_id("b")]
    b: Image,
}

fn ui(a: ImageId, b: ImageId) -> Ui {
    let mut u = Ui { a: Image::new(a, Size { w: 40.0, h: 40.0 }), b: Image::new(b, Size { w: 40.0, h: 40.0 }) };
    u.a.tint = Color(255, 255, 255, 255);
    u.b.tint = Color(255, 255, 255, 255);
    u
}

/// The centre of node `i`'s box, as RGBA.
fn centre<A: App>(screen: &Offscreen, host: &Host<A>, i: usize) -> [u8; 4] {
    let st = &host.states[i];
    let (x, y) = ((st.pos.x + st.size.w / 2.0) as u32, (st.pos.y + st.size.h / 2.0) as u32);
    let p = ((y * W + x) * 4) as usize;
    screen.canvas.pixels[p..p + 4].try_into().unwrap()
}

// ---- headless write ------------------------------------------------------

/// A stream in miniature: each message is a colour, written into the store
/// with `replace`, whose answer is the frame the image paints.
struct Feed {
    ui: Ui,
    store: ImageStore,
    applied: u32,
}

impl App for Feed {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = Ui;
    type Msg = [u8; 4];
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    fn apply(this: &mut Self, c: [u8; 4]) -> bool {
        this.applied += 1;
        match this.store.replace(ImageId(5), SIDE, SIDE, &fill(c)) {
            Some(frame) => {
                this.ui.a.frame = frame;
                true
            }
            None => false,
        }
    }
    fn images(&self) -> Option<&dyn ImageSource> {
        Some(&self.store)
    }
}

/// **Headless write.** Red at boot; `apply` replaces id 5 with blue and
/// paints the frame `replace` answered; the next present shows blue at
/// the image's centre and damages exactly its box (1.00×), and the
/// neighbour (an id no store holds) stays page. *Injection:* the host's
/// `FrameSource::images` answers `None` — the centre reads the page.
#[test]
fn an_apply_that_replaces_pixels_reaches_the_next_present() {
    let mut store = ImageStore::new();
    assert_eq!(store.replace(ImageId(5), SIDE, SIDE, &fill(RED)), Some(1));
    let mut app = Feed { ui: ui(ImageId(5), ImageId(9)), store, applied: 0 };
    app.ui.a.frame = 1;
    let mut host: Host<Feed> = Host::new(app, W, H);
    let mut screen = Offscreen::new(W, H);
    host.pump();
    screen.present(&mut host, PAGE);
    let before = centre(&screen, &host, 1);
    println!("boot: id 5's centre {before:?} (want the store's red {RED:?})");
    assert_eq!(before, RED, "setup: the application's store did not reach the first present");

    host.sender().send(BLUE).expect("the inbox takes a message");
    let handled = host.pump();
    let t = screen.present(&mut host, PAGE);
    let after = centre(&screen, &host, 1);
    let neighbour = centre(&screen, &host, 2);
    let px = t.damaged_px.unwrap_or(u64::MAX);
    let rect = (host.states[1].size.w * host.states[1].size.h) as u64;
    let ratio = px as f64 / rect as f64;
    println!(
        "after apply: applied {} (pump {handled}), a.frame {}; centre {after:?} (want {BLUE:?}), neighbour {neighbour:?}; \
         damaged_px {px} against the image's box {rect} px: {ratio:.2}x (bound 1.00x)",
        host.app.applied, host.app.ui.a.frame,
    );
    assert_eq!(host.app.applied, 1, "setup: apply did not run on the message");
    assert_eq!(host.app.ui.a.frame, 2, "setup: apply did not paint the frame replace answered");
    assert_ne!(after, PAGE_PX, "the image drew the page colour: the application's store never reached the renderer");
    assert_eq!(after, BLUE, "the replaced pixels did not reach the next present");
    assert_eq!(neighbour, PAGE_PX, "an id no store holds drew something");
    assert_eq!(ratio, 1.0, "replacing one image damaged {px} px, not its box of {rect} px");
}

// ---- order ---------------------------------------------------------------

/// Id 1 in the application's store (red) and the window's (blue); id 2 in
/// the window's alone (green).
#[derive(DeriveApp)]
#[app(images = app_store)]
struct Both {
    #[tree]
    ui: Ui,
    store: ImageStore,
    /// The panic gate's arm: `images` panics once when set.
    bad: core::cell::Cell<bool>,
}

fn app_store(b: &Both) -> Option<&dyn ImageSource> {
    if b.bad.replace(false) {
        panic!("an App::images that panics");
    }
    Some(&b.store)
}

fn both() -> Both {
    let mut store = ImageStore::new();
    assert!(store.insert(ImageId(1), SIDE, SIDE, vec![fill(RED)]));
    Both { ui: ui(ImageId(1), ImageId(2)), store, bad: Default::default() }
}

fn window_store() -> ImageStore {
    let mut w = ImageStore::new();
    assert!(w.insert(ImageId(1), SIDE, SIDE, vec![fill(BLUE)]));
    assert!(w.insert(ImageId(2), SIDE, SIDE, vec![fill(GREEN)]));
    w
}

/// **Order.** The application's store first: id 1 is red, not the
/// window's blue; id 2, which only the window holds, still draws green
/// (the accept direction). *Injection:* `ImageChain::resolve` asks the
/// window's store first — id 1 reads blue.
#[test]
fn the_applications_store_wins_and_the_windows_still_draws() {
    let mut host: Host<Both> = Host::new(both(), W, H);
    let mut screen = Offscreen::new(W, H);
    screen.canvas.images = Some(Box::new(window_store()));
    host.pump();
    screen.present(&mut host, PAGE);
    let (one, two) = (centre(&screen, &host, 1), centre(&screen, &host, 2));
    println!(
        "id 1 (both stores): {one:?} (want the application's {RED:?}, not the window's {BLUE:?}); \
         id 2 (window only): {two:?} (want {GREEN:?})"
    );
    assert!(host.images().is_some(), "setup: the derived application answered no store");
    assert_eq!(one, RED, "an id both stores hold drew the window's pixels: the application's store is not first");
    assert_eq!(two, GREEN, "an id only the window holds did not draw");
}

// ---- panic ---------------------------------------------------------------

/// **Panic.** A session asks `App::images` inside its frame's guarded
/// entry: armed, the frame records the failure (M36's screen), and while
/// it is up `images()` answers `None` without asking. After the retry it
/// answers again. *Injections:* (1) the session's frame does not ask
/// `App::images` — `failed` reads false, the panic swallowed by the
/// present's catch and nothing recorded; (2) `Local::images` forwards
/// regardless of the failure — it reads `Some` while the screen is up.
#[test]
fn a_panicking_images_raises_the_screen_and_the_session_answers_none() {
    let mut host: Host<Both> = Host::new(both(), W, H);
    host.pump();
    let mut s = Local::from_host(host, "app-images", PAGE, Recovering);
    assert!(!s.failed() && s.images().is_some(), "setup: a healthy session hands over the application's store");
    s.host.app.bad.set(true);
    s.frame(16.0, W, H);
    let failed = s.failed();
    let during = s.images().is_some();
    println!("after an armed frame: failed {failed} (want true); images() while the screen is up: {}", if during { "Some" } else { "None" });
    assert!(failed, "a panic in App::images was not recorded by the frame's guard: no screen would be drawn");
    assert!(!during, "a session showing M36's screen handed over the application's store");
    assert!(s.retry(), "this build recovers");
    assert!(s.images().is_some(), "after the retry the session does not answer the store again");
}
