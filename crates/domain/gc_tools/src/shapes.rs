//! The Point, Polygon, Slot and Offset tools (docs/ux/inventor-sketch-parity.md §2.2–2.4), Inventor 2026
//! help: To Work with Points (GUID-D4A4288F-32F1-4E23-A382-930EBA152E87), To Create 2D Shape Geometry
//! (GUID-D489CE6D-7299-4211-A43A-F3580A4BA357), To Stretch, Scale, or Offset 2D Sketch Geometry
//! (GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4).
//!
//! - Point: each click places a point (a centre point unless the Center Point toggle is off); the X and Y
//!   boxes show the cursor's coordinates, and typed values place it there (Tab, X, Tab, Y, Enter; Inventor
//!   2026 help, Dynamic Input, GUID-33A8B11D-A04C-4809-BC82-C0419215915F).
//! - Polygon: click the centre, then a corner (inscribed) or an edge midpoint (circumscribed).
//! - Slot: two clicks as the slot kind reads them, then the width.
//! - Arc slot: three clicks for the centre arc (start, end, a point on it; or centre, start, end), then the
//!   width.
//! - Offset: click a curve, then where the offset goes (or type the distance).
//! - Midpoint Line: the midpoint, then an end (To Create Lines, Arcs, and Bends,
//!   GUID-4F861674-F74C-474E-869A-4C190ED6EC4B).
//! - Center rectangles: the centre, then a corner; or the centre, a side's middle and the other half-size.
//! - Tangent Circle: three lines, the circle touching them nearest the clicks.
//! - Split: a curve, cut at the crossing nearest the click (To Split, Trim, or Extend Curves,
//!   GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D).
//! - Chamfer: two lines, bevelled as the 2D Chamfer dialog says (Create Chamfers in To Create 2D Shape
//!   Geometry).

use gc_doc::{ArcSlotKind, SketchOp, preview_sketch_op};
use gc_math::fm;
use gc_sketch::{CurveGeom, Sketch};

use super::{
    HudField, Preview, Prompt, Tool, ToolEvent, ToolKind, ToolOutput, arc_through, distance, end_snap,
};
use crate::pick::{closest_on_curve, nearest_curve};

/// The kinds these tools cover.
pub(super) fn is_shape_tool(kind: ToolKind) -> bool {
    matches!(
        kind,
        ToolKind::Point
            | ToolKind::Polygon
            | ToolKind::Slot(_)
            | ToolKind::ArcSlot(_)
            | ToolKind::Offset
            | ToolKind::MidpointLine
            | ToolKind::CenterRectangle
            | ToolKind::CenterRectangle3
            | ToolKind::TangentCircle
            | ToolKind::Split
            | ToolKind::Chamfer
            | ToolKind::Move
            | ToolKind::Copy
            | ToolKind::Rotate
    )
}

/// The width of a slot: twice the cursor's distance from the line through the first two clicks.
fn slot_width(a: [f64; 2], b: [f64; 2], cursor: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = fm::hypot(dx, dy);
    if length < 1e-12 {
        return 2.0 * distance(a, cursor);
    }
    2.0 * ((cursor[0] - a[0]) * dy - (cursor[1] - a[1]) * dx).abs() / length
}

/// The centre arc of an arc slot from its three clicks: centre, start and end, counter-clockwise.
fn slot_arc(
    kind: ArcSlotKind,
    a: [f64; 2],
    b: [f64; 2],
    c: [f64; 2],
) -> Option<([f64; 2], [f64; 2], [f64; 2])> {
    match kind {
        ArcSlotKind::ThreePoint => {
            let (centre, _, _, sweep) = arc_through(a, b, c)?;
            Some(if sweep >= 0.0 { (centre, a, b) } else { (centre, b, a) })
        }
        ArcSlotKind::CenterPoint => {
            if distance(a, b) < 1e-12 || distance(a, c) < 1e-12 {
                return None;
            }
            // The shorter way round from the start to the end.
            let angle = |p: [f64; 2]| fm::atan2(p[1] - a[1], p[0] - a[0]);
            let ccw = (angle(c) - angle(b)).rem_euclid(std::f64::consts::TAU);
            Some(if ccw <= std::f64::consts::PI { (a, b, c) } else { (a, c, b) })
        }
    }
}

