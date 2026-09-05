//! The system clipboard — `docs/plan-desktop.md` T14.5.
//!
//! The framework's half of copy and paste is `Host::copy` (what is
//! selected) and `Host::paste` (type this in). This is the other half:
//! the platform. It lives here rather than in the shell because a
//! clipboard *is* a platform, and `lumen-static-shell` has none — which
//! is the property that keeps `cargo test` free of a windowing system.
//!
//! Behind the default-on `clipboard` feature. Without it the type still
//! exists and every operation is a no-op, so a host does not need a
//! `cfg` around its Ctrl+C.

/// A handle on the system clipboard.
///
/// Held open rather than opened per operation: on X11 a clipboard owner
/// must stay alive to answer paste requests from other applications, so
/// a handle dropped after each copy would put text on the clipboard that
/// vanishes the moment the user switches windows.
pub struct Clipboard {
    #[cfg(feature = "clipboard")]
    inner: Option<arboard::Clipboard>,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard {
    /// Open it, or degrade to a no-op.
    ///
    /// There are real machines with no clipboard — a bare TTY, a
    /// container with no display — and an application should not fail to
    /// start on one. `None` inside means copy and paste do nothing,
    /// which is what "there is no clipboard" should look like.
    pub fn new() -> Self {
        Clipboard {
            #[cfg(feature = "clipboard")]
            inner: arboard::Clipboard::new().ok(),
        }
    }

    /// Put `text` on the clipboard. Returns whether it got there.
    #[cfg(feature = "clipboard")]
    pub fn set(&mut self, text: &str) -> bool {
        self.inner.as_mut().is_some_and(|c| c.set_text(text).is_ok())
    }

    /// Without the feature there is nowhere to put it.
    #[cfg(not(feature = "clipboard"))]
    pub fn set(&mut self, text: &str) -> bool {
        let _ = text;
        false
    }

    /// Read the clipboard's text, if it has any.
    #[cfg(feature = "clipboard")]
    pub fn get(&mut self) -> Option<String> {
        self.inner.as_mut().and_then(|c| c.get_text().ok())
    }

    /// Without the feature there is nothing to read.
    #[cfg(not(feature = "clipboard"))]
    pub fn get(&mut self) -> Option<String> {
        None
    }
}
