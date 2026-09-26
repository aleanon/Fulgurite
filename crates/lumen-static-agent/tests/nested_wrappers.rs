//! **A wrapper of a wrapper is live** — M48/T48.0 and T48.7 (R13-F5,
//! `docs/plan-agent-truth.md` §2.3).
//!
//! `live_nodes` called a node a stand-in when `node(i)` returned the same
//! address *and* kind as its parent's, and its rustdoc said the residue
//! "needs a type to contain itself". `Glass<Glass<Label>>` is not that:
//! the inner glass sits at offset 0 of the outer one with the same kind,
//! so it and everything under it were reported hidden, and the journal
//! stopped seeing the label (the review's probe c9d6760). Card and
//! ScrollView have the same shape.
//!
//! Each case asserts every node live and exactly one change after the
//! label's text is set, printing `live=<vec>, changes=N/1`.

use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, FrameTiming, TreeMut, TreeRef};
use lumen_static_widgets::{Card, Glass, Label, ScrollView};
use serde_json::json;

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 300.0 };

macro_rules! nested {
    ($name:ident, $test:ident, $ty:ty, $label:ident) => {
        #[derive(StaticTree, Default)]
        #[widget(column, spacing = 0.0)]
        struct $name {
            #[widget_id("w")]
            w: $ty,
        }

        #[test]
        #[ignore = "red until T48.7: R13-F5, a same-kind wrapper at offset 0 reads as a stand-in"]
        fn $test() {
            let mut ui = $name::default();
            fn $label(ui: &mut $name) -> &mut Label<String> {
                &mut ui.w.content.content
            }
            $label(&mut ui).set("hello".into());
            let styles = pass::default_styles(&ui);
            let mut states = vec![NodeState::default(); $name::NODE_COUNT];
            pass::layout(&mut ui, &styles, &mut states, VIEW);
            let table = <$name as Widget>::SUB_TABLE;
            let mut live = Vec::new();
            lumen_static_agent::live_nodes(&ui, table, &mut live);

            let mut agent = Agent::new();
            agent.frame_done(&mut TreeMut::of_root(&mut ui, &styles, &states), FrameTiming::default());
            let since = agent.frame;
            $label(&mut ui).set("world".into());
            pass::layout(&mut ui, &styles, &mut states, VIEW);
            agent.frame_done(&mut TreeMut::of_root(&mut ui, &styles, &states), FrameTiming::default());
            let tree = TreeRef::of_root(&ui, &styles, &states);
            let r = agent.dispatch(
                &tree,
                &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lastChange", "params": { "since": since } }),
            );
            let changes = r["result"]["changed"].as_array().map_or(0, |a| a.len());
            println!("{}: live={live:?}, changes={changes}/1", stringify!($ty));
            assert!(live.iter().all(|l| *l), "{}: a live node was reported a stand-in: {live:?}", stringify!($ty));
            assert_eq!(changes, 1, "{}: the label's change was not journaled: {r}", stringify!($ty));
        }
    };
}

nested!(Glasses, nested_glass_is_live, Glass<Glass<Label<String>>>, label);
nested!(Cards, nested_card_is_live, Card<Card<Label<String>>>, label);
nested!(Scrolls, nested_scrollview_is_live, ScrollView<ScrollView<Label<String>>>, label);
