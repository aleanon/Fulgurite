//! Derived presentation: tie a widget's displayed value to the state it
//! comes from, so the formatting step is *declared at the field* instead
//! of written out in an update method that can drift or be forgotten.
//!
//! The runtime here is deliberately small — two functions. The wiring is
//! `#[derive(App)]`, which turns `#[bind(...)]` attributes into the body
//! of [`App::update`](crate::event::App::update); the frame loop calls
//! that between input and layout. Nothing is registered, observed or
//! scheduled: a binding is generated straight-line code, which is why it
//! costs no runtime graph, survives a hot reload without migration (it
//! is code, re-supplied by the new dylib), and cannot capture anything —
//! the same properties that made handlers `fn` pointers rather than
//! closures (`docs/events-decision.md`).
//!
//! ## Change detection compares the *output*, not the source
//!
//! A binding must not re-measure text that did not change, since shaping
//! is the expensive part of a frame. The two obvious ways to know both
//! have holes:
//!
//! - a dirty bit on the source can be routed around — mutate the field
//!   directly and the screen lies, with nothing to detect it;
//! - a shadow copy of the source costs `size_of::<T>()` per binding and
//!   needs a field to live in, which a derive cannot add.
//!
//! So the comparison is against the formatted result. It is impossible
//! to defeat, because there is no setter to bypass and no bit to forget;
//! it stores nothing; and it keys on exactly the thing the dirty bit
//! protects, since equal text measures equally. The cost is running the
//! formatter every frame, which for the values that go in labels is
//! nanoseconds into a reused buffer.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1).
use alloc::string::String;

use core::fmt::Write;
// The scratch `thread_local!` is the only user, and it is std-only.
#[cfg(feature = "std")]
use core::cell::RefCell;

use crate::ops::Label;
use crate::text::TextBuffer;

#[cfg(feature = "std")]
std::thread_local! {
    /// One reusable formatting buffer per thread. It never escapes a
    /// call; it exists so that a binding running every frame allocates
    /// on no frame but the first.
    static SCRATCH: RefCell<String> = const { RefCell::new(String::new()) };
}

/// The `no_std` scratch buffer: a fresh one per call.
///
/// `thread_local!` is std-only, and the obvious `static` replacement is
/// not `Sync`, so reusing one buffer without `std` needs either
/// `critical-section` or `unsafe` — a dependency and an M0 violation
/// respectively, for a path that is already allocating.
///
/// **So `no_std` pays one allocation per bound label per frame**, which
/// `std` does not (M13/T13.1). That is a real cost and it is named here
/// rather than hidden: the reason it is acceptable is that this function
/// only runs for a `Label<S>` whose `S` allocates in the first place, so
/// the alloc-free path T13.3 gates never reaches it. An embedded app
/// that wants a bound label wants the fixed-capacity text storage T13.5
/// adds, and that is where a fixed-capacity scratch belongs — not here.
#[cfg(not(feature = "std"))]
fn scratch() -> String {
    String::new()
}

/// Format `args` into `dst`, but only actually store the result — and
/// only mark the label for re-measurement — if it differs from the text
/// the label already holds. Returns whether the label changed.
///
/// This is what `#[bind(target, "…{}…")]` expands to.
///
/// ```
/// use lumen_static::bind::format_into;
/// use lumen_static::ops::Label;
///
/// let mut l: Label<String> = Label::new("count: 1".into());
/// assert!(!format_into(&mut l, format_args!("count: {}", 1)), "same text, no write");
/// assert!(format_into(&mut l, format_args!("count: {}", 2)), "changed");
/// assert_eq!(l.as_str(), "count: 2");
/// ```
pub fn format_into<S: TextBuffer>(dst: &mut Label<S>, args: core::fmt::Arguments<'_>) -> bool {
    // Taken rather than borrowed: a `Display` impl that itself runs a
    // binding would double-borrow a `RefCell` and panic. Taking hands
    // the inner call an empty buffer instead, which is merely slower.
    #[cfg(feature = "std")]
    let mut buf: String = SCRATCH.with(|c| c.take());
    #[cfg(not(feature = "std"))]
    let mut buf: String = scratch();

    buf.clear();
    let _ = buf.write_fmt(args); // writing to a String cannot fail
    let changed = dst.set_str(&buf);

    #[cfg(feature = "std")]
    SCRATCH.with(|c| c.replace(buf));
    changed
}

/// Store `text` in `dst` only if it differs. The non-formatting form,
/// for a binding whose source is already a string.
pub fn set_if_changed<S: TextBuffer>(dst: &mut Label<S>, text: &str) -> bool {
    // `Label::set_str` compares as of F4.6; this is kept as the named
    // spelling for a binding whose source is already a string.
    dst.set_str(text)
}
