//! Step 5 correctness: AccessKit snapshot shape, roles/values/bounds,
//! repeater instances with row-stable ids, source provenance, and the
//! agent-facing JSON dump.

use accesskit::{NodeId, Role};
use lumen_static::observe::{dump, snapshot};
use lumen_static::ops::{Fixed, Label, Spacer};
use lumen_static::repeat::Repeat;
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
struct RowT {
    icon: Fixed,
    name: Label,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
struct Bar {
    title: Label,
    pad: Spacer,
    badge: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct App {
    bar: Bar,
    list: Repeat<RowT>,
    footer: Label,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };

fn bind_name(item: &mut RowT, row: usize) {
    item.icon.w = row as f32 + 1.0;
}

fn make_app(rows: usize) -> (App, [NodeState; App::NODE_COUNT]) {
    let mut app = App { list: Repeat::new(rows, 24.0, bind_name), ..Default::default() };
    app.bar.title.text = "hello";
    app.footer.text = "bye";
    let mut st = [NodeState::default(); App::NODE_COUNT];
    pass::layout(&mut app, &mut st, VIEW);
    (app, st)
}

#[test]
fn snapshot_shape_roles_and_bounds() {
    let (app, st) = make_app(100);
    let up = snapshot(&app, &st);

    // Main table (7 nodes) + window instances (window * 3 template nodes).
    let win = app.list.window_len();
    assert!(win > 0);
    assert_eq!(up.nodes.len(), App::NODE_COUNT + win * RowT::NODE_COUNT);
    assert_eq!(up.tree.unwrap().root, NodeId(1));

    let get = |id: u64| &up.nodes.iter().find(|(n, _)| n.0 == id).unwrap().1;

    // Root (table 0 → id 1): container with bar, list, footer as children.
    let root = get(1);
    assert_eq!(root.role(), Role::GenericContainer);
    let kids: Vec<u64> = root.children().iter().map(|c| c.0).collect();
    // bar at table 1, list at 1 + Bar::NODE_COUNT, footer after the repeater node.
    let list_idx = 1 + Bar::NODE_COUNT;
    assert_eq!(kids, vec![2, (list_idx + 1) as u64, (list_idx + 2) as u64]);

    // The title label carries role + value.
    let title = get(3); // table: 0=app, 1=bar, 2=bar.title
    assert_eq!(title.role(), Role::Label);
    assert_eq!(title.value().unwrap(), "hello");

    // Bounds are absolute and match layout state.
    let b = title.bounds().unwrap();
    assert_eq!(b.x0 as f32, st[2].pos.x);
    assert_eq!((b.x1 - b.x0) as f32, st[2].size.w);

    // The repeater is a List whose children are the window instances.
    let list = get((list_idx + 1) as u64);
    assert_eq!(list.role(), Role::List);
    assert_eq!(list.children().len(), win);
}

#[test]
fn repeater_instance_ids_are_row_stable() {
    let (mut app, mut st) = make_app(10_000);
    let up1 = snapshot(&app, &st);
    let list_id = (1 + Bar::NODE_COUNT + 1) as u64;
    let first_ids = |up: &accesskit::TreeUpdate| -> Vec<u64> {
        up.nodes
            .iter()
            .find(|(n, _)| n.0 == list_id)
            .unwrap()
            .1
            .children()
            .iter()
            .map(|c| c.0)
            .collect()
    };
    let ids_before = first_ids(&up1);

    // Scroll by exactly 5 rows: the window shifts, ids follow the ROWS —
    // 5 ids survive (rows still visible), and none are renumbered.
    app.list.scroll = 5.0 * 24.0;
    pass::layout(&mut app, &mut st, VIEW);
    let up2 = snapshot(&app, &st);
    let ids_after = first_ids(&up2);

    assert_eq!(ids_after[..ids_before.len() - 5], ids_before[5..]);
    assert!(ids_after.iter().all(|id| id & (1 << 63) != 0), "synthetic id space");
}

#[test]
fn provenance_points_at_declaration_sites() {
    // Every table entry carries the file it was declared in…
    for d in App::SUB_TABLE {
        assert!(d.src.file.ends_with("tests/observe.rs"), "src file: {}", d.src.file);
        assert!(d.src.line > 0);
    }
    // …and lines follow declaration order: struct App, then its fields.
    let root = App::SUB_TABLE[0].src.line;
    let bar = App::SUB_TABLE[1].src.line;
    let list = App::SUB_TABLE[1 + Bar::NODE_COUNT].src.line;
    let footer = App::SUB_TABLE[1 + Bar::NODE_COUNT + 1].src.line;
    assert!(root < bar && bar < list && list < footer, "{root} {bar} {list} {footer}");

    // A nested component's *internals* point at the component's own
    // declaration, not the use site: bar.title comes from struct Bar.
    let bar_title = App::SUB_TABLE[2].src.line;
    assert!(bar_title < root, "Bar is declared above App in this file");
}

#[test]
fn dump_is_agent_readable() {
    let (app, st) = make_app(100);
    let d = dump(&app, &st);
    let text = serde_json::to_string(&d).unwrap();

    assert!(text.contains("tests/observe.rs:"), "provenance in dump");
    assert!(text.contains("\"role\":\"Label\""), "roles in dump");
    assert!(text.contains("\"value\":\"hello\""), "values in dump");
    assert_eq!(d["root"], 1);
}
