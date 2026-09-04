//! **Gate: the alloc-free static path (`docs/plan-embedded.md` T13.3).**
//!
//! A `#![no_std]` tree of the alloc-free widget set, laid out and
//! painted into a checksumming [`Painter`] — no display list, no arena,
//! no `Vec` of styles or states. Everything this frame touches is either
//! a `const` or a caller-owned array.
//!
//! This is the library half. It compiles for `thumbv7em-none-eabihf`
//! (`just thumb`), which is the *static* proof and the strong
//! one: the dynamic check in `src/main.rs` only covers paths that run.
//! T13.4's `--no-default-features` build, where `Vec` cannot be named at
//! all, is stronger still.

#![no_std]

use lumen_static::display::{Color, Painter, Rect};
use lumen_static::prelude::*;
use lumen_static::{pass, Constraints, NodeState, Style};

// ---- the tree: alloc-free widgets only -------------------------------

/// The state a handler mutates. Beside the tree, not inside it —
/// `docs/layout-and-storage-decisions.md` §1.
#[derive(Default)]
pub struct Panel {
    pub ui: Ui,
    pub clicks: u32,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Controls {
    #[widget_id("dec")]
    dec: Button<Panel>,
    gap: Spacer,
    #[widget_id("inc")]
    inc: Button<Panel>,
}

/// `Label` and `Button` both default their text storage to
/// `&'static str`, so neither needs an allocator. A `Label<String>`
/// would, which is why this fixture uses the defaults rather than
/// spelling them out — the defaults *are* the alloc-free set.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 6.0)]
pub struct Ui {
    #[widget_id("title")]
    title: Label,
    #[widget_id("badge")]
    badge: Fixed,
    controls: Controls,
}

fn on_inc(p: &mut Panel, _: &EventCtx) {
    p.clicks += 1;
}

fn on_dec(p: &mut Panel, _: &EventCtx) {
    p.clicks = p.clicks.saturating_sub(1);
}

impl Panel {
    pub fn new() -> Self {
        let mut p = Panel::default();
        p.ui.title.set("embedded");
        p.ui.badge.w = 24.0;
        p.ui.badge.h = 8.0;
        p.ui.controls.dec.label = "-";
        p.ui.controls.dec.on_click = on_dec;
        p.ui.controls.inc.label = "+";
        p.ui.controls.inc.on_click = on_inc;
        p
    }
}

/// The tree's node count, so a caller can size its arrays as a `const`.
pub const NODES: usize = Ui::NODE_COUNT;

/// The M13 device's panel: 320x240 (T13.0).
pub const VIEW: Constraints =
    Constraints { min_w: 0.0, max_w: 320.0, min_h: 0.0, max_h: 240.0 };

// ---- a Painter that keeps nothing --------------------------------------

/// Folds every drawing command into a checksum and forgets it.
///
/// The point of the M10a `Painter` seam, exercised: paint reaches a sink
/// without a [`lumen_static::DisplayList`] existing anywhere. On the real
/// device this is where SPI writes would go — the M13 panel has its own
/// GRAM and no framebuffer, because one would be 150 KB on a 128 KB part.
#[derive(Default)]
pub struct Checksum {
    pub hash: u64,
    pub cmds: u32,
}

impl Checksum {
    /// FNV-1a, so the digest is order-sensitive: two frames that draw the
    /// same shapes in a different order must not agree.
    fn fold(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.hash ^= b as u64;
            self.hash = self.hash.wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn f32s(&mut self, tag: u8, vs: &[f32]) {
        self.cmds += 1;
        self.fold(&[tag]);
        for v in vs {
            self.fold(&v.to_bits().to_le_bytes());
        }
    }

    /// `Rect` is `[f32; 4]`, so this folds it whole.
    fn r(&mut self, tag: u8, rect: Rect, extra: &[f32]) {
        self.f32s(tag, &rect);
        for v in extra {
            self.fold(&v.to_bits().to_le_bytes());
        }
    }

    fn color(&mut self, c: Color) {
        self.fold(&[c.0, c.1, c.2, c.3]);
    }
}

impl Painter for Checksum {
    fn rect(&mut self, rect: Rect, color: Color) {
        self.r(1, rect, &[]);
        self.color(color);
    }
    fn text_weighted(&mut self, rect: Rect, s: &str, size: f32, weight: f32, color: Color) {
        self.r(2, rect, &[size, weight]);
        self.fold(s.as_bytes());
        self.color(color);
    }
    fn rounded_rect(&mut self, rect: Rect, color: Color, radius: f32) {
        self.r(3, rect, &[radius]);
        self.color(color);
    }
    fn shadow(&mut self, rect: Rect, radius: f32, spread: f32) {
        self.r(4, rect, &[radius, spread]);
    }
    fn push_clip(&mut self, rect: Rect) {
        self.r(5, rect, &[]);
    }
    fn pop_clip(&mut self) {
        self.f32s(6, &[]);
    }
}

// ---- one frame, over caller-owned storage ------------------------------

/// Lay out and paint one frame into `out`, using only the arrays the
/// caller supplies.
///
/// Both arrays are `&mut [_]` rather than `Vec`: the whole task is that
/// nothing here can reach an allocator. `default_styles_into` (T13.3) is
/// what makes the style array possible — `pass::default_styles` returns a
/// `Vec` and was, until this task, the only way to build one.
pub fn frame(p: &mut Panel, st: &mut [NodeState], sty: &mut [Style], out: &mut Checksum) {
    pass::default_styles_into(&p.ui, sty);
    pass::layout(&mut p.ui, sty, st, VIEW);
    pass::paint(&p.ui, st, out);
}
