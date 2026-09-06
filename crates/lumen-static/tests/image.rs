//! T15.2: images without pixels in the core.
//!
//! The design's claim is that `Cmd::Image` names a *host-owned* surface,
//! so the same widget, command and layout work with an allocator and
//! without one. These tests are the core half; the shell's decode path
//! is behind its own feature and its own test.

use lumen_static::display::ImageId;
use lumen_static::{
    pass, Cmd, Color, Constraints, DisplayList, Fit, Image, NodeState, StaticTree, Widget,
};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("pic")]
    pic: Image,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 100.0 };

fn frame(pic: Image) -> (DisplayList, [NodeState; Ui::NODE_COUNT]) {
    let mut ui = Ui { pic };
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);
    (list, st)
}

fn image_rect(list: &DisplayList) -> Option<[f32; 4]> {
    list.cmds.iter().find_map(|c| match c {
        Cmd::Image { rect, .. } => Some(*rect),
        _ => None,
    })
}

#[test]
fn an_image_emits_a_command_naming_a_host_surface() {
    let (list, _) = frame(Image::new(ImageId(7), lumen_static::Size { w: 20.0, h: 10.0 }));
    let cmd = list.cmds.iter().find_map(|c| match c {
        Cmd::Image { id, tint, .. } => Some((*id, *tint)),
        _ => None,
    });
    assert_eq!(cmd, Some((ImageId(7), Color(255, 255, 255, 255))), "id passes through, untinted");
}

#[test]
fn its_intrinsic_size_is_what_shrink_shrinks_to() {
    let (_, st) = frame(Image::new(ImageId(1), lumen_static::Size { w: 32.0, h: 16.0 }));
    assert_eq!((st[1].size.w, st[1].size.h), (32.0, 16.0));
}

#[test]
fn contain_keeps_the_aspect_and_centres() {
    // A 2:1 image in a 200x100 box under Contain fits the width and is
    // letterboxed vertically.
    let mut pic = Image::new(ImageId(1), lumen_static::Size { w: 200.0, h: 100.0 });
    pic.fit = Fit::Contain;
    let mut ui = Ui { pic };
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    // A square box, so aspect actually has to be preserved.
    let square = Constraints { min_w: 100.0, max_w: 100.0, min_h: 100.0, max_h: 100.0 };
    pass::layout(&mut ui, &sty, &mut st, square);
    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);

    let r = image_rect(&list).expect("an image command");
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    assert!((w / h - 2.0).abs() < 0.01, "aspect kept: {w}x{h}");
    assert!(w <= 100.01 && h <= 100.01, "fits inside the box");
    // Centred vertically.
    assert!((r[1] - (100.0 - h) / 2.0).abs() < 0.01, "letterboxed evenly");
}

#[test]
fn cover_fills_the_box_and_overflows_the_other_axis() {
    let mut pic = Image::new(ImageId(1), lumen_static::Size { w: 200.0, h: 100.0 });
    pic.fit = Fit::Cover;
    let mut ui = Ui { pic };
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let square = Constraints { min_w: 100.0, max_w: 100.0, min_h: 100.0, max_h: 100.0 };
    pass::layout(&mut ui, &sty, &mut st, square);
    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);

    let r = image_rect(&list).expect("an image command");
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    assert!(w >= 99.99 && h >= 99.99, "covers the box: {w}x{h}");
    assert!((w / h - 2.0).abs() < 0.01, "and still keeps the aspect");
}

#[test]
fn stretch_takes_the_whole_box() {
    let mut pic = Image::new(ImageId(1), lumen_static::Size { w: 200.0, h: 100.0 });
    pic.fit = Fit::Stretch;
    let (list, st) = frame(pic);
    let r = image_rect(&list).unwrap();
    assert_eq!([r[2] - r[0], r[3] - r[1]], [st[1].size.w, st[1].size.h]);
}

#[test]
fn a_tint_reaches_the_command() {
    let mut pic = Image::new(ImageId(3), lumen_static::Size { w: 8.0, h: 8.0 });
    pic.tint = Color(255, 0, 0, 255);
    let (list, _) = frame(pic);
    let tint = list.cmds.iter().find_map(|c| match c {
        Cmd::Image { tint, .. } => Some(*tint),
        _ => None,
    });
    assert_eq!(tint, Some(Color(255, 0, 0, 255)), "one monochrome asset, several colours");
}

/// The whole point of naming a surface rather than carrying pixels.
#[test]
fn the_core_holds_no_pixels() {
    // `Cmd` is the vocabulary a `no_std` host paints through; if an
    // image variant carried a buffer this would not be `Copy` and the
    // embedded profile could not have images at all.
    fn assert_copy<T: Copy>() {}
    assert_copy::<Cmd>();
    assert_copy::<ImageId>();
    assert_copy::<Image>();
}
