//! A headless window loader: swap `fixtures/reload-app` from v1 to v2
//! and check the swapped session against v2 run from the start —
//! M37/T37.1 (`docs/plan-window-reload.md`).
//!
//! `reload-host <v1.so> <v2.so> <v3.so> <v-panic.so> <v-grow.so> <v4.so>
//! <chain-v1.so> <chain-v2.so>`. The exit code is the verdict.
//!
//! **What is compared.** v1 is clicked once, saved, dropped; v2 boots
//! its own program with those bytes. A second v2 session boots with
//! nothing and is clicked once. The two frames must agree command for
//! command and byte for byte: the restore produced exactly the state the
//! input would have, under the new code.

use std::path::Path;

use fulgurite::display::Cmd;
use fulgurite::request::{Answer, ClipFormat, ClipKind, Request};
use fulgurite::Event;
use fulgurite_reload::hot::{Build, Hot, Outcome, Reloader};
use fulgurite_shell::session::{FrameSource, Session};

const W: u32 = 240;
const H: u32 = 120;
/// The button fills the window's width and the top 60 px.
const AT: (f32, f32) = (120.0, 30.0);
/// `chain-app`'s buttons share the top 60 px — two since M88/T88.1,
/// three since M88/T88.3, a third each: `#start`, `#sip`, `#ask`.
const CHAIN_AT: (f32, f32) = (40.0, 30.0);
const SIP_AT: (f32, f32) = (120.0, 30.0);
const ASK_AT: (f32, f32) = (200.0, 30.0);

fn text(s: &Hot) -> String {
    let l = s.list();
    let mut out = Vec::new();
    for c in &l.cmds {
        if let Cmd::Text { text, .. } = c {
            out.push(l.str_of(*text).to_string());
        }
    }
    out.join(" | ")
}

fn text_of(s: &dyn Session) -> String {
    let l = s.list();
    let mut out = Vec::new();
    for c in &l.cmds {
        if let Cmd::Text { text, .. } = c {
            out.push(l.str_of(*text).to_string());
        }
    }
    out.join(" | ")
}

fn click(s: &mut Hot) {
    s.post(Event::down(AT.0, AT.1));
    s.post(Event::up(AT.0, AT.1));
    s.frame(16.0, W, H);
}

