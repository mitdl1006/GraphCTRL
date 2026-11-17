//! Drawing the scene with egui's painter: grid, axes, sketches, dimensions, constraint glyphs and tool
//! previews.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, pos2, vec2};
use gc_draw::Camera;
use gc_math::{DVec3, fm};
use gc_scene::Scene;
use gc_session::Session;
use gc_sketch::{Curve, CurveGeom, DimKind, Dimension, GeoConstraint, Item, Sketch};
use gc_tools::SnapKind;
use gc_ui::colors;

use crate::{SketchPlane, plane_of};

/// A length for display: millimetres with at most three decimals.
pub fn format_length(metres: f64) -> String {
    format!("{} mm", trim(metres * 1000.0))
}

/// An angle for display: degrees with at most three decimals.
pub fn format_angle(radians: f64) -> String {
    format!("{} deg", trim(radians.to_degrees()))
}

fn trim(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".to_owned() } else { text.to_owned() }
}

/// Everything needed to turn sketch coordinates into screen positions.
struct View<'a> {
    painter: Painter,
    rect: Rect,
    camera: &'a Camera,
    size: [f64; 2],
}

impl View<'_> {
    fn screen(&self, world: DVec3) -> Option<Pos2> {
        let p = self.camera.project(world, self.size)?;
        #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
        Some(pos2(self.rect.left() + p[0] as f32, self.rect.top() + p[1] as f32))
    }

    fn segment(&self, a: DVec3, b: DVec3, stroke: Stroke) {
        if let (Some(a), Some(b)) = (self.screen(a), self.screen(b)) {
            self.painter.line_segment([a, b], stroke);
        }
    }

    fn polyline(&self, points: impl Iterator<Item = DVec3>, stroke: Stroke) {
        let screen: Vec<Pos2> = points.filter_map(|p| self.screen(p)).collect();
        if screen.len() >= 2 {
            self.painter.add(egui::Shape::line(screen, stroke));
        }
    }

    fn text(&self, at: Pos2, anchor: Align2, text: &str, size: f32, color: Color32) {
        self.painter.text(at, anchor, text, FontId::proportional(size), color);
    }
}

fn view_of<'a>(ui: &egui::Ui, rect: Rect, camera: &'a Camera) -> View<'a> {
    View {
        painter: ui.painter_at(rect),
        rect,
        camera,
        size: [f64::from(rect.width()), f64::from(rect.height())],
    }
}

/// Paints what lies under the solids: the background and the grid.
pub(crate) fn under(ui: &egui::Ui, rect: Rect, camera: &Camera, session: &Session) {
    let view = view_of(ui, rect, camera);
    view.painter.rect_filled(rect, 0.0, colors::BACKGROUND_TOP);
    let world_xy = SketchPlane { origin: DVec3::ZERO, u: DVec3::X, v: DVec3::Y, normal: DVec3::Z };
    let grid_plane = match session.active_sketch() {
        // A 3D sketch shows its working plane.
        Some((_, sketch)) if sketch.is_spatial() => {
            let frame = plane_of(session, sketch);
            let axis = |k: usize| [frame.u, frame.v, frame.normal][k % 3];
            let normal = session.spatial_plane();
            SketchPlane {
                origin: frame.origin,
                u: axis(normal + 1),
                v: axis(normal + 2),
                normal: axis(normal),
            }
        }
        Some((_, sketch)) => plane_of(session, sketch),
        None => world_xy,
    };
    grid(&view, &grid_plane);
}

/// Paints what lies over the solids: axes, sketches, traces, waypoints and tool previews.
pub(crate) fn over(ui: &egui::Ui, rect: Rect, camera: &Camera, session: &Session, scene: &Scene, gpu: bool) {
    let view = view_of(ui, rect, camera);
    let active = session.active_sketch();
    axes(&view);
    for (id, sketch) in session.doc().sketches().iter() {
        let plane = plane_of(session, sketch);
        if active.is_some_and(|(active_id, _)| active_id == id) {
            active_sketch(&view, &plane, sketch, session);
        } else {
            let stroke = Stroke::new(1.2, colors::INACTIVE);
            for curve in sketch.curves.values() {
                curve_path(&view, &plane, curve, |p| sketch.pos(p), None, stroke);
            }
        }
    }
    cell(&view, scene, gpu);
}