/// The width of an arc slot: twice the cursor's distance from the centre arc's circle.
fn arc_slot_width(centre: [f64; 2], start: [f64; 2], cursor: [f64; 2]) -> f64 {
    2.0 * (distance(centre, cursor) - distance(centre, start)).abs()
}

fn xyz(p: [f64; 2]) -> [f64; 3] {
    [p[0], p[1], 0.0]
}

/// The curves an edit adds, drawn into a preview.
fn draw_new(preview: &mut Preview, before: &Sketch, op: &SketchOp) {
    let Some((after, _)) = preview_sketch_op(before, op) else { return };
    let at = |p| after.pos(p).map(|q| [q[0], q[1]]);
    for (id, curve) in &after.curves {
        if before.curves.contains_key(id) || curve.construction {
            continue;
        }
        match curve.geom {
            CurveGeom::Line { a, b } => {
                if let (Some(a), Some(b)) = (at(a), at(b)) {
                    preview.lines.push((a, b));
                }
            }
            CurveGeom::Circle { c, r } => {
                if let Some(c) = at(c) {
                    preview.circles.push((c, r));
                }
            }
            CurveGeom::Arc { c, s, e } => {
                if let (Some(c), Some(s), Some(e)) = (at(c), at(s), at(e)) {
                    let angle = |p: [f64; 2]| fm::atan2(p[1] - c[1], p[0] - c[0]);
                    let mut sweep = (angle(e) - angle(s)).rem_euclid(std::f64::consts::TAU);
                    if sweep < 1e-12 {
                        sweep = std::f64::consts::TAU;
                    }
                    preview.arcs.push((c, distance(c, s), angle(s), sweep));
                }
            }
        }
    }
}

impl Tool {
    /// The Center Point toggle (Format panel): points are centre points (`true`, Inventor's default) or
    /// sketch points.
    pub fn center_point(&self) -> bool {
        self.center_point
    }

    /// A fresh tool of `kind` that keeps this one's settings (Construction, Center Point, the Polygon
    /// dialog, the Offset toggles).
    pub fn renewed(&self, kind: ToolKind) -> Tool {
        let mut tool = Tool::new(kind);
        tool.construction = self.construction;
        tool.center_point = self.center_point;
        tool.polygon_sides = self.polygon_sides;
        tool.polygon_circumscribed = self.polygon_circumscribed;
        tool.offset_loop = self.offset_loop;
        tool.offset_constrain = self.offset_constrain;
        tool.chamfer = self.chamfer.clone();
        tool.chamfer_dimensions = self.chamfer_dimensions;
        tool
    }

    /// The 2D Chamfer dialog: the type with its sizes, and Create Dimensions.
    pub fn chamfer_options(&self) -> (gc_doc::ChamferKind, bool) {
        (self.chamfer.clone(), self.chamfer_dimensions)
    }

    /// Sets the 2D Chamfer dialog's values.
    pub fn set_chamfer_options(&mut self, kind: gc_doc::ChamferKind, dimensions: bool) {
        self.chamfer = kind;
        self.chamfer_dimensions = dimensions;
    }

    /// Sets the Center Point toggle.
    pub fn set_center_point(&mut self, on: bool) {
        self.center_point = on;
    }

    /// Polygon: the number of sides (3 to 120) and whether it is circumscribed.
    pub fn polygon(&self) -> (u32, bool) {
        (self.polygon_sides, self.polygon_circumscribed)
    }

    /// Sets the Polygon dialog's values (sides clamped to 3–120).
    pub fn set_polygon(&mut self, sides: u32, circumscribed: bool) {
        self.polygon_sides = sides.clamp(3, 120);
        self.polygon_circumscribed = circumscribed;
    }

