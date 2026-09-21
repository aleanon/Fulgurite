use lumen_static_widgets::ops::Label;
use lumen_static::StaticTree;

// The agent's stable address: a dropped one fails far from the typo, so
// it must not be silently empty.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct BadId {
    #[widget_id(save)]
    a: Label,
}

fn main() {}
