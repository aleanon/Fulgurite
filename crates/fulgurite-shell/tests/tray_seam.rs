//! **The tray's seam** — M90/T90.6 (`docs/plan-desktop-integration.md`,
//! "Model and state cross by data").
//!
//! A program's tray is data (`TraySpec`), installed by `Program::tray`
//! through `Program::host()`; the window reads it only as two owned
//! answers. `Session::menu_state` is `Some` exactly when the enabled mask
//! changed since it was last asked, and `Session::tray_model`'s hash moves
//! exactly when the spec's content does — across a swap to a build whose
//! spec differs, and not across one whose spec does not.
//!
//! Driven through `Local` and `hot::boot`, the session a window holds and
//! the path a hot library boots by. Each gate asserts its setup landed and
//! prints its quantity beside its bound.

use fulgurite::event::EventCtx;
use fulgurite::menu::{MenuEntry, MenuMask};
use fulgurite::prelude::*;
use fulgurite::Event;
use fulgurite_shell::program::{application, Program};
use fulgurite_shell::session::{hot, Local, Recovering, Session};
use fulgurite_shell::tray::TraySpec;
use fulgurite_widgets::prelude::*;

const W: u32 = 240;
const H: u32 = 120;

#[derive(StaticTree, Default, fulgurite::Reloadable)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("flip")]
    flip: Button<Kept>,
}

/// What the tray's enabled mask is computed from; kept across a swap.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Kept {
    /// Whether "Quit" (entry 1) is enabled.
    quit: bool,
    /// How many times the button flipped it — the setup's evidence.
    flips: u32,
}

#[derive(DeriveApp, fulgurite::Reloadable)]
struct Tray {
    #[tree]
    ui: Ui,
    #[state]
    #[reload(keep)]
    kept: Kept,
}

fn flip(k: &mut Kept, _: &EventCtx) {
    k.quit = !k.quit;
    k.flips += 1;
}

fn enabled(k: &Kept) -> MenuMask {
    MenuMask::ALL.with(1, k.quit)
}

const ICON: [u8; 16] = [0x20; 16];
const V1: [MenuEntry; 2] = [MenuEntry::new("Open", 1), MenuEntry::new("Quit", 2)];
const V2: [MenuEntry; 2] = [MenuEntry::new("Open", 1), MenuEntry::new("Quit now", 2)];

fn spec(menu: &'static [MenuEntry]) -> TraySpec<Tray> {
    TraySpec { icon: (2, 2, &ICON), tooltip: "tray-seam", menu, enabled }
}

fn build() -> Tray {
    let mut t = Tray { ui: Ui::default(), kept: Kept::default() };
    t.ui.flip.label = "flip";
    t.ui.flip.on_click = flip;
    t
}

fn program(menu: &'static [MenuEntry]) -> Program<Tray> {
    application(build).size(W, H).tray(spec(menu))
}

/// The button's centre, from the frame just laid out.
fn click_flip(s: &mut Local<Tray, Recovering>) {
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "flip").expect("#flip");
    let st = s.host.states[i];
    let (x, y) = (st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0);
    s.post(Event::down(x, y));
    s.post(Event::up(x, y));
}

/// A mask as its bits, for the printed lines: Quit is bit 1.
fn bits(m: Option<MenuMask>) -> String {
    m.map_or("None".into(), |m| format!("Some({:#x})", m.0))
}

/// `menu_state` over ten frames in which the enabled answer changes once
/// is `Some` exactly once; a frame that changes it again yields a second.
#[test]
fn menu_state_answers_once_per_change() {
    let mut s = Local::from_program(program(&V1), Recovering);
    let _ = s.frame(16.0, W, H);
    // The first ask after a session starts is `Some`: nothing has been
    // handed out yet, and a window applies it to the tray it just built.
    let first = s.menu_state();
    println!("first ask: {}", bits(first));
    assert_eq!(first, Some(MenuMask::ALL.with(1, false)), "the first ask must hand out the mask");

    let mut somes = Vec::new();
    for n in 0..10 {
        if n == 4 {
            click_flip(&mut s);
        }
        let _ = s.frame(16.0, W, H);
        if let Some(m) = s.menu_state() {
            somes.push((n, m));
        }
    }
    let flips = s.host.app.kept.flips;
    let shown: Vec<String> = somes.iter().map(|(n, m)| format!("frame {n} {}", bits(Some(*m)))).collect();
    println!("ten frames, one flip at frame 4: flips {flips}, Some answers {} (bound 1): {shown:?}", somes.len());
    assert_eq!(flips, 1, "setup: the click at frame 4 did not flip the state");
    assert_eq!(somes.len(), 1, "menu_state answered Some {} times over ten frames with one change", somes.len());
    assert_eq!(somes[0], (4, MenuMask::ALL), "the Some is the changed mask, on the frame that changed it");

    // Accept: a frame that changes it again yields a second `Some`.
    click_flip(&mut s);
    let _ = s.frame(16.0, W, H);
    let again = s.menu_state();
    println!("a second change: flips {}, answer {}", s.host.app.kept.flips, bits(again));
    assert_eq!(s.host.app.kept.flips, 2, "setup: the second click did not flip the state");
    assert_eq!(again, Some(MenuMask::ALL.with(1, false)), "a second change gave no second Some");
}

