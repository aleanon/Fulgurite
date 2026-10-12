//! **The core's shader bracket** — M83/T83.3 (`docs/plan-gpu-extension.md`),
//! behind the core's `shader` feature (`required-features` in the manifest,
//! so the binary is not built empty without it).
//!
//! A widget paints `p.shader(rect, id, &uniforms)`, its fallback with
//! ordinary calls, then `p.shader_end()`. What is checked here is what the
//! core owns of that: the uniform bytes are content-hashed and never
//! index-hashed (S2, `docs/plan-iced-roadmap.md` §2.9), retention damages a
//! bracket whose only change is its bytes, every wrapper in this crate
//! forwards the bracket rather than inheriting the clip default, and the
//! wire keeps the bytes.

use fulgurite::display::{hash_bytes, Cmd, Color, DisplayList, Painter, Rect, Scaled, ShaderId};
use fulgurite::overlay::Deferring;
use fulgurite::retain::{Damage, Incremental, Recording, Retention};
use fulgurite::NoOverlays;

const A: Rect = [10.0, 10.0, 60.0, 40.0];
const B: Rect = [100.0, 10.0, 150.0, 40.0];
const VIEW: Rect = [0.0, 0.0, 200.0, 100.0];
const FILL: Color = Color(40, 90, 200, 255);

/// A bracket over `rect` with `bytes`, its fallback a fill 20 px larger
/// than the shader's rect.
fn bracket(p: &mut dyn Painter, rect: Rect, id: u16, bytes: &[u8]) {
    p.shader(rect, ShaderId(id), bytes);
    p.rect([rect[0] - 10.0, rect[1] - 10.0, rect[2] + 10.0, rect[3] + 10.0], FILL);
    p.shader_end();
}

/// Nodes A and B, each a bracket, B painted after A.
fn two(p: &mut dyn Painter, a: &[u8], b: &[u8]) {
    bracket(p, A, 1, a);
    bracket(p, B, 2, b);
}

fn list_of(a: &[u8], b: &[u8]) -> DisplayList {
    let mut l = DisplayList::new();
    two(&mut l, a, b);
    l
}

/// Index of B's bracket in a [`two`] list.
const B_AT: usize = 3;

fn b_offset(l: &DisplayList) -> u32 {
    match l.cmds[B_AT] {
        Cmd::Shader { uniforms, .. } => uniforms.0,
        ref c => panic!("setup: command {B_AT} is {c:?}, not B's bracket"),
    }
}

/// **The shift** (S2). A's uniforms grow from 16 to 32 bytes, so B's
/// offset into the shared buffer moves; B's journal signature — the list's
/// `hash_from` at B, which is how the agent's journal signs a node — must
/// not.
#[test]
fn a_bracket_whose_uniforms_grow_does_not_move_the_next_ones_signature() {
    let b = [7u8; 16];
    let one = list_of(&[1u8; 16], &b);
    let two = list_of(&[1u8; 32], &b);
    let (o1, o2) = (b_offset(&one), b_offset(&two));
    let (h1, h2) = (one.hash_from(B_AT), two.hash_from(B_AT));
    println!("B's offset: {o1} in list 1, {o2} in list 2 (setup: they differ)");
    println!("B's hash_from: {h1:#018x} vs {h2:#018x} (bound: equal)");
    assert_ne!(o1, o2, "setup: B's offset did not move, so the shift was never exercised");
    assert_eq!(h1, h2, "B's signature moved with its offset: hash_into folds the index (M27/T27.3)");
}

/// **Same slot, new bytes.** B's sixteen bytes change at the same offset and
/// length; the command is equal field for field, so only the bytes can tell
/// the two lists apart.
#[test]
fn new_bytes_in_the_same_slot_change_the_signature() {
    let a = [1u8; 16];
    let one = list_of(&a, &[7u8; 16]);
    let two = list_of(&a, &[8u8; 16]);
    assert_eq!(one.cmds[B_AT], two.cmds[B_AT], "setup: B's command is identical, offset and length included");
    let (h1, h2) = (one.hash_from(B_AT), two.hash_from(B_AT));
    println!("B's hash_from: {h1:#018x} vs {h2:#018x} (bound: differ)");
    assert_ne!(h1, h2, "B's bytes changed and its signature did not: hash_from skips the uniforms");
}

/// One frame of [`two`] through `Recording` into `ret`.
fn frame(ret: &mut Incremental, list: &mut DisplayList, a: &[u8], b: &[u8]) -> Damage {
    list.clear();
    {
        let mut rec = Recording::new(list, ret);
        two(&mut rec, a, b);
    }
    ret.finish()
}

fn brackets(list: &DisplayList) -> usize {
    list.cmds.iter().filter(|c| matches!(c, Cmd::Shader { .. })).count()
}

fn area(r: Rect) -> f32 {
    (r[2] - r[0]) * (r[3] - r[1])
}

