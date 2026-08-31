//! lumen-static — static-tree GUI framework core (Step 2 scope: node
//! tables, traversal, partitioning; spec in `docs/00-handoff-static-tree.md`).

pub mod assemble;
pub mod desc;
pub mod event;
pub mod fuse;
pub mod hit;
pub mod inbox;
#[cfg(feature = "observe")]
pub mod observe;
pub mod ops;
pub mod pass;
#[cfg(feature = "reload")]
pub mod reload;
pub mod repeat;
pub mod safe;
pub mod widget;

pub use desc::{Constraints, Inner, NodeDesc, NodeState, Point, Size, WidgetOps, NO_PARENT};
pub use event::{Event, EventCtx, InputQueue, Key};
pub use lumen_static_macros::{Reloadable, SafeTree, StaticTree};
pub use widget::Widget;
