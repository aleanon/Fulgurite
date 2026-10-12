//! **The Linux tray** — M90/T90.7 (`docs/plan-desktop-integration.md`,
//! "The Linux tray is D-Bus, not GTK"). Compiled with the `tray` feature,
//! on Linux only.
//!
//! A StatusNotifierItem with a `com.canonical.dbusmenu` menu, served by
//! `ksni`'s blocking service over zbus on a thread of its own. **No
//! GTK**: `tray-icon` needs a GTK loop on the thread that creates it, and
//! the `dialogs` feature's `rfd` already starts its own GTK thread, so a
//! second one would be two GTK initialisations on two threads. With
//! `async-io` as `ksni`'s runtime the transport is the one `a11y`'s
//! AT-SPI adapter already links; the crate count is in the manifest's
//! `tray` comment, from `docs/results-desktop-integration.md` T90.7.
//!
//! **Everything crosses as data** (seam S1). The window builds the item
//! from the primary session's owned `TrayModel` (`Session::tray_model`)
//! and applies each `MenuMask` `Session::menu_state` hands it; nothing of
//! the application's — no `fn`, no pointer into a hot library — is held
//! here. A click on an entry runs on `ksni`'s thread, which only **sends
//! the entry's id** through the event loop's proxy
//! (`UserEvent::Menu`); the window then posts `Event::Menu` to the
//! program's primary session on its own thread (`Win`'s `user_event`), so
//! the activation is input in the one queue, recorded and replayed like
//! any other.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use ksni::blocking::{Handle, TrayMethods};
use fulgurite::menu::MenuMask;
use fulgurite_shell::tray::TrayModel;
use winit::event_loop::EventLoopProxy;

use crate::UserEvent;

/// The window's tray: the running service (if it started), the model and
/// the mask it was last given, and whether the desktop's watcher holds it.
pub(crate) struct Tray {
    handle: Option<Handle<Item>>,
    /// The model the item was built from — its labels are what `app.tray`
    /// reports, and its hash what a new model is compared with.
    pub(crate) model: TrayModel,
    /// The mask last applied, `MenuMask::ALL` until the session hands one.
    pub(crate) mask: MenuMask,
    #[cfg_attr(not(feature = "agent"), allow(dead_code))]
    registered: Arc<AtomicBool>,
    /// Why the service did not start: no session bus, or no
    /// `org.kde.StatusNotifierWatcher` on it. `None` once it runs.
    #[cfg_attr(not(feature = "agent"), allow(dead_code))]
    pub(crate) error: Option<String>,
}

impl Tray {
    /// **Start the service** for `model`, with clicks sent through
    /// `proxy`. A service that cannot start — no session bus, no watcher —
    /// is reported once on stderr and kept as an error `app.tray` reads;
    /// the application runs on without a tray.
    pub(crate) fn start(model: TrayModel, proxy: EventLoopProxy<UserEvent>, id: String) -> Tray {
        let registered = Arc::new(AtomicBool::new(false));
        let item = Item { model: model.clone(), mask: MenuMask::ALL, proxy, registered: registered.clone(), id };
        let (handle, error) = match item.spawn() {
            Ok(h) => {
                registered.store(true, Ordering::Relaxed);
                (Some(h), None)
            }
            Err(e) => {
                let e = e.to_string();
                eprintln!("fulgurite: no tray: {e}");
                (None, Some(e))
            }
        };
        Tray { handle, model, mask: MenuMask::ALL, registered, error }
    }

    /// Whether the desktop's StatusNotifierWatcher holds the item now —
    /// `app.tray`'s `registered`.
    #[cfg_attr(not(feature = "agent"), allow(dead_code))]
    pub(crate) fn registered(&self) -> bool {
        self.handle.as_ref().is_some_and(|h| !h.is_closed()) && self.registered.load(Ordering::Relaxed)
    }

    /// **A new model, rebuilt only when its hash differs** — after a hot
    /// swap, whose new session may carry other labels or another icon.
    /// Answers whether it rebuilt.
    #[cfg_attr(not(feature = "hot"), allow(dead_code))]
    pub(crate) fn rebuild(&mut self, model: TrayModel) -> bool {
        if model.hash == self.model.hash {
            return false;
        }
        if let Some(h) = &self.handle {
            let m = model.clone();
            h.update(move |item| item.model = m);
        }
        self.model = model;
        true
    }

    /// Apply the enabled mask `Session::menu_state` answered.
    pub(crate) fn set_mask(&mut self, mask: MenuMask) {
        if mask == self.mask {
            return;
        }
        if let Some(h) = &self.handle {
            h.update(move |item| item.mask = mask);
        }
        self.mask = mask;
    }
}

/// The item `ksni` serves: owned data and the proxy, nothing else.
struct Item {
    model: TrayModel,
    mask: MenuMask,
    proxy: EventLoopProxy<UserEvent>,
    registered: Arc<AtomicBool>,
    id: String,
}

impl ksni::Tray for Item {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn title(&self) -> String {
        self.model.tooltip.clone()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip { title: self.model.tooltip.clone(), ..Default::default() }
    }

    /// The spec's straight RGBA, as the ARGB32 (network byte order) the
    /// StatusNotifierItem protocol carries.
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let (w, h, rgba) = &self.model.icon;
        let data = rgba.chunks_exact(4).flat_map(|p| [p[3], p[0], p[1], p[2]]).collect();
        vec![ksni::Icon { width: *w as i32, height: *h as i32, data }]
    }

    /// One standard entry per model entry, enabled by its mask bit. A
    /// dbusmenu label treats `_` as a mnemonic marker, so each is doubled
    /// to show the label as written.
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        self.model
            .entries
            .iter()
            .enumerate()
            .map(|(n, e)| {
                let id = e.id;
                ksni::menu::StandardItem {
                    label: e.label.replace('_', "__"),
                    enabled: self.mask.enabled(n),
                    // **The click only sends the id** through the loop's
                    // proxy; the window posts it on its own thread.
                    activate: Box::new(move |item: &mut Item| {
                        let _ = item.proxy.send_event(UserEvent::Menu(id));
                    }),
                    ..Default::default()
                }
                .into()
            })
            .collect()
    }

    fn watcher_online(&self) {
        self.registered.store(true, Ordering::Relaxed);
    }

    fn watcher_offline(&self, _reason: ksni::OfflineReason) -> bool {
        self.registered.store(false, Ordering::Relaxed);
        // Keep the service: a watcher that comes back (a restarted
        // panel) registers the item again.
        true
    }
}
