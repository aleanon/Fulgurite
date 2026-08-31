//! `Repeat<T>` — the repeater from handoff §6: one main-table entry, a
//! const template, a runtime instance count, and only the visible window
//! materialized.
//!
//! The template is *not* spliced into the parent's table. It already
//! exists once as `T::SUB_TABLE`/`T::KIND_LIST`; the repeater's ops run
//! the ordinary flat passes over it per visible instance, with instance
//! values and `NodeState`s in a growable side arena behind a `RefCell`
//! (the pass API hands ops a shared base pointer). This is what makes the
//! §6 scaling property structural:
//!
//! > `NODE_COUNT` grows with how much UI you **wrote**, not how much data
//! > you **display**.
//!
//! Model notes (Step 3 scope, see `docs/step3-repeaters.md`):
//! - uniform `item_extent` along the vertical axis (the standard
//!   virtualization simplification; measured extents are later work);
//! - the repeater is its own scrollport: `scroll` lives on the widget and
//!   its height constraint is the viewport; its reported size is the full
//!   logical extent clamped by the viewport;
//! - `bind` is a plain `fn` pointer, not a closure — the §7 "events are
//!   data" rule applied ahead of time;
//! - rows sliding out of the window lose their instance state (recycled);
//!   the §12.4 lifecycle question stays open.

use core::cell::RefCell;

use crate::desc::{leaf_nop, Constraints, NodeDesc, NodeState, Size, WidgetOps, NO_PARENT};
use crate::ops::fold;
use crate::pass;
use crate::widget::Widget;

pub fn noop_bind<T>(_: &mut T, _: usize) {}

struct Inner<T> {
    window: Vec<T>,
    states: Vec<NodeState>,
    first: usize,
}

pub struct Repeat<T: Widget + Default + 'static> {
    /// Logical row count — the data size. Never reflected in any table.
    pub count: usize,
    /// Scroll offset in px along the main (vertical) axis.
    pub scroll: f32,
    /// Uniform per-row extent along the main axis.
    pub item_extent: f32,
    /// Configure a recycled template instance for a row.
    pub bind: fn(&mut T, usize),
    inner: RefCell<Inner<T>>,
}

impl<T: Widget + Default> Default for Repeat<T> {
    fn default() -> Self {
        Self::new(0, 1.0, noop_bind::<T>)
    }
}

impl<T: Widget + Default> Repeat<T> {
    pub fn new(count: usize, item_extent: f32, bind: fn(&mut T, usize)) -> Self {
        Repeat {
            count,
            scroll: 0.0,
            item_extent,
            bind,
            inner: RefCell::new(Inner { window: Vec::new(), states: Vec::new(), first: 0 }),
        }
    }

    /// Materialized instances (visible window), for tests/inspection.
    pub fn window_len(&self) -> usize {
        self.inner.borrow().window.len()
    }

    /// First materialized row.
    pub fn first_row(&self) -> usize {
        self.inner.borrow().first
    }

    /// Copy of instance `k`'s state for template node `node`.
    pub fn instance_state(&self, k: usize, node: usize) -> NodeState {
        self.inner.borrow().states[k * T::NODE_COUNT + node]
    }

    fn visible_range(&self, viewport_h: f32) -> (usize, usize) {
        if self.count == 0 || self.item_extent <= 0.0 {
            return (0, 0);
        }
        let first = ((self.scroll / self.item_extent).floor().max(0.0) as usize)
            .min(self.count - 1);
        let vis = ((viewport_h / self.item_extent).ceil() as usize + 1).min(self.count - first);
        (first, vis)
    }

