//! Test helpers: GPU probing and snapshot utilities.
//!
//! The sandbox and CI have no hardware GPU. Tests that need one call [`gpu_or_skip`], which returns
//! `None` (after printing `SKIPPED`) when no adapter exists, unless `GC_REQUIRE_GPU=1` is set, in which
//! case a missing adapter is a test failure. `cargo xtask test-gpu` sets that variable.

/// A wgpu device for tests.
pub struct GpuCtx {
    /// Device created from the first adapter wgpu offers.
    pub device: wgpu::Device,
    /// Queue belonging to [`GpuCtx::device`].
    pub queue: wgpu::Queue,
    /// Which adapter was picked (used to decide whether snapshots are compared strictly).
    pub adapter_info: wgpu::AdapterInfo,
}

impl GpuCtx {
    /// True when the adapter is the reference software renderer used for strict snapshot comparison.
    pub fn is_reference_renderer(&self) -> bool {
        self.adapter_info.name.contains("SwiftShader")
    }
}

/// True when the environment demands a GPU (`GC_REQUIRE_GPU=1`).
pub fn gpu_required() -> bool {
    std::env::var("GC_REQUIRE_GPU").is_ok_and(|v| v == "1")
}

/// Probes for an adapter and creates a device. Returns `None` when the machine has none.
pub fn gpu() -> Option<GpuCtx> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
    let adapter_info = adapter.get_info();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
    Some(GpuCtx { device, queue, adapter_info })
}

/// Returns a GPU context, or `None` after printing `SKIPPED`.
///
/// # Panics
/// Panics when no adapter exists and `GC_REQUIRE_GPU=1`, so a GPU test run cannot pass vacuously.
#[allow(clippy::panic, clippy::print_stderr, reason = "test-support crate: failing loudly is the purpose")]
pub fn gpu_or_skip(test_name: &str) -> Option<GpuCtx> {
    match gpu() {
        Some(ctx) => Some(ctx),
        None if gpu_required() => panic!("{test_name}: GC_REQUIRE_GPU=1 but no wgpu adapter is available"),
        None => {
            eprintln!("SKIPPED(no adapter): {test_name}");
            None
        }
    }
}

/// Number of pixels whose RGBA channels differ by more than `tolerance` between two images of equal size.
/// Returns `None` when the buffers differ in length.
pub fn differing_pixels(a: &[u8], b: &[u8], tolerance: u8) -> Option<usize> {
    if a.len() != b.len() {
        return None;
    }
    Some(
        a.chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(pa, pb)| pa.iter().zip(pb.iter()).any(|(x, y)| x.abs_diff(*y) > tolerance))
            .count(),
    )
}
