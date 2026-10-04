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
//!
//! **And the source of four linked firmwares** (M52/T52.1):
//! `src/bin/firmware.rs` paints one frame of [`Panel`]; the
//! `firmware_*.rs` binaries beside it dispatch, bind and cascade over
//! [`Device`] and [`Panel`], because a gate that reads only the paint-only binary cannot
//! see input, binding or the cascade grow (R11-F4,
//! `docs/plan-embedded-footprint.md`). `scripts/check_text.sh` and
//! `scripts/check_bss.sh` judge all four.

#![no_std]

pub mod face;

/// The allocator an `alloc`-tier firmware links, and never calls
/// successfully — M52/T52.4. A `no_std` binary with `alloc` in its graph
/// must name a global allocator to link at all; this one refuses every
/// request, because the firmwares measure what the `alloc` tier *links*
/// and nothing in their frames allocates. Only with `alloc-tier`, which
/// only thumb record builds turn on.
#[cfg(feature = "alloc-tier")]
mod refuse {
    use core::alloc::{GlobalAlloc, Layout};

    struct Refuse;

    unsafe impl GlobalAlloc for Refuse {
        unsafe fn alloc(&self, _: Layout) -> *mut u8 {
            core::ptr::null_mut()
        }
        unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
    }

    #[global_allocator]
    static A: Refuse = Refuse;
}

use lumen_static::display::{Color, Painter, Rect};
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
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

