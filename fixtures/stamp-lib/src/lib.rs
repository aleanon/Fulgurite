//! A library that exports its ABI stamp and nothing else — M37/T37.0.
//!
//! It names one widget type so that `lumen-static-widgets` is in its
//! build graph: M49/T49.1's accepting case edits that crate alone and
//! requires the library still to be accepted, which proves nothing if
//! the crate was never compiled into it.
#[cfg(not(feature = "shell"))]
lumen_static_reload::export_abi_stamp!();
// M81/T81.1: the shell's fields, as `export_program!` stamps a window
// library, so a shell feature (`shaping`) reaches the stamp.
#[cfg(feature = "shell")]
lumen_static_reload::export_abi_stamp!(&lumen_static_shell::session::abi_fields());

/// Keeps `lumen-static-widgets` in the graph (see above).
pub fn widget_size() -> usize {
    core::mem::size_of::<lumen_static_widgets::ops::Spacer>()
}
