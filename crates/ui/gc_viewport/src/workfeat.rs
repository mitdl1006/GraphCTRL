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
) {
    for (id, feature) in session.doc().work_features().iter() {
        if !feature.visible {
            continue;
        }
        let Some(shape) = session.work_shape(id) else { continue };
        let stroke = Stroke::new(1.6, COLOR);
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
