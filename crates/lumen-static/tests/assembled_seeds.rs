//! M26/T26.4: an `Assembled` slab's default styles come from the seeds,
//! not from a live probe.
//!
//! `default_styles` built one `C::default()` and asked it
//! `probe.node(j).defaults()` for every index. `Node::node(j)` answers for
//! an index inside an inactive enum variant with the *container* standing
//! in for it, so every hidden node in a slab was seeded with the enum's
//! defaults instead of its own kind's — M24/T24.1's defect in a fourth
//! place (`docs/plan-review-fixes.md` F4.2).
//!
//! Latent until now only because no `Assembled` fixture in the tree
//! contained an enum: `benches`' `Item`, `Card` and `Toggles` are flat.

use lumen_static::assemble::Assembled;
use lumen_static_widgets::ops::Label;
use lumen_static::style::Length;
use lumen_static::{StaticTree, Widget};
use lumen_static_widgets::{CheckBox};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Pane {
    text: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Boxy {
    b: CheckBox<()>,
}

/// `Boxy` is never built: staying hidden is the point.
#[derive(StaticTree)]
#[allow(dead_code)]
enum Slot {
    Shown(Pane),
    Hidden(Boxy),
}

impl Default for Slot {
    fn default() -> Self {
        Slot::Shown(Pane::default())
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Row {
    slot: Slot,
}

/// The checkbox's index inside one instance: row 0, slot 1, pane 2, its
/// label 3, the hidden `Boxy` 4, its checkbox 5.
const CHECK: usize = 5;

#[test]
fn a_hidden_variant_in_a_slab_is_seeded_from_its_own_kind() {
    // Setup, from the seed table rather than from prose: index 5 is a
    // checkbox, and a checkbox's defaults differ from the enum's in the
    // field this asserts on.
    let seeds = <Row as Widget>::SEEDS;
    assert_eq!(seeds[CHECK].kind, "checkbox", "setup: index {CHECK} is the hidden checkbox");
    // `Seed::style`, not the raw defaults: seeding resolves a kind's
    // palette roles too (M57/A8), against the light palette headless.
    let want = seeds[CHECK].style(&lumen_static::style::Palette::LIGHT);
    let enum_defaults = (seeds[1].defaults)();
    assert_eq!(want.width, Length::Fixed(24.0), "setup: a checkbox has a fixed width");
    assert_ne!(
        want.width, enum_defaults.width,
        "setup: the container's defaults differ, or a stand-in would be indistinguishable"
    );

    let slab = Assembled::<Row>::new(2);
    for k in 0..2 {
        let at = 1 + k * <Row as Widget>::NODE_COUNT + CHECK;
        println!("instance {k}: styles[{at}].width = {:?}", slab.styles[at].width);
        assert_eq!(
            slab.styles[at], want,
            "instance {k}'s hidden checkbox was seeded from the live tree, which answers \
             for it with the enum standing in"
        );
    }
}
