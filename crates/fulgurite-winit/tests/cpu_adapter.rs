//! M51/T51.2 (R9-F6): a CPU adapter is not a GPU under `Prefer::Auto`;
//! and M83/T83.2: a program's backend factory is asked before either.
//!
//! Two halves. The decision — which constructor each `Prefer` calls, and
//! whether it tells the GPU one to accept a CPU adapter — runs everywhere,
//! with no adapter and no window. The live half needs a machine whose
//! **only** adapter is a CPU rasteriser, which is the configuration the
//! finding describes: `just gpu` makes one out of this machine by pointing
//! the Vulkan loader at lavapipe and the EGL loader at Mesa, and sets
//! `FULGURITE_REQUIRE_CPU_ONLY=1`, under which a GPU being visible is a
//! failure rather than a "not run".

use fulgurite_winit::{choose_backend, Prefer};

/// Which constructor ran, and what it was told.
#[derive(Debug, PartialEq)]
enum Made {
    Custom { accept_cpu: bool },
    Gpu { accept_cpu: bool },
    Software,
}

fn decide(prefer: Prefer, gpu_opens: bool) -> Made {
    choose_backend(prefer, |_| None, |accept_cpu| gpu_opens.then_some(Made::Gpu { accept_cpu }), || Made::Software)
}

#[test]
fn auto_refuses_a_cpu_adapter_and_gpu_accepts_one() {
    let cases = [
        (Prefer::Auto, true, Made::Gpu { accept_cpu: false }),
        (Prefer::Gpu, true, Made::Gpu { accept_cpu: true }),
        (Prefer::Software, true, Made::Software),
        (Prefer::Auto, false, Made::Software),
        (Prefer::Gpu, false, Made::Software),
    ];
    for (prefer, opens, want) in cases {
        let got = decide(prefer, opens);
        println!("{prefer:?}, gpu opens: {opens}: {got:?} (want {want:?})");
        assert_eq!(got, want, "{prefer:?} with the GPU {}", if opens { "opening" } else { "absent" });
    }
}

/// What the GPU slot is, in a row of the factory table.
#[derive(Clone, Copy, Debug)]
enum GpuSlot {
    /// A `gpu` build whose adapter opens.
    Opens,
    /// A `gpu` build with no adapter: the constructor answers `None`.
    None,
    /// The `not(gpu)` branch of `pick_backend`: its constructor always
    /// answers `None`, and is asked nonetheless.
    Absent,
}

/// **M83/T83.2: the custom column** — `Program::backend_with`'s factory
/// is asked first, `None` falls through to the built-in choice, and
/// `Prefer::Software` never calls it. Each row prints what was made and
/// how often the factory and the GPU constructor were called.
#[test]
fn a_program_factory_is_asked_first_and_skipped_under_software() {
    // (prefer, custom answers Some, gpu slot, want, custom calls, gpu calls)
    let rows = [
        (Prefer::Auto, true, GpuSlot::Opens, Made::Custom { accept_cpu: false }, 1, 0),
        (Prefer::Gpu, false, GpuSlot::Opens, Made::Gpu { accept_cpu: true }, 1, 1),
        (Prefer::Software, true, GpuSlot::Opens, Made::Software, 0, 0),
        (Prefer::Auto, false, GpuSlot::None, Made::Software, 1, 1),
        (Prefer::Auto, true, GpuSlot::Absent, Made::Custom { accept_cpu: false }, 1, 0),
    ];
    for (prefer, some, slot, want, want_c, want_g) in rows {
        let (mut c_calls, mut g_calls) = (0, 0);
        let got = choose_backend(
            prefer,
            |accept_cpu| {
                c_calls += 1;
                some.then_some(Made::Custom { accept_cpu })
            },
            |accept_cpu| {
                g_calls += 1;
                match slot {
                    GpuSlot::Opens => Some(Made::Gpu { accept_cpu }),
                    GpuSlot::None | GpuSlot::Absent => None,
                }
            },
            || Made::Software,
        );
        println!(
            "({prefer:?}, custom {}, gpu {slot:?}) -> {got:?}; custom called {c_calls}, gpu called {g_calls} \
             (want {want:?}, {want_c}, {want_g})",
            if some { "Some" } else { "None" }
        );
        assert_eq!(got, want, "({prefer:?}, custom {some}, gpu {slot:?}) made the wrong backend");
        assert_eq!((c_calls, g_calls), (want_c, want_g), "({prefer:?}, custom {some}, gpu {slot:?}): calls (custom, gpu)");
    }
}

/// The live half: on a CPU-only machine, `Auto` yields the software
/// backend and `Gpu` still opens the CPU adapter.
#[cfg(feature = "gpu")]
#[test]
fn on_a_cpu_only_machine_auto_keeps_the_software_canvas() {
    use fulgurite_wgpu::Gpu;
    let required = std::env::var("FULGURITE_REQUIRE_CPU_ONLY").is_ok_and(|v| v == "1");
    let all = fulgurite_wgpu::adapters();
    println!("adapters: {all:?}");
    let gpus: Vec<_> = all.iter().filter(|(_, t)| *t != wgpu_cpu()).collect();
    if all.is_empty() || !gpus.is_empty() {
        let why = if all.is_empty() { "no adapter at all".to_string() } else { format!("a GPU is visible: {gpus:?}") };
        assert!(!required, "FULGURITE_REQUIRE_CPU_ONLY=1 but {why}");
        println!("NOT RUN: {why} -- this is not the machine the finding describes");
        return;
    }
    let open = |accept_cpu: bool| Gpu::headless_accepting(64, 64, accept_cpu).map(|g| format!("gpu ({})", g.adapter()));
    let auto = choose_backend(Prefer::Auto, |_| None, open, || "software".to_string());
    let asked = choose_backend(Prefer::Gpu, |_| None, open, || "software".to_string());
    println!("Auto: {auto}; Gpu: {asked}");
    assert_eq!(auto, "software", "Prefer::Auto opened a CPU rasteriser");
    assert!(asked.starts_with("gpu ("), "Prefer::Gpu must still take the CPU adapter, got {asked}");
}

#[cfg(feature = "gpu")]
fn wgpu_cpu() -> fulgurite_wgpu::wgpu::DeviceType {
    fulgurite_wgpu::wgpu::DeviceType::Cpu
}
