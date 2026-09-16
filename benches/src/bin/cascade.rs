//! What a runtime stylesheet swap costs, at scale — the operation a
//! class toggle performs (M8c T8c.4).
use benches::{bench, Ten10};
use lumen_static::style::{Edges, Length, Patch, Rule, Selector};
use lumen_static::{pass, Color, Style, Widget};

fn main() {
    let sheet: Vec<Rule> = vec![
        Rule::new(Selector::Kind("Ten10"), Patch::new().font_size(15.0)),
        Rule::new(Selector::Kind("Leaf"), Patch::new().font_size(16.0)),
    ];
    let mut app = Ten10::default();
    let mut styles: Vec<Style> = pass::default_styles(&app);
    println!("cascade: a runtime stylesheet swap, {} nodes\n", Ten10::NODE_COUNT);
    bench("cascade_deep (whole table)", 20_000, || {
        lumen_static::style::cascade_deep(&mut app, &sheet, &mut styles);
        styles.len() as u64
    });

    // What a class toggle costs when it is scoped to the subtree it
    // actually changes (M8c, T8c.4). The fixture is ten columns of ten;
    // one of them is a tenth of the tree.
    let table = <Ten10 as Widget>::SUB_TABLE;
    let root = table
        .iter()
        .enumerate()
        .find(|(i, d)| *i > 0 && d.is_container())
        .map(|(i, _)| i)
        .expect("a container below the root");
    let span = lumen_static::style::subtree_range(table, root).len();
    bench(&format!("cascade_subtree ({span} of {} nodes)", table.len()), 20_000, || {
        lumen_static::style::cascade_subtree(&mut app, &sheet, &mut styles, root);
        span as u64
    });
    println!("   the scoped form is what a `#[widget_id]` toggle should call");

    // **A showcase-shaped sheet** — M25/T25.1.
    //
    // The sheet above is two `Kind` rules, and `Kind("Leaf")` names no
    // kind in this tree, so `apply_to` runs about once per cascade and a
    // change to `Patch`'s size could not move the number. The showcase's
    // real sheet is 24 `Id` rules and 3 `Kind` rules, each setting several
    // fields. This one has that shape against this tree: every `Kind`
    // rule matches a kind that is present, and the `Id` rules match
    // nothing — `Ten10` carries no ids — so they cost their selector test
    // and no more, exactly as the showcase's do for every node they miss.
    let mut showcase: Vec<Rule> = (0..24)
        .map(|k| {
            let id: &'static str = Box::leak(format!("id{k}").into_boxed_str());
            Rule::new(
                Selector::Id(id),
                Patch::new().padding(Edges::all(4.0)).font_size(13.0).font_weight(600.0),
            )
        })
        .collect();
    showcase.push(Rule::new(
        Selector::Kind("label"),
        Patch::new().color(Color(20, 24, 34, 255)).font_size(14.0),
    ));
    showcase.push(Rule::new(
        Selector::Kind("fixed"),
        Patch::new().background(Color(226, 232, 240, 255)).radius(6.0).width(Length::Fixed(20.0)),
    ));
    showcase.push(Rule::new(
        Selector::Kind("Block"),
        Patch::new().padding(Edges::all(8.0)).background(Color(250, 250, 252, 255)).radius(8.0),
    ));
    let matched = table.iter().enumerate().filter(|(i, _)| {
        matches!(<Ten10 as Widget>::SEEDS[*i].kind, "label" | "fixed" | "Block")
    });
    println!(
        "\nshowcase-shaped sheet: {} rules; the 3 Kind rules match {} of {} nodes",
        showcase.len(),
        matched.count(),
        table.len()
    );
    bench("cascade_deep (showcase-shaped sheet)", 20_000, || {
        lumen_static::style::cascade_deep(&mut app, &showcase, &mut styles);
        styles.len() as u64
    });

    // ---- the sheet lint — M26/T26.0 ------------------------------------
    //
    // `lint_sheet_deep` runs on the same event the rows above do: a host
    // being handed a sheet. It walks every arena once *per rule*
    // (M26/T26.17 #10), and nothing has ever priced it — so a change
    // there has had no way to be judged.
    let mut found = 0u64;
    bench("lint_sheet_deep (showcase-shaped sheet)", 20_000, || {
        found = 0;
        lumen_static::style::lint_sheet_deep(&mut app, &showcase, &mut |_| found += 1);
        found
    });
    println!(
        "   {found} findings — `Ten10` carries no ids, so every `Id` rule is reported unmatched"
    );
}
