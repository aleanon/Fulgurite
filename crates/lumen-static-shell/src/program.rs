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

/// Where a program's `.lss` sheet can be read from at run time, kept by
/// [`Program::style_lss`] and [`Program::style_lss_by`] until
/// [`Program::live_lss`] uses it — M61/T61.5. **Data only**: nothing
/// here names the reader, so a program whose window never goes live links
/// none of it (kill rule K3).
struct LssSource<A> {
    path: &'static str,
    /// The `@sheet` for this state, if any.
    block: BlockOf<A>,
    /// The sheet this build compiled in for this state — drawn with when
    /// the file cannot be read or does not lower.
    compiled: CompiledOf<A>,
}

/// Which `@sheet` a state reads, if any — [`LssSource::block`].
type BlockOf<A> = Rc<dyn Fn(&A) -> Option<&'static str>>;
/// The sheet a build compiled in, for a state — [`LssSource::compiled`].
type CompiledOf<A> = Rc<dyn Fn(&A) -> &'static [Rule]>;

/// [`SheetKey`] for a live `.lss`: the program's own key, **and the
/// file's modification time**, so saving the file is a change of key like
/// any other and reaches the frame through the one pump every host runs.
struct MtimeKey<A> {
    inner: Option<Box<dyn SheetKey<A>>>,
    path: &'static str,
    last: Option<Option<std::time::SystemTime>>,
}

impl<A> SheetKey<A> for MtimeKey<A> {
    fn changed(&mut self, app: &A) -> bool {
        // Both asked every time: a key that short-circuited would stop
        // recording the other's value.
        let inner = self.inner.as_mut().is_some_and(|k| k.changed(app));
        let now = crate::lss::modified(self.path);
        let file = self.last != Some(now);
        self.last = Some(now);
        inner || file
    }
}

