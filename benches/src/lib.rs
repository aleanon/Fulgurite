//! Shapes and baselines for the Step 2 crossover benchmark (handoff §10):
//! the static node-table arm and a naive `Box<dyn Widget>` arm over the
//! same tree shapes and the same layout math.

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

pub trait DynWidget {
    fn layout(&mut self, c: Constraints) -> Size;
    fn place(&mut self, at: Point);
    fn paint(&self, out: &mut u64);
    fn size_of(&self) -> Size;
}

#[inline]
fn fold(out: &mut u64, v: u64) {
    *out = out.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(v);
}

pub struct DBox {
    pub children: Vec<Box<dyn DynWidget>>,
    pub spacing: f32,
    pub horizontal: bool,
    pub size: Size,
    pub pos: Point,
}

impl DynWidget for DBox {
    fn layout(&mut self, c: Constraints) -> Size {
        let n = self.children.len().max(1) as f32;
        let spaced = if self.children.len() > 1 {
            self.spacing * (self.children.len() - 1) as f32
        } else {
            0.0
        };
        let cc = if self.horizontal {
            Constraints { min_w: 0.0, max_w: ((c.max_w - spaced) / n).max(0.0), min_h: 0.0, max_h: c.max_h }
        } else {
            Constraints { min_w: 0.0, max_w: c.max_w, min_h: 0.0, max_h: ((c.max_h - spaced) / n).max(0.0) }
        };
        let mut sum = 0.0f32;
        let mut max = 0.0f32;
        for ch in &mut self.children {
            let s = ch.layout(cc);
            if self.horizontal {
                sum += s.w;
                max = max.max(s.h);
            } else {
                sum += s.h;
                max = max.max(s.w);
            }
        }
        let raw = if self.horizontal {
            Size { w: sum + spaced, h: max }
        } else {
            Size { w: max, h: sum + spaced }
        };
        self.size = c.clamp(raw);
        self.size
    }

    fn place(&mut self, at: Point) {
        self.pos = at;
        let mut cur = at;
        for ch in &mut self.children {
            ch.place(cur);
            let s = ch.size_of();
            cur = if self.horizontal {
                Point { x: cur.x + s.w + self.spacing, y: cur.y }
            } else {
                Point { x: cur.x, y: cur.y + s.h + self.spacing }
            };
        }
    }

    fn paint(&self, out: &mut u64) {
        fold(out, 100);
        fold(out, self.size.w.to_bits() as u64 ^ ((self.size.h.to_bits() as u64) << 32));
        fold(out, self.pos.x.to_bits() as u64 ^ ((self.pos.y.to_bits() as u64) << 32));
        for ch in &self.children {
            ch.paint(out);
        }
    }

    fn size_of(&self) -> Size {
        self.size
    }
}

pub struct DLabel {
    pub text: &'static str,
    pub size: Size,
    pub pos: Point,
}

impl DynWidget for DLabel {
    fn layout(&mut self, c: Constraints) -> Size {
        self.size = c.clamp(Size { w: 8.0 * self.text.len() as f32, h: 16.0 });
        self.size
    }
    fn place(&mut self, at: Point) {
        self.pos = at;
    }
    fn paint(&self, out: &mut u64) {
        fold(out, 101);
        fold(out, self.size.w.to_bits() as u64 ^ ((self.size.h.to_bits() as u64) << 32));
        fold(out, self.pos.x.to_bits() as u64 ^ ((self.pos.y.to_bits() as u64) << 32));
        for &b in self.text.as_bytes() {
            fold(out, b as u64);
        }
    }
    fn size_of(&self) -> Size {
        self.size
    }
}

pub struct DFixed {
    pub w: f32,
    pub h: f32,
    pub size: Size,
    pub pos: Point,
}

impl DynWidget for DFixed {
    fn layout(&mut self, c: Constraints) -> Size {
        self.size = c.clamp(Size { w: self.w, h: self.h });
        self.size
    }
    fn place(&mut self, at: Point) {
        self.pos = at;
    }
    fn paint(&self, out: &mut u64) {
        fold(out, 102);
        fold(out, self.size.w.to_bits() as u64 ^ ((self.size.h.to_bits() as u64) << 32));
        fold(out, self.pos.x.to_bits() as u64 ^ ((self.pos.y.to_bits() as u64) << 32));
        fold(out, self.w.to_bits() as u64 ^ self.h.to_bits() as u64);
    }
    fn size_of(&self) -> Size {
        self.size
    }
}

pub struct DSpacer {
    pub size: Size,
    pub pos: Point,
}

impl DynWidget for DSpacer {
    fn layout(&mut self, c: Constraints) -> Size {
        self.size = Size { w: c.max_w, h: c.max_h };
        self.size
    }
    fn place(&mut self, at: Point) {
        self.pos = at;
    }
    fn paint(&self, out: &mut u64) {
        fold(out, 103);
        fold(out, self.size.w.to_bits() as u64 ^ ((self.size.h.to_bits() as u64) << 32));
        fold(out, self.pos.x.to_bits() as u64 ^ ((self.pos.y.to_bits() as u64) << 32));
    }
    fn size_of(&self) -> Size {
        self.size
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
