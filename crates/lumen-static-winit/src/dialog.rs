//! Platform file, colour and font pickers — `docs/plan-capabilities.md`
//! T15.11.
//!
//! Behind the default-off `dialogs` feature, and **the measurement is
//! why rather than a formality**: `rfd` costs **79 crates**
//! (218 → 297, 2026-09-07) against `clipboard`'s 11 and `a11y`'s 62.
//! It is the most expensive optional thing in this workspace.
//!
//! The `gtk3` backend was chosen over `xdg_portal` on the same
//! measurement: the `xdg_portal` backend costs **273** crates (218 → 491),
//! because it pulls an async runtime for a dialog this framework opens
//! synchronously. Neither is cheap, which is why the feature is off and
//! why the number is written down where someone deciding can see it.
//!
//! Without the feature the type still exists and every call returns
//! `None`, so a host needs no `cfg` around its "Open…" handler and an
//! application built without dialogs still compiles and still runs.

/// The platform's pickers.
///
/// A unit struct rather than a handle: `rfd` opens each dialog fresh,
/// and holding state would imply a lifetime it does not have. It exists
/// so the no-feature build has something to call.
#[derive(Clone, Copy, Debug, Default)]
pub struct Dialogs;

impl Dialogs {
    pub fn new() -> Self {
        Dialogs
    }

    /// Ask for one file to open. `None` if the user cancelled — or if
    /// the feature is off, which a caller cannot distinguish and does
    /// not need to: there is no file either way.
    #[cfg(feature = "dialogs")]
    pub fn open_file(&self) -> Option<std::path::PathBuf> {
        rfd::FileDialog::new().pick_file()
    }

    /// Without the feature there is no picker.
    #[cfg(not(feature = "dialogs"))]
    pub fn open_file(&self) -> Option<std::path::PathBuf> {
        None
    }

    /// Ask where to save.
    #[cfg(feature = "dialogs")]
    pub fn save_file(&self) -> Option<std::path::PathBuf> {
        rfd::FileDialog::new().save_file()
    }

    #[cfg(not(feature = "dialogs"))]
    pub fn save_file(&self) -> Option<std::path::PathBuf> {
        None
    }

    /// Ask for a folder.
    #[cfg(feature = "dialogs")]
    pub fn pick_folder(&self) -> Option<std::path::PathBuf> {
        rfd::FileDialog::new().pick_folder()
    }

    #[cfg(not(feature = "dialogs"))]
    pub fn pick_folder(&self) -> Option<std::path::PathBuf> {
        None
    }
}
