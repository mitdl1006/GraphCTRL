//! Work features in the 3D view: axes as long lines, planes as outlined squares, points as small crosses.
//! (Inventor 2026 help, To Create Work Features, GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385.)

use egui::{Color32, Painter, Pos2, Stroke};
use gc_doc::WorkShape;
use gc_math::DVec3;
use gc_session::Session;

/// Work features are drawn in this colour: teal, apart from the red, green and blue of the origin axes.
const COLOR: Color32 = Color32::from_rgb(0x0E, 0x9F, 0x9A);

/// Draws the visible work features. `size` is half the length of an axis, metres.
pub(crate) fn paint(
    painter: &Painter,
    screen: &impl Fn(DVec3) -> Option<Pos2>,
    session: &Session,
    size: f64,
    selected: Option<gc_core::Id<gc_doc::WorkFeature>>,
) {
    for (id, feature) in session.doc().work_features().iter() {
        if !feature.visible {
            continue;
        }
        let Some(shape) = session.work_shape(id) else { continue };
        let picked = selected == Some(id);
        let stroke = Stroke::new(
            if picked { 3.0 } else { 1.6 },
            if picked { Color32::from_rgb(0xE0, 0x7A, 0x10) } else { COLOR },
        );
        match shape {
            WorkShape::Axis { point, direction } => {
                if let (Some(a), Some(b)) =
                    (screen(point - direction * size), screen(point + direction * size))
                {
                    painter.line_segment([a, b], stroke);
                    painter.text(
                        b,
                        egui::Align2::LEFT_BOTTOM,
                        &feature.name,
                        egui::FontId::proportional(11.0),
                        COLOR,
                    );
                }
            }
            WorkShape::Plane(pose) => {
                let half = size * 0.5;
                let corner = |x: f64, y: f64| screen(pose * DVec3::new(x, y, 0.0));
                let corners =
                    [corner(-half, -half), corner(half, -half), corner(half, half), corner(-half, half)];
                if let [Some(a), Some(b), Some(c), Some(d)] = corners {
                    painter.add(egui::Shape::closed_line(vec![a, b, c, d], stroke));
                }
            }
            WorkShape::Point(point) => {
                if let Some(at) = screen(point) {
                    painter.line_segment([at - egui::vec2(4.0, 0.0), at + egui::vec2(4.0, 0.0)], stroke);
                    painter.line_segment([at - egui::vec2(0.0, 4.0), at + egui::vec2(0.0, 4.0)], stroke);
                }
            }
        }
    }
}

/// The work feature under `at` (view pixels), if one is within a few pixels of its line, outline or point.
pub(crate) fn pick(
    session: &Session,
    screen: &impl Fn(DVec3) -> Option<[f64; 2]>,
    at: [f64; 2],
    size: f64,
) -> Option<gc_core::Id<gc_doc::WorkFeature>> {
    const TOLERANCE: f64 = 6.0;
    let near = |a: Option<[f64; 2]>, b: Option<[f64; 2]>| match (a, b) {
        (Some(a), Some(b)) => crate::segment_distance(at, a, b) <= TOLERANCE,
        _ => false,
    };
    session.doc().work_features().iter().find_map(|(id, feature)| {
        if !feature.visible {
            return None;
        }
        let hit = match session.work_shape(id)? {
            WorkShape::Axis { point, direction } => {
                near(screen(point - direction * size), screen(point + direction * size))
            }
            WorkShape::Plane(pose) => {
                let half = size * 0.5;
                let corner = |x: f64, y: f64| screen(pose * DVec3::new(x, y, 0.0));
                let c = [corner(-half, -half), corner(half, -half), corner(half, half), corner(-half, half)];
                (0..4).any(|i| near(c[i], c[(i + 1) % 4]))
            }
            WorkShape::Point(point) => near(screen(point), screen(point)),
        };
        hit.then_some(id)
    })
}
