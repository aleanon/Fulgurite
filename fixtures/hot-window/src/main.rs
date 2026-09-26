//! `hot-window` — with `--features hot`, the window rebuilds this
//! crate's library on every save and swaps to it (M37). Built with
//! `agent`, it serves the endpoint at `LUMEN_AGENT_ADDR`, which the
//! window reads (M48/T48.11).
// ANCHOR: main
use lumen_static_winit::Run;

fn main() {
    hot_window::program().run();
}
// ANCHOR_END: main
