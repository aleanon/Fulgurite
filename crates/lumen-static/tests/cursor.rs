//! T14.6: the pointer shape as a style property.

use lumen_static::style::{Cursor, Patch, Rule, Selector};
use lumen_static::{pass, Style};

/// **The budget this design was gated on.** `Style` is resolved per
/// node, so a byte here is a byte times every node of every tree —
/// which is why the cursor is a `Style` field and not a `NodeState`
/// one, and why this assertion exists rather than a comment saying it
/// was free once.
///
/// **84 since M82/T82.3**: `background_fill`, the fill table index, found
/// no spare byte (M80 took the last two) and grew the tail slot by four;
/// 81 of 84 bytes are used, and the mirror below prints both.
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
    // M80/T80.2-T80.3 took both spare tail bytes of `Style` and one of
    // `NodeState`; **M82/T82.3's `background_fill` grew `Style` 80 -> 84**
    // and took `NodeState`'s last spare byte (84/84) —
    // `docs/plan-iced-roadmap.md` §2.1, `docs/results-embedded.md`.
    let (style, state) = (core::mem::size_of::<Style>(), core::mem::size_of::<lumen_static::NodeState>());
    let (style_used, state_used) = (style_used_bytes(), state_used_bytes());
    println!(
        "Style {style} B (bound 84; {style_used} B used, bound 81), NodeState {state} B (bound 84; {state_used} B used, bound 84): {} B/node, {:.1} KB per 100 nodes (bound 24 KB)",
        style + state,
        (style + state) as f32 * 100.0 / 1024.0
    );
    assert_eq!(core::mem::size_of::<Style>(), 84, "Style grew; every node grew with it");
    assert_eq!(core::mem::size_of::<lumen_static::NodeState>(), 84);
    // **The used-bytes mirrors** (the `glass_cmd.rs` payload pattern):
    // `size_of` alone cannot see a byte that lands in padding — a second
    // `u8` beside `background_fill` still pads to 84 — so the fields are
    // summed too. The destructuring names every field, so a new one is a
    // compile error here before it is a number.
    assert_eq!(style_used, 81, "Style's fields grew into its padding; that is a byte every node pays for");
    assert_eq!(state_used, 84, "NodeState's fields changed");
    assert_eq!(core::mem::size_of::<Cursor>(), 1);
}

/// **`Length` is 8 B** — M85/T85.0 (`docs/plan-layout-input.md`). `Style`
/// holds two of them, so a variant wider than `Fixed(f32)` would grow every
/// node by 16 B; the flexible length T85.1 adds is decided on the claim
/// that it does not, and this is where that claim can fail.
#[test]
fn a_length_is_eight_bytes() {
    let l = core::mem::size_of::<lumen_static::style::Length>();
    println!("Length {l} B (bound 8), Style {} B", core::mem::size_of::<Style>());
    assert_eq!(l, 8, "Length grew; Style carries two of them");
}

/// The sum of `Style`'s field sizes, without padding — M82/T82.3.
fn style_used_bytes() -> usize {
    use core::mem::size_of_val as z;
    let Style {
        width,
        height,
        padding,
        spacing,
        color,
        background,
        radius,
        font_size,
        font_weight,
        shadow,
        pressed_dim,
        hover_dim,
        align,
        focus_ring,
        cursor,
        border_width,
        border_color,
        line_height,
        overflow,
        background_fill,
        visible,
        disabled,
    } = Style::default();
    z(&width)
        + z(&height)
        + z(&padding)
        + z(&spacing)
        + z(&color)
        + z(&background)
        + z(&radius)
        + z(&font_size)
        + z(&font_weight)
        + z(&shadow)
        + z(&pressed_dim)
        + z(&hover_dim)
        + z(&align)
        + z(&focus_ring)
        + z(&cursor)
        + z(&border_width)
        + z(&border_color)
        + z(&line_height)
        + z(&overflow)
        + z(&background_fill)
        + z(&visible)
        + z(&disabled)
}

/// The same for `NodeState`.
fn state_used_bytes() -> usize {
    use core::mem::size_of_val as z;
    let lumen_static::NodeState {
        constraints,
        size,
        pos,
        fg,
        bg,
        radius,
        font_size,
        font_weight,
        shadow,
        pad,
        pressed,
        hovered,
        focused,
        focus_ring,
        border_color,
        border_width,
        disabled,
        line_height,
        fill,
    } = lumen_static::NodeState::default();
    z(&constraints)
        + z(&size)
        + z(&pos)
        + z(&fg)
        + z(&bg)
        + z(&radius)
        + z(&font_size)
        + z(&font_weight)
        + z(&shadow)
        + z(&pad)
        + z(&pressed)
        + z(&hovered)
        + z(&focused)
        + z(&focus_ring)
        + z(&border_color)
        + z(&border_width)
        + z(&disabled)
        + z(&line_height)
        + z(&fill)
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
