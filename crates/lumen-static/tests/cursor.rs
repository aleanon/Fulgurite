//! T14.6: the pointer shape as a style property.

use lumen_static::style::{Cursor, Patch, Rule, Selector};
use lumen_static::{pass, Style};

/// **The budget this design was gated on.** `Style` is resolved per
/// node, so a byte here is a byte times every node of every tree —
/// which is why the cursor is a `Style` field and not a `NodeState`
/// one, and why this assertion exists rather than a comment saying it
/// was free once.
///
/// **80 since M27/T27.1, and re-budgeted rather than edited.** The
/// cursor really was free: at 72 B it fit the tail padding beside
/// `align`, `focus_ring` and `visible`. That slot is now *exactly* full
/// — four `u8`s in four bytes, zero spare — so the border's
/// `border_width: u8` could not fit and cost four bytes, and
/// `border_color: Color` four more. This comment's predecessor said a
/// field "either fits or gets re-budgeted rather than paid for
/// silently"; the measurement that discharges that is in
/// `docs/results-embedded.md` — **`.text` +328 B, `.rodata` +4 B on
/// `thumbv7em-none-eabihf`, and per-node RAM 148 B → 156 B**.
///
/// The alternative was to move `align` out, since only containers read
/// it — that frees one byte of the slot and would have made
/// `border_width` free. It was declined: `i18n::mirror_styles` rewrites
/// `align` in one flat pass over the style array, and per-widget
/// alignment turns RTL mirroring into a tree walk through a trait method
/// every custom container must remember to implement.
/// `docs/plan-widget-parity.md` T27.1 has the argument.
#[test]
fn the_cursor_costs_nothing() {
    // M80/T80.2 took one spare byte of each (`line_height`), and neither
    // struct grew: printed so a later field that does shows its cost.
    let (style, state) = (core::mem::size_of::<Style>(), core::mem::size_of::<lumen_static::NodeState>());
    println!(
        "Style {style} B (bound 80), NodeState {state} B (bound 84): {} B/node, {:.1} KB per 100 nodes",
        style + state,
        (style + state) as f32 * 100.0 / 1024.0
    );
    assert_eq!(core::mem::size_of::<Style>(), 80, "Style grew; every node grew with it");
    assert_eq!(core::mem::size_of::<lumen_static::NodeState>(), 84);
    assert_eq!(core::mem::size_of::<Cursor>(), 1);
}

#[test]
fn the_default_is_the_default_cursor() {
    assert_eq!(Style::default().cursor, Cursor::Default);
}

#[test]
fn a_stylesheet_rule_sets_it() {
    use lumen_static_widgets::ops::Label;
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
    use lumen_static_widgets::ops::Button;
    use lumen_static::widget::Node;
    use lumen_static_widgets::{TextArea, TextInput};

    struct S;
    let field: TextInput = TextInput::default();
    let area: TextArea = TextArea::default();
    let button: Button<S> = Button::default();

    assert_eq!(field.defaults().cursor, Cursor::Text);
    assert_eq!(area.defaults().cursor, Cursor::Text);
    assert_eq!(button.defaults().cursor, Cursor::Pointer);
}
