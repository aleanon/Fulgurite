//! Compile-fail and compile-pass cases for the three derives.
//!
//! The crate had 976 lines and no tests. Its only coverage was whatever
//! spellings the ~100 downstream tests happened to type — which is how
//! `#[widget_class("danger")]`, the *documented* form, silently produced
//! an empty class list with zero uses anywhere in the repo to notice
//! (`docs/plan-review-fixes.md` F6.8).
//!
//! The derive emits twenty distinct diagnostics and several type-level
//! guarantees. Each `.stderr` here locks a message in place, so a change
//! to a span or a wording shows up as a diff rather than as a worse
//! error someone meets later.
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
