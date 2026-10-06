//! The system clipboard — `docs/plan-desktop.md` T14.5, and since
//! M88/T88.4 (`docs/plan-platform-requests.md`) what an application's
//! clipboard requests reach.
//!
//! The framework's half of copy and paste is `Host::copy` (what is
//! selected) and `Host::paste` (type this in). This is the other half:
//! the platform. It lives here rather than in the shell because a
//! clipboard *is* a platform, and `lumen-static-shell` has none — which
//! is the property that keeps `cargo test` free of a windowing system.
//!
//! **Two callers.** The window's own Ctrl+C/Ctrl+V and `app.copy` use
//! [`Clipboard::set`]/[`Clipboard::get`]. An application asks through
//! `lumen_static::request` — a `ClipboardWrite` or `ClipboardRead` its
//! handler pushes — and `Win::serve_requests` answers it from here:
//! text and HTML on the clipboard ([`Clipboard::set_html`],
//! [`Clipboard::get_html`]), and text on the **primary selection**
//! ([`Clipboard::set_primary`], [`Clipboard::get_primary`]), the X11
//! select-to-copy buffer, which arboard reaches through its
//! `GetExtLinux`/`SetExtLinux` extension traits with
//! `LinuxClipboardKind::Primary`. None of the four needs an arboard
//! feature, so they ride `clipboard` with no new crate
//! (`docs/results-tasks.md` T88.4).
//!
//! **Where the primary selection exists** is [`Clipboard::HAS_PRIMARY`]:
//! the unixes arboard drives through X11 (not macOS, not Android).
//! Elsewhere `set_primary` is `false` and `get_primary` `None`, and a
//! primary request is answered `Refused(Unsupported)`.
//!
//! **Native Wayland has no clipboard in this build.** The workspace
//! builds arboard with `default-features = false` and without
//! `wayland-data-control`, so arboard talks to X11 only: under XWayland
//! everything here works; on a Wayland-only session [`Clipboard::new`]
//! finds no clipboard and every operation is a no-op. The manifest's
//! `clipboard` comment records why it is not added.
//!
//! **Images** ([`Clipboard::set_image`], [`Clipboard::get_image`]) are
//! behind the default-off `clipboard-image` (M88/T88.5), which turns on
//! arboard's `image-data` and with it the `image` crate; without it they
//! are `false`/`None` and [`Clipboard::HAS_IMAGE`] is `false`, so an image
//! request is answered `Refused(Unsupported)`. On X11 an image travels as
//! PNG, which is lossless, so RGBA8 written is RGBA8 read.
//!
//! Behind the default-on `clipboard` feature. Without it — and on
//! Android, where arboard has no backend (M54/T54.8) — the type still
//! exists and every operation is a no-op, so a host does not need a
//! `cfg` around its Ctrl+C.

// The unixes arboard drives through X11, where its Linux extension
// traits — and so the primary selection — exist.
#[cfg(all(feature = "clipboard", unix, not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))))]
use arboard::{GetExtLinux, LinuxClipboardKind, SetExtLinux};
#[cfg(all(feature = "clipboard-image", not(target_os = "android")))]
use arboard::ImageData;

