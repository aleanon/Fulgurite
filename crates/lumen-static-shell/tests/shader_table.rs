//! **A program's shader table reaches every host it builds, and the
//! session the window asks** — M83/T83.4 (`docs/plan-gpu-extension.md`,
//! "Shaders are data, delivered to the backend").
//!
//! `Program::shaders` is consumed on the host side, through
//! `Program::host()` and `Host::set_shaders`; `Session::shaders` is what
//! the window hands each backend. Both are asked here, printed by name.
//! The window's half — every backend `resumed` makes is handed the table —
//! is `lumen-static-winit`'s `a_second_resume_still_has_the_images`.

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static_shell::program::application;
use lumen_static_shell::session::{Crashing, Local, Session};
use lumen_static_shell::shader::{ShaderDesc, ShaderTable};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    panel: Fixed,
}

#[derive(Default)]
struct Shaded {
    ui: Ui,
}

impl App for Shaded {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    type Tree = Ui;
    type Msg = lumen_static::event::Never;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    fn apply(_: &mut Self, m: lumen_static::event::Never) -> bool {
        match m {}
    }
}

const SOLID: &str = "@fragment fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(1.0, 0.0, 0.0, 1.0); }";
const TINT: &str = "struct U { c: vec4<f32> } @group(0) @binding(1) var<uniform> u: U; @fragment fn fs_main() -> @location(0) vec4<f32> { return u.c; }";

/// Two shaders, a `static` table: `ShaderDesc::new` is `const`.
static TWO: [ShaderDesc; 2] = [ShaderDesc::new("solid", SOLID, 0), ShaderDesc::new("tint", TINT, 16)];

fn names(t: ShaderTable<'_>) -> Vec<&str> {
    t.descs().iter().map(|d| d.name()).collect()
}

#[test]
fn the_host_and_the_session_both_hold_the_programs_table() {
    // The host every headless frame is rendered from.
    let host: Host<Shaded> = application(Shaded::default).shaders(&TWO).host();
    let h = host.shaders();
    println!("Program::shaders(&TWO).host(): {} descs {:?}", h.len(), names(h));

    // The session the window drives, booted from the same description.
    let s = Local::from_program(application(Shaded::default).shaders(&TWO), Crashing);
    let t = s.shaders();
    println!("Local::from_program(..).shaders(): {} descs {:?}", t.len(), names(t));

    assert_eq!(h.len(), 2, "M83/T83.4: `Program::host` must install the program's shader table");
    assert_eq!(names(h), ["solid", "tint"], "the host's table, in ShaderId order");
    assert_eq!(t.len(), 2, "`Session::shaders` must answer the host's table");
    assert_eq!(names(t), ["solid", "tint"], "the session's table, in ShaderId order");
    assert_eq!(t.descs()[1].uniform_size(), 16);
    assert!(t.descs()[1].wgsl().contains("@binding(1)"));
}

/// The accept direction: a program that registers none answers an empty
/// table, as a host built without `Program::host` does.
#[test]
fn a_program_without_shaders_answers_none() {
    let host: Host<Shaded> = application(Shaded::default).host();
    let bare: Host<Shaded> = Host::new(Shaded::default(), 100, 100);
    println!("no table: program host {}, bare host {}", host.shaders().len(), bare.shaders().len());
    assert!(host.shaders().is_empty());
    assert!(bare.shaders().is_empty());
    assert!(ShaderTable::EMPTY.is_empty());
}
