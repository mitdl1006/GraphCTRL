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
