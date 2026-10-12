//! **Loading a Markdown image is the image's rect** — M87/T87.4
//! (`docs/plan-markdown.md`).
//!
//! A `Markdown` image changes **by id**, never in place: `Cmd::Image` folds
//! its rect, id, tint and frame, so pixels replaced under an id already
//! painted are no command change and no frame (M84's P1). An image
//! "loads" when the application's binding names the id its pixels were
//! registered under. Here the binding first names an id the source lacks
//! (the command is there; the canvas draws nothing for it), then the
//! registered one at the same size: the frame's damage — the renderer's,
//! taken from the command stream, which is what a frame redraws — must be
//! exactly the image's rect.
//!
//! Through a real `Host` under `Boundaries<256>`: the write is a
//! `#[bind(.., with = ..)]` on a state field, as an application binds
//! [`ImageRef`]s. A `with =` projection cannot say whether it wrote, so it
//! marks everything (`fulgurite`'s attribute table) — and the damage is
//! still the image's rect, because damage is taken from the command
//! stream, not from the marks.

use fulgurite::display::{Cmd, Color, ImageId, ImageSource};
use fulgurite::invalidate::Boundaries;
use fulgurite::prelude::*;
use fulgurite::retain::{Damage, Incremental};
use fulgurite_shell::session::{Offscreen, Screen};
use fulgurite_shell::Host;
use fulgurite_widgets::{ImageRef, Markdown};

const W: u32 = 400;
const H: u32 = 300;
const PAGE: Color = Color(250, 250, 250, 255);
const RED: [u8; 4] = [220, 30, 30, 255];
/// The id the source holds; any other draws nothing.
const LOADED: u32 = 7;
const MISSING: u32 = 99;

/// One 8 × 8 red picture under [`LOADED`].
struct Store(Vec<u8>);

impl ImageSource for Store {
    fn image(&self, id: ImageId, _frame: u32) -> Option<(u32, u32, &[u8])> {
        (id == ImageId(LOADED)).then_some((8, 8, &self.0[..]))
    }
}

const DOC: &str = "Before the chart.\n\n![a chart](chart)\n\nAnd words after it, which move if it grows.";

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("doc")]
    doc: Markdown,
}

#[derive(DeriveApp, Default)]
struct Doc {
    #[tree]
    ui: Ui,
    /// The chart's id and height, or `None` for no ref at all (its alt is
    /// drawn as words).
    #[bind(ui.doc, with = show_chart)]
    chart: Option<(u32, f32)>,
}

fn show_chart(m: &mut Markdown, v: &Option<(u32, f32)>) {
    match *v {
        Some((id, h)) => m.set_images(&[ImageRef { name: "chart", id: ImageId(id), size: Size { w: 120.0, h } }]),
        None => m.set_images(&[]),
    }
}

type Bounded = Host<Doc, Incremental, Boundaries<256>>;

fn boot(chart: Option<(u32, f32)>) -> (Bounded, Offscreen) {
    // The refs first and the text second, so the first frame's image is
    // resolved by `set_str` — what the injection below leaves working —
    // and only the rebinding depends on `set_images` re-resolving.
    let mut doc = Markdown::new("");
    show_chart(&mut doc, &chart);
    doc.set_str(DOC);
    let a = Doc { ui: Ui { doc }, chart };
    let mut h: Bounded = Host::new(a, W, H);
    let mut screen = Offscreen::new(W, H);
    screen.canvas.images = Some(Box::new(Store(RED.iter().copied().cycle().take(8 * 8 * 4).collect())));
    h.pump();
    h.pump();
    screen.present(&mut h, PAGE);
    (h, screen)
}

fn doc_index() -> usize {
    fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "doc").expect("#doc")
}

/// Image 0's rect in window coordinates.
fn image_rect(h: &Bounded) -> [f32; 4] {
    let s = h.states[doc_index()];
    let r = h.app.ui.doc.image_rect(0).expect("setup: the chart has no rect (unresolved?)");
    [s.pos.x + r[0], s.pos.y + r[1], s.pos.x + r[2], s.pos.y + r[3]]
}

fn images_in(h: &Bounded) -> Vec<([f32; 4], ImageId)> {
    h.list.cmds.iter().filter_map(|c| match c { Cmd::Image { rect, id, .. } => Some((*rect, *id)), _ => None }).collect()
}

fn pixel(screen: &Offscreen, x: f32, y: f32) -> [u8; 4] {
    let p = ((y as u32 * W + x as u32) * 4) as usize;
    screen.canvas.pixels[p..p + 4].try_into().unwrap()
}

