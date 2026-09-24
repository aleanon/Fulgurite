//! `hot-window` — with `--features hot`, the window rebuilds this
//! crate's library on every save and swaps to it (M37). `LUMEN_AGENT`
//! serves the agent when built with `agent`.
// ANCHOR: main
use lumen_static_winit::Run;

fn main() {
    let mut p = hot_window::program();
    if let Ok(addr) = std::env::var("LUMEN_AGENT") {
        p = p.agent(addr);
    }
    p.run();
}
// ANCHOR_END: main
