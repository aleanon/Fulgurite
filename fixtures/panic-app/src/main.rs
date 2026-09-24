//! `panic-app <site>`: arm one site and drive the frames that reach it,
//! headless. Exits 0 if nothing panicked. M36/T36.0.
use lumen_static::spawn::Immediate;
use lumen_static_shell::Host;
use panic_app::build;

fn main() {
    let site: u8 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut h: Host<panic_app::Fx> = Host::new(build(), 300, 200);
    h.set_spawner(Box::new(Immediate));
    h.pump();
    h.pump();
    h.app.armed = site;
    let i = lumen_static::style::index_of_wid(<panic_app::Ui as lumen_static::Widget>::SUB_TABLE, "go").unwrap();
    let s = h.states[i];
    h.click(s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    for _ in 0..3 {
        h.tick(16.0);
        h.pump();
    }
    println!("no panic");
}
