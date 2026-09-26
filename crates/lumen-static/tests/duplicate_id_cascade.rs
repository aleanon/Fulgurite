//! M46/T46.2, R2-F7: a scoped cascade by an id that names two nodes.
//!
//! `cascade_wid_with` re-cascades every match and runs the debug witness
//! once, over the union — so a correct call does not panic. And a caller
//! that still scopes to *one* of the two gets a witness message naming
//! the duplicated id, not one blaming the sheet.

use lumen_static::style::{
    cascade_deep, cascade_subtree, cascade_wid_with, count_wid, index_of_wid, Palette, Patch, Rule,
    Selector,
};
use lumen_static::{pass, StaticTree, Widget};
use lumen_static_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Inner {
    #[widget_id("leaf")]
    leaf: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    x: Inner,
    y: Inner,
}

fn sheet() -> Vec<Rule> {
    vec![Rule::new(Selector::Id("leaf"), Patch::new().font_size(40.0))]
}

#[test]
fn every_match_is_restyled_and_the_witness_is_quiet() {
    let t = Ui::SUB_TABLE;
    assert_eq!(count_wid(t, "leaf"), 2, "setup: two copies of the id");
    let mut ui = Ui::default();
    let mut out = pass::default_styles(&ui);
    cascade_deep(&mut ui, &[], &mut out);
    let n = cascade_wid_with(&mut ui, &sheet(), &Palette::LIGHT, &mut out, "leaf");
    let sizes: Vec<f32> = (0..t.len()).filter(|&i| t[i].wid == "leaf").map(|i| out[i].font_size).collect();
    println!("cascade_wid_with -> {n} roots; font sizes {sizes:?} / bound [40, 40]");
    assert_eq!(n, 2);
    assert_eq!(sizes, [40.0, 40.0]);
    assert_eq!(cascade_wid_with(&mut ui, &sheet(), &Palette::LIGHT, &mut out, "nope"), 0);
}

#[test]
#[should_panic(expected = "`#[widget_id(\"leaf\")]` is on 2 nodes")]
#[cfg(debug_assertions)]
fn scoping_to_one_of_two_names_the_duplicate_not_the_sheet() {
    let mut ui = Ui::default();
    let mut out = pass::default_styles(&ui);
    cascade_deep(&mut ui, &[], &mut out);
    let first = index_of_wid(Ui::SUB_TABLE, "leaf").unwrap();
    cascade_subtree(&mut ui, &sheet(), &mut out, first);
}
