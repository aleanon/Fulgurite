//! `panic-app <site>`: arm one site and drive the frames that reach it,
//! headless. Exits 0 if nothing panicked. M36/T36.0.
use lumen_static::spawn::Immediate;
use lumen_static_shell::Host;
use panic_app::build;

fn main() {
    // `--window`: the guarded window, driven over the agent (M36/T36.3).
    #[cfg(feature = "window")]
    if std::env::args().nth(1).as_deref() == Some("--window") {
        use lumen_static_winit::{application, Run};
        // The endpoint is `LUMEN_AGENT_ADDR`, read by the window (M48/T48.11).
        let program = application(panic_app::build_window).title("panic-app").size(480, 220);
        // M55/T55.4: the strategy is this application's choice.
        #[cfg(feature = "everything")]
        let program = program.invalidation::<lumen_static::Everything>();
        return program.run();
    }
    let site: u8 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut h: Host<panic_app::Fx> = Host::new(build(), 300, 200);
    h.set_spawner(Box::new(Immediate));
    h.pump();
    h.pump();
    h.app.armed = site;
    let i = lumen_static::style::index_of_wid(<panic_app::Ui as lumen_static::Widget>::SUB_TABLE, panic_app::target(site))
        .unwrap();
    let s = h.states[i];
    h.click(s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    for _ in 0..3 {
        h.tick(16.0);
        h.pump();
        // What a window asks after each frame (M80/T80.5): site 8; its
        // next deadline (M84/T84.3): site 9; and the store the renderer
        // reads (M84/T84.6): site 10.
        let _ = h.caret_rect();
        let _ = h.next_deadline_ms();
        let _ = lumen_static::event::App::images(&h.app).is_some();
        // And the requests a window answers after a frame (M88/T88.3):
        // site 11.
        for o in h.take_requests() {
            h.answer(o.request.tag().expect("every request here carries a tag"), lumen_static::request::Answer::Refused(lumen_static::request::Refusal::Unsupported));
        }
    }
    println!("no panic");
}