/// The axes of a pose: X red, Y green, Z blue, `length` metres long.
fn pose_axes(view: &View<'_>, pose: &gc_math::DPose3, length: f64, width: f32) {
    for (axis, color) in [
        (DVec3::X, Color32::from_rgb(0xD0, 0x30, 0x30)),
        (DVec3::Y, Color32::from_rgb(0x20, 0xA0, 0x40)),
        (DVec3::Z, Color32::from_rgb(0x30, 0x50, 0xD0)),
    ] {
        view.segment(pose.translation, *pose * (axis * length), Stroke::new(width, color));
    }
}

/// Tool-tip traces of planned paths and the waypoints.
fn cell(view: &View<'_>, scene: &Scene, gpu: bool) {
    // With a GPU the traces are part of the 3D picture; without one they are drawn here, on top.
    for trace in scene.traces.iter().filter(|_| !gpu) {
        let color = if trace.current { colors::TRACE } else { colors::INACTIVE };
        view.polyline(trace.points.iter().copied(), Stroke::new(2.0, color));
    }
    // Work frames: larger axes with the frame's name.
    for (name, frame) in &scene.frames {
        pose_axes(view, frame, 0.1, 2.5);
        if let Some(at) = view.screen(frame.translation) {
            view.text(pos2(at.x - 6.0, at.y + 6.0), Align2::RIGHT_TOP, name, 13.0, colors::DIMENSION);
        }
    }
    // The tool centre point of each robot: a small frame, X red, Y green, Z blue.
    for tool in &scene.tool_points {
        pose_axes(view, tool, 0.05, 2.0);
    }
    for waypoint in &scene.waypoints {
        // The tool's axes at the waypoint: where it points (Z) and how it is turned about that.
        // A constant size on screen, so that the axes stay readable whatever the zoom.
        let length = view.camera.px_scale(view.size) * 44.0;
        let pose = gc_math::DPose3::from_parts(waypoint.position, waypoint.rotation);
        pose_axes(view, &pose, length, 2.0);
        for (axis, name, color) in [
            (DVec3::X, "X", Color32::from_rgb(0xD0, 0x30, 0x30)),
            (DVec3::Y, "Y", Color32::from_rgb(0x20, 0xA0, 0x40)),
            (DVec3::Z, "Z", Color32::from_rgb(0x30, 0x50, 0xD0)),
        ] {
            if let Some(end) = view.screen(pose * (axis * length * 1.18)) {
                view.text(end, Align2::CENTER_CENTER, name, 10.0, color);
            }
        }
        let Some(at) = view.screen(waypoint.position) else { continue };
        let r = 5.0;
        let diamond =
            vec![pos2(at.x, at.y - r), pos2(at.x + r, at.y), pos2(at.x, at.y + r), pos2(at.x - r, at.y)];
        view.painter.add(egui::Shape::convex_polygon(
            diamond,
            colors::WAYPOINT,
            Stroke::new(1.0, Color32::WHITE),
        ));
        view.text(pos2(at.x + 8.0, at.y - 8.0), Align2::LEFT_BOTTOM, &waypoint.name, 13.0, colors::WAYPOINT);
    }
}

/// Grid spacing for a given size of one pixel: a 1-2-5 step that is at least about 40 pixels wide.
fn grid_step(px: f64) -> f64 {
    let wanted = px * 40.0;
    let decade = fm::powf(10.0, wanted.log10().floor());
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * decade).find(|s| *s >= wanted).unwrap_or(10.0 * decade)
}

fn grid(view: &View<'_>, plane: &SketchPlane) {
    let Some(geometric) = plane.plane() else { return };
    let corners = [[0.0, 0.0], [view.size[0], 0.0], [view.size[0], view.size[1]], [0.0, view.size[1]]];
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    for corner in corners {
        // No grid when the plane is seen (nearly) edge-on.
        let Some(world) = view.camera.on_plane(corner, view.size, &geometric) else { return };
        let local = plane.local(world);
        for axis in 0..2 {
            low[axis] = low[axis].min(local[axis]);
            high[axis] = high[axis].max(local[axis]);
        }
    }
    let step = grid_step(view.camera.px_scale(view.size));
    for axis in 0..2 {
        let (first, last) = ((low[axis] / step).floor(), (high[axis] / step).ceil());
        if last - first > 400.0 {
            return;
        }
        #[allow(clippy::cast_possible_truncation, reason = "bounded by the check above")]
        for index in first as i64..=last as i64 {
            let at = index as f64 * step;
            let major = index % 5 == 0;
            let stroke = Stroke::new(
                if major { 1.0 } else { 0.5 },
                if major { colors::GRID_MAJOR } else { colors::GRID },
            );
            let other = 1 - axis;
            let mut a = [0.0; 3];
            let mut b = [0.0; 3];
            a[axis] = at;
            b[axis] = at;
            a[other] = low[other];
            b[other] = high[other];
            view.segment(plane.world(a), plane.world(b), stroke);
        }
    }
}

