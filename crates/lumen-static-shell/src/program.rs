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
use lumen_static::{Key, Mods};

use crate::Host;

/// What an application's key hook did with a key no widget consumed —
/// M60/T60.1 (`docs/plan-tally-slim.md` §2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Not the application's key. Nothing is marked, and the pump does
    /// not count it as handled.
    Ignored,
    /// The application acted on it. Counted as handled, and marked as a
    /// handled event is: a split application's bindings mark the nodes
    /// they write, an unsplit one's whole tree is marked.
    Handled,
    /// The application asks to close. [`Host::quit_requested`] turns
    /// true, and a window exits at its next `about_to_wait`; a headless
    /// host only records it. Counted as handled, and marks nothing.
    Quit,
}

/// An application's hook for a key press no widget consumed —
/// [`Program::on_key`]. It gets what a handler gets, the state, and the
/// modifiers held with the key.
pub type KeyHook<A> = fn(&mut <A as App>::State, Key, Mods) -> KeyOutcome;

/// A stylesheet key, type-erased: whether its value changed since it was
/// last asked — [`Program::style_by`], M60/T60.2. The first call answers
/// `true`, and records the value.
pub trait SheetKey<A> {
    fn changed(&mut self, app: &A) -> bool;
}

/// [`SheetKey`] over a key function and the value it last returned.
struct Keyed<A, K> {
    key: fn(&A) -> K,
    last: Option<K>,
}

impl<A, K: PartialEq + Copy> SheetKey<A> for Keyed<A, K> {
    fn changed(&mut self, app: &A) -> bool {
        let k = (self.key)(app);
        let changed = self.last != Some(k);
        self.last = Some(k);
        changed
    }
}

/// Makes a fresh [`SheetKey`] for each host a program builds.
type SheetKeyMaker<A> = Rc<dyn Fn() -> Box<dyn SheetKey<A>>>;

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
    /// a GPU adapter exists; the software canvas otherwise. The default,
    /// because falling back is the whole point: a machine with no
    /// driver, a container with no `/dev/dri` and a remote session all
    /// have to keep working.
    ///
    /// **A CPU rasteriser is not a GPU adapter here** (M51/T51.2,
    /// R9-F6). On a machine with Mesa and no working driver, wgpu's only
    /// adapter is llvmpipe; `Auto` used to take it, and the software
    /// canvas is several times faster there (`docs/results-gpu.md` §6).
    #[default]
    Auto,
    /// The software canvas, whatever hardware is present.
    Software,
    /// The GPU, and **still fall back** if there is none. A refusal to
    /// open a window would be a worse answer than a slower one. Unlike
    /// `Auto`, this takes a CPU rasteriser such as llvmpipe when that is
    /// the only adapter, because the person asked for this one.
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

/// Which invalidation strategy a window's host runs, when the
/// application does not say — the default of [`Program`]'s `I`, and of
/// the shell's `Local` and `Live` sessions, as it is [`Host`]'s.
///
/// **A type the application chooses, not a feature** — M55/T55.4
/// (R12-F5). Until then the Inval alias was this type unless the shell's
/// `everything` feature was on, and cargo unifies features across a build
/// graph: one crate turning the diagnostic on changed the type of every
/// public `host` field for every other crate, and plain `Host<A>` — the
/// most natural spelling, since `Host`'s default is also
/// `Boundaries<256>` — stopped compiling (E0308). The diagnostic is now
/// `Program::invalidation::<Everything>()`, in the application's own code.
/// 256 marks because `Boundaries<N>` stores ranges, not nodes, and
/// M26/T26.5 fixed it wrapping its length at exactly that count.
pub type DefaultInval = lumen_static::Boundaries<256>;

/// A boot function, erased: the application and the jobs to start.
///
/// Shared, because the host keeps it (M49/T49.4): `Host::reload` (behind `reload`)
/// rebuilds through it, as a window swap rebuilds through the new
/// library's `program()`, so there is one construction path.
pub type Boot<A> = Rc<dyn Fn() -> (A, Vec<Job<<A as App>::Msg>>)>;

/// A program, described: how to build it and what its window looks
/// like. Built by [`application`].
///
/// `I` is the invalidation strategy the program's **window** runs
/// (M55/T55.4, R12-F5): `Boundaries<256>` unless
/// [`Program::invalidation`] chose another. A type the application
/// chooses in its own code, never a feature, so nothing else in the build
/// graph can change it. [`Program::host`] stays generic over its own
/// strategy, so a headless frame or a test may pick either.
pub struct Program<A: App, I: Invalidation = DefaultInval> {
    boot: Boot<A>,
    title: Box<dyn Title<A>>,
    sheet: Option<Rc<dyn Sheet<A>>>,
    /// The key [`Program::style_by`] re-asks the sheet on, when it was
    /// used; `None` for a sheet that changes only on `adopt`.
    sheet_key: Option<SheetKeyMaker<A>>,
    /// [`Program::on_key`]'s hook.
    on_key: Option<KeyHook<A>>,
    /// Window size in logical pixels.
    pub width: u32,
    pub height: u32,
    /// Cleared behind every damaged region before it is redrawn.
    pub page: Color,
    /// The palette a stylesheet's roles resolve against (M25/T25.11).
    pub palette: Palette,
    pub backend: Prefer,
    /// Where `Cmd::Image` gets its pixels. Moved into the window's
    /// `Config` by `run`, which hands it to each backend it makes and
    /// takes it back on suspend (M47/T47.7), so one source serves every
    /// resume.
    pub images: Option<Box<dyn lumen_static::ImageSource>>,
    /// Where to serve the agent endpoint, when the window crate was
    /// built with its `agent` feature. Inert otherwise.
    pub agent: Option<String>,
    /// Whether the window hands the platform an accessibility tree — M54/T54.2.
    /// Defaults to whether the window crate was built with `a11y`, which
    /// is on by default for desktop; `false` turns the adapter off at run
    /// time without a rebuild. Inert in a build without the feature.
    pub accessible: bool,
    /// The application's source directory, from `application`'s caller —
    /// M36: a panic report's application frame is found under it.
    source_root: Option<&'static str>,
    strategy: core::marker::PhantomData<fn() -> I>,
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
        boot: Rc::new(move || boot().into_boot()),
        title: Box::new("lumen-static"),
        sheet: None,
        sheet_key: None,
        on_key: None,
        width: 640,
        height: 480,
        page: Color(255, 255, 255, 255),
        palette: Palette::LIGHT,
        backend: Prefer::Auto,
        images: None,
        agent: None,
        strategy: core::marker::PhantomData,
        accessible: cfg!(feature = "a11y"),
    }
}

