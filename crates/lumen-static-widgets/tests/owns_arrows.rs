//! **Which widgets own the arrow keys** — M44/T44.1
//! (`docs/plan-input-semantics.md` §2.1).
//!
//! The eight arrow-driven widgets answer `owns_arrows` `true`, so an
//! arrow on one never moves focus to a sibling; a control whose arrows
//! are navigation — a button, a checkbox — answers `false`, so a toolbar
//! still walks. Asked of the node at the widget's own index, as the pass
//! asks it.

use lumen_static::widget::Node;
use lumen_static_widgets::ops::{Button, Label};
use lumen_static_widgets::{
    CheckBox, NumberInput, PaneGrid, Pagination, RangeSlider, Slider, SplitPane, TextArea, TextInput,
};

fn owns(n: &dyn Node) -> bool {
    n.node(0).owns_arrows()
}

#[test]
fn the_arrow_driven_widgets_own_their_arrows() {
    let answers: [(&str, bool); 8] = [
        ("TextInput", owns(&TextInput::<String>::default())),
        ("TextArea", owns(&TextArea::<String>::default())),
        ("NumberInput", owns(&NumberInput::default())),
        ("Slider", owns(&Slider::default())),
        ("RangeSlider", owns(&RangeSlider::default())),
        ("SplitPane", owns(&SplitPane::<Label, Label>::default())),
        ("PaneGrid", owns(&PaneGrid::<Label, Label, Label, Label>::default())),
        ("Pagination", owns(&Pagination::default())),
    ];
    println!("{answers:?}");
    for (name, owns) in answers {
        assert!(owns, "{name} does not own its arrows, so an arrow on it moves focus to a sibling");
    }
}

#[test]
fn a_navigation_control_leaves_its_arrows_to_the_group() {
    #[derive(Default)]
    struct S;
    let b: Button<S> = Button::default();
    let c: CheckBox<S> = CheckBox::default();
    println!("Button {}, CheckBox {}", owns(&b), owns(&c));
    assert!(!owns(&b), "a Button took the arrows, so a toolbar cannot be walked");
    assert!(!owns(&c), "a CheckBox took the arrows, so a group of them cannot be walked");
}