fn axes(view: &View<'_>) {
    let length = view.camera.px_scale(view.size) * 70.0;
    for (axis, color, name) in [
        (DVec3::X, Color32::from_rgb(0xD0, 0x30, 0x30), "X"),
        (DVec3::Y, Color32::from_rgb(0x30, 0xA0, 0x30), "Y"),
        (DVec3::Z, Color32::from_rgb(0x30, 0x50, 0xD0), "Z"),
    ] {
        view.segment(DVec3::ZERO, axis * length, Stroke::new(2.0, color));
        if let Some(tip) = view.screen(axis * length * 1.12) {
            view.text(tip, Align2::CENTER_CENTER, name, 12.0, color);
        }
    }
}

/// Draws one curve. `radius` overrides the stored radius of a circle (drag preview).
fn curve_path(
    view: &View<'_>,
    plane: &SketchPlane,
    curve: &Curve,
    pos: impl Fn(gc_core::Id<gc_sketch::SkPoint>) -> Option<[f64; 3]>,
    radius: Option<f64>,
    stroke: Stroke,
) {
    const SEGMENTS: usize = 72;
    let tau = std::f64::consts::TAU;
    let ring = |c: [f64; 3], r: f64, from: f64, sweep: f64| {
        (0..=SEGMENTS).map(move |i| {
            let angle = from + sweep * i as f64 / SEGMENTS as f64;
            plane.world([c[0] + r * fm::cos(angle), c[1] + r * fm::sin(angle), 0.0])
        })
    };
    match curve.geom {
        CurveGeom::Line { a, b } => {
            if let (Some(a), Some(b)) = (pos(a), pos(b)) {
                view.segment(plane.world(a), plane.world(b), stroke);
            }
        }
        CurveGeom::Circle { c, r } => {
            if let Some(c) = pos(c) {
                view.polyline(ring(c, radius.unwrap_or(r), 0.0, tau), stroke);
            }
        }
        CurveGeom::Arc { c, s, e } => {
            if let (Some(c), Some(s), Some(e)) = (pos(c), pos(s), pos(e)) {
                let angle = |p: [f64; 3]| fm::atan2(p[1] - c[1], p[0] - c[0]);
                let r = ((s[0] - c[0]).powi(2) + (s[1] - c[1]).powi(2)).sqrt();
                view.polyline(ring(c, r, angle(s), (angle(e) - angle(s)).rem_euclid(tau)), stroke);
            }
        }
    }
}

fn active_sketch(view: &View<'_>, plane: &SketchPlane, sketch: &Sketch, session: &Session) {
    let pos = |p| session.point_pos(p);
    let report = session.report();
    let selection = session.selection();
    for (id, curve) in &sketch.curves {
        let free = report.is_none_or(|r| r.curve_is_free(sketch, *id));
        let color = if selection.contains(&Item::Curve(*id)) {
            colors::SELECTED
        } else if curve.construction {
            colors::CONSTRUCTION
        } else if free {
            colors::FREE
        } else {
            colors::CONSTRAINED
        };
        let width = if selection.contains(&Item::Curve(*id)) { 2.6 } else { 1.8 };
        curve_path(view, plane, curve, pos, session.radius(*id), Stroke::new(width, color));
    }
    for id in sketch.points.keys() {
        let Some(at) = pos(*id).and_then(|p| view.screen(plane.world(p))) else { continue };
        let selected = selection.contains(&Item::Point(*id));
        let free = report.is_none_or(|r| r.point_is_free(*id));
        let color = if selected {
            colors::SELECTED
        } else if free {
            colors::FREE
        } else {
            colors::CONSTRAINED
        };
        view.painter.rect_filled(
            Rect::from_center_size(at, vec2(5.0, 5.0) * if selected { 1.6 } else { 1.0 }),
            0.0,
            color,
        );
    }
    if sketch.is_spatial() {
        spatial_marks(view, plane, sketch, session);
        return;
    }
    glyphs(view, plane, sketch, session);
    for (id, dimension) in &sketch.dimensions {
        let selected = selection.contains(&Item::Dimension(*id));
        dimension_marks(view, plane, sketch, dimension, session, selected);
    }
    preview(view, plane, session);
}