/// The damage's regions, or a printable reason there are none.
fn regions(d: Damage) -> Result<Vec<[f32; 4]>, String> {
    match d {
        Damage::Regions(r) => Ok(r.as_slice().to_vec()),
        other => Err(format!("{other:?}")),
    }
}

/// **Gate: rebinding from a missing id to a registered one damages the
/// image's rect, exactly (1.00x).** Setup landed: the first frame's list
/// holds the image under the missing id and its centre shows the page; the
/// second holds one `Cmd::Image` with the new id and its centre shows the
/// picture. Injection (recorded in the commit): refs resolved once in
/// `set_str` and never by `set_images` — the command keeps the old id and
/// the damage is `None`.
#[test]
fn loading_an_image_damages_its_rect() {
    let (mut h, mut screen) = boot(Some((MISSING, 80.0)));
    let rect = image_rect(&h);
    let (cx, cy) = ((rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0);
    let before = (images_in(&h), pixel(&screen, cx, cy));
    println!("frame 1: images {:?}, centre {:?} (the page is {PAGE:?})", before.0, before.1);
    assert_eq!(before.0, [(rect, ImageId(MISSING))], "setup: the first frame does not hold the image under the missing id");

    h.app.chart = Some((LOADED, 80.0));
    h.pump();
    let dmg = h.damage();
    let shown = screen.present(&mut h, PAGE);
    let after = (images_in(&h), pixel(&screen, cx, cy));
    let marks = h.last_marks();
    println!("frame 2: images {:?}, centre {:?}; marks {marks:?}", after.0, after.1);
    let area = (rect[2] - rect[0]) * (rect[3] - rect[1]);
    let r = regions(dmg).unwrap_or_else(|e| panic!("loading the image damaged nothing: {e} (image rect {rect:?})"));
    let total: f32 = r.iter().map(|q| (q[2] - q[0]) * (q[3] - q[1])).sum();
    println!(
        "loading: damage regions {r:?} beside the image rect {rect:?}: {:.2}x of its {area:.0} px (bound: equal, 1.00x); \
         damaged_px presented {:?}",
        total / area,
        shown.damaged_px
    );
    assert_eq!(after.0, [(rect, ImageId(LOADED))], "setup: the second frame does not hold one image under the new id");
    assert_eq!(after.1, RED, "setup: the loaded picture is not on the canvas at the image's centre");
    assert_eq!(r, [rect], "loading the image damaged {r:?}, not exactly its rect {rect:?}");
}

/// **Accept: an unresolved image resolving at a new size** damages the
/// image and the words below it, which move, and no more than the node's
/// rect. The node's own clip is one of its commands and its rect grew, so
/// the renderer's prefix/suffix diff damages the node's whole rect — the
/// plan's "from the image's top" is narrower than the command stream can
/// say (`docs/plan-markdown.md`, "What building found"); its bound, the
/// node's rect, holds.
#[test]
fn resolving_at_a_new_size_damages_from_the_image_down() {
    let (mut h, mut screen) = boot(None);
    assert!(!h.app.ui.doc.image_resolved(0), "setup: the chart resolved with no refs");
    h.app.chart = Some((LOADED, 120.0));
    h.pump();
    let dmg = h.damage();
    screen.present(&mut h, PAGE);
    let rect = image_rect(&h);
    let s = h.states[doc_index()];
    let node = [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h];
    let r = regions(dmg).unwrap_or_else(|e| panic!("resolving the image damaged nothing: {e}"));
    let u = r.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |a, q| {
        [a[0].min(q[0]), a[1].min(q[1]), a[2].max(q[2]), a[3].max(q[3])]
    });
    println!(
        "resolving at 120 x 120: damage {r:?}, union {u:?}; image {rect:?}, node {node:?} \
         (bound: covers the image down to the node's bottom, inside the node's rect)"
    );
    assert!(h.app.ui.doc.image_resolved(0), "setup: the chart did not resolve");
    assert!(u[1] <= rect[1] && u[0] <= rect[0] && u[2] >= rect[2], "the damage does not cover the image: {u:?} vs {rect:?}");
    assert!(u[3] >= node[3] - 0.5, "the damage stops above the node's bottom, where the words moved to");
    assert!(u[0] >= node[0] - 0.5 && u[1] >= node[1] - 0.5 && u[2] <= node[2] + 0.5 && u[3] <= node[3] + 0.5, "the damage leaves the node's rect: {u:?} vs {node:?}");
}
