//! A GUI framework where the widget tree's shape lives in the type.
//!
//! A `#[derive(StaticTree)]` struct *is* the tree. Its fields are its
//! children, `NODE_COUNT` and the node table are computed at compile
//! time, and a frame allocates nothing.
//!
//! ```
//! use lumen_static::prelude::*;
//!
//! #[derive(StaticTree, Default)]
//! #[widget(column, spacing = 4.0)]
//! struct Ui {
//!     #[widget_id("title")]
//!     title: Label,
//!     gap: Spacer,
//!     #[widget_id("badge")]
//!     badge: Fixed,
//! }
//!
//! # fn main() {
//! assert_eq!(<Ui as Widget>::NODE_COUNT, 4);   // itself plus three fields
//! # }
//! ```
//!
//! # Writing a widget
//!
//! ## `Node` and `Widget` are two halves of one thing
//!
//! [`Widget`] carries the compile-time facts — `NODE_COUNT`, `DEPTH`,
//! `SUB_TABLE` — which is exactly what a trait object cannot have.
//! [`Node`] carries the methods and is object-safe, so a container
//! reaches a child as `&mut dyn Node`. Every widget implements both.
//!
//! The practical consequence, and the first thing most people hit:
//! **calling `.node(i)`, `.layout(..)` or `.defaults()` needs
//! `use lumen_static::Node` in scope**, because they are trait methods on
//! the object-safe half. `prelude` brings it in.
//!
//! ## The attributes
//!
//! Seven, across three derives. This list is the only place they appear
//! together.
//!
//! | attribute | on | meaning |
//! |---|---|---|
//! | `#[widget(row \| column, spacing = 4.0)]` | a `StaticTree` struct | axis, and the gap between children |
//! | `#[widget(fuse)]` | a `StaticTree` struct | emit static calls instead of `&dyn` — the erasure dial |
//! | `#[widget_id("save")]` | a field | stable author id: an agent selector, and `#save` in a stylesheet |
//! | `#[widget_class("row", "striped")]` | a field | style classes, matched by `.row` |
//! | `#[reload(keep \| fresh \| reset)]` | a `Reloadable` field | survives a hot swap / comes from the new code / recomputed |
//! | `#[tree]` | an `App` field | which field is the widget tree |
//! | `#[bind(ui.value, "count: {}")]` | an `App` field | how this state reaches the screen |
//! | `#[app(after_update = f)]` | an `App` struct | `fn(&mut Self)`, run after the bindings |
//!
//! `#[bind]` also takes `with = f` for a `fn(&mut Target, &Source)`
//! projection, and extra format arguments after the string.
//!
//! ## Two naming conventions for `kind_name`
//!
//! Selectors match on it, and it is not uniform: **built-in leaves are
//! lowercase** (`label`, `button`, `fixed`, `spacer`) while **derived
//! containers use the struct's own PascalCase ident** (`Ui`, `Controls`).
//! So `Selector::Kind("label")` and `Selector::Kind("Ui")` are both
//! right. Worth knowing before writing a stylesheet.
//!
//! ## Where state lives
//!
//! Application state sits in sibling fields; widget fields hold the
//! *presentation copy*, because only owned storage can invalidate a
//! measurement cache (`docs/layout-and-storage-decisions.md` §1).
//! `#[bind]` keeps the two in agreement, and the frame loop runs it
//! between input and layout so it cannot be forgotten.
//!
//! Off-table constructs — [`Repeat`](repeat::Repeat),
//! [`List`], [`Either`], [`Dyn`] —
//! keep their instances outside the table's parallel arrays, in an
//! [`Arena`](arena::Arena). A stylesheet reaches inside them by template
//! (`style::cascade_deep`); per-instance styling does not exist.
//!
//! # Design record
//!
//! Spec: `docs/00-handoff-static-tree.md`. Traversal is recursive descent
//! over the widget struct (`docs/results-traversal.md`); the const node
//! table survives for index addressing, which the agent surface, change
//! journal and ambient audit are keyed on. No `unsafe` in a release
//! build — see `docs/results-safe-prototype.md`, and
//! `scripts/check_no_unsafe.sh`, which enforces it.

pub mod arena;
pub mod assemble;
pub mod bind;
pub mod cx;
pub mod desc;
pub mod display;
pub mod event;
pub mod hatch;
pub mod hit;
pub mod inbox;
pub mod num;
#[cfg(feature = "observe")]
pub mod observe;
pub mod ops;
pub mod pass;
#[cfg(feature = "reload")]
pub mod reload;
pub mod input;
pub mod retain;
pub mod scroll;
pub mod slider;
pub mod textarea;
pub mod toggle;
pub mod repeat;
pub mod style;
pub mod text;
pub mod widget;

pub use desc::{Constraints, NodeDesc, NodeState, Point, Size, MAX_DEPTH, NO_PARENT};
pub use hatch::{Dyn, Either, List, Side};
pub use input::TextInput;
pub use scroll::ScrollView;
pub use slider::Slider;
pub use textarea::TextArea;
pub use toggle::{CheckBox, Radio, Switch, Toggle};
pub use hit::{Hit, Inner};
pub use cx::{BlockMetrics, Cluster, GlyphSink, LayoutCx, NoSink, TextMeasure};
pub use retain::{Damage, Incremental, Rebuild, Retention};
pub use display::{Cmd, Color, DisplayList, Painter, Renderer, Scaled};
pub use style::{Align, Edges, Length, Patch, Rule, Selector, Style};
pub use text::{ClusterBuf, Dirty, Measured, TextBuffer, TextStorage};
#[cfg(feature = "reload")]
pub use text::TextRestore;
pub use event::{Event, EventCtx, InputQueue, Key};
// `App` is both a trait and a derive; they live in different namespaces,
// so `use lumen_static::App` brings in whichever the position calls for.
pub use event::App;
pub use lumen_static_macros::{App, Reloadable, StaticTree};
pub use widget::{Node, Widget};

/// Everything an application needs to build a tree.
///
/// There was no prelude, and the root re-exports were inverted: the rare
/// escape hatches (`Dyn`, `Either`, `List`) were at the root while
/// `Label` and `Button` needed `lumen_static::ops::`. The shipped
/// example imports from three modules plus the root for a forty-line app
/// (`docs/plan-review-fixes.md` F6.1).
///
/// `Node` is here because a container reaches its children as
/// `&mut dyn Node`, so calling `.node(i)` needs the trait in scope — and
/// the first place most people meet that fact is a rustc error.
pub mod prelude {
    pub use crate::desc::{Constraints, NodeState, Point, Size};
    pub use crate::event::{App, EventCtx, Event, Key};
    pub use crate::hatch::{Dyn, Either, List, Side};
    pub use crate::input::TextInput;
    pub use crate::scroll::ScrollView;
    pub use crate::slider::Slider;
    pub use crate::textarea::TextArea;
    pub use crate::toggle::{CheckBox, Radio, Switch};
    pub use crate::ops::{Button, Fixed, Label, Spacer};
    pub use crate::repeat::Repeat;
    pub use crate::style::{Align, Edges, Length, Patch, Rule, Selector, Style};
    pub use crate::widget::{Node, Widget};
    pub use crate::{pass, App as DeriveApp, Reloadable, StaticTree};
}
