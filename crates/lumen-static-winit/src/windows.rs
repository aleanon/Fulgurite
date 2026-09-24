//! More than one window — `docs/plan-capabilities.md` T15.10.
//!
//! **The core learns nothing about this.** `Host` is already one tree
//! and one surface, which is the right unit: a second window is a
//! second `Host`, not a `Host` with two of anything. So multi-window is
//! bookkeeping in the windowing crate — a map from `WindowId` to the
//! things that window owns — and needs no feature, because it adds no
//! dependency and costs an application that opens one window a single
//! `Vec` with one entry in it.
//!
//! What it does *not* do is share state between windows. Two windows
//! are two applications as far as this crate is concerned; an
//! application that wants them to agree owns the shared part itself,
//! which is the same answer the single-window case gives for state
//! shared between widgets.

use std::sync::Arc;

use lumen_static::event::App;
use lumen_static::Color;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::Host;
use winit::window::{Window, WindowId};

use crate::{surface, Cursor, Surface};

/// One window and everything it owns.
///
/// **Software only.** It holds a [`Surface`] and a `Canvas` directly
/// rather than a `Box<dyn crate::Backend>`, so a second window is
/// always rasterised on the CPU even in a binary built with the `gpu`
/// feature. Nothing prevents the swap; it was not needed, and a second
/// window would be a second GPU context — 174 MB resident
/// (`docs/results-gpu.md` §5), which is a decision rather than a detail.
pub struct Pane<A: App> {
    pub host: Host<A, crate::Ret, crate::Inval>,
    pub window: Arc<Window>,
    pub surface: Surface,
    pub canvas: Canvas,
    pub metrics: Option<FontMetrics>,
    pub cursor: Cursor,
    pub page: Color,
    font: Option<FontStack>,
    cursor_shown: lumen_static::style::Cursor,
}

impl<A: App> Pane<A> {
    pub fn new(host: A, window: Arc<Window>, page: Color, font: Option<FontStack>) -> Self {
        let size = window.inner_size();
        let mut host = Host::new(host, size.width.max(1), size.height.max(1));
        let scale = window.scale_factor() as f32;
        host.set_scale(scale);
        let mut metrics = font.clone().map(FontMetrics::new);
        if let Some(m) = metrics.as_mut() {
            m.set_scale(scale);
        }
        let canvas = surface::canvas(size.width.max(1), size.height.max(1), font.as_ref());
        let surface = Surface::new(window.clone());
        Pane {
            host,
            window,
            surface,
            canvas,
            metrics,
            cursor: Cursor::default(),
            page,
            font,
            cursor_shown: lumen_static::style::Cursor::Default,
        }
    }

    /// One frame for this window.
    pub fn redraw(&mut self) {
        let size = self.window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        if surface::sync_size(&mut self.canvas, self.font.as_ref(), w, h) {
            self.host.resize(w, h);
        }
        let start = std::time::Instant::now();
        match self.metrics.as_mut() {
            Some(m) => self.host.pump_with(m),
            None => self.host.pump(),
        };
        self.surface.present(&mut self.host, &mut self.canvas, self.page, start);
        crate::cursor::apply(&self.window, self.host.cursor(), &mut self.cursor_shown);
    }
}

/// Every open window.
///
/// A `Vec` and a linear scan rather than a map: an application with
/// enough windows for a hash to pay for itself is not an application
/// this is written for, and a `Vec` keeps the common case — one window
/// — to one comparison.
pub struct Panes<A: App> {
    panes: Vec<Pane<A>>,
}

impl<A: App> Default for Panes<A> {
    fn default() -> Self {
        Panes { panes: Vec::new() }
    }
}

impl<A: App> Panes<A> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, pane: Pane<A>) {
        self.panes.push(pane);
    }

    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut Pane<A>> {
        self.panes.iter_mut().find(|p| p.window.id() == id)
    }

    /// Close one window. Returns whether anything was closed.
    ///
    /// Closing the last one does **not** exit the loop — that is the
    /// application's decision, and a program that keeps running with no
    /// window (a tray application, a daemon with a settings panel) is a
    /// real thing. [`is_empty`](Self::is_empty) is how a caller that
    /// *does* want to exit finds out.
    pub fn close(&mut self, id: WindowId) -> bool {
        let before = self.panes.len();
        self.panes.retain(|p| p.window.id() != id);
        self.panes.len() != before
    }

    pub fn len(&self) -> usize {
        self.panes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Pane<A>> {
        self.panes.iter_mut()
    }
}
