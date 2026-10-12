//! The tally: the tree, its stylesheet, and its handlers.
//!
//! Deliberately shaped after Lumen's `examples/counter` so the two are
//! comparable — a caption, a hero number that recolours with the sign, a
//! status pill, four step buttons and a reset. What differs is
//! structural, not cosmetic: the tree is a `struct`, so its shape is
//! fixed at compile time and `NODE_COUNT` is a constant.

use fulgurite::display::Color;
use fulgurite::prelude::*;
use fulgurite_shell::program::KeyOutcome;
use fulgurite_widgets::prelude::*;

// ---- palette ---------------------------------------------------------

/// The page: `app.lss`'s `$ground`, which the window also clears to
/// behind every damaged region. The rest of Lumen's counter tokens live
/// in `app.lss` since M61 (`docs/plan-lss.md`).
pub const PAGE: Color = Color(0x0e, 0x11, 0x18, 0xff);

// ---- the tree --------------------------------------------------------

#[derive(StaticTree, Default)]
#[widget(row, spacing = 8.0)]
pub struct Steps {
    #[widget_id("dec10")]
    pub dec10: Button<Count, String>,
    #[widget_id("dec")]
    pub dec: Button<Count, String>,
    #[widget_id("inc")]
    pub inc: Button<Count, String>,
    #[widget_id("inc10")]
    pub inc10: Button<Count, String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 16.0)]
pub struct Card {
    #[widget_id("caption")]
    pub caption: Label,
    #[widget_id("value")]
    pub value: Label<String>,
    #[widget_id("pill")]
    pub pill: Label<String>,
    #[widget_id("steps")]
    pub steps: Steps,
    #[widget_id("reset")]
    pub reset: Button<Count, String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct Page {
    /// The framework has no `justify-content`: a container places
    /// children sequentially and `Fill` divides the remainder. Two Fill
    /// spacers sharing it is main-axis centring, and it is the same
    /// trick the control row uses to push its buttons apart.
    pub top: Spacer,
    #[widget_id("card")]
    pub card: Card,
    pub bottom: Spacer,
}

// ---- state, and how it reaches the screen ----------------------------

#[derive(DeriveApp, Default)]
pub struct Tally {
    #[tree]
    pub ui: Page,

    /// **What a handler may touch** — M33/T33.4. The one piece of state;
    /// everything on screen derives from it, through two bindings that
    /// both compare before they write.
    #[state]
    #[bind(ui.card.value, n, "{}")]
    #[bind(ui.card.pill, sign_word(), "{}")]
    pub count: Count,
}

/// The tally, apart from the tree it is shown in.
#[derive(Default)]
pub struct Count {
    pub n: i64,
}

impl Count {
    /// The pill's text is a word, not the number. Named in a binding's
    /// **source path** — a source path is an expression within the field,
    /// so a method call is one — which keeps the pill on the comparing
    /// form. It was a `with =` projection until M33/T33.4, and `with =`
    /// reports a write unconditionally.
    pub fn sign_word(&self) -> &'static str {
        match self.n.signum() {
            1 => "POSITIVE",
            -1 => "NEGATIVE",
            _ => "AT ZERO",
        }
    }
}

/// **The program, described once** — M32 (`docs/plan-program.md`), and
/// since M60 (`docs/plan-tally-slim.md`) everything the window needs:
///
/// - the agent endpoint's default address, which `FULGURITE_AGENT_ADDR`
///   overrides (read by the window, M48/T48.11);
/// - **the tally's keys** — [`keys`], through `Program::on_key`: a key
///   press no widget took;
/// - **the sheet for the count's sign**, from `app.lss` since M61
///   (`docs/plan-lss.md`) through `Program::style_lss_by`: the host
///   re-applies it on the frame the sign changes, on every path, and a
///   debug window re-reads the file when it is saved.
///
/// Until M60 the last two needed a hand-written event loop, and the
/// sign re-cascade was written twice in it — once for the window's pump
/// and once for the agent's.
pub fn program(count: i64, w: u32, h: u32) -> fulgurite_shell::program::Program<Tally> {
    fulgurite_shell::program::application(move || Tally { count: Count { n: count }, ..build() })
        .title("fulgurite — tally")
        .size(w, h)
        .page(PAGE)
        .agent("127.0.0.1:9230")
        .accessible(cfg!(feature = "a11y"))
        .on_key(keys)
        // ANCHOR: program
        .style_lss_by(|t: &Tally| t.count.n.signum(), stylesheet_for, SHEET_PATH, sheet_block)
    // ANCHOR_END: program
}

