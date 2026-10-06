//! **Every widget** — M23, and all of them since M30.
//!
//! This crate began as the second tier: `lumen-static` kept the widgets
//! "an application cannot be written without" — a label, a button, a
//! field, a scroller — and a badge or a chart lived here. **M30 moved
//! the first tier in too**, so there is one place a widget can be.
//!
//! The old split was defensible and cost more than it was worth. It put
//! the seam where no user could predict it — `Label` in core, `Badge`
//! here — and made drawing a form a two-crate dependency. What decided
//! it was measuring the thing the split was argued from: the embedded
//! leaf profile links `.text` **27,024 B** with the widgets here against
//! 27,036 B with them in core (`docs/results-widget-crate.md`), because
//! the gate was never the crate — it is `#[cfg(feature)]` and the
//! linker, which drop what a tree does not name either way.
//!
//! **The crate is still the gate for the *optional* tier.** A widget set
//! grows without limit, and an embedded profile that links a pie chart
//! it will never draw has paid `.text` for nothing (`AGENT.md`'s M15
//! gating policy) — so `alloc`-needing widgets stay behind `alloc`, and
//! a leaf profile takes neither.
//!
//! # What stayed in the core
//!
//! Not widgets: the machinery the **derive** builds. `Either`, `List`,
//! `Dyn` and `Repeat` are containers a user never places by hand — the
//! derive emits them — and `lumen_static::ops` keeps the layout helpers
//! derive-generated code calls (`inner_limits`, `resolve_self`,
//! `record_painted`). A widget is something you put in a tree; those are
//! how a tree is built.
//!
//! # Everything here is a leaf
//!
//! Each widget in this first tranche is `NODE_COUNT == 1`: it occupies
//! one node and paints itself from primitives. That is a deliberate
//! starting point rather than a coincidence, and it is the half of a
//! widget set that a static tree makes *cheaper* than a dynamic one —
//! no children, no sub-table splicing, no depth. Composites — tabs, an
//! accordion, a split pane — are the other half and cost more, because
//! in this architecture a container's shape is part of its type.
//!
//! # What a widget owes
//!
//! The same contract the core's widgets meet, and the two that are easy
//! to miss:
//!
//! * **`defaults` must be visible with no stylesheet.** A control whose
//!   default styling draws nothing is not a default — `Toggle`'s module
//!   note records how that shipped once, with an "on" switch whose knob
//!   was the transparent default.
//! * **A value field is a presentation copy, not the truth.** The
//!   application owns the state and `#[bind]` writes it here; a widget
//!   that mutated its own copy would let the two disagree.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

// **`alloc`, for the one widget that holds a growable string.**
// `NumberInput` owns a `TextInput<String>`, because a number field's
// text is edited and cannot be `&'static str`. Everything else in this
// crate is allocation-free and stays so; the module is feature-gated, so
// an embedded profile that does not take `alloc` does not get it.
#[cfg(feature = "alloc")]
extern crate alloc;

// **The feature seam fails the build** — M48/T48.9 (R12-F3,
// `docs/plan-agent-truth.md` §2.4). Every widget's `observe` impl is
// gated on THIS crate's `observe`, not the core's, so a build that turned
// on the core's (every agent build does) without this one's got a working
// agent that reported every widget here as `GenericContainer` with no
// text, and no warning. This fires in exactly the unified graph that has
// that defect. `scripts/check_feature_seam.sh` shows it both ways.
const _: () = assert!(
    !lumen_static::OBSERVE || cfg!(feature = "observe"),
    "lumen-static has `observe` on and lumen-static-widgets does not, so every widget would \
     report `GenericContainer` to the agent: enable `lumen-static-widgets/observe` (M48/T48.9)"
);
// **The same seam, one level down** — M54/T54.1. Every widget's
// `Node::observe` is gated on THIS crate's `a11y` since the split, so a
// screen-reader build that turned on the core's `a11y` without this one's
// would announce every widget here as `GenericContainer`.
// `scripts/check_feature_seam.sh` shows it both ways, for both features.
const _: () = assert!(
    !lumen_static::A11Y || cfg!(feature = "a11y"),
    "lumen-static has `a11y` on and lumen-static-widgets does not, so every widget would \
     report `GenericContainer` to a screen reader: enable `lumen-static-widgets/a11y` (M54/T54.1)"
);

// ---- M30: the widgets that used to live in the core ------------------
//
// `lumen-static` kept a "cannot be written without" tier — a label, a
// button, a field, a scroller — and this crate kept the next one. The
// split cost every application two dependencies to draw a form and put
// the seam in a place no user could predict: `Label` in core, `Badge`
// here. One crate owns widgets now.
//
// What stayed in core is the machinery the *derive* builds, not
// anything a user places: `Either`, `List`, `Dyn` and `Repeat`.
pub mod glass;
#[cfg(feature = "reload")]
pub mod reload;
pub mod image;
pub(crate) mod edit;
mod caret;
pub mod fit;
pub mod input;
pub mod labelled;
pub mod ops;
pub mod popup;
pub mod scroll;
// `select` alone of the moved ten needs `alloc`: its rows are a
// `lumen_static::repeat::Repeat`, which core gates the same way.
#[cfg(feature = "alloc")]
pub mod select;
// M80/T80.4: read-only selection holds a cluster list, so it is behind
// `alloc` with `Select` and `RichText`, and the leaf firmware links none.
#[cfg(feature = "alloc")]
pub mod selectable;
pub mod slider;
pub mod textarea;
pub mod toggle;

