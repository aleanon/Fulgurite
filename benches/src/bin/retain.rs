//! **The measurement that decides M13/T13.8**: streaming retention
//! against the list-copying implementation it replaced.
//!
//! T13.8's motivation is that damage must work on a host with no
//! display list — but the plan set a gate rather than a licence: the
//! streaming form hashes every command where the copying form compared
//! stored ones, so it may be a wash, and *"keep the copying
//! implementation if streaming is not faster."*
//!
//! Commands are issued directly against a `Painter` rather than by
//! painting a tree, for two reasons: it is the only way to reach the
//! ~2 000-command frame the plan asks about, and it keeps the tree walk
//! — which both forms pay identically — out of the number.
//!
//! The copying implementation is reproduced here from the version this
//! task replaced (`retain.rs` before 2026-09-05): the old code no
//! longer exists to link against, and a like-for-like number is the
//! whole point.

use benches::bench;
use lumen_static::display::{Cmd, Color, DisplayList, Painter};
use lumen_static::retain::{Damage, Incremental, Rebuild, Recording, Retention, Streaming};

// ---- the copying implementation, as it was ---------------------------

fn copy_list(dst: &mut DisplayList, src: &DisplayList) {
    dst.cmds.clear();
    dst.cmds.extend_from_slice(&src.cmds);
    dst.text.clear();
    dst.text.push_str(&src.text);
    // M82/T82.5: a path's elements live in the list too.
    dst.paths.clear();
    dst.paths.extend_from_slice(&src.paths);
}

fn ink(c: &Cmd) -> [f32; 4] {
    match c {
        Cmd::Shadow { rect, spread, .. } => {
            [rect[0] - spread, rect[1] - spread, rect[2] + spread, rect[3] + spread]
        }
        Cmd::Rect { rect, .. }
        | Cmd::RoundedRect { rect, .. }
        | Cmd::StrokedRect { rect, .. }
        | Cmd::Text { rect, .. }
        | Cmd::Image { rect, .. }
        | Cmd::Gradient { rect, .. }
        | Cmd::Glass { rect, .. }
        | Cmd::Path { rect, .. }
        | Cmd::PushClip { rect } => *rect,
        Cmd::Line { a, b, .. } => [a.x, a.y, b.x, b.y],
        Cmd::PopClip => [0.0; 4],
        // M83/T83.3: a bracket inks its rect, as `Recording::shader` does;
        // its end, like a pop, has no geometry. `cfg(lumen_shader)` from
        // this crate's `build.rs`, never its own feature.
        #[cfg(lumen_shader)]
        Cmd::Shader { rect, .. } => *rect,
        #[cfg(lumen_shader)]
        Cmd::ShaderEnd => [0.0; 4],
    }
}

fn text_differs(prev: &DisplayList, now: &DisplayList, i: usize, a: &Cmd) -> bool {
    let Cmd::Text { .. } = a else { return false };
    let (Cmd::Text { text: ra, .. }, Cmd::Text { text: rb, .. }) = (&prev.cmds[i], &now.cmds[i])
    else {
        return false;
    };
    prev.str_of(*ra) != now.str_of(*rb)
}

fn diff(prev: &DisplayList, now: &DisplayList) -> Damage {
    if prev.cmds.len() != now.cmds.len() {
        return Damage::All;
    }
    let mut d = Damage::None;
    for (i, (a, b)) in prev.cmds.iter().zip(now.cmds.iter()).enumerate() {
        if a == b && !text_differs(prev, now, i, a) {
            continue;
        }
        d = d.union(Damage::rect(ink(a))).union(Damage::rect(ink(b)));
        if d == Damage::All {
            break;
        }
    }
    d
}

// ---- a synthetic frame ------------------------------------------------

/// One frame of `n` commands: the mix a real tree paints — a background,
/// a rounded control, a text run, a clip scope.
fn emit(p: &mut dyn Painter, n: usize) {
    for i in 0..n {
        let x = (i % 40) as f32 * 20.0;
        let y = (i / 40) as f32 * 24.0;
        let r = [x, y, x + 18.0, y + 22.0];
        match i % 4 {
            0 => p.rect(r, Color(30, 30, 40, 255)),
            1 => p.rounded_rect(r, Color(90, 120, 200, 255), 4.0),
            2 => p.text(r, "label", 14.0, Color(240, 240, 240, 255)),
            _ => {
                p.push_clip(r);
                p.pop_clip();
            }
        }
    }
}

/// Label texts of the lengths a real tree shows — a button, a count, a
/// menu row, a sentence — cycled through. M46/T46.1's retention half of
/// the kill rule (`docs/plan-hash-identity.md` §5, K2): the per-byte fold
/// was paid per byte of every text command every frame, so an arm whose
/// text is five bytes (`emit`'s `"label"`) cannot price a change to it.
const LABELS: [&str; 6] = [
    "Save",
    "count: 41",
    "Preferences",
    "Last synced 3 minutes ago",
    "Downloading update 2 of 7 (41%)",
    "The quick brown fox jumps over the lazy dog and keeps runnin",
];

