//! **Capture and host recovery, headless** — M36/T36.1
//! (`docs/plan-panic-recovery.md`).
//!
//! For each of the eleven sites: the guarded entry catches the panic; the
//! report's location is the site's marker line (site 7: inside the
//! framework, with the *application* frame at the fixture's calling
//! line); the retried frame is full damage and byte-identical to a host
//! that was never armed; and the recovered host holds no pointer state.
//! Plus the two things a guard can get subtly wrong: events queued behind
//! the trigger, and a job's panic that M28's spawner already owns.

use lumen_static::retain::Damage;
use lumen_static::spawn::Immediate;
use lumen_static_shell::Host;
use panic_app::{build, site_line, target, Fx, Ui, SITES};

fn host() -> Host<Fx> {
    let mut h: Host<Fx> = Host::new(build(), 300, 200);
    h.set_source_root("fixtures/panic-app");
    h.set_spawner(Box::new(Immediate));
    h.pump();
    h.pump();
    h
}

fn centre(h: &Host<Fx>, wid: &str) -> (f32, f32) {
    let i = lumen_static::style::index_of_wid(<Ui as lumen_static::Widget>::SUB_TABLE, wid).unwrap();
    let s = h.states[i];
    (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
}

/// What every site's session does: click the site's target, then tick
/// and pump, guarded, and ask the focused caret as a window does after a
/// frame (`Session::caret_rect`, M80/T80.5), and the next deadline
/// (`Session::next_deadline_ms`, M84/T84.3), and the application's store
/// (`App::images`, which `Local`'s frame asks guarded, M84/T84.6), guarded too.
fn session(h: &mut Host<Fx>, site: u8) {
    let (x, y) = centre(h, target(site));
    h.click(x, y);
    for _ in 0..3 {
        if h.guard(|h| {
            h.tick(16.0);
            h.pump();
        })
        .is_none()
        {
            return;
        }
        if h.guard(|h| h.caret_rect()).is_none() {
            return;
        }
        // And its next deadline, guarded as `Local` asks it (M84/T84.3).
        if h.guard(|h| h.next_deadline_ms()).is_none() {
            return;
        }
        // And the store the renderer reads, guarded as `Local`'s frame
        // asks it (M84/T84.6).
        if h.guard(|h| lumen_static::event::App::images(&h.app).is_some()).is_none() {
            return;
        }
        // And the requests, answered guarded as `Local` answers them
        // (M88/T88.3): site 11.
        let answered = h.guard(|h| {
            for o in h.take_requests() {
                h.answer(o.request.tag(), lumen_static::request::Answer::Refused(lumen_static::request::Refusal::Unsupported));
            }
        });
        if answered.is_none() {
            return;
        }
    }
}

#[test]
fn every_site_is_caught_reported_and_recovered_from() {
    for (site, what) in SITES {
        let mut a = host();
        a.app.armed = site;
        session(&mut a, site);
        let r = a.failure().cloned().unwrap_or_else(|| panic!("site {site} ({what}) was not caught"));
        let (file, line, _) = r.location.clone().expect("a location");
        println!("site {site} ({what}): {file}:{line}  app frame {:?}  note {:?}", r.app_frame, r.note);
        if site == 7 {
            assert!(file.starts_with("crates/lumen-static/src/"), "site 7 is raised in the framework: {file}");
            assert_eq!(
                r.app_frame,
                Some(("fixtures/panic-app/src/lib.rs".to_string(), site_line(7))),
                "site 7: the application frame must be the fixture's calling line"
            );
        } else {
            assert_eq!(
                (file.as_str(), line),
                ("fixtures/panic-app/src/lib.rs", site_line(site)),
                "site {site}: the reported location is not the site's line"
            );
        }
        assert!(a.input.hovered.is_none() && a.input.pressed.is_none() && a.input.capture.is_none(),
            "site {site}: pointer state survived the panic");
        assert!(a.states.iter().all(|n| !n.hovered && !n.pressed), "site {site}: a node is still hovered or pressed");
        assert!(a.guard(|h| h.pump()).is_none(), "site {site}: a failed host ran a guarded entry before retry");

        // Retry with the trigger cleared; compare with a host never armed.
        a.app.armed = 0;
        // **Taken first**, so the damage below is the retry frame's own.
        // `take_damage_for_age` accumulates what nobody took, and a host
        // whose damage was never taken answers `All` whatever happened —
        // which is how the first version of this assertion passed with
        // the `retain` reset removed.
        let _ = a.take_damage_for_age(1);
        a.retry();
        a.guard(|h| h.pump()).expect("the retry panicked");
        let damage = a.take_damage_for_age(1);
        assert!(matches!(damage, Damage::All), "site {site}: the retried frame is not full damage: {damage:?}");
        let mut b = host();
        session(&mut b, site);
        // The same pointer move for both: the recovered host released
        // its hover (as it must), the never-armed one still has the
        // pointer over the button. After a panic the next move restores
        // hover; compared before one, the frames differ by a hover shade.
        for h in [&mut a, &mut b] {
            h.post(lumen_static::Event::PointerMove { x: 1.0, y: 199.0 });
        }
        b.guard(|h| h.pump()).unwrap();
        a.guard(|h| h.pump()).unwrap();
        assert_eq!(a.list.hash_from(0), b.list.hash_from(0), "site {site}: the retried frame differs from a never-armed host's");
    }
}

#[test]
fn events_queued_behind_the_trigger_are_dropped() {
    let mut h = host();
    h.app.armed = 1;
    let (x, y) = centre(&h, "go");
    h.click(x, y);
    h.click(x, y);
    assert!(h.guard(|h| h.pump()).is_none(), "setup: the first click did not panic");
    h.retry();
    // Still armed: had the second click survived, it would panic again.
    let again = h.guard(|h| h.pump());
    println!("clicks seen {}, failure after retry {:?}", h.app.clicks, h.failure().map(|r| &r.message));
    assert!(again.is_some(), "the click queued behind the trigger survived the recovery and fired");
}

#[test]
fn a_jobs_panic_is_the_spawners_and_is_not_reported() {
    let mut h = host();
    h.app.armed = panic_app::JOB_PANIC;
    session(&mut h, panic_app::JOB_PANIC);
    assert!(h.failure().is_none(), "a job's panic, caught by the spawner, was reported as the frame's");
    // And its hook record is not left for the next real panic to inherit.
    h.app.armed = 1;
    session(&mut h, 1);
    let r = h.failure().expect("site 1 was not caught");
    assert_eq!(r.location.as_ref().map(|l| l.1), Some(site_line(1)), "a stale record was reported: {r:?}");
}
