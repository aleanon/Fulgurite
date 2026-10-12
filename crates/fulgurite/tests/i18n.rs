//! T15.9: translation and mirroring, both allocator-free.

use fulgurite::i18n::{mirror_align, mirror_edges, mirror_styles};
use fulgurite::style::{Align, Edges};
use fulgurite::{Catalogue, Direction, Style};

/// A catalogue as it lives on an embedded target: `const` data in
/// flash, sorted, costing no RAM at all.
static NB: Catalogue = Catalogue {
    lang: "nb",
    direction: Direction::LeftToRight,
    entries: &[("cancel", "Avbryt"), ("ok", "OK"), ("save", "Lagre")],
};

static AR: Catalogue = Catalogue {
    lang: "ar",
    direction: Direction::RightToLeft,
    entries: &[("ok", "حسنا")],
};

#[test]
fn a_catalogue_translates() {
    assert_eq!(NB.tr("save"), "Lagre");
    assert_eq!(NB.tr("cancel"), "Avbryt");
}

/// **A missing translation returns the key.**
#[test]
fn a_missing_key_comes_back_as_itself() {
    // Legible: a half-translated application shows English among
    // translated strings. Blank would not be, and a visible key is a
    // bug report a user can read.
    assert_eq!(NB.tr("delete"), "delete");
    assert_eq!(Catalogue::EMPTY.tr("anything"), "anything");
}

#[test]
fn the_default_catalogue_is_the_identity() {
    // So `tr` is safe to call before anyone has written a translation,
    // and the keys are the English text.
    let c = Catalogue::default();
    assert_eq!(c.tr("Save"), "Save");
    assert!(!c.is_rtl());
}

#[test]
fn the_tables_are_sorted_so_the_binary_search_is_valid() {
    assert!(NB.check(), "nb catalogue is sorted");
    assert!(AR.check());
    assert!(Catalogue::EMPTY.check());
}

#[test]
fn mirroring_swaps_logical_alignment_and_leaves_symmetric_ones() {
    // Start and End are logical, which is the whole reason the layout
    // model has them rather than left and right: mirroring is a swap,
    // not a rewrite.
    assert_eq!(mirror_align(Align::Start, Direction::RightToLeft), Align::End);
    assert_eq!(mirror_align(Align::End, Direction::RightToLeft), Align::Start);
    assert_eq!(mirror_align(Align::Center, Direction::RightToLeft), Align::Center);
    assert_eq!(mirror_align(Align::Fill, Direction::RightToLeft), Align::Fill);
    // And left-to-right changes nothing at all.
    for a in [Align::Start, Align::End, Align::Center, Align::Fill] {
        assert_eq!(mirror_align(a, Direction::LeftToRight), a);
    }
}

#[test]
fn mirroring_swaps_horizontal_padding_only() {
    let e = Edges { left: 1.0, right: 2.0, top: 3.0, bottom: 4.0 };
    let m = mirror_edges(e, Direction::RightToLeft);
    assert_eq!((m.left, m.right), (2.0, 1.0));
    assert_eq!((m.top, m.bottom), (3.0, 4.0), "vertical is unaffected");
    assert_eq!(mirror_edges(e, Direction::LeftToRight), e);
}

#[test]
fn mirroring_a_style_array_is_a_post_cascade_pass() {
    // Once after the cascade, not per frame -- a locale changes about
    // as often as a stylesheet does.
    let mut styles = vec![
        Style { align: Align::Start, padding: Edges { left: 8.0, right: 2.0, top: 0.0, bottom: 0.0 }, ..Style::default() },
        Style { align: Align::Center, ..Style::default() },
    ];
    mirror_styles(&mut styles, Direction::RightToLeft);
    assert_eq!(styles[0].align, Align::End);
    assert_eq!((styles[0].padding.left, styles[0].padding.right), (2.0, 8.0));
    assert_eq!(styles[1].align, Align::Center, "symmetric alignment is untouched");

    // And it is a no-op left-to-right, so a host may call it always.
    let before = styles.clone();
    mirror_styles(&mut styles, Direction::LeftToRight);
    assert_eq!(styles, before);
}

#[test]
fn a_catalogue_costs_no_ram() {
    // The whole point of the `const` shape: on an embedded target this
    // lives in flash, exactly as `BakedFace` does.
    fn assert_copy<T: Copy>() {}
    assert_copy::<Catalogue>();
    assert_copy::<Direction>();
}
