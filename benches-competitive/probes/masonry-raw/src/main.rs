//! Drives masonry 0.4 headlessly through the PUBLIC `masonry::app::RenderRoot`
//! rather than `masonry_testing::TestHarness`.
//!
//! What this establishes, and nothing more: the non-test driver exists and
//! compiles. It builds a vello `Scene` + an AccessKit `TreeUpdate` per frame —
//! the same artefacts `TestHarness::render()` produces under
//! `SKIP_RENDER_TESTS=1` — minus the harness's global tracing subscriber, its
//! `process_signals()` drain and its `accesskit_consumer::Tree` update.
//!
//! It prints the scene's encoded op count so a run proves work happened; it
//! prints no time and MUST NOT grow a timing loop without a fresh
//! `docs/results-*.md` entry, because dropping the AccessKit consumer update
//! moves the stopping point away from the 2026-09-05 masonry row.
use masonry::app::{RenderRoot, RenderRootOptions, WindowSizePolicy};
use masonry::core::{NewWidget, Widget, WidgetTag, WindowEvent};
use masonry::dpi::PhysicalSize;
use masonry::theme::default_property_set;
use masonry::widgets::{Flex, Label};
use std::sync::Arc;

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(100);
    // The knob `TestHarness` does not expose: it sends `EnableAccessTree`
    // unconditionally at construction, so every masonry frame it drives
    // rebuilds an AccessKit tree. `docs/results-competitive.md` §1 puts that
    // row beside a fulgurite row and a `Lumen` row that build no semantics
    // (Lumen's own harness notes `pump()` never calls `sem_root()`), and beside
    // a separate `Lumen + semantics` row that does.
    let a11y = std::env::args().nth(2).as_deref() != Some("no-a11y");

    let tag: WidgetTag<Label> = WidgetTag::new("counter");
    let mut flex = Flex::column().with_child(NewWidget::new_with_tag(Label::new("counter: 0"), tag));
    for i in 1..n {
        flex = flex.with_child(Label::new(format!("row {i}")).with_auto_id());
    }

    let mut root = RenderRoot::new(
        flex.with_auto_id(),
        |_signal| {},
        RenderRootOptions {
            default_properties: Arc::new(default_property_set()),
            // TestHarness sets this false and injects a bundled Roboto so its
            // screenshots are cross-platform. Left TRUE here: every other arm
            // in this comparison shapes against the system font stack.
            use_system_fonts: true,
            size_policy: WindowSizePolicy::User,
            size: PhysicalSize::new(400, 800),
            scale_factor: 1.0,
            test_font: None,
        },
    );

    if a11y {
        root.handle_window_event(WindowEvent::EnableAccessTree);
    }

    // One frame, to prove the driver works. `edit_widget_with_tag` runs the
    // rewrite passes itself; `redraw` produces the scene and the a11y update.
    root.edit_widget_with_tag(tag, |mut l| Label::set_text(&mut l, "counter: 1"));
    let (scene, tree_update) = root.redraw();
    let enc = scene.encoding();
    let glyph_runs: usize = enc.resources.glyph_runs.len();
    let a11y_nodes = tree_update.map_or(0, |t| t.nodes.len());
    println!(
        "masonry-raw: n={n} a11y={a11y} n_paths={} draw_tags={} glyph_runs={glyph_runs} a11y_nodes={a11y_nodes}",
        enc.n_paths,
        enc.draw_tags.len()
    );
    // The lesson every harness in this comparison was rewritten to learn: an
    // opponent that silently does nothing reports a flattering number and no
    // error. A frame with no glyph runs shaped no text.
    assert!(glyph_runs > 0, "the frame shaped no text, so it measured nothing");
    assert!(
        !a11y || a11y_nodes > 0,
        "no AccessKit update: the a11y pass did not run"
    );
}
