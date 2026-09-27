//! **One construction path, in process and in a window** — M49/T49.4,
//! R5-F10 (`docs/plan-reload-hardening.md` §2.2).
//!
//! A window's swap boots the library's `program()` and restores onto it
//! (`hot::boot`); an in-process `Host::reload` used to call
//! `Reloadable::fresh`, which the derive routed to `Application::build` —
//! a second constructor nothing checked against the boot. Now both go
//! through the program's boot. This drives the same program both ways,
//! with a click before and after each rebuild, and requires the two
//! frames to agree command for command. The click after the rebuild is
//! what makes it a construction test: a rebuilt application whose
//! handler was not wired by the boot would not count it.

use lumen_static::display::Cmd;
use lumen_static::event::EventCtx;
use lumen_static::prelude::*;
use lumen_static::Event;
use lumen_static_reload::hot;
use lumen_static_shell::program::{application, Program};
use lumen_static_shell::session::{FrameSource, Local, Recovering, Session};
use lumen_static_widgets::prelude::*;
use serde::{Deserialize, Serialize};

const W: u32 = 240;
const H: u32 = 120;
const AT: (f32, f32) = (120.0, 20.0);

#[derive(StaticTree, Default, lumen_static::Reloadable)]
#[widget(column, spacing = 8.0)]
pub struct Ui {
    #[widget_id("inc")]
    pub inc: Button<Tally>,
    #[widget_id("value")]
    pub value: Label<String>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Tally {
    pub count: i64,
}

#[derive(DeriveApp, lumen_static::Reloadable)]
pub struct Counter {
    #[tree]
    pub ui: Ui,
    #[state]
    #[reload(keep)]
    #[bind(ui.value, count, "count {}")]
    pub tally: Tally,
}

fn on_inc(t: &mut Tally, _: &EventCtx) {
    t.count += 1;
}

/// A free function: no constructor trait at all.
fn boot() -> Counter {
    let mut c = Counter { ui: Ui::default(), tally: Tally::default() };
    c.ui.inc.label = "+";
    c.ui.inc.on_click = on_inc;
    c
}

fn program() -> Program<Counter> {
    use lumen_static::style::{Length, Patch, Rule, Selector};
    application(boot).size(W, H).style(|_: &Counter| {
        vec![Rule::new(Selector::Id("inc"), Patch::new().width(Length::FILL).height(Length::Fixed(40.0)))]
    })
}

fn click(s: &mut dyn Session) {
    s.post(Event::down(AT.0, AT.1));
    s.post(Event::up(AT.0, AT.1));
    s.frame(16.0, W, H);
    s.post(Event::PointerMove { x: 230.0, y: 115.0 });
    s.frame(16.0, W, H);
}

fn text(s: &dyn Session) -> String {
    let l = s.list();
    l.cmds
        .iter()
        .filter_map(|c| if let Cmd::Text { text, .. } = c { Some(l.str_of(*text).to_string()) } else { None })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// What a window installs (M49/T49.10: the host's work in flight rides
/// beside the application's state).
const SAVER: lumen_static_shell::session::Saver<Counter> = hot::save::<Counter>;

#[test]
fn host_reload_and_a_window_swap_build_the_same_frame() {
    // In process: click, `Host::reload`, click.
    let mut a = Local::from_program(program(), Recovering).with_saver(SAVER);
    a.frame(0.0, W, H);
    click(&mut a);
    let reloaded = a.host.reload();
    println!("Host::reload rebuilt through the boot: {reloaded}");
    assert!(reloaded, "Host::reload had nothing to rebuild from: the host did not keep the program's boot");
    a.frame(16.0, W, H);
    click(&mut a);

    // A window's swap: click, save, the library side boots the program
    // and restores, click.
    let mut b0 = Local::from_program(program(), Recovering).with_saver(SAVER);
    b0.frame(0.0, W, H);
    click(&mut b0);
    let saved = b0.save();
    let mut b = hot::boot(program(), Some(&saved));
    b.frame(0.0, W, H);
    click(&mut *b);

    let (ta, tb) = (text(&a), text(&*b));
    println!("Host::reload: {} commands, `{ta}`", a.list().cmds.len());
    println!("window swap:  {} commands, `{tb}`", b.list().cmds.len());
    assert!(ta.contains("count 2"), "a click did not land after Host::reload: `{ta}`");
    assert!(tb.contains("count 2"), "a click did not land after the swap: `{tb}`");
    assert_eq!(a.list().cmds, b.list().cmds, "the two construction paths drew different frames");
    assert_eq!(a.list().text, b.list().text);
}

/// The other half of "reports that rather than guessing": a host built
/// with `Host::new`, not from a program, has no boot.
#[test]
fn a_host_not_built_from_a_program_says_it_cannot_reload() {
    let mut h: lumen_static_shell::Host<Counter> = lumen_static_shell::Host::new(boot(), W, H);
    h.app.tally.count = 4;
    assert!(!h.reload(), "a host with no boot claimed to reload");
    assert_eq!(h.app.tally.count, 4, "a refused reload changed the application");
}
