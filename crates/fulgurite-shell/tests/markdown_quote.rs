//! **A blockquote is drawn in the theme's quote colour** — M87/T87.2
//! (`docs/plan-markdown.md`).
//!
//! Through a real `Host`, under both built-in palettes. Kill rule K87c
//! withdrew the cascade-resolved part nodes the plan first built (they
//! tripled `--bin cascade`'s Markdown row, `docs/results-markdown.md`), so
//! a quote's colour is `MarkdownTheme::quote_ink`: by default derived from
//! the node's own colour, which follows the palette, and fixed when the
//! theme names one. Both halves are checked, each against the body text
//! beside it — a palette switch that recoloured nothing would make either
//! claim vacuous, so the body following the palette is the setup check.
//!
//! The quote's contrast on the page is printed under each palette: M57's
//! D4 record for the derived colour (bound AA's 4.5:1).

use fulgurite::display::{Cmd, Color, DisplayList};
use fulgurite::prelude::*;
use fulgurite::style::{Palette, Role};
use fulgurite_shell::Host;
use fulgurite_widgets::{Markdown, MarkdownTheme};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("doc")]
    doc: Markdown,
}

#[derive(DeriveApp, Default)]
struct Doc {
    #[tree]
    ui: Ui,
}

/// A body line, then a quote whose words are found by text below.
const SRC: &str = "plain words here\n\n> quoted words there";

/// The colour of the text run that says `word`.
fn ink(list: &DisplayList, word: &str) -> Option<Color> {
    list.cmds.iter().find_map(|c| match c {
        Cmd::Text { text, color, .. } if list.str_of(*text) == word => Some(*color),
        _ => None,
    })
}

fn host(theme: MarkdownTheme) -> Host<Doc> {
    let mut a = Doc::default();
    a.ui.doc = Markdown::new(SRC);
    a.ui.doc.set_theme(theme);
    Host::new(a, 400, 200)
}

/// **Gate: by default the quote is the body's colour at `QUOTE_ALPHA`,
/// under either palette** — so it follows the palette, and reads.
/// Injection (recorded in the commit): paint a quote span in the node's
/// own `fg` — the quote then prints the body's colour, alpha 255.
#[test]
fn a_quote_follows_the_palette_by_default() {
    let mut h = host(MarkdownTheme::DEFAULT);
    for (name, palette) in [("LIGHT", Palette::LIGHT), ("DARK", Palette::DARK)] {
        h.set_palette(palette);
        h.pump();
        let text = palette.get(Role::Text);
        let want = MarkdownTheme::DEFAULT.quote_ink(text);
        let (q, b) = (ink(&h.list, "quoted"), ink(&h.list, "plain"));
        let r = fulgurite_shell::contrast::text_contrast(&h.list, palette.get(Role::Page));
        let qr = r.measured.iter().filter(|m| m.text == "quoted").map(|m| m.ratio).next();
        println!(
            "{name}: quote {q:?} (bound {want:?}), body {b:?} (bound {text:?}); quote contrast on the page {qr:?} (D4 record, AA 4.5)"
        );
        assert_eq!(b, Some(text), "setup: under {name} the body is {b:?}, not the palette's Text");
        assert_eq!(q, Some(want), "under {name} the quote is {q:?}, not the body's colour at QUOTE_ALPHA {want:?}");
        assert!(qr.is_some_and(|r| r >= 4.5), "under {name} the quote reads at {qr:?}, under AA");
    }
}

/// **Gate: a fixed quote colour stays put when the palette changes**, while
/// the body beside it moves. Injection (recorded in the commit): the same
/// as above — the quote then follows the body.
#[test]
fn a_fixed_quote_colour_does_not_follow_the_palette() {
    let fixed = Color(150, 60, 20, 255);
    let mut h = host(MarkdownTheme { quote_color: Some(fixed), ..MarkdownTheme::DEFAULT });
    for (name, palette) in [("LIGHT", Palette::LIGHT), ("DARK", Palette::DARK)] {
        h.set_palette(palette);
        h.pump();
        let (q, b) = (ink(&h.list, "quoted"), ink(&h.list, "plain"));
        println!("{name}: quote {q:?} (bound {fixed:?}, fixed), body {b:?} (bound {:?})", palette.get(Role::Text));
        assert_eq!(b, Some(palette.get(Role::Text)), "setup: under {name} the body did not take the palette");
        assert_eq!(q, Some(fixed), "under {name} a fixed quote colour moved to {q:?}");
    }
}