/// **The tree, written once for two applications** — M52/T52.1.
///
/// `Button<A>` and the toggles are typed on the application whose state
/// their handlers take, so the interactive firmware's [`Device`] cannot
/// hold a `Ui` built for [`Panel`]. The template is one macro, so "the
/// same tree plus one bound label" is true by construction: [`Ui`] and
/// [`DeviceUi`] expand from the same text, and the paint-only firmware's
/// `.text` does not move (`docs/results-embedded.md`, "Re-measured
/// 2026-09-27").
macro_rules! panel_tree {
    ($app:ident, $ui:ident, $controls:ident $(, $(#[$m:meta])* $extra:ident : $ety:ty)?) => {
        #[derive(StaticTree, Default)]
        #[widget(row, spacing = 4.0)]
        pub struct $controls {
            #[widget_id("dec")]
            dec: Button<$app>,
            gap: Spacer,
            #[widget_id("inc")]
            inc: Button<$app>,
        }

        /// `Label` and `Button` both default their text storage to
        /// `&'static str`, so neither needs an allocator. A `Label<String>`
        /// would, which is why this fixture uses the defaults rather than
        /// spelling them out — the defaults *are* the alloc-free set.
        #[derive(StaticTree, Default)]
        #[widget(column, spacing = 6.0)]
        pub struct $ui {
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
            controls: $controls,
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
            check: CheckBox<$app>,
            #[widget_id("radio")]
            radio: Radio<$app>,
            #[widget_id("switch")]
            switch: Switch<$app>,
            $($(#[$m])* pub $extra: $ety,)?
        }

        impl $app {
            fn on_inc(p: &mut $app, _: &EventCtx) {
                p.clicks += 1;
            }

            fn on_dec(p: &mut $app, _: &EventCtx) {
                p.clicks = p.clicks.saturating_sub(1);
            }

            pub fn new() -> Self {
                let mut p = $app::default();
                p.ui.title.set("embedded");
                p.ui.badge.w = 24.0;
                p.ui.badge.h = 8.0;
                p.ui.controls.dec.label = "-";
                p.ui.controls.dec.on_click = $app::on_dec;
                p.ui.controls.inc.label = "+";
                p.ui.controls.inc.on_click = $app::on_inc;
                p
            }
        }
    };
}

panel_tree!(Panel, Ui, Controls);

/// **The interactive firmware's application** — M52/T52.1
/// (`docs/plan-embedded-footprint.md`, R11-F4).
///
/// [`Panel`]'s tree plus one bound label. The paint-only firmware has no
/// `#[bind]` and never dispatches, and M29 and M33 grew binding and
/// dispatch with every embedded gate green; `src/bin/firmware_interactive.rs`
/// and `src/bin/firmware_bind.rs` link this so the gates can see them.
/// Non-generic on purpose: a generic application gets no per-site
/// `BindSite` static, and that static is one of the things priced.
#[derive(DeriveApp, Default)]
pub struct Device {
    #[tree]
    pub ui: DeviceUi,
    #[bind(ui.readout, "{}")]
    pub clicks: u32,
}

panel_tree!(Device, DeviceUi, DeviceControls,
    /// The bound label: `clicks`, formatted into inline storage.
    #[widget_id("readout")]
    readout: Label<InlineText<12>>
);

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
    ///
    /// The line pitch is folded beside the size (M80/T80.1): this is the
    /// one required text body, so `text_weighted` and `text` arrive here
    /// with `0`. The pen walk below is one line and does not re-place
    /// lines at the pitch; the checksum proves the byte reached paint.
    fn text_spaced(&mut self, rect: Rect, s: &str, size: f32, weight: f32, line_height: u8, color: Color) {
        self.r(2, rect, &[size, weight]);
        self.fold(&[line_height]);
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
    /// No fill table — M82/T82.3: a firmware has no `Program::fills`, so
    /// `ops::fill` finds nothing and every node paints its flat `bg`.
    fn fills(&self) -> &[lumen_static::style::Fill] {
        &[]
    }
    fn pop_clip(&mut self) {
        self.f32s(6, &[]);
    }
    /// The translation, the paint and the element count — M82/T82.5.
    /// Required, so the firmware's vtable carries a slot for it (D3
    /// accepts the growth); no firmware widget paints a path, so this folds
    /// what reached paint rather than rasterising it.
    ///
    /// **One call to the out-of-line `r`, shaped like `shadow`'s**, the
    /// colours as their bits: the first version folded every field with
    /// inlined `fold`s and linked **956 B** for a method nothing in the
    /// firmware calls; a ten-value `f32s` linked 964 B, unrolled
    /// (`docs/results-embedded.md` M82/T82.5).
    fn path(&mut self, els: &[lumen_static::display::PathEl], xf: lumen_static::display::Affine, paint: lumen_static::display::PathPaint) {
        let bits = |c: Color| f32::from_bits(u32::from_be_bytes([c.0, c.1, c.2, c.3]));
        self.r(8, [xf[4], xf[5], paint.width, els.len() as f32], &[bits(paint.fill), bits(paint.stroke)]);
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

/// [`Device`]'s node count, for the interactive firmwares' arrays.
pub const DEVICE_NODES: usize = DeviceUi::NODE_COUNT;

/// A three-rule sheet — an id, a kind and a class-free `Any` — for the
/// firmwares that cascade (M52/T52.1). Three specificity levels, so
/// `cascade_range`'s level scan has more than one pass to run.
///
/// **Written twice, on purpose** (M61/T61.0, kill rule K1 of
/// `docs/plan-lss.md`): by hand here, and as `sheet.lss` under
/// `cfg(lumen_lss_sheet)` below (`LUMEN_SHEET=lss`; `build.rs` says why it
/// is not a feature). `scripts/check_lss_parity.sh` links the firmwares both
/// ways and requires their `.text` and `.rodata` to be byte-identical.
#[cfg(not(lumen_lss_sheet))]
pub const SHEET: &[lumen_static::Rule] = &[
    lumen_static::Rule::new(
        lumen_static::Selector::Any,
        lumen_static::Patch::new().spacing(5.0),
    ),
    lumen_static::Rule::new(
        lumen_static::Selector::Kind("button"),
        lumen_static::Patch::new().radius(3.0),
    ),
    lumen_static::Rule::new(
        lumen_static::Selector::Id("title"),
        lumen_static::Patch::new().font_size(18.0),
    ),
];

/// [`SHEET`], authored as `.lss` and parsed while this crate compiles —
/// the other side of K1's comparison (`docs/plan-lss.md` §5).
#[cfg(lumen_lss_sheet)]
pub const SHEET: &[lumen_static::Rule] = lumen_static::include_lss!(Ui, "sheet.lss");

/// Lay out and paint over styles the caller already has — the cascade's
/// output, which [`frame`] would overwrite with the defaults — laying out
/// only what `changed` says moved (M52/T52.1).
pub fn frame_changed<W: lumen_static::Widget>(
    ui: &mut W,
    st: &mut [NodeState],
    sty: &[Style],
    changed: lumen_static::invalidate::Changed<'_>,
    out: &mut Checksum,
) {
    let mut face: BakedFace = face::FACE;
    let mut cx = LayoutCx::with_changed(&mut face, changed);
    pass::layout_with(ui, &mut cx, sty, st, VIEW);
    pass::paint(ui, st, out);
}

/// The centre of node `i`'s laid-out box: where a press lands on it.
pub fn centre(st: &[NodeState], i: usize) -> (f32, f32) {
    let r = lumen_static::ops::rect_of(&st[i]);
    ((r[0] + r[2]) * 0.5, (r[1] + r[3]) * 0.5)
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
    //
    // **84 since M82/T82.3** — `Style` gained `background_fill: u8`, the
    // fill table index. M80 had taken both spare tail bytes
    // (`line_height`, `overflow`), so the lone byte cost a 4-byte slot:
    // 81 of 84 bytes used (`tests/cursor.rs` prints the used-bytes
    // mirror). `docs/plan-iced-roadmap.md` §2.1 budgeted it;
    // `docs/results-embedded.md` M82/T82.3 records it. `NodeState`'s copy
    // took its last spare byte and did not grow (84/84).
    const _: () = assert!(STYLE == 84, "Style grew; re-budget docs/results-embedded.md");
    const _: () = assert!(PER_NODE_RAM == 168, "per-node RAM changed");
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
    //
    // **Re-budgeted 1,152 -> 1,168 on 2026-09-20** (M26/T26.17 finding
    // 12): `MeasureKey` gained the display scale, because the measurer's
    // answer depends on it and the memo did not key on it — a stale box
    // after an ordinary monitor-to-monitor drag. Four bytes on each of
    // the four memo-holding widgets in this eleven-node tree, attributed
    // by removing the field and watching the number return to 1,152.
    // `docs/results-embedded.md` carries the argument for paying it.
    //
    // **Re-budgeted 1,168 -> 1,176 on 2026-09-26** (M45/T45.2): the key
    // gained the measurer's generation, so one stack's box is not served
    // to another's frame. Attributed the same way: without the field, 1,168.
    // `docs/results-embedded.md` has the argument.
    //
    // **Re-budgeted 1,176 -> 1,192 on 2026-09-29** (the M56 merge): all
    // sixteen bytes are `TextInput`'s (932 -> 948), the fields M57 gave it
    // -- `read_only`, `submitted`, `presses`, `placeholder_w`. The branch
    // asserted 1,168 and never checked it here: its thumb build did not
    // compile. `docs/results-embedded.md` has the measurement.
    //
    // **Re-budgeted 1,192 -> 1,216 on 2026-10-04** (M80/T80.2): the key
    // gained `flow`, the line height, so a restyle that changes only the
    // pitch is not served the old box. Four bytes of key on each of the
    // four memo-holding widgets, which alignment makes 24: the title
    // `Label`'s `Measured` +4, the field `TextInput`'s shaped key +4, and
    // each `Button`'s `FoldedText` +8 (its `u64` fold rounds the tuple up
    // to eight). Attributed by removing the field: 1,192 again.
    // `docs/results-embedded.md` M80/T80.2.
    //
    // **Unchanged at 1,216 by M80/T80.3** in the leaf tier: `Label.fit`
    // (+8 B, which made this 1,224 on the first build) is compiled out of
    // it with the cut itself, K80b's fallback. The `alloc-tier` record
    // build keeps the cut, so there the title `Label` is 8 B larger and the
    // tree **1,224** (`docs/results-embedded.md` M80/T80.3).
    #[cfg(all(target_pointer_width = "32", not(feature = "alloc-tier")))]
    const _: () = assert!(UI == 1216, "Ui is no longer 1216 B on the target; re-budget docs/results-frame-and-types.md");
    #[cfg(all(target_pointer_width = "32", feature = "alloc-tier"))]
    const _: () = assert!(UI == 1224, "the alloc-tier Ui is no longer 1224 B on the target; re-budget docs/results-embedded.md");

    // **A depth backstop, not the stack budget's measurement** — M52/T52.6.
    //
    // `docs/results-embedded.md` has budgeted 4 KB of stack since T13.9.
    // This asserts the *model* `stack_bytes(DEPTH)` against it at compile
    // time, so a tree that got deeper fails the build. It cannot see a
    // widget whose own frame grew — a 4 KB local in `TextInput::layout`
    // leaves `DEPTH`, and so this, unchanged (R11-F3) — which is what
    // `just stack` is for: `src/bin/fwq.rs` runs under qemu and measures
    // the real stack of a frame, a keystroke and a bound update, in
    // release, against the same 4 KB.
    //
    // Against the tree's *actual* `DEPTH`, not `MAX_DEPTH` — those are
    // different numbers and conflating them was the bug this closed on
    // 2026-09-07. `MAX_DEPTH` is 64 and would want ~18 KB, which is why
    // it is a runaway-recursion backstop and not this.
    pub const STACK: usize = lumen_static::stack_bytes(<super::Ui as lumen_static::Widget>::DEPTH);
    const _: () = assert!(
        STACK <= 4096,
        "the tree is deeper than docs/results-embedded.md's 4 KB stack budget allows by the model (a depth backstop; `just stack` measures)"
    );
}

