//! What a runtime stylesheet swap costs, at scale — the operation a
//! class toggle performs (M8c T8c.4).
use benches::{bench, Ten10};
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::{pass, Style, Widget};

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
}