    /// Offset: Loop Select and Constrain Offset (both on by default).
    pub fn offset_options(&self) -> (bool, bool) {
        (self.offset_loop, self.offset_constrain)
    }

    /// Sets Loop Select and Constrain Offset (the right-click toggles).
    pub fn set_offset_options(&mut self, loop_select: bool, constrain: bool) {
        self.offset_loop = loop_select;
        self.offset_constrain = constrain;
    }

    pub(super) fn shape_prompt(&self) -> Prompt {
        match self.kind {
            ToolKind::Point => Prompt::PointPlace,
            ToolKind::Polygon if self.anchor.is_none() => Prompt::PolygonCentre,
            ToolKind::Polygon => Prompt::PolygonCorner,
            ToolKind::Slot(_) if self.anchor.is_none() => Prompt::SlotFirst,
            ToolKind::Slot(_) if self.second.is_none() => Prompt::SlotSecond,
            ToolKind::Slot(_) => Prompt::SlotWidth,
            ToolKind::ArcSlot(ArcSlotKind::CenterPoint) if self.anchor.is_none() => Prompt::ArcSlotCentre,
            ToolKind::ArcSlot(_) if self.anchor.is_none() => Prompt::SlotFirst,
            ToolKind::ArcSlot(_) if self.second.is_none() => Prompt::SlotSecond,
            ToolKind::ArcSlot(ArcSlotKind::ThreePoint) if self.slot_third.is_none() => Prompt::ArcSlotThrough,
            ToolKind::ArcSlot(_) if self.slot_third.is_none() => Prompt::ArcSlotEnd,
            ToolKind::ArcSlot(_) => Prompt::SlotWidth,
            ToolKind::MidpointLine if self.anchor.is_none() => Prompt::MidpointLineMid,
            ToolKind::MidpointLine => Prompt::MidpointLineEnd,
            ToolKind::CenterRectangle | ToolKind::CenterRectangle3 if self.anchor.is_none() => {
                Prompt::CenterRectCentre
            }
            ToolKind::CenterRectangle => Prompt::CenterRectCorner,
            ToolKind::CenterRectangle3 if self.second.is_none() => Prompt::CenterRectSide,
            ToolKind::CenterRectangle3 => Prompt::CenterRectWidth,
            ToolKind::TangentCircle => Prompt::TangentCircleLine,
            ToolKind::Split => Prompt::SplitPick,
            ToolKind::Move | ToolKind::Copy if self.anchor.is_none() => Prompt::MoveBase,
            ToolKind::Move | ToolKind::Copy => Prompt::MoveEnd,
            ToolKind::Rotate if self.anchor.is_none() => Prompt::RotateCentre,
            ToolKind::Rotate => Prompt::RotateEnd,
            ToolKind::Chamfer if self.fillet_first.is_none() => Prompt::ChamferFirst,
            ToolKind::Chamfer => Prompt::ChamferSecond,
            ToolKind::Offset if self.offset_pick.is_none() => Prompt::OffsetPick,
            _ => Prompt::OffsetPlace,
        }
    }

    /// The signed offset distance that puts the offset through `cursor` (`None` before a curve is
    /// picked): its size from the picked curve, its side from which offset passes nearer the cursor.
    fn offset_distance(&self, sketch: &Sketch, cursor: [f64; 2]) -> Option<f64> {
        let curve = sketch.curves.get(&self.offset_pick?)?;
        let size = match self.locked_value(HudField::Distance) {
            Some(locked) => locked.abs(),
            None => distance(closest_on_curve(sketch, curve, cursor)?, cursor),
        };
        if size < 1e-9 {
            return None;
        }
        let nearest = |d: f64| -> Option<f64> {
            let op = self.offset_op(d)?;
            let (after, _) = preview_sketch_op(sketch, &op)?;
            after
                .curves
                .iter()
                .filter(|(id, c)| !sketch.curves.contains_key(id) && !c.construction)
                .filter_map(|(_, c)| closest_on_curve(&after, c, cursor).map(|p| distance(p, cursor)))
                .min_by(f64::total_cmp)
        };
        match (nearest(size), nearest(-size)) {
            (Some(plus), Some(minus)) => Some(if plus <= minus { size } else { -size }),
            (Some(_), None) => Some(size),
            (None, Some(_)) => Some(-size),
            (None, None) => None,
        }
    }

