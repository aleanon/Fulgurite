#!/usr/bin/env python3
"""Generate the Step 1 compile-time-firewall experiment workspace.

Emits experiments/firewall/generated/: a cargo workspace with N component
crates (the firewall arm), a thin `root` binary concatenating their const
sub-tables, and a `monolith` crate holding the identical component code as
modules (the control arm). See docs/00-handoff-static-tree.md §10 Step 1.

Deterministic: running it twice produces byte-identical output.
"""

import argparse
import os
import shutil
import sys

N_COMPONENTS = 200
NODES_PER_COMPONENT = 24  # --nodes
N_KINDS = 8  # widget kinds per component; --kinds scales component weight

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
OUT = os.path.join(ROOT, "experiments", "firewall", "generated")


def write(path, content):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(content)


def widget_struct(i, k):
    """One widget kind: a struct with a measure body that does real work,
    salted per (component, kind) so no two crates are byte-identical."""
    salt = (i * 31 + k * 7) % 97 + 1
    return f"""
pub struct W{k} {{
    pub pad: f32,
    pub gap: f32,
}}

impl W{k} {{
    pub fn measure(&self, c: Constraints) -> Size {{
        let mut w = self.pad * TWEAK + {salt}.0;
        let mut h = self.gap + {k}.5;
        for step in 0..{8 + k}u32 {{
            w = w.mul_add(1.0001, (step as f32) * 0.01{salt % 10});
            h = h.mul_add(0.9998, 0.02 + (step as f32) * 0.001);
        }}
        Size {{
            w: w.clamp(c.min_w, c.max_w),
            h: h.clamp(c.min_h, c.max_h),
        }}
    }}
}}

impl Measure for W{k} {{
    fn m(&self, c: Constraints) -> Size {{
        self.measure(c)
    }}
}}
"""


# Layout fns 4..8 instantiate generic Pairs (intra-crate monomorphization,
# the #[fuse] stand-in). 0..4 call a leaf widget directly.
PAIR_BODIES = {
    4: "Pair { a: W0 { pad: p, gap: g }, b: W1 { pad: g, gap: p } }",
    5: "Pair { a: W2 { pad: p, gap: g }, b: W3 { pad: g, gap: p } }",
    6: "Pair { a: Pair { a: W0 { pad: p, gap: g }, b: W2 { pad: g, gap: p } }, b: W4 { pad: p, gap: g } }",
    7: "Pair { a: W5 { pad: p, gap: g }, b: Pair { a: W6 { pad: g, gap: p }, b: W7 { pad: p, gap: g } } }",
}


def layout_fn(k):
    if k < 4:
        make = f"W{k} {{ pad: p, gap: g }}"
    elif k in PAIR_BODIES:
        make = PAIR_BODIES[k]
    else:
        a, b = k % 8, (k * 5 + 3) % N_KINDS
        make = f"Pair {{ a: W{a} {{ pad: p, gap: g }}, b: W{b} {{ pad: g, gap: p }} }}"
    return f"""
fn layout{k}(d: &NodeDesc, c: Constraints, s: &mut NodeState) -> Size {{
    let p = d.offset as f32 * 0.25;
    let g = d.child_count as f32 + 1.0;
    let widget = {make};
    let sz = widget.m(c);
    s.size = sz;
    sz
}}
"""


def component_lib(i):
    parts = [f"""//! comp{i:03} — generated component, firewall experiment (see §10 Step 1).
use firewall_shared::{{Constraints, NodeDesc, NodeState, Size, WidgetOps}};

/// The literal the measurement script edits to simulate a leaf change.
pub const TWEAK: f32 = 1.0;

pub trait Measure {{
    fn m(&self, c: Constraints) -> Size;
}}

pub struct Pair<A, B> {{
    pub a: A,
    pub b: B,
}}

impl<A: Measure, B: Measure> Measure for Pair<A, B> {{
    fn m(&self, c: Constraints) -> Size {{
        let s1 = self.a.m(c);
        let s2 = self.b.m(c);
        Size {{ w: s1.w.max(s2.w), h: s1.h + s2.h }}
    }}
}}
"""]
    for k in range(N_KINDS):
        parts.append(widget_struct(i, k))
    for k in range(N_KINDS):
        parts.append(layout_fn(k))

    parts.append(f"""
fn paint(d: &NodeDesc, s: &NodeState, acc: &mut u64) {{
    *acc = acc
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((d.kind as u64) ^ (s.size.w.to_bits() as u64) ^ {i});
}}

pub static OPS: [WidgetOps; {N_KINDS}] = [
""")
    for k in range(N_KINDS):
        parts.append(
            f'    WidgetOps {{ name: "comp{i:03}::w{k}", layout: layout{k}, paint }},\n'
        )
    parts.append("];\n\npub static SUB_TABLE: [NodeDesc; %d] = [\n" % NODES_PER_COMPONENT)
    for n in range(NODES_PER_COMPONENT):
        kind = (n + i) % N_KINDS
        first_child = (n + 1) if n < NODES_PER_COMPONENT - 1 else 0
        child_count = 2 if n % 3 == 0 else 0
        flags = 1 if (n + i) % 4 == 0 else 0  # bit 0 = rigid
        parts.append(
            f"    NodeDesc {{ offset: {n * 16}, kind: {kind}, "
            f"first_child: {first_child}, child_count: {child_count}, flags: {flags} }},\n"
        )
    parts.append("];\n")
    return "".join(parts)


