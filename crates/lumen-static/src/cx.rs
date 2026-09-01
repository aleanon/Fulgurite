//! Layout context — plan T5.2.
//!
//! Some measurements need a service a widget cannot own: real text needs
//! a font system and a shaper. Threading that as a *type parameter*
//! would be viral through every signature and would spend §8's
//! single-type-axis budget, so it arrives as a concrete context struct
//! with `dyn` inside — the shape recorded in the renderer/executor
//! answer (`docs/plan-agent-parity.md`, appendix).

use crate::desc::Size;

/// Measures a run of text. Implemented by whatever shaping stack the
/// host composes in (parley/swash via `lumen-text`, per parity Phase A);
/// the default below keeps the framework runnable with none.
pub trait TextMeasure {
    /// Size of `text` at `size` px, wrapped to at most `max_w`.
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size;
}

/// The stand-in used until a shaping stack is composed in: a fixed
/// advance per byte and a single line. Deliberately crude, and the only
/// thing that has to change for real text to work.
#[derive(Default, Debug)]
pub struct BlockMetrics;

impl TextMeasure for BlockMetrics {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        let advance = size * 0.5;
        let natural = advance * text.len() as f32;
        if max_w <= 0.0 || natural <= max_w {
            return Size { w: natural, h: size };
        }
        // Wrap by character count — enough to make wrapping *happen*, so
        // the layout model is exercised; a real shaper breaks on grapheme
        // and word boundaries.
        let per_line = (max_w / advance).floor().max(1.0);
        let lines = (text.len() as f32 / per_line).ceil();
        Size { w: max_w, h: size * lines }
    }
}

/// Services layout may need. Passed by `&mut` through every `layout`
/// call; concrete, so no type parameter goes viral.
pub struct LayoutCx<'a> {
    pub text: &'a mut dyn TextMeasure,
}

impl<'a> LayoutCx<'a> {
    pub fn new(text: &'a mut dyn TextMeasure) -> Self {
        LayoutCx { text }
    }
}
