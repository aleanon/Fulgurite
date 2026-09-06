//! T15.12: that `Renderer` admits a second implementation.
//!
//! The architectural claim M10a made and nothing had tested from the
//! other side: a renderer is a consumer of the command list, not the
//! definition of one. This is the first non-rasterising one, which is a
//! stronger test than a second rasteriser would have been.

use lumen_static::display::{Color, ImageId, Renderer, Stops};
use lumen_static::{DisplayList, Point};
use lumen_static_shell::svg::Svg;

const RED: Color = Color(255, 0, 0, 255);

fn render(f: impl FnOnce(&mut DisplayList)) -> String {
    let mut l = DisplayList::new();
    f(&mut l);
    let mut s = Svg::new();
    s.render(&l, 100, 50);
    s.finish()
}

#[test]
fn it_produces_a_document_with_the_right_viewport() {
    let out = render(|l| l.rect([0.0, 0.0, 10.0, 10.0], RED));
    assert!(out.starts_with("<svg xmlns="));
    assert!(out.contains(r#"viewBox="0 0 100 50""#));
    assert!(out.ends_with("</svg>"));
}

/// **Every command in the vocabulary, including the ones M15 added.**
#[test]
fn the_whole_command_vocabulary_renders() {
    let out = render(|l| {
        l.rect([0.0, 0.0, 10.0, 10.0], RED);
        l.rounded_rect([0.0, 0.0, 10.0, 10.0], RED, 4.0);
        l.shadow([0.0, 0.0, 10.0, 10.0], 4.0, 3.0);
        l.text([0.0, 0.0, 50.0, 16.0], "hi", 12.0, RED);
        l.line(Point { x: 0.0, y: 0.0 }, Point { x: 9.0, y: 9.0 }, 2.0, RED);
        l.gradient([0.0, 0.0, 10.0, 10.0], Stops::two(RED, Color(0, 0, 255, 255)), 0.0);
        l.image([0.0, 0.0, 10.0, 10.0], ImageId(3), RED);
    });
    for tag in ["<rect", "rx=", "<text", "<line", "linearGradient", r#"data-image="3""#] {
        assert!(out.contains(tag), "{tag} missing from: {out}");
    }
}

/// The failure that shows up in someone else's tool, days later.
#[test]
fn text_is_xml_escaped() {
    let out = render(|l| l.text([0.0, 0.0, 50.0, 16.0], "a < b & \"c\"", 12.0, RED));
    assert!(out.contains("a &lt; b &amp; &quot;c&quot;"), "{out}");
    assert!(!out.contains("a < b &"), "a raw ampersand makes the document unparseable");
}

#[test]
fn clips_open_and_close_as_groups() {
    let out = render(|l| {
        l.push_clip([0.0, 0.0, 10.0, 10.0]);
        l.rect([0.0, 0.0, 5.0, 5.0], RED);
        l.pop_clip();
    });
    assert_eq!(out.matches("<g clip-path=").count(), 1);
    assert_eq!(out.matches("</g>").count(), 1);
    assert!(out.contains("<clipPath"));
}

/// A renderer must not trust the list to be balanced, exactly as the
/// canvas does not — an unbalanced one must still parse.
#[test]
fn an_unbalanced_clip_still_closes() {
    let out = render(|l| {
        l.push_clip([0.0, 0.0, 10.0, 10.0]);
        l.rect([0.0, 0.0, 5.0, 5.0], RED);
        // no pop
    });
    assert_eq!(out.matches("<g clip-path=").count(), 1);
    assert_eq!(out.matches("</g>").count(), 1, "closed anyway");
    assert!(out.ends_with("</svg>"));
}

#[test]
fn rendering_twice_does_not_accumulate() {
    // `defs` and the clip depth are per-frame state; a renderer reused
    // across frames must not grow.
    let mut l = DisplayList::new();
    l.gradient([0.0, 0.0, 10.0, 10.0], Stops::two(RED, RED), 0.0);
    let mut s = Svg::new();
    s.render(&l, 100, 50);
    let first = s.finish();
    let mut s = Svg::new();
    s.render(&l, 100, 50);
    s.render(&l, 100, 50);
    assert_eq!(s.finish().len(), first.len(), "a second frame is the same size");
}