/// The tally's own keys: Up and Down nudge the count, Escape quits.
///
/// **Only a key nothing else took** (`docs/plan-tally-slim.md` §2.1): with
/// a step button focused, Up and Down move focus along the row of four,
/// as arrows do in any toolbar, and do not reach this.
pub fn keys(c: &mut Count, key: fulgurite::Key, _: fulgurite::Mods) -> KeyOutcome {
    match key {
        fulgurite::Key::Up => {
            c.n += 1;
            KeyOutcome::Handled
        }
        fulgurite::Key::Down => {
            c.n -= 1;
            KeyOutcome::Handled
        }
        fulgurite::Key::Escape => KeyOutcome::Quit,
        _ => KeyOutcome::Ignored,
    }
}

pub fn build() -> Tally {
    let mut t = Tally::default();
    t.ui.card.caption.set("TALLY");
    t.ui.card.steps.dec10.label = "\u{2212}10".into();
    t.ui.card.steps.dec10.on_click = |t, _| t.n -= 10;
    t.ui.card.steps.dec.label = "\u{2212}1".into();
    t.ui.card.steps.dec.on_click = |t, _| t.n -= 1;
    t.ui.card.steps.inc.label = "+1".into();
    t.ui.card.steps.inc.on_click = |t, _| t.n += 1;
    t.ui.card.steps.inc10.label = "+10".into();
    t.ui.card.steps.inc10.on_click = |t, _| t.n += 10;
    t.ui.card.reset.label = "Reset".into();
    t.ui.card.reset.on_click = |t, _| t.n = 0;
    t
}

// ---- the stylesheet: `app.lss` ----------------------------------------

// ANCHOR: lss
/// The sheet's file — Lumen's `examples/counter/app.lss`, selector for
/// selector (its header says where the two differ, and why).
pub const SHEET_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/app.lss");

/// `app.lss` for each sign, parsed while this crate compiles (M61,
/// `docs/plan-lss.md`): the top-level rules, then the sign's `@sheet`.
/// Every `#id` in the file is checked against [`Page`] here, so a typo
/// is a build error at the file's line.
const POS_SHEET: &[Rule] = fulgurite::include_lss!(Page, "../app.lss", pos);
const NEG_SHEET: &[Rule] = fulgurite::include_lss!(Page, "../app.lss", neg);
const ZERO_SHEET: &[Rule] = fulgurite::include_lss!(Page, "../app.lss", zero);

/// The stylesheet for a given sign.
///
/// Lumen's counter toggles a `.lss` class (`.value.pos` / `.value.neg`)
/// and re-cascades. There is no class *toggling* here — `#[widget_class]`
/// is static, fixed in the type — so the sign-dependent rules are
/// `@sheet` blocks, swapped instead: [`program`] keys the sheet on the
/// sign (`Program::style_lss_by`), and the host re-applies it when the
/// sign changes. Same mechanism underneath: the cascade runs when the
/// sheet changes, not per frame.
pub fn stylesheet_for(sign: i64) -> &'static [Rule] {
    match sign {
        1 => POS_SHEET,
        -1 => NEG_SHEET,
        _ => ZERO_SHEET,
    }
}

/// The `@sheet` [`stylesheet_for`] compiled in for `sign` — what a debug
/// window reads from `app.lss` instead. The two must agree, and
/// `app_lss_is_the_hand_written_sheet_both_ways` holds them to it.
pub fn sheet_block(sign: i64) -> Option<&'static str> {
    Some(match sign {
        1 => "pos",
        -1 => "neg",
        _ => "zero",
    })
}
// ANCHOR_END: lss

/// **The migration's acceptance checks** — M60/T60.0
/// (`docs/plan-tally-slim.md`). The tally's own keys and its sign sheet
/// through the program alone, which is what lets the window be the
/// framework's: a headless `Program::host()` pumps exactly as the window
/// and the agent do.
#[cfg(test)]
mod tests {
    use super::*;
    use hand::{NEG, POS};
    use fulgurite::{Event, Key};
    use fulgurite_shell::Host;

    /// **The sheet the pilot replaced, kept as its reference** — M61/T61.6
    /// (`docs/plan-lss.md`). This was `stylesheet_for` until `app.lss`
    /// took over, verbatim; the two tests after it hold `app.lss` to it,
    /// rule by rule and pixel by pixel.
    mod hand {
        use fulgurite::display::Color;
        use fulgurite::prelude::*;

