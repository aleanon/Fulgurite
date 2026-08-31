//! Shapes and baselines for the benchmarks.
//!
//! The `DynWidget` arm is the §10 control: a naive `Box<dyn Widget>`
//! tree over the same shapes and the same layout math. The framework arm
//! is now recursive descent over the derived `Widget` impls
//! (`docs/results-traversal.md`).

use lumen_static::ops::{Fixed, Label, Spacer};
use lumen_static::{Constraints, Point, Size, StaticTree};

// ---- static arm shapes -----------------------------------------------------

/// 10 nodes exactly (1 + 9 leaves).
#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
pub struct Ten {
    pub l1: Label,
    pub f1: Fixed,
    pub s1: Spacer,
    pub l2: Label,
    pub f2: Fixed,
    pub l3: Label,
    pub f3: Fixed,
    pub s2: Spacer,
    pub l4: Label,
}

/// 5 nodes: a list-row shape (icon, name, filler, detail).
#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
pub struct Item {
    pub icon: Fixed,
    pub name: Label,
    pub gap: Spacer,
    pub size: Label,
}

/// 22 nodes: title + 4 items. The repeated unit for the big trees.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
pub struct Card {
    pub title: Label,
    pub r1: Item,
    pub r2: Item,
    pub r3: Item,
    pub r4: Item,
}

/// 90 nodes, fully const: nesting depth check for the derive.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct Block {
    pub title: Label,
    pub c1: Card,
    pub c2: Card,
    pub c3: Card,
    pub c4: Card,
}

// ---- dyn arm: the naive recursive-descent OOP tree -------------------------
//
// The §10 control, implementing the *same* layout model as the framework
// (iced's `Length`: Fixed / Shrink / Fill), so geometry equivalence is a
// meaningful oracle. What it does not have is the framework's const
// tables, compile-time node counts or zero allocation — it is a
// `Box<dyn>` tree built at runtime, which is the point of the contrast.

#[derive(Clone, Copy, PartialEq)]
pub enum DLen {
    Fixed(f32),
    Shrink,
    Fill(u16),
}

impl DLen {
    fn portion(&self) -> u16 {
        match self {
            DLen::Fill(k) => *k,
            _ => 0,
        }
    }
    fn is_fill(&self) -> bool {
        matches!(self, DLen::Fill(_))
    }
}

pub trait DynWidget {
    fn layout(&mut self, c: Constraints) -> Size;
    fn place(&mut self, at: Point);
    fn paint(&self, out: &mut u64);
    fn size_of(&self) -> Size;
    fn len_w(&self) -> DLen;
    fn len_h(&self) -> DLen;
    /// Overwrite the cross extent when a container stretches this child.
    fn stretch_cross(&mut self, cross: f32, horizontal: bool);
}

#[inline]
fn fold(out: &mut u64, v: u64) {
    *out = out.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(v);
}

fn resolve(c: Constraints, w: DLen, h: DLen, content: Size) -> Size {
    let ww = match w {
        DLen::Fixed(v) => v,
        DLen::Shrink => content.w,
        DLen::Fill(_) => c.max_w,
    };
    let hh = match h {
        DLen::Fixed(v) => v,
        DLen::Shrink => content.h,
        DLen::Fill(_) => c.max_h,
    };
    Size { w: ww.clamp(c.min_w, c.max_w), h: hh.clamp(c.min_h, c.max_h) }
}

pub struct DBox {
    pub children: Vec<Box<dyn DynWidget>>,
    pub spacing: f32,
    pub horizontal: bool,
    pub size: Size,
    pub pos: Point,
}

impl DBox {
    fn main(&self, s: Size) -> f32 {
        if self.horizontal { s.w } else { s.h }
    }
    fn cross(&self, s: Size) -> f32 {
        if self.horizontal { s.h } else { s.w }
    }
    fn child_len(&self, c: &dyn DynWidget) -> DLen {
        if self.horizontal { c.len_w() } else { c.len_h() }
    }
    fn child_cross_len(&self, c: &dyn DynWidget) -> DLen {
        if self.horizontal { c.len_h() } else { c.len_w() }
    }
}

