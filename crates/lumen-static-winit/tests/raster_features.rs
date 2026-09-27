//! **The rasteriser's `threads` feature is reachable from the window
//! crate, and off by default** — M50/T50.5, R6-F5.
//!
//! `lumen-static-winit` had no `simd` or `threads` feature, so an
//! application that depends only on it could not turn the fast raster
//! paths on; `cargo build -p minimal --features lumen-static-winit/threads`
//! failed to resolve the feature. This asks the shell what it compiled:
//! `Canvas::bands_for` answers 1 without `threads` and the worker count
//! with it. `just threads` runs it with the feature; `just test` runs the
//! default arm.

use lumen_static_shell::software::Canvas;

#[test]
fn banding_follows_the_window_crates_threads_feature() {
    let c = Canvas::new(3840, 2160);
    let bands = c.bands_for(None);
    let on = cfg!(feature = "threads");
    println!("lumen-static-winit threads = {on}: a 3840x2160 canvas bands into {bands}");
    if on {
        assert!(bands >= 2, "--features threads, yet a 4K full redraw bands into {bands}: banding is not compiled in");
    } else {
        assert_eq!(bands, 1, "the default build bands a 4K redraw into {bands}: `threads` is on by default");
    }
}
