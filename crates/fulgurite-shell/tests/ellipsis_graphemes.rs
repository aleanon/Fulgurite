//! **Where an ellipsis cuts a grapheme cluster** — M80/T80.0 committed
//! this gate pinning the gap; M80/T80.3 flipped it
//! (`docs/plan-text-fit.md`).
//!
//! `AppBar::fit_title` searched `char` boundaries (`nav.rs`), so a title
//! whose fitting prefix ended inside a ZWJ family, a flag or a skin-tone
//! emoji kept half a character before the `…`: at `553fa5e` the sweep
//! below counted 20, 10 and 10 such cuts. Since T80.3 the bar and a
//! `Label` under `TextOverflow::Ellipsis` cut through the one shared
//! search, `fulgurite_widgets::fit::cut`, which snaps every candidate
//! down to a **grapheme** boundary — so every count is 0.
//!
//! This sweeps the room a text gets in 1 px steps, for the four clusters
//! `graphemes.rs` edits, and counts the cuts that land **inside** each
//! cluster and the cuts made **near** it (within one cluster's length).
//! The near count is the setup: a sweep that never cut there would report
//! 0 inside and prove nothing.
//!
//! The combining accent is 0 under the old char search too, and not
//! because that search respected it: in `tests/fonts/DejaVuSans.ttf`
//! U+0301 has advance 0, so `w("…e") == w("…e\u{301}")` and a search for
//! the widest fitting prefix always lands after the accent. It is printed
//! and is not the injection's witness; the other three are.
#![cfg(feature = "truetype")]

use fulgurite::style::{Patch, Rule, Selector, TextOverflow};
use fulgurite::{App as DeriveApp, StaticTree};
use fulgurite_shell::text::{FontMetrics, FontStack};
use fulgurite_shell::Host;
use fulgurite_widgets::ops::{Button, Label};
use fulgurite_widgets::AppBar;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    bar: AppBar<String, Button<App>, Button<App>>,
    #[widget_id("label")]
    label: Label<String>,
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

/// The inside-counts the char search left at `553fa5e`, for the record
/// (printed beside today's): T80.3 cuts by grapheme and every one is 0.
const BEFORE_T80_3: [usize; 4] = [0, 20, 10, 10];

/// Which widget made the cut.
#[derive(Clone, Copy, Debug)]
enum Cutter {
    AppBar,
    /// A `Label` under `TextOverflow::Ellipsis`, as wide as the window.
    Label,
}

fn face() -> FontStack {
    let regular = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fonts/DejaVuSans.ttf")).unwrap();
    FontStack::from_bytes(&regular, None).expect("the test face parses")
}

/// The byte where `who` cut `title` at `width`, or `None` uncut.
fn cut_at(who: Cutter, title: &str, width: u32, face: &FontStack) -> Option<usize> {
    let mut app = App::default();
    app.ui.bar = AppBar::with(title.to_string(), Button::<App>::default(), Button::<App>::default());
    app.ui.label.set(title.to_string());
    let mut h: Host<App> = Host::new(app, width, 120);
    h.set_stylesheet(&[Rule::new(Selector::Id("label"), Patch::new().overflow(TextOverflow::Ellipsis))]);
    let mut m = FontMetrics::new(face.clone());
    h.pump_with(&mut m);
    let (shown, cut) = match who {
        Cutter::AppBar => h.app.ui.bar.shown_title(),
        Cutter::Label => h.app.ui.label.shown(),
    };
    cut.then_some(shown.len())
}

/// `(inside, near)` for a sample, swept over every width that cuts
/// somewhere in the title. `bounds` are the sample's internal grapheme
/// boundaries (byte offsets into it): a cut on one is not inside.
fn sweep(who: Cutter, cluster: &str, bounds: &[usize], face: &FontStack) -> (usize, usize) {
    let title = format!("WWWWWW{cluster}WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW");
    let (start, end) = (6, 6 + cluster.len());
    let (mut inside, mut near) = (0, 0);
    for width in 60..700 {
        let Some(c) = cut_at(who, &title, width, face) else { continue };
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
fn no_ellipsis_cut_lands_inside_a_cluster() {
    let face = face();
    let mut inside_any = Vec::new();
    for who in [Cutter::AppBar, Cutter::Label] {
        for (k, (name, cluster)) in CLUSTERS.iter().enumerate() {
            let (inside, near) = sweep(who, cluster, &[], &face);
            println!(
                "{who:?} {name}: cuts inside {inside} (bound 0; {} before T80.3), cuts near {near} (bound > 0)",
                BEFORE_T80_3[k]
            );
            assert!(near > 0, "{who:?} {name}: no width cut near the cluster -- the sweep proves nothing");
            if inside > 0 {
                inside_any.push(format!("{who:?} split the {name} {inside} times"));
            }
        }
    }
    assert!(inside_any.is_empty(), "a cut landed inside a grapheme cluster: {}", inside_any.join("; "));
}

#[test]
fn an_ascii_title_has_nothing_to_cut_inside() {
    // Accept direction: two one-byte graphemes, so every cut between them
    // is on a boundary and none lands inside one -- while the sweep still
    // cuts near them, so the 0 is not a sweep that cut nowhere.
    let face = face();
    for who in [Cutter::AppBar, Cutter::Label] {
        let (inside, near) = sweep(who, "ee", &[1], &face);
        println!("{who:?} ascii: cuts inside {inside} (bound 0), cuts near {near} (bound > 0)");
        assert_eq!(inside, 0, "{who:?}: an ASCII pair was cut inside");
        assert!(near > 0, "{who:?}: the ASCII sweep cut nowhere near the pair");
    }
}
