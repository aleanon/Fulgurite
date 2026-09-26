//! **A swap keeps what the user did, never what the code wrote** —
//! M49/T49.0, R5-F1 (`docs/plan-reload-hardening.md` §1.4, §2.3).
//!
//! v1 and v2 differ only in what their build function writes: a button's
//! label, a static label's text, a repeater's row height and count. That
//! is an **edit to the code**, and after a swap the frame must show it.
//! The one value a user changed — the `#[state]` count — must keep v1's.
//!
//! The tree is in the derive's default mode, keep, which is the mode
//! T49.5 teaches. Before T49.5 a kept `Button` saved its label and a kept
//! `Label` its text, so an edit to either never appeared after a swap,
//! with no log line to say so.

use lumen_static::event::EventCtx;
use lumen_static::prelude::*;
use lumen_static::reload::{take_log, Reloadable};
use lumen_static::repeat::Repeat;
use lumen_static_widgets::prelude::*;

fn no_bind(_: &mut Label, _: usize) {}

#[derive(StaticTree, Default, lumen_static::Reloadable)]
#[widget(column, spacing = 0.0)]
pub struct Ui {
    pub save: Button<u32, String>,
    pub title: Label<String>,
    pub rows: Repeat<Label>,
    pub value: Label<String>,
}

#[derive(DeriveApp, lumen_static::Reloadable)]
pub struct Page {
    #[tree]
    pub ui: Ui,
    #[state]
    #[reload(keep)]
    #[bind(ui.value, "count {}")]
    pub count: u32,
}

fn on_save(n: &mut u32, _: &EventCtx) {
    *n += 1;
}

/// The build, parameterised by the edit: `v2` is what the same function
/// says after the source changed.
fn build(v2: bool) -> Page {
    let mut ui = Ui {
        rows: Repeat::new(if v2 { 12 } else { 8 }, if v2 { 30.0 } else { 20.0 }, no_bind),
        ..Ui::default()
    };
    ui.save.label = String::from(if v2 { "Store" } else { "Save" });
    ui.save.on_click = on_save;
    ui.title.set(String::from(if v2 { "Title v2" } else { "Title v1" }));
    Page { ui, count: 0 }
}

#[test]
fn a_swap_shows_the_new_codes_values_and_keeps_the_users() {
    let mut v1 = build(false);
    v1.count = 5;
    let saved = v1.save();
    let _ = take_log();
    let mut after = build(true);
    after.restore(&saved);
    let log = take_log();
    let v2 = build(true);

    let rows: [(&str, String, String); 5] = [
        ("Button.label", v2.ui.save.label.clone(), after.ui.save.label.clone()),
        ("Label (static) text", v2.ui.title.as_str().to_string(), after.ui.title.as_str().to_string()),
        ("Repeat.item_extent", v2.ui.rows.item_extent.to_string(), after.ui.rows.item_extent.to_string()),
        ("Repeat.count", v2.ui.rows.count.to_string(), after.ui.rows.count.to_string()),
        ("#[state] count", "5 (v1's)".to_string(), after.count.to_string()),
    ];
    let mut stale = Vec::new();
    for (name, want, got) in &rows[..4] {
        let ok = want == got;
        println!("{name:<22} v2 says {want:>10}, after reload {got:>10}  {}", if ok { "ok" } else { "KEPT v1" });
        if !ok {
            stale.push(*name);
        }
    }
    println!("{:<22} v1 set  {:>10}, after reload {:>10}", rows[4].0, rows[4].1, rows[4].2);
    println!("{} of 4 code rows kept v1's value; {} log lines", stale.len(), log.len());
    assert_eq!(after.count, 5, "the user's #[state] value must survive the swap");
    assert!(stale.is_empty(), "a swap restored what v1's code wrote: {stale:?}");
}