/// A program with no tray answers `None` to both, and never asks.
#[test]
fn no_tray_no_answers() {
    let p = application(build).size(W, H);
    let mut s = Local::from_program(p, Recovering);
    let _ = s.frame(16.0, W, H);
    let (model, state) = (s.tray_model(), s.menu_state());
    println!("no tray: model {model:?}, state {state:?}");
    assert!(model.is_none() && state.is_none(), "a program with no tray answered");
}

/// `tray_model` across a swap to a build whose spec differs returns the
/// new labels and a different hash; across one whose spec does not, and
/// across `Host::reload`, an equal hash.
#[test]
fn tray_model_follows_the_spec_across_a_swap() {
    let mut v1 = hot::boot(program(&V1), None);
    let _ = v1.frame(16.0, W, H);
    let m1 = v1.tray_model().expect("v1 declared a tray");
    let labels = |m: &fulgurite_shell::tray::TrayModel| m.entries.iter().map(|e| e.label.clone()).collect::<Vec<_>>();
    println!("v1: {:?}, tooltip {:?}, icon {}x{} ({} B), hash {:#018x}", labels(&m1), m1.tooltip, m1.icon.0, m1.icon.1, m1.icon.2.len(), m1.hash);
    assert_eq!(labels(&m1), ["Open", "Quit"], "setup: v1's model is not v1's spec");
    assert_eq!(m1.icon, (2, 2, ICON.to_vec()), "the icon is copied out whole");
    let saved = v1.save();

    // The swap's sequence: the new build's program, booted with the saved
    // state, as `export_program!`'s entry does.
    let mut v2 = hot::boot(program(&V2), Some(&saved));
    let _ = v2.frame(16.0, W, H);
    let m2 = v2.tray_model().expect("v2 declared a tray");
    println!("v2: {:?}, hash {:#018x}", labels(&m2), m2.hash);
    assert_eq!(labels(&m2), ["Open", "Quit now"], "after the swap the model still has v1's labels");
    assert_ne!(m1.hash, m2.hash, "two specs with different labels hashed equal");
    // And the new session hands its mask out again, so a window re-applies
    // it to the tray it rebuilt.
    let state = v2.menu_state();
    println!("v2's first menu_state: {}", bits(state));
    assert!(state.is_some(), "the swapped-in session did not hand its mask out");

    // Accept: the same build's spec hashes equal across a swap...
    let mut same = hot::boot(program(&V1), Some(&saved));
    let _ = same.frame(16.0, W, H);
    let m3 = same.tray_model().expect("v1 again");
    println!("v1 again: hash {:#018x} (v1 {:#018x})", m3.hash, m1.hash);
    assert_eq!(m3, m1, "the same spec gave a different model");

    // ...and across `Host::reload`, which rebuilds through the same boot
    // and keeps the spec `Program::host()` installed.
    let mut local = Local::from_program(program(&V1), Recovering);
    let before = local.tray_model().expect("a tray").hash;
    assert!(local.host.reload(), "setup: Host::reload had no boot to rebuild through");
    let after = local.tray_model().expect("a tray").hash;
    println!("Host::reload: hash {before:#018x} -> {after:#018x}");
    assert_eq!(before, after, "Host::reload changed the hash of an unchanged spec");
}

/// `Program::tray` refuses what a tray cannot represent, by a sentence
/// naming the numbers, and accepts the limit itself.
#[test]
fn program_tray_refuses_by_name() {
    static MANY: [MenuEntry; 65] = [MenuEntry::new("x", 0); 65];
    let caught = |menu: &'static [MenuEntry], icon: (u32, u32, &'static [u8])| {
        std::panic::catch_unwind(move || {
            let _ = application(build).tray(TraySpec { icon, tooltip: "t", menu, enabled });
        })
        .err()
        .map(|e| e.downcast_ref::<String>().cloned().unwrap_or_default())
    };
    let over = caught(&MANY, (2, 2, &ICON));
    println!("65 entries: {over:?}");
    assert_eq!(
        over.as_deref(),
        Some("a tray's menu holds at most 64 entries (one bit each of its MenuMask); this one has 65"),
        "Program::tray accepted 65 entries"
    );
    let at = caught(&MANY[..64], (2, 2, &ICON));
    println!("64 entries: {at:?}");
    assert!(at.is_none(), "Program::tray refused 64 entries");
    let icon = caught(&V1, (2, 2, &ICON[..12]));
    println!("a 2 x 2 icon of 12 B: {icon:?}");
    assert_eq!(
        icon.as_deref(),
        Some("a tray icon of 2 x 2 is 16 bytes of RGBA; this one has 12"),
        "Program::tray accepted a short icon"
    );
}