    fn offset_op(&self, distance: f64) -> Option<SketchOp> {
        Some(SketchOp::Offset {
            curves: vec![self.offset_pick?],
            distance,
            loop_select: self.offset_loop,
            constrain: self.offset_constrain,
        })
    }

    /// The edit the next click would make with the cursor at `cursor`.
    fn shape_op(&self, sketch: &Sketch, cursor: [f64; 2]) -> Option<SketchOp> {
        match self.kind {
            ToolKind::Point => Some(SketchOp::PlacePoint { pos: xyz(cursor), center: self.center_point }),
            ToolKind::Polygon => {
                let centre = self.anchor?;
                (distance(centre, cursor) > 1e-12).then(|| SketchOp::AddPolygon {
                    centre: xyz(centre),
                    at: xyz(cursor),
                    sides: self.polygon_sides,
                    circumscribed: self.polygon_circumscribed,
                })
            }
            ToolKind::Slot(kind) => {
                let (a, b) = (self.anchor?, self.second?.pos);
                let width = self.locked_value(HudField::Width).unwrap_or_else(|| slot_width(a, b, cursor));
                (width > 1e-9).then(|| SketchOp::AddSlot { a: xyz(a), b: xyz(b), width, kind })
            }
            ToolKind::ArcSlot(kind) => {
                let (centre, start, end) = slot_arc(kind, self.anchor?, self.second?.pos, self.slot_third?)?;
                let width = self
                    .locked_value(HudField::Width)
                    .unwrap_or_else(|| arc_slot_width(centre, start, cursor));
                (width > 1e-9).then(|| SketchOp::AddArcSlot {
                    centre: xyz(centre),
                    start: xyz(start),
                    end: xyz(end),
                    width,
                })
            }
            ToolKind::Offset => self.offset_op(self.offset_distance(sketch, cursor)?),
            ToolKind::MidpointLine => {
                let mid = self.anchor?;
                (distance(mid, cursor) > 1e-12).then(|| SketchOp::AddMidpointLine {
                    mid: xyz(mid),
                    end: xyz(cursor),
                    snap_mid: self.anchor_snap.as_ref().map(end_snap).unwrap_or_default(),
                    snap_end: self.cursor.as_ref().map(end_snap).unwrap_or_default(),
                })
            }
            ToolKind::CenterRectangle => Some(SketchOp::AddCenterRectangle {
                centre: xyz(self.anchor?),
                corner: xyz(cursor),
                snap_centre: self.anchor_snap.as_ref().map(end_snap).unwrap_or_default(),
            }),
            ToolKind::CenterRectangle3 => Some(SketchOp::AddCenterRectangle3 {
                centre: xyz(self.anchor?),
                side: xyz(self.second?.pos),
                width: xyz(cursor),
                snap_centre: self.anchor_snap.as_ref().map(end_snap).unwrap_or_default(),
            }),
            _ => None,
        }
    }

    /// Move, Copy, Rotate: the motion the base (or centre) point and `end` give. A typed Rotate angle wins
    /// over the end point's.
    pub fn transform_motion(&self, end: [f64; 2]) -> Option<gc_doc::Motion> {
        let base = self.anchor?;
        match self.kind {
            ToolKind::Move | ToolKind::Copy => {
                Some(gc_doc::Motion::Translate { by: [end[0] - base[0], end[1] - base[1]] })
            }
            ToolKind::Rotate => {
                let angle = match self.locked_value(HudField::Angle) {
                    Some(angle) => angle,
                    None if distance(base, end) > 1e-12 => fm::atan2(end[1] - base[1], end[0] - base[0]),
                    None => return None,
                };
                Some(gc_doc::Motion::Rotate { centre: base, angle })
            }
            _ => None,
        }
    }

