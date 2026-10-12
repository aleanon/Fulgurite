//! A library that exports its ABI stamp and nothing else — M37/T37.0.
//!
//! It names one widget type so that `fulgurite-widgets` is in its
//! build graph: M49/T49.1's accepting case edits that crate alone and
//! requires the library still to be accepted, which proves nothing if
//! the crate was never compiled into it.
#[cfg(not(feature = "shell"))]
fulgurite_reload::export_abi_stamp!();
// M81/T81.1: the shell's fields, as `export_program!` stamps a window
// library, so a shell feature (`shaping`) reaches the stamp.
#[cfg(feature = "shell")]
fulgurite_reload::export_abi_stamp!(&fulgurite_shell::session::abi_fields());

/// Keeps `fulgurite-widgets` in the graph (see above).
pub fn widget_size() -> usize {
    core::mem::size_of::<fulgurite_widgets::ops::Spacer>()
}
