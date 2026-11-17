//! The view cube: a small cube that shows how the view is turned and turns it when clicked.
//!
//! The cube has half-size 1 and sits at the origin of its own little scene, seen through the camera's
//! rotation without perspective. Its 26 targets are the 6 faces, 12 edges and 8 corners; each is named by
//! the outward direction it faces, with components -1, 0 or 1.

use gc_math::DVec3;

use crate::Camera;

/// Part of a face this close to its border belongs to the edge or corner there.
const BORDER: f64 = 0.62;

/// An outward direction of the cube: one non-zero component is a face, two an edge, three a corner.
pub type CubeTarget = [i8; 3];

/// A face that is turned towards the viewer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubeFace {
    /// The outward normal.
    pub normal: CubeTarget,
    /// Its four corners on screen, in cube units with x to the right and y up.
    pub corners: [[f64; 2]; 4],
    /// Its centre on screen.
    pub centre: [f64; 2],
}

fn on_screen(camera: &Camera, point: DVec3) -> [f64; 2] {
    [point.dot(camera.right()), point.dot(camera.up())]
}

fn vector(target: CubeTarget) -> DVec3 {
    DVec3::new(f64::from(target[0]), f64::from(target[1]), f64::from(target[2]))
}

/// Where the middle of a target appears on screen (cube units, x right, y up).
pub fn screen_of(camera: &Camera, target: CubeTarget) -> [f64; 2] {
    on_screen(camera, vector(target))
}

/// The faces turned towards the viewer (at most three), farthest first.
pub fn visible_faces(camera: &Camera) -> Vec<CubeFace> {
    let forward = camera.forward();
    let mut faces = Vec::new();
    for axis in 0..3 {
        for sign in [-1_i8, 1] {
            let mut normal = [0_i8; 3];
            normal[axis] = sign;
            let n = vector(normal);
            if n.dot(forward) > -1e-6 {
                continue;
            }
            // Two directions spanning the face, ordered so that the corners go round it.
            let mut u = [0_i8; 3];
            let mut v = [0_i8; 3];
            u[(axis + 1) % 3] = 1;
            v[(axis + 2) % 3] = 1;
            let (u, v) = (vector(u), vector(v));
            let corners =
                [n - u - v, n + u - v, n + u + v, n - u + v].map(|corner| on_screen(camera, corner));
            faces.push((n.dot(forward), CubeFace { normal, corners, centre: on_screen(camera, n) }));
        }
    }
    // The face most squarely towards the viewer is drawn last.
    faces.sort_by(|a, b| b.0.total_cmp(&a.0));
    faces.into_iter().map(|(_, face)| face).collect()
}

/// The target under a screen position (cube units, x right, y up), or `None` beside the cube.
pub fn pick(camera: &Camera, at: [f64; 2]) -> Option<CubeTarget> {
    let forward = camera.forward();
    let origin = camera.right() * at[0] + camera.up() * at[1] - forward * 10.0;
    // Where the ray along the view direction enters the cube (slab test).
    let (mut enter, mut leave) = (f64::NEG_INFINITY, f64::INFINITY);
    for axis in 0..3 {
        let (o, d) = (origin.to_array()[axis], forward.to_array()[axis]);
        if d.abs() < 1e-12 {
            if o.abs() > 1.0 {
                return None;
            }
            continue;
        }
        let (a, b) = ((-1.0 - o) / d, (1.0 - o) / d);
        enter = enter.max(a.min(b));
        leave = leave.min(a.max(b));
    }
    if enter > leave {
        return None;
    }
    let hit = origin + forward * enter;
    #[allow(clippy::cast_possible_truncation, reason = "the value is -1, 0 or 1")]
    let target = hit.to_array().map(|c| if c.abs() > BORDER { c.signum() as i8 } else { 0 });
    (target != [0; 3]).then_some(target)
}

/// Turns the camera to look at the cube from `target`: straight at a face, or along an edge or corner
/// diagonal. Top and bottom keep +Y and -Y up, everything else keeps Z up.
pub fn look(camera: &mut Camera, target: CubeTarget) {
    let outward = vector(target);
    let up = match target {
        [0, 0, 1] => DVec3::Y,
        [0, 0, -1] => -DVec3::Y,
        _ => DVec3::Z,
    };
    camera.look_along(-outward, up);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StandardView;

    fn close(a: DVec3, b: DVec3) -> bool {
        (a - b).length() < 1e-9
    }

    #[test]
    fn the_home_view_shows_three_faces_and_its_centre_is_the_near_corner() {
        let camera = Camera::default(); // looks along (-1, 1, -1): the corner at (1, -1, 1) is nearest
        let faces = visible_faces(&camera);
        let mut normals: Vec<CubeTarget> = faces.iter().map(|f| f.normal).collect();
        normals.sort_unstable();
        assert_eq!(normals, vec![[0, -1, 0], [0, 0, 1], [1, 0, 0]]);
        assert_eq!(pick(&camera, [0.0, 0.0]), Some([1, -1, 1]));
        assert_eq!(pick(&camera, [5.0, 0.0]), None);
        // The middle of each visible face picks that face.
        for face in &faces {
            assert_eq!(pick(&camera, face.centre), Some(face.normal));
        }
    }

    #[test]
    fn a_face_view_has_one_face_with_edges_and_corners_around_it() {
        let mut camera = Camera::default();
        camera.look(StandardView::Front); // looking along +Y at the face whose normal is -Y
        let faces = visible_faces(&camera);
        assert_eq!(faces.len(), 1);
        assert_eq!(faces[0].normal, [0, -1, 0]);
        assert_eq!(pick(&camera, [0.0, 0.0]), Some([0, -1, 0]));
        assert_eq!(pick(&camera, [0.9, 0.0]), Some([1, -1, 0]), "right edge");
        assert_eq!(pick(&camera, [0.0, 0.9]), Some([0, -1, 1]), "top edge");
        assert_eq!(pick(&camera, [-0.9, -0.9]), Some([-1, -1, -1]), "bottom-left corner");
        assert_eq!(pick(&camera, [1.1, 0.0]), None);
    }

    #[test]
    fn every_target_turns_the_view_to_face_it() {
        for x in -1..=1_i8 {
            for y in -1..=1_i8 {
                for z in -1..=1_i8 {
                    let target = [x, y, z];
                    if target == [0; 3] {
                        continue;
                    }
                    let mut camera = Camera::default();
                    look(&mut camera, target);
                    let outward = vector(target).normalize();
                    assert!(close(camera.forward(), -outward), "{target:?}");
                    // After turning, the target is what lies under the middle of the cube.
                    assert_eq!(pick(&camera, [0.0, 0.0]), Some(target));
                    // The view is level: the screen's right is horizontal, except looking straight down
                    // or up, where the documented up direction applies.
                    match target {
                        [0, 0, 1] => assert!(close(camera.up(), DVec3::Y)),
                        [0, 0, -1] => assert!(close(camera.up(), -DVec3::Y)),
                        _ => assert!(camera.right().z.abs() < 1e-9 && camera.up().z > 0.0, "{target:?}"),
                    }
                }
            }
        }
    }
}
