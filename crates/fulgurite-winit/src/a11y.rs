//! **The platform accessibility adapter** — M54/T54.2
//! (`docs/plan-a11y-platform.md` §2.2, R15-F1).
//!
//! `accesskit_winit` bridges one `accesskit::TreeUpdate` to AT-SPI, UI
//! Automation and NSAccessibility. This is the one place the framework
//! drives it: [`run`](crate::run) and every `Program` through `Win`.
//! `examples/counter-window`'s own event loop carried a second copy of
//! all of this until T54.2, and a second caller of it until M60 deleted
//! that loop.
//!
//! The order matters and is `accesskit_winit`'s contract: the window is
//! created **invisible**, the adapter is built, then the window is shown
//! ([`Accessible::attach`] does the last two). The first tree is sent
//! when the platform asks for it (`InitialTreeRequested`), and after
//! every frame while an assistive technology is attached; while none is,
//! `update_if_active` never builds one, so an application nobody reads
//! pays nothing per frame. Incremental updates are deferred (the plan's
//! §4): each update is the whole tree.

use accesskit_winit::{Adapter, WindowEvent as AkEvent};
/// The adapter's callback, as a loop's user event must accept it
/// (`From<Event>`): re-exported so an application with its own loop
/// needs no `accesskit_winit` dependency of its own.
pub use accesskit_winit::Event;
use fulgurite::observe::accesskit::{ActionRequest, TreeUpdate};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::{Window, WindowAttributes};

/// The session's tree, or — when it could not build one (a title that
/// panicked, M36) — a window node alone, so the platform is never handed
/// nothing.
pub fn tree_of(session: &dyn fulgurite_shell::session::Session) -> TreeUpdate {
    use fulgurite::observe::accesskit::{Node, Role, TreeId, TreeInfo};
    use fulgurite::observe::WINDOW_NODE;
    session.a11y_tree().map(|t| t.update).unwrap_or_else(|| TreeUpdate {
        nodes: vec![(WINDOW_NODE, Node::new(Role::Window))],
        tree: Some(TreeInfo::new(WINDOW_NODE)),
        tree_id: TreeId::ROOT,
        focus: WINDOW_NODE,
    })
}

/// The framework's action for a platform's request — M54/T54.4. `Click`
/// is `Default`; a `SetValue` carries a number, narrowed to `f32`
/// (`Event` is 16 B and the firmware links no soft-`f64`); anything the
/// framework has no word for is `None` and is not routed.
pub fn action_of(req: &ActionRequest) -> Option<(u64, fulgurite::Action)> {
    use fulgurite::observe::accesskit::{Action as Ak, ActionData};
    use fulgurite::Action;
    let a = match req.action {
        Ak::Click => Action::Default,
        Ak::Increment => Action::Increment,
        Ak::Decrement => Action::Decrement,
        Ak::Expand => Action::Expand,
        Ak::Collapse => Action::Collapse,
        Ak::Focus => Action::Focus,
        // M90/T90.3: a context request inside the target.
        Ak::ShowContextMenu => Action::ContextMenu,
        Ak::SetValue => match req.data {
            Some(ActionData::NumericValue(v)) => Action::SetValue(v as f32),
            _ => return None,
        },
        _ => return None,
    };
    Some((req.target_node.0, a))
}

/// The window's attributes, invisible when an adapter will be attached —
/// `accesskit_winit` panics if the window has been shown before it.
pub fn attributes(attrs: WindowAttributes, accessible: bool) -> WindowAttributes {
    if accessible {
        attrs.with_visible(false)
    } else {
        attrs
    }
}

/// One window's adapter.
pub struct Accessible {
    adapter: Adapter,
}

impl Accessible {
    /// Build the adapter for `window`, which must not have been shown,
    /// then show it. Callbacks arrive as user events through `proxy`.
    pub fn attach<T: From<Event> + Send + 'static>(
        event_loop: &ActiveEventLoop,
        window: &Window,
        proxy: EventLoopProxy<T>,
    ) -> Self {
        let adapter = Adapter::with_event_loop_proxy(event_loop, window, proxy);
        window.set_visible(true);
        Accessible { adapter }
    }

    /// Every window event, before anything else sees it: the adapter
    /// watches focus and geometry and never consumes one.
    pub fn process_event(&mut self, window: &Window, event: &winit::event::WindowEvent) {
        self.adapter.process_event(window, event);
    }

    /// Send the tree if an assistive technology is attached. `tree` is not
    /// called otherwise.
    pub fn update(&mut self, tree: impl FnOnce() -> TreeUpdate) {
        self.adapter.update_if_active(tree);
    }

    /// A callback from the adapter. The first tree is sent here; an
    /// action is handed back for the caller to route.
    pub fn event(&mut self, e: Event, tree: impl FnOnce() -> TreeUpdate) -> Option<ActionRequest> {
        match e.window_event {
            AkEvent::InitialTreeRequested => {
                self.adapter.update_if_active(tree);
                None
            }
            AkEvent::ActionRequested(req) => Some(req),
            AkEvent::AccessibilityDeactivated => None,
        }
    }
}
