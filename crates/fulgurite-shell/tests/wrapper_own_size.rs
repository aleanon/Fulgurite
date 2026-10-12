//! **A hand-written wrapper lays its content out inside its own `Fixed`
//! size, not across what it was offered** — M57/B26.
//!
//! M56 found this in the derive (`ops::within_own_size`,
//! `crates/fulgurite/tests/fixed_container.rs`) and fixed it there.
//! The hand-written wrappers never asked for it: a `Fixed(220)`
//! `SearchField` in a 400 px row laid its field out 328 px wide, 148 px
//! past its own edge. `Card`, `Glass` and `ScrollView` had the same line.
//! Each is put here at `Fixed(200)` in a 400 px row around a `FILL`
//! slider, which must end inside it.

use fulgurite::style::{Length, Patch, Rule, Selector};
use fulgurite::{App as DeriveApp, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::{Card, Glass, ScrollView, SearchField, Slider};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 10.0)]
struct Ui {
    #[widget_id("card")]
    card: Card<Slider>,
    #[widget_id("glass")]
    glass: Glass<Slider>,
    #[widget_id("scroll")]
    scroll: ScrollView<Slider>,
    #[widget_id("search")]
    search: SearchField,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

#[test]
fn a_fixed_wrapper_keeps_its_content_inside_it() {
    let mut app = App::default();
    // Long enough to fill: a short query shrinks its field below either
    // width, and the leg could not see the defect (it read 112 both ways).
    app.ui.search = SearchField::new("a query long enough to fill any field it is typed into");
    let mut h: Host<App> = Host::new(app, 400, 600);
    let fixed = |id| Rule::new(Selector::Id(id), Patch::new().width(Length::Fixed(200.0)));
    h.set_stylesheet(&[fixed("card"), fixed("glass"), fixed("scroll"), fixed("search")]);
    h.pump();
    let table = <Ui as Widget>::SUB_TABLE;
    let mut over = Vec::new();
    for id in ["card", "glass", "scroll", "search"] {
        let w = fulgurite::style::index_of_wid(table, id).unwrap();
        let me = h.states[w];
        // The content is the last node of the wrapper's subtree: the slider,
        // or the search field's text field.
        let c = fulgurite::style::subtree_range(table, w).last().unwrap();
        let (end, edge) = (h.states[c].pos.x + h.states[c].size.w, me.pos.x + me.size.w);
        println!("{id:7} box {:>5.1} wide, content ends at {end:>5.1}, its edge at {edge:>5.1}", me.size.w);
        assert_eq!(me.size.w, 200.0, "setup: #{id} is not Fixed(200)");
        if end > edge + 0.5 {
            over.push(format!("#{id}: content ends at {end}, past its edge at {edge}"));
        }
    }
    assert!(over.is_empty(), "{}", over.join("\n"));
}