        pub const PAGE: Color = Color(0x0e, 0x11, 0x18, 0xff);
        pub const SURFACE: Color = Color(0x16, 0x1c, 0x28, 0xff);
        pub const INK: Color = Color(0xf1, 0xf5, 0xfb, 0xff);
        pub const MUTED: Color = Color(0x8b, 0x94, 0xa7, 0xff);
        pub const ACCENT: Color = Color(0x35, 0x63, 0xef, 0xff);
        pub const GHOST: Color = Color(0x23, 0x2c, 0x3e, 0xff);
        pub const POS: Color = Color(0x34, 0xd3, 0x99, 0xff);
        pub const NEG: Color = Color(0xfb, 0x71, 0x85, 0xff);
        pub const ZERO: Color = Color(0x9a, 0xa3, 0xb6, 0xff);
        pub const WHITE: Color = Color(0xff, 0xff, 0xff, 0xff);

        pub fn stylesheet_for(sign: i64) -> Vec<Rule> {
            let (value_fg, pill_bg) = match sign {
                1 => (POS, POS),
                -1 => (NEG, NEG),
                _ => (INK, ZERO),
            };
            let mut rules = stylesheet();
            rules.push(Rule::new(
                Selector::Id("value"),
                Patch::new().color(value_fg).font_size(76.0).font_weight(800.0),
            ));
            rules.push(Rule::new(
                Selector::Id("pill"),
                Patch::new()
                    .background(pill_bg)
                    .color(PAGE)
                    .radius(999.0)
                    .font_size(12.0)
                    .font_weight(700.0)
                    .padding(Edges { left: 14.0, right: 14.0, top: 5.0, bottom: 5.0 }),
            ));
            rules
        }

        const fn btn(bg: Color, fg: Color) -> Patch {
            Patch::new()
                .background(bg)
                .color(fg)
                .radius(13.0)
                .font_size(16.0)
                .font_weight(600.0)
                .padding(Edges { left: 16.0, right: 16.0, top: 11.0, bottom: 11.0 })
        }

        fn stylesheet() -> Vec<Rule> {
            vec![
                Rule::new(
                    Selector::Id("card"),
                    Patch::new()
                        .background(SURFACE)
                        .radius(26.0)
                        .padding(Edges::all(32.0))
                        .width(Length::Fixed(360.0))
                        .shadow(18.0)
                        .align(Align::Center),
                ),
                Rule::new(Selector::Id("caption"), Patch::new().color(MUTED).font_size(13.0).font_weight(700.0)),
                Rule::new(Selector::Id("steps"), Patch::new().align(Align::Center)),
                Rule::new(
                    Selector::Kind("Page"),
                    Patch::new().background(PAGE).padding(Edges::all(24.0)).align(Align::Center),
                ),
                Rule::new(Selector::Id("reset"), btn(Color(0x1b, 0x23, 0x33, 0xff), MUTED)),
                Rule::new(Selector::Id("dec10"), btn(GHOST, INK)),
                Rule::new(Selector::Id("dec"), btn(GHOST, INK)),
                Rule::new(Selector::Id("inc"), btn(ACCENT, WHITE)),
                Rule::new(Selector::Id("inc10"), btn(ACCENT, WHITE)),
            ]
        }
    }

    const SIGNS: [i64; 3] = [-1, 0, 1];

    /// `app.lss` is the hand-written sheet, rule for rule, for every sign
    /// — through `include_lss!` (what a release window draws with) and
    /// through `lss::load` (what a debug window reads), so the pair
    /// [`program`] hands `Program::style_lss_by` agrees with itself too.
    #[test]
    fn app_lss_is_the_hand_written_sheet_both_ways() {
        let mut differ = Vec::new();
        for sign in SIGNS {
            let compiled = stylesheet_for(sign);
            let hand = hand::stylesheet_for(sign);
            let read = fulgurite_shell::lss::load(SHEET_PATH, sheet_block(sign)).unwrap_or_else(|e| panic!("{e}"));
            println!(
                "sign {sign:>2}: include_lss! {} rules, hand-written {}, lss::load {}",
                compiled.len(),
                hand.len(),
                read.len()
            );
            for (i, h) in hand.iter().enumerate() {
                if compiled.get(i) != Some(h) {
                    differ.push(format!("sign {sign}, rule {i}:\n  app.lss: {:?}\n  hand:    {h:?}", compiled.get(i)));
                }
            }
            if compiled.len() != hand.len() {
                differ.push(format!("sign {sign}: {} rules in app.lss, {} by hand", compiled.len(), hand.len()));
            }
            if read != compiled {
                differ.push(format!("sign {sign}: lss::load and include_lss! disagree"));
            }
        }
        assert!(differ.is_empty(), "{}", differ.join("\n"));
    }