    /// The motion the cursor gives now (for the preview).
    pub fn transform_preview(&self) -> Option<gc_doc::Motion> {
        self.transform_motion(self.cursor.map(|c| c.pos)?)
    }

    fn chamfer_op(&self, second: (gc_core::Id<gc_sketch::Curve>, [f64; 2])) -> Option<SketchOp> {
        let first = self.fillet_first?;
        (first.0 != second.0).then(|| SketchOp::Chamfer {
            lines: [first, second],
            kind: self.chamfer.clone(),
            dimensions: self.chamfer_dimensions,
        })
    }

    pub(super) fn shape_preview(&self, sketch: &Sketch, preview: &mut Preview) {
        if let (ToolKind::Split, Some((curve, at))) = (self.kind, self.hover) {
            preview.picks.push(gc_sketch::Item::Curve(curve));
            if let Some(p) = sketch.split_preview(curve, at) {
                // A small cross where the curve would be cut.
                let span = match sketch.line(curve) {
                    Some((a, b)) => {
                        sketch.pos(a).zip(sketch.pos(b)).map(|(a, b)| distance([a[0], a[1]], [b[0], b[1]]))
                    }
                    None => sketch.round(curve).map(|(_, r)| r),
                };
                let size = span.unwrap_or(0.01) * 0.03;
                preview.lines.push(([p[0] - size, p[1] - size], [p[0] + size, p[1] + size]));
                preview.lines.push(([p[0] - size, p[1] + size], [p[0] + size, p[1] - size]));
            }
            return;
        }
        if self.kind == ToolKind::Chamfer {
            if let Some((first, _)) = self.fillet_first {
                preview.picks.push(gc_sketch::Item::Curve(first));
            }
            if let Some(op) = self.hover.and_then(|h| self.chamfer_op(h)) {
                draw_new(preview, sketch, &op);
            }
            return;
        }
        let Some(cursor) = self.cursor.map(|c| c.pos) else { return };
        if let (ToolKind::Slot(_) | ToolKind::ArcSlot(_), Some(a), None) =
            (self.kind, self.anchor, self.second)
        {
            preview.lines.push((a, cursor));
            return;
        }
        if let (ToolKind::ArcSlot(kind), Some(a), Some(b), None) =
            (self.kind, self.anchor, self.second, self.slot_third)
        {
            // The centre arc follows the cursor.
            if let Some((centre, start, end)) = slot_arc(kind, a, b.pos, cursor) {
                let angle = |p: [f64; 2]| fm::atan2(p[1] - centre[1], p[0] - centre[0]);
                let sweep = (angle(end) - angle(start)).rem_euclid(std::f64::consts::TAU);
                preview.arcs.push((centre, distance(centre, start), angle(start), sweep));
            }
            return;
        }
        if let (ToolKind::CenterRectangle3, Some(c), None) = (self.kind, self.anchor, self.second) {
            preview.lines.push((c, cursor));
            return;
        }
        if let Some(op) = self.shape_op(sketch, cursor)
            && self.kind != ToolKind::Point
        {
            draw_new(preview, sketch, &op);
        }
    }

