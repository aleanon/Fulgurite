//! A library that exports its ABI stamp and nothing else — M37/T37.0.
//!
//! It names one widget type so that `lumen-static-widgets` is in its
//! build graph: M49/T49.1's accepting case edits that crate alone and
//! requires the library still to be accepted, which proves nothing if
//! the crate was never compiled into it.
lumen_static_reload::export_abi_stamp!();

/// Keeps `lumen-static-widgets` in the graph (see above).
pub fn widget_size() -> usize {
    core::mem::size_of::<lumen_static_widgets::ops::Spacer>()
}