impl DynWidget for DBox {
    fn stretch_cross(&mut self, cross: f32, horizontal: bool) {
        if horizontal {
            self.size.h = cross;
        } else {
            self.size.w = cross;
        }
    }

    fn layout(&mut self, c: Constraints) -> Size {
        let n = self.children.len();
        let gaps = if n > 1 { self.spacing * (n - 1) as f32 } else { 0.0 };
        let avail_main = if self.horizontal { c.max_w } else { c.max_h };

        // Scan 1: non-fill children.
        let mut used = gaps;
        let mut cross = 0.0f32;
        let mut portions: u32 = 0;
        for k in 0..n {
            let l = self.child_len(self.children[k].as_ref());
            if l.is_fill() {
                portions += l.portion() as u32;
                continue;
            }
            let rem = (avail_main - used).max(0.0);
            let cc = if self.horizontal {
                Constraints { min_w: 0.0, max_w: rem, min_h: 0.0, max_h: c.max_h }
            } else {
                Constraints { min_w: 0.0, max_w: c.max_w, min_h: 0.0, max_h: rem }
            };
            let s = self.children[k].layout(cc);
            used += self.main(s);
            // A cross-stretched child conforms to the container; it does
            // not define the container's cross size.
            if !self.child_cross_len(self.children[k].as_ref()).is_fill() {
                cross = cross.max(self.cross(s));
            }
        }

        // Scan 2: fills share the remainder.
        let free = (avail_main - used).max(0.0);
        for k in 0..n {
            let l = self.child_len(self.children[k].as_ref());
            if !l.is_fill() {
                continue;
            }
            let share =
                if portions == 0 { 0.0 } else { free * l.portion() as f32 / portions as f32 };
            let cc = if self.horizontal {
                Constraints { min_w: share, max_w: share, min_h: 0.0, max_h: c.max_h }
            } else {
                Constraints { min_w: 0.0, max_w: c.max_w, min_h: share, max_h: share }
            };
            let s = self.children[k].layout(cc);
            used += self.main(s);
            if !self.child_cross_len(self.children[k].as_ref()).is_fill() {
                cross = cross.max(self.cross(s));
            }
        }

        let content =
            if self.horizontal { Size { w: used, h: cross } } else { Size { w: cross, h: used } };
        self.size = resolve(c, self.len_w(), self.len_h(), content);
        self.size
    }

    fn place(&mut self, at: Point) {
        self.pos = at;
        let cross_extent = self.cross(self.size);
        let horizontal = self.horizontal;
        let spacing = self.spacing;
        let mut cur = at;
        for ch in &mut self.children {
            // Stretch cross-fill children to the settled extent, as the
            // framework's `place` does.
            let cl = if horizontal { ch.len_h() } else { ch.len_w() };
            if cl.is_fill() {
                ch.stretch_cross(cross_extent, horizontal);
            }
            ch.place(cur);
            let s = ch.size_of();
            cur = if horizontal {
                Point { x: cur.x + s.w + spacing, y: cur.y }
            } else {
                Point { x: cur.x, y: cur.y + s.h + spacing }
            };
        }
    }

    fn paint(&self, out: &mut u64) {
        fold(out, 0);
        fold(out, self.size.w.to_bits() as u64 ^ ((self.size.h.to_bits() as u64) << 32));
        fold(out, self.pos.x.to_bits() as u64 ^ ((self.pos.y.to_bits() as u64) << 32));
        for ch in &self.children {
            ch.paint(out);
        }
    }

    fn size_of(&self) -> Size {
        self.size
    }
    fn len_w(&self) -> DLen {
        DLen::Fill(1)
    }
    fn len_h(&self) -> DLen {
        DLen::Shrink
    }
}