/// Where the value of a dimension of a 3D sketch is shown, in sketch coordinates: half-way along what
/// it measures. `None` for dimensions of 2D sketches, which carry their own text position.
pub(crate) fn dimension_anchor(
    session: &Session,
    sketch: &Sketch,
    kind: &gc_sketch::DimKind,
) -> Option<[f64; 3]> {
    if !sketch.is_spatial() {
        return None;
    }
    let (from, to) = match *kind {
        gc_sketch::DimKind::Distance { p, q } => (session.point_pos(p)?, session.point_pos(q)?),
        gc_sketch::DimKind::PlaneDistance { p, axis, .. } => {
            let at = session.point_pos(p)?;
            let mut foot = at;
            foot[axis % 3] = 0.0;
            (at, foot)
        }
        _ => return None,
    };
    Some([(from[0] + to[0]) * 0.5, (from[1] + to[1]) * 0.5, (from[2] + to[2]) * 0.5])
}

/// Dimensions and the line in progress of a 3D sketch.
fn spatial_marks(view: &View<'_>, plane: &SketchPlane, sketch: &Sketch, session: &Session) {
    let selection = session.selection();
    for (id, dimension) in &sketch.dimensions {
        let Some(anchor) = dimension_anchor(session, sketch, &dimension.kind) else { continue };
        let color = if selection.contains(&Item::Dimension(*id)) {
            colors::SELECTED
        } else if dimension.driving {
            colors::DIMENSION
        } else {
            colors::REFERENCE
        };
        // A height is drawn down to the plane it is measured from.
        if let gc_sketch::DimKind::PlaneDistance { p, axis, .. } = dimension.kind
            && let Some(at) = session.point_pos(p)
        {
            let mut foot = at;
            foot[axis % 3] = 0.0;
            view.segment(plane.world(at), plane.world(foot), Stroke::new(1.0, color));
        }
        let value = sketch.measure(&dimension.kind).unwrap_or(dimension.value);
        let text =
            if dimension.driving { format_length(value) } else { format!("({})", format_length(value)) };
        if let Some(at) = view.screen(plane.world(anchor)) {
            let font = FontId::proportional(13.0);
            let galley = view.painter.layout_no_wrap(text, font, color);
            let frame = Rect::from_center_size(at, galley.size() + vec2(6.0, 2.0));
            view.painter.rect_filled(frame, 2.0, Color32::from_white_alpha(215));
            view.painter.galley(frame.min + vec2(3.0, 1.0), galley, color);
        }
    }
    if let Some((from, to, axis)) = session.spatial_preview() {
        // Locked to an axis, the rubber band takes the axis' colour and name.
        let (color, name) = match axis {
            Some(0) => (Color32::from_rgb(0xD0, 0x30, 0x30), "X"),
            Some(1) => (Color32::from_rgb(0x20, 0xA0, 0x40), "Y"),
            Some(_) => (Color32::from_rgb(0x30, 0x50, 0xD0), "Z"),
            None => (colors::PREVIEW, ""),
        };
        view.segment(plane.world(from), plane.world(to), Stroke::new(1.6, color));
        if let Some(at) = view.screen(plane.world(to)) {
            view.painter.circle_stroke(at, 4.0, Stroke::new(1.4, color));
            view.text(pos2(at.x + 10.0, at.y - 10.0), Align2::LEFT_BOTTOM, name, 13.0, color);
        }
    }
}

