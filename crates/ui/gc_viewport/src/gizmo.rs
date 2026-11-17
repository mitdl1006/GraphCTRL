//! The move and rotate handles of a selected body or robot: three arrows along the world axes and a ring
//! about the vertical, dragged as in CAD programs. While dragging, the change is a preview; releasing
//! the mouse makes it one undo step.

use egui::{Color32, FontId, Pos2, Stroke, pos2};
use gc_doc::Placement;
use gc_draw::Camera;
use gc_math::{DVec3, Plane, fm};
use gc_session::{CellItem, Session};

/// Length of an arrow and radius of the ring, in pixels.
const ARROW_PX: f64 = 80.0;
const RING_PX: f64 = 60.0;
/// How close the cursor must be to a handle, in pixels.
const GRAB_PX: f32 = 7.0;

/// A handle of the gizmo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// Move along the world axis 0, 1 or 2.
    Move(usize),
    /// Turn about the vertical through the origin.
    Turn,
}

/// A drag in progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Drag {
    item: CellItem,
    handle: Handle,
    start: Placement,
    /// Where the drag started along the axis (metres), or the angle about the vertical (radians).
    from: f64,
}

const AXIS_COLOURS: [Color32; 3] = [
    Color32::from_rgb(0xD0, 0x30, 0x30),
    Color32::from_rgb(0x20, 0xA0, 0x40),
    Color32::from_rgb(0x30, 0x50, 0xD0),
];
const ACTIVE: Color32 = Color32::from_rgb(0xFF, 0xC0, 0x00);

fn axis(index: usize) -> DVec3 {
    [DVec3::X, DVec3::Y, DVec3::Z][index % 3]
}

/// The placement the gizmo works on.
fn placement_of(session: &Session, item: CellItem) -> Option<Placement> {
    match item {
        CellItem::Body(id) => session.shown_body(id).map(|b| b.placement),
        CellItem::Robot(id) => session.shown_base(id),
        _ => None,
    }
}

fn preview(session: &mut Session, item: CellItem, placement: Placement) {
    match item {
        CellItem::Body(id) => {
            if let Some(mut body) = session.shown_body(id).cloned() {
                body.placement = placement;
                session.preview_body(id, body);
            }
        }
        CellItem::Robot(id) => session.preview_base(id, placement),
        _ => {}
    }
}

fn commit(session: &mut Session, item: CellItem) {
    match item {
        CellItem::Body(_) => session.end_body_preview(),
        CellItem::Robot(_) => session.end_base_preview(),
        _ => {}
    }
}

/// Where along the axis through `origin` the cursor's ray passes closest, in metres from the origin.
fn along_axis(
    camera: &Camera,
    cursor: [f64; 2],
    size: [f64; 2],
    origin: DVec3,
    direction: DVec3,
) -> Option<f64> {
    let ray = camera.ray(cursor, size);
    let cosine = direction.dot(ray.dir);
    let denominator = 1.0 - cosine * cosine;
    // An axis pointing at the viewer cannot be dragged along.
    if denominator < 1e-4 {
        return None;
    }
    let offset = ray.origin - origin;
    Some((offset.dot(direction) - cosine * offset.dot(ray.dir)) / denominator)
}

/// The angle about the vertical through `origin` of where the cursor's ray meets the horizontal plane.
fn about_vertical(camera: &Camera, cursor: [f64; 2], size: [f64; 2], origin: DVec3) -> Option<f64> {
    let plane = Plane::from_point_normal(origin, DVec3::Z)?;
    let point = camera.on_plane(cursor, size, &plane)?;
    let d = point - origin;
    (d.length() > 1e-9).then(|| fm::atan2(d.y, d.x))
}

/// The handles' screen geometry: arrow tips, and points around the ring.
struct Shape {
    origin: Pos2,
    tips: [Option<Pos2>; 3],
    ring: Vec<Pos2>,
}

fn shape(camera: &Camera, rect: egui::Rect, origin: DVec3) -> Option<Shape> {
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
    let screen =
        |w: DVec3| camera.project(w, size).map(|p| pos2(rect.left() + p[0] as f32, rect.top() + p[1] as f32));
    let px = camera.px_scale(size);
    let o = screen(origin)?;
    let tips = [0, 1, 2].map(|k| screen(origin + axis(k) * ARROW_PX * px));
    let ring = (0..=48)
        .filter_map(|i| {
            let a = std::f64::consts::TAU * f64::from(i) / 48.0;
            screen(origin + DVec3::new(fm::cos(a), fm::sin(a), 0.0) * RING_PX * px)
        })
        .collect();
    Some(Shape { origin: o, tips, ring })
}

fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let (ab, ap) = (b - a, p - a);
    let t = if ab.length_sq() > 0.0 { (ap.dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t).distance(p)
}

fn handle_at(shape: &Shape, cursor: Pos2) -> Option<Handle> {
    let arrow = (0..3)
        .filter_map(|k| Some((distance_to_segment(cursor, shape.origin, shape.tips[k]?), Handle::Move(k))))
        .filter(|(d, _)| *d <= GRAB_PX)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, handle)) = arrow {
        return Some(handle);
    }
    let ring =
        shape.ring.windows(2).map(|w| distance_to_segment(cursor, w[0], w[1])).fold(f32::INFINITY, f32::min);
    (ring <= GRAB_PX).then_some(Handle::Turn)
}