/// A handle on the system clipboard.
///
/// Held open rather than opened per operation: on X11 a clipboard owner
/// must stay alive to answer paste requests from other applications, so
/// a handle dropped after each copy would put text on the clipboard that
/// vanishes the moment the user switches windows.
pub struct Clipboard {
    #[cfg(all(feature = "clipboard", not(target_os = "android")))]
    inner: Option<arboard::Clipboard>,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard {
    /// Whether this build has a clipboard at all: the `clipboard`
    /// feature, on a platform arboard has a backend for. `false` means
    /// every operation is a no-op and a request is answered
    /// `Refused(Unsupported)`; `true` with no clipboard found at run time
    /// ([`Clipboard::is_open`]) is a platform that failed.
    pub const BUILT: bool = cfg!(all(feature = "clipboard", not(target_os = "android")));

    /// Whether this build can reach the primary selection:
    /// [`Clipboard::BUILT`] on the unixes arboard drives through X11.
    pub const HAS_PRIMARY: bool =
        cfg!(all(feature = "clipboard", unix, not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))));

    /// Whether this build can put images on the clipboard and read them
    /// back: [`Clipboard::BUILT`] with the default-off `clipboard-image`.
    pub const HAS_IMAGE: bool = cfg!(all(feature = "clipboard-image", not(target_os = "android")));

    /// Open it, or degrade to a no-op.
    ///
    /// There are real machines with no clipboard — a bare TTY, a
    /// container with no display, a Wayland-only session (above) — and an
    /// application should not fail to start on one. `None` inside means
    /// copy and paste do nothing, which is what "there is no clipboard"
    /// should look like.
    pub fn new() -> Self {
        Clipboard {
            #[cfg(all(feature = "clipboard", not(target_os = "android")))]
            inner: arboard::Clipboard::new().ok(),
        }
    }

    /// A handle with no clipboard behind it: what [`Clipboard::new`]
    /// degrades to on a machine with none. For a test that must not touch
    /// the desktop's clipboard.
    pub fn closed() -> Self {
        Clipboard {
            #[cfg(all(feature = "clipboard", not(target_os = "android")))]
            inner: None,
        }
    }

    /// Whether a clipboard was found when this handle was opened.
    pub fn is_open(&self) -> bool {
        #[cfg(all(feature = "clipboard", not(target_os = "android")))]
        return self.inner.is_some();
        #[cfg(not(all(feature = "clipboard", not(target_os = "android"))))]
        false
    }

    /// Put `text` on the clipboard. Returns whether it got there.
    #[cfg(all(feature = "clipboard", not(target_os = "android")))]
    pub fn set(&mut self, text: &str) -> bool {
        self.inner.as_mut().is_some_and(|c| c.set_text(text).is_ok())
    }

    /// Without the feature there is nowhere to put it.
    #[cfg(not(all(feature = "clipboard", not(target_os = "android"))))]
    pub fn set(&mut self, text: &str) -> bool {
        let _ = text;
        false
    }

    /// Read the clipboard's text, if it has any.
    #[cfg(all(feature = "clipboard", not(target_os = "android")))]
    pub fn get(&mut self) -> Option<String> {
        self.inner.as_mut().and_then(|c| c.get_text().ok())
    }

    /// Without the feature there is nothing to read.
    #[cfg(not(all(feature = "clipboard", not(target_os = "android"))))]
    pub fn get(&mut self) -> Option<String> {
        None
    }

    /// Put `html` on the clipboard, with `alt` as the plain-text
    /// alternative a reader that cannot take HTML pastes. Returns whether
    /// it got there. On X11 the HTML is offered as `text/html`, byte for
    /// byte, and `alt` as plain UTF-8 text.
    #[cfg(all(feature = "clipboard", not(target_os = "android")))]
    pub fn set_html(&mut self, html: &str, alt: &str) -> bool {
        self.inner.as_mut().is_some_and(|c| c.set_html(html, Some(alt)).is_ok())
    }

    /// Without the feature there is nowhere to put it.
    #[cfg(not(all(feature = "clipboard", not(target_os = "android"))))]
    pub fn set_html(&mut self, html: &str, alt: &str) -> bool {
        let _ = (html, alt);
        false
    }

    /// Read the clipboard as HTML, if it holds any.
    #[cfg(all(feature = "clipboard", not(target_os = "android")))]
    pub fn get_html(&mut self) -> Option<String> {
        self.inner.as_mut().and_then(|c| c.get().html().ok())
    }

    /// Without the feature there is nothing to read.
    #[cfg(not(all(feature = "clipboard", not(target_os = "android"))))]
    pub fn get_html(&mut self) -> Option<String> {
        None
    }

    /// Put an RGBA8 image, `width × height` pixels row-major from the top
    /// left, on the clipboard. Returns whether it got there; `false` when
    /// `rgba` is not `4 × width × height` bytes, and always `false`
    /// without `clipboard-image` ([`Clipboard::HAS_IMAGE`]).
    pub fn set_image(&mut self, width: u32, height: u32, rgba: &[u8]) -> bool {
        #[cfg(all(feature = "clipboard-image", not(target_os = "android")))]
        {
            if rgba.len() != 4 * width as usize * height as usize {
                return false;
            }
            let image = ImageData { width: width as usize, height: height as usize, bytes: rgba.into() };
            self.inner.as_mut().is_some_and(|c| c.set_image(image).is_ok())
        }
        #[cfg(not(all(feature = "clipboard-image", not(target_os = "android"))))]
        {
            let _ = (width, height, rgba);
            false
        }
    }

    /// Read the clipboard as an RGBA8 image — `(width, height, rgba)` —
    /// if it holds one; always `None` without `clipboard-image`.
    pub fn get_image(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        #[cfg(all(feature = "clipboard-image", not(target_os = "android")))]
        return self.inner.as_mut().and_then(|c| c.get_image().ok()).map(|i| (i.width as u32, i.height as u32, i.bytes.into_owned()));
        #[cfg(not(all(feature = "clipboard-image", not(target_os = "android"))))]
        None
    }

    /// Put `text` on the **primary selection**. Returns whether it got
    /// there; always `false` where there is none
    /// ([`Clipboard::HAS_PRIMARY`]). The clipboard is left alone.
    pub fn set_primary(&mut self, text: &str) -> bool {
        #[cfg(all(feature = "clipboard", unix, not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))))]
        return self.inner.as_mut().is_some_and(|c| c.set().clipboard(LinuxClipboardKind::Primary).text(text).is_ok());
        #[cfg(not(all(feature = "clipboard", unix, not(any(target_os = "macos", target_os = "android", target_os = "emscripten")))))]
        {
            let _ = text;
            false
        }
    }

    /// Read the primary selection's text, if it has any; always `None`
    /// where there is none ([`Clipboard::HAS_PRIMARY`]).
    pub fn get_primary(&mut self) -> Option<String> {
        #[cfg(all(feature = "clipboard", unix, not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))))]
        return self.inner.as_mut().and_then(|c| c.get().clipboard(LinuxClipboardKind::Primary).text().ok());
        #[cfg(not(all(feature = "clipboard", unix, not(any(target_os = "macos", target_os = "android", target_os = "emscripten")))))]
        None
    }
}
