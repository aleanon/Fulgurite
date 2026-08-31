//! Memory footprint: unsafe base+offset path vs safe `&dyn` accessor.
//!
//! Splits **static** (const tables and kind lists — `.rodata`, which on a
//! microcontroller lives in flash) from **dynamic** (per-frame state and
//! any resolved index — RAM, the scarce resource on that profile).
//!
//! Run: `cargo run --release -p benches --bin memory`

use benches::{Block, Card, SafeCard, SafeTen, Ten};
use lumen_static::assemble::Assembled;
use lumen_static::desc::{NodeDesc, NodeState};
use lumen_static::safe::{self, Node, Slab, Tree};
use lumen_static::Widget;

/// What `NodeDesc` shrinks to once the safe path makes `kind` and
/// `offset` dead: neither is read when the vtable rides on the `&dyn`
/// and addressing is by index.
#[allow(dead_code)]
struct SlimDesc {
    first_child: u32,
    parent: u32,
    next_sibling: u32,
    child_count: u16,
    flags: u8,
}

fn row(label: &str, n: usize, desc: usize, kinds: usize, index: usize) {
    let state = n * size_of::<NodeState>();
    let stat = n * desc + kinds;
    println!(
        "{label:<22} {n:>7} {:>9} {:>9} {:>9} {:>9} {:>8}",
        n * desc,
        kinds,
        state,
        index,
        stat + state + index
    );
}

fn main() {
    println!("type sizes (release build, no observe/reload):");
    println!("  NodeDesc          {:>3} B   (offset+kind carried for the unsafe path)", size_of::<NodeDesc>());
    println!("  SlimDesc          {:>3} B   (what the safe path needs: no kind, no offset)", size_of::<SlimDesc>());
    println!("  NodeState         {:>3} B", size_of::<NodeState>());
    println!("  &dyn Node         {:>3} B   (fat pointer, if the index is cached)", size_of::<&dyn Node>());
    println!("  &'static WidgetOps{:>3} B   (one KIND_LIST slot per node — see below)", size_of::<&'static lumen_static::WidgetOps>());

    println!("\nkind lists are NOT deduplicated (docs/step2-design-notes.md §7):");
    for (n, nc, kc) in [
        ("Ten", Ten::NODE_COUNT, Ten::KIND_COUNT),
        ("Card", Card::NODE_COUNT, Card::KIND_COUNT),
        ("Block", Block::NODE_COUNT, Block::KIND_COUNT),
    ] {
        println!("  {n:<6} NODE_COUNT {nc:>3}   KIND_COUNT {kc:>3}   -> {:.1} kind slots/node", kc as f32 / nc as f32);
    }

    println!("\nper-tree footprint (bytes)");
    println!("{:<22} {:>7} {:>9} {:>9} {:>9} {:>9} {:>8}", "arm", "nodes", "table", "kinds", "state", "index", "total");

    // --- const tree: table and kinds are .rodata (flash on embedded) ---
    let d = size_of::<NodeDesc>();
    let slim = size_of::<SlimDesc>();
    let kp = size_of::<&'static lumen_static::WidgetOps>();
    row("10/unsafe", Ten::NODE_COUNT, d, Ten::KIND_COUNT * kp, 0);
    row("10/safe", SafeTen::COUNT, slim, 0, 0);
    row("10/safe+index", SafeTen::COUNT, slim, 0, SafeTen::COUNT * 16);

    // --- 100k runtime slab: table becomes a heap Vec ---
    let n = 1 + 4545 * Card::NODE_COUNT;
    row("100k/unsafe", n, d, (1 + Card::KIND_COUNT) * kp, 0);
    row("100k/safe", n, slim, 0, 0);
    row("100k/safe+index", n, slim, 0, n * 16);

    println!("\nmeasured, not computed (100k slab, actual allocations):");
    let a = Assembled::<Card>::new(4545);
    println!(
        "  unsafe: table {:>9} B   kinds {:>7} B   state {:>9} B",
        std::mem::size_of_val(a.table.as_slice()),
        std::mem::size_of_val(a.kinds.as_slice()),
        std::mem::size_of_val(a.states.as_slice()),
    );
    let slab = Slab::<SafeCard>::new(4545);
    let st = vec![NodeState::default(); slab.table().len()];
    let index = safe::resolve_slab(&slab);
    println!(
        "  safe:   table {:>9} B   kinds {:>7} B   state {:>9} B   index {:>9} B",
        std::mem::size_of_val(slab.table()),
        0,
        std::mem::size_of_val(st.as_slice()),
        std::mem::size_of_val(index.as_slice()),
    );
    println!(
        "  (the safe table still carries kind+offset in this prototype; \
         slim would be {} B)",
        slab.table().len() * slim
    );

    println!("\nRAM vs flash, embedded reading (const tree, N nodes):");
    println!("  unsafe: flash {} B/node (table+kinds)   RAM {} B/node (state)", d + kp, size_of::<NodeState>());
    println!("  safe:   flash {} B/node (table)         RAM {} B/node (state)", slim, size_of::<NodeState>());
    println!("  safe+index: as above, plus 16 B/node RAM for the cached index");
}
