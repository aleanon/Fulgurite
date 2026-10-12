//! Translation and text direction — `docs/plan-capabilities.md` T15.9.
//!
//! # A `const` catalogue, because the target may have no allocator
//!
//! A [`Catalogue`] is a sorted `&'static [(&str, &str)]` — the same
//! shape [`crate::font::BakedFace`] uses for glyphs, and for the same
//! reason: on an embedded target it lives in flash and costs no RAM,
//! and on a desktop it is just as good. A loadable catalogue is a
//! desktop concern and belongs above this, behind `alloc`.
//!
//! # Two halves, and only one of them needs a shaper
//!
//! **Mirroring** — a right-to-left locale swapping `Align::Start` and
//! `End`, and left and right padding — is layout, and works with the
//! text stack this framework already has. **Bidirectional text** — a
//! Hebrew phrase inside an English sentence — needs a real shaper and
//! is T15.8's separate crate. Splitting them means an application can
//! have a mirrored Arabic UI before it has an Arabic *shaper*, which is
//! most of the way there and is what `docs/text-render-adoption-
//! decision.md` deferred.

/// Which way a locale runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "observe", derive(serde::Serialize, serde::Deserialize))]
pub enum Direction {
    #[default]
    LeftToRight,
    RightToLeft,
}

impl Direction {
    pub fn is_rtl(self) -> bool {
        self == Direction::RightToLeft
    }
}

/// A translation table.
///
/// `entries` **must be sorted by key**, which [`Catalogue::check`]
/// verifies and which the lookup relies on for a binary search. A
/// generator should sort; a hand-written table gets an assertion rather
/// than a silent linear-search fallback.
#[derive(Clone, Copy, Debug)]
pub struct Catalogue {
    pub lang: &'static str,
    pub direction: Direction,
    pub entries: &'static [(&'static str, &'static str)],
}

impl Catalogue {
    /// The empty catalogue: every key resolves to itself.
    ///
    /// The default, and what an untranslated application uses — so
    /// `tr()` is safe to call before anyone has written a translation,
    /// and the keys *are* the English text.
    pub const EMPTY: Catalogue =
        Catalogue { lang: "", direction: Direction::LeftToRight, entries: &[] };

    /// The translation of `key`, or `key` itself.
    ///
    /// **A missing translation returns the key.** It does not panic and
    /// does not return blank: a half-translated application should show
    /// English strings among translated ones, which is legible, and a
    /// visible key is a bug report a user can read. Blank is neither.
    ///
    /// One method, not two. An earlier version had a `'static` form
    /// beside this and the two disagreed about what a miss returns —
    /// the doc said "the key", the code said `""` — which is precisely
    /// the drift `AGENT.md` exists to stop.
    pub fn tr<'a>(&self, key: &'a str) -> &'a str {
        match self.entries.binary_search_by(|(k, _)| (*k).cmp(key)) {
            Ok(i) => self.entries[i].1,
            Err(_) => key,
        }
    }

    /// Whether the table is sorted, so the binary search is valid.
    ///
    /// Meant for a test, not a frame: a catalogue is `const` data and
    /// checking it once at build time is what a generator should do.
    pub fn check(&self) -> bool {
        self.entries.windows(2).all(|w| w[0].0 < w[1].0)
    }

    pub fn is_rtl(&self) -> bool {
        self.direction.is_rtl()
    }
}

impl Default for Catalogue {
    fn default() -> Self {
        Catalogue::EMPTY
    }
}

/// Mirror `align` for a right-to-left locale.
///
/// `Start` and `End` are *logical*, so mirroring is where they become
/// physical. `Center` and `Fill` are symmetric and unchanged, which is
/// the whole reason the model has logical alignment rather than
/// left/right.
pub fn mirror_align(a: crate::style::Align, dir: Direction) -> crate::style::Align {
    use crate::style::Align::*;
    match (a, dir) {
        (Start, Direction::RightToLeft) => End,
        (End, Direction::RightToLeft) => Start,
        (other, _) => other,
    }
}

/// Mirror an edge set for a right-to-left locale.
pub fn mirror_edges(e: crate::style::Edges, dir: Direction) -> crate::style::Edges {
    if dir.is_rtl() {
        crate::style::Edges { left: e.right, right: e.left, ..e }
    } else {
        e
    }
}

/// Mirror a resolved style array for a right-to-left locale.
///
/// **Called once after the cascade, not per frame**, which is why
/// mirroring needs no plumbing through `LayoutCx` and costs a laid-out
/// frame nothing. A locale changes about as often as a stylesheet does,
/// and this is the same shape as re-cascading for one.
pub fn mirror_styles(styles: &mut [crate::style::Style], dir: Direction) {
    if !dir.is_rtl() {
        return;
    }
    for s in styles.iter_mut() {
        s.align = mirror_align(s.align, dir);
        s.padding = mirror_edges(s.padding, dir);
    }
}
