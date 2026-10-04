//! Text storage — plan M3 (`docs/layout-and-storage-decisions.md` §1).
//!
//! Text lives in a *widget field*, not in a sibling with a getter,
//! because shaping is the expensive part of a real frame and only owned
//! storage can know when to invalidate: mutation goes through the
//! widget, so the widget can set a dirty bit. A getter cannot observe
//! that application state changed and would re-measure every frame.
//!
//! Storage is generic so each profile can choose: `&'static str` or an
//! inline buffer for `no_std`, `Cow<'static, str>` for desktop (literals
//! cost nothing, computed values allocate only when they must), `String`
//! for simplicity.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
#[cfg(feature = "alloc")]
use alloc::borrow::Cow;
#[cfg(feature = "alloc")]
use alloc::borrow::ToOwned;
#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::desc::{Constraints, Size};
use crate::style::Style;

/// A value plus a dirty bit. Reads are transparent; every mutable access
/// marks the node for re-measurement — §3's original `Dirty<String>`.
#[derive(Debug, Default, Clone)]
pub struct Dirty<T> {
    value: T,
    dirty: bool,
}

impl<T> Dirty<T> {
    pub const fn new(value: T) -> Self {
        Dirty { value, dirty: true }
    }

    #[inline]
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Mutable access; marks dirty because the caller may change it.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.dirty = true;
        &mut self.value
    }

    #[inline]
    pub fn set(&mut self, value: T) {
        self.value = value;
        self.dirty = true;
    }

    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    #[inline]
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

impl<T> core::ops::Deref for Dirty<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

/// Everything a text measurement depends on besides the text itself —
/// M26/T26.3.
///
/// **The only constructor takes the whole [`Style`]**, so a caller cannot
/// build a key that leaves a field out. That is the defect this type
/// exists to close: `Measured` used to key on the constraints alone, while
/// `Label::layout` read the font size and weight inside the closure it
/// handed the memo — so a restyle that changed only the size, under
/// unchanged limits, was served last frame's box.
///
/// The text is deliberately *not* in the key. `Label` knows when its text
/// changed from its [`Dirty`] bit, which costs nothing per frame; `Button`
/// has no dirty bit and folds its label beside this key instead.
///
/// # The display scale is in the key — M26/T26.17 Phase D #12
///
/// Added because the measurer's answer depends on it and nothing said
/// so. `FontMetrics` measures at `size * scale` and divides the answer
/// back down, because a hinted face's advances are **not** linear in
/// size; 16 px doubled is not 32 px.
///
/// A scale change on its own moves the logical viewport and therefore
/// the limits, which is why this looked unreachable. `winit`'s
/// `ScaleFactorChanged` delivers a new scale *and* a new physical size
/// together, and the ordinary case — a window dragged from a 1x monitor
/// to a 2x one — holds the **logical** size constant. Limits unchanged,
/// scale doubled, memo hit, stale box; the symptom is the one
/// `docs/results-frame-cost.md` records under another cause, text drifting
/// out of the box laid out for it.
///
/// `crates/lumen-static/tests/measure_scale.rs` is the gate for `Label`;
/// `crates/lumen-static-widgets/tests/measure_scale_widgets.rs` for
/// `TextInput` and `TextArea`, whose shaped memos are built on this key
/// since M45/T45.2.
///
/// # Bit patterns throughout — M45/T45.2, R2-F10
///
/// The limits were stored as `Constraints` and compared with `f32`'s
/// `PartialEq`, under which a NaN is unequal to itself: five layouts
/// under a NaN limit measured five times (`docs/results-text.md` T45.0).
/// Every field is a bit pattern now, so "is this the same input" has one
/// answer for every value.
///
/// # Which measurer answered — M45/T45.2
///
/// [`TextMeasure::generation`](crate::cx::TextMeasure::generation) is in
/// the key, so a box measured by one stack is not served to a frame
/// measured by another: a paste pumped with the `BlockMetrics` stand-in
/// once handed the real frame its box (R2-F1, fixed at the call site by
/// M41/T41.5; this is the key's half), and a font stack that loads a
/// fallback face changes what it measures without being replaced.
///
/// # How the text flows — M80/T80.2
///
/// `flow` holds [`Style::line_height`] (the low byte; M80/T80.3 puts the
/// overflow mode in the next), filled from the `Style` this constructor
/// already takes, so every text memo keys on it without a call-site
/// change. Without it a restyle that changed only the line height served
/// the box measured at the old pitch (`crates/lumen-static/tests/measure_key.rs`).
/// Four bytes on every memo-holding widget, 32 → 36; the embedded `Ui`
/// re-budget attributes them (`docs/results-embedded.md` M80/T80.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasureKey {
    limits: [u32; 4],
    px: u32,
    weight: u32,
    scale: u32,
    generation: u32,
    flow: u32,
}