def main_body(n_components, path_prefix):
    """Shared main() for root and monolith; path_prefix is '' (monolith
    modules) or unused since both reference compNNN:: paths."""
    lines = [
        "static PARTS: [(&[NodeDesc], &[WidgetOps; %d]); %d] = [\n" % (N_KINDS, n_components)
    ]
    for i in range(n_components):
        lines.append(f"    (&{path_prefix}comp{i:03}::SUB_TABLE, &{path_prefix}comp{i:03}::OPS),\n")
    lines.append("""];

fn main() {
    let t0 = std::time::Instant::now();
    let total: usize = PARTS.iter().map(|(t, _)| t.len()).sum();
    let mut states = vec![NodeState::default(); total];
    let c = Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h: 1080.0 };
    let mut acc = 0u64;
    let mut idx = 0;
    for (table, ops) in PARTS.iter() {
        for d in table.iter() {
            let op = &ops[d.kind as usize];
            let sz = (op.layout)(d, c, &mut states[idx]);
            (op.paint)(d, &states[idx], &mut acc);
            acc ^= sz.h.to_bits() as u64;
            idx += 1;
        }
    }
    println!("nodes={} checksum={:#018x} pass={:?}", total, acc, t0.elapsed());
}
""")
    return "".join(lines)


def generate():
    if os.path.exists(OUT):
        shutil.rmtree(OUT)

    # ---- workspace manifest ----
    write(os.path.join(OUT, "Cargo.toml"), """[workspace]
resolver = "2"
members = ["shared", "root", "monolith", "components/*"]
""")

    # ---- shared crate ----
    write(os.path.join(OUT, "shared", "Cargo.toml"), """[package]
name = "firewall-shared"
version = "0.1.0"
edition = "2021"

[lib]
name = "firewall_shared"
""")
    write(os.path.join(OUT, "shared", "src", "lib.rs"), """//! Value types shared across the firewall-experiment seam (handoff §5).

#[derive(Clone, Copy, Debug)]
pub struct NodeDesc {
    pub offset: u32,
    pub kind: u16,
    pub first_child: u32,
    pub child_count: u16,
    pub flags: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct Constraints {
    pub min_w: f32,
    pub max_w: f32,
    pub min_h: f32,
    pub max_h: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NodeState {
    pub size: Size,
    pub x: f32,
    pub y: f32,
}

pub struct WidgetOps {
    pub name: &'static str,
    pub layout: fn(&NodeDesc, Constraints, &mut NodeState) -> Size,
    pub paint: fn(&NodeDesc, &NodeState, &mut u64),
}
""")

    # ---- component crates (firewall arm) ----
    for i in range(N_COMPONENTS):
        d = os.path.join(OUT, "components", f"comp{i:03}")
        write(os.path.join(d, "Cargo.toml"), f"""[package]
name = "comp{i:03}"
version = "0.1.0"
edition = "2021"

[dependencies]
firewall-shared = {{ path = "../../shared" }}
""")
        write(os.path.join(d, "src", "lib.rs"), component_lib(i))

    # ---- root binary (firewall arm) ----
    deps = "\n".join(
        f'comp{i:03} = {{ path = "../components/comp{i:03}" }}' for i in range(N_COMPONENTS)
    )
    write(os.path.join(OUT, "root", "Cargo.toml"), f"""[package]
name = "root"
version = "0.1.0"
edition = "2021"

[dependencies]
firewall-shared = {{ path = "../shared" }}
{deps}
""")
    write(
        os.path.join(OUT, "root", "src", "main.rs"),
        "//! Thin root: concatenate 200 const sub-tables, kind-dispatched pass.\n"
        "use firewall_shared::{Constraints, NodeDesc, NodeState, WidgetOps};\n\n"
        + main_body(N_COMPONENTS, ""),
    )

    # ---- monolith (control arm): identical code, one crate ----
    write(os.path.join(OUT, "monolith", "Cargo.toml"), """[package]
name = "monolith"
version = "0.1.0"
edition = "2021"

[dependencies]
firewall-shared = { path = "../shared" }
""")
    mods = "".join(f"mod comp{i:03};\n" for i in range(N_COMPONENTS))
    write(
        os.path.join(OUT, "monolith", "src", "main.rs"),
        "//! Control arm: the same 200 components as modules of one crate.\n"
        "// Widget kinds unused by any layout fn are reachable in the library\n"
        "// crates but dead here; the lint would emit ~MBs of warnings per\n"
        "// rebuild and distort the control arm's timing.\n"
        "#![allow(dead_code)]\n"
        "use firewall_shared::{Constraints, NodeDesc, NodeState, WidgetOps};\n\n"
        + mods
        + "\n"
        + main_body(N_COMPONENTS, ""),
    )
    for i in range(N_COMPONENTS):
        write(
            os.path.join(OUT, "monolith", "src", f"comp{i:03}.rs"),
            component_lib(i).replace(
                "use firewall_shared::", "#[allow(unused_imports)]\nuse firewall_shared::"
            ),
        )

    total_files = sum(len(fs) for _, _, fs in os.walk(OUT))
    print(f"generated {N_COMPONENTS} components ({total_files} files) in {OUT}")


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--kinds", type=int, default=N_KINDS)
    ap.add_argument("--nodes", type=int, default=NODES_PER_COMPONENT)
    args = ap.parse_args()
    N_KINDS = args.kinds
    NODES_PER_COMPONENT = args.nodes
    sys.exit(generate())
