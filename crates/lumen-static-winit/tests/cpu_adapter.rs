//! M51/T51.2 (R9-F6): a CPU adapter is not a GPU under `Prefer::Auto`.
//!
//! Two halves. The decision — which constructor each `Prefer` calls, and
//! whether it tells the GPU one to accept a CPU adapter — runs everywhere,
//! with no adapter and no window. The live half needs a machine whose
//! **only** adapter is a CPU rasteriser, which is the configuration the
//! finding describes: `just gpu` makes one out of this machine by pointing
//! the Vulkan loader at lavapipe and the EGL loader at Mesa, and sets
//! `LUMEN_REQUIRE_CPU_ONLY=1`, under which a GPU being visible is a
//! failure rather than a "not run".

use lumen_static_winit::{choose_backend, Prefer};

/// Which constructor ran, and what it was told.
#[derive(Debug, PartialEq)]
enum Made {
    Gpu { accept_cpu: bool },
    Software,
}

fn decide(prefer: Prefer, gpu_opens: bool) -> Made {
    choose_backend(
        prefer,
        |accept_cpu| gpu_opens.then_some(Made::Gpu { accept_cpu }),
        || Made::Software,
    )
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

/// The live half: on a CPU-only machine, `Auto` yields the software
/// backend and `Gpu` still opens the CPU adapter.
#[cfg(feature = "gpu")]
#[test]
fn on_a_cpu_only_machine_auto_keeps_the_software_canvas() {
    use lumen_static_wgpu::Gpu;
    let required = std::env::var("LUMEN_REQUIRE_CPU_ONLY").is_ok_and(|v| v == "1");
    let all = lumen_static_wgpu::adapters();
    println!("adapters: {all:?}");
    let gpus: Vec<_> = all.iter().filter(|(_, t)| *t != wgpu_cpu()).collect();
    if all.is_empty() || !gpus.is_empty() {
        let why = if all.is_empty() { "no adapter at all".to_string() } else { format!("a GPU is visible: {gpus:?}") };
        assert!(!required, "LUMEN_REQUIRE_CPU_ONLY=1 but {why}");
        println!("NOT RUN: {why} -- this is not the machine the finding describes");
        return;
    }
    let open = |accept_cpu: bool| Gpu::headless_accepting(64, 64, accept_cpu).map(|g| format!("gpu ({})", g.adapter()));
    let auto = choose_backend(Prefer::Auto, open, || "software".to_string());
    let asked = choose_backend(Prefer::Gpu, open, || "software".to_string());
    println!("Auto: {auto}; Gpu: {asked}");
    assert_eq!(auto, "software", "Prefer::Auto opened a CPU rasteriser");
    assert!(asked.starts_with("gpu ("), "Prefer::Gpu must still take the CPU adapter, got {asked}");
}

#[cfg(feature = "gpu")]
fn wgpu_cpu() -> lumen_static_wgpu::wgpu::DeviceType {
    lumen_static_wgpu::wgpu::DeviceType::Cpu
}
