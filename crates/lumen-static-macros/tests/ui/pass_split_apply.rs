//! M33/T33.2: `apply` receives the state, and for a split application
//! the tree is not reachable from it. The other direction of `fail_split_apply_reaches_tree.rs`: identical but for `apply`'s body, which writes the state.
use lumen_static::event::App;
use lumen_static::StaticTree;
use lumen_static_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    title: Label,
}

#[derive(Default)]
struct Work {
    landed: Vec<u64>,
}

struct Split {
    ui: Ui,
    state: Work,
}

impl App for Split {
    type State = Work;
    fn state_mut(&mut self) -> &mut Work {
        &mut self.state
    }
    const STATE_IS_SPLIT: bool = true;
    type Tree = Ui;
    type Msg = u64;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    fn apply(s: &mut Work, m: u64) -> bool {
        s.landed.push(m);
        true
    }
}

fn main() {}
