//! **The two lowerings of a `.lss` agree** — M61/T61.5, T61.0's item 5
//! (`docs/plan-lss.md`).
//!
//! `include_lss!` turns a sheet into `const` tokens at build time;
//! `lss::load` turns the same sheet into values at run time. They share
//! the parser and the lowering (`fulgurite_lss`) and nothing after
//! it, so a setter or a value mapped differently on one side is exactly
//! what this can see — named by rule and selector.
//!
//! Every `.lss` in the repository is compared by some test, and
//! `scripts/check_lss_coverage.sh` refuses one that is not. The sheets
//! whose trees live here are compared here; the tally's
//! (`examples/counter-window/app.lss`) and the showcase's
//! (`examples/showcase/showcase.lss`) beside their trees.

use fulgurite::{include_lss, Rule, StaticTree};
use fulgurite_shell::lss;
use fulgurite_widgets::ops::{Fixed, Label};

/// One tree carrying every id and class the sheets below name, so each
/// `include_lss!` passes its tree checks.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
    #[widget_id("badge")]
    badge: Fixed,
    #[widget_class("note")]
    note: Label,
}

fn root(rel: &str) -> String {
    format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"))
}

/// Compare, rule by rule, and say which differ.
fn same(what: &str, compiled: &[Rule], read: &[Rule]) -> Vec<String> {
    println!("{what}: include_lss! {} rules, load {} rules", compiled.len(), read.len());
    let mut out: Vec<String> = compiled
        .iter()
        .zip(read)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (a, b))| format!("{what}: rule {i} ({:?}) differs:\n  include_lss!: {a:?}\n  load:         {b:?}", a.selector))
        .collect();
    if compiled.len() != read.len() {
        out.push(format!("{what}: {} rules compiled, {} read", compiled.len(), read.len()));
    }
    out
}

#[test]
fn include_lss_and_load_agree_on_every_sheet_here() {
    // Each sheet's path is on a line of its own, beside the `include_lss!`
    // of it: `scripts/check_lss_coverage.sh` reads this file for both.
    let cases: [(&str, &[Rule], Option<&str>); 4] = [
        (
            "fixtures/embedded/src/sheet.lss",
            include_lss!(Ui, "../../../fixtures/embedded/src/sheet.lss"),
            None,
        ),
        (
            "crates/fulgurite-macros/tests/ui-lss/pass_accept.lss",
            include_lss!(Ui, "../../fulgurite-macros/tests/ui-lss/pass_accept.lss"),
            None,
        ),
        (
            "crates/fulgurite-macros/tests/ui-lss/pass_accept.lss",
            include_lss!(Ui, "../../fulgurite-macros/tests/ui-lss/pass_accept.lss", loud),
            Some("loud"),
        ),
        (
            "crates/fulgurite-agent/tests/lss_kind.lss",
            include_lss!(Ui, "../../fulgurite-agent/tests/lss_kind.lss"),
            None,
        ),
    ];
    let mut differ = Vec::new();
    for (path, compiled, block) in cases {
        let read = lss::load(root(path), block).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert!(!compiled.is_empty(), "setup: {path} lowered to no rules");
        differ.extend(same(&format!("{path} {block:?}"), compiled, &read));
    }
    assert!(differ.is_empty(), "{}", differ.join("\n"));
}

/// The refusing direction: every `.lss` the macro refuses, `load` refuses
/// with the same message — read off the case's pinned `.stderr`.
#[test]
fn load_refuses_every_sheet_the_macro_refuses_with_the_same_message() {
    let dir = root("crates/fulgurite-macros/tests/ui-lss");
    let mut checked = 0;
    let mut wrong = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dir).expect("ui-lss").map(|e| e.expect("entry").path()).collect();
    entries.sort();
    for lss_path in entries {
        let name = lss_path.file_name().unwrap().to_string_lossy().into_owned();
        if !(name.starts_with("fail_") && name.ends_with(".lss")) {
            continue;
        }
        let stderr = std::fs::read_to_string(lss_path.with_extension("stderr")).expect("a pinned .stderr");
        // Only the cases whose error is in the file itself: the macro's
        // own (`no @sheet`) are about the invocation, not the sheet.
        if !stderr.starts_with(&format!("error: {name}:")) {
            continue;
        }
        checked += 1;
        match lss::load(&lss_path, None) {
            Ok(rules) => wrong.push(format!("{name}: load accepted it, {} rules", rules.len())),
            Err(e) => {
                for m in &e.messages {
                    let tail = m.rsplit_once(&format!("{name}:")).map_or(m.as_str(), |(_, t)| t);
                    let pinned = format!("error: {name}:{tail}");
                    if !stderr.contains(&pinned) {
                        wrong.push(format!("{name}: load says {m:?}, which the .stderr does not"));
                    }
                }
            }
        }
    }
    println!("{checked} refused sheets compared (bound >= 18)");
    assert!(checked >= 18, "setup: only {checked} refused sheets found in {dir}");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
