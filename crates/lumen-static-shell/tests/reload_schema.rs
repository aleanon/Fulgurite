//! **A kept value whose schema moved either survives or is named** —
//! M49/T49.0, R5-F3 and R5-F4 (`docs/plan-reload-hardening.md` §1.4).
//!
//! Two builds of one application, `V1` and `V2`, with the same field
//! names and different types — the swap's sequence without the dylib:
//! save from v1, drain the log (as `hot::boot` does, because the save
//! runs in the old library's copy of the core), restore onto v2's fresh
//! value. Every row must end **at the fresh value or with a log line**;
//! the kept struct that gained a field must keep the fields it had. The
//! table prints change, before, after and the lines each row logged.

use std::collections::HashMap;

use lumen_static::reload::{take_log, Reloadable};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct SettingsV1 {
    pub a: u32,
    pub b: String,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct SettingsV2 {
    pub a: u32,
    pub b: String,
    pub c: bool,
}

/// Keys that are not strings: serde_json cannot save this map at all.
pub type Grid = HashMap<(u8, u8), u32>;

#[derive(lumen_static::Reloadable, Default)]
pub struct V1 {
    #[reload(keep)]
    pub ratio: Option<f32>,
    #[reload(keep)]
    pub cells: Option<Grid>,
    #[reload(keep)]
    pub settings: SettingsV1,
    #[reload(keep)]
    pub n: u32,
}

#[derive(lumen_static::Reloadable)]
pub struct V2 {
    #[reload(keep)]
    pub ratio: Option<f32>,
    #[reload(keep)]
    pub cells: Option<Grid>,
    #[reload(keep)]
    pub settings: SettingsV2,
    #[reload(keep)]
    pub n: u64,
}

/// v2's boot values: what a field falls back to.
impl Default for V2 {
    fn default() -> Self {
        V2 {
            ratio: Some(1.5),
            cells: Some(HashMap::from([((0, 0), 1)])),
            settings: SettingsV2::default(),
            n: 0,
        }
    }
}

/// Save v1, drain the log, restore onto v2's boot value: what `hot::boot`
/// does. Returns the restored value and the lines the restore logged.
fn swap() -> (V2, Vec<String>) {
    let v1 = V1 {
        ratio: Some(f32::NAN),
        cells: Some(HashMap::from([((3, 4), 9)])),
        settings: SettingsV1 { a: 7, b: "kept?".into() },
        n: 7,
    };
    let saved = v1.save();
    let _ = take_log();
    let mut v2 = V2::default();
    v2.restore(&saved);
    let log = take_log();
    for l in &log {
        println!("log: {l}");
    }
    println!("{:<34} {:>22} {:>34} {:>6}", "change", "before", "after", "lines");
    (v2, log)
}

fn lines(log: &[String], key: &str) -> usize {
    log.iter().filter(|l| l.contains(&format!("V2.{key}:"))).count()
}

/// R5-F3: a kept field whose save failed, or whose value serde writes as
/// null, keeps its fresh value and says so — it does not become `None`.
#[test]
#[ignore = "red until T49.6: R5-F3, a failed save restores None with no line"]
fn a_value_that_did_not_save_keeps_its_fresh_value_and_says_so() {
    let (v2, log) = swap();
    let fresh = V2::default();
    let mut bad = Vec::new();

    let n = lines(&log, "ratio");
    println!("{:<34} {:>22} {:>34} {:>6}", "Option<f32>: Some(NaN) saved", "Some(NaN)", format!("{:?}", v2.ratio), n);
    if !(v2.ratio == fresh.ratio && n == 1) {
        bad.push("Option<f32>: NaN must keep the fresh Some(1.5), with one line");
    }

    let n = lines(&log, "cells");
    println!("{:<34} {:>22} {:>34} {:>6}", "Option<HashMap>: save fails", "Some({(3, 4): 9})", format!("{:?}", v2.cells), n);
    if !(v2.cells == fresh.cells && n == 1) {
        bad.push("Option<HashMap>: a failed save must keep the fresh value, with one line");
    }

    // The other direction: a widened integer keeps its value, silently.
    let n = lines(&log, "n");
    println!("{:<34} {:>22} {:>34} {:>6}", "u32 -> u64", "7", v2.n, n);
    if !(v2.n == 7 && n == 0) {
        bad.push("u32 -> u64 must keep 7 with no line");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// R5-F4: a kept struct that gained a field keeps the fields it had, and
/// names the one it did not.
#[test]
#[ignore = "red until T49.7: R5-F4, a struct that gained a field resets whole"]
fn a_struct_that_gained_a_field_keeps_the_fields_it_had() {
    let (v2, log) = swap();
    let s = &v2.settings;
    let named: Vec<&String> = log.iter().filter(|l| l.contains("V2.settings")).collect();
    println!("{:<34} {:>22} {:>34} {:>6}", "Settings {a,b} -> {a,b,c}", "a 7, b \"kept?\"", format!("{s:?}"), named.len());
    assert!(s.a == 7 && s.b == "kept?" && !s.c, "a field added reset the fields that were kept: {s:?}");
    assert_eq!(named.len(), 1, "exactly one line for `settings`: {named:?}");
    assert!(named[0].contains("settings.c") || named[0].contains("`c`"), "the line must name `c`: {}", named[0]);
}
