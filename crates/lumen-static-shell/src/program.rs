//! `application(boot)`: a program described once, and built from that
//! description everywhere — M32 (`docs/plan-program.md`).
//!
//! **The shape is iced's**: a boot function, then settings that are
//! either constants or functions of the state, then `run`. It lives in
//! the shell rather than in `lumen-static-winit` for two reasons the plan
//! records in §2.4:
//!
//! - [`Program::host`](crate::program::Program::host) builds the `Host` a window then drives, and a
//!   shell test can call it without a window system. That is what makes
//!   the one-path gate (`tests/program.rs`) able to go through the same
//!   code the window does.
//! - M37's hot library builds a description of itself, and must not have
//!   to link winit to do it.
//!
//! # Why this exists
//!
//! M31 collapsed *construction* to one path and left *styling* with two:
//! `Application::style` for the headless host and a reload, and
//! `Config::stylesheet` for the window. Edit one and the other silently
//! won. A description has exactly one place for each answer, and every
//! host is built from it — the window's, every `--shot`, and a reload,
//! which re-asks [`Sheet`](crate::program::Sheet) through [`Host::adopt`](crate::Host::adopt).
//!
//! # Boot is cold
//!
//! The boot function runs when a program starts, and **it may capture**:
//! showcase opens on an entry read from argv. The prototype refused a
//! capturing closure and presented the refusal as a result; that refusal
//! was protecting a *reload* constructor, and boot is not one. A reload
//! builds through code resolved from the build that is loaded now, never
//! through a pointer held from before the swap
//! (`docs/events-decision.md` rule 3).

use std::borrow::Cow;
use std::rc::Rc;

use lumen_static::display::Color;
use lumen_static::event::App;
use lumen_static::invalidate::Invalidation;
use lumen_static::retain::Retention;
use lumen_static::spawn::Job;
use lumen_static::style::{Palette, Rule};

use crate::Host;

/// What a boot function may return: the application, or the
/// application and the jobs it wants started before the first frame —
/// iced's `(State, Task)`.
pub trait IntoBoot<A: App> {
    fn into_boot(self) -> (A, Vec<Job<A::Msg>>);
}

impl<A: App> IntoBoot<A> for A {
    fn into_boot(self) -> (A, Vec<Job<A::Msg>>) {
        (self, Vec::new())
    }
}

impl<A: App> IntoBoot<A> for (A, Vec<Job<A::Msg>>) {
    fn into_boot(self) -> (A, Vec<Job<A::Msg>>) {
        self
    }
}

/// A window title: a constant, or a function of the state.
///
/// The two impls cannot overlap because `&'static str` is not `Fn` —
/// the trick iced's title function uses.
pub trait Title<A> {
    fn title(&self, app: &A) -> Cow<'static, str>;
}

impl<A> Title<A> for &'static str {
    fn title(&self, _: &A) -> Cow<'static, str> {
        Cow::Borrowed(self)
    }
}

impl<A, F: Fn(&A) -> String> Title<A> for F {
    fn title(&self, app: &A) -> Cow<'static, str> {
        Cow::Owned(self(app))
    }
}

/// A stylesheet: a constant, or a function of the state.
///
/// **Owned, not borrowed** (`docs/plan-program.md` §2.2). The borrowed
/// `Stylesheet<'_>` made every application with a computed sheet keep
/// it in a field, and its reason — the core trait must not need `alloc`
/// — does not apply here: this is std-only, and a sheet is asked for
/// when a host is built and when it adopts a new application, never per
/// frame.
pub trait Sheet<A> {
    fn sheet(&self, app: &A) -> Vec<Rule>;
}

impl<A> Sheet<A> for &'static [Rule] {
    fn sheet(&self, _: &A) -> Vec<Rule> {
        self.to_vec()
    }
}

impl<A, F: Fn(&A) -> Vec<Rule>> Sheet<A> for F {
    fn sheet(&self, app: &A) -> Vec<Rule> {
        self(app)
    }
}

/// Which renderer a window should try for.
///
/// Here rather than in `lumen-static-winit` because it is part of the
/// description, and the description must be buildable without a window
/// system; winit re-exports it unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Prefer {
    /// GPU when the window crate was built with the `gpu` feature *and*
    /// an adapter exists; the software canvas otherwise. The default,
    /// because falling back is the whole point: a machine with no
    /// driver, a container with no `/dev/dri` and a remote session all
    /// have to keep working.
    #[default]
    Auto,
    /// The software canvas, whatever hardware is present.
    Software,
    /// The GPU, and **still fall back** if there is none. A refusal to
    /// open a window would be a worse answer than a slower one.
    Gpu,
}