fn open(p: &str) -> Build {
    match Build::open(Path::new(p)) {
        Ok(b) => b,
        Err(e) => panic!("{p} refused: {e}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let v1 = open(&args[0]);
    println!("stamp: {} agreed", args[0]);

    let mut a = v1.boot(0, None).expect("v1 boots");
    a.frame(0.0, W, H);
    let before = text(&a);
    println!("v1 fresh:   {before}");
    assert!(before.contains("count 0"), "v1's first frame: `{before}`");
    click(&mut a);
    let clicked = text(&a);
    println!("v1 clicked: {clicked}");
    // The setup must happen: a click that missed leaves `count 0`, and
    // everything after would compare two sessions nobody touched.
    assert!(clicked.contains("count 1"), "the click did not land: `{clicked}`");

    let bytes = a.save();
    println!("saved {} bytes: {}", bytes.len(), String::from_utf8_lossy(&bytes));
    drop(a); // dropped inside v1, which stays loaded (§2.5)

    let v2 = open(&args[1]);
    let mut b = v2.boot(0, Some(&bytes)).expect("v2 boots with v1's state");
    b.frame(0.0, W, H);
    let swapped = text(&b);

    let mut c = v2.boot(0, None).expect("v2 boots fresh");
    c.frame(0.0, W, H);
    click(&mut c);
    // The pointer is still over the button, which is hover-shaded; the
    // swapped session never had a pointer. Hover is not kept state (the
    // first run of this gate compared the two and found exactly that
    // shade), so the pointer leaves before the frames are compared.
    c.post(Event::PointerMove { x: 230.0, y: 115.0 });
    c.frame(16.0, W, H);
    let direct = text(&c);
    println!("v2 swapped: {swapped}");
    println!("v2 direct:  {direct}");

    assert!(
        swapped.contains("clicks: 1"),
        "after the swap the frame must show the NEW format with the OLD count; it shows `{swapped}`"
    );
    assert_eq!(swapped, direct, "the swapped frame's text differs from v2 run directly");
    assert_eq!(
        b.list().cmds,
        c.list().cmds,
        "the swapped frame's commands differ from v2 run directly"
    );
    assert_eq!(b.list().text, c.list().text);
    println!("  ok: {} commands, byte for byte", b.list().cmds.len());
    // ---- T37.2: when the window swaps, and when it must not ---------
    //
    // `Reloader` is the window's swap logic without the window. A failed
    // build must leave the running library in place — its `output` names
    // v2's library, as a stale file from an earlier build would — and
    // the next good build must swap.
    let mut r = Reloader::new();
    let mut cur: Box<dyn Session> = Box::new(v1.boot(0, None).expect("v1 boots"));
    cur.frame(0.0, W, H);
    cur.post(Event::down(AT.0, AT.1));
    cur.post(Event::up(AT.0, AT.1));
    cur.frame(16.0, W, H);
    assert!(text_of(&*cur).contains("count 1"), "the click did not land: `{}`", text_of(&*cur));
    let adopt = |s: &mut dyn Session| {
        s.frame(0.0, W, H);
    };

    r.outcome(Outcome::Failed {
        log: "error[E0308]: mismatched types\n --> src/lib.rs:40:9".into(),
        output: std::path::PathBuf::from(&args[1]),
    });
    let swapped = r.swap(&mut cur, adopt);
    cur.frame(16.0, W, H);
    let after_failure = text_of(&*cur);
    println!("after a failed build: {after_failure} (swapped: {swapped}, reloads {})", r.reloads);
    assert!(
        after_failure.contains("count 1") && !swapped,
        "a failed build swapped the running library: the frame is `{after_failure}`, not v1's"
    );
    assert!(r.error.as_deref().is_some_and(|e| e.contains("E0308")), "the build log must be kept to show");

    r.outcome(Outcome::Built(std::path::PathBuf::from(&args[1])));
    let pending = r.pending().map(|p| p.display().to_string());
    assert!(r.swap(&mut cur, adopt), "the next good build must swap");
    let after_good = text_of(&*cur);
    println!("after a good build:   {after_good} (from {}, reloads {})", pending.unwrap_or_default(), r.reloads);
    assert!(after_good.contains("clicks: 1"), "the good build's frame: `{after_good}`");
    assert_eq!(r.error, None, "a successful swap clears the error");
    assert_eq!(r.reloads, 1);
    // M49/T49.7's other direction: nothing about the kept state changed
    // between v1 and v2, so nothing may be logged.
    assert_eq!(r.warning, None, "an unchanged kept struct logged: {:?}", r.warning);
    // M49/T49.11's other direction: a format edit keeps the tree's shape.
    println!("v1 -> v2 shape changed: {} (bound: false)", r.shape_changed);
    assert!(!r.shape_changed, "a format edit reported a shape change");

    // The same bytes again change nothing, so nothing swaps.
    r.outcome(Outcome::Built(std::path::PathBuf::from(&args[1])));
    assert!(r.pending().is_none(), "an unchanged library was queued to swap");
    // A failure, then the source fixed back to what last built: the
    // rebuilt library is byte-identical to the running one, so nothing
    // swaps -- but the failure must not stay on screen.
    r.outcome(Outcome::Failed { log: "error: expected `;`".into(), output: std::path::PathBuf::from(&args[1]) });
    r.outcome(Outcome::Built(std::path::PathBuf::from(&args[1])));
    assert!(r.pending().is_none() && r.error.is_none(), "a fixed build left the error up: {:?}", r.error);
    println!("  ok: a failed build kept v1; the good one swapped; an unchanged one did not");

    // ---- T37.3: kept state whose type changed -------------------------
    //
    // v3 changes the kept `name` from a `String` to a `u32`. The swap
    // must complete; `name` must hold v3's boot value; the warning must
    // name it; and `tally` must survive — per field, not all or nothing.
    let saved = String::from_utf8_lossy(&cur.save()).into_owned();
    r.outcome(Outcome::Built(std::path::PathBuf::from(&args[2])));
    assert!(r.swap(&mut cur, adopt), "a changed field's type must not refuse the swap");
    cur.frame(16.0, W, H);
    let after = text_of(&*cur);
    let warning = r.warning.clone().unwrap_or_default();
    println!("saved before v3: {saved}");
    println!("after v3:        {after}");
    println!("warning:         {warning}");
    assert!(warning.contains("name"), "the warning must name the field that reset: `{warning}`");
    assert!(after.contains("name 7"), "`name` must hold v3's boot value: `{after}`");
    assert!(after.contains("clicks: 1"), "every other kept field survives -- `tally` did not: `{after}`");
    assert!(r.error.is_none(), "a reset field is a warning, not a failure");
    println!("  ok: `name` reset to its boot value and was named; `tally` kept count 1");

    // ---- M49/T49.0: a boot that panics keeps the running session ----
    //
    // R4-F5's window half. The re-triage probed it and found it fixed;
    // this is the gate nothing had committed. `v-panic.so` is v3 with a
    // boot that panics: the swap must not happen, the error must carry
    // the library's own report -- the phrase and a `file:line` -- and the
    // running session must still take a click.
    r.outcome(Outcome::Built(std::path::PathBuf::from(&args[3])));
    let swapped = r.swap(&mut cur, adopt);
    let err = r.error.clone().unwrap_or_default();
    println!("after a panicking boot: swapped {swapped}, reloads {}, error: {err}", r.reloads);
    assert!(!swapped, "a panicking boot swapped: reloads {}", r.reloads);
    assert!(
        err.contains("panicked while booting") && err.contains("lib.rs:"),
        "the error must say the boot panicked and where: `{err}`"
    );
    cur.post(Event::down(AT.0, AT.1));
    cur.post(Event::up(AT.0, AT.1));
    cur.frame(16.0, W, H);
    let after = text_of(&*cur);
    println!("after a click:          {after}");
    assert!(after.contains("clicks: 2"), "a panicking boot lost the session: `{after}`");
    println!("  ok: the panicking build was refused and the running session still counts");

    // ---- M49/T49.7: a kept struct that gained a field ---------------
    //
    // `v-grow.so` is v3 with a field added to the kept `Tally`. The
    // taught pattern keeps all application state in one such struct, so
    // restoring it whole-or-not-at-all reset every kept value on the
    // first edit that added one (R5-F4). The count must survive, and the
    // warning must name the new field.
    r.outcome(Outcome::Built(std::path::PathBuf::from(&args[4])));
    assert!(r.swap(&mut cur, adopt), "a grown kept struct must not refuse the swap");
    cur.frame(16.0, W, H);
    let after = text_of(&*cur);
    let warning = r.warning.clone().unwrap_or_default();
    println!("after v-grow:  {after}");
    println!("warning:       {warning}");
    assert!(after.contains("clicks: 2"), "a field added to the kept struct reset its count: `{after}`");
    assert!(warning.contains("tally.step"), "the warning must name the new field: `{warning}`");
    println!("  ok: `Tally` gained `step`; the count 2 survived and `tally.step` was named");

    // ---- M49/T49.11: a swap says when the shape changed ---------------
    //
    // v4 adds a label before `#value`, so every index after it moves: an
    // agent holding `#value`'s index from before the swap would now
    // address another node (R4-F7). The swap must say the shape changed.
    let value_index = |p: &str| -> usize {
        // SAFETY: the fixture's own `extern "C" fn() -> usize`.
        let lib = unsafe { libloading::Library::new(p) }.expect("load");
        let f: libloading::Symbol<extern "C" fn() -> usize> =
            unsafe { lib.get(b"reload_app_value_index") }.expect("the fixture exports its #value index");
        let i = f();
        std::mem::forget(lib); // never unloaded, as every build here
        i
    };
    let (before, after_i) = (value_index(&args[4]), value_index(&args[5]));
    let shape_before_grow = r.shape_changed;
    r.outcome(Outcome::Built(std::path::PathBuf::from(&args[5])));
    assert!(r.swap(&mut cur, adopt), "v4 must swap");
    cur.frame(16.0, W, H);
    let after = text_of(&*cur);
    println!("#value index: v-grow {before}, v4 {after_i}; v3 -> v-grow shape changed {shape_before_grow}, v-grow -> v4 {}", r.shape_changed);
    println!("after v4:      {after}");
    assert_ne!(before, after_i, "setup: v4 did not move #value, so this cannot test a shape change");
    assert!(!shape_before_grow, "a kept-struct edit reported a shape change");
    assert!(r.shape_changed, "a swap that moved #value from {before} to {after_i} reported no shape change");
    assert!(after.contains("clicks: 2"), "the swap lost the count: `{after}`");
    println!("  ok: the swap reported the shape change; #value moved {before} -> {after_i}");

    chain_across_a_swap(&args[6], &args[7]);
    sipper_across_a_swap(&args[6], &args[7]);
    request_across_a_swap(&args[6], &args[7]);
    answer_then_swap(&args[6], &args[7]);

    println!("\nWINDOW RELOAD (headless): PASS");
}

/// The descriptors a session's saved bytes carry as in flight — the
/// host's ledger, less running subscriptions, as `hot::save` writes it
/// (M49/T49.10).
fn in_flight_of(s: &dyn Session) -> Vec<[u64; 3]> {
    let v: serde_json::Value = serde_json::from_slice(&s.save()).expect("a session saves JSON");
    v[fulgurite_reload::hot::RESERVED]["in_flight"]
        .as_array()
        .expect("the saved bytes carry the work in flight")
        .iter()
        .map(|e| {
            let a = e.as_array().expect("[kind, step, payload]");
            [a[0].as_u64().unwrap(), a[1].as_u64().unwrap(), a[2].as_u64().unwrap()]
        })
        .collect()
}

/// Frame until the chain's label shows a result, or `ms` pass.
fn drain_chain(s: &mut dyn Session, ms: u64) -> String {
    let until = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    loop {
        s.frame(16.0, W, H);
        let t = text_of(s);
        if !t.contains("chain -") || std::time::Instant::now() >= until {
            return t;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// Call a fixture-only `extern "C" fn()` in the library at `p`.
fn call(p: &str, name: &[u8]) {
    // SAFETY: the fixture's own `extern "C" fn()`; the library is already
    // loaded by `Build::open`, so this only finds it again, and it is
    // never unloaded, as every build here.
    let lib = unsafe { libloading::Library::new(p) }.expect("load");
    let f: libloading::Symbol<extern "C" fn()> = unsafe { lib.get(name) }.expect("the fixture exports it");
    f();
    std::mem::forget(lib);
}

/// **M49/T49.10 (R4-F3): a chain in flight at a swap completes under the
/// new build's step function.**
///
/// `chain-app` v1's step 0 blocks until released, so a click leaves it in
/// flight — listed in the ledger, the label at `chain -`. The swap must
/// carry the descriptor as data and the new build must re-issue it through
/// its own `job_for`: v2's step 0 is `p + 100`, so the chain ends at
/// `chain 1040`. The old build's code run after the swap ends at
/// `chain 50`; a dropped chain never ends.
fn chain_across_a_swap(v1_path: &str, v2_path: &str) {
    const V2: &str = "chain 1040";

    // The accepting direction first: v2 with no swap completes, and its
    // ledger is empty after.
    let v2 = open(v2_path);
    let mut d: Box<dyn Session> = Box::new(v2.boot(0, None).expect("chain v2 boots"));
    d.frame(0.0, W, H);
    d.post(Event::down(CHAIN_AT.0, CHAIN_AT.1));
    d.post(Event::up(CHAIN_AT.0, CHAIN_AT.1));
    let direct = drain_chain(&mut *d, 5000);
    let direct_left = in_flight_of(&*d);
    println!("chain, v2 with no swap: `{direct}`, in flight after {}", direct_left.len());
    assert!(direct.contains(V2), "setup: v2's chain with no swap did not reach `{V2}`: `{direct}`");
    assert!(direct_left.is_empty(), "a completed chain left work listed: {direct_left:?}");

    // v1: click, and step 0 is in flight (blocked).
    let v1 = open(v1_path);
    let mut r = Reloader::new();
    let mut cur: Box<dyn Session> = Box::new(v1.boot(0, None).expect("chain v1 boots"));
    cur.frame(0.0, W, H);
    cur.post(Event::down(CHAIN_AT.0, CHAIN_AT.1));
    cur.post(Event::up(CHAIN_AT.0, CHAIN_AT.1));
    cur.frame(16.0, W, H);
    std::thread::sleep(std::time::Duration::from_millis(20));
    cur.frame(16.0, W, H);
    let before = in_flight_of(&*cur);
    let shown = text_of(&*cur);
    println!("chain, v1 clicked: `{shown}`, in flight before the swap {} {before:?}", before.len());
    // The setup must happen: a click that missed spawns nothing, and the
    // swap would carry an empty ledger whichever way the code went.
    assert_eq!(before, vec![[9, 0, 4]], "setup: v1's step 0 is not in flight at the swap");
    assert!(shown.contains("chain -"), "setup: v1's chain finished before the swap: `{shown}`");

    r.outcome(Outcome::Built(std::path::PathBuf::from(v2_path)));
    assert!(r.swap(&mut cur, |s: &mut dyn Session| { s.frame(0.0, W, H); }), "chain v2 must swap");
    println!("chain, swapped: reissued {:?}", r.reissued);
    // The old build's worker finishes now, into an inbox nobody drains.
    call(v1_path, b"chain_app_release");

    let after = drain_chain(&mut *cur, 5000);
    let left = in_flight_of(&*cur);
    println!("chain, after the swap and a drain: `{after}` (want `{V2}`), in flight after {}", left.len());
    assert!(!after.contains("chain -"), "chain did not complete after swap: `{after}`, reissued {:?}", r.reissued);
    assert!(
        after.contains(V2),
        "the chain completed into the OLD build's step 0 (`chain 50` is v1's p + 1, times 10): `{after}`"
    );
    assert_eq!(r.reissued.spawned, 1, "the swap re-issued {:?}, want one job", r.reissued);
    assert!(left.is_empty(), "the completed chain left work listed after the swap: {left:?}");
    assert_eq!(after, direct, "the swapped chain's frame differs from v2 run with no swap");
    println!("  ok: a chain in flight at the swap completed under v2's step function");
}

/// Frame until `pred` holds for the window's text, or `ms` pass.
fn frame_until(s: &mut dyn Session, ms: u64, pred: impl Fn(&str) -> bool) -> String {
    let until = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    loop {
        s.frame(16.0, W, H);
        let t = text_of(s);
        if pred(&t) || std::time::Instant::now() >= until {
            return t;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// **M88/T88.1: a sipper in flight at a swap finishes under the new
/// build.**
///
/// `chain-app`'s `#sip` streams `0..10000` through `Sender::progress`;
/// v1 reports `p` and blocks after reporting 5, v2 reports `p + 100`. A
/// click on v1 leaves the sipper listed with `sip 5` shown. The swap
/// carries its descriptor, v2 re-issues it through its own `job_for`
/// (M49/T49.10), and the label must end at v2's `sip 10099` — not v1's
/// `sip 9999`, which only a slot read across the swap could show, and
/// not `sip -`, which is what a dropped re-issue leaves (`Work::sip` is
/// `#[reload(fresh)]`, so v2 starts with no value, and v1's worker
/// streams on into a slot nobody reads).
fn sipper_across_a_swap(v1_path: &str, v2_path: &str) {
    const V2: &str = "sip 10099";
    const V1: &str = "sip 9999";

    // Accept: v2 with no swap reaches its final value.
    let v2 = open(v2_path);
    let mut d: Box<dyn Session> = Box::new(v2.boot(0, None).expect("chain v2 boots"));
    d.frame(0.0, W, H);
    d.post(Event::down(SIP_AT.0, SIP_AT.1));
    d.post(Event::up(SIP_AT.0, SIP_AT.1));
    let direct = frame_until(&mut *d, 5000, |t| t.contains(V2));
    let direct_left = in_flight_of(&*d);
    println!("sipper, v2 with no swap: `{direct}` (want `{V2}`), in flight after {}", direct_left.len());
    assert!(direct.contains(V2), "accept: v2's sipper with no swap did not reach `{V2}`: `{direct}`");
    assert!(direct_left.is_empty(), "a finished sipper left work listed: {direct_left:?}");

    // v1: click, and the sipper blocks after reporting 5.
    let v1 = open(v1_path);
    let mut r = Reloader::new();
    let mut cur: Box<dyn Session> = Box::new(v1.boot(0, None).expect("chain v1 boots"));
    cur.frame(0.0, W, H);
    cur.post(Event::down(SIP_AT.0, SIP_AT.1));
    cur.post(Event::up(SIP_AT.0, SIP_AT.1));
    let shown = frame_until(&mut *cur, 5000, |t| t.contains("sip 5"));
    let before = in_flight_of(&*cur);
    println!("sipper, v1 clicked: `{shown}`, in flight before the swap {before:?}");
    assert_eq!(before, vec![[10, 0, 10_000]], "setup: v1's sipper is not listed at the swap");
    assert!(shown.contains("sip 5"), "setup: v1's sipper does not show `sip 5` in flight: `{shown}`");

    r.outcome(Outcome::Built(std::path::PathBuf::from(v2_path)));
    assert!(r.swap(&mut cur, |s: &mut dyn Session| { s.frame(0.0, W, H); }), "chain v2 must swap");
    println!("sipper, swapped: reissued {:?}", r.reissued);
    // v1's worker reports the rest of its stream now, into a slot nobody
    // reads.
    call(v1_path, b"chain_app_release_sip");

    let after = frame_until(&mut *cur, 5000, |t| t.contains(V2) || t.contains(V1));
    // A few more frames, so a late v1 value would have time to show.
    std::thread::sleep(std::time::Duration::from_millis(50));
    let after = if after.contains(V2) { frame_until(&mut *cur, 0, |_| true) } else { after };
    let left = in_flight_of(&*cur);
    println!("sipper, after the swap and the release: `{after}` (want `{V2}`, never `{V1}`), in flight after {}", left.len());
    assert!(!after.contains(V1), "the label shows v1's final value `{V1}` after the swap: `{after}`");
    assert!(
        after.contains(V2),
        "the sipper did not finish under v2 after the swap -- `sip -` is a dropped re-issue (the state is fresh, \
         and v1's worker streams on into a slot nobody reads): `{after}`, reissued {:?}",
        r.reissued
    );
    assert!(left.is_empty(), "the finished sipper left work listed after the swap: {left:?}");
    println!("  ok: a sipper in flight at the swap finished under v2");
}

/// `chain-app`'s `#asked` label: the last text run, which is its last
/// widget.
fn asked_of(s: &dyn Session) -> String {
    text_of(s).rsplit(" | ").next().unwrap_or("").to_string()
}

/// Click `#ask` and take its request — the window's half, which this
/// loader stands in for. The setup: exactly one `ClipboardRead { tag: 7 }`.
fn ask(s: &mut dyn Session, who: &str) {
    s.frame(0.0, W, H);
    s.post(Event::down(ASK_AT.0, ASK_AT.1));
    s.post(Event::up(ASK_AT.0, ASK_AT.1));
    s.frame(16.0, W, H);
    let taken = s.take_requests();
    println!("request, {who}: #ask clicked, took {} request(s) (want 1) {:?}", taken.len(), taken.iter().map(|o| o.request).collect::<Vec<_>>());
    let want = Request::ClipboardRead { tag: 7, kind: ClipKind::Clipboard, format: ClipFormat::Text };
    assert!(
        taken.len() == 1 && taken[0].request == want,
        "setup: the click on #ask did not ask for the clipboard ({} taken)",
        taken.len()
    );
}

/// **M88/T88.3: an answer is read by whatever build is loaded** — the
/// property M89's pending requests rely on.
///
/// v1 is clicked and its `ClipboardRead { tag: 7 }` taken; the session is
/// swapped to v2, and the answer goes to the **current** session: the
/// label must read `v2:x`, because `App::answered` runs in the build
/// loaded when the answer arrives and the window holds only the tag.
fn request_across_a_swap(v1_path: &str, v2_path: &str) {
    // Accept: with no swap, v1 converts it.
    let v1 = open(v1_path);
    let mut d: Box<dyn Session> = Box::new(v1.boot(0, None).expect("chain v1 boots"));
    ask(&mut *d, "v1, no swap");
    d.answer(7, Answer::Text("x"));
    d.frame(16.0, W, H);
    let direct = asked_of(&*d);
    println!("request, v1 with no swap: `{direct}` (want `v1:x`)");
    assert_eq!(direct, "v1:x", "accept: v1 answered with no swap");

    let mut r = Reloader::new();
    let mut cur: Box<dyn Session> = Box::new(v1.boot(0, None).expect("chain v1 boots"));
    ask(&mut *cur, "v1");
    r.outcome(Outcome::Built(std::path::PathBuf::from(v2_path)));
    assert!(r.swap(&mut cur, |s: &mut dyn Session| { s.frame(0.0, W, H); }), "chain v2 must swap");
    let before = asked_of(&*cur);
    // The window holds only the tag; the answer goes to the session
    // loaded now.
    cur.answer(7, Answer::Text("x"));
    cur.frame(16.0, W, H);
    let after = asked_of(&*cur);
    println!("request, swapped to v2 then answered: `{before}` -> `{after}` (want `v2:x`)");
    assert_eq!(
        after, "v2:x",
        "the answer was not converted by the build loaded when it arrived -- `v2:` with no text is an answer \
         given to another session (answered to v1)"
    );
    println!("  ok: a request taken from v1 and answered after the swap is read by v2");
}

/// **M88/T88.3 (second review A1): an answer applied before a swap is
/// kept state, not a message lost with the old build.**
///
/// v1 is clicked, its request taken and answered on v1's session, and the
/// swap to v2 happens **before any pump**. `Host::answer` applied the
/// message inside the call, so v1's `answered` converted it and the kept
/// field already holds `v1:x`; after v2's first frame the label reads
/// `v1:x`. A host that parked the message for its next pump would lose
/// it at the swap, which `hot::save` cannot carry, and read `v2:`.
fn answer_then_swap(v1_path: &str, v2_path: &str) {
    let v1 = open(v1_path);
    // Accept: with no swap, one pump shows it.
    let mut d: Box<dyn Session> = Box::new(v1.boot(0, None).expect("chain v1 boots"));
    ask(&mut *d, "v1, no swap");
    d.answer(7, Answer::Text("x"));
    d.frame(16.0, W, H);
    let direct = asked_of(&*d);
    println!("answer-then-swap, v1 with no swap, one pump: `{direct}` (want `v1:x`)");
    assert_eq!(direct, "v1:x", "accept: v1's answer after one pump");

    let mut r = Reloader::new();
    let mut cur: Box<dyn Session> = Box::new(v1.boot(0, None).expect("chain v1 boots"));
    ask(&mut *cur, "v1");
    cur.answer(7, Answer::Text("x"));
    // Setup landed: v1's kept field holds the answer before the swap, read
    // from what the swap will carry.
    let saved: serde_json::Value = serde_json::from_slice(&cur.save()).expect("a session saves JSON");
    let kept = saved.to_string();
    println!("answer-then-swap: v1's saved state right after answer, before any pump: {kept}");
    r.outcome(Outcome::Built(std::path::PathBuf::from(v2_path)));
    assert!(r.swap(&mut cur, |s: &mut dyn Session| { s.frame(0.0, W, H); }), "chain v2 must swap");
    cur.frame(16.0, W, H);
    let after = asked_of(&*cur);
    println!("answer-then-swap: after v2's first frame `{after}` (want `v1:x`)");
    // The claim before the setup check, so a parked answer fails by the
    // claim's name (the setup check would see the same cause first).
    assert_eq!(after, "v1:x", "answer lost at the swap: v2 shows `{after}`, not the answer v1 applied");
    assert!(kept.contains("v1:x"), "setup: v1's kept `asked` does not hold `v1:x` right after answer: {kept}");
    println!("  ok: an answer applied before the swap survived it as kept state");
}
