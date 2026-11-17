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

mod meshes {
    use gc_draw::{Camera, StandardView};
    use gc_math::DVec3;
    use gc_mesh::primitives;
    use gc_render::{Instance, Line, MeshRenderer, View, ViewTarget, read_rgba};

    const SIZE: (u32, u32) = (64, 48);

    #[allow(clippy::cast_possible_truncation, reason = "GPU matrices are f32")]
    fn narrow(m: [[f64; 4]; 4]) -> [[f32; 4]; 4] {
        m.map(|column| column.map(|v| v as f32))
    }

    /// A camera looking along +Y at the origin, and the placement of a mesh at a world position.
    fn view(ortho: bool) -> (Camera, View) {
        let mut camera = Camera { ortho, distance: 2.0, ..Camera::default() };
        camera.look(StandardView::Front);
        let f = camera.forward();
        #[allow(clippy::cast_possible_truncation, reason = "GPU vectors are f32")]
        let forward = [f.x as f32, f.y as f32, f.z as f32];
        let clip = camera.clip_from_eye_relative([f64::from(SIZE.0), f64::from(SIZE.1)], 3.0);
        (camera, View { clip_from_relative: narrow(clip), light: forward, forward })
    }

    #[allow(clippy::cast_possible_truncation, reason = "GPU matrices are f32")]
    fn at(camera: &Camera, mesh: u64, position: DVec3, color: [f32; 4]) -> Instance {
        let r = position - camera.eye();
        let mut model =
            [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        model[3] = [r.x as f32, r.y as f32, r.z as f32, 1.0];
        Instance { mesh, model, color }
    }

    /// The lit face shows its own colour plus a little white highlight: the channel that is the colour
    /// stays near full and the others stay low.
    fn mostly(p: [u8; 4], channel: usize) -> bool {
        (0..3).all(|c| if c == channel { p[c] > 200 } else { p[c] < 80 }) && p[3] == 255
    }

    fn pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
        let i = ((y * SIZE.0 + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    }

    #[test]
    fn meshes_are_shaded_depth_tested_and_batched() {
        let Some(gpu) = gc_test_support::gpu_or_skip("meshes_are_shaded_depth_tested_and_batched") else {
            return;
        };
        let target = ViewTarget::new(&gpu.device, SIZE.0, SIZE.1);
        let mut renderer = MeshRenderer::new(&gpu.device);
        renderer.upload(&gpu.device, &gpu.queue, 1, &primitives::cuboid([0.4, 0.4, 0.4]).unwrap());
        renderer.upload(&gpu.device, &gpu.queue, 2, &primitives::sphere(0.15, 24).unwrap());
        assert!(renderer.has_mesh(1) && !renderer.has_mesh(3));
        const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
        const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
        for ortho in [true, false] {
            let (camera, view) = view(ortho);
            // A red cube at the origin; a green cube behind it, shifted right so that part of it shows.
            let front = at(&camera, 1, DVec3::ZERO, RED);
            let back = at(&camera, 1, DVec3::new(0.25, 1.0, 0.0), GREEN);
            for instances in [[front, back], [back, front]] {
                let stats = renderer.draw(
                    &gpu.device,
                    &gpu.queue,
                    &target,
                    [0.0, 0.0, 1.0, 1.0],
                    &view,
                    &instances,
                    &[],
                    &[],
                );
                assert_eq!(stats.draw_calls, 1, "two instances of one mesh are one call");
                assert_eq!(stats.triangles, 24);
                let pixels = read_rgba(&gpu.device, &gpu.queue, &target).unwrap();
                // The face looks straight at the light: bright red with a touch of highlight. Whatever the
                // draw order, the nearer cube covers the centre.
                let centre = pixel(&pixels, SIZE.0 / 2, SIZE.1 / 2);
                assert!(mostly(centre, 0), "ortho {ortho}: {centre:?}");
                assert_eq!(pixel(&pixels, 1, 1), [0, 0, 255, 255], "background, ortho {ortho}");
                let right_of_front = pixel(&pixels, SIZE.0 / 2 + if ortho { 11 } else { 9 }, SIZE.1 / 2);
                assert!(
                    mostly(right_of_front, 1),
                    "the cube behind shows beside it, ortho {ortho}: {right_of_front:?}"
                );
            }
            // A white line 4 px wide across the view: in front of the cube it covers it, behind the cube
            // it shows only beside it.
            #[allow(clippy::cast_possible_truncation, reason = "GPU data is f32")]
            let line = |y: f64| {
                let end = |x: f64| {
                    let r = DVec3::new(x, y, 0.0) - camera.eye();
                    [r.x as f32, r.y as f32, r.z as f32]
                };
                Line { a: end(-0.6), b: end(0.6), color: [1.0; 4], width: 4.0 }
            };
            for (y, covers) in [(-0.5, true), (0.5, false)] {
                renderer.draw(
                    &gpu.device,
                    &gpu.queue,
                    &target,
                    [0.0, 0.0, 1.0, 1.0],
                    &view,
                    &[front],
                    &[line(y)],
                    &[],
                );
                let pixels = read_rgba(&gpu.device, &gpu.queue, &target).unwrap();
                let centre = pixel(&pixels, SIZE.0 / 2, SIZE.1 / 2);
                assert_eq!(
                    centre == [255, 255, 255, 255],
                    covers,
                    "ortho {ortho}, line at y = {y}: {centre:?}"
                );
                if ortho {
                    // Beside the cube the line is there either way, about four pixels thick.
                    let column = SIZE.0 / 2 - 14;
                    let white = (0..SIZE.1).filter(|row| pixel(&pixels, column, *row)[0] > 128).count();
                    assert!((3..=5).contains(&white), "{white} px thick, line at y = {y}");
                }
            }
            // A translucent sphere in front of the red cube tints it (its front and its back both blend
            // over the cube: a quarter of the red remains); an unknown mesh is skipped.
            let glass = at(&camera, 2, DVec3::new(0.0, -0.5, 0.0), [0.0, 0.0, 1.0, 0.5]);
            let unknown = at(&camera, 9, DVec3::ZERO, GREEN);
            let stats = renderer.draw(
                &gpu.device,
                &gpu.queue,
                &target,
                [0.0; 4],
                &view,
                &[glass, unknown, front],
                &[],
                &[],
            );
            assert_eq!(stats.draw_calls, 2);
            let centre = pixel(&read_rgba(&gpu.device, &gpu.queue, &target).unwrap(), SIZE.0 / 2, SIZE.1 / 2);
            assert!(centre[0] > 55 && centre[0] < 100 && centre[1] < 60 && centre[2] > 150, "{centre:?}");
        }
    }
}