mod chart;
#[cfg(feature = "alloc")]
pub mod choose;
#[cfg(feature = "alloc")]
mod data;
#[cfg(feature = "alloc")]
mod field;
mod indicator;
mod nav;
#[cfg(feature = "alloc")]
pub mod pick;
#[cfg(feature = "alloc")]
pub mod rich;
pub mod split;
pub mod surface;
// M82/T82.8: a run drawn as outlines keeps them in a `Vec`, so it is behind
// `alloc` with `RichText`, and the leaf firmware links none of it.
#[cfg(feature = "alloc")]
pub mod vector;
// M83/T83.6: a node a custom shader draws, behind `shader` (owner decision
// D3), which turns the core's bracket on.
#[cfg(feature = "shader")]
pub mod shader;
mod trim;
mod veil;
// M84/T84.8: a picture a feed keeps replacing, with transport controls --
// controlled, decoding nothing (`docs/plan-images-in-motion.md`).
#[cfg(feature = "std")]
pub mod video;

pub use glass::Glass;
pub use image::{Fit, Image};
pub use input::TextInput;
pub use labelled::Labelled;
pub use ops::{Button, Fixed, Label, Spacer};
pub use popup::{Anchor, Popup};
pub use scroll::ScrollView;
#[cfg(feature = "alloc")]
pub use select::Select;
#[cfg(feature = "alloc")]
pub use selectable::Selectable;
pub use slider::Slider;
pub use textarea::TextArea;
pub use toggle::{CheckBox, Radio, Switch, Toggle};

pub use chart::{fmt_num, nice_scale, BarChart, Chart, LineChart, PieChart, Series, MAX_POINTS};
#[cfg(feature = "alloc")]
pub use choose::{
    Chooser, Combobox, Menu, OptionList, PickList, COMBOBOX, MAX_OPTIONS, MENU, PICKLIST,
};
#[cfg(feature = "alloc")]
pub use data::{
    Branch, DataGrid, PageLabel, PageSlot, Pagination, Tree, MAX_COLUMNS, MAX_PAGES, MAX_TREE_NODES,
};
#[cfg(feature = "alloc")]
pub use field::{NumberInput, RangeSlider, SearchField};
#[cfg(feature = "alloc")]
pub use rich::{
    Markdown, PaintBox, RichText, RichTextEditor, Span, MAX_EDITOR_NODES, MAX_SPANS,
};
#[cfg(feature = "alloc")]
pub use pick::{
    ColorPicker, DatePicker, FilePicker, Pad, Picker, TimePicker, COLOR, DATE, FILE, MAX_FILES,
    MAX_PICKER_NODES, SWATCHES, TIME,
};
pub use nav::{AppBar, BottomNav, PullToRefresh, Item, NavigationRail, Strip, Tabs, MAX_ITEMS, BOTTOM, RAIL, TABS};
pub use indicator::{ProgressBar, Skeleton, Spinner};
pub use split::{Collapsible, PaneGrid, SplitPane, VSplitPane, MAX_SPLIT_NODES};
pub use surface::{Card, Pin, Toast, Tooltip, MAX_SURFACE_NODES};
#[cfg(feature = "alloc")]
pub use vector::VectorText;
#[cfg(feature = "shader")]
pub use shader::ShaderView;
pub use veil::{Drawer, Edge, Modal, Sheet, Veil, DRAWER, MODAL, SHEET};
pub use trim::{Avatar, Badge, Chip, Divider, Icon, Symbol};
#[cfg(feature = "std")]
pub use video::{seek_fraction, SeekTrack, Video};

/// Everything in this crate, for a `use` that keeps up.
pub mod prelude {
    // M30: the former core widgets. An application that used
    // `lumen_static::prelude::*` for these now uses this one — and gets
    // the rest of the set with them, which is the point.
    pub use crate::{
        Button, CheckBox, Fixed, Glass, Image, Label, Popup, Radio, ScrollView, Slider, Spacer,
        Labelled, Switch, TextArea, TextInput, Toggle,
    };
    #[cfg(feature = "alloc")]
    pub use crate::{Select, Selectable};

    pub use crate::{
        Avatar, Badge, BarChart, Card, Chip, Divider, Icon, LineChart, PieChart, ProgressBar,
        AppBar, BottomNav, Collapsible, Drawer, Modal, NavigationRail, PullToRefresh, Series,
        PaneGrid, Pin, Sheet, SplitPane,
        Skeleton, Spinner, Symbol,
        Tabs, Toast, Tooltip,
    };
    #[cfg(feature = "alloc")]
    pub use crate::{
        Branch, ColorPicker, Combobox, DataGrid, DatePicker, FilePicker, Markdown, Menu,
        NumberInput, OptionList, PaintBox, Pagination, PickList, RangeSlider, RichText,
        RichTextEditor, SearchField, TimePicker, Tree, VectorText,
    };
    #[cfg(feature = "shader")]
    pub use crate::ShaderView;
    #[cfg(feature = "std")]
    pub use crate::Video;
}