    fn up_op(b: *const u8, t: &[NodeDesc], st: &[NodeState], i: usize) -> Size {
        let r: &Self = unsafe { &*(b.add(t[i].offset as usize) as *const Self) };
        let c = st[i].constraints;
        let (first, vis) = r.visible_range(c.max_h);

        let mut inner = r.inner.borrow_mut();
        inner.first = first;
        inner.window.resize_with(vis, T::default);
        inner.states.resize(vis * T::NODE_COUNT, NodeState::default());
        for (k, item) in inner.window.iter_mut().enumerate() {
            (r.bind)(item, first + k);
        }

        // Lay out each visible instance with the ordinary flat passes over
        // the template table; positions come out relative to the instance
        // origin and are shifted in `place_op`.
        let row_c = Constraints { min_w: 0.0, max_w: c.max_w, min_h: 0.0, max_h: r.item_extent };
        let Inner { window, states, .. } = &mut *inner;
        for (k, item) in window.iter().enumerate() {
            let ibase = item as *const T as *const u8;
            let ist = &mut states[k * T::NODE_COUNT..(k + 1) * T::NODE_COUNT];
            pass::layout(ibase, T::SUB_TABLE, T::KIND_LIST, ist, row_c);
        }

        c.clamp(Size { w: c.max_w, h: r.count as f32 * r.item_extent })
    }

    fn place_op(b: *const u8, t: &[NodeDesc], st: &mut [NodeState], i: usize) {
        let r: &Self = unsafe { &*(b.add(t[i].offset as usize) as *const Self) };
        let origin = st[i].pos;
        let mut inner = r.inner.borrow_mut();
        let first = inner.first;
        let extent = r.item_extent;
        let scroll = r.scroll;
        for k in 0..inner.window.len() {
            let dy = (first + k) as f32 * extent - scroll;
            for s in &mut inner.states[k * T::NODE_COUNT..(k + 1) * T::NODE_COUNT] {
                s.pos.x += origin.x;
                s.pos.y += origin.y + dy;
            }
        }
    }

    fn paint_op(b: *const u8, d: &NodeDesc, st: &NodeState, out: &mut u64) {
        let r: &Self = unsafe { &*(b.add(d.offset as usize) as *const Self) };
        fold(out, d.kind as u64);
        fold(out, st.size.w.to_bits() as u64 ^ ((st.size.h.to_bits() as u64) << 32));
        fold(out, r.count as u64 ^ ((r.scroll.to_bits() as u64) << 32));

        let inner = r.inner.borrow();
        fold(out, inner.first as u64);
        for (k, item) in inner.window.iter().enumerate() {
            let ibase = item as *const T as *const u8;
            for (j, td) in T::SUB_TABLE.iter().enumerate() {
                let s = &inner.states[k * T::NODE_COUNT + j];
                (T::KIND_LIST[td.kind as usize].paint)(ibase, td, s, out);
            }
        }
    }

    /// Map a point to the row and template node under it (plan E1). The
    /// row comes from the window's laid-out instance states, so it is
    /// recomputed every frame — which is exactly why a handler never
    /// needs to capture an index (`docs/events-decision.md`).
    fn hit_op(
        b: *const u8,
        d: &NodeDesc,
        _st: &NodeState,
        x: f32,
        y: f32,
    ) -> Option<crate::desc::Inner> {
        let r: &Self = unsafe { &*(b.add(d.offset as usize) as *const Self) };
        let inner = r.inner.borrow();
        for k in 0..inner.window.len() {
            let ist = &inner.states[k * T::NODE_COUNT..(k + 1) * T::NODE_COUNT];
            if let Some(node) = crate::hit::deepest(T::SUB_TABLE, ist, x, y) {
                return Some(crate::desc::Inner {
                    row: (inner.first + k) as u64,
                    node,
                    origin: ist[node as usize].pos,
                });
            }
        }
        None
    }

