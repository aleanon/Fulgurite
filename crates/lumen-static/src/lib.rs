//! A GUI framework where the widget tree's shape lives in the type.
//!
//! A `#[derive(StaticTree)]` struct *is* the tree. Its fields are its
//! children, `NODE_COUNT` and the node table are computed at compile
//! time, and the tree's passes allocate nothing in steady state — the
//! shell's truetype text path included, since M50/T50.2
//! (`docs/results-allocation.md`, "The truetype path, covered").
//!
//! ```
//! use lumen_static::prelude::*;
//! use lumen_static_widgets::prelude::*;
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
//! Across three derives. How many is not written here (it was wrong
//! twice): `scripts/check_attr_table.sh` requires every attribute the
//! derives declare, and every key their parsers accept, to have a row in
//! this table and in the book's (`book/src/attributes.md`).
//!
//! | attribute | on | meaning |
//! |---|---|---|
//! | `#[widget(row \| column \| stack, spacing = 4.0)]` | a `StaticTree` struct | container mode, and the gap between children. `stack`'s children share its rect (M11/T11.6) and it takes no spacing |
//! | `#[widget(wrap)]` | a `StaticTree` struct | children flow left to right and wrap onto the next line when the width runs out (M27/T27.8) |
//! | `#[widget(grid, columns = 3)]` | a `StaticTree` struct | a wrap that breaks every `columns` children instead of on width; `columns` belongs to `grid` alone, and 0 is refused |
//! | `#[widget(fuse)]` | a `StaticTree` struct | emit static calls instead of `&dyn` — the erasure dial |
//! | `#[widget_id("save")]` | a field | stable author id: an agent selector, and `#save` in a stylesheet |
//! | `#[widget_class("row", "striped")]` | a field **or the struct** | style classes, matched by `.row`; on the struct they land on the container's own node |
//! | `#[reload(keep \| fresh \| reset)]` | a `Reloadable` field | survives a hot swap / comes from the new code / recomputed |
//! | `#[tree]` | an `App` field | which field is the widget tree |
//! | `#[state]` | an `App` field | what a handler receives, and the tree is not in it. Absent, a handler gets the whole application |
//! | `#[bind(ui.value, "count: {}")]` | an `App` field | how this state reaches the screen |
//! | `#[bind(ui.value, with = f)]` | an `App` field | a `fn(&mut Target, &Source)` projection; it cannot say whether it wrote, so it marks everything |
//! | `#[app(after_update = f)]` | an `App` struct | `fn(&mut Self) -> bool`, run after the bindings; the `bool` says whether it changed anything |
//! | `#[app(tick = f)]` | an `App` struct | `fn(&mut Self, f32) -> bool`, called each frame with the elapsed ms; the `bool` says whether anything is still moving (M22/T22.4) |
//!
//! **Each key once, and nothing ignored** (M55/T55.2). A repeated key —
//! two container modes, `spacing` twice, a second `#[widget_id]` on a
//! field, `tick` twice, two reload modes — is an error, not a silent
//! overwrite; so are `#[tree]` or `#[state]` with arguments, an empty or
//! struct-level `#[widget_id]`, and a `StaticTree` attribute on an enum.
//!
//! `#[bind]` also takes `with = f` for a `fn(&mut Target, &Source)`
//! projection, and extra format arguments after the string — evaluated
//! in your scope, never the derive's: a function of yours called
//! `changed` is yours (M55/T55.1). A **source
//! path** may sit between the target and the format string —
//! `#[bind(ui.value, count, "count: {}")]` on a field `state: Tally`
//! formats `self.state.count` — which is what lets an application whose
//! state is its own struct keep the comparing form instead of falling
//! back on `with =`. With a source path and **no** format string it is
//! the **value form** — `#[bind(ui.pane.w, blur)]` copies
//! `self.state.blur` through [`bind::BindValue`], which compares first —
//! for a target that is not text (M33/T33.1).
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

// M13/T13.1. `std` and `alloc` are both default-on and there is no
// `no_std` feature, because cargo features are additive: a subtractive
// one would let a dependent strip `std` from crates that never asked.
// `docs/plan-embedded.md` T13.1 has the argument.
#![cfg_attr(not(feature = "std"), no_std)]
// M0's "no `unsafe` in the core", held by the compiler since M43/T43.1
// rather than by a text scan: `forbid` cannot be overridden by an inner
// `allow`, and it covers `#[path]` modules and code a build script writes
// to OUT_DIR for the crate to include, which a scan of `src/` never reads.
// Every crate but `lumen-static-reload` (the FFI shims) carries it;
// `scripts/check_no_unsafe.sh` checks that it does.
#![forbid(unsafe_code)]

