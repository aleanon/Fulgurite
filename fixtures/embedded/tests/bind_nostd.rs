//! **Core's `no_std` bind branch, executed** (M43/T43.8, R12-F2).
//!
//! `fulgurite::bind::format_into` has two bodies. With `std` it formats
//! into a reused thread-local `String`; without, into a stack
//! `InlineText<SCRATCH_CAP>` of 256 bytes, which truncates at a `char`
//! boundary where the `std` path would grow. Until M43 no gate ran the
//! second body: core's own tests build with `std`, and nothing in
//! `fixtures/embedded` held a `#[bind]`.
//!
//! This test does, and it is in the TEST, not in the fixture's library, so
//! the firmware's sections do not move. `fixtures/embedded` depends on core
//! with `default-features = false` and has no dev edge that turns `std`
//! on, so `cargo test -p embedded` -- which `just thumb` runs -- builds the
//! `no_std` branch. **The stored byte count is the proof**: the label below
//! holds 1,024 bytes, so under `std` a 1,000-byte value is stored whole,
//! and only the `no_std` scratch stops it at 256.
//!
//! Inside `cargo test --workspace` other members unify `std` into core and
//! the `std` body runs instead; the test accepts that (1,000 stored) and
//! says so. `just thumb` sets `FULGURITE_REQUIRE_NOSTD=1`, under which only
//! the `no_std` answer passes.

use core::fmt;

use fulgurite::prelude::*;
use fulgurite::event::Wrote;
use fulgurite::{wid_index, InlineText};
use fulgurite_widgets::prelude::*;

const CAP: usize = 256;

/// `n` bytes of text, made of `unit` repeated.
struct Repeat(&'static str, usize);
impl fmt::Display for Repeat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for _ in 0..self.1 {
            f.write_str(self.0)?;
        }
        Ok(())
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("head")]
    head: Label,
    #[widget_id("out")]
    out: Label<InlineText<1024>>,
}

#[derive(DeriveApp)]
struct Readout {
    #[tree]
    ui: Ui,
    #[bind(ui.out, "{}")]
    value: Repeat,
}

fn app(value: Repeat) -> Readout {
    Readout { ui: Ui::default(), value }
}

fn required() -> bool {
    std::env::var("FULGURITE_REQUIRE_NOSTD").is_ok_and(|v| v == "1")
}

#[test]
fn a_bound_label_stores_the_scratch_cap_and_no_more() {
    let mut a = app(Repeat("x", 1000));
    // The count is printed and judged before whether the binding "wrote":
    // a branch that stores nothing also reports no write, and the number
    // is the diagnosis.
    let wrote = a.update();
    let n = a.ui.out.as_str().len();
    let branch = if n == 1000 { "std (unified by the workspace)" } else { "no_std" };
    println!("stored {n} of 1000, cap {CAP} -- the {branch} branch ran");
    if required() {
        assert_eq!(n, CAP, "FULGURITE_REQUIRE_NOSTD=1: stored {n} of 1000, expected the no_std cap {CAP}");
    } else {
        assert!(n == CAP || n == 1000, "stored {n} of 1000: neither the no_std cap {CAP} nor the whole value");
    }
    assert!(wrote, "the binding wrote");
    assert!(a.ui.out.as_str().bytes().all(|b| b == b'x'), "what was stored is a prefix of the value");
    // A second update with the same value writes nothing.
    assert!(!a.update(), "an unchanged value reports no write");
}

#[test]
fn truncation_lands_on_a_char_boundary() {
    // 255 ASCII bytes, then two-byte chars: the 256th byte is the first
    // half of an 'é', so the no_std branch must stop at 255.
    struct Mixed;
    impl fmt::Display for Mixed {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for _ in 0..255 {
                f.write_str("a")?;
            }
            for _ in 0..100 {
                f.write_str("é")?;
            }
            Ok(())
        }
    }
    let mut ui = Ui::default();
    fulgurite::bind::format_into(&mut ui.out, format_args!("{}", Mixed));
    let s = ui.out.as_str();
    println!("mixed: stored {} bytes, {} chars", s.len(), s.chars().count());
    if required() || s.len() < 455 {
        assert_eq!(s.len(), 255, "cut inside a char: {} bytes", s.len());
    }
    assert!(s.is_char_boundary(s.len()));
}

#[test]
fn update_marked_reports_the_bound_node() {
    const OUT: usize = wid_index!(Ui, "out");
    let mut a = app(Repeat("y", 10));
    let mut marked = Vec::new();
    let w = a.update_marked(&mut |r| marked.push(r));
    println!("update_marked: {w:?}, ranges {marked:?}, label at {OUT}");
    assert!(matches!(w, Wrote::Marked), "a format binding reports where it wrote, got {w:?}");
    assert!(marked.iter().any(|r| r.contains(&OUT)), "no range covers the bound label {OUT}: {marked:?}");
    assert!(!marked.iter().any(|r| r.contains(&wid_index!(Ui, "head"))), "the unbound label was marked");
}
