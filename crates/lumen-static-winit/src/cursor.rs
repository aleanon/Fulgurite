//! The pointer shape — `docs/plan-desktop.md` T14.6.
//!
//! `Style::cursor` says what a node wants; this is the half that tells
//! the platform. It is a `match` and a guard, and the guard is the
//! interesting part: setting a window's cursor is a round trip to the
//! display server, and a frame loop that set it unconditionally would
//! make one per frame for a cursor that has not changed.

use lumen_static::style::Cursor;
use winit::window::{CursorIcon, Window};

/// Show `want` if it differs from `shown`, and remember it.
pub fn apply(window: &Window, want: Cursor, shown: &mut Cursor) {
    if want == *shown {
        return;
    }
    *shown = want;
    window.set_cursor(icon(want));
}

/// This framework's cursors to winit's.
///
/// Total, deliberately: every variant maps to a real platform cursor.
/// A shape that cannot be mapped would be a silent no-op, which is why
/// `Cursor` is a closed enum rather than a string.
pub fn icon(c: Cursor) -> CursorIcon {
    match c {
        Cursor::Default => CursorIcon::Default,
        Cursor::Text => CursorIcon::Text,
        Cursor::Pointer => CursorIcon::Pointer,
        Cursor::Grab => CursorIcon::Grab,
        Cursor::Grabbing => CursorIcon::Grabbing,
        Cursor::ColResize => CursorIcon::ColResize,
        Cursor::RowResize => CursorIcon::RowResize,
        Cursor::NotAllowed => CursorIcon::NotAllowed,
    }
}
