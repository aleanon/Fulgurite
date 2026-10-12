//! **The tray's seam** — M90/T90.6 (`docs/plan-desktop-integration.md`,
//! "The Linux tray is D-Bus, not GTK", "Model and state cross by data").
//!
//! A program declares its tray once, as data: [`TraySpec`], installed by
//! `Program::tray` through `Program::host()` the way `Program::menu` is,
//! so a hot swap's new session carries the new build's spec. The window
//! never sees the spec. It asks the session two questions, and both
//! answers are owned data:
//!
//! - `Session::tray_model` — the labels, the tooltip and the icon as a
//!   [`TrayModel`] of owned `String`s and RGBA, with a content hash, so a
//!   window rebuilds its tray only when the hash differs;
//! - `Session::menu_state` — the enabled [`MenuMask`](fulgurite::menu::MenuMask), `Some` only when it
//!   changed since the session was last asked.
//!
//! No `fn` of the application's reaches the loader-side window, which is
//! type-erased over `Box<dyn Session>` and could not call one (review
//! A23): the spec's `enabled` function is called **in the host**
//! ([`Host::tray_enabled`](crate::Host::tray_enabled)).
//!
//! Which side of the seam each tray setting is on
//! (`docs/plan-window-reload.md` §2.7): the spec's content is re-read
//! from the new session after a swap; whether a tray exists at all is the
//! window's, read once from the static binary, and not reloadable.

use fulgurite::event::App;
use fulgurite::menu::{MenuEntry, MAX_ENTRIES};

use crate::program::MenuEnabled;

/// **A program's tray, declared once** — what `Program::tray` installs.
///
/// - `icon`: `(width, height, rgba)`, straight RGBA8, row-major,
///   `width × height × 4` bytes. Pixels in the spec, not a name resolved
///   against the window's image store: that store is lent to the backend
///   while a window exists, a daemon may have no backend, and it is read
///   once from the static binary, so a swap could never change the icon
///   (`docs/plan-desktop-integration.md`, "The icon is pixels in the
///   spec").
/// - `tooltip`: what the tray shows on hover.
/// - `menu`: the entries, one flat list; a chosen entry reaches
///   `Program::on_menu` as its id, posted as `Event::Menu`.
/// - `enabled`: which entries are enabled, from the state — bit *n* is
///   `menu[n]`. Called in the host, never in the window.
pub struct TraySpec<A: App> {
    pub icon: (u32, u32, &'static [u8]),
    pub tooltip: &'static str,
    pub menu: &'static [MenuEntry],
    pub enabled: MenuEnabled<A>,
}

impl<A: App> Clone for TraySpec<A> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<A: App> Copy for TraySpec<A> {}

impl<A: App> TraySpec<A> {
    /// **The spec, as owned data** — what `Session::tray_model` hands the
    /// window. Copies the labels, the tooltip and the icon; the content
    /// hash folds all of them, so a window that compares hashes rebuilds
    /// its tray exactly when one of them changed.
    pub fn model(&self) -> TrayModel {
        let (w, h, rgba) = self.icon;
        let entries: Vec<TrayEntry> =
            self.menu.iter().map(|e| TrayEntry { label: e.label.to_owned(), id: e.id }).collect();
        let hash = model_hash(self.tooltip, &entries, (w, h, rgba));
        TrayModel { icon: (w, h, rgba.to_vec()), tooltip: self.tooltip.to_owned(), entries, hash }
    }
}

/// One tray entry, owned: its label and the id `Program::on_menu` hears.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayEntry {
    pub label: String,
    pub id: u32,
}

/// **A tray, as the window gets it** — owned, so nothing in it points
/// into a hot library that may be swapped out after the call returns
/// (seam S1, `docs/plan-iced-roadmap.md` §2).
///
/// `hash` is the content hash of everything else here: the window keeps
/// the hash it built from and rebuilds only when a new model's differs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayModel {
    /// `(width, height, rgba)`, as [`TraySpec::icon`].
    pub icon: (u32, u32, Vec<u8>),
    pub tooltip: String,
    pub entries: Vec<TrayEntry>,
    pub hash: u64,
}

/// FNV-1a over the tooltip, every entry's label and id, and the icon's
/// size and pixels — each string length-prefixed, so moving a byte from
/// one label to the next changes the hash.
fn model_hash(tooltip: &str, entries: &[TrayEntry], icon: (u32, u32, &[u8])) -> u64 {
    fn eat(h: &mut u64, bytes: &[u8]) {
        for b in bytes {
            *h = (*h ^ *b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn text(h: &mut u64, s: &str) {
        eat(h, &(s.len() as u64).to_le_bytes());
        eat(h, s.as_bytes());
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    text(&mut h, tooltip);
    eat(&mut h, &(entries.len() as u64).to_le_bytes());
    for e in entries {
        text(&mut h, &e.label);
        eat(&mut h, &e.id.to_le_bytes());
    }
    eat(&mut h, &icon.0.to_le_bytes());
    eat(&mut h, &icon.1.to_le_bytes());
    eat(&mut h, icon.2);
    h
}

/// Refuse a spec the tray cannot represent, with a sentence naming why:
/// more entries than a [`MenuMask`](fulgurite::menu::MenuMask) has bits, or an icon whose byte count
/// is not `width × height × 4`. Called by `Program::tray`.
pub(crate) fn check<A: App>(spec: &TraySpec<A>) {
    let n = spec.menu.len();
    assert!(
        n <= MAX_ENTRIES,
        "a tray's menu holds at most {MAX_ENTRIES} entries (one bit each of its MenuMask); this one has {n}"
    );
    let (w, h, rgba) = spec.icon;
    let want = w as usize * h as usize * 4;
    assert!(
        rgba.len() == want,
        "a tray icon of {w} x {h} is {want} bytes of RGBA; this one has {}",
        rgba.len()
    );
}
