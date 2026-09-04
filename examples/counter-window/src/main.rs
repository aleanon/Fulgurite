//! The counter, in a window.
//!
//! Since M14/T14.0 this file is only the entry point. The window is
//! `lumen-static-winit`; what is left in this crate is what makes the
//! tally *this* application rather than the minimal one:
//!
//! | module | what it is |
//! |---|---|
//! | `app` | the tree, the stylesheet, the palette |
//! | `window` | the agent endpoint, the a11y adapter and the sign cascade, on `lumen_static_winit::Surface` |
//! | `agent_socket` | the JSON-RPC listener and its bridge to `AgentHost` |
//! | `headless` | `--headless`, `--profile`, `--drive` — rungs 1–3 of the ladder |
//! | `form` | `--form`, the M8/M9 widgets through the real rasteriser |
//!
//! It was 1160 lines, ~480 of them a window every other application
//! would have had to copy. `examples/minimal` is the same window in 24.

mod agent_socket;
mod app;
mod form;
mod headless;
mod window;
fn main() {
    let mut args = std::env::args().skip(1);
    if let Some(flag) = args.next() {
        if flag == "--drive" {
            headless::drive();
            return;
        }
        if flag == "--form" {
            let path = args.next().unwrap_or_else(|| "form.ppm".into());
            form::form(&path);
            return;
        }
        if flag == "--headless" {
            let path = args.next().unwrap_or_else(|| "tally.ppm".into());
            let count = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            headless::headless(&path, count);
            return;
        }
    }
    window::run();
}
