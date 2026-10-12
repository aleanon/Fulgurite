//! Step 5 correctness: AccessKit snapshot shape, roles/values/bounds,
//! repeater instances with row-stable ids, source provenance, and the
//! agent-facing JSON dump.

use accesskit::{NodeId, Role};
use fulgurite::observe::{dump, index_of_node, snapshot, snapshot_focused};
use fulgurite_widgets::ops::{Fixed, Label, Spacer};
use fulgurite::repeat::Repeat;
use fulgurite::{pass, Constraints, NodeState, StaticTree, Widget};

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
    app.bar.title.set("hello");
    app.footer.set("bye");
    let mut st = [NodeState::default(); App::NODE_COUNT];
    let sty = pass::default_styles(&app);
    pass::layout(&mut app, &sty, &mut st, VIEW);
    (app, st)
}

#[test]
fn snapshot_shape_roles_and_bounds() {
    let (app, st) = make_app(100);
    let up = snapshot(&app, &st);

    // Main table + the *visible* window instances.
    //
    // Not every materialized row: a repeater's window overruns its
    // viewport by construction, and since 2026-09-02 those rows are
    // excluded from the snapshot because paint clips them and hit
    // rejects them — announcing them put nodes in the AccessKit tree at
    // bounds where nothing draws and no click lands
    // (`docs/plan-review-fixes.md` F4.7).
    let win = app.list.window_len();
    assert!(win > 0);
    let announced = (up.nodes.len() - App::NODE_COUNT) / RowT::NODE_COUNT;
    assert!(announced > 0, "some rows are on screen");
    assert!(announced <= win, "announced {announced} of {win} materialized");
    assert_eq!(
        up.nodes.len(),
        App::NODE_COUNT + announced * RowT::NODE_COUNT,
        "the snapshot is the main table plus whole row subtrees"
    );
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

    // The repeater is a List whose children are the *visible* window
    // instances — same reason as the node count above (F4.7).
    let list = get((list_idx + 1) as u64);
    assert_eq!(list.role(), Role::List);
    assert_eq!(list.children().len(), announced);
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
    let sty = pass::default_styles(&app);
    pass::layout(&mut app, &sty, &mut st, VIEW);
    let up2 = snapshot(&app, &st);
    let ids_after = first_ids(&up2);

    // Ids follow the rows, so the still-visible rows keep theirs: after
    // scrolling five rows the two windows overlap, and the overlapping
    // ids are identical element for element.
    assert!(ids_before.len() > 5 && ids_after.len() > 5);
    let overlap = (ids_before.len() - 5).min(ids_after.len());
    assert!(overlap > 0);
    assert_eq!(ids_after[..overlap], ids_before[5..5 + overlap]);
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

// ---- T7.5: what the platform adapter needs ---------------------------

#[test]
fn focus_reaches_the_snapshot_and_round_trips_to_an_index() {
    let (app, st) = make_app(3);

    // No focus: the root, which is how a shell without focus tracking
    // reports "nothing is focused".
    assert_eq!(snapshot(&app, &st).focus, NodeId(1));
    assert_eq!(snapshot_focused(&app, &st, None).focus, NodeId(1));

    // A focused node is announced as itself, and the id the adapter
    // sends back in an action request maps to the same index.
    for i in 0..App::NODE_COUNT as u32 {
        let id = snapshot_focused(&app, &st, Some(i)).focus;
        assert_eq!(id, NodeId(i as u64 + 1), "node {i} announces itself");
        assert_eq!(index_of_node::<App>(id), Some(i as usize), "and maps back");
    }
}

#[test]
fn an_out_of_range_focus_is_reported_as_no_focus_not_as_a_dangling_id() {
    let (app, st) = make_app(3);
    // A stale focus index — a hot reload shrank the tree, say — must not
    // put an id in the tree that names no node: a screen reader
    // following it would land nowhere and stop announcing.
    let up = snapshot_focused(&app, &st, Some(App::NODE_COUNT as u32 + 5));
    assert_eq!(up.focus, NodeId(1));
    assert!(up.nodes.iter().any(|(id, _)| *id == up.focus), "focus names a node in the tree");
}

#[test]
fn ids_an_adapter_could_invent_do_not_map_to_nodes() {
    // `index_of_node` is fed ids from outside the process. 0 is not a
    // node (ids are index + 1), and neither is anything past the table.
    assert_eq!(index_of_node::<App>(NodeId(0)), None, "0 is not index -1");
    assert_eq!(index_of_node::<App>(NodeId(App::NODE_COUNT as u64 + 1)), None);
    assert_eq!(index_of_node::<App>(NodeId(u64::MAX)), None, "no overflow");
    assert_eq!(index_of_node::<App>(NodeId(1)), Some(0), "the root");
}

#[test]
fn every_announced_focus_target_has_bounds_a_reader_can_point_at() {
    let (app, st) = make_app(3);
    let up = snapshot_focused(&app, &st, Some(1));
    let (_, node) = up.nodes.iter().find(|(id, _)| *id == up.focus).expect("focus is in the tree");
    // An action request arrives with a node id and the shell turns it
    // into a click at the node's centre; without bounds there is no
    // centre to click.
    assert!(node.bounds().is_some(), "the focused node carries bounds");
}
