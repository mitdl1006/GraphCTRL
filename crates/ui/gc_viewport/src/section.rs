//! Section views: half, quarter and three-quarter. The model is cut by one or two planes and what lies
//! on the cut side is not drawn; the mini-toolbar flips a plane, moves it and applies the section.
//! Reference: Inventor 2026 help, To Work with Section Views
//! (GUID-9A47D4FC-30CE-422D-86EC-5E4AFC624C2D).

use egui::{Pos2, Shape, Stroke};
use gc_i18n::{Lang, tr};
use gc_math::DVec3;
use gc_scene::Scene;
use gc_session::Session;
use gc_ui::colors;

use crate::target::Slicing;

/// The kind of section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionKind {
    /// One plane.
    Half,
    /// Two planes; a quarter of the model is cut away.
    Quarter,
    /// Two planes; three quarters of the model are cut away (the part both planes keep is what stays).
    ThreeQuarter,
}

/// A cutting plane: normal along a world axis, at a distance from the origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionPlane {
    /// The axis the plane's normal runs along: 0 = X (the YZ plane), 1 = Y (XZ), 2 = Z (XY).
    pub axis: usize,
    /// Where the plane cuts the axis, metres.
    pub offset: f64,
    /// The cut side is the negative one.
    pub flipped: bool,
}

impl SectionPlane {
    /// The plane as a normal pointing to the cut side and the constant `normal . p` on it.
    fn world(&self) -> (DVec3, f64) {
        let mut normal = DVec3::ZERO;
        normal[self.axis] = if self.flipped { -1.0 } else { 1.0 };
        (normal, normal[self.axis] * self.offset)
    }
}

/// A section view in progress or applied.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    /// The kind.
    pub kind: SectionKind,
    /// The cutting planes: one for a half, two otherwise.
    pub planes: Vec<SectionPlane>,
    /// The plane being edited (the toolbar is up while this is `Some`).
    pub editing: Option<usize>,
}

impl Section {
    /// Starts a section of the given kind with its first plane at the origin.
    pub fn new(kind: SectionKind) -> Self {
        Self { kind, planes: vec![SectionPlane { axis: 1, offset: 0.0, flipped: false }], editing: Some(0) }
    }

    /// What the renderer cuts away.
    pub(crate) fn slicing(&self) -> Slicing {
        Slicing {
            planes: self.planes.iter().map(SectionPlane::world).collect(),
            union: self.kind == SectionKind::ThreeQuarter,
        }
    }

    /// Draws the plane being edited as a translucent rectangle over the model.
    pub(crate) fn paint_plane(
        &self,
        painter: &egui::Painter,
        screen: &impl Fn(DVec3) -> Option<Pos2>,
        session: &Session,
    ) {
        let Some(index) = self.editing else { return };
        let plane = self.planes[index];
        let bounds = Scene::of(session).bounds();
        let (centre, half) = if bounds.is_empty() {
            (DVec3::ZERO, 0.5)
        } else {
            (bounds.center(), bounds.diagonal().max(0.2) * 0.6)
        };
        let (a, b) = match plane.axis {
            0 => (DVec3::Y, DVec3::Z),
            1 => (DVec3::X, DVec3::Z),
            _ => (DVec3::X, DVec3::Y),
        };
        let mut middle = centre;
        middle[plane.axis] = plane.offset;
        let corners: Option<Vec<Pos2>> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .iter()
            .map(|(u, v)| screen(middle + a * (u * half) + b * (v * half)))
            .collect();
        if let Some(corners) = corners {
            painter.add(Shape::convex_polygon(
                corners,
                colors::ACCENT.gamma_multiply(0.18),
                Stroke::new(1.5, colors::ACCENT),
            ));
        }
    }
}

/// What the toolbar asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolbarAction {
    /// Nothing.
    None,
    /// Cancel: the section goes away.
    Cancel,
}

/// The mini-toolbar of the section view: plane, position, flip, continue / OK, cancel.
pub(crate) fn toolbar(
    ctx: &egui::Context,
    section: &mut Section,
    free: egui::Rect,
    lang: Lang,
) -> ToolbarAction {
    let Some(index) = section.editing else { return ToolbarAction::None };
    let mut action = ToolbarAction::None;
    let two = section.kind != SectionKind::Half;
    let last = !two || index == 1;
    egui::Area::new(egui::Id::unique("gc_section_toolbar"))
        .fixed_pos(egui::pos2(free.center().x - 220.0, free.top() + 10.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(gc_ui::panel_fill(false))
                .stroke(Stroke::new(1.0, colors::border()))
                .corner_radius(8)
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let kind = match section.kind {
                            SectionKind::Half => tr!(lang, SectionHalf),
                            SectionKind::Quarter => tr!(lang, SectionQuarter),
                            SectionKind::ThreeQuarter => tr!(lang, SectionThreeQuarter),
                        };
                        ui.label(egui::RichText::new(kind).strong());
                        let plane = &mut section.planes[index];
                        for (axis, name) in
                            [(0, tr!(lang, OriginYz)), (1, tr!(lang, OriginXz)), (2, tr!(lang, OriginXy))]
                        {
                            if ui.selectable_label(plane.axis == axis, name).clicked() {
                                plane.axis = axis;
                            }
                        }
                        let mut millimetres = plane.offset * 1000.0;
                        let label = ui.label(tr!(lang, SectionDistance));
                        let value = ui.add(egui::DragValue::new(&mut millimetres).speed(1.0).suffix(" mm"));
                        value.labelled_by(label.id);
                        plane.offset = millimetres / 1000.0;
                        if ui.button(tr!(lang, SectionFlip)).clicked() {
                            plane.flipped = !plane.flipped;
                        }
                        let go = if last { tr!(lang, SectionOk) } else { tr!(lang, SectionContinue) };
                        if ui.button(go).clicked() {
                            if last {
                                section.editing = None;
                            } else {
                                section.planes.push(SectionPlane { axis: 0, offset: 0.0, flipped: false });
                                section.editing = Some(1);
                            }
                        }
                        if ui.button(tr!(lang, SectionCancel)).clicked() {
                            action = ToolbarAction::Cancel;
                        }
                    });
                });
        });
    action
}
