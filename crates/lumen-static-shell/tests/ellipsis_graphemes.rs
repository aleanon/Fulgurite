//! **Where an ellipsis cuts a grapheme cluster** — M80/T80.0, the gate
//! `docs/plan-text-fit.md` commits before T80.3 builds the shared cut.
//!
//! `AppBar::fit_title` searches `char` boundaries (`nav.rs`), so a title
//! whose fitting prefix ends inside a ZWJ family, a flag or a skin-tone
//! emoji keeps half a character before the `…`. This sweeps the room a
//! title gets in 1 px steps, for the four clusters `graphemes.rs` edits,
//! and counts the cuts that land **inside** each cluster and the cuts made
//! **near** it (within one cluster's length). The near count is the
//! setup: a sweep that never cut there would report 0 inside and prove
//! nothing.
//!
//! **It pins today's gap, green.** Each cluster's inside-count is asserted
//! at the value measured at `553fa5e`; T80.3, which cuts by grapheme,
//! flips every pin to 0. The combining accent is pinned at 0 today, and
//! not because the search respects it: in `tests/fonts/DejaVuSans.ttf`
//! U+0301 has advance 0, so `w("…e") == w("…e\u{301}")` and the search
//! for the widest fitting char prefix always lands after the accent.
#![cfg(feature = "truetype")]

use lumen_static::{App as DeriveApp, StaticTree};
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Button;
use lumen_static_widgets::AppBar;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    bar: AppBar<String, Button<App>, Button<App>>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const CLUSTERS: [(&str, &str); 4] = [
    ("combining accent", "e\u{301}"),
    ("ZWJ family", "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}"),
    ("flag", "\u{1f1f3}\u{1f1f4}"),
    ("skin tone", "\u{1f44d}\u{1f3fd}"),
];

/// Today's inside-counts, measured at `553fa5e` (printed by this test).
const PINNED_INSIDE: [usize; 4] = [0, 20, 10, 10];

fn face() -> FontStack {
    let regular = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fonts/DejaVuSans.ttf")).unwrap();
    FontStack::from_bytes(&regular, None).expect("the test face parses")
}

/// The byte where the bar cut `title` at `width`, or `None` uncut.
fn cut_at(title: &str, width: u32, face: &FontStack) -> Option<usize> {
    let mut app = App::default();
    app.ui.bar = AppBar::with(title.to_string(), Button::<App>::default(), Button::<App>::default());
    let mut h: Host<App> = Host::new(app, width, 80);
    let mut m = FontMetrics::new(face.clone());
    h.pump_with(&mut m);
    let (shown, cut) = h.app.ui.bar.shown_title();
    cut.then_some(shown.len())
}

/// `(inside, near)` for a sample, swept over every width that cuts
/// somewhere in the title. `bounds` are the sample's internal grapheme
/// boundaries (byte offsets into it): a cut on one is not inside.
fn sweep(cluster: &str, bounds: &[usize], face: &FontStack) -> (usize, usize) {
    let title = format!("WWWWWW{cluster}WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW");
    let (start, end) = (6, 6 + cluster.len());
    let (mut inside, mut near) = (0, 0);
    for width in 60..700 {
        let Some(c) = cut_at(&title, width, face) else { continue };
        if c > start && c < end && !bounds.contains(&(c - start)) {
            inside += 1;
        }
        if c + cluster.len() >= start && c <= end + cluster.len() {
            near += 1;
        }
    }
    (inside, near)
}

#[test]
fn an_ellipsis_cut_inside_a_cluster_is_pinned_per_cluster() {
    let face = face();
    let mut got = [0usize; 4];
    for (k, (name, cluster)) in CLUSTERS.iter().enumerate() {
        let (inside, near) = sweep(cluster, &[], &face);
        println!("{name}: cuts inside {inside} (pinned {}), cuts near {near} (bound > 0)", PINNED_INSIDE[k]);
        assert!(near > 0, "{name}: no width cut near the cluster -- the sweep proves nothing");
        got[k] = inside;
    }
    assert_eq!(got, PINNED_INSIDE, "the inside-counts moved; T80.3 flips them to 0 deliberately");
}

#[test]
fn an_ascii_title_has_nothing_to_cut_inside() {
    // Accept direction: two one-byte graphemes, so every cut between them
    // is on a boundary and none lands inside one -- while the sweep still
    // cuts near them, so the 0 is not a sweep that cut nowhere.
    let face = face();
    let (inside, near) = sweep("ee", &[1], &face);
    println!("ascii: cuts inside {inside} (bound 0), cuts near {near} (bound > 0)");
    assert_eq!(inside, 0, "an ASCII pair was cut inside");
    assert!(near > 0, "the ASCII sweep cut nowhere near the pair");
}