/// Small letters next to the geometry that say which constraints apply.
fn glyphs(view: &View<'_>, plane: &SketchPlane, sketch: &Sketch, session: &Session) {
    let point = |p| session.point_pos(p).map(|p| plane.world(p));
    let middle = |curve: gc_core::Id<Curve>| -> Option<DVec3> {
        match sketch.curves.get(&curve)?.geom {
            CurveGeom::Line { a, b } => Some((point(a)? + point(b)?) * 0.5),
            CurveGeom::Circle { c, .. } | CurveGeom::Arc { c, .. } => {
                let r = session.radius(curve)?;
                Some(point(c)? + (plane.u + plane.v) * (r * std::f64::consts::FRAC_1_SQRT_2))
            }
        }
    };
    let mut used: std::collections::BTreeMap<(i32, i32), f32> = std::collections::BTreeMap::new();
    for (id, constraint) in &sketch.constraints {
        let (label, anchor) = match *constraint {
            GeoConstraint::Coincident(p, _) => {
                if let Some(at) = point(p).and_then(|w| view.screen(w)) {
                    view.painter.circle_stroke(at, 4.5, Stroke::new(1.2, colors::GLYPH));
                }
                continue;
            }
            GeoConstraint::Horizontal(p, q) => ("H", point(p).zip(point(q)).map(|(a, b)| (a + b) * 0.5)),
            GeoConstraint::Vertical(p, q) => ("V", point(p).zip(point(q)).map(|(a, b)| (a + b) * 0.5)),
            GeoConstraint::PointOnCurve(p, _) => ("on", point(p)),
            GeoConstraint::Midpoint(p, _) => ("M", point(p)),
            GeoConstraint::Parallel(_, b) => ("//", middle(b)),
            GeoConstraint::Perpendicular(_, b) => ("_|_", middle(b)),
            GeoConstraint::Collinear(_, b) => ("col", middle(b)),
            GeoConstraint::Tangent { b, .. } => ("T", middle(b)),
            GeoConstraint::Equal(_, b) => ("=", middle(b)),
            GeoConstraint::Concentric(_, b) => ("O", middle(b)),
            GeoConstraint::Symmetric { p, .. } => ("sym", point(p)),
            GeoConstraint::Fix(p) => ("fix", point(p)),
            GeoConstraint::AlongAxis { q, .. } => ("ax", point(q)),
            GeoConstraint::OnPlane { p, .. } => ("pl", point(p)),
        };
        let Some(at) = anchor.and_then(|w| view.screen(w)) else { continue };
        // Glyphs that land on the same spot are stacked instead of drawn over each other.
        #[allow(clippy::cast_possible_truncation, reason = "coarse screen cell")]
        let cell = ((at.x / 16.0) as i32, (at.y / 16.0) as i32);
        let stack = used.entry(cell).or_insert(0.0);
        let color = if session.selection().contains(&Item::Constraint(*id)) {
            colors::SELECTED
        } else {
            colors::GLYPH
        };
        view.text(at + vec2(8.0 + *stack, -9.0), Align2::LEFT_BOTTOM, label, 11.0, color);
        *stack += 16.0;
    }
}

fn dimension_marks(
    view: &View<'_>,
    plane: &SketchPlane,
    sketch: &Sketch,
    dimension: &Dimension,
    session: &Session,
    selected: bool,
) {
    let color = if selected {
        colors::SELECTED
    } else if dimension.driving {
        colors::DIMENSION
    } else {
        colors::REFERENCE
    };
    let stroke = Stroke::new(1.0, color);
    let xy = |p| session.point_pos(p).map(|p| [p[0], p[1]]);
    let world = |p: [f64; 2]| plane.world([p[0], p[1], 0.0]);
    let text_at = dimension.text_pos;
    // A dimension line through the text, parallel to `along`, with extension lines from both points.
    let linear = |p: [f64; 2], q: [f64; 2], along: [f64; 2]| {
        let length = (along[0] * along[0] + along[1] * along[1]).sqrt();
        if length == 0.0 {
            return;
        }
        let d = [along[0] / length, along[1] / length];
        // Project both points onto the dimension line.
        let on_line = |point: [f64; 2]| {
            let t = (point[0] - text_at[0]) * d[0] + (point[1] - text_at[1]) * d[1];
            [text_at[0] + d[0] * t, text_at[1] + d[1] * t]
        };
        let (a, b) = (on_line(p), on_line(q));
        view.segment(world(p), world(a), stroke);
        view.segment(world(q), world(b), stroke);
        view.segment(world(a), world(b), stroke);
    };
    match dimension.kind {
        DimKind::Distance { p, q } => {
            if let (Some(p), Some(q)) = (xy(p), xy(q)) {
                linear(p, q, [q[0] - p[0], q[1] - p[1]]);
            }
        }
        DimKind::HDistance { p, q, .. } => {
            if let (Some(p), Some(q)) = (xy(p), xy(q)) {
                linear(p, q, [1.0, 0.0]);
            }
        }
        DimKind::VDistance { p, q, .. } => {
            if let (Some(p), Some(q)) = (xy(p), xy(q)) {
                linear(p, q, [0.0, 1.0]);
            }
        }
        DimKind::PointLine { p, line, .. } => {
            if let (Some(p), Some((a, b))) = (xy(p), sketch.line(line))
                && let (Some(a), Some(b)) = (xy(a), xy(b))
            {
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let length2 = dx * dx + dy * dy;
                if length2 > 0.0 {
                    let t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length2;
                    let foot = [a[0] + t * dx, a[1] + t * dy];
                    linear(foot, p, [p[0] - foot[0], p[1] - foot[1]]);
                }
            }
        }
        DimKind::Radius(curve) | DimKind::Diameter(curve) => {
            if let Some(centre) = sketch.round(curve).and_then(|(c, _)| xy(c)) {
                view.segment(world(centre), world(text_at), stroke);
            }
        }
        DimKind::Angle { .. } | DimKind::PlaneDistance { .. } => {}
    }
    let Some(value) =
        (if dimension.driving { Some(dimension.value) } else { sketch.measure(&dimension.kind) })
    else {
        return;
    };
    let mut label = match dimension.kind {
        DimKind::Angle { .. } => format_angle(value),
        DimKind::Radius(_) => format!("R {}", format_length(value)),
        DimKind::Diameter(_) => format!("D {}", format_length(value)),
        _ => format_length(value),
    };
    if !dimension.driving {
        label = format!("({label})");
    }
    if let Some(at) = view.screen(world(text_at)) {
        let galley = view.painter.layout_no_wrap(label, FontId::proportional(13.0), color);
        let frame = Rect::from_center_size(at, galley.size() + vec2(6.0, 2.0));
        view.painter.rect_filled(frame, 2.0, colors::BACKGROUND_TOP);
        view.painter.galley(frame.min + vec2(3.0, 1.0), galley, color);
    }
}

