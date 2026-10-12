//! **The tree a window hands its platform adapter** — M54/T54.2
//! (`docs/plan-a11y-platform.md` §2.2, R15-F1).
//!
//! What `Win` sends on `InitialTreeRequested` and after each frame is
//! `Session::a11y_tree` of a session booted from a `Program` — the same
//! `Local::from_program` the window builds. Two things are checked
//! without a window:
//!
//! - **every visible node of the table is in it**, under one window node
//!   named by the program's title (a root-only tree would announce an
//!   empty window);
//! - **bounds are physical**: at scale 2.0 a button's box is twice its
//!   logical one. A platform places a highlight in the window's physical
//!   pixels, so an unscaled tree puts it in the wrong place.
#![cfg(feature = "a11y")]

use fulgurite::observe::{accesskit::Role, WINDOW_NODE};
use fulgurite::prelude::*;
use fulgurite::widget::Widget;
use fulgurite_shell::program::application;
use fulgurite_shell::session::{Crashing, Local, Session};
use fulgurite_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    title: Label<String>,
    #[widget_id("save")]
    save: Button<App, String>,
    #[widget_id("agree")]
    agree: CheckBox<App>,
    /// Never shown: an absent branch is hidden, not dropped.
    extra: Option<Label<String>>,
}

#[derive(fulgurite::App, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn boot() -> App {
    let mut a = App::default();
    a.ui.title.set("A form".into());
    a.ui.save.label = "Save".into();
    a
}

fn session(scale: f32) -> Local<App, Crashing> {
    let program = application(boot).title("A11y tree").size(320, 200);
    let mut s = Local::from_program(program, Crashing);
    s.set_scale(scale);
    s.resize((320.0 * scale) as u32, (200.0 * scale) as u32);
    s.frame(0.0, (320.0 * scale) as u32, (200.0 * scale) as u32);
    s
}

#[test]
fn the_tree_holds_every_visible_node_under_a_named_window() {
    let s = session(1.0);
    let tree = s.a11y_tree().expect("an a11y build's session builds a tree").update;
    let table = <Ui as Widget>::SUB_TABLE.len();
    let visible = s.host.states.iter().filter(|st| st.size.w > 0.0 && st.size.h > 0.0).count();
    let emitted: Vec<_> = tree.nodes.iter().filter(|(id, _)| *id != WINDOW_NODE).collect();
    let shown = emitted.iter().filter(|(_, n)| !n.is_hidden()).count();
    println!(
        "tree: {} table nodes emitted ({shown} shown) / table {table} ({visible} visible), plus the window",
        emitted.len()
    );
    assert_eq!(emitted.len(), table, "the tree does not hold every table node");
    assert_eq!(shown, visible, "the tree shows {shown} nodes, the frame {visible}");
    let (_, window) = tree.nodes.iter().find(|(id, _)| *id == WINDOW_NODE).expect("no window node");
    println!("window: role {:?}, label {:?}, root {:?}", window.role(), window.label(), tree.tree.as_ref().map(|t| t.root));
    assert_eq!(window.role(), Role::Window);
    assert_eq!(window.label(), Some("A11y tree"));
    assert_eq!(tree.tree.map(|t| t.root), Some(WINDOW_NODE), "the window is not the tree's root");
    let buttons = emitted.iter().filter(|(_, n)| n.role() == Role::Button).count();
    assert_eq!(buttons, 1, "setup: the form's one button is not a Button");
}

#[test]
fn bounds_are_in_physical_pixels() {
    let save = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "save").unwrap();
    // The same session's logical box: text is measured at the device
    // scale, so a 1.0 session's box is not the one to double.
    let two = session(2.0);
    let logical = two.host.states[save];
    let b = two
        .a11y_tree()
        .unwrap()
        .update
        .nodes
        .iter()
        .find(|(id, _)| id.0 == save as u64 + 1)
        .and_then(|(_, n)| n.bounds())
        .expect("the button has no bounds");
    let (w, h) = (b.x1 - b.x0, b.y1 - b.y0);
    println!(
        "button at scale 2.0: {w:.1} x {h:.1} at ({:.1}, {:.1}); logical {:.1} x {:.1} at ({:.1}, {:.1})",
        b.x0, b.y0, logical.size.w, logical.size.h, logical.pos.x, logical.pos.y
    );
    assert!(logical.size.w > 0.0, "setup: the button was not laid out");
    assert!((w - 2.0 * logical.size.w as f64).abs() < 0.5, "width {w} is not twice {}", logical.size.w);
    assert!((h - 2.0 * logical.size.h as f64).abs() < 0.5, "height {h} is not twice {}", logical.size.h);
    assert!((b.x0 - 2.0 * logical.pos.x as f64).abs() < 0.5, "x {} is not twice {}", b.x0, logical.pos.x);
}