    /// **The frames are the hand-written sheet's frames** — T61.6's gate,
    /// headless: the tally at -3, 0 and 7 rendered from `app.lss` compiled
    /// in, from `app.lss` read live (`Program::live_lss`, the debug
    /// window's path), and from the hand-written sheet; every byte equal.
    /// The differing pixels are counted and printed, so a failure says how
    /// much moved.
    #[test]
    fn the_lss_frames_are_the_hand_written_frames() {
        use fulgurite_shell::software::Canvas;
        fn frame(program: fulgurite_shell::program::Program<Tally>) -> Vec<u8> {
            let (w, h) = (program.width, program.height);
            let mut host: Host<Tally> = program.host();
            host.pump();
            host.pump();
            let mut canvas = Canvas::new(w, h);
            canvas.clear(PAGE);
            host.render(&mut canvas);
            canvas.to_ppm()
        }
        fn pixels_apart(a: &[u8], b: &[u8]) -> usize {
            a.chunks(3).zip(b.chunks(3)).filter(|(x, y)| x != y).count() + a.len().abs_diff(b.len()) / 3
        }
        let mut bad = Vec::new();
        for count in [-3, 0, 7] {
            let hand = frame(program(count, 440, 520).style_by(|t: &Tally| t.count.n.signum(), hand::stylesheet_for));
            let compiled = frame(program(count, 440, 520));
            let live = frame(program(count, 440, 520).live_lss());
            let (c, l) = (pixels_apart(&compiled, &hand), pixels_apart(&live, &hand));
            println!("count {count:>2}: {} px; compiled differs by {c} px, live by {l} px (bound 0)", (440 * 520));
            assert_ne!(hand, frame(program(count, 440, 520).style(&[][..])), "setup: the sheet changes nothing on screen");
            if c + l != 0 {
                bad.push(format!("count {count}: compiled {c} px, live {l} px apart from the hand-written frame"));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    fn press(h: &mut Host<Tally>, key: Key) -> bool {
        h.post(Event::Key { key, down: true });
        h.post(Event::Key { key, down: false });
        h.pump()
    }

    fn value_colour(h: &Host<Tally>) -> Color {
        let i = fulgurite::style::index_of_wid(<Page as Widget>::SUB_TABLE, "value").expect("#value");
        h.styles[i].color
    }

    #[test]
    fn up_down_and_escape_are_the_programs_and_the_sheet_follows_the_sign() {
        let mut h: Host<Tally> = program(0, 440, 520).host();
        h.pump();
        let handled = press(&mut h, Key::Up);
        println!(
            "Up: count {} (want 1), handled {handled}, #value {:?}, pill {:?}, colour {:?} (want POS {POS:?})",
            h.app.count.n,
            h.app.ui.card.value.as_str(),
            h.app.ui.card.pill.as_str(),
            value_colour(&h)
        );
        assert_eq!(h.app.count.n, 1, "Up did not nudge the count");
        assert_eq!(h.app.ui.card.pill.as_str(), "POSITIVE");
        assert_eq!(value_colour(&h), POS, "the count went positive and #value kept the zero sheet");
        press(&mut h, Key::Down);
        press(&mut h, Key::Down);
        println!("Down twice: count {} (want -1), colour {:?} (want NEG {NEG:?})", h.app.count.n, value_colour(&h));
        assert_eq!(h.app.count.n, -1, "Down did not nudge the count");
        assert_eq!(value_colour(&h), NEG, "the count went negative and #value kept the positive sheet");
        assert!(!h.quit_requested());
        press(&mut h, Key::Escape);
        println!("Escape: quit requested {}", h.quit_requested());
        assert!(h.quit_requested(), "Escape did not ask to quit");
    }

    /// The accept direction of the check below: it can see a loop when
    /// one is there.
    const LOOP_MARKERS: [&str; 3] = ["ApplicationHandler", "EventLoop", "run_app"];

    #[test]
    fn the_example_runs_no_event_loop_of_its_own() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = Vec::new();
        let mut files = 0;
        for e in std::fs::read_dir(&src).expect("src/") {
            let p = e.expect("an entry").path();
            if p.extension().is_some_and(|x| x == "rs") {
                files += 1;
                let text = std::fs::read_to_string(&p).expect("readable");
                for m in LOOP_MARKERS {
                    // This file names the markers to look for them.
                    if text.contains(m) && !p.ends_with("app.rs") {
                        found.push(format!("{}: {m}", p.display()));
                    }
                }
            }
        }
        println!("{files} source files scanned; loop markers found: {found:?} (bound none)");
        assert!(files >= 2, "setup: found {files} source files");
        assert!(found.is_empty(), "the tally still runs an event loop of its own:\n{}", found.join("\n"));
    }
}