impl MeasureKey {
    /// The key for text measured under `limits` and `style` by the
    /// measurer `cx` holds, at the scale `cx` holds — **the one
    /// constructor** every text memo builds its key from (M45/T45.2).
    ///
    /// **Required, not defaulted.** The same reasoning that made this
    /// constructor take the whole [`Style`] rather than a size: a
    /// parameter a caller may omit is a field that will be omitted, and
    /// the omission is invisible until a display changes underneath a
    /// running window. Taking the context rather than its scale is the
    /// same move one step further: the next field a measurement depends
    /// on goes here, once, and not into every widget's tuple.
    ///
    /// Bit patterns, not values: see the type's docs.
    pub fn new(cx: &crate::LayoutCx, limits: Constraints, style: &Style) -> MeasureKey {
        MeasureKey::of(limits, style, cx.scale(), cx.generation())
    }

    /// [`MeasureKey::new`] with the scale and the measurer's generation
    /// given explicitly — for a caller holding no [`LayoutCx`]: a test
    /// asking whether a memo is warm, a bench building a key.
    ///
    /// [`LayoutCx`]: crate::LayoutCx
    pub fn of(limits: Constraints, style: &Style, scale: f32, generation: u32) -> MeasureKey {
        MeasureKey {
            limits: [limits.min_w.to_bits(), limits.max_w.to_bits(), limits.min_h.to_bits(), limits.max_h.to_bits()],
            px: style.font_size.to_bits(),
            weight: style.font_weight.to_bits(),
            scale: scale.to_bits(),
            generation,
            flow: style.line_height as u32,
        }
    }
}

/// A memoized measurement, valid for the [`MeasureKey`] it was taken
/// under.
///
/// Keyed on the limits because the same text measures differently in a
/// narrower box once shaping wraps it (M5), and on the font size and
/// weight because it measures differently at a different size or weight
/// (M26/T26.3) — plus the caller's dirty bit, for the text itself.
#[derive(Debug, Default, Clone)]
pub struct Measured {
    cache: Option<(MeasureKey, Size)>,
}

impl Measured {
    pub fn new() -> Self {
        Measured { cache: None }
    }

    /// Return the cached size, or compute and store one.
    #[inline]
    /// Takes `&mut self`.
    ///
    /// This was `Cell<Option<..>>` with a `&self` signature — interior
    /// mutability inherited from the flat-pass design, where ops were
    /// handed a shared base pointer. `layout` takes `&mut self`, and
    /// `get_or` has exactly one caller, inside it. The `Cell` bought
    /// nothing and cost `Sync`: every tree holding a `Label` was
    /// `!Sync`, which the repeater's own docs record removing for the
    /// same reason (`docs/plan-review-fixes.md` F6.3).
    pub fn get_or(&mut self, key: MeasureKey, dirty: bool, f: impl FnOnce() -> Size) -> Size {
        if !dirty {
            if let Some((k, s)) = self.cache {
                if k == key {
                    return s;
                }
            }
        }
        let s = f();
        self.cache = Some((key, s));
        s
    }

    /// Drop the memo (a reload restores values without going through
    /// `Dirty`, so the cache must be invalidated explicitly).
    pub fn invalidate(&mut self) {
        self.cache = None;
    }

    /// Whether a `get_or` under `key` would be served from the memo — for
    /// tests.
    pub fn is_warm(&self, key: MeasureKey) -> bool {
        matches!(self.cache, Some((k, _)) if k == key)
    }
}

/// A text memo for a widget whose text is a **plain public field**.
///
/// [`Measured`] pairs with a [`Dirty`] flag: the widget owns its text
/// and knows when it changed. `Button`, `Badge` and `Chip` do not —
/// their text is bound by the application, which writes it directly —
/// so the key carries a **hash of the bytes** in place of a dirty bit.
/// A few nanoseconds of hashing against a shaping call that allocates:
/// the `Badge` and `Chip` "layout, unchanged" arms of `--bin bindings`
/// price a hit (`docs/results-retention.md`, "What a text hash
/// guarantees", has them before and after M46/T46.1).
///
/// **What identity the key gives.** Two strings share a memo only if
/// [`crate::display::hash_bytes`] collides on them: never for strings of
/// different lengths by construction of the seed, never for the pairs the
/// old linear fold had (`"x"` and `"\0x"`, and solved same-length pairs,
/// which served a stale width — `lumen-static-widgets/tests/folded_text_identity.rs`),
/// and for text somebody chose only by a search, since the hash is
/// unkeyed. Comparing the bytes themselves would be exact, and would make
/// the memo own a copy of its text: a heap allocation per text change,
/// which M26/T26.8's budget exists to keep out of the frame.
///
/// **One helper, because it was a convention.** `Button` open-coded
/// exactly this and `Badge` and `Chip` had no memo at all — they
/// shaped on every layout, which `--bin bindings` measured at 409 ns
/// and 417 ns against a 4.97 ns hit (M26/T26.8). A memo that lives in
/// a type cannot be left out of the next widget by forgetting to
/// repeat it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FoldedText {
    cache: Option<(MeasureKey, u64, Size)>,
}

