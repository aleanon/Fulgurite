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
//! | `headless` | `--drive` and `--drive2`, rung 2 of the ladder, through a `Local` session |
//!
//! ```text
//! counter-window [count]            the window, opening at `count`
//! FULGURITE_SHOT=f.ppm counter-window 12   one headless frame at 12 (the window crate's)
//!   … FULGURITE_PROFILE=1 FULGURITE_PROFILE_CLICK=inc   and its per-stage cost
//! counter-window --drive | --drive2    the agent protocol, then the harness
//! ```
//!
//! It was 1160 lines before M14 and 1,383 before M60, ~540 of them a
//! copy of the window crate's loop kept for two things the framework
//! could not do: app-level keys (`Program::on_key`) and a sheet keyed on
//! the count's sign (`Program::style_by`). The M8/M9 form fixture that
//! shared this crate is `examples/form`.

mod app;
mod headless;

use fulgurite_winit::Run;

fn main() {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("--drive") => headless::drive(),
        // M12/T12.1: the same session through the harness. Both run in
        // `just tally-drive` and `just fixtures`, because the comparison
        // is the task's evidence.
        Some("--drive2") => headless::drive_harness(),
        // The count the tally opens at, read here and captured by the
        // boot — which is what `FULGURITE_SHOT` renders, too.
        _ => {
            let count = arg.and_then(|a| a.parse().ok()).unwrap_or(0);
            // M89/T89.2: the strategy is this application's choice, made
            // here, so `just windows-drive` can drive both.
            #[cfg(feature = "everything")]
            app::program(count, 440, 520).invalidation::<fulgurite::Everything>().run();
            #[cfg(not(feature = "everything"))]
            app::program(count, 440, 520).run();
        }
    }
}