pub struct DLabel {
    pub text: &'static str,
    pub size: Size,
    pub pos: Point,
}

impl DynWidget for DLabel {
    fn stretch_cross(&mut self, cross: f32, horizontal: bool) {
        if horizontal {
            self.size.h = cross;
        } else {
            self.size.w = cross;
        }
    }

    fn layout(&mut self, c: Constraints) -> Size {
        let content = Size { w: 8.0 * self.text.len() as f32, h: 16.0 };
        self.size = resolve(c, DLen::Shrink, DLen::Shrink, content);
        self.size
    }
    fn place(&mut self, at: Point) {
        self.pos = at;
    }
    fn paint(&self, out: &mut u64) {
        fold(out, 1);
        fold(out, self.size.w.to_bits() as u64 ^ ((self.size.h.to_bits() as u64) << 32));
        fold(out, self.pos.x.to_bits() as u64 ^ ((self.pos.y.to_bits() as u64) << 32));
        for &b in self.text.as_bytes() {
            fold(out, b as u64);
        }
    }
    fn size_of(&self) -> Size {
        self.size
    }
    fn len_w(&self) -> DLen {
        DLen::Shrink
    }
    fn len_h(&self) -> DLen {
        DLen::Shrink
    }
}

pub struct DFixed {
    pub w: f32,
    pub h: f32,
    pub size: Size,
    pub pos: Point,
}

impl DynWidget for DFixed {
    fn stretch_cross(&mut self, cross: f32, horizontal: bool) {
        if horizontal {
            self.size.h = cross;
        } else {
            self.size.w = cross;
        }
    }

    fn layout(&mut self, c: Constraints) -> Size {
        self.size = resolve(c, DLen::Shrink, DLen::Shrink, Size { w: self.w, h: self.h });
        self.size
    }
    fn place(&mut self, at: Point) {
        self.pos = at;
    }
    fn paint(&self, out: &mut u64) {
        fold(out, 2);
        fold(out, self.size.w.to_bits() as u64 ^ ((self.size.h.to_bits() as u64) << 32));
        fold(out, self.pos.x.to_bits() as u64 ^ ((self.pos.y.to_bits() as u64) << 32));
        fold(out, self.w.to_bits() as u64 ^ self.h.to_bits() as u64);
    }
    fn size_of(&self) -> Size {
        self.size
    }
    fn len_w(&self) -> DLen {
        DLen::Shrink
    }
    fn len_h(&self) -> DLen {
        DLen::Shrink
    }
}

pub struct DSpacer {
    pub size: Size,
    pub pos: Point,
}

impl DynWidget for DSpacer {
    fn stretch_cross(&mut self, cross: f32, horizontal: bool) {
        if horizontal {
            self.size.h = cross;
        } else {
            self.size.w = cross;
        }
    }

    fn layout(&mut self, c: Constraints) -> Size {
        self.size = resolve(c, DLen::Fill(1), DLen::Fill(1), Size::default());
        self.size
    }
    fn place(&mut self, at: Point) {
        self.pos = at;
    }
    fn paint(&self, out: &mut u64) {
        fold(out, 3);
        fold(out, self.size.w.to_bits() as u64 ^ ((self.size.h.to_bits() as u64) << 32));
        fold(out, self.pos.x.to_bits() as u64 ^ ((self.pos.y.to_bits() as u64) << 32));
    }
    fn size_of(&self) -> Size {
        self.size
    }
    fn len_w(&self) -> DLen {
        DLen::Fill(1)
    }
    fn len_h(&self) -> DLen {
        DLen::Fill(1)
    }
}

// ---- dyn shape builders (mirror the static shapes exactly) -----------------

pub fn d_label() -> Box<dyn DynWidget> {
    Box::new(DLabel { text: "", size: Size::default(), pos: Point::default() })
}