    /// Re-dispatch into the hit instance: handlers declared on template
    /// widgets run with `ctx.row` filled in.
    fn event_op(
        b: *const u8,
        state: *mut u8,
        d: &NodeDesc,
        _st: &NodeState,
        ev: &crate::event::Event,
        ctx: &crate::event::EventCtx,
    ) -> bool {
        let Some(row) = ctx.row else { return false };
        let r: &Self = unsafe { &*(b.add(d.offset as usize) as *const Self) };
        // Copy what is needed out of the borrow before any handler runs:
        // a handler may mutate the app state, and the repeater lives in it.
        let (ibase, ist, mut node) = {
            let inner = r.inner.borrow();
            let Some(k) = (row as usize).checked_sub(inner.first) else { return false };
            if k >= inner.window.len() {
                return false;
            }
            let ibase = &inner.window[k] as *const T as *const u8;
            let ist = inner.states[k * T::NODE_COUNT..(k + 1) * T::NODE_COUNT].to_vec();
            (ibase, ist, ctx.inner as usize)
        };
        // Bubble within the template: leaf → template root.
        loop {
            let td = &T::SUB_TABLE[node];
            let mut ictx = *ctx;
            ictx.inner = node as u32;
            if (T::KIND_LIST[td.kind as usize].event)(ibase, state, td, &ist[node], ev, &ictx) {
                return true;
            }
            if td.parent == NO_PARENT {
                return false;
            }
            node = td.parent as usize;
        }
    }

    pub const OPS: WidgetOps = {
        let ops = WidgetOps::new("repeat", leaf_nop, Self::up_op, Self::place_op, Self::paint_op)
            .with_hit(Self::hit_op)
            .with_event(Self::event_op);
        #[cfg(feature = "observe")]
        let ops = ops.with_observe(Self::observe_op).with_stats(Self::stats_op);
        ops
    };

    /// Rows and materialized window — what distinguishes a legitimately
    /// empty list from a broken one, which geometry alone cannot show.
    #[cfg(feature = "observe")]
    fn stats_op(b: *const u8, d: &NodeDesc, out: &mut crate::desc::NodeStats) {
        let r: &Self = unsafe { &*(b.add(d.offset as usize) as *const Self) };
        out.items = Some(r.count as u64);
        out.materialized = Some(r.inner.borrow().window.len() as u32);
    }

    /// Role `List`; pushes each visible instance's template subtree with
    /// row-stable synthetic ids and links their roots as children.
    /// Id encoding (documented limits: repeater table index < 2^19,
    /// template nodes < 2^12, rows < 2^32, one level of nesting):
    /// `1<<63 | my_id<<44 | row<<12 | template_node`.
    #[cfg(feature = "observe")]
    fn observe_op(
        b: *const u8,
        d: &NodeDesc,
        _st: &NodeState,
        my_id: u64,
        node: &mut accesskit::Node,
        extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        let r: &Self = unsafe { &*(b.add(d.offset as usize) as *const Self) };
        node.set_role(accesskit::Role::List);
        node.set_size_of_set(r.count);
        let inner = r.inner.borrow();
        for k in 0..inner.window.len() {
            let row = (inner.first + k) as u64;
            let id_of = move |tnode: usize| -> u64 {
                (1u64 << 63) | (my_id << 44) | (row << 12) | tnode as u64
            };
            let ibase = &inner.window[k] as *const T as *const u8;
            let ist = &inner.states[k * T::NODE_COUNT..(k + 1) * T::NODE_COUNT];
            crate::observe::emit_subtree(ibase, T::SUB_TABLE, T::KIND_LIST, ist, &id_of, extra);
            node.push_child(accesskit::NodeId(id_of(0)));
        }
    }
}

impl<T: Widget + Default + 'static> Widget for Repeat<T> {
    const NODE_COUNT: usize = 1;
    const KIND_COUNT: usize = 1;
    const IS_RIGID: bool = false;
    /// One entry: no table children (the template is not in this table),
    /// but CONTAINER so the place phase dispatches the instance shift.
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, 0, 0, 0, NO_PARENT, 0, NodeDesc::CONTAINER)];
    const KIND_LIST: &'static [&'static WidgetOps] = &[&Self::OPS];
}