impl FoldedText {
    pub fn new() -> Self {
        FoldedText { cache: None }
    }

    /// The hash this memo keys on — [`crate::display::hash_bytes`] of
    /// the text. Public so a test can key on the same function rather
    /// than on a second copy of the arithmetic.
    #[inline]
    pub fn fold_of(text: &str) -> u64 {
        crate::display::hash_bytes(text.as_bytes())
    }

    /// The cached size for `text` under `key`, or compute and store one.
    #[inline]
    pub fn get_or(&mut self, key: MeasureKey, text: &str, f: impl FnOnce() -> Size) -> Size {
        let fold = Self::fold_of(text);
        if let Some((k, d, s)) = self.cache {
            if k == key && d == fold {
                return s;
            }
        }
        let s = f();
        self.cache = Some((key, fold, s));
        s
    }

    /// The size last measured, or zero. `paint` centres the *measured*
    /// line box, so it needs the same number `layout` used.
    pub fn size(&self) -> Size {
        self.cache.map_or(Size::default(), |(_, _, s)| s)
    }

    /// Drop the memo — a reload restores fields without going through
    /// any setter, exactly as for [`Measured::invalidate`].
    pub fn invalidate(&mut self) {
        self.cache = None;
    }

    /// Whether a `get_or` for this text under `key` would hit — for
    /// tests, and the reason the gate can name a widget.
    pub fn is_warm(&self, key: MeasureKey, text: &str) -> bool {
        self.cache == Some((key, Self::fold_of(text), self.cache.map_or(Size::default(), |c| c.2)))
    }
}

/// What `TextInput<S>` and `TextArea<S>` use when the parameter is
/// elided: `String` where there is an allocator, `InlineText<64>` where
/// there is not (M13/T13.4).
///
/// An alias rather than a `cfg` on the parameter default, because a
/// default type parameter cannot be *removed* by a `cfg` — the
/// definition still has to name some type. That is why T13.5 had to
/// land before T13.4 could finish.
#[cfg(feature = "alloc")]
pub type DefaultTextStorage = String;

/// See the `alloc` form above. 64 bytes is the same capacity the
/// `fixtures/embedded` field uses.
#[cfg(not(feature = "alloc"))]
pub type DefaultTextStorage = crate::inline::InlineText<64>;

/// What `Label<S>` accepts.
///
/// Nothing but "can be read as a string, and has an empty value" (B11).
/// Rebuilding from an owned `String` used to be here, which quietly
/// required every storage to depend on `alloc`: a fixed-capacity inline
/// buffer — the shape a `no_std` profile needs — had to pull in a heap
/// allocator solely to satisfy a method only the hot-reload path calls.
/// That method is `TextRestore` now, gated behind `reload`, so the
/// no-allocation profile stays reachable
/// (`docs/text-and-scrolling-decisions.md` B11).
///
/// The reference to `TextRestore` is deliberately not a doc link: it
/// exists only under `reload`, and the shipped-shape rustdoc build
/// (`just check`) has that feature off, where a link would be a broken
/// one.
pub trait TextStorage: AsRef<str> + Default {}

impl<T: AsRef<str> + Default> TextStorage for T {}

/// Where a shaped run's cluster positions are kept (B9).
///
/// An associated type of the *storage*, not a fixed `Vec`, so a
/// fixed-capacity inline buffer can pair with a fixed-capacity cluster
/// array and the no-allocation profile stays reachable. `String` pairs
/// with `Vec<Cluster>`.
pub trait ClusterBuf: crate::cx::GlyphSink + Default {
    fn clear(&mut self);
    fn as_slice(&self) -> &[crate::cx::Cluster];
}

#[cfg(feature = "alloc")]
impl ClusterBuf for Vec<crate::cx::Cluster> {
    fn clear(&mut self) {
        Vec::clear(self);
    }
    fn as_slice(&self) -> &[crate::cx::Cluster] {
        self
    }
}

