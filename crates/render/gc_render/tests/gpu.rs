//! GPU tests for the renderer. Skipped when no adapter exists unless `GC_REQUIRE_GPU=1`.

use gc_render::{ViewTarget, clear, read_rgba};

#[test]
fn clear_and_readback_returns_the_clear_colour() {
    let Some(gpu) = gc_test_support::gpu_or_skip("clear_and_readback_returns_the_clear_colour") else {
        return;
    };
    let target = ViewTarget::new(&gpu.device, 37, 21); // odd width exercises row padding
    clear(&gpu.device, &gpu.queue, &target, [1.0, 0.5, 0.0, 1.0]);
    let pixels = read_rgba(&gpu.device, &gpu.queue, &target).unwrap();
    assert_eq!(pixels.len(), 37 * 21 * 4);
    for px in pixels.chunks_exact(4) {
        assert_eq!(px[0], 255, "red");
        assert!((i32::from(px[1]) - 128).abs() <= 1, "green was {}", px[1]);
        assert_eq!(px[2], 0, "blue");
        assert_eq!(px[3], 255, "alpha");
    }
}
