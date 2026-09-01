//! lumen-static — static-tree GUI framework core.
//!
//! Spec: `docs/00-handoff-static-tree.md`. Traversal is recursive descent
//! over the widget struct (`docs/results-traversal.md`); the const node
//! table survives for index addressing, which the agent surface, change
//! journal and ambient audit are keyed on. No `unsafe` in a release
//! build — see `docs/results-safe-prototype.md`.

pub mod assemble;
pub mod cx;
pub mod desc;
pub mod display;
pub mod event;
pub mod hatch;
pub mod hit;
pub mod inbox;
#[cfg(feature = "observe")]
pub mod observe;
pub mod ops;
pub mod pass;
#[cfg(feature = "reload")]
pub mod reload;
pub mod repeat;
pub mod style;
pub mod text;
pub mod widget;

pub use desc::{Constraints, NodeDesc, NodeState, Point, Size, MAX_DEPTH, NO_PARENT};
pub use hatch::{Dyn, Either, List, Side};
pub use hit::{Hit, Inner};
pub use cx::{BlockMetrics, LayoutCx, TextMeasure};
pub use display::{Cmd, Color, DisplayList, Renderer};
pub use style::{Align, Edges, Length, Patch, Rule, Selector, Style};
pub use text::{Dirty, Measured, TextStorage};
pub use event::{Event, EventCtx, InputQueue, Key};
pub use lumen_static_macros::{Reloadable, StaticTree};
pub use widget::{Node, Widget};
