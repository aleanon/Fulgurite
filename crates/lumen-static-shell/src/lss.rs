//! **Reading a `.lss` sheet at run time** — M61/T61.5
//! (`docs/plan-lss.md` §2.4).
//!
//! The same parser and lowering `include_lss!` runs at build time
//! (`lumen_static_lss::compile`), turned into [`Rule`](lumen_static::style::Rule) *values* here
//! instead of `const` tokens — two independent lowerings of one IR, held
//! to each other by T61.5's parity gate (every `.lss` in the repository,
//! `include_lss!` and [`load`](crate::lss::load) equal; `scripts/check_lss_coverage.sh`
//! refuses a `.lss` with no such test).
//!
//! It is what lets a colour change reach a running window without a
//! rebuild: `Program::live_lss` reads the program's file, and again when
//! its modification time changes. **The window calls that in debug and
//! `reload` builds only**; a release build without `reload` never names
//! anything here, and the linker drops it (kill rule K3: no
//! `lumen_static_lss` symbol, `scripts/check_lss_release.sh`). The gate is
//! the window crate's and not a `cfg` in this module, because the
//! workspace's dev profile builds this crate without debug assertions —
//! see `Program::live_lss`.
//!
//! **Selectors are interned and leaked.** A `Rule`'s selector is a
//! `&'static str`, and a sheet read at run time has no `'static` text to
//! point into. Each distinct id, class and kind is leaked once and reused
//! (`intern`), so what a session leaks is bounded by the distinct names
//! it ever styled — not by how many times the file was saved. Recorded,
//! not hidden (§2.4).

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use lumen_static::display::Color;
use lumen_static::style::{Align, Cursor, Edges, Length, Patch, Role, Rule, Selector};
use lumen_static_lss::ast::Part;
use lumen_static_lss::lower::{self, Val};

/// Why a sheet could not be read: every error, as `file:line:col: message`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LssError {
    pub messages: Vec<String>,
}

impl core::fmt::Display for LssError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.messages.join("\n"))
    }
}

impl std::error::Error for LssError {}

/// Read and lower the sheet at `path`: its top-level rules, then those of
/// `@sheet block` when one is named — what `include_lss!(Tree, path,
/// block)` emits, as values.
///
/// **No tree checks.** `include_lss!` asserts every `#id` and `.class`
/// against the tree at compile time; a sheet read here was checked when
/// the build that ships it compiled, and between builds `W1010` reports
/// what matches nothing.
pub fn load(path: impl AsRef<Path>, block: Option<&str>) -> Result<Vec<Rule>, LssError> {
    let path = path.as_ref();
    let name = path.display().to_string();
    let src = std::fs::read_to_string(path)
        .map_err(|e| LssError { messages: vec![format!("{name}: cannot read: {e}")] })?;
    load_str(&name, &src, block)
}

/// [`load`], from text already in hand. `file` is only for the messages.
pub fn load_str(file: &str, src: &str, block: Option<&str>) -> Result<Vec<Rule>, LssError> {
    let sheet = lumen_static_lss::compile(src)
        .map_err(|errors| LssError { messages: errors.iter().map(|e| e.at(file)).collect() })?;
    let rules = sheet.select(block).map_err(|m| LssError { messages: vec![format!("{file}: {m}")] })?;
    Ok(rules.into_iter().map(rule).collect())
}

fn rule(r: &lower::Rule) -> Rule {
    let selector = match &r.selector {
        Part::Id(s) => Selector::Id(intern(s)),
        Part::Class(s) => Selector::Class(intern(s)),
        Part::Kind(s) => Selector::Kind(intern(s)),
        Part::Any => Selector::Any,
    };
    let patch = r.setters.iter().fold(Patch::new(), |p, s| set(p, s.method, s.value));
    Rule::new(selector, patch)
}

