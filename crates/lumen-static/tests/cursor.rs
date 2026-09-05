//! T14.6: the pointer shape as a style property.

use lumen_static::style::{Cursor, Patch, Rule, Selector};
use lumen_static::{pass, Style};

/// **The budget this design was gated on.** `Style` is resolved per
/// node, so a byte here is a byte times every node of every tree —
/// which is why the cursor is a `Style` field and not a `NodeState`
/// one, and why this assertion exists rather than a comment saying it
/// was free once.
#[test]
fn the_cursor_costs_nothing() {
    assert_eq!(core::mem::size_of::<Style>(), 72, "Style grew; every node grew with it");
    assert_eq!(core::mem::size_of::<lumen_static::NodeState>(), 76);
    assert_eq!(core::mem::size_of::<Cursor>(), 1);
}

#[test]
fn the_default_is_the_default_cursor() {
    assert_eq!(Style::default().cursor, Cursor::Default);
}

#[test]
fn a_stylesheet_rule_sets_it() {
    use lumen_static::ops::Label;
    use lumen_static::{StaticTree, Widget};

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Ui {
        #[widget_id("grabbable")]
        a: Label,
        b: Label,
    }

    let ui = Ui::default();
    let mut styles = pass::default_styles(&ui);
    let sheet = [Rule::new(Selector::Id("grabbable"), Patch::new().cursor(Cursor::Grab))];
    lumen_static::style::cascade_deep(&mut Ui::default(), &sheet, &mut styles);

    assert_eq!(styles[1].cursor, Cursor::Grab, "the rule reached it");
    assert_eq!(styles[2].cursor, Cursor::Default, "and nothing else");
    let _ = <Ui as Widget>::NODE_COUNT;
}

/// Widgets carry the shape their kind implies, so an application does
/// not write a rule to get an I-beam over a text field.
#[test]
fn the_widgets_that_should_have_one_do() {
    use lumen_static::ops::Button;
    use lumen_static::widget::Node;
    use lumen_static::{TextArea, TextInput};

    struct S;
    let field: TextInput = TextInput::default();
    let area: TextArea = TextArea::default();
    let button: Button<S> = Button::default();

    assert_eq!(field.defaults().cursor, Cursor::Text);
    assert_eq!(area.defaults().cursor, Cursor::Text);
    assert_eq!(button.defaults().cursor, Cursor::Pointer);
}