pub fn d_fixed() -> Box<dyn DynWidget> {
    Box::new(DFixed { w: 24.0, h: 24.0, size: Size::default(), pos: Point::default() })
}

pub fn d_spacer() -> Box<dyn DynWidget> {
    Box::new(DSpacer { size: Size::default(), pos: Point::default() })
}

pub fn d_ten() -> Box<dyn DynWidget> {
    Box::new(DBox {
        children: vec![
            d_label(), d_fixed(), d_spacer(), d_label(), d_fixed(),
            d_label(), d_fixed(), d_spacer(), d_label(),
        ],
        spacing: 2.0,
        horizontal: true,
        size: Size::default(),
        pos: Point::default(),
    })
}

pub fn d_item() -> Box<dyn DynWidget> {
    Box::new(DBox {
        children: vec![d_fixed(), d_label(), d_spacer(), d_label()],
        spacing: 4.0,
        horizontal: true,
        size: Size::default(),
        pos: Point::default(),
    })
}

pub fn d_card() -> Box<dyn DynWidget> {
    Box::new(DBox {
        children: vec![d_label(), d_item(), d_item(), d_item(), d_item()],
        spacing: 2.0,
        horizontal: false,
        size: Size::default(),
        pos: Point::default(),
    })
}

/// `count` cards under a spacing-0 column — mirrors `Assembled<Card>`.
pub fn d_assembled(count: usize) -> Box<dyn DynWidget> {
    Box::new(DBox {
        children: (0..count).map(|_| d_card()).collect(),
        spacing: 0.0,
        horizontal: false,
        size: Size::default(),
        pos: Point::default(),
    })
}

// ---- rigid-heavy shapes for the partition probe ----------------------------

/// 5 nodes, entirely rigid once derived (all children rigid → the row is).
#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
pub struct RigidItem {
    pub a: Fixed,
    pub b: Fixed,
    pub c: Fixed,
    pub d: Fixed,
}

/// 22 nodes, ~91% rigid: only the card root and its title are flexible.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
pub struct RigidCard {
    pub title: Label,
    pub r1: RigidItem,
    pub r2: RigidItem,
    pub r3: RigidItem,
    pub r4: RigidItem,
}

// ---- shared micro-bench harness ---------------------------------------------

/// Median/p10/p90 of `iters` timed runs after a warmup.
pub fn bench(label: &str, iters: usize, mut f: impl FnMut() -> u64) {
    for _ in 0..iters.div_ceil(10).max(3) {
        std::hint::black_box(f());
    }
    let mut samples: Vec<u64> = Vec::with_capacity(iters);
    for _ in 0..iters {
        let t0 = std::time::Instant::now();
        std::hint::black_box(f());
        samples.push(t0.elapsed().as_nanos() as u64);
    }
    samples.sort_unstable();
    let med = samples[samples.len() / 2];
    let p10 = samples[samples.len() / 10];
    let p90 = samples[samples.len() * 9 / 10];
    println!("{label:<34} median {med:>12} ns   p10 {p10:>12}   p90 {p90:>12}");
}

// ---- fused shapes (Step 6): identical geometry to Ten/Item/Card ------------

/// `Ten`, fused: 1 table entry instead of 10.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0, fuse)]
pub struct FusedTen {
    pub l1: Label,
    pub f1: Fixed,
    pub s1: Spacer,
    pub l2: Label,
    pub f2: Fixed,
    pub l3: Label,
    pub f3: Fixed,
    pub s2: Spacer,
    pub l4: Label,
}

/// `Item`, fused.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0, fuse)]
pub struct FusedItem {
    pub icon: Fixed,
    pub name: Label,
    pub gap: Spacer,
    pub size: Label,
}

/// `Card`, fused all the way down: 1 table entry instead of 22.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0, fuse)]
pub struct FusedCard {
    pub title: Label,
    pub r1: FusedItem,
    pub r2: FusedItem,
    pub r3: FusedItem,
    pub r4: FusedItem,
}