impl<A: App, I: Invalidation> Program<A, I> {
    /// Run the window under the strategy `J` instead — M55/T55.4.
    ///
    /// `application(boot).invalidation::<Everything>()` is the
    /// diagnostic that the `everything` feature used to be: if a stale
    /// frame goes away under it, an invalidation defect has been found.
    /// An application that builds both keeps its own feature and makes
    /// this call under it, which changes only its own code.
    pub fn invalidation<J: Invalidation>(self) -> Program<A, J> {
        Program {
            boot: self.boot,
            title: self.title,
            sheet: self.sheet,
            sheet_key: self.sheet_key,
            on_key: self.on_key,
            width: self.width,
            height: self.height,
            page: self.page,
            palette: self.palette,
            backend: self.backend,
            images: self.images,
            agent: self.agent,
            source_root: self.source_root,
            strategy: core::marker::PhantomData,
            accessible: self.accessible,
        }
    }

    pub fn title(mut self, title: impl Title<A> + 'static) -> Self {
        self.title = Box::new(title);
        self
    }

    /// The program's stylesheet — the **only** place one comes from.
    pub fn style(mut self, sheet: impl Sheet<A> + 'static) -> Self {
        self.sheet = Some(Rc::new(sheet));
        self.sheet_key = None;
        self
    }

    /// **A stylesheet that is a function of a key of the state** —
    /// M60/T60.2 (`docs/plan-tally-slim.md` §2.2).
    ///
    /// `sheet(key(app))` is the program's sheet, re-asked where
    /// [`Program::style`]'s is (`adopt`, `restore`, `reload`) — and
    /// **also whenever `key`'s value changes**: every host this program
    /// builds compares the key after `App::update` and before layout, in
    /// every pump, so the frame that lands a change of key is the frame
    /// drawn with its sheet. The window, `host()` and the agent all pump
    /// through `Host::pump_with`, so there is one path.
    ///
    /// `key` should be cheap and coarse: the tally's is the count's sign,
    /// three values for every count there is. A key that changes every
    /// frame re-cascades every frame.
    pub fn style_by<K: PartialEq + Copy + 'static>(mut self, key: fn(&A) -> K, sheet: fn(K) -> Vec<Rule>) -> Self
    where
        A: 'static,
    {
        self.sheet = Some(Rc::new(move |a: &A| sheet(key(a))));
        self.sheet_key = Some(Rc::new(move || Box::new(Keyed { key, last: None })));
        self
    }

    /// **A key press no widget consumed** goes to `hook` — M60/T60.1
    /// (`docs/plan-tally-slim.md` §2.1).
    ///
    /// Called from `Host::pump_with` for a key *press* that dispatch did
    /// not consume — so not Tab, not an arrow that moved focus within a
    /// group or that a focused text field owns, not an Escape that closed
    /// an overlay. The window, a headless `host()` and the agent's
    /// `input.key` all reach it through that one pump. What it returns is
    /// a [`KeyOutcome`].
    pub fn on_key(mut self, hook: KeyHook<A>) -> Self {
        self.on_key = Some(hook);
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

    /// Where a window built with the `agent` feature serves its endpoint
    /// **by default**: `LUMEN_AGENT_ADDR` in the environment overrides it,
    /// read by the window alone (M48/T48.11). An application does not read
    /// the variable itself.
    pub fn agent(mut self, addr: impl Into<String>) -> Self {
        self.agent = Some(addr.into());
        self
    }

    /// Whether a screen reader can see this window — M54/T54.2
    /// (`docs/plan-a11y-platform.md` §2.2). On by default wherever the
    /// window crate has `a11y`; `false` builds no adapter.
    pub fn accessible(mut self, on: bool) -> Self {
        self.accessible = on;
        self
    }

    /// The application's source directory as the compiler spelled it
    /// (`examples/x`, relative to where cargo ran) — M36 finds panic
    /// frames under it, and M37's window rebuilds the crate there.
    pub fn source_root(&self) -> Option<&'static str> {
        self.source_root
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
    /// keeps its source so [`Host::adopt`] can re-ask it, and keeps the
    /// boot so `Host::reload` can rebuild through it (M49/T49.4). Jobs the boot
    /// returned are held until a spawner is installed
    /// ([`Host::set_spawner`]), because a headless host has none and the
    /// window installs its own after this returns.
    pub fn host<R: Retention, J: Invalidation>(&self) -> Host<A, R, J> {
        let (app, jobs) = (self.boot)();
        let mut host = Host::new(app, self.width, self.height);
        host.set_boot(Rc::clone(&self.boot));
        host.set_palette(self.palette);
        if let Some(sheet) = &self.sheet {
            host.set_sheet_source(Rc::clone(sheet));
        }
        if let Some(make) = &self.sheet_key {
            host.set_sheet_key(make());
        }
        if let Some(hook) = self.on_key {
            host.set_on_key(hook);
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
