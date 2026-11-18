//! Icons drawn with a few strokes, so they stay sharp at any scale and need no image files.

use egui::{Color32, Painter, Pos2, Rect, Stroke, pos2};

/// The icons of the ribbon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// An arrow cursor.
    Select,
    /// A diagonal segment with end points.
    Line,
    /// A circle with its centre.
    Circle,
    /// A quarter arc with its centre.
    Arc,
    /// A rectangle.
    Rectangle,
    /// A dimension line with arrows.
    Dimension,
    /// A sketch on a plane.
    Sketch,
    /// A check mark.
    Finish,
    /// A cube seen from a corner.
    View,
    /// A robot arm: base, two links and a gripper.
    Robot,
    /// A box seen from a corner.
    Cuboid,
    /// An upright cylinder.
    Cylinder,
    /// A sphere with its equator.
    Sphere,
    /// A waypoint: a diamond with the tool's axis.
    Waypoint,
    /// Three axes from an origin.
    Frame,
    /// A path through three points.
    Path,
    /// A play triangle.
    Play,
    /// An arrow into a tray.
    Import,
    /// A house.
    Home,
    /// A ring with an arrow: orbit.
    Orbit,
    /// Four arrows: pan.
    Pan,
    /// A magnifier: zoom.
    Zoom,
    /// Corner brackets round a box: zoom all.
    ZoomAll,
    /// A face seen straight on with an eye line: look at.
    LookAt,
    /// A box in perspective, its far face smaller.
    Perspective,
    /// A sketch point: a dot in a small cross.
    Point,
    /// A hexagon.
    Polygon,
    /// A slot: two half circles joined by straight sides.
    Slot,
    /// A grid of small squares: rectangular pattern.
    RectPattern,
    /// Dots on a circle around a centre: circular pattern.
    CircPattern,
    /// A dashed axis between a shape and its mirror image.
    Mirror,
    /// Two nested outlines: offset.
    Offset,
    /// A box cut by a plane: Project Cut Edges.
    CutEdges,
    /// A ground symbol: a stem over three shrinking lines.
    Ground,
    /// A padlock: a locked joint.
    Lock,
    /// A slanted plane with its centre marked: a work plane.
    WorkPlane,
    /// A long centre line with its end points: a work axis.
    WorkAxis,
    /// A point in a ring with a cross: a work point.
    WorkPoint,
    /// A block moving along a reference line: Grip Snap.
    GripSnap,
    /// A short text, for constraint symbols.
    Text(&'static str),
}

