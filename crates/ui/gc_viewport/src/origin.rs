//! The origin folders of components in the 3D view: planes, axes and the centre point that the user made
//! visible, drawn at the component's pose, picked with the mouse (Inventor 2026 help, assembly origin
//! planes: docs/ux/inventor-assembly-parity.md A52/A53).

use egui::{Color32, Pos2, Shape, Stroke};
use gc_doc::{ComponentOrigin, Occurrence, OriginElement, OriginPlane};
use gc_draw::Camera;
use gc_math::DVec3;
use gc_session::Session;
use gc_ui::colors;

/// Half the side of a drawn origin plane, metres.
const HALF: f64 = 0.15;
/// Length of a drawn axis, metres.
const AXIS: f64 = 0.3;
/// Pick tolerances in pixels: centre point and axis lines.
const POINT_PX: f64 = 8.0;
const LINE_PX: f64 = 6.0;

/// The origin elements the user showed, with their component's origin in the world.
pub(crate) fn visible(session: &Session) -> Vec<(Occurrence, ComponentOrigin, Vec<OriginElement>)> {
    let mut found = Vec::new();
    for node in session.components() {
        let shown: Vec<OriginElement> =
            ALL.iter().copied().filter(|element| session.origin_visible(node.occurrence, *element)).collect();
        if shown.is_empty() {
            continue;
        }
        if let Some(origin) = session.component_origin(node.occurrence) {
            found.push((node.occurrence, origin, shown));
        }
    }
    found
}

/// The seven elements of an origin folder, in the order the browser lists them.
pub const ALL: [OriginElement; 7] = [
    OriginElement::Plane(OriginPlane::Yz),
    OriginElement::Plane(OriginPlane::Xz),
    OriginElement::Plane(OriginPlane::Xy),
    OriginElement::Axis(0),
    OriginElement::Axis(1),
    OriginElement::Axis(2),
    OriginElement::Point,
];

fn axis_color(axis: u8) -> Color32 {
    match axis {
        0 => Color32::from_rgb(0xD0, 0x30, 0x30),
        1 => Color32::from_rgb(0x20, 0xA0, 0x40),
        _ => Color32::from_rgb(0x30, 0x50, 0xD0),
    }
}

/// Draws the visible origin elements. `selected` is drawn in the selection colour.
pub(crate) fn paint(
    painter: &egui::Painter,
    screen: &impl Fn(DVec3) -> Option<Pos2>,
    session: &Session,
    selected: Option<(Occurrence, OriginElement)>,
) {
    for (occurrence, origin, elements) in visible(session) {
        for element in elements {
            let chosen = selected == Some((occurrence, element));
            match element {
                OriginElement::Plane(plane) => {
                    let pose = origin.plane(plane);
                    let corners: Option<Vec<Pos2>> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                        .iter()
                        .map(|(u, v)| screen(pose * DVec3::new(u * HALF, v * HALF, 0.0)))
                        .collect();
                    if let Some(corners) = corners {
                        let base = if chosen { colors::SELECTED } else { colors::ACCENT };
                        let fill = base.gamma_multiply(if chosen { 0.35 } else { 0.16 });
                        painter.add(Shape::convex_polygon(corners, fill, Stroke::new(1.2, base)));
                    }
                }
                OriginElement::Axis(axis) => {
                    let from = origin.point();
                    let to = from + origin.axis(usize::from(axis)) * AXIS;
                    if let (Some(a), Some(b)) = (screen(from), screen(to)) {
                        let color = if chosen { colors::SELECTED } else { axis_color(axis) };
                        painter.line_segment([a, b], Stroke::new(if chosen { 3.0 } else { 2.0 }, color));
                    }
                }
                OriginElement::Point => {
                    if let Some(at) = screen(origin.point()) {
                        let color = if chosen { colors::SELECTED } else { colors::text() };
                        painter.circle_filled(at, if chosen { 5.5 } else { 4.0 }, color);
                    }
                }
            }
        }
    }
}

/// The visible origin element under a screen position: the centre point first, then axes, then planes.
pub(crate) fn pick(
    camera: &Camera,
    size: [f64; 2],
    session: &Session,
    at: [f64; 2],
) -> Option<(Occurrence, OriginElement)> {
    let near =
        |p: DVec3| camera.project(p, size).map(|s| ((s[0] - at[0]).powi(2) + (s[1] - at[1]).powi(2)).sqrt());
    let shown = visible(session);
    let mut best: Option<(f64, u8, Occurrence, OriginElement)> = None;
    let mut consider = |distance: f64, rank: u8, occurrence: Occurrence, element: OriginElement| {
        if best.is_none_or(|(d, r, _, _)| (rank, distance) < (r, d)) {
            best = Some((distance, rank, occurrence, element));
        }
    };
    let ray = camera.ray(at, size);
    for (occurrence, origin, elements) in &shown {
        for element in elements {
            match *element {
                OriginElement::Point => {
                    if let Some(d) = near(origin.point()).filter(|d| *d <= POINT_PX) {
                        consider(d, 0, *occurrence, *element);
                    }
                }
                OriginElement::Axis(axis) => {
                    let from = origin.point();
                    let to = from + origin.axis(usize::from(axis)) * AXIS;
                    if let (Some(a), Some(b)) = (camera.project(from, size), camera.project(to, size)) {
                        let d = segment_distance(at, a, b);
                        if d <= LINE_PX {
                            consider(d, 1, *occurrence, *element);
                        }
                    }
                }
                OriginElement::Plane(plane) => {
                    let pose = origin.plane(plane);
                    let normal = pose.rotation * DVec3::Z;
                    let denom = normal.dot(ray.dir);
                    if denom.abs() > 1e-9 {
                        let t = normal.dot(pose.translation - ray.origin) / denom;
                        if t > 0.0 {
                            let local = pose.inverse() * (ray.origin + ray.dir * t);
                            if local.x.abs() <= HALF && local.y.abs() <= HALF {
                                consider(t, 2, *occurrence, *element);
                            }
                        }
                    }
                }
            }
        }
    }
    best.map(|(_, _, occurrence, element)| (occurrence, element))
}

/// Distance from `p` to the segment `a`–`b`, in pixels.
fn segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dy * dy;
    let t =
        if length > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length).clamp(0.0, 1.0) } else { 0.0 };
    let (x, y) = (a[0] + dx * t - p[0], a[1] + dy * t - p[1]);
    gc_math::fm::hypot(x, y)
}