/// **Retention.** Only A's uniform bytes change between two frames: the
/// damage is A's rect, at 1.00x its area, and B's rect is not in it.
#[test]
fn retention_damages_a_bracket_whose_bytes_changed_and_nothing_else() {
    let mut ret = Incremental::default();
    let mut list = DisplayList::new();
    let b = [7u8; 16];
    frame(&mut ret, &mut list, &[1u8; 16], &b);
    let d = frame(&mut ret, &mut list, &[2u8; 16], &b);
    let regions: Vec<Rect> = d.regions(VIEW).collect();
    let over_a = regions.iter().fold(0.0, |s, r| s + area(*r)) / area(A);
    let b_hit = regions.iter().any(|r| r[0] < B[2] && B[0] < r[2] && r[1] < B[3] && B[1] < r[3]);
    println!(
        "damage {over_a:.2}x A's rect (bound 1.00), regions {regions:?}, B in them: {b_hit}; \
         brackets in the recorded list {} (setup 2)",
        brackets(&list)
    );
    assert_eq!(brackets(&list), 2, "setup: the list under Recording must hold both brackets: {:?}", list.cmds);
    assert!((over_a - 1.0).abs() < 1e-6, "the damage is {over_a:.2}x A's rect, not 1.00x: {d:?}");
    assert!(!b_hit, "B did not change and was damaged: {regions:?}");

    // The accept direction (S2): B's bytes change, and B is damaged at
    // 1.00x its own box, A not at all.
    let d = frame(&mut ret, &mut list, &[2u8; 16], &[8u8; 16]);
    let regions: Vec<Rect> = d.regions(VIEW).collect();
    let over_b = regions.iter().fold(0.0, |s, r| s + area(*r)) / area(B);
    let a_hit = regions.iter().any(|r| r[0] < A[2] && A[0] < r[2] && r[1] < A[3] && A[1] < r[3]);
    println!("accept: damage {over_b:.2}x B's rect (bound 1.00), A in it: {a_hit}");
    assert!((over_b - 1.0).abs() < 1e-6 && !a_hit, "B's change damaged {regions:?}");
}

/// **Deferring forwards the bracket**, as the bracket and not as the clip
/// the provided default would turn it into on the wrapper (AGENT.md's
/// provided-method trap). Compared with the unwrapped stream.
#[test]
fn deferring_forwards_the_bracket() {
    fn paint(p: &mut dyn Painter) {
        p.rect([0.0, 0.0, 10.0, 10.0], Color(1, 2, 3, 255));
        bracket(p, A, 3, &[9, 8, 7, 6]);
    }
    let mut direct = DisplayList::new();
    paint(&mut direct);
    let mut wrapped = DisplayList::new();
    {
        let mut sink = NoOverlays;
        let mut d = Deferring::new(&mut wrapped, &mut sink);
        paint(&mut d);
    }
    let shaders = |l: &DisplayList| l.cmds.iter().filter(|c| matches!(c, Cmd::Shader { .. })).count();
    println!(
        "deferring: {} in, {} out; Shader in the direct list {}, in the wrapped one {}",
        direct.cmds.len(),
        wrapped.cmds.len(),
        shaders(&direct),
        shaders(&wrapped)
    );
    assert_eq!(shaders(&direct), 1, "setup: the unwrapped list holds one bracket");
    let first = direct.cmds.iter().zip(&wrapped.cmds).position(|(a, b)| a != b);
    assert!(
        direct.cmds == wrapped.cmds && direct.uniforms == wrapped.uniforms,
        "Deferring rewrote the bracket: first difference at {first:?}, {:?} became {:?}",
        first.map(|i| direct.cmds[i]),
        first.map(|i| wrapped.cmds[i])
    );
}

/// **And `Scaled`**: the rect scales, the bytes are forwarded untouched.
#[test]
fn scaled_forwards_the_bracket_with_its_rect_scaled_and_its_bytes_whole() {
    let bytes = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let mut l = DisplayList::new();
    {
        let mut s = Scaled::new(&mut l, 2.0);
        s.shader(A, ShaderId(5), &bytes);
        s.shader_end();
    }
    println!("scaled: {:?}, bytes {:?}", l.cmds, l.uniforms);
    match l.cmds[..] {
        [Cmd::Shader { rect, id, uniforms }, Cmd::ShaderEnd] => {
            assert_eq!(rect, [20.0, 20.0, 120.0, 80.0], "the rect is not scaled by 2");
            assert_eq!(id, ShaderId(5));
            assert_eq!(l.uniforms_of(uniforms), bytes, "the bytes changed on the way through");
        }
        _ => panic!("Scaled did not forward the bracket: {:?}", l.cmds),
    }
}

