//! **Today, a panic at any of the sites kills the process** —
//! M36/T36.0 (`docs/plan-panic-recovery.md`).
//!
//! Every site in `panic_app::SITES` — seven at M36, the caret at M80/T80.5,
//! `App::wake_in_ms` at M84/T84.3, `App::images` at M84/T84.6 — run through
//! the unguarded headless binary.
//!
//! The defect, shown, before anything recovers from it: the binary runs
//! the unguarded `Host::pump`, arms one site, and must *die*, with the
//! site's own line in its stderr. The unarmed run is the other direction:
//! it exits cleanly, so "it died" is known to be the site and not the
//! harness. Site 7's panic is raised inside the framework; its location
//! must be under `crates/`, which is what makes T36.1's "application
//! frame" filter have real work to do.

use std::process::Command;

fn run(site: u8) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_panic-app"))
        .arg(site.to_string())
        .output()
        .expect("run panic-app");
    (out.status.success(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn every_site_kills_the_process_today() {
    for (site, what) in panic_app::SITES {
        let (ok, stderr) = run(site);
        let at = stderr.lines().find(|l| l.contains("panicked at")).unwrap_or("").to_string();
        println!("site {site} ({what}): exit ok={ok}, {at}");
        assert!(!ok, "site {site} ({what}) did not die — is it reached at all?");
        if site == 7 {
            assert!(at.contains("crates/lumen-static/src/"), "site 7's panic should be inside the framework: {at}");
        } else {
            let want = format!("fixtures/panic-app/src/lib.rs:{}", panic_app::site_line(site));
            assert!(at.contains(&want), "site {site}: expected {want} in {at:?}");
        }
    }
}

#[test]
fn an_unarmed_run_exits_cleanly() {
    let (ok, stderr) = run(0);
    assert!(ok, "the unarmed run died: {stderr}");
}
