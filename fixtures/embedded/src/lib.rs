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

pub mod face;

use lumen_static::display::{Color, Painter, Rect};
use lumen_static::prelude::*;
use lumen_static::cx::LayoutCx;
use lumen_static::font::BakedFace;
use lumen_static::{pass, Constraints, InlineText, NodeState, Style};

// ---- the tree: alloc-free widgets only -------------------------------

/// The state a handler mutates. Beside the tree, not inside it —
/// `docs/layout-and-storage-decisions.md` §1.
#[derive(DeriveApp, Default)]
pub struct Panel {
    #[tree]
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
    /// T13.5's gate: an editable field with no allocator behind it.
    /// `InlineText<64>` pairs with `InlineClusters<64>` through
    /// `TextBuffer::Clusters`, which is what B9 built the associated
    /// type for and what nothing inhabited until M13.
    #[widget_id("field")]
    pub field: TextInput<InlineText<64>>,
    controls: Controls,
    /// One of each toggle shape — M25/T25.1.
    ///
    /// Here so the firmware **links** them. `Toggle<S, SHAPE>` is generic,
    /// and each `SHAPE` is its own instantiation, so a tree that does not
    /// hold all three measures a `.text` none of them can reach. M25's
    /// pilot migrates `Toggle` and is stopped by a flash threshold, which
    /// before this was measured on a firmware with zero Toggle symbols in
    /// it (`docs/plan-appearance.md` §5). `scripts/check_text.sh` fails if
    /// they stop being linked.
    #[widget_id("check")]
    check: CheckBox<Panel>,
    #[widget_id("radio")]
    radio: Radio<Panel>,
    #[widget_id("switch")]
    switch: Switch<Panel>,
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

/// Type one character into the focused field, through the real
/// dispatch path rather than by calling a method on the widget — which
/// is the whole point of T13.5's gate: the editable widget has to work,
/// not merely exist.
///
/// `focus` and `capture` are the caller's, as they are on a real host:
/// dispatch holds `&mut Panel` for the duration, so they cannot live
/// inside it.
pub fn type_char(
    p: &mut Panel,
    st: &mut [NodeState],
    sty: &[Style],
    input: &mut lumen_static::event::InputState,
    ch: char,
) -> bool {
    pass::dispatch(p, sty, st, input, &Event::Text { ch })
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
    /// Glyphs resolved through the baked face and folded (T13.7).
    pub glyphs: u32,
    /// Characters the face has no glyph for.
    pub missing: u32,
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

    /// Resolves each character to a baked glyph and folds its
    /// **coverage**, which is what an SPI driver would push to the
    /// panel's GRAM. Folding the `&str` alone would have proved the
    /// text reached paint but not that anything could draw it — and
    /// "nothing can draw it" was exactly M13's missing piece before
    /// T13.7 (`docs/plan-embedded.md`).
    fn text_weighted(&mut self, rect: Rect, s: &str, size: f32, weight: f32, color: Color) {
        self.r(2, rect, &[size, weight]);
        self.color(color);
        let mut pen = rect[0];
        for ch in s.chars() {
            match face::FACE.glyph(ch) {
                Some(g) => {
                    self.glyphs += 1;
                    self.fold(&[g.w, g.h]);
                    self.fold(face::FACE.coverage(g));
                    pen += g.advance;
                }
                // Counted, not skipped silently: a face is a fixed set,
                // and a run full of characters it lacks should be
                // visible as a number rather than as a blank line.
                None => self.missing += 1,
            }
            self.fold(&pen.to_bits().to_le_bytes());
        }
    }
    fn rounded_rect(&mut self, rect: Rect, color: Color, radius: f32) {
        self.r(3, rect, &[radius]);
        self.color(color);
    }
    fn shadow(&mut self, rect: Rect, radius: f32, spread: f32) {
        self.r(4, rect, &[radius, spread]);
    }
    fn stroked_rect(&mut self, rect: Rect, color: Color, radius: f32, width: f32) {
        self.r(7, rect, &[radius, width]);
        self.color(color);
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
    // `pass::layout` would use `BlockMetrics`, the half-em-per-byte
    // stand-in. The baked face is the real measurer on this target, so
    // the fixture goes through `layout_with` to supply it (T13.7).
    let mut face: BakedFace = face::FACE;
    let mut cx = LayoutCx::new(&mut face);
    pass::layout_with(&mut p.ui, &mut cx, sty, st, VIEW);
    pass::paint(&p.ui, st, out);
}

/// Target-measured type sizes — `docs/plan-embedded.md` T13.9.
///
/// These are `const` on the **target**, not the host: `NodeDesc` holds
/// two pointers, so it is smaller on 32-bit, and carrying a host figure
/// over would be exactly the mistake `docs/results-embedded.md` records
/// having made once already.
///
/// They are asserted, not merely computed. A `const` assertion fails the
/// `thumbv7em-none-eabihf` build if a field is added to `NodeState`, which
/// is how the RAM budget in `docs/results-embedded.md` stays true rather than
/// becoming a number someone measured in 2026 — the same drift that doc
/// exists to record.
pub mod sizes {
    use core::mem::size_of;
    use lumen_static::{NodeDesc, NodeState, Style};

    pub const NODE_STATE: usize = size_of::<NodeState>();
    pub const STYLE: usize = size_of::<Style>();
    pub const NODE_DESC: usize = size_of::<NodeDesc>();
    pub const UI: usize = size_of::<super::Ui>();
    pub const PANEL: usize = size_of::<super::Panel>();

    /// State + style per node, which is what a tree costs before its
    /// widget structs. `NodeDesc` is `const` data in flash, not RAM.
    pub const PER_NODE_RAM: usize = NODE_STATE + STYLE;

    // Measured on `thumbv7em-none-eabihf`, 2026-09-05, and asserted so
    // the build fails rather than the budget silently drifting — which
    // is exactly what happened to this figure once already
    // (`docs/results-embedded.md`, the 72/64 correction). `NodeDesc` is
    // 32 B here against 48 on the host: it holds two pointers.
    // **84 since M27/T27.1.** The border has to reach `paint`, which has
    // no `&[Style]`, so it travels through `NodeState` the way `bg` and
    // `radius` do — and this struct's tail was full too: three `bool`s
    // and `focus_ring` in one 4-byte slot. So the border costs eight
    // bytes here as well as eight in `Style`.
    const _: () = assert!(NODE_STATE == 84, "NodeState grew; re-budget docs/results-embedded.md");
    // **80, not 72, since M27/T27.1** — `Style` gained `border_width: u8`
    // and `border_color: Color`. It had *no* spare tail padding: `align`,
    // `focus_ring`, `cursor` and `visible` fill one 4-byte slot exactly,
    // so even the lone `u8` cost four. Re-budgeted with a measurement
    // rather than by editing the number: `docs/results-embedded.md`.
    const _: () = assert!(STYLE == 80, "Style grew; re-budget docs/results-embedded.md");
    const _: () = assert!(PER_NODE_RAM == 164, "per-node RAM changed");
    // 32 on a 32-bit target, 48 on a 64-bit host: this one is
    // deliberately checked only where the budget applies.
    #[cfg(target_pointer_width = "32")]
    const _: () = assert!(NODE_DESC == 32, "NodeDesc changed on the target");
    // **The third resident array**, asserted — M25/T25.1. The per-node
    // budget is state + style + widget struct (`docs/results-embedded.md`),
    // and `PER_NODE_RAM` above counts only the first two. A change that
    // moves bytes out of `Style` and onto widgets — M25's — shrinks
    // `PER_NODE_RAM` whatever it costs, so the struct term has to be held
    // too or the saving cannot be told from a transfer. 1,104 B for 11
    // nodes, measured on the target 2026-09-15; a `const` panic cannot
    // print a value, so the expected one is in the message.
    #[cfg(target_pointer_width = "32")]
    const _: () = assert!(UI == 1152, "Ui is no longer 1152 B on the target; re-budget docs/results-frame-and-types.md");

    // **The stack budget, enforced rather than recorded.**
    //
    // `docs/results-embedded.md` has budgeted 4 KB of stack since
    // T13.9 and nothing checked it: the threshold was written down,
    // the depth was measured once by hand, and the two were never
    // connected. They are now, at compile time.
    //
    // Against the tree's *actual* `DEPTH`, not `MAX_DEPTH` — those are
    // different numbers and conflating them was the bug this closes.
    // `MAX_DEPTH` is 64 and would want ~17 KB, which is why it is a
    // runaway-recursion backstop and not this.
    pub const STACK: usize = lumen_static::stack_bytes(<super::Ui as lumen_static::Widget>::DEPTH);
    const _: () = assert!(
        STACK <= 4096,
        "the tree is deeper than docs/results-embedded.md's 4 KB stack budget"
    );
}