/// **Recording forwards it too**, and hashes the bytes it is handed: the
/// list under it holds the bracket, not a clip.
#[test]
fn recording_forwards_the_bracket() {
    let mut ret = Incremental::default();
    let mut l = DisplayList::new();
    {
        let mut rec = Recording::new(&mut l, &mut ret);
        rec.shader(A, ShaderId(1), &[4, 4]);
        rec.shader_end();
    }
    println!("recorded: {:?}", l.cmds);
    assert!(matches!(l.cmds[..], [Cmd::Shader { .. }, Cmd::ShaderEnd]), "Recording rewrote the bracket: {:?}", l.cmds);
}

/// **A leaf inherits the clip default**, which is the documented contract
/// for a painter that cannot run shaders: the fallback, clipped.
#[test]
fn a_leaf_without_an_override_draws_the_fallback_clipped() {
    struct Leaf(Vec<&'static str>);
    impl Painter for Leaf {
        fn rect(&mut self, _: Rect, _: Color) {
            self.0.push("rect");
        }
        fn text_spaced(&mut self, _: Rect, _: &str, _: f32, _: f32, _: u8, _: Color) {}
        fn rounded_rect(&mut self, _: Rect, _: Color, _: f32) {}
        fn shadow(&mut self, _: Rect, _: f32, _: f32) {}
        fn stroked_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
        fn push_clip(&mut self, _: Rect) {
            self.0.push("push");
        }
        fn pop_clip(&mut self) {
            self.0.push("pop");
        }
        fn fills(&self) -> &[fulgurite::style::Fill] {
            &[]
        }
        fn path(&mut self, _: &[fulgurite::display::PathEl], _: fulgurite::display::Affine, _: fulgurite::display::PathPaint) {}
    }
    let mut leaf = Leaf(Vec::new());
    bracket(&mut leaf, A, 0, &[1]);
    println!("leaf: {:?}", leaf.0);
    assert_eq!(leaf.0, ["push", "rect", "pop"]);
}

/// **The wire keeps the bytes**, with and without paths beside them, and a
/// bracket whose range names bytes that are not there is refused.
#[test]
fn a_bracket_round_trips_with_its_uniform_bytes() {
    use fulgurite::wire::{decode, encode, VERSION};
    let l = list_of(&[1, 2, 3, 4, 5], &[9; 16]);
    let mut b = Vec::new();
    encode(&l, &mut b);
    let d = decode(&b).expect("decodes");
    println!(
        "wire: {} B at version {VERSION}; {} uniform bytes back of {}; round trip equal: {}",
        b.len(),
        d.uniforms.len(),
        l.uniforms.len(),
        d.cmds == l.cmds && d.uniforms == l.uniforms
    );
    assert_eq!(l.uniforms.len(), 21, "setup: 5 + 16 bytes recorded");
    assert_eq!(d.uniforms, l.uniforms, "the uniform bytes did not survive the trip");
    assert_eq!(d.cmds, l.cmds);
    assert_eq!(d.hash_from(0), l.hash_from(0));

    // With a path in the list too: the element section comes first.
    let mut lp = list_of(&[1, 2], &[3]);
    {
        use fulgurite::display::{PathEl, PathPaint, AFFINE_IDENTITY};
        use fulgurite::Point;
        let p = |x, y| Point { x, y };
        lp.path(&[PathEl::Move(p(1.0, 1.0)), PathEl::Line(p(9.0, 9.0))], AFFINE_IDENTITY, PathPaint {
            fill: Color::TRANSPARENT,
            stroke: FILL,
            width: 1.0,
        });
    }
    let mut bp = Vec::new();
    encode(&lp, &mut bp);
    let dp = decode(&bp).expect("decodes with a path");
    assert_eq!((dp.cmds == lp.cmds, &dp.paths, &dp.uniforms), (true, &lp.paths, &lp.uniforms));

    // A range past the bytes: the same buffer with its last byte cut off
    // and the section's count lowered to match.
    let mut cut = b.clone();
    cut.pop();
    let at = cut.len() - 20 - 4;
    assert_eq!(u32::from_le_bytes(cut[at..at + 4].try_into().unwrap()), 21, "setup: the byte count is where the layout says");
    cut[at..at + 4].copy_from_slice(&20u32.to_le_bytes());
    assert!(decode(&cut).is_none(), "a bracket whose bytes are missing decoded");

    // A list with no shader still encodes as version 4 did.
    let mut empty = Vec::new();
    encode(&DisplayList::new(), &mut empty);
    assert_eq!(empty, [VERSION, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
}

/// **A copy carries the bytes** (winit's `hot::Barred` copies a session's
/// list with `clone`), and `clear` empties them with the frame.
#[test]
fn a_cloned_list_carries_its_uniforms_and_clear_empties_them() {
    let mut l = list_of(&[1; 4], &[2; 4]);
    let c = l.clone();
    assert_eq!((c.uniforms.len(), c.hash_from(0)), (8, l.hash_from(0)));
    l.clear();
    assert!(l.uniforms.is_empty(), "clear left {} uniform bytes", l.uniforms.len());
    // The content hash the journal folds is the bytes' own.
    assert_ne!(hash_bytes(&[1; 4]), hash_bytes(&[2; 4]));
}
