//! `hot-window` — with `--features hot`, the window rebuilds this
//! crate's library on every save and swaps to it (M37). Built with
//! `agent`, it serves the endpoint at `FULGURITE_AGENT_ADDR`, which the
//! window reads (M48/T48.11).
//!
//! `hot-window --hash basic|complex X0 Y0 X1 Y1` (with `agent`) renders
//! the program headlessly instead — `Local` and an `Offscreen` in the
//! mode named, as M81/T81.4's drive compares a live window against — and
//! prints the agent's `ui.resolve` of `#hebrew` and `ui.regionHash` of
//! the rect, one JSON reply per line.
// ANCHOR: main
use fulgurite_winit::Run;

fn main() {
    #[cfg(feature = "agent")]
    if std::env::args().nth(1).as_deref() == Some("--hash") {
        return hash();
    }
    hot_window::program().run();
}
// ANCHOR_END: main

/// The headless half of the drive's comparison (M81/T81.4).
#[cfg(feature = "agent")]
fn hash() {
    use fulgurite_shell::session::{Crashing, Local, Offscreen, Screen, Session};
    use fulgurite_shell::text::Shaping;
    let args: Vec<String> = std::env::args().skip(2).collect();
    let shaping = match args.first().map(String::as_str) {
        Some("basic") => Shaping::Basic,
        #[cfg(feature = "shaping")]
        Some("complex") => Shaping::Complex,
        other => panic!("--hash: no mode {other:?} in this build (basic, or complex with `shaping`)"),
    };
    let rect: Vec<f32> = args[1..5].iter().map(|v| v.parse().expect("--hash: a number")).collect();
    let p = hot_window::program().shaping(shaping);
    let (w, h) = (p.width, p.height);
    let mut s = Local::from_program(p, Crashing);
    let mut screen = Offscreen::with_shaping(w, h, s.shaping());
    s.frame(0.0, w, h);
    let page = s.page();
    screen.present(&mut s, page);
    let resolve = r##"{"jsonrpc":"2.0","id":1,"method":"ui.resolve","params":{"selector":"#hebrew"}}"##;
    println!("{}", s.agent(resolve, &mut screen));
    let region = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"ui.regionHash","params":{{"rect":[{},{},{},{}]}}}}"#,
        rect[0], rect[1], rect[2], rect[3]
    );
    println!("{}", s.agent(&region, &mut screen));
    println!("{{\"shaping\":\"{:?}\"}}", s.shaping());
}
