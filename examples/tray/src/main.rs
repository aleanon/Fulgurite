//! `tray` — a tray application (M90/T90.7). See the library's
//! documentation for what it does.
use lumen_static_winit::Run;

fn main() {
    #[cfg(feature = "everything")]
    tray::program().invalidation::<lumen_static::Everything>().run();
    #[cfg(not(feature = "everything"))]
    tray::program().run();
}