impl Prefer {
    /// `LUMEN_BACKEND=software` / `=gpu`, overriding the application.
    ///
    /// An escape hatch for the person running the program rather than
    /// the person who wrote it: a driver bug on one machine should not
    /// need a rebuild to work around. An unrecognised value is ignored
    /// rather than fatal.
    pub fn from_env(default: Prefer) -> Prefer {
        match std::env::var("LUMEN_BACKEND").as_deref() {
            Ok("software") | Ok("cpu") => Prefer::Software,
            Ok("gpu") | Ok("wgpu") => Prefer::Gpu,
            _ => default,
        }
    }
}

/// A boot function, erased: the application and the jobs to start.
type Boot<A> = Box<dyn Fn() -> (A, Vec<Job<<A as App>::Msg>>)>;

/// A program, described: how to build it and what its window looks
/// like. Built by [`application`].
pub struct Program<A: App> {
    boot: Boot<A>,
    title: Box<dyn Title<A>>,
    sheet: Option<Rc<dyn Sheet<A>>>,
    /// Window size in logical pixels.
    pub width: u32,
    pub height: u32,
    /// Cleared behind every damaged region before it is redrawn.
    pub page: Color,
    /// The palette a stylesheet's roles resolve against (M25/T25.11).
    pub palette: Palette,
    pub backend: Prefer,
    /// Where `Cmd::Image` gets its pixels. Taken once, by the window.
    pub images: Option<Box<dyn lumen_static::ImageSource>>,
    /// Where to serve the agent endpoint, when the window crate was
    /// built with its `agent` feature. Inert otherwise.
    pub agent: Option<String>,
    /// The application's source directory, from `application`'s caller —
    /// M36: a panic report's application frame is found under it.
    source_root: Option<&'static str>,
}

/// Start describing a program from the function that builds it.
///
/// ```ignore
/// application(Counter::build)            // an `fn` item
/// application(|| build(14.0))            // a closure
/// application(move || build(start, false)) // …which may capture
/// ```
#[track_caller]
pub fn application<A, F, C>(boot: F) -> Program<A>
where
    A: App,
    F: Fn() -> C + 'static,
    C: IntoBoot<A>,
{
    // The caller's file, as the compiler spelled it (`fixtures/x/src/
    // main.rs`): the crate's directory is everything before `/src/`.
    let file = std::panic::Location::caller().file();
    let source_root = file.find("/src/").map(|i| &file[..i]);
    Program {
        source_root,
        boot: Box::new(move || boot().into_boot()),
        title: Box::new("lumen-static"),
        sheet: None,
        width: 640,
        height: 480,
        page: Color(255, 255, 255, 255),
        palette: Palette::LIGHT,
        backend: Prefer::Auto,
        images: None,
        agent: None,
    }
}

impl<A: App> Program<A> {
    pub fn title(mut self, title: impl Title<A> + 'static) -> Self {
        self.title = Box::new(title);
        self
    }

    /// The program's stylesheet — the **only** place one comes from.
    pub fn style(mut self, sheet: impl Sheet<A> + 'static) -> Self {
        self.sheet = Some(Rc::new(sheet));
        self
    }

    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn page(mut self, page: Color) -> Self {
        self.page = page;
        self
    }

    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    pub fn backend(mut self, prefer: Prefer) -> Self {
        self.backend = prefer;
        self
    }

    pub fn images(mut self, images: impl lumen_static::ImageSource + 'static) -> Self {
        self.images = Some(Box::new(images));
        self
    }

    pub fn agent(mut self, addr: impl Into<String>) -> Self {
        self.agent = Some(addr.into());
        self
    }

    /// The window title for this state.
    pub fn title_of(&self, app: &A) -> Cow<'static, str> {
        self.title.title(app)
    }

    /// **The host a window drives, and the host every headless frame is
    /// rendered from** — the same function, so a `--shot` compares the
    /// frame the window draws (`docs/plan-program.md` rule 2).
    ///
    /// Boots the application, sets the palette, installs the sheet and
    /// keeps its source so [`Host::adopt`] can re-ask it. Jobs the boot
    /// returned are held until a spawner is installed
    /// ([`Host::set_spawner`]), because a headless host has none and the
    /// window installs its own after this returns.
    pub fn host<R: Retention, I: Invalidation>(&self) -> Host<A, R, I> {
        let (app, jobs) = (self.boot)();
        let mut host = Host::new(app, self.width, self.height);
        host.set_palette(self.palette);
        if let Some(sheet) = &self.sheet {
            host.set_sheet_source(Rc::clone(sheet));
        }
        if let Some(root) = self.source_root {
            host.set_source_root(root);
        }
        for job in jobs {
            host.hold_until_spawner(job);
        }
        host
    }
}