// The release-shape tripwire (M43/T43.7). `just release-shape` builds with
// `--cfg lumen_release_shape` in its own target directory; under it, core
// refuses to compile with the dev surface on. A dev-dependency's features
// unify into the crate under test, so an edge that asks for `observe` or
// `reload` silently turns the "shipped shape" gate into a dev build --
// which is what core's own dev edge on the widgets did until M43 (R12-F1).
#[cfg(all(lumen_release_shape, any(feature = "observe", feature = "reload")))]
compile_error!(
    "release-shape tripwire: lumen-static was built with `observe` or `reload` on in a \
     release-shape build -- some dependency edge (often a dev-dependency) enabled it. \
     `cargo tree -p lumen-static -e features -i lumen-static` names the edge."
);

// Available in a `std` build too -- `std` re-exports it -- so this needs
// no cfg of its own beyond the tier that supplies `Vec` and `String`.
#[cfg(feature = "alloc")]
extern crate alloc;

pub mod anim;
#[cfg(feature = "alloc")]
pub mod arena;
#[cfg(feature = "alloc")]
pub mod assemble;
pub mod bind;
pub mod cx;
pub mod desc;
pub mod display;
pub mod drag;
pub mod event;
pub mod font;
pub mod gesture;
pub mod hatch;
pub mod hit;
pub mod i18n;
pub mod inline;
pub mod invalidate;
// `mpsc`, so `std` (M13/T13.1). Deferred work handed back to the frame
// loop from another thread is not an embedded concern.
#[cfg(feature = "std")]
pub mod inbox;
pub mod ops;
pub mod num;
// Threads and the inbox both, so `std` for the same reason (M13/T13.1).
#[cfg(feature = "std")]
pub mod spawn;
// The accessibility tree (`a11y`, M54/T54.1); its JSON view for agents
// is the `observe` half inside the module.
#[cfg(feature = "a11y")]
pub mod observe;

/// Whether this build of the core has `observe` on — M48/T48.9 (R12-F3).
///
/// A dependent crate's `cfg` cannot see another crate's features, so a
/// crate whose `observe` impls are gated on its *own* feature asks this
/// instead. `lumen-static-widgets` does: an agent build that turns on the
/// core's `observe` without the widgets' reported every widget as
/// `GenericContainer`, silently, and now fails to compile, naming the
/// feature (`docs/results-widget-crate.md`, "One feature seam").
pub const OBSERVE: bool = cfg!(feature = "observe");

/// Whether this build of the core has `a11y` on — M54/T54.1. The same
/// seam as [`OBSERVE`], one level down: `a11y` gates every widget's
/// `Node::observe`, and a widgets crate whose own `a11y` is off would
/// report `GenericContainer` for all of them. `observe` implies `a11y`.
pub const A11Y: bool = cfg!(feature = "a11y");
pub mod overlay;
pub mod pass;
#[cfg(feature = "reload")]
pub mod reload;
pub mod retain;
#[cfg(feature = "alloc")]
pub mod repeat;
pub mod style;
pub mod text;
pub mod widget;
// A display list as build-independent bytes: the hot-reload boundary
// (M37/T37.0). Needs a `Vec`, and a `no_std` build that never names it
// links none of it.
#[cfg(feature = "alloc")]
pub mod wire;

