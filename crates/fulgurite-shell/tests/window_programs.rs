//! **M89/T89.3: a program's window table** — `docs/plan-windows.md`,
//! "A window program is a table the program carries".
//!
//! `Program::window_program(n)` answers the program window `n` runs, from
//! the table `Program::windows` set; `OpenProgram::session` boots it into
//! the session a window drives. The window crate calls only these, so a
//! table that answered the wrong program would open the wrong window
//! everywhere.
//!
//! | claim | bound |
//! |---|---|
//! | entry 1 boots the detail: its session's `title()` | `"detail"` |
//! | an index the table has no entry for | `None` (9 and 0) |
//! | the entry's `WindowSpec` is its own | 300×200, its agent address kept to be refused |

use fulgurite::prelude::*;
use fulgurite_shell::program::{application, OpenProgram, Program};
use fulgurite_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct OneUi {
    shown: Label,
}

/// `Reloadable`, as every window program is in a `reload` build
/// (`WindowApp`, M89/T89.7); expands to nothing without it.
#[derive(Default, fulgurite::Reloadable)]
struct One {
    #[reload(fresh)]
    ui: OneUi,
}

impl App for One {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = OneUi;
    type Msg = ();
    fn tree(&self) -> &OneUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut OneUi {
        &mut self.ui
    }
    fn apply(_: &mut Self, _: ()) -> bool {
        false
    }
}

fn detail() -> Program<One> {
    application(One::default).title("detail").size(300, 200).agent("127.0.0.1:7999")
}

fn table(index: u16) -> Option<Box<dyn OpenProgram>> {
    match index {
        1 => Some(Box::new(detail())),
        _ => None,
    }
}

fn list() -> Program<One> {
    application(One::default).title("list").windows(table)
}

#[test]
fn the_table_boots_the_entry_it_names_and_no_other() {
    let p = list();
    let mut one = p.window_program(1).expect("entry 1 is the detail");
    let spec = one.spec();
    let session = one.session(false);
    let title = session.title();
    let nine = p.window_program(9).is_some();
    let zero = p.window_program(0).is_some();
    println!(
        "window_program(1): title {title:?} (want \"detail\"), spec {}x{}, agent {:?}; \
         window_program(9) is_some {nine} (want false); window_program(0) is_some {zero} (want false)",
        spec.width, spec.height, spec.agent
    );
    assert_eq!(title, "detail", "entry 1 booted another program");
    assert_eq!((spec.width, spec.height), (300, 200), "the entry's WindowSpec is not its own");
    assert_eq!(spec.agent.as_deref(), Some("127.0.0.1:7999"), "the entry's address must reach the window to be refused");
    assert!(!nine, "an index the table has no entry for answered a program");
    assert!(!zero, "0 is the program itself, never a table entry");
    // Both recovery policies boot (the cost recorded in docs/results-windows.md).
    assert_eq!(p.window_program(1).unwrap().session(true).title(), "detail");
}
