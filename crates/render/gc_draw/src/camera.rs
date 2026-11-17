//! The view camera.

use gc_math::{Aabb, DMat3, DQuat, DVec3, Plane, Ray, fm};

/// The named views of the view menu and the ViewCube.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs, reason = "the names are the documentation")]
pub enum StandardView {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
    /// The home view: looking down at the model from the front right.
    Iso,
}

/// A camera that orbits a pivot point.
///
/// The same camera serves orthographic and perspective projection; switching keeps the size of things at
/// the pivot unchanged on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// The point the camera looks at and orbits about.
    pub pivot: DVec3,
    /// Camera orientation: maps camera axes (X right, Y up, Z towards the viewer) to world axes.
    pub rotation: DQuat,
    /// Distance from the eye to the pivot.
    pub distance: f64,
    /// Vertical field of view of the perspective projection, radians.
    pub fov_y: f64,
    /// True for orthographic projection.
    pub ortho: bool,
}

impl Default for Camera {
    fn default() -> Self {
        let mut camera =
            Self { pivot: DVec3::ZERO, rotation: DQuat::IDENTITY, distance: 1.0, fov_y: 0.6, ortho: true };
        camera.look(StandardView::Iso);
        camera
    }
}

impl Camera {
    /// World direction of the screen's right.
    pub fn right(&self) -> DVec3 {
        self.rotation * DVec3::X
    }

    /// World direction of the screen's up.
    pub fn up(&self) -> DVec3 {
        self.rotation * DVec3::Y
    }

    /// World direction the camera looks along.
    pub fn forward(&self) -> DVec3 {
        self.rotation * -DVec3::Z
    }

    /// Position of the eye.
    pub fn eye(&self) -> DVec3 {
        self.pivot - self.forward() * self.distance
    }

    /// Height of the view at the pivot, in world units.
    pub fn view_height(&self) -> f64 {
        2.0 * self.distance * fm::tan(self.fov_y * 0.5)
    }

    /// World units covered by one pixel at the pivot's depth.
    pub fn px_scale(&self, viewport: [f64; 2]) -> f64 {
        self.view_height() / viewport[1].max(1.0)
    }

    /// Points the camera along `direction` with `up` as the screen's up, keeping pivot and distance.
    pub fn look_along(&mut self, direction: DVec3, up: DVec3) {
        let Some(back) = (-direction).try_normalize() else { return };
        let Some(right) = up.cross(back).try_normalize() else { return };
        self.rotation = DQuat::from_mat3(&DMat3::from_cols(right, back.cross(right), back)).normalize();
    }

    /// Switches to a named view.
    pub fn look(&mut self, view: StandardView) {
        let (direction, up) = match view {
            StandardView::Front => (DVec3::Y, DVec3::Z),
            StandardView::Back => (-DVec3::Y, DVec3::Z),
            StandardView::Right => (-DVec3::X, DVec3::Z),
            StandardView::Left => (DVec3::X, DVec3::Z),
            StandardView::Top => (-DVec3::Z, DVec3::Y),
            StandardView::Bottom => (DVec3::Z, -DVec3::Y),
            StandardView::Iso => (DVec3::new(-1.0, 1.0, -1.0), DVec3::Z),
        };
        self.look_along(direction, up);
    }