pub use desc::{
    stack_bytes, Constraints, NodeDesc, NodeState, Point, Size, MAX_DEPTH, NO_PARENT,
    STACK_BASE_BYTES, STACK_BYTES_PER_LEVEL,
};
// `Dyn` and `List` own runtime-sized content, so they follow `alloc`
// (T13.4). `Either` and `Side` are compile-time and always present.
pub use hatch::{Either, Side};
pub use invalidate::{Boundaries, Everything, Invalidation};
#[cfg(feature = "alloc")]
pub use hatch::{Dyn, List, NodeFor};
pub use hit::{Hit, Inner};
pub use cx::{BlockMetrics, Cluster, GlyphSink, LayoutCx, LineCount, NoSink, PathSink, TextMeasure};
// T13.8 removed the `&DisplayList` from `Retention`, so retention is
// available with no allocator: `Streaming<N>` keeps `(hash, ink)` in a
// fixed array. Only `Incremental`, whose buffer grows, needs `alloc`.
pub use retain::{Damage, Rebuild, Recording, Retention, Streaming};
#[cfg(feature = "alloc")]
pub use retain::Incremental;
// `DisplayList` is one `Painter`, not the vocabulary: it owns a `Vec`
// of commands and a `String` of text, so it follows `alloc` (T13.4).
// A host without one paints straight through the trait (M10a).
pub use display::{Cmd, Color, ImageId, ImageSource, Painter, Scaled};
#[cfg(feature = "alloc")]
pub use display::{DisplayList, Renderer};
pub use style::{Align, Edges, Length, Patch, Rule, Selector, Style, TextOverflow};
pub use inline::{InlineClusters, InlineText};
pub use text::{ClusterBuf, Dirty, MeasureKey, Measured, TextBuffer, TextStorage};
#[cfg(feature = "reload")]
pub use text::TextRestore;
pub use event::{Action, Event, EventCtx, InputQueue, Key, Mods, PointerButton, TouchPhase};
// `App` is both a trait and a derive; they live in different namespaces,
// so `use lumen_static::App` brings in whichever the position calls for.
pub use event::App;
pub use lumen_static_macros::{App, Reloadable, StaticTree};
/// **A `.lss` stylesheet as `const` rules, parsed at compile time** — M61
/// (`docs/plan-lss.md`).
///
/// ```text
/// const BASE: &[Rule] = include_lss!(Page, "../app.lss");       // the top-level rules
/// const POS: &[Rule] = include_lss!(Page, "../app.lss", pos);   // …then `@sheet pos`'s
/// ```
///
/// The file is read while the invoking crate compiles, and the expansion
/// is the `Rule::new(..)` / `Patch::new().setter(..)` a person would
/// write, as a `const` — so it links to the same bytes as the hand-written
/// sheet (kill rule K1, `scripts/check_lss_parity.sh`) and costs nothing at
/// run time. The path is relative to the invoking source file, as
/// `include_bytes!`'s is, and an edited file rebuilds the crate
/// (`scripts/lss_tracking.sh`).
///
/// **A mistake is a compile error at the file's line and column**: an
/// unknown property, a value its setter cannot take, and everything v1
/// refuses — a compound selector, a combinator, a `:state`, `@media` —
/// each with its own message. The grammar is the book's styling chapter's
/// (`book/src/styling.md`).
pub use lumen_static_macros::include_lss;

/// **What `#[derive(Reloadable)]` expands through** — M49/T49.9, R5-F8.
///
/// The derive and its `#[reload(..)]` attribute are always registered, so
/// an application that derives `Reloadable` builds whether or not hot
/// reload is on. The derive wraps everything it emits in this macro, and
/// the core defines it twice: with the `reload` feature it passes the
/// tokens through, and without it it expands to **nothing** — no impl,
/// no path into `lumen_static::reload` (which does not exist then), no
/// serde. So the dev surface costs a release build nothing, which
/// handoff §9 requires, and `just reload-gating` checks.
#[doc(hidden)]
#[cfg(feature = "reload")]
#[macro_export]
macro_rules! __reloadable {
    ($($t:tt)*) => { $($t)* };
}

/// See the `reload` definition above; this one is the release shape.
#[doc(hidden)]
#[cfg(not(feature = "reload"))]
#[macro_export]
macro_rules! __reloadable {
    ($($t:tt)*) => {};
}
pub use overlay::{NoOverlays, OverlaySink, Overlays};
pub use anim::{Animated, Easing, Lerp};
pub use drag::{Drag, DragPhase};
pub use i18n::{Catalogue, Direction};
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
    pub use crate::event::{App, EventCtx, Event, Key, Mods, PointerButton};
    pub use crate::hatch::{Either, Side};
    #[cfg(feature = "alloc")]
    pub use crate::hatch::{Dyn, List};
    #[cfg(feature = "alloc")]
    pub use crate::repeat::Repeat;
    pub use crate::style::{Align, Edges, Length, Patch, Rule, Selector, Style, Stylesheet};
    pub use crate::widget::{Node, Widget};
    pub use crate::{pass, App as DeriveApp, Reloadable, StaticTree};
}