    /// The input boxes these tools show.
    pub(super) fn shape_hud(&self) -> Vec<super::HudSlot> {
        let Some(cursor) = self.cursor.map(|c| c.pos) else { return Vec::new() };
        let slot = |field, measured| super::HudSlot {
            field,
            measured,
            locked: self.locked(field).map(str::to_owned),
        };
        match self.kind {
            ToolKind::Point => vec![slot(HudField::X, cursor[0]), slot(HudField::Y, cursor[1])],
            ToolKind::Rotate => match self.anchor {
                Some(c) if distance(c, cursor) > 1e-12 => {
                    vec![slot(HudField::Angle, fm::atan2(cursor[1] - c[1], cursor[0] - c[0]))]
                }
                _ => Vec::new(),
            },
            ToolKind::Slot(_) => match (self.anchor, self.second) {
                (Some(a), Some(b)) => vec![slot(HudField::Width, slot_width(a, b.pos, cursor))],
                _ => Vec::new(),
            },
            ToolKind::ArcSlot(kind) => match (self.anchor, self.second, self.slot_third) {
                (Some(a), Some(b), Some(c)) => slot_arc(kind, a, b.pos, c)
                    .map(|(centre, start, _)| {
                        vec![slot(HudField::Width, arc_slot_width(centre, start, cursor))]
                    })
                    .unwrap_or_default(),
                _ => Vec::new(),
            },
            ToolKind::Offset if self.offset_pick.is_some() => {
                vec![slot(HudField::Distance, self.offset_measured)]
            }
            _ => Vec::new(),
        }
    }

    /// Keeps what the input boxes show up to date as the cursor moves.
    pub(super) fn shape_moved(&mut self, sketch: &Sketch) {
        if self.kind == ToolKind::Offset
            && let Some(cursor) = self.cursor.map(|c| c.pos)
        {
            self.offset_measured = self.offset_distance(sketch, cursor).map_or(0.0, f64::abs);
        }
    }