/// The sheet at `path`, or — when it cannot be read or does not lower —
/// the one this build compiled in, with the errors on stderr.
fn read_lss(path: &str, block: Option<&str>, compiled: &'static [Rule]) -> Vec<Rule> {
    match crate::lss::load(path, block) {
        Ok(rules) => rules,
        Err(e) => {
            eprintln!("lumen-static: {e}\n  (drawing with the sheet this build compiled in)");
            compiled.to_vec()
        }
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
    /// Where [`Program::live_lss`] reads the sheet from, when it came
    /// from a `.lss` (M61/T61.5).
    lss: Option<LssSource<A>>,
    /// Window size in logical pixels.
    pub width: u32,
    pub height: u32,
    /// Cleared behind every damaged region before it is redrawn.
    pub page: Color,
    /// The palette a stylesheet's roles resolve against (M25/T25.11).
    pub palette: Palette,
    /// The fill table a node's `Style::background_fill` indexes —
    /// M82/T82.3 ([`Program::fills`]).
    pub fills: &'static [lumen_static::style::Fill],
    pub backend: Prefer,
    /// Where `Cmd::Image` gets its pixels. Moved into the window's
    /// `Config` by `run`, which hands it to each backend it makes and
    /// takes it back on suspend (M47/T47.7), so one source serves every
    /// resume.
    pub images: Option<Box<dyn lumen_static::ImageSource>>,
    /// [`Program::backend_with`]'s factory — M83/T83.2. Moved into the
    /// window's `Config` by `run`, once; never read by a hot swap.
    #[cfg(feature = "truetype")]
    pub backend_with: Option<crate::backend::BackendFactory>,
    /// [`Program::shaders`]' table — M83/T83.4. Installed in every host
    /// [`Program::host`] builds.
    #[cfg(feature = "shader")]
    pub shaders: &'static [crate::shader::ShaderDesc],
    /// Where to serve the agent endpoint, when the window crate was
    /// built with its `agent` feature. Inert otherwise.
    pub agent: Option<String>,
    /// Whether the window hands the platform an accessibility tree — M54/T54.2.
    /// Defaults to whether the window crate was built with `a11y`, which
    /// is on by default for desktop; `false` turns the adapter off at run
    /// time without a rebuild. Inert in a build without the feature.
    pub accessible: bool,
    /// How the window, its session's measurer, the agent's audit and every
    /// headless frame of this program lay text out — M81/T81.4
    /// (`docs/plan-shaped-paint.md`, "Choosing is a `Program` setting").
    /// `Basic` unless [`Program::shaping`] chose `Complex`. Behind
    /// `truetype`, as [`Shaping`](crate::text::Shaping) is: a build with
    /// no face has no stack to choose for.
    #[cfg(feature = "truetype")]
    pub shaping: crate::text::Shaping,
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
        lss: None,
        width: 640,
        height: 480,
        page: Color(255, 255, 255, 255),
        palette: Palette::LIGHT,
        fills: &[],
        backend: Prefer::Auto,
        images: None,
        #[cfg(feature = "truetype")]
        backend_with: None,
        #[cfg(feature = "shader")]
        shaders: &[],
        agent: None,
        strategy: core::marker::PhantomData,
        accessible: cfg!(feature = "a11y"),
        #[cfg(feature = "truetype")]
        shaping: crate::text::Shaping::Basic,
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
            lss: self.lss,
            width: self.width,
            height: self.height,
            page: self.page,
            palette: self.palette,
            fills: self.fills,
            backend: self.backend,
            images: self.images,
            #[cfg(feature = "truetype")]
            backend_with: self.backend_with,
            #[cfg(feature = "shader")]
            shaders: self.shaders,
            agent: self.agent,
            source_root: self.source_root,
            strategy: core::marker::PhantomData,
            accessible: self.accessible,
            #[cfg(feature = "truetype")]
            shaping: self.shaping,
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
        self.lss = None;
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
        self.lss = None;
        self
    }

    /// **A `.lss` sheet: compiled in, and live where the window says so**
    /// — M61/T61.5 (`docs/plan-lss.md` §2.4).
    ///
    /// `compiled` is `include_lss!`'s output and `path` the same file
    /// (`concat!(env!("CARGO_MANIFEST_DIR"), "/app.lss")`). As written it
    /// is [`Program::style`]`(compiled)` and nothing more; the path is
    /// kept for [`Program::live_lss`], which the window calls in a debug
    /// build and in a `reload` build, and which reads the file instead —
    /// again whenever it changes. A release build without `reload` never
    /// calls it, so no part of the `.lss` reader is linked (kill rule K3,
    /// `scripts/check_lss_release.sh`).
    ///
    /// **Why the compiled sheet is an argument**, when the plan's §2.4
    /// named only the path: a program must build in release, where there
    /// is no reader, so the release answer has to be in the call.
    pub fn style_lss(mut self, compiled: &'static [Rule], path: &'static str) -> Self
    where
        A: 'static,
    {
        self = self.style(compiled);
        self.lss = Some(LssSource { path, block: Rc::new(|_: &A| None), compiled: Rc::new(move |_: &A| compiled) });
        self
    }

    /// [`Program::style_lss`] keyed on the state, as [`Program::style_by`]
    /// is: `compiled(key(app))` is the sheet, and a live program reads the
    /// `@sheet` named `block(key(app))` from `path` instead. `compiled`
    /// and `block` must agree — `include_lss!(T, "app.lss", pos)` for the
    /// key `block` maps to `"pos"` — and the tally's parity test is what
    /// holds its pair to that (M61/T61.6).
    pub fn style_lss_by<K: PartialEq + Copy + 'static>(
        mut self,
        key: fn(&A) -> K,
        compiled: fn(K) -> &'static [Rule],
        path: &'static str,
        block: fn(K) -> Option<&'static str>,
    ) -> Self
    where
        A: 'static,
    {
        self.sheet = Some(Rc::new(move |a: &A| compiled(key(a)).to_vec()));
        self.sheet_key = Some(Rc::new(move || Box::new(Keyed { key, last: None })));
        self.lss = Some(LssSource {
            path,
            block: Rc::new(move |a: &A| block(key(a))),
            compiled: Rc::new(move |a: &A| compiled(key(a))),
        });
        self
    }

    /// **Read the program's `.lss` at run time, and again when it
    /// changes** — M61/T61.5. A no-op for a sheet that did not come from
    /// [`Program::style_lss`] or [`Program::style_lss_by`].
    ///
    /// The sheet becomes the file's, re-read whenever the program's key
    /// or the file's modification time changes — checked where
    /// [`Program::style_by`]'s key is, in every pump, so a saved colour
    /// reaches the next frame with no rebuild. A file that cannot be read
    /// or does not lower draws with the compiled sheet and says why on
    /// stderr.
    ///
    /// **Called by the window** (`lumen-static-winit`'s `run`) in debug
    /// builds and `reload` builds — not decided here, because the
    /// workspace's dev profile builds this crate with `debug-assertions =
    /// false`, and a `cfg(debug_assertions)` in this crate is false in
    /// every debug build. `recover`'s module note records the same trap,
    /// met first by M36. A headless host a test builds stays on the
    /// compiled sheet unless the test calls this.
    pub fn live_lss(mut self) -> Self
    where
        A: 'static,
    {
        if let Some(LssSource { path, block, compiled }) = self.lss.take() {
            self.sheet = Some(Rc::new(move |a: &A| read_lss(path, block(a), compiled(a))));
            let inner = self.sheet_key.take();
            self.sheet_key = Some(Rc::new(move || {
                Box::new(MtimeKey { inner: inner.as_ref().map(|m| m()), path, last: None }) as Box<dyn SheetKey<A>>
            }));
        }
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

    /// **The fill table** — M82/T82.3 (`docs/plan-paint-vocabulary.md`):
    /// a node whose `Style::background_fill` is `n` paints entry `n - 1`
    /// as a gradient under its background. Role-named stops
    /// ([`Fill::role`](lumen_static::style::Fill::role)) resolve against
    /// the palette and follow `Host::set_palette`.
    ///
    /// Consumed on the host side, by [`Program::host`] through
    /// [`Host::set_fills`], so every headless frame paints it and a hot
    /// swap re-asks the new build's table; never through the window's
    /// once-read `Config`.
    pub fn fills(mut self, table: &'static [lumen_static::style::Fill]) -> Self {
        self.fills = table;
        self
    }

    pub fn backend(mut self, prefer: Prefer) -> Self {
        self.backend = prefer;
        self
    }

    /// **The window's image store** — window-side, read **once**: `run`
    /// moves it into the window's config and hands it to each backend it
    /// makes (taken back on suspend, M47/T47.7). A renderer consults it
    /// *after* the frame's store, `App::images`, which a session hands over
    /// on every present (M84/T84.6, `crate::image::ImageChain`), so an id
    /// both hold draws the application's pixels.
    ///
    /// Because it is read once, a hot build's edits to it never reach the
    /// running window — it is the loader's. Pixels that must follow a swap,
    /// or change while the program runs, belong in the application's store.
    pub fn images(mut self, images: impl lumen_static::ImageSource + 'static) -> Self {
        self.images = Some(Box::new(images));
        self
    }

    /// **Build this program's window backend yourself** — M83/T83.2
    /// (`docs/plan-gpu-extension.md`, "The factory").
    ///
    /// `factory` is called with the window's
    /// [`BackendTarget`](crate::backend::BackendTarget) — raw handles,
    /// physical size, the session's face and `accept_cpu` — **before** the
    /// built-in choice, on every `resumed` (once per window, and again
    /// after a suspend), in a build with or without the window crate's
    /// `gpu`. `None` falls through to the GPU-then-software choice, so the
    /// software fallback stands.
    ///
    /// Where each setting is consumed:
    ///
    /// | setting | read | across a hot swap |
    /// |---|---|---|
    /// | [`Program::backend`] | once, into the window's `Config` | the loader's |
    /// | [`Program::images`] | once; handed to each backend, taken back on suspend | the loader's |
    /// | `backend_with` | once, into `Config`; called per `resumed` | the loader's; **a hot library's is ignored** |
    ///
    /// - **`Prefer::Software` skips it** — from [`Program::backend`] or
    ///   from `LUMEN_BACKEND=software` — so the escape hatch for a driver
    ///   bug works however the application was written.
    /// - **Not reloadable.** The loader reads the static binary's program
    ///   once and a swap never re-reads the window's `Config`, so the
    ///   closure is loader code and never a pointer into a dylib. Changing
    ///   it needs a restart.
    /// - A backend it returns is a forwarding wrapper as often as not, so
    ///   every `Backend` method that acts on what it holds is required.
    ///
    /// Behind the shell's `truetype`, as the `backend` module is.
    #[cfg(feature = "truetype")]
    pub fn backend_with(
        mut self,
        factory: impl FnMut(&crate::backend::BackendTarget) -> Option<Box<dyn crate::backend::Backend>> + 'static,
    ) -> Self {
        self.backend_with = Some(Box::new(factory));
        self
    }

    /// **The program's custom shaders** — M83/T83.4
    /// (`docs/plan-gpu-extension.md`, "Shaders are data, delivered to the
    /// backend"). Entry `n` is `ShaderId(n)` in a `Cmd::Shader` bracket;
    /// the table is fixed for a build, as an image id is.
    ///
    /// Consumed on the host side, like [`Program::fills`]:
    ///
    /// | setting | side | read | across a hot swap |
    /// |---|---|---|---|
    /// | `shaders` | host | [`Program::host`], every build, through [`Host::set_shaders`] | re-asked through `Session::shaders` and re-sent to the backend |
    ///
    /// The window hands the table to every backend it makes and again
    /// after every successful swap (`Backend::set_shaders`); **no borrow
    /// of it survives that call** — a backend copies what it keeps
    /// ([`crate::shader`]). Behind the shell's `shader` feature.
    #[cfg(feature = "shader")]
    pub fn shaders(mut self, table: &'static [crate::shader::ShaderDesc]) -> Self {
        self.shaders = table;
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

    /// **How this program's text is laid out and painted** — M81/T81.4
    /// (`docs/plan-shaped-paint.md`). `Shaping::Complex` (the shell's
    /// `shaping` feature) shapes every run that needs it — right-to-left
    /// order, joining, marks — in the measure, the agent's audit **and the
    /// paint**; `Basic`, the default, is fontdue's left-to-right layout.
    ///
    /// A window setting, so it lives here (M32's rule): `Local::from_program`
    /// builds its stack with it, the window reads it back through
    /// `Session::shaping` **before its first backend is built** and again
    /// after every swap, and `LUMEN_SHOT` and a session's
    /// `Offscreen::with_shaping` render with it — so the frame a window
    /// draws, the box its host measured and every headless render agree.
    /// Chosen per program, not per widget: per-widget shaping is reopened
    /// by K81a, not built (`docs/plan-shaped-paint.md`, "Not in M81").
    #[cfg(feature = "truetype")]
    pub fn shaping(mut self, shaping: crate::text::Shaping) -> Self {
        self.shaping = shaping;
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
    /// Boots the application, sets the palette, the fill table and the
    /// shader table (M83/T83.4), installs the sheet and
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
        host.set_fills(self.fills);
        #[cfg(feature = "shader")]
        host.set_shaders(self.shaders);
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
