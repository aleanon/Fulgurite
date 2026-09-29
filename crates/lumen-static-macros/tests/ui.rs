//! Compile-fail and compile-pass cases for the three derives.
//!
//! The crate had 976 lines and no tests. Its only coverage was whatever
//! spellings the ~100 downstream tests happened to type — which is how
//! `#[widget_class("danger")]`, the *documented* form, silently produced
//! an empty class list with zero uses anywhere in the repo to notice
//! (`docs/plan-review-fixes.md` F6.8).
//!
//! The derive emits many diagnostics and several type-level guarantees.
//! Each `.stderr` here locks a message in place, so a change to a span or
//! a wording shows up as a diff rather than as a worse error someone
//! meets later. **How many is not stated here** (M55, R16-F10's rule 4):
//! `scripts/check_derive_errors.sh` counts the derive's error sites on
//! every `just static` and fails on any sentence no `.stderr` pins.
//!
//! Refresh expected output with `TRYBUILD=overwrite cargo test -p
//! lumen-static-macros --test ui`.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/fail_*.rs");
    t.pass("tests/ui/pass_*.rs");
}

/// M49 (`docs/plan-reload-hardening.md`): the `Reloadable` derive as a hot
/// window library meets it. Its own directory, so a red case here can be
/// ignored without ignoring the twenty above.
#[test]
fn reload_program() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui-reload/pass_*.rs");
}

/// M49/T49.8 (R5-F7): the derive's errors land on the field and name the
/// attribute that fixes them.
#[test]
fn reload_diagnostics() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui-reload/fail_*.rs");
}


/// M61/T61.0 gate 2 (`docs/plan-lss.md` §2.1): every refusal is a compile
/// error that names the `.lss` file, line and column, and the reason; and
/// every accepted construct lowers to the rules a person would write.
#[test]
fn lss_errors() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui-lss/fail_*.rs");
    t.pass("tests/ui-lss/pass_*.rs");
}

/// M61/T61.0 gate 3 (`docs/plan-lss.md` §2.3): an `#id` or a `.class` the
/// tree does not have fails to compile, by name. (A kind does not: that
/// is W1010 at run time, pinned by `lumen-static-agent`'s `tests/lss_kind.rs`.)
#[test]
fn lss_tree_checks() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui-lss/tree_fail_*.rs");
}
