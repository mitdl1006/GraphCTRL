//! The Point, Polygon, Slot and Offset tools (docs/ux/inventor-sketch-parity.md §2.2–2.4), Inventor 2026
//! help: To Work with Points (GUID-D4A4288F-32F1-4E23-A382-930EBA152E87), To Create 2D Shape Geometry
//! (GUID-D489CE6D-7299-4211-A43A-F3580A4BA357), To Stretch, Scale, or Offset 2D Sketch Geometry
//! (GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4).
//!
//! - Point: each click places a point (a centre point unless the Center Point toggle is off).
//! - Polygon: click the centre, then a corner (inscribed) or an edge midpoint (circumscribed).
//! - Slot: two clicks as the slot kind reads them, then the width.
//! - Offset: click a curve, then where the offset goes (or type the distance).

use gc_doc::{SketchOp, preview_sketch_op};
use gc_math::fm;
use gc_sketch::{CurveGeom, Sketch};

use super::{HudField, Preview, Prompt, Tool, ToolEvent, ToolKind, ToolOutput, distance};
use crate::pick::{closest_on_curve, nearest_curve};

/// The kinds these tools cover.
pub(super) fn is_shape_tool(kind: ToolKind) -> bool {
    matches!(kind, ToolKind::Point | ToolKind::Polygon | ToolKind::Slot(_) | ToolKind::Offset)
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
        tool
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
            ToolKind::Offset => self.offset_op(self.offset_distance(sketch, cursor)?),
            _ => None,
        }
    }

    pub(super) fn shape_preview(&self, sketch: &Sketch, preview: &mut Preview) {
        let Some(cursor) = self.cursor.map(|c| c.pos) else { return };
        if let (ToolKind::Slot(_), Some(a), None) = (self.kind, self.anchor, self.second) {
            preview.lines.push((a, cursor));
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
            ToolKind::Slot(_) => match (self.anchor, self.second) {
                (Some(a), Some(b)) => vec![slot(HudField::Width, slot_width(a, b.pos, cursor))],
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
                vec![ToolOutput::Edit(SketchOp::PlacePoint { pos: xyz(here), center: self.center_point })]
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