    /// The matrix (column-major) taking a position relative to the eye to clip space, for a renderer with
    /// reverse-Z depth (1 at the near plane, 0 at the far plane).
    ///
    /// `extent` is the radius of what must stay visible around the pivot. Working relative to the eye
    /// keeps single-precision GPU arithmetic exact far from the world origin.
    pub fn clip_from_eye_relative(&self, viewport: [f64; 2], extent: f64) -> [[f64; 4]; 4] {
        let half_h = (self.view_height() * 0.5).max(1e-12);
        let half_w = half_h * viewport[0].max(1.0) / viewport[1].max(1.0);
        let extent = extent.max(self.distance).max(1e-6);
        let (right, up, forward) = (self.right(), self.up(), self.forward());
        // Rows of the result: clip x, y, z and w as functions of (relative position, 1).
        let rows: [[f64; 4]; 4] = if self.ortho {
            // An orthographic eye may sit inside the scene: keep what is behind it too.
            let (near, far) = (self.distance - 2.0 * extent, self.distance + 2.0 * extent);
            let k = 1.0 / (far - near);
            [
                [right.x / half_w, right.y / half_w, right.z / half_w, 0.0],
                [up.x / half_h, up.y / half_h, up.z / half_h, 0.0],
                [-forward.x * k, -forward.y * k, -forward.z * k, far * k],
                [0.0, 0.0, 0.0, 1.0],
            ]
        } else {
            let far = self.distance + 2.0 * extent;
            let near = (self.distance * 0.02).min(far * 0.5);
            let (sx, sy) = (self.distance / half_w, self.distance / half_h);
            let k = near / (far - near);
            [
                [right.x * sx, right.y * sx, right.z * sx, 0.0],
                [up.x * sy, up.y * sy, up.z * sy, 0.0],
                [-forward.x * k, -forward.y * k, -forward.z * k, far * k],
                [forward.x, forward.y, forward.z, 0.0],
            ]
        };
        let mut columns = [[0.0; 4]; 4];
        for (r, row) in rows.iter().enumerate() {
            for (c, value) in row.iter().enumerate() {
                columns[c][r] = *value;
            }
        }
        columns
    }

    /// Where a world point appears on screen. `None` when it is behind a perspective camera.
    pub fn project(&self, point: DVec3, viewport: [f64; 2]) -> Option<[f64; 2]> {
        let relative = point - self.eye();
        let (x, y, depth) =
            (relative.dot(self.right()), relative.dot(self.up()), relative.dot(self.forward()));
        let px = self.px_scale(viewport);
        let scale = if self.ortho {
            1.0
        } else if depth > 1e-9 * self.distance {
            self.distance / depth
        } else {
            return None;
        };
        Some([viewport[0] * 0.5 + x * scale / px, viewport[1] * 0.5 - y * scale / px])
    }

    /// The ray through a screen position.
    pub fn ray(&self, screen: [f64; 2], viewport: [f64; 2]) -> Ray {
        let px = self.px_scale(viewport);
        let offset = self.right() * ((screen[0] - viewport[0] * 0.5) * px)
            - self.up() * ((screen[1] - viewport[1] * 0.5) * px);
        if self.ortho {
            Ray { origin: self.eye() + offset, dir: self.forward() }
        } else {
            let through = self.pivot + offset;
            let dir = (through - self.eye()).try_normalize().unwrap_or_else(|| self.forward());
            Ray { origin: self.eye(), dir }
        }
    }

    /// Where the ray through a screen position meets a plane; `None` when the view is (nearly) edge-on to
    /// the plane or the plane is behind the camera.
    pub fn on_plane(&self, screen: [f64; 2], viewport: [f64; 2], plane: &Plane) -> Option<DVec3> {
        let ray = self.ray(screen, viewport);
        // Reject views closer than about 3 degrees to the plane: the hit would be meaningless.
        plane.intersect_ray(&ray, 0.05).map(|t| ray.at(t)).or_else(|| {
            // An orthographic eye can sit on the wrong side of the plane without it mattering.
            let reversed = Ray { origin: ray.origin, dir: -ray.dir };
            self.ortho.then(|| plane.intersect_ray(&reversed, 0.05).map(|t| reversed.at(t))).flatten()
        })
    }

    /// Pans so that what was under `from` is now under `to` (screen positions).
    pub fn pan(&mut self, from: [f64; 2], to: [f64; 2], viewport: [f64; 2]) {
        let px = self.px_scale(viewport);
        self.pivot -= self.right() * ((to[0] - from[0]) * px);
        self.pivot += self.up() * ((to[1] - from[1]) * px);
    }