fn preview(view: &View<'_>, plane: &SketchPlane, session: &Session) {
    let preview = session.preview();
    let stroke = Stroke::new(1.4, colors::PREVIEW);
    let world = |p: [f64; 2]| plane.world([p[0], p[1], 0.0]);
    for (a, b) in &preview.lines {
        view.segment(world(*a), world(*b), stroke);
    }
    for (centre, radius) in &preview.circles {
        let tau = std::f64::consts::TAU;
        view.polyline(
            (0..=72).map(|i| {
                let angle = tau * f64::from(i) / 72.0;
                world([centre[0] + radius * fm::cos(angle), centre[1] + radius * fm::sin(angle)])
            }),
            stroke,
        );
    }
    for (centre, radius, start, sweep) in &preview.arcs {
        view.polyline(
            (0..=72).map(|i| {
                let angle = start + sweep * f64::from(i) / 72.0;
                world([centre[0] + radius * fm::cos(angle), centre[1] + radius * fm::sin(angle)])
            }),
            stroke,
        );
    }
    if let Some(snap) = preview.snap {
        let Some(at) = view.screen(world(snap.pos)) else { return };
        match snap.kind {
            SnapKind::Free => {}
            SnapKind::Point => {
                view.painter.rect_stroke(
                    Rect::from_center_size(at, vec2(11.0, 11.0)),
                    0.0,
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
            SnapKind::OnCurve => {
                view.painter.line_segment([at - vec2(6.0, 6.0), at + vec2(6.0, 6.0)], stroke);
                view.painter.line_segment([at - vec2(6.0, -6.0), at + vec2(6.0, -6.0)], stroke);
            }
            SnapKind::Horizontal => {
                view.text(at + vec2(10.0, -10.0), Align2::LEFT_BOTTOM, "H", 12.0, colors::PREVIEW)
            }
            SnapKind::Vertical => {
                view.text(at + vec2(10.0, -10.0), Align2::LEFT_BOTTOM, "V", 12.0, colors::PREVIEW)
            }
        }
    }
    for item in &preview.picks {
        if let Item::Point(p) = item
            && let Some(at) = session.point_pos(*p).and_then(|p| view.screen(plane.world(p)))
        {
            view.painter.circle_stroke(at, 7.0, stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_shown_without_trailing_zeros() {
        assert_eq!(format_length(0.025), "25 mm");
        assert_eq!(format_length(0.012_345_6), "12.346 mm");
        assert_eq!(format_length(-0.000_000_01), "0 mm");
        assert_eq!(format_angle(std::f64::consts::FRAC_PI_2), "90 deg");
    }

    #[test]
    fn grid_steps_follow_the_1_2_5_series() {
        for (px, expected) in [(1e-4, 0.005), (1.2e-4, 0.005), (1.3e-4, 0.01), (2.6e-4, 0.02), (1.0, 50.0)] {
            assert!((grid_step(px) - expected).abs() < 1e-12 * expected, "px {px}: {}", grid_step(px));
        }
    }
}
