// A container with no fields. Compiles today by accident; pinned
// deliberately.
use fulgurite::{StaticTree, Widget};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Empty {}

fn main() {
    assert_eq!(<Empty as Widget>::NODE_COUNT, 1);
}