/// One setter call, by name — the run-time twin of the macro's
/// `.#method(#value)`. An unknown pairing is a bug in this function or in
/// the table, and `scripts/check_lss_setters.sh` plus the parity gate are
/// what would show it; it panics rather than drop a declaration.
fn set(p: Patch, method: &str, v: Val) -> Patch {
    match (method, v) {
        ("width", Val::Length(l)) => p.width(length(l)),
        ("height", Val::Length(l)) => p.height(length(l)),
        ("padding", Val::Edges(e)) => p.padding(Edges { left: e.left, right: e.right, top: e.top, bottom: e.bottom }),
        ("spacing", Val::F32(x)) => p.spacing(x),
        ("align", Val::Align(a)) => p.align(align(a)),
        ("visible", Val::Bool(b)) => p.visible(b),
        ("cursor", Val::Cursor(c)) => p.cursor(cursor(c)),
        ("focus_ring", Val::U8(x)) => p.focus_ring(x),
        ("color", Val::Color(c)) => p.color(colour(c)),
        ("color_role", Val::Role(r)) => p.color_role(role(r)),
        ("background", Val::Color(c)) => p.background(colour(c)),
        ("background_role", Val::Role(r)) => p.background_role(role(r)),
        ("border_width", Val::U8(x)) => p.border_width(x),
        ("border_color", Val::Color(c)) => p.border_color(colour(c)),
        ("border_color_role", Val::Role(r)) => p.border_color_role(role(r)),
        ("radius", Val::F32(x)) => p.radius(x),
        ("font_size", Val::F32(x)) => p.font_size(x),
        ("font_weight", Val::F32(x)) => p.font_weight(x),
        ("shadow", Val::F32(x)) => p.shadow(x),
        ("pressed_dim", Val::F32(x)) => p.pressed_dim(x),
        ("hover_dim", Val::F32(x)) => p.hover_dim(x),
        (m, v) => panic!("lumen_static_shell::lss: no setter `{m}` taking {v:?}"),
    }
}

fn colour([r, g, b, a]: [u8; 4]) -> Color {
    Color(r, g, b, a)
}

fn length(l: lower::Length) -> Length {
    match l {
        lower::Length::Fixed(x) => Length::Fixed(x),
        lower::Length::Shrink => Length::Shrink,
        lower::Length::FillPortion(k) => Length::FillPortion(k),
    }
}

fn align(a: lower::Align) -> Align {
    match a {
        lower::Align::Start => Align::Start,
        lower::Align::Center => Align::Center,
        lower::Align::End => Align::End,
        lower::Align::Fill => Align::Fill,
    }
}

fn cursor(c: lower::Cursor) -> Cursor {
    match c {
        lower::Cursor::Default => Cursor::Default,
        lower::Cursor::Text => Cursor::Text,
        lower::Cursor::Pointer => Cursor::Pointer,
        lower::Cursor::Grab => Cursor::Grab,
        lower::Cursor::Grabbing => Cursor::Grabbing,
        lower::Cursor::ColResize => Cursor::ColResize,
        lower::Cursor::RowResize => Cursor::RowResize,
        lower::Cursor::NotAllowed => Cursor::NotAllowed,
    }
}

fn role(r: lower::Role) -> Role {
    match r {
        lower::Role::Page => Role::Page,
        lower::Role::Surface => Role::Surface,
        lower::Role::Control => Role::Control,
        lower::Role::Text => Role::Text,
        lower::Role::Muted => Role::Muted,
        lower::Role::Accent => Role::Accent,
        lower::Role::OnAccent => Role::OnAccent,
        lower::Role::Focus => Role::Focus,
    }
}

/// `s` as a `&'static str`, leaked the first time it is seen and reused
/// after — see the module's note on what that bounds.
fn intern(s: &str) -> &'static str {
    static NAMES: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut names = NAMES.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&kept) = names.get(s) {
        return kept;
    }
    let kept: &'static str = Box::leak(s.to_owned().into_boxed_str());
    names.insert(kept);
    kept
}

/// A file's modification time, `None` when it cannot be read — the key
/// `Program::style_lss` re-reads the sheet on.
pub(crate) fn modified(path: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}