/// Storage that a hot reload can rebuild from saved bytes.
///
/// Behind `reload` because that is the only caller: restoring is a
/// dev-surface operation (§7), and requiring it of every storage is what
/// B11 objected to. `&'static str` satisfies it by leaking, which is
/// acceptable exactly once per reload and is why [`TextBuffer`] is a
/// separate trait for the repeated case.
#[cfg(feature = "reload")]
pub trait TextRestore: TextStorage {
    /// Rebuild from an owned string — the hot-reload restore path.
    fn from_string(s: String) -> Self;
}

/// Storage that can accept a new value *repeatedly* without leaking.
///
/// Split out of [`TextStorage`] deliberately. `&'static str` can satisfy
/// `from_string` only by leaking, which is acceptable exactly once per
/// hot reload (§7's dlopen-without-dlclose spirit) and catastrophic in a
/// binding, which runs every frame. Keeping the repeated-write method in
/// a separate trait turns that misuse into a **compile error** at the
/// binding site instead of an unbounded leak nobody sees:
///
/// ```compile_fail
/// # use lumen_static_widgets::ops::Label;
/// # use lumen_static::bind::format_into;
/// let mut l: Label = Label::new("start");      // default storage
/// format_into(&mut l, format_args!("{}", 1));  // no TextBuffer impl
/// ```
///
/// Measured before the split: 1000 bound updates to a default `Label`
/// leaked 9,906 bytes and never released them. `Label<String>` allocated
/// twice and then stayed flat.
pub trait TextBuffer: TextStorage {
    /// Where this storage's shaped cluster positions live (B9).
    type Clusters: ClusterBuf;

    /// Replace the contents from a borrowed string, reusing the existing
    /// allocation where the storage can.
    fn set_from_str(&mut self, s: &str);

    /// Insert `s` at byte offset `at` (B3: methods, never a raw
    /// `&mut String`), returning whether it was stored.
    ///
    /// `at` must be a `char` boundary — every caller here derives it
    /// from a caret, and a caret only ever sits on one.
    ///
    /// **The return value is what makes a bounded storage safe to use.**
    /// This returned `()` until M13/T13.5, which was sound only while
    /// every implementation was unbounded: a caller advances its caret
    /// by `s.len()` after inserting, so a storage that *refuses* leaves
    /// the caret past the end of the text. `InlineText` hit exactly that
    /// — `debug_assert!(at <= self.len)` fired on the first keystroke
    /// after the buffer filled. An unbounded storage returns `true`
    /// always and costs nothing for saying so.
    fn insert_str(&mut self, at: usize, s: &str) -> bool;

    /// Remove `range`, whose ends must be `char` boundaries.
    fn remove_range(&mut self, range: core::ops::Range<usize>);
}

#[cfg(feature = "reload")]
impl TextRestore for String {
    fn from_string(s: String) -> Self {
        s
    }
}

#[cfg(feature = "alloc")]
impl TextBuffer for String {
    type Clusters = Vec<crate::cx::Cluster>;

    /// Keeps the capacity: steady-state binding updates allocate on no
    /// frame but the first for any given length.
    fn set_from_str(&mut self, s: &str) {
        self.clear();
        self.push_str(s);
    }

    fn insert_str(&mut self, at: usize, s: &str) -> bool {
        String::insert_str(self, at, s);
        true
    }

    fn remove_range(&mut self, range: core::ops::Range<usize>) {
        self.replace_range(range, "");
    }
}

#[cfg(feature = "reload")]
#[cfg(feature = "alloc")]
impl TextRestore for Cow<'static, str> {
    fn from_string(s: String) -> Self {
        Cow::Owned(s)
    }
}

#[cfg(feature = "alloc")]
impl TextBuffer for Cow<'static, str> {
    type Clusters = Vec<crate::cx::Cluster>;

    fn insert_str(&mut self, at: usize, s: &str) -> bool {
        self.to_mut().insert_str(at, s);
        true
    }

    fn remove_range(&mut self, range: core::ops::Range<usize>) {
        self.to_mut().replace_range(range, "");
    }

    /// Reuses the buffer when this `Cow` already owns one; a `Borrowed`
    /// literal has no buffer to reuse and becomes `Owned`.
    fn set_from_str(&mut self, s: &str) {
        match self {
            Cow::Owned(buf) => {
                buf.clear();
                buf.push_str(s);
            }
            Cow::Borrowed(_) => *self = Cow::Owned(s.to_owned()),
        }
    }
}

#[cfg(feature = "reload")]
impl TextRestore for &'static str {
    /// The documented dev-mode leak: this storage cannot own, and hot
    /// reload is a development path (§7's dlopen-without-dlclose spirit).
    /// It happens once per swap, not once per frame — which is why there
    /// is deliberately no [`TextBuffer`] impl here.
    fn from_string(s: String) -> Self {
        Box::leak(s.into_boxed_str())
    }
}