impl Icon {
    /// Paints the icon into `rect`.
    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32) {
        let stroke = Stroke::new(1.6, color);
        // Icon coordinates run from 0 to 1 in both directions.
        let at = |x: f32, y: f32| pos2(rect.left() + x * rect.width(), rect.top() + y * rect.height());
        let line = |a: Pos2, b: Pos2| painter.line_segment([a, b], stroke);
        let dot = |p: Pos2| painter.circle_filled(p, 2.2, color);
        match self {
            Self::Select => {
                let outline = [
                    at(0.25, 0.1),
                    at(0.25, 0.8),
                    at(0.43, 0.62),
                    at(0.57, 0.92),
                    at(0.68, 0.86),
                    at(0.55, 0.57),
                    at(0.8, 0.55),
                ];
                for i in 0..outline.len() {
                    line(outline[i], outline[(i + 1) % outline.len()]);
                }
            }
            Self::Point => {
                line(at(0.5, 0.25), at(0.5, 0.4));
                line(at(0.5, 0.6), at(0.5, 0.75));
                line(at(0.25, 0.5), at(0.4, 0.5));
                line(at(0.6, 0.5), at(0.75, 0.5));
                painter.circle_filled(at(0.5, 0.5), 3.0, color);
            }
            Self::Polygon => {
                let corners: Vec<Pos2> = (0..6)
                    .map(|i| {
                        let angle = std::f32::consts::TAU * i as f32 / 6.0;
                        at(0.5 + 0.38 * angle.cos(), 0.5 + 0.38 * angle.sin())
                    })
                    .collect();
                for i in 0..corners.len() {
                    line(corners[i], corners[(i + 1) % corners.len()]);
                }
                dot(at(0.5, 0.5));
            }
            Self::Slot => {
                let radius = rect.height() * 0.2;
                let (left, right) = (at(0.32, 0.5), at(0.68, 0.5));
                line(left + egui::vec2(0.0, -radius), right + egui::vec2(0.0, -radius));
                line(left + egui::vec2(0.0, radius), right + egui::vec2(0.0, radius));
                for (centre, sign) in [(left, -1.0_f32), (right, 1.0)] {
                    let points: Vec<Pos2> = (0..=12)
                        .map(|i| {
                            let angle = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 12.0;
                            centre + egui::vec2(sign * radius * angle.cos(), radius * angle.sin())
                        })
                        .collect();
                    painter.add(egui::Shape::line(points, stroke));
                }
                dot(left);
                dot(right);
            }
            Self::RectPattern => {
                for (x, y) in [(0.15, 0.15), (0.55, 0.15), (0.15, 0.55), (0.55, 0.55)] {
                    painter.rect_stroke(
                        Rect::from_min_max(at(x, y), at(x + 0.3, y + 0.3)),
                        1.0,
                        stroke,
                        egui::StrokeKind::Inside,
                    );
                }
            }
            Self::CircPattern => {
                painter.circle_stroke(at(0.5, 0.5), rect.width() * 0.32, Stroke::new(1.0, color));
                for i in 0..6 {
                    let angle = std::f32::consts::TAU * i as f32 / 6.0;
                    painter.circle_filled(at(0.5 + 0.32 * angle.cos(), 0.5 + 0.32 * angle.sin()), 2.6, color);
                }
                dot(at(0.5, 0.5));
            }
            Self::Mirror => {
                for i in 0..5 {
                    let y = 0.1 + 0.17 * i as f32;
                    line(at(0.5, y), at(0.5, y + 0.09));
                }
                for (a, b, c) in
                    [((0.4, 0.25), (0.4, 0.75), (0.12, 0.6)), ((0.6, 0.25), (0.6, 0.75), (0.88, 0.6))]
                {
                    line(at(a.0, a.1), at(b.0, b.1));
                    line(at(b.0, b.1), at(c.0, c.1));
                    line(at(c.0, c.1), at(a.0, a.1));
                }
            }
            Self::Offset => {
                painter.rect_stroke(
                    Rect::from_min_max(at(0.32, 0.32), at(0.68, 0.68)),
                    2.0,
                    stroke,
                    egui::StrokeKind::Inside,
                );
                painter.rect_stroke(
                    Rect::from_min_max(at(0.12, 0.12), at(0.88, 0.88)),
                    5.0,
                    Stroke::new(1.0, color),
                    egui::StrokeKind::Inside,
                );
            }
            Self::CutEdges => {
                // A box, and a plane through it drawn as a slanted band edge.
                painter.rect_stroke(
                    Rect::from_min_max(at(0.25, 0.3), at(0.75, 0.85)),
                    1.0,
                    Stroke::new(1.0, color),
                    egui::StrokeKind::Inside,
                );
                line(at(0.08, 0.6), at(0.92, 0.6));
                line(at(0.25, 0.6), at(0.75, 0.6));
                dot(at(0.25, 0.6));
                dot(at(0.75, 0.6));
            }
            Self::WorkPlane => {
                let corners = [at(0.12, 0.7), at(0.62, 0.7), at(0.88, 0.3), at(0.38, 0.3)];
                painter.add(egui::Shape::closed_line(corners.to_vec(), stroke));
                dot(at(0.5, 0.5));
            }
            Self::WorkAxis => {
                // A centre line: long dash, short dash, long dash, with the end points.
                line(at(0.12, 0.88), at(0.4, 0.6));
                line(at(0.46, 0.54), at(0.54, 0.46));
                line(at(0.6, 0.4), at(0.88, 0.12));
                dot(at(0.12, 0.88));
                dot(at(0.88, 0.12));
            }
            Self::WorkPoint => {
                painter.circle_stroke(at(0.5, 0.5), rect.width() * 0.2, stroke);
                dot(at(0.5, 0.5));
                line(at(0.5, 0.12), at(0.5, 0.26));
                line(at(0.5, 0.74), at(0.5, 0.88));
                line(at(0.12, 0.5), at(0.26, 0.5));
                line(at(0.74, 0.5), at(0.88, 0.5));
            }
            Self::GripSnap => {
                painter.rect_stroke(
                    Rect::from_min_max(at(0.14, 0.18), at(0.5, 0.52)),
                    1.0,
                    stroke,
                    egui::StrokeKind::Inside,
                );
                // The reference line below with an arrow along it.
                line(at(0.1, 0.78), at(0.9, 0.78));
                line(at(0.9, 0.78), at(0.78, 0.68));
                line(at(0.9, 0.78), at(0.78, 0.88));
                dot(at(0.32, 0.52));
                line(at(0.32, 0.52), at(0.32, 0.78));
            }
            Self::Lock => {
                painter.rect_stroke(
                    Rect::from_min_max(at(0.2, 0.46), at(0.8, 0.88)),
                    2.0,
                    stroke,
                    egui::StrokeKind::Inside,
                );
                // The shackle: a half circle over the body.
                let radius = rect.width() * 0.18;
                let points: Vec<egui::Pos2> = (0..=10)
                    .map(|i| {
                        let angle = std::f32::consts::PI * i as f32 / 10.0;
                        at(0.5, 0.46) + egui::vec2(radius * angle.cos(), -radius * angle.sin())
                    })
                    .collect();
                painter.add(egui::Shape::line(points, stroke));
                dot(at(0.5, 0.67));
            }
            Self::Ground => {
                line(at(0.5, 0.15), at(0.5, 0.5));
                line(at(0.2, 0.5), at(0.8, 0.5));
                line(at(0.3, 0.65), at(0.7, 0.65));
                line(at(0.4, 0.8), at(0.6, 0.8));
            }
            Self::Line => {
                line(at(0.15, 0.85), at(0.85, 0.15));
                dot(at(0.15, 0.85));
                dot(at(0.85, 0.15));
            }
            Self::Circle => {
                painter.circle_stroke(at(0.5, 0.5), rect.width() * 0.38, stroke);
                dot(at(0.5, 0.5));
            }
            Self::Arc => {
                let radius = rect.width() * 0.62;
                let points: Vec<egui::Pos2> = (0..=12)
                    .map(|i| {
                        let angle = std::f32::consts::FRAC_PI_2 * i as f32 / 12.0;
                        at(0.18, 0.82) + egui::vec2(radius * angle.cos(), -radius * angle.sin())
                    })
                    .collect();
                painter.add(egui::Shape::line(points, stroke));
                dot(at(0.18, 0.82));
            }
            Self::Rectangle => {
                painter.rect_stroke(
                    Rect::from_min_max(at(0.12, 0.25), at(0.88, 0.75)),
                    0.0,
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
            Self::Dimension => {
                line(at(0.15, 0.2), at(0.15, 0.8));
                line(at(0.85, 0.2), at(0.85, 0.8));
                line(at(0.15, 0.5), at(0.85, 0.5));
                line(at(0.15, 0.5), at(0.3, 0.4));
                line(at(0.15, 0.5), at(0.3, 0.6));
                line(at(0.85, 0.5), at(0.7, 0.4));
                line(at(0.85, 0.5), at(0.7, 0.6));
            }
            Self::Sketch => {
                let plane = [at(0.1, 0.7), at(0.4, 0.3), at(0.9, 0.3), at(0.6, 0.7)];
                for i in 0..4 {
                    line(plane[i], plane[(i + 1) % 4]);
                }
                line(at(0.35, 0.6), at(0.65, 0.4));
            }
            Self::Finish => {
                let thick = Stroke::new(2.4, color);
                painter.line_segment([at(0.18, 0.55), at(0.42, 0.78)], thick);
                painter.line_segment([at(0.42, 0.78), at(0.85, 0.22)], thick);
            }
            Self::View => {
                let (top, mid, low) = (at(0.5, 0.1), at(0.5, 0.5), at(0.5, 0.92));
                let (left_up, right_up) = (at(0.12, 0.3), at(0.88, 0.3));
                let (left_low, right_low) = (at(0.12, 0.72), at(0.88, 0.72));
                for (a, b) in [
                    (top, left_up),
                    (top, right_up),
                    (left_up, mid),
                    (right_up, mid),
                    (mid, low),
                    (left_up, left_low),
                    (right_up, right_low),
                    (left_low, low),
                    (right_low, low),
                ] {
                    line(a, b);
                }
            }
            Self::Robot => {
                let thick = Stroke::new(2.2, color);
                painter.line_segment([at(0.15, 0.9), at(0.55, 0.9)], thick);
                painter.line_segment([at(0.35, 0.9), at(0.35, 0.62)], thick);
                painter.line_segment([at(0.35, 0.62), at(0.55, 0.28)], thick);
                painter.line_segment([at(0.55, 0.28), at(0.82, 0.42)], thick);
                painter.circle_filled(at(0.35, 0.62), 2.6, color);
                painter.circle_filled(at(0.55, 0.28), 2.6, color);
                line(at(0.82, 0.42), at(0.9, 0.34));
                line(at(0.82, 0.42), at(0.88, 0.52));
            }
            Self::Cuboid | Self::Perspective => {
                let far = if matches!(self, Self::Perspective) { 0.14 } else { 0.0 };
                let front = [at(0.12, 0.38), at(0.62, 0.38), at(0.62, 0.88), at(0.12, 0.88)];
                let back = [
                    at(0.38 + far, 0.12 + far),
                    at(0.88 - far, 0.12 + far),
                    at(0.88 - far, 0.62 - far),
                    at(0.38 + far, 0.62 - far),
                ];
                for i in 0..4 {
                    line(front[i], front[(i + 1) % 4]);
                    line(front[i], back[i]);
                }
                line(back[0], back[1]);
                line(back[1], back[2]);
            }
            Self::Cylinder => {
                let (w, h) = (rect.width() * 0.34, rect.height() * 0.1);
                for y in [0.22, 0.8] {
                    painter.add(egui::Shape::ellipse_stroke(at(0.5, y), egui::vec2(w, h), stroke));
                }
                line(at(0.16, 0.22), at(0.16, 0.8));
                line(at(0.84, 0.22), at(0.84, 0.8));
            }
            Self::Sphere => {
                painter.circle_stroke(at(0.5, 0.5), rect.width() * 0.38, stroke);
                painter.add(egui::Shape::ellipse_stroke(
                    at(0.5, 0.5),
                    egui::vec2(rect.width() * 0.38, rect.height() * 0.12),
                    stroke,
                ));
            }
            Self::Waypoint => {
                let diamond = [at(0.5, 0.2), at(0.78, 0.5), at(0.5, 0.8), at(0.22, 0.5)];
                for i in 0..4 {
                    line(diamond[i], diamond[(i + 1) % 4]);
                }
                dot(at(0.5, 0.5));
            }
            Self::Frame => {
                let o = at(0.3, 0.72);
                painter
                    .line_segment([o, at(0.9, 0.72)], Stroke::new(1.8, Color32::from_rgb(0xD0, 0x30, 0x30)));
                painter
                    .line_segment([o, at(0.3, 0.1)], Stroke::new(1.8, Color32::from_rgb(0x30, 0x50, 0xD0)));
                painter
                    .line_segment([o, at(0.62, 0.42)], Stroke::new(1.8, Color32::from_rgb(0x20, 0xA0, 0x40)));
                dot(o);
            }
            Self::Path => {
                let points = vec![at(0.12, 0.8), at(0.4, 0.3), at(0.62, 0.62), at(0.88, 0.2)];
                painter.add(egui::Shape::line(points.clone(), stroke));
                for p in points {
                    dot(p);
                }
            }
            Self::Play => {
                painter.add(egui::Shape::convex_polygon(
                    vec![at(0.3, 0.18), at(0.82, 0.5), at(0.3, 0.82)],
                    color,
                    Stroke::NONE,
                ));
            }
            Self::Import => {
                line(at(0.5, 0.1), at(0.5, 0.62));
                line(at(0.5, 0.62), at(0.32, 0.44));
                line(at(0.5, 0.62), at(0.68, 0.44));
                line(at(0.15, 0.6), at(0.15, 0.88));
                line(at(0.15, 0.88), at(0.85, 0.88));
                line(at(0.85, 0.88), at(0.85, 0.6));
            }
            Self::Home => {
                line(at(0.12, 0.5), at(0.5, 0.15));
                line(at(0.5, 0.15), at(0.88, 0.5));
                let walls = [at(0.22, 0.42), at(0.22, 0.88), at(0.78, 0.88), at(0.78, 0.42)];
                for pair in walls.windows(2) {
                    line(pair[0], pair[1]);
                }
            }
            Self::Orbit => {
                painter.circle_stroke(at(0.5, 0.5), rect.width() * 0.32, stroke);
                dot(at(0.5, 0.5));
                line(at(0.82, 0.3), at(0.9, 0.18));
                line(at(0.82, 0.3), at(0.7, 0.28));
            }
            Self::Pan => {
                line(at(0.5, 0.1), at(0.5, 0.9));
                line(at(0.1, 0.5), at(0.9, 0.5));
                for (a, b, c) in [
                    (at(0.5, 0.1), at(0.4, 0.22), at(0.6, 0.22)),
                    (at(0.5, 0.9), at(0.4, 0.78), at(0.6, 0.78)),
                    (at(0.1, 0.5), at(0.22, 0.4), at(0.22, 0.6)),
                    (at(0.9, 0.5), at(0.78, 0.4), at(0.78, 0.6)),
                ] {
                    line(a, b);
                    line(a, c);
                }
            }
            Self::Zoom => {
                painter.circle_stroke(at(0.43, 0.43), rect.width() * 0.26, stroke);
                line(at(0.62, 0.62), at(0.88, 0.88));
                line(at(0.33, 0.43), at(0.53, 0.43));
                line(at(0.43, 0.33), at(0.43, 0.53));
            }
            Self::ZoomAll => {
                for (x, y, dx, dy) in [
                    (0.12, 0.12, 1.0, 1.0),
                    (0.88, 0.12, -1.0, 1.0),
                    (0.12, 0.88, 1.0, -1.0),
                    (0.88, 0.88, -1.0, -1.0),
                ] {
                    line(at(x, y), at(x + 0.22 * dx, y));
                    line(at(x, y), at(x, y + 0.22 * dy));
                }
                painter.rect_stroke(
                    egui::Rect::from_two_pos(at(0.36, 0.36), at(0.64, 0.64)),
                    1.0,
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
            Self::LookAt => {
                painter.rect_stroke(
                    egui::Rect::from_two_pos(at(0.2, 0.25), at(0.8, 0.75)),
                    1.0,
                    stroke,
                    egui::StrokeKind::Middle,
                );
                line(at(0.5, 0.05), at(0.5, 0.25));
                line(at(0.42, 0.14), at(0.5, 0.25));
                line(at(0.58, 0.14), at(0.5, 0.25));
            }
            Self::Text(text) => {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::proportional(rect.height() * 0.62),
                    color,
                );
            }
        }
    }
}