/// The gizmo of the selected body or robot: drawn, and dragged. Returns true while it has the mouse, so
/// that the click is not taken as a selection.
pub(crate) fn ui(
    ui: &egui::Ui,
    response: &egui::Response,
    rect: egui::Rect,
    camera: &Camera,
    session: &mut Session,
    drag: &mut Option<Drag>,
) -> bool {
    let item = drag.map(|d| d.item).or_else(|| session.selected_item());
    let Some(item) = item else { return false };
    let Some(placement) = placement_of(session, item) else { return false };
    let origin = DVec3::from_array(placement.xyz);
    let Some(shape) = shape(camera, rect, origin) else { return false };
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let to_view = |p: Pos2| [f64::from(p.x - rect.left()), f64::from(p.y - rect.top())];
    let cursor = response.hover_pos().or_else(|| ui.input(|i| i.pointer.latest_pos()));
    let hovered = drag.map(|d| d.handle).or_else(|| cursor.and_then(|c| handle_at(&shape, c)));
    let (shift, ctrl) = ui.input(|i| (i.modifiers.shift, i.modifiers.command || i.modifiers.ctrl));

    // Start, continue and end a drag.
    if drag.is_none()
        && response.drag_started_by(egui::PointerButton::Primary)
        && let Some(start) = ui.input(|i| i.pointer.press_origin())
        && let Some(handle) = handle_at(&shape, start)
    {
        let from = match handle {
            Handle::Move(k) => along_axis(camera, to_view(start), size, origin, axis(k)),
            Handle::Turn => about_vertical(camera, to_view(start), size, origin),
        };
        if let Some(from) = from {
            *drag = Some(Drag { item, handle, start: placement, from });
        }
    }
    let mut readout = None;
    if let (Some(active), Some(at)) = (*drag, cursor) {
        let mut moved = active.start;
        match active.handle {
            Handle::Move(k) => {
                if let Some(now) =
                    along_axis(camera, to_view(at), size, DVec3::from_array(active.start.xyz), axis(k))
                {
                    // Whole millimetres; with Shift, whole centimetres; with Ctrl, free.
                    let step = if ctrl {
                        0.0
                    } else if shift {
                        0.01
                    } else {
                        0.001
                    };
                    let mut delta = now - active.from;
                    if step > 0.0 {
                        delta = (delta / step).round() * step;
                    }
                    moved.xyz[k] = active.start.xyz[k] + delta;
                    readout = Some(format!("{} {:+.1} mm", ["X", "Y", "Z"][k], delta * 1000.0));
                }
            }
            Handle::Turn => {
                if let Some(now) =
                    about_vertical(camera, to_view(at), size, DVec3::from_array(active.start.xyz))
                {
                    let mut delta = (now - active.from + std::f64::consts::PI)
                        .rem_euclid(std::f64::consts::TAU)
                        - std::f64::consts::PI;
                    // Whole degrees; with Shift, steps of 15 degrees; with Ctrl, free.
                    let step = if ctrl {
                        0.0
                    } else if shift {
                        15.0_f64.to_radians()
                    } else {
                        1.0_f64.to_radians()
                    };
                    if step > 0.0 {
                        delta = (delta / step).round() * step;
                    }
                    moved.rpy[2] = active.start.rpy[2] + delta;
                    readout = Some(format!("Rz {:+.1}°", delta.to_degrees()));
                }
            }
        }
        preview(session, active.item, moved);
        ui.ctx().request_repaint();
    }
    let releasing = drag.is_some() && !ui.input(|i| i.pointer.primary_down());
    if releasing && let Some(active) = drag.take() {
        commit(session, active.item);
    }

    // Draw: the ring, then the arrows; the handle under the cursor or being dragged in amber.
    // Above the 3D picture, which is painted after the input is handled, and below menus and windows.
    let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::unique("gc_gizmo"));
    let painter = ui.ctx().layer_painter(layer).with_clip_rect(rect);
    let colour = |handle: Handle, normal: Color32| if hovered == Some(handle) { ACTIVE } else { normal };
    let width = |handle: Handle| if hovered == Some(handle) { 3.5 } else { 2.2 };
    painter.add(egui::Shape::line(
        shape.ring.clone(),
        Stroke::new(width(Handle::Turn), colour(Handle::Turn, AXIS_COLOURS[2])),
    ));
    for k in 0..3 {
        let Some(tip) = shape.tips[k] else { continue };
        let handle = Handle::Move(k);
        let stroke = Stroke::new(width(handle), colour(handle, AXIS_COLOURS[k]));
        painter.line_segment([shape.origin, tip], stroke);
        // The arrowhead: a small triangle pointing along the shaft.
        let along = (tip - shape.origin).normalized();
        let across = along.rot90();
        let head = vec![tip + along * 9.0, tip - across * 5.0, tip + across * 5.0];
        painter.add(egui::Shape::convex_polygon(head, stroke.color, Stroke::NONE));
    }
    painter.circle_filled(shape.origin, 3.5, Color32::WHITE);
    if let (Some(text), Some(at)) = (readout, cursor) {
        let galley = painter.layout_no_wrap(text, FontId::proportional(13.0), Color32::BLACK);
        let frame =
            egui::Rect::from_min_size(at + egui::vec2(14.0, 10.0), galley.size() + egui::vec2(8.0, 4.0));
        painter.rect_filled(frame, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 230));
        painter.galley(frame.min + egui::vec2(4.0, 2.0), galley, Color32::BLACK);
    }
    drag.is_some() || hovered.is_some()
}