    /// Zooms by `factor` (> 1 zooms in), keeping what is under the cursor where it is.
    pub fn zoom(&mut self, factor: f64, cursor: [f64; 2], viewport: [f64; 2]) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        let before = self.px_scale(viewport);
        self.distance = (self.distance / factor).clamp(1e-6, 1e6);
        let after = self.px_scale(viewport);
        let (dx, dy) = (cursor[0] - viewport[0] * 0.5, cursor[1] - viewport[1] * 0.5);
        self.pivot += self.right() * (dx * (before - after)) - self.up() * (dy * (before - after));
    }

    /// Orbits about the pivot: horizontal motion turns about the world's Z axis, vertical motion tilts.
    pub fn orbit(&mut self, dx: f64, dy: f64) {
        const RADIANS_PER_PIXEL: f64 = 0.008;
        let turn = DQuat::from_axis_angle(DVec3::Z, -dx * RADIANS_PER_PIXEL);
        let tilt = DQuat::from_axis_angle(DVec3::X, -dy * RADIANS_PER_PIXEL);
        self.rotation = (turn * self.rotation * tilt).normalize();
    }

    /// Frames a box: the pivot moves to its centre and the distance is set so that it fills most of the view.
    pub fn fit(&mut self, bounds: &Aabb, viewport: [f64; 2]) {
        if bounds.is_empty() {
            return;
        }
        self.pivot = bounds.center();
        let radius = (bounds.diagonal() * 0.5).max(1e-4);
        let aspect = (viewport[0] / viewport[1].max(1.0)).min(1.0);
        // The view height must hold the bounding sphere, with a margin, in the narrower direction.
        self.distance = (radius * 1.2 / aspect / fm::tan(self.fov_y * 0.5)).clamp(1e-6, 1e6);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: [f64; 2] = [800.0, 600.0];

    fn close(a: DVec3, b: DVec3) -> bool {
        (a - b).length() < 1e-9
    }

    #[test]
    fn the_clip_matrix_agrees_with_project_and_orders_depth_near_to_far() {
        for ortho in [true, false] {
            let mut camera =
                Camera { pivot: DVec3::new(3.0, -2.0, 1.0), distance: 2.5, ortho, ..Camera::default() };
            camera.orbit(0.4, -0.3);
            let m = camera.clip_from_eye_relative(VIEW, 4.0);
            let clip = |p: DVec3| {
                let r = p - camera.eye();
                let row = |i: usize| m[0][i] * r.x + m[1][i] * r.y + m[2][i] * r.z + m[3][i];
                (row(0) / row(3), row(1) / row(3), row(2) / row(3))
            };
            for point in [camera.pivot, DVec3::new(3.4, -1.5, 1.2), DVec3::new(2.0, -2.5, 0.1)] {
                let (x, y, z) = clip(point);
                let screen = camera.project(point, VIEW).unwrap();
                assert!((VIEW[0] * 0.5 * (1.0 + x) - screen[0]).abs() < 1e-6, "{ortho}");
                assert!((VIEW[1] * 0.5 * (1.0 - y) - screen[1]).abs() < 1e-6, "{ortho}");
                assert!(z > 0.0 && z < 1.0, "{ortho}: {z}");
            }
            let (near, far) = (camera.pivot - camera.forward(), camera.pivot + camera.forward());
            assert!(clip(near).2 > clip(far).2, "reverse-Z: nearer is larger");
        }
    }

    #[test]
    fn standard_views_look_the_documented_way() {
        let mut camera = Camera::default();
        camera.look(StandardView::Front);
        assert!(
            close(camera.forward(), DVec3::Y)
                && close(camera.up(), DVec3::Z)
                && close(camera.right(), DVec3::X)
        );
        camera.look(StandardView::Top);
        assert!(
            close(camera.forward(), -DVec3::Z)
                && close(camera.up(), DVec3::Y)
                && close(camera.right(), DVec3::X)
        );
        camera.look(StandardView::Right);
        assert!(close(camera.forward(), -DVec3::X) && close(camera.up(), DVec3::Z));
        camera.look(StandardView::Iso);
        assert!(close(camera.forward(), DVec3::new(-1.0, 1.0, -1.0).normalize()));
        assert!(camera.up().z > 0.0);
    }

    #[test]
    fn projection_and_rays_are_inverse_in_both_projections() {
        for ortho in [true, false] {
            let mut camera =
                Camera { ortho, distance: 3.0, pivot: DVec3::new(0.2, -0.1, 0.4), ..Camera::default() };
            camera.orbit(120.0, -45.0);
            let point = DVec3::new(0.7, 0.3, -0.2);
            let screen = camera.project(point, VIEW).unwrap();
            assert!(camera.ray(screen, VIEW).line_distance(point) < 1e-9, "ortho = {ortho}");
            let centre = camera.project(camera.pivot, VIEW).unwrap();
            assert!((centre[0] - 400.0).abs() < 1e-9 && (centre[1] - 300.0).abs() < 1e-9);
        }
        let camera = Camera { ortho: false, ..Camera::default() };
        assert_eq!(camera.project(camera.eye() - camera.forward(), VIEW), None, "behind the eye");
    }

    #[test]
    fn switching_projection_keeps_the_size_at_the_pivot() {
        let mut camera = Camera { distance: 2.0, ..Camera::default() };
        let near_pivot = camera.pivot + camera.right() * 0.1;
        let ortho = camera.project(near_pivot, VIEW).unwrap();
        camera.ortho = false;
        let perspective = camera.project(near_pivot, VIEW).unwrap();
        assert!((ortho[0] - perspective[0]).abs() < 1e-9);
    }

    #[test]
    fn panning_keeps_the_grabbed_point_under_the_cursor() {
        let mut camera = Camera::default();
        camera.look(StandardView::Top);
        let plane = Plane::from_point_normal(DVec3::ZERO, DVec3::Z).unwrap();
        let grabbed = camera.on_plane([250.0, 180.0], VIEW, &plane).unwrap();
        camera.pan([250.0, 180.0], [420.0, 333.0], VIEW);
        let now = camera.project(grabbed, VIEW).unwrap();
        assert!((now[0] - 420.0).abs() < 1e-9 && (now[1] - 333.0).abs() < 1e-9);
    }

    #[test]
    fn zooming_keeps_the_point_under_the_cursor() {
        let mut camera = Camera::default();
        camera.look(StandardView::Top);
        let plane = Plane::from_point_normal(DVec3::ZERO, DVec3::Z).unwrap();
        let cursor = [650.0, 120.0];
        let under = camera.on_plane(cursor, VIEW, &plane).unwrap();
        let scale = camera.px_scale(VIEW);
        camera.zoom(1.5, cursor, VIEW);
        assert!((camera.px_scale(VIEW) - scale / 1.5).abs() < 1e-12);
        assert!((camera.on_plane(cursor, VIEW, &plane).unwrap() - under).length() < 1e-9);
        camera.zoom(f64::NAN, cursor, VIEW);
        camera.zoom(0.0, cursor, VIEW);
        assert!(camera.distance.is_finite() && camera.distance > 0.0);
    }

    #[test]
    fn orbiting_keeps_the_pivot_in_place_and_the_horizon_level() {
        let mut camera = Camera::default();
        let before = camera.project(camera.pivot, VIEW).unwrap();
        camera.orbit(300.0, 80.0);
        let after = camera.project(camera.pivot, VIEW).unwrap();
        assert!((after[0] - before[0]).abs() < 1e-9 && (after[1] - before[1]).abs() < 1e-9);
        assert!(camera.right().z.abs() < 1e-12, "the screen's right stays horizontal");
        assert!((camera.rotation.length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn fit_frames_a_box_and_edge_on_planes_are_refused() {
        let mut camera = Camera::default();
        let bounds = Aabb::from_points([DVec3::new(1.0, 1.0, 0.0), DVec3::new(3.0, 2.0, 1.0)]);
        camera.fit(&bounds, VIEW);
        assert!(close(camera.pivot, bounds.center()));
        for corner in bounds.corners() {
            let screen = camera.project(corner, VIEW).unwrap();
            assert!(screen[0] > 0.0 && screen[0] < 800.0 && screen[1] > 0.0 && screen[1] < 600.0);
        }
        camera.look(StandardView::Front);
        let ground = Plane::from_point_normal(DVec3::ZERO, DVec3::Z).unwrap();
        assert_eq!(
            camera.on_plane([400.0, 300.0], VIEW, &ground),
            None,
            "the ground is edge-on in the front view"
        );
    }
}
