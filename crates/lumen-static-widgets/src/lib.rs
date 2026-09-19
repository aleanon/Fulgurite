//! Widgets that are **not** in the core — M23.
//!
//! `lumen-static` carries the widgets an application cannot be written
//! without: a label, a button, a field, a scroller, the containers the
//! derive builds. Everything here is the next tier — a badge, a chart, a
//! spinner — and it lives in its own crate for the reason every optional
//! thing in this workspace does: **the crate is the gate**. A widget set
//! grows without limit, and an embedded profile that links a pie chart
//! it will never draw has paid `.text` for nothing (`AGENT.md`'s M15
//! gating policy).
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

mod chart;
#[cfg(feature = "alloc")]
pub mod choose;
#[cfg(feature = "alloc")]
mod data;
#[cfg(feature = "alloc")]
mod field;
mod indicator;
mod nav;
pub mod pick;
#[cfg(feature = "alloc")]
pub mod rich;
pub mod split;
pub mod surface;
mod trim;
mod veil;

pub use chart::{BarChart, Chart, LineChart, PieChart, Series, MAX_POINTS};
#[cfg(feature = "alloc")]
pub use choose::{
    Chooser, Combobox, Menu, OptionList, PickList, COMBOBOX, MAX_OPTIONS, MENU, PICKLIST,
};
#[cfg(feature = "alloc")]
pub use data::{
    Branch, DataGrid, PageLabel, Pagination, Tree, MAX_COLUMNS, MAX_PAGES, MAX_TREE_NODES,
};
#[cfg(feature = "alloc")]
pub use field::{NumberInput, RangeSlider, SearchField};
#[cfg(feature = "alloc")]
pub use rich::{
    Markdown, PaintBox, RichText, RichTextEditor, Span, MAX_EDITOR_NODES, MAX_SPANS,
};
pub use pick::{
    ColorPicker, DatePicker, FilePicker, Pad, Picker, TimePicker, COLOR, DATE, FILE, MAX_FILES,
    MAX_PICKER_NODES, SWATCHES, TIME,
};
pub use nav::{AppBar, BottomNav, PullToRefresh, Item, NavigationRail, Strip, Tabs, MAX_ITEMS, BOTTOM, RAIL, TABS};
pub use indicator::{ProgressBar, Skeleton, Spinner};
pub use split::{Collapsible, PaneGrid, SplitPane, MAX_SPLIT_NODES};
pub use surface::{Card, Toast, Tooltip, MAX_SURFACE_NODES};
pub use veil::{Drawer, Modal, Sheet, Veil, DRAWER, MODAL, SHEET};
pub use trim::{Avatar, Badge, Chip, Divider, Icon, Symbol};

/// Everything in this crate, for a `use` that keeps up.
pub mod prelude {
    pub use crate::{
        Avatar, Badge, BarChart, Card, Chip, Divider, Icon, LineChart, PieChart, ProgressBar,
        AppBar, BottomNav, Collapsible, Drawer, Modal, NavigationRail, PullToRefresh, Series,
        PaneGrid, Sheet, SplitPane,
        Skeleton, Spinner, Symbol,
        Tabs, Toast, Tooltip,
    };
    #[cfg(feature = "alloc")]
    pub use crate::{
        Branch, ColorPicker, Combobox, DataGrid, DatePicker, FilePicker, Markdown, Menu,
        NumberInput, OptionList, PaintBox, Pagination, PickList, RangeSlider, RichText,
        RichTextEditor, SearchField, TimePicker, Tree,
    };
}