/// `n` text commands, every one of them text.
fn emit_text(p: &mut dyn Painter, n: usize) {
    for i in 0..n {
        let x = (i % 40) as f32 * 20.0;
        let y = (i / 40) as f32 * 24.0;
        p.text([x, y, x + 18.0, y + 22.0], LABELS[i % LABELS.len()], 14.0, Color(240, 240, 240, 255));
    }
}

/// The text-heavy arm alone (`--text`): what K2 compares across builds.
fn run_text(n: usize) {
    println!("\n--- {n} text commands (text-heavy, M46) ---");
    let mut bare = DisplayList::new();
    bench("text-heavy: build the list", 20_000, || {
        bare.clear();
        emit_text(&mut bare, n);
        bare.cmds.len() as u64
    });
    let mut ret = Incremental::default();
    let mut out = DisplayList::new();
    bench("text-heavy: build + Incremental", 20_000, || {
        out.clear();
        {
            let mut rec = Recording::new(&mut out, &mut ret);
            emit_text(&mut rec, n);
        }
        matches!(ret.finish(), Damage::None) as u64
    });
}

fn run(n: usize) {
    println!("\n--- {n} commands ---");

    // The list build both forms pay, so it can be subtracted.
    let mut bare = DisplayList::new();
    bench("build the list (baseline)", 20_000, || {
        bare.clear();
        emit(&mut bare, n);
        bare.cmds.len() as u64
    });

    // Copying: build the list, then copy it and walk both.
    {
        let mut prev = DisplayList::new();
        let mut now = DisplayList::new();
        let mut seeded = false;
        bench("copying: build + copy + diff", 20_000, || {
            now.clear();
            emit(&mut now, n);
            let d = if seeded { diff(&prev, &now) } else { Damage::All };
            seeded = true;
            copy_list(&mut prev, &now);
            matches!(d, Damage::None) as u64
        });
    }

    // The wrapper's own cost, with an empty `observe`: what `Recording`
    // charges before any hashing happens.
    {
        let mut ret = Rebuild;
        let mut out = DisplayList::new();
        bench("wrapper only (Rebuild)", 20_000, || {
            out.clear();
            {
                let mut rec = Recording::new(&mut out, &mut ret);
                emit(&mut rec, n);
            }
            matches!(ret.finish(), Damage::None) as u64
        });
    }

    {
        let mut ret = Incremental::default();
        let mut out = DisplayList::new();
        bench("streaming: build + Incremental", 20_000, || {
            out.clear();
            {
                let mut rec = Recording::new(&mut out, &mut ret);
                emit(&mut rec, n);
            }
            matches!(ret.finish(), Damage::None) as u64
        });
    }

    {
        // The embedded shape: fixed array, no heap. Sized above the
        // frame, so nothing overflows to `All`.
        let mut ret: Streaming<4096> = Streaming::default();
        let mut out = DisplayList::new();
        bench("streaming: build + Streaming<4096>", 20_000, || {
            out.clear();
            {
                let mut rec = Recording::new(&mut out, &mut ret);
                emit(&mut rec, n);
            }
            matches!(ret.finish(), Damage::None) as u64
        });
    }

    // And the thing only streaming can do: retention with no list at
    // all, which is the whole reason T13.8 exists.
    {
        struct Sink;
        impl Painter for Sink {
            // M82/T82.3: a leaf with no fill table.
            fn fills(&self) -> &[lumen_static::style::Fill] {
                &[]
            }
            fn rect(&mut self, _: [f32; 4], _: Color) {}
            fn text_spaced(&mut self, _: [f32; 4], _: &str, _: f32, _: f32, _: u8, _: Color) {}
            fn rounded_rect(&mut self, _: [f32; 4], _: Color, _: f32) {}
            fn stroked_rect(&mut self, _: [f32; 4], _: Color, _: f32, _: f32) {}
            fn shadow(&mut self, _: [f32; 4], _: f32, _: f32) {}
            fn push_clip(&mut self, _: [f32; 4]) {}
            fn pop_clip(&mut self) {}
            fn path(&mut self, _: &[lumen_static::display::PathEl], _: lumen_static::display::Affine, _: lumen_static::display::PathPaint) {}
        }
        let mut ret: Streaming<4096> = Streaming::default();
        bench("streaming: NO list + Streaming<4096>", 20_000, || {
            let mut sink = Sink;
            {
                let mut rec = Recording::new(&mut sink, &mut ret);
                emit(&mut rec, n);
            }
            matches!(ret.finish(), Damage::None) as u64
        });
    }
}

fn main() {
    if std::env::args().any(|a| a == "--text") {
        println!("retain --text: the text-heavy arm only (M46/T46.1, K2)");
        run_text(2_000);
        return;
    }
    println!("retain: streaming (T13.8) vs the list-copying form it replaced");
    run(34);
    run(2_000);
    run_text(2_000);
    println!(
        "\n   `build the list` is the shared floor: subtract it from every row but the last.\n   \
         The last row is what only streaming can do -- damage with no list to keep."
    );
}