    /// Handles a click or Enter for these tools.
    pub(super) fn shape_event(&mut self, sketch: &Sketch, event: &ToolEvent) -> Option<Vec<ToolOutput>> {
        let (pos, px, modifier) = match *event {
            ToolEvent::Click { pos, px, modifier } => (pos, px, modifier),
            ToolEvent::Key(super::Key::Enter) => {
                let cursor = self.cursor.map(|c| c.pos)?;
                (cursor, 0.0, false)
            }
            _ => return None,
        };
        Some(match self.kind {
            ToolKind::Point => {
                let here = self.place(sketch, pos, px, None, modifier).pos;
                // Typed coordinates win over the cursor's.
                let at = [
                    self.locked_value(HudField::X).unwrap_or(here[0]),
                    self.locked_value(HudField::Y).unwrap_or(here[1]),
                ];
                self.hud.clear();
                vec![ToolOutput::Edit(SketchOp::PlacePoint { pos: xyz(at), center: self.center_point })]
            }
            ToolKind::Polygon => {
                let here = self.place(sketch, pos, px, None, modifier).pos;
                if self.anchor.is_none() {
                    self.anchor = Some(here);
                    return Some(Vec::new());
                }
                let out = self.shape_op(sketch, here).map(|op| self.emit(op, &[]));
                self.reset();
                out.into_iter().collect()
            }
            ToolKind::Slot(_) => {
                let here = self.place(sketch, pos, px, None, modifier);
                match (self.anchor, self.second) {
                    (None, _) => {
                        self.anchor = Some(here.pos);
                        self.anchor_snap = Some(here);
                        Vec::new()
                    }
                    (Some(a), None) => {
                        if distance(a, here.pos) > px.max(1e-12) {
                            self.second = Some(here);
                        }
                        Vec::new()
                    }
                    (Some(_), Some(_)) => {
                        let out = self.shape_op(sketch, here.pos).map(|op| self.emit(op, &[]));
                        self.reset();
                        out.into_iter().collect()
                    }
                }
            }
            ToolKind::ArcSlot(kind) => {
                let here = self.place(sketch, pos, px, None, modifier);
                match (self.anchor, self.second, self.slot_third) {
                    (None, _, _) => {
                        self.anchor = Some(here.pos);
                        self.anchor_snap = Some(here);
                    }
                    (Some(a), None, _) => {
                        if distance(a, here.pos) > px.max(1e-12) {
                            self.second = Some(here);
                        }
                    }
                    (Some(a), Some(b), None) => {
                        if slot_arc(kind, a, b.pos, here.pos).is_some() {
                            self.slot_third = Some(here.pos);
                        }
                    }
                    (Some(_), Some(_), Some(_)) => {
                        let out = self.shape_op(sketch, here.pos).map(|op| self.emit(op, &[]));
                        self.reset();
                        return Some(out.into_iter().collect());
                    }
                }
                Vec::new()
            }
            ToolKind::MidpointLine | ToolKind::CenterRectangle => {
                let here = self.place(sketch, pos, px, None, modifier);
                if self.anchor.is_none() {
                    self.anchor = Some(here.pos);
                    self.anchor_snap = Some(here);
                    return Some(Vec::new());
                }
                self.cursor = Some(here);
                let out = self.shape_op(sketch, here.pos).map(|op| self.emit(op, &[]));
                self.reset();
                out.into_iter().collect()
            }
            ToolKind::CenterRectangle3 => {
                let here = self.place(sketch, pos, px, None, modifier);
                match (self.anchor, self.second) {
                    (None, _) => {
                        self.anchor = Some(here.pos);
                        self.anchor_snap = Some(here);
                        Vec::new()
                    }
                    (Some(c), None) => {
                        if distance(c, here.pos) > px.max(1e-12) {
                            self.second = Some(here);
                        }
                        Vec::new()
                    }
                    (Some(_), Some(_)) => {
                        let out = self.shape_op(sketch, here.pos).map(|op| self.emit(op, &[]));
                        self.reset();
                        out.into_iter().collect()
                    }
                }
            }
            ToolKind::TangentCircle => {
                let line = nearest_curve(sketch, pos, px, &[])
                    .map(|(c, _)| c)
                    .filter(|c| sketch.line(*c).is_some() && !self.tangent_lines.iter().any(|(l, _)| l == c));
                if let Some(line) = line {
                    self.tangent_lines.push((line, pos));
                }
                if let [(a, pa), (b, pb), (c, pc)] = self.tangent_lines[..] {
                    self.tangent_lines.clear();
                    return Some(vec![ToolOutput::Edit(SketchOp::AddTangentCircle {
                        lines: [a, b, c],
                        near: [pa, pb, pc],
                    })]);
                }
                Vec::new()
            }
            ToolKind::Move | ToolKind::Copy | ToolKind::Rotate => {
                let here = self.place(sketch, pos, px, None, modifier).pos;
                if self.anchor.is_none() {
                    self.anchor = Some(here);
                    return Some(Vec::new());
                }
                let out = self.transform_motion(here).map(ToolOutput::Transform);
                self.reset();
                out.into_iter().collect()
            }
            ToolKind::Split => {
                let Some((curve, _)) = nearest_curve(sketch, pos, px, &[]) else { return Some(Vec::new()) };
                self.hover = None;
                vec![ToolOutput::Edit(SketchOp::Split { curve, at: pos })]
            }
            ToolKind::Chamfer => {
                let hit = nearest_curve(sketch, pos, px, &[]).filter(|(c, _)| sketch.line(*c).is_some());
                let Some((curve, _)) = hit else { return Some(Vec::new()) };
                match self.chamfer_op((curve, pos)) {
                    Some(op) => {
                        self.fillet_first = None;
                        self.hover = None;
                        vec![ToolOutput::Edit(op)]
                    }
                    None => {
                        self.fillet_first = Some((curve, pos));
                        Vec::new()
                    }
                }
            }
            ToolKind::Offset => {
                if self.offset_pick.is_none() {
                    self.offset_pick = nearest_curve(sketch, pos, px, &[]).map(|(c, _)| c);
                    return Some(Vec::new());
                }
                let out = self.shape_op(sketch, pos).map(ToolOutput::Edit);
                self.hud.clear();
                self.offset_pick = None;
                out.into_iter().collect()
            }
            _ => return None,
        })
    }
}
