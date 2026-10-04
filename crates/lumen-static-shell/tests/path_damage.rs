//! M82/T82.5: what a path costs retention and the journal, and where its
//! transform puts it (`docs/plan-paint-vocabulary.md`).
//!
//! A path's elements live in the list's shared store, as a text run's
//! bytes do, so the M27/T27.3 hazard applies: a command that folded its
//! *offset* would report every later path changed whenever an earlier one
//! gained an element. The content is what is hashed, never the index (S2).

use lumen_static::display::{
    hash_path, path_bounds, Affine, Cmd, Color, DisplayList, PathEl, PathPaint, Rect, Scaled, AFFINE_IDENTITY,
};
use lumen_static::retain::{Damage, Incremental, Recording, Retention};
use lumen_static::{Painter, Point};
use lumen_static_shell::software::Canvas;

const INK: Color = Color(30, 90, 200, 255);
const PAGE: Color = Color(250, 250, 250, 255);
const VIEW: Rect = [0.0, 0.0, 256.0, 256.0];

fn pt(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn paint() -> PathPaint {
    PathPaint { fill: INK, stroke: Color(0, 0, 0, 255), width: 2.0 }
}

/// Path A: a square, with an extra collinear point on its top edge when
/// `mid` is `Some` — one more element, the same bounds.
fn path_a(mid: Option<f32>) -> Vec<PathEl> {
    let mut v = vec![PathEl::Move(pt(10.0, 10.0))];
    if let Some(y) = mid {
        v.push(PathEl::Line(pt(30.0, y)));
    }
    v.extend([PathEl::Line(pt(50.0, 10.0)), PathEl::Line(pt(50.0, 50.0)), PathEl::Line(pt(10.0, 50.0)), PathEl::Close]);
    v
}

fn path_b() -> Vec<PathEl> {
    vec![PathEl::Move(pt(120.0, 20.0)), PathEl::Line(pt(160.0, 60.0)), PathEl::Line(pt(120.0, 60.0)), PathEl::Close]
}

/// A, a text run, B — into `list`, through `ret`.
fn frame(ret: &mut Incremental, list: &mut DisplayList, a: &[PathEl]) -> Damage {
    list.clear();
    {
        let mut rec = Recording::new(list, ret);
        rec.path(a, AFFINE_IDENTITY, paint());
        rec.text([60.0, 10.0, 110.0, 30.0], "between", 12.0, Color(0, 0, 0, 255));
        rec.path(&path_b(), AFFINE_IDENTITY, paint());
    }
    assert!(
        matches!(list.cmds[..], [Cmd::Path { .. }, Cmd::Text { .. }, Cmd::Path { .. }]),
        "setup: the frame must hold path, text, path: {:?}",
        list.cmds
    );
    ret.finish()
}

fn area(d: Damage) -> (f32, usize) {
    let rs: Vec<Rect> = d.regions(VIEW).collect();
    (rs.iter().fold(0.0, |a, r| a + (r[2] - r[0]) * (r[3] - r[1])), rs.len())
}

/// **No phantom damage** (S2, T5). A gains an element: retention damages
/// A's ink and nothing else, and B's journal signature — the list's
/// `hash_from` at B, which is what the agent's journal signs a node by —
/// does not move although B's offset into the store did.
#[test]
fn a_path_that_gains_an_element_damages_only_itself() {
    let mut ret = Incremental::default();
    let mut list = DisplayList::new();
    frame(&mut ret, &mut list, &path_a(None));
    let b_before = list.hash_from(2);
    let off_before = match list.cmds[2] {
        Cmd::Path { els, .. } => els.0,
        _ => unreachable!(),
    };

    let d = frame(&mut ret, &mut list, &path_a(Some(10.0)));
    let b_after = list.hash_from(2);
    let off_after = match list.cmds[2] {
        Cmd::Path { els, .. } => els.0,
        _ => unreachable!(),
    };
    let (got, n) = area(d);
    // A's ink as `Recording` reports it: its box grown by width / 2 + 1.
    let b = path_bounds(&path_a(None), &AFFINE_IDENTITY).unwrap();
    let g = paint().width / 2.0 + 1.0;
    let want = (b[2] - b[0] + 2.0 * g) * (b[3] - b[1] + 2.0 * g);
    println!(
        "phantom: A 5 -> 6 elements; damage {got} px² in {n} region(s) (want A's ink {want}); \
         B at offset {off_before} -> {off_after}, signature {b_before:016x} -> {b_after:016x}"
    );
    assert_ne!(off_before, off_after, "setup: B's offset into the store must have moved");
    assert_eq!(got, want, "A's element change damaged {got} px², not A's ink {want}");
    assert_eq!(b_before, b_after, "B reported changed: its signature moved with its offset");

    // The same count, a different shape: the content is hashed, not the
    // count, so this is damage too.
    let d = frame(&mut ret, &mut list, &path_a(Some(4.0)));
    let (moved, _) = area(d);
    let b2 = path_bounds(&path_a(Some(4.0)), &AFFINE_IDENTITY).unwrap();
    let want2 = (b2[2] - b2[0] + 2.0 * g) * (b2[3] - b2[1] + 2.0 * g);
    println!("same count, moved point: damage {moved} px² (want the union of A's old and new ink, {want2})");
    assert_eq!(moved, want2, "a path that changed shape but not length damaged {moved} px²");

    // Accept: an unchanged frame damages nothing.
    let (same, _) = area(frame(&mut ret, &mut list, &path_a(Some(4.0))));
    println!("unchanged: damage {same} px² (want 0)");
    assert_eq!(same, 0.0, "an unchanged path damaged {same} px²");
}

/// Collects each command's ink, so a test can read what `Recording`
/// reported without a damage pass in the way.
#[derive(Default)]
struct Inks(Vec<Rect>);

impl Retention for Inks {
    fn observe(&mut self, _: u64, ink: Rect) {
        self.0.push(ink);
    }
    fn finish(&mut self) -> Damage {
        Damage::None
    }
    fn take_damage_for_age(&mut self, _: u8) -> Damage {
        Damage::None
    }
    fn reset(&mut self) {}
    fn still(&mut self) -> Damage {
        Damage::None
    }
}

/// **The transform, through `Scaled`.** A wedge rotated 30° and moved,
/// at scale 1 and 2: `Recording` (logical, above `Scaled` as in the host)
/// inks the rotated control-point box grown by `width / 2 + 1`; the list
/// below `Scaled` holds `k` times that box grown by `k · width / 2`; and
/// the canvas inks the rotated tip.
#[test]
fn a_rotated_path_scales_with_its_transform() {
    let (s, c) = (30f32.to_radians().sin(), 30f32.to_radians().cos());
    let xf: Affine = [c, s, -s, c, 60.0, 30.0];
    let els = [PathEl::Move(pt(0.0, 0.0)), PathEl::Line(pt(80.0, 0.0)), PathEl::Line(pt(80.0, 12.0)), PathEl::Close];
    let w = 2.0;
    // Independently of `path_bounds`: rotate the three points by hand.
    let rot = |x: f32, y: f32| (c * x - s * y + 60.0, s * x + c * y + 30.0);
    let ps = [rot(0.0, 0.0), rot(80.0, 0.0), rot(80.0, 12.0)];
    let bx = [
        ps.iter().map(|p| p.0).fold(f32::INFINITY, f32::min),
        ps.iter().map(|p| p.1).fold(f32::INFINITY, f32::min),
        ps.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max),
        ps.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max),
    ];
    for k in [1.0f32, 2.0] {
        let mut list = DisplayList::new();
        let mut inks = Inks::default();
        {
            let mut scaled = Scaled::new(&mut list, k);
            let mut rec = Recording::new(&mut scaled, &mut inks);
            rec.path(&els, xf, PathPaint { fill: INK, stroke: Color(0, 0, 0, 255), width: w });
        }
        let g = w / 2.0 + 1.0;
        let want_ink = [bx[0] - g, bx[1] - g, bx[2] + g, bx[3] + g];
        let Cmd::Path { rect, width, .. } = list.cmds[0] else { panic!("setup: one path, got {:?}", list.cmds) };
        let h = k * w / 2.0;
        let want_rect = [k * bx[0] - h, k * bx[1] - h, k * bx[2] + h, k * bx[3] + h];
        println!("scale {k}: Recording ink {:?} (want {want_ink:?}); list rect {rect:?} width {width} (want {want_rect:?}, {})", inks.0[0], k * w);
        let close = |a: Rect, b: Rect| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3);
        assert!(close(inks.0[0], want_ink), "scale {k}: Recording's ink is not the rotated box grown by width / 2 + 1");
        assert!(close(rect, want_rect), "scale {k}: the list's path is not the rotated box at scale {k}");

        // The tip, 4 px in from the wedge's far end, inked at its physical place.
        let (tx, ty) = rot(76.0, 6.0);
        let (px, py) = ((tx * k) as usize, (ty * k) as usize);
        let mut cv = Canvas::new(256, 256);
        cv.clear(PAGE);
        lumen_static::Renderer::render(&mut cv, &list, 256, 256);
        let i = (py * 256 + px) * 4;
        let got = Color(cv.pixels[i], cv.pixels[i + 1], cv.pixels[i + 2], 255);
        println!("scale {k}: rotated tip pixel ({px}, {py}) = {got:?} (want {INK:?})");
        assert_eq!(got, INK, "scale {k}: the rotated tip is not inked at its physical place");
    }
}

/// The element hash is the content: two paths with the same elements hash
/// the same, one moved point does not (the function both retention and
/// the journal fold).
#[test]
fn the_element_hash_is_the_content() {
    let a = path_a(Some(10.0));
    let mut b = a.clone();
    assert_eq!(hash_path(&a), hash_path(&b));
    b[1] = PathEl::Line(pt(30.0, 11.0));
    assert_ne!(hash_path(&a), hash_path(&b), "a moved point hashed the same");
}
