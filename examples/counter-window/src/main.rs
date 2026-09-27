//! The counter, in a window.
//!
//! **The window is the framework's** since M60
//! (`docs/plan-tally-slim.md`): `main` describes the program and runs it,
//! and `Program::run` owns the event loop, the agent endpoint on
//! 127.0.0.1:9230, the a11y adapter, IME, the frame timings and
//! `app.quit`. What is left in this crate is what makes the tally *this*
//! application:
//!
//! | module | what it is |
//! |---|---|
//! | `app` | the tree, the stylesheet, the palette, its keys, the program |
//! | `headless` | `--drive` and `--drive2`, rungs 2 of the ladder, through a `Local` session |
//!
//! It was 1160 lines before M14 and 1,383 before M60, ~540 of them a
//! copy of the window crate's loop kept for two things the framework
//! could not do: app-level keys (`Program::on_key`) and a sheet keyed on
//! the count's sign (`Program::style_by`).

mod app;
mod form;
mod headless;

use lumen_static_winit::Run;

fn main() {
    let mut args = std::env::args().skip(1);
    if let Some(flag) = args.next() {
        if flag == "--drive" {
            headless::drive();
            return;
        }
        // M12/T12.1: the same session through the harness. Both run in
        // `just tally-drive`, because the comparison is the task's evidence.
        if flag == "--drive2" {
            headless::drive_harness();
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
    app::program(0, 440, 520).run();
}
