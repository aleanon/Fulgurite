//! `daemon` — a program with no window of its own (M89/T89.6). See the
//! library's documentation for what it does.
use lumen_static_winit::Run;

fn main() {
    #[cfg(feature = "everything")]
    daemon::program().invalidation::<lumen_static::Everything>().run();
    #[cfg(not(feature = "everything"))]
    daemon::program().run();
}
