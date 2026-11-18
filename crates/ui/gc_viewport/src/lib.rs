//! The viewport: view navigation, sketch tool input and drawing of the scene.
//!
//! Solids (bodies, robot links) come from the renderer's offscreen target when a GPU is present. The grid
//! is painted under that image and sketches, dimensions, traces and tool previews over it with egui's
//! painter, through the same camera. Without a GPU everything still works; only the solids are missing.

mod dof;
mod freemove;
mod gizmo;
mod hud;
mod origin;
mod paint;
mod radial;
mod repr;
mod section;
mod target;

use egui::{Key, PointerButton, Sense, vec2};
use gc_core::Id;
use gc_doc::Sketch;
use gc_draw::{Camera, NavAction, NavInput, NavPreset, StandardView, nav_action, viewcube};
use gc_i18n::{Lang, tr};
use gc_math::{Aabb, DPose3, DVec3, Plane};
use gc_scene::Scene;
use gc_session::Session;
use gc_sketch::CurveGeom;
use gc_sketch::Space;
use gc_tools::{Key as ToolKey, ToolEvent, ToolKind};
use gc_ui::colors;

pub use freemove::FreeTool;
pub use origin::ALL as ORIGIN_ELEMENTS;
pub use paint::{format_angle, format_length};
pub use radial::ViewportRequest;
pub use repr::{camera_from_json, camera_to_json, section_from_json, section_to_json};
pub use section::{Section, SectionKind, SectionPlane};
pub use target::Background;

/// The plane a sketch lies in, with its in-plane axes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SketchPlane {
    pub origin: DVec3,
    pub u: DVec3,
    pub v: DVec3,
    pub normal: DVec3,
}

impl SketchPlane {
    pub(crate) fn of_pose(pose: &DPose3) -> Self {
        Self {
            origin: pose.translation,
            u: pose.rotation * DVec3::X,
            v: pose.rotation * DVec3::Y,
            normal: pose.rotation * DVec3::Z,
        }
    }

    /// World position of a sketch point.
    pub(crate) fn world(&self, pos: [f64; 3]) -> DVec3 {
        self.origin + self.u * pos[0] + self.v * pos[1] + self.normal * pos[2]
    }

    /// Sketch coordinates of a world point on the plane.
    pub(crate) fn local(&self, world: DVec3) -> [f64; 2] {
        let d = world - self.origin;
        [d.dot(self.u), d.dot(self.v)]
    }

    pub(crate) fn plane(&self) -> Option<Plane> {
        Plane::from_point_normal(self.origin, self.normal)
    }
}

/// The frame plane of a sketch.
pub(crate) fn plane_of(session: &Session, sketch: &Sketch) -> SketchPlane {
    let (Space::Planar(frame) | Space::Spatial(frame)) = sketch.space;
    let pose = session.doc().frame_world_pose(Id::from_raw(frame.raw)).unwrap_or(DPose3::IDENTITY);
    SketchPlane::of_pose(&pose)
}

/// A tool of the navigation bar that stays on: the left button pans, zooms or orbits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NavTool {
    Pan,
    Zoom,
    Orbit,
}

/// The viewport widget and its view state.
pub struct Viewport {
    /// Whether the frame time and draw statistics are shown in the corner.
    pub perf_overlay: bool,
    /// The Constrain dialog is open: every origin element is drawn and a click picks one for it.
    pub constrain_mode: bool,
    /// What the last click picked for the Constrain dialog.
    constrain_pick: Option<(gc_doc::Occurrence, gc_doc::OriginElement)>,
    /// The navigation tool chosen on the navigation bar: it works with the left button until it ends.
    nav_tool: Option<NavTool>,
    /// The origin element (plane, axis, point) of a component that is selected in the view or the browser.
    pub origin_selected: Option<(gc_doc::Occurrence, gc_doc::OriginElement)>,
    /// The heads-up display of the drawing tools.
    hud: hud::Hud,
    /// Whether the heads-up display (dynamic input) is on.
    pub hud_on: bool,
    /// Free Move or Free Rotate, while on.
    pub free_tool: Option<FreeTool>,
    /// A component being dragged with it.
    free_drag: Option<freemove::FreeDrag>,
    /// The marking menu of the graphics window and the commands it offers.
    marking: gc_ui::marking::MarkingMenu,
    marking_set: radial::Set,
    /// What the view asks the application to do (the Constrain dialog, placing a component).
    request: Option<ViewportRequest>,
    /// The section view, in progress or applied.
    pub section: Option<Section>,
    /// Slice Graphics (F7): the part of the model in front of the sketch plane is cut away while a 2D
    /// sketch is edited.
    pub slice: bool,
    /// Whether the constraint glyphs are drawn (F8 shows them, F9 hides them).
    pub show_constraints: bool,
    /// A selection box being dragged in a sketch: where the mouse went down.
    box_select: Option<egui::Pos2>,
    /// A point of a 3D sketch being dragged: the working plane's normal axis and the level it keeps.
    spatial_drag: Option<(usize, f64)>,
    /// The part of the window that no floating panel covers. The view cube and the statistics sit in its
    /// corners and "fit" frames the model in it. `None`: the whole viewport.
    pub clear: Option<egui::Rect>,
    /// Smoothed frame time in seconds.
    frame_time: f32,
    background: Background,
    /// The view camera.
    pub camera: Camera,
    /// The navigation style.
    pub preset: NavPreset,
    navigating: Option<NavAction>,
    viewed_sketch: Option<Id<Sketch>>,
    value_text: String,
    value_open: bool,
    cube_centre: Option<egui::Pos2>,
    /// A dimension whose expression is being edited in place: which, the text, and whether the box
    /// still has to take the keyboard.
    dimension_edit: Option<(Id<gc_sketch::Dimension>, String, bool)>,
    picker: gc_scene::Picker,
    gizmo: Option<gizmo::Drag>,
    /// Where the viewport was drawn last.
    rect: egui::Rect,
    /// The item the context menu is about.
    /// While true (the Measure command with the Edges and Faces priority), a click picks a corner, an
    /// edge or a face instead of selecting a component.
    pub measure_features: bool,
    /// The last such pick: `Some(None)` for a click on a blank area.
    measure_pick: Option<Option<(gc_session::CellItem, gc_session::Feature)>>,
}

impl Viewport {
    /// A viewport that renders its background with `render_state` when there is one.
    pub fn new(render_state: Option<egui_wgpu::RenderState>) -> Self {
        Self {
            perf_overlay: false,
            clear: None,
            spatial_drag: None,
            constrain_mode: false,
            constrain_pick: None,
            nav_tool: None,
            origin_selected: None,
            hud: hud::Hud::default(),
            hud_on: true,
            free_tool: None,
            free_drag: None,
            marking: gc_ui::marking::MarkingMenu::default(),
            marking_set: radial::Set::default(),
            request: None,
            section: None,
            slice: false,
            show_constraints: true,
            box_select: None,
            frame_time: 0.0,
            background: Background::new(render_state),
            camera: Camera { distance: 0.35, ..Camera::default() },
            preset: NavPreset::Inventor,
            navigating: None,
            viewed_sketch: None,
            value_text: String::new(),
            value_open: false,
            cube_centre: None,
            dimension_edit: None,
            picker: gc_scene::Picker::default(),
            gizmo: None,
            rect: egui::Rect::NOTHING,
            measure_features: false,
            measure_pick: None,
        }
    }

    /// Where the view cube is on screen and how many pixels its half-size is; `None` when the viewport is
    /// too small to show it.
    pub fn view_cube_placement(&self) -> Option<(egui::Pos2, f32)> {
        self.cube_centre.map(|centre| (centre, 27.0))
    }

    /// The background renderer, for screenshots and status display.
    pub fn background(&self) -> &Background {
        &self.background
    }

    /// Switches to a named view.
    pub fn look(&mut self, view: StandardView) {
        self.camera.look(view);
    }

    /// Frames everything in the document.
    pub fn fit(&mut self, session: &Session, size: [f64; 2]) {
        let mut bounds = Aabb::EMPTY;
        for (_, sketch) in session.doc().sketches().iter() {
            let plane = plane_of(session, sketch);
            for point in sketch.points.values() {
                bounds = bounds.including(plane.world(point.pos));
            }
        }
        let scene = Scene::of(session).bounds();
        if !scene.is_empty() {
            bounds = bounds.union(&scene);
        }
        if bounds.is_empty() || bounds.diagonal() < 1e-6 {
            bounds = Aabb::from_points([DVec3::splat(-0.05), DVec3::splat(0.05)]);
        }
        match self.clear.filter(|_| !self.rect.is_negative()) {
            Some(clear) if self.rect.width() > 1.0 => {
                // Frame the model in what the panels leave free, then move it to the middle of that.
                let free = [f64::from(clear.width()), f64::from(clear.height())];
                self.camera.fit(&bounds, free);
                let whole = [f64::from(self.rect.width()), f64::from(self.rect.height())];
                let shift = clear.center() - self.rect.center();
                let middle = [whole[0] * 0.5, whole[1] * 0.5];
                self.camera.pan(
                    middle,
                    [middle[0] + f64::from(shift.x), middle[1] + f64::from(shift.y)],
                    whole,
                );
            }
            _ => self.camera.fit(&bounds, size),
        }
    }

    /// Shows the viewport and feeds the user's input to the view and to the session's tool.
    pub fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session, lang: Lang) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.rect = rect;
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let to_view = |p: egui::Pos2| [f64::from(p.x - rect.left()), f64::from(p.y - rect.top())];

        // Entering a sketch turns the view to face its plane, as CAD users expect.
        let active = session.active_sketch().map(|(id, sketch)| (id, plane_of(session, sketch)));
        if self.viewed_sketch != active.map(|(id, _)| id) {
            self.viewed_sketch = active.map(|(id, _)| id);
            // A 3D sketch is drawn from whatever view the user has.
            if let (Some((_, plane)), false) = (active, session.sketch_is_spatial()) {
                self.camera.look_along(-plane.normal, plane.v);
            }
        }

        self.navigate(ui, &response, rect, size);

        let cursor = response
            .hover_pos()
            .or_else(|| ui.input(|i| i.pointer.latest_pos()))
            .filter(|p| rect.contains(*p));
        if let (Some((_, plane)), None, true) = (active, self.navigating, session.sketch_is_spatial()) {
            self.spatial_drag_input(ui, &response, rect, session, &plane, cursor);
            self.spatial_input(ui, &response, rect, session, &plane, cursor);
        } else if let (Some((_, plane)), None) = (active, self.navigating) {
            let on_sketch = |p: egui::Pos2| {
                plane
                    .plane()
                    .and_then(|pl| self.camera.on_plane(to_view(p), size, &pl))
                    .map(|w| plane.local(w))
            };
            let px = self.camera.px_scale(size);
            let ctrl = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
            let value_box = session.value_request().is_some();
            if !value_box {
                if response.drag_started_by(PointerButton::Primary)
                    && let Some(pos) = ui.input(|i| i.pointer.press_origin()).and_then(on_sketch)
                {
                    session.tool_event(ToolEvent::DragStart { pos, px });
                    // A drag that grabbed nothing with the select tool draws a selection box.
                    if !session.is_dragging() && session.tool() == ToolKind::Select {
                        self.box_select = ui.input(|i| i.pointer.press_origin());
                    }
                }
                if let (Some(start), Some(now)) = (self.box_select, cursor) {
                    let crossing = now.x < start.x;
                    let rect_box = egui::Rect::from_two_pos(start, now);
                    let color = colors::SELECTED;
                    let painter = ui.painter();
                    painter.rect_filled(rect_box, 0.0, color.gamma_multiply(0.12));
                    // Window (left to right): solid; crossing (right to left): dashed.
                    let stroke = egui::Stroke::new(1.0, color);
                    if crossing {
                        painter.add(egui::Shape::dashed_line(
                            &[
                                rect_box.left_top(),
                                rect_box.right_top(),
                                rect_box.right_bottom(),
                                rect_box.left_bottom(),
                                rect_box.left_top(),
                            ],
                            stroke,
                            5.0,
                            3.0,
                        ));
                    } else {
                        painter.rect_stroke(rect_box, 0.0, stroke, egui::StrokeKind::Inside);
                    }
                    ui.ctx().request_repaint();
                }
                if response.drag_stopped_by(PointerButton::Primary)
                    && let Some(start) = self.box_select.take()
                    && let (Some(a), Some(b)) = (on_sketch(start), cursor.and_then(on_sketch))
                {
                    select_in_box(session, a, b, b[0] < a[0], ctrl || ui.input(|i| i.modifiers.shift));
                }
                if response.dragged_by(PointerButton::Primary) {
                    if let Some(pos) = cursor.and_then(on_sketch) {
                        session.tool_event(ToolEvent::Drag { pos });
                    }
                } else if let Some(pos) = cursor.and_then(on_sketch) {
                    session.tool_event(ToolEvent::Move { pos, px, suppress_snap: ctrl });
                }
                if response.drag_stopped_by(PointerButton::Primary) {
                    session.tool_event(ToolEvent::DragEnd);
                }
                if response.clicked_by(PointerButton::Primary)
                    && let Some(pos) = response.interact_pointer_pos().and_then(on_sketch)
                {
                    session.tool_event(ToolEvent::Click { pos, px, modifier: ctrl });
                }
                // A double click on a dimension's text edits its expression in place.
                if response.double_clicked_by(PointerButton::Primary)
                    && let Some(pos) = response.interact_pointer_pos().and_then(on_sketch)
                    && let Some((_, sketch)) = session.active_sketch()
                {
                    let hit = sketch
                        .dimensions
                        .iter()
                        .filter(|(_, dimension)| dimension.driving)
                        .map(|(id, d)| {
                            let (dx, dy) = (d.text_pos[0] - pos[0], d.text_pos[1] - pos[1]);
                            (dx * dx + dy * dy, *id)
                        })
                        .filter(|(squared, _)| *squared <= (18.0 * px) * (18.0 * px))
                        .min_by(|a, b| a.0.total_cmp(&b.0));
                    if let Some((_, id)) = hit {
                        self.edit_dimension(session, id);
                    }
                }
            }
            if session.is_dragging() {
                ui.ctx().request_repaint();
            }
        } else if active.is_none() && self.navigating.is_none() {
            self.cell_input(ui, &response, rect, session);
        }

        // The marking menu: a right press opens it (in a sketch or among the components); a quick mark
        // toward a command picks it, otherwise the radial menu and its overflow list show.
        let secondary_pressed = ui.input(|i| i.pointer.button_pressed(PointerButton::Secondary));
        if secondary_pressed && response.hovered() && self.navigating.is_none() && !self.marking.is_open() {
            let at = ui.input(|i| i.pointer.press_origin().or(i.pointer.interact_pos()));
            self.marking_set = if active.is_some() {
                radial::Set::sketch(session.sketch_is_spatial())
            } else {
                // What is under the cursor is selected, and decides the short menu.
                let view = at.map(to_view);
                let origin = view.and_then(|p| origin::pick(&self.camera, size, session, p, false));
                let item =
                    if origin.is_some() { None } else { view.and_then(|p| self.pick_item(session, p, size)) };
                self.origin_selected = origin;
                session.select_item(item);
                radial::Set::assembly(item, origin)
            };
            if let Some(at) = at {
                self.marking.press(at, ui.input(|i| i.time));
            }
        }
        if self.marking.is_open() {
            let radial: Vec<Option<gc_ui::marking::MarkingItem>> =
                self.marking_set.radial.iter().map(|cmd| cmd.map(|cmd| cmd.item(session, lang))).collect();
            let overflow: Vec<gc_ui::marking::MarkingItem> =
                self.marking_set.overflow.iter().map(|cmd| cmd.item(session, lang)).collect();
            let picked = self.marking.show(ui.ctx(), &radial, &overflow);
            let cmd = picked.and_then(|pick| match pick {
                gc_ui::marking::MarkingPick::Radial(sector) => {
                    self.marking_set.radial.get(sector).copied().flatten()
                }
                gc_ui::marking::MarkingPick::Overflow(index) => self.marking_set.overflow.get(index).copied(),
            });
            if let Some(cmd) = cmd {
                match radial::run(cmd, session) {
                    radial::Effect::None => {}
                    radial::Effect::Request(request) => self.request = Some(request),
                    radial::Effect::Home => self.camera.look(StandardView::Iso),
                    radial::Effect::Fit => self.fit(session, [1000.0, 700.0]),
                    radial::Effect::Free(tool) => self.free_tool = Some(tool),
                }
            }
        }

        let scene = Scene::of(session);
        paint::under(ui, rect, &self.camera, session);
        // Finishing the sketch restores the sliced graphics.
        if active.is_none() || session.sketch_is_spatial() {
            self.slice = false;
        }
        let slice = if let Some(section) = &self.section {
            section.slicing()
        } else {
            let one = active.filter(|_| self.slice).map(|(_, plane)| {
                // The side of the plane the eye is on is cut away.
                let toward_eye =
                    if plane.normal.dot(self.camera.forward()) < 0.0 { plane.normal } else { -plane.normal };
                (toward_eye, toward_eye.dot(plane.origin))
            });
            target::Slicing { planes: one.into_iter().collect(), union: false, cap: false }
        };
        self.background.paint(ui, rect, &self.camera, &scene, slice);
        paint::over(
            ui,
            rect,
            &self.camera,
            session,
            &scene,
            self.background.has_gpu(),
            self.show_constraints,
        );
        {
            let camera = self.camera;
            let screen = |world: DVec3| {
                let p = camera.project(world, size)?;
                #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
                Some(egui::pos2(rect.left() + p[0] as f32, rect.top() + p[1] as f32))
            };
            origin::paint(&ui.painter_at(rect), &screen, session, self.origin_selected, self.constrain_mode);
            dof::paint(&ui.painter_at(rect), &screen, session, camera.distance * 0.12);
        }
        if let Some(section) = &self.section {
            let camera = self.camera;
            let screen = |world: DVec3| {
                let p = camera.project(world, size)?;
                #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
                Some(egui::pos2(rect.left() + p[0] as f32, rect.top() + p[1] as f32))
            };
            section.paint_plane(&ui.painter_at(rect), &screen, session);
        }
        if let Some(section) = &mut self.section
            && section::toolbar(ui.ctx(), section, self.clear.unwrap_or(rect), lang)
                == section::ToolbarAction::Cancel
        {
            self.section = None;
        }
        if self.hud_on && active.is_some() && !session.sketch_is_spatial() {
            self.hud.show(ui.ctx(), session, cursor, lang);
        } else {
            self.hud.cancel();
        }
        self.view_cube(ui, rect, lang);
        self.nav_bar(ui, rect, session, lang);
        if self.perf_overlay {
            self.perf(ui, rect, lang);
        }
        self.value_box(ui, rect, session, size);
        self.dimension_box(ui, rect, session, size);
    }

    /// Frame time, draw calls and triangles in the bottom-left corner. Keeps the viewport repainting, so the
    /// frame time is the time of a frame and not of the wait between inputs.
    fn perf(&mut self, ui: &egui::Ui, rect: egui::Rect, lang: Lang) {
        let dt = ui.input(|i| i.unstable_dt).clamp(0.0, 1.0);
        self.frame_time = if self.frame_time == 0.0 { dt } else { self.frame_time * 0.9 + dt * 0.1 };
        let ms = self.frame_time * 1000.0;
        let fps = if self.frame_time > 0.0 { 1.0 / self.frame_time } else { 0.0 };
        let stats = self.background.stats();
        let text =
            tr!(lang, PerfStats, format!("{ms:.1}"), format!("{fps:.0}"), stats.draw_calls, stats.triangles);
        let painter = ui.painter_at(rect);
        let galley = painter.layout_no_wrap(text, egui::FontId::monospace(12.0), egui::Color32::WHITE);
        let frame = egui::Rect::from_min_size(
            egui::pos2(
                self.clear.unwrap_or(rect).left() + 8.0,
                self.clear.unwrap_or(rect).bottom() - 8.0 - galley.size().y - 6.0,
            ),
            galley.size() + egui::vec2(12.0, 6.0),
        );
        painter.rect_filled(frame, 4.0, egui::Color32::from_black_alpha(170));
        painter.galley(frame.min + egui::vec2(6.0, 3.0), galley, egui::Color32::WHITE);
        ui.ctx().request_repaint();
    }

    /// What the view asks the application to do, once.
    pub fn take_request(&mut self) -> Option<ViewportRequest> {
        self.request.take()
    }

    /// The origin element the last click picked for the Constrain dialog, once.
    pub fn take_constrain_pick(&mut self) -> Option<(gc_doc::Occurrence, gc_doc::OriginElement)> {
        self.constrain_pick.take()
    }

    /// The navigation bar under the view cube: zoom all, pan, zoom, orbit and look at, with a home button
    /// at the cube. A tool stays on until Esc or the next choice.
    /// Reference: Inventor 2026 help, To Work with the Navigation Bar (GUID-FAAB453E-31EB-46B8-9A74-3BF7A4D88EC9).
    fn nav_bar(&mut self, ui: &mut egui::Ui, rect: egui::Rect, session: &Session, lang: Lang) {
        let free = self.clear.unwrap_or(rect);
        let button = 30.0;
        let tools = [
            (gc_ui::Icon::ZoomAll, tr!(lang, NavZoomAll), None),
            (gc_ui::Icon::Pan, tr!(lang, NavPan), Some(NavTool::Pan)),
            (gc_ui::Icon::Zoom, tr!(lang, NavZoom), Some(NavTool::Zoom)),
            (gc_ui::Icon::Orbit, tr!(lang, NavOrbit), Some(NavTool::Orbit)),
            (gc_ui::Icon::LookAt, tr!(lang, NavLookAt), None),
        ];
        let top_left = egui::pos2(free.right() - 8.0 - button, free.top() + 140.0);
        let bar = egui::Rect::from_min_size(top_left, vec2(button, button * tools.len() as f32));
        if !free.contains_rect(bar) {
            return;
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(bar.expand(2.0), 8.0, gc_ui::panel_fill(false));
        painter.rect_stroke(
            bar.expand(2.0),
            8.0,
            egui::Stroke::new(1.0, colors::border()),
            egui::StrokeKind::Outside,
        );
        let look_target = self.look_target(session);
        let mut act = None;
        for (index, (icon, label, tool)) in tools.iter().enumerate() {
            let cell =
                egui::Rect::from_min_size(top_left + vec2(0.0, button * index as f32), vec2(button, button));
            let enabled = !(index == 4 && look_target.is_none());
            let sense = if enabled { Sense::click() } else { Sense::hover() };
            let response = ui.interact(cell, egui::Id::unique(("gc_nav_bar", index)), sense);
            let active = tool.is_some() && self.nav_tool == *tool;
            response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, active, *label));
            if active {
                painter.rect_filled(cell.shrink(2.0), 6.0, colors::ACCENT.gamma_multiply(0.25));
            } else if response.hovered() && enabled {
                painter.rect_filled(cell.shrink(2.0), 6.0, colors::text().gamma_multiply(0.1));
            }
            let tint = if enabled { colors::text() } else { colors::muted().gamma_multiply(0.5) };
            icon.paint(&painter, cell.shrink(7.0), tint);
            if response.on_hover_text(*label).clicked() {
                act = Some(index);
            }
        }
        match act {
            Some(0) => {
                self.nav_tool = None;
                self.fit(session, [1000.0, 700.0]);
            }
            Some(4) => {
                self.nav_tool = None;
                if let Some((normal, up)) = look_target {
                    self.camera.look_along(-normal, up);
                }
            }
            Some(index) => {
                let chosen = tools[index].2;
                self.nav_tool = if self.nav_tool == chosen { None } else { chosen };
            }
            None => {}
        }
        // The home button at the view cube's corner.
        let home = egui::Rect::from_center_size(
            free.right_top() + vec2(-74.0 - 52.0, 74.0 - 52.0),
            vec2(22.0, 22.0),
        );
        if free.contains_rect(home) {
            let response = ui.interact(home, egui::Id::unique("gc_nav_home"), Sense::click());
            let label = tr!(lang, NavHome);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
            if response.hovered() {
                painter.rect_filled(home, 5.0, colors::text().gamma_multiply(0.1));
            }
            gc_ui::Icon::Home.paint(&painter, home.shrink(3.0), colors::text());
            if response.on_hover_text(label).clicked() {
                self.camera.look(StandardView::Iso);
            }
        }
    }

    /// The face the Look At tool turns the view to: a selected origin plane, or the sketch plane being
    /// edited. Returns the face's normal and the direction that is up in it.
    fn look_target(&self, session: &Session) -> Option<(DVec3, DVec3)> {
        if let Some((occurrence, gc_doc::OriginElement::Plane(plane))) = self.origin_selected
            && let Some(origin) = session.component_origin(occurrence)
        {
            let pose = origin.plane(plane);
            return Some((pose.rotation * DVec3::Z, pose.rotation * DVec3::Y));
        }
        let (_, sketch) = session.active_sketch()?;
        let plane = plane_of(session, sketch);
        (!session.sketch_is_spatial()).then_some((plane.normal, plane.v))
    }

    /// The view cube in the top-right corner: shows how the view is turned; clicking a face, an edge or a
    /// corner looks at the model from there.
    fn view_cube(&mut self, ui: &mut egui::Ui, rect: egui::Rect, lang: Lang) {
        /// Pixels per cube unit (the cube's half-size).
        const HALF: f32 = 27.0;
        let free = self.clear.unwrap_or(rect);
        let centre = free.right_top() + egui::vec2(-74.0, 74.0);
        let area = egui::Rect::from_center_size(centre, egui::Vec2::splat(124.0));
        self.cube_centre = free.contains_rect(area).then_some(centre);
        if self.cube_centre.is_none() {
            return;
        }
        let response = ui.interact(area, egui::Id::unique("gc_view_cube"), Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "ViewCube"));
        let to_cube =
            |p: egui::Pos2| [f64::from((p.x - centre.x) / HALF), f64::from((centre.y - p.y) / HALF)];
        #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
        let to_screen =
            |c: [f64; 2]| egui::pos2(centre.x + c[0] as f32 * HALF, centre.y - c[1] as f32 * HALF);
        let hovered = response.hover_pos().and_then(|p| viewcube::pick(&self.camera, to_cube(p)));
        let painter = ui.painter_at(rect);
        let stroke = egui::Stroke::new(1.0, colors::cube_edge());
        for face in viewcube::visible_faces(&self.camera) {
            let fill = if hovered == Some(face.normal) { colors::cube_hover() } else { colors::cube_face() };
            let corners = face.corners;
            painter.add(egui::Shape::convex_polygon(corners.map(to_screen).to_vec(), fill, stroke));
            // A face seen nearly edge-on is too thin for its name.
            let area: f64 = (0..4)
                .map(|i| {
                    let (a, b) = (corners[i], corners[(i + 1) % 4]);
                    a[0] * b[1] - b[0] * a[1]
                })
                .sum::<f64>()
                .abs()
                * 0.5;
            if area > 1.3 {
                let name = match face.normal {
                    [0, -1, 0] => tr!(lang, ViewFront),
                    [0, 1, 0] => tr!(lang, ViewBack),
                    [1, 0, 0] => tr!(lang, ViewRight),
                    [-1, 0, 0] => tr!(lang, ViewLeft),
                    [0, 0, 1] => tr!(lang, ViewTop),
                    _ => tr!(lang, ViewBottom),
                };
                let font = egui::FontId::proportional(11.0);
                painter.text(
                    to_screen(face.centre),
                    egui::Align2::CENTER_CENTER,
                    name,
                    font,
                    colors::cube_edge(),
                );
            }
        }
        // Edges and corners light up as a dot where the click would look from.
        if let Some(target) = hovered.filter(|t| t.iter().filter(|c| **c != 0).count() > 1) {
            painter.circle_filled(
                to_screen(viewcube::screen_of(&self.camera, target)),
                6.0,
                colors::cube_hover(),
            );
            painter.circle_stroke(to_screen(viewcube::screen_of(&self.camera, target)), 6.0, stroke);
        }
        if response.clicked()
            && let Some(target) =
                response.interact_pointer_pos().and_then(|p| viewcube::pick(&self.camera, to_cube(p)))
        {
            viewcube::look(&mut self.camera, target);
        }
    }

    /// Wheel zoom and the drag bindings of the navigation preset.
    fn navigate(&mut self, ui: &egui::Ui, response: &egui::Response, rect: egui::Rect, size: [f64; 2]) {
        let to_view = |p: egui::Pos2| [f64::from(p.x - rect.left()), f64::from(p.y - rect.top())];
        let (scroll, delta, modifiers, keys, pointer) = ui.input(|i| {
            let keys = (
                i.key_down(Key::F2) || self.nav_tool == Some(NavTool::Pan),
                i.key_down(Key::F3) || self.nav_tool == Some(NavTool::Zoom),
                i.key_down(Key::F4) || self.nav_tool == Some(NavTool::Orbit),
            );
            (i.smooth_scroll_delta.y, i.pointer.delta(), i.modifiers, keys, i.pointer.latest_pos())
        });
        if response.hovered()
            && scroll != 0.0
            && let Some(at) = pointer
        {
            // Rolling the wheel towards the user zooms in (the Inventor default).
            self.camera.zoom(gc_math::fm::exp(f64::from(-scroll) * 0.0025), to_view(at), size);
        }
        let input = NavInput {
            left: response.dragged_by(PointerButton::Primary),
            middle: response.dragged_by(PointerButton::Middle),
            shift: modifiers.shift,
            ctrl: modifiers.ctrl,
            f2: keys.0,
            f3: keys.1,
            f4: keys.2,
        };
        self.navigating = nav_action(self.preset, input);
        if let (Some(action), Some(at)) = (self.navigating, pointer) {
            let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));
            match action {
                NavAction::Pan => {
                    let now = to_view(at);
                    self.camera.pan([now[0] - dx, now[1] - dy], now, size);
                }
                NavAction::Orbit => self.camera.orbit(dx, dy),
                NavAction::Zoom => {
                    self.camera.zoom(gc_math::fm::exp(-dy * 0.01), [size[0] * 0.5, size[1] * 0.5], size)
                }
            }
        }
    }

    /// The box in which the value of a dimension being placed is typed.
    fn value_box(&mut self, ui: &mut egui::Ui, rect: egui::Rect, session: &mut Session, size: [f64; 2]) {
        let Some(request) = session.value_request().copied() else {
            self.value_open = false;
            return;
        };
        let Some((_, sketch)) = session.active_sketch() else { return };
        let plane = plane_of(session, sketch);
        if !self.value_open {
            let angle = matches!(request.kind, gc_sketch::DimKind::Angle { .. });
            self.value_text =
                if angle { format_angle(request.measured) } else { format_length(request.measured) };
        }
        let anchor = paint::dimension_anchor(session, sketch, &request.kind).unwrap_or([
            request.text_pos[0],
            request.text_pos[1],
            0.0,
        ]);
        let at = self
            .camera
            .project(plane.world(anchor), size)
            .map_or(rect.center(), |p| egui::pos2(rect.left() + p[0] as f32, rect.top() + p[1] as f32));
        let mut submit = false;
        let mut cancel = false;
        egui::Area::new(egui::Id::unique("gc_dimension_value"))
            .fixed_pos(at)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let edit = egui::TextEdit::singleline(&mut self.value_text).desired_width(110.0);
                    let response = ui.add(edit);
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::TextInput, true, "dimension value")
                    });
                    if !self.value_open {
                        response.request_focus();
                    }
                    if response.lost_focus() {
                        if ui.input(|i| i.key_pressed(Key::Enter)) {
                            submit = true;
                        } else if ui.input(|i| i.key_pressed(Key::Escape)) {
                            cancel = true;
                        }
                    }
                });
            });
        self.value_open = true;
        if submit {
            let text = std::mem::take(&mut self.value_text);
            session.submit_value(&text);
            self.value_open = false;
        } else if cancel {
            session.cancel_value();
            self.value_open = false;
        }
    }

    /// Dragging a point of a 3D sketch with the select tool moves it in the plane through it that the
    /// working plane sets; the solver keeps the constraints.
    fn spatial_drag_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        session: &mut Session,
        plane: &SketchPlane,
        cursor: Option<egui::Pos2>,
    ) {
        if session.tool() != gc_tools::ToolKind::Select {
            return;
        }
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let to_view = |p: egui::Pos2| [f64::from(p.x - rect.left()), f64::from(p.y - rect.top())];
        let local = |w: DVec3| {
            let d = w - plane.origin;
            DVec3::new(d.dot(plane.u), d.dot(plane.v), d.dot(plane.normal))
        };
        let turned = |d: DVec3| DVec3::new(d.dot(plane.u), d.dot(plane.v), d.dot(plane.normal));
        let camera = self.camera;
        let on_screen = move |p: DVec3| camera.project(plane.world(p.to_array()), size);
        let points: Vec<(Id<gc_sketch::SkPoint>, DVec3)> = match session.active_sketch() {
            Some((_, sketch)) => sketch
                .points
                .keys()
                .filter_map(|id| Some((*id, DVec3::from_array(session.point_pos(*id)?))))
                .collect(),
            None => return,
        };
        let away = |screen: [f64; 2]| {
            move |p: DVec3| {
                on_screen(p).map(|s| ((s[0] - screen[0]).powi(2) + (s[1] - screen[1]).powi(2)).sqrt())
            }
        };
        let on_plane = |at: egui::Pos2, axis: usize, level: f64| -> Option<[f64; 3]> {
            let ray = camera.ray(to_view(at), size);
            let (origin, dir) = (local(ray.origin), turned(ray.dir));
            let (o, d) = (origin.to_array(), dir.to_array());
            (d[axis].abs() > 0.1).then(|| {
                let t = (level - o[axis]) / d[axis];
                [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t]
            })
        };
        if response.drag_started_by(PointerButton::Primary)
            && let Some(press) = ui.input(|i| i.pointer.press_origin())
        {
            let distance = away(to_view(press));
            let grabbed = points
                .iter()
                .filter_map(|(id, pos)| Some((distance(*pos)?, *id, *pos)))
                .filter(|(d, _, _)| *d <= gc_tools::tolerance::POINT_PX)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let axis = session.spatial_plane();
            if let Some((_, id, pos)) = grabbed
                && let Some(start) = on_plane(press, axis, pos[axis])
                && session.spatial_drag_start(id, start)
            {
                self.spatial_drag = Some((axis, pos[axis]));
            }
        }
        if let Some((axis, level)) = self.spatial_drag {
            if response.dragged_by(PointerButton::Primary) {
                if let Some(to) = cursor.and_then(|at| on_plane(at, axis, level)) {
                    drop(session.spatial_drag_move(to));
                }
                ui.ctx().request_repaint();
            }
            if response.drag_stopped_by(PointerButton::Primary) {
                session.spatial_drag_end();
                self.spatial_drag = None;
            }
        }
    }

    /// Input of a 3D sketch: the line tool places points along the cursor's ray, the other tools get what
    /// is under the cursor on screen.
    fn spatial_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        session: &mut Session,
        plane: &SketchPlane,
        cursor: Option<egui::Pos2>,
    ) {
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let to_view = |p: egui::Pos2| [f64::from(p.x - rect.left()), f64::from(p.y - rect.top())];
        let Some((_, sketch)) = session.active_sketch() else { return };
        // Everything in the sketch's own coordinates.
        let local = |w: DVec3| {
            let d = w - plane.origin;
            DVec3::new(d.dot(plane.u), d.dot(plane.v), d.dot(plane.normal))
        };
        let turned = |d: DVec3| DVec3::new(d.dot(plane.u), d.dot(plane.v), d.dot(plane.normal));
        let points: Vec<(Id<gc_sketch::SkPoint>, DVec3)> = sketch
            .points
            .keys()
            .filter_map(|id| Some((*id, DVec3::from_array(session.point_pos(*id)?))))
            .collect();
        let lines: Vec<(Id<gc_sketch::Curve>, DVec3, DVec3)> = sketch
            .curves
            .keys()
            .filter_map(|id| {
                let (a, b) = sketch.line(*id)?;
                let at = |p| session.point_pos(p).map(DVec3::from_array);
                Some((*id, at(a)?, at(b)?))
            })
            .collect();
        let camera = self.camera;
        let on_screen = move |p: DVec3| camera.project(plane.world(p.to_array()), size);
        let away = |screen: [f64; 2]| {
            move |p: DVec3| {
                on_screen(p).map(|s| ((s[0] - screen[0]).powi(2) + (s[1] - screen[1]).powi(2)).sqrt())
            }
        };
        let ctrl = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
        let clicked =
            response.clicked_by(PointerButton::Primary).then(|| response.interact_pointer_pos()).flatten();
        if session.tool() == gc_tools::ToolKind::Line {
            let anchor = session.spatial_anchor().map(DVec3::from_array);
            let plane_axis = session.spatial_plane();
            let place = |at: egui::Pos2| {
                let ray = camera.ray(to_view(at), size);
                let ray = gc_math::Ray { origin: local(ray.origin), dir: turned(ray.dir) };
                gc_tools::spatial::place(&ray, anchor, plane_axis, &points, &away(to_view(at)))
            };
            let hover = cursor.and_then(place).map(|placed| (placed.pos.to_array(), placed.axis));
            let click = clicked.and_then(place);
            session.spatial_hover(hover);
            if let Some(placed) = click {
                session.spatial_click(placed.pos.to_array(), placed.point, placed.axis);
            }
        } else if let Some(at) = clicked {
            // Points before lines, nearest first.
            let distance = away(to_view(at));
            let point = points
                .iter()
                .filter_map(|(id, pos)| Some((distance(*pos)?, *id)))
                .filter(|(d, _)| *d <= gc_tools::tolerance::POINT_PX)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let screen = to_view(at);
            let line = || {
                lines
                    .iter()
                    .filter_map(|(id, a, b)| {
                        let (a, b) = (on_screen(*a)?, on_screen(*b)?);
                        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                        let length = dx * dx + dy * dy;
                        let t = if length > 0.0 {
                            (((screen[0] - a[0]) * dx + (screen[1] - a[1]) * dy) / length).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        let (x, y) = (a[0] + dx * t - screen[0], a[1] + dy * t - screen[1]);
                        Some(((x * x + y * y).sqrt(), *id))
                    })
                    .filter(|(d, _)| *d <= gc_tools::tolerance::CURVE_PX)
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            };
            // A glyph is picked by its letters, before the geometry under them.
            let glyph = paint::glyph_spots(plane, sketch, session, |w| {
                if !self.show_constraints {
                    return None;
                }
                let p = camera.project(w, size)?;
                #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
                Some(egui::pos2(rect.left() + p[0] as f32, rect.top() + p[1] as f32))
            })
            .into_iter()
            .map(|spot| {
                let half = 3.5 * spot.label.len().max(1) as f32;
                let centre = if spot.label.is_empty() { spot.at } else { spot.at + egui::vec2(half, -5.0) };
                (centre.distance(at) - half.min(8.0), spot.id)
            })
            .filter(|(d, _)| *d <= 6.0)
            .min_by(|a, b| a.0.total_cmp(&b.0));
            let item = glyph
                .map(|(_, id)| gc_sketch::Item::Constraint(id))
                .or_else(|| point.map(|(_, id)| gc_sketch::Item::Point(id)))
                .or_else(|| line().map(|(_, id)| gc_sketch::Item::Curve(id)));
            session.tool_event(ToolEvent::ClickItem { item, pos: [0.0; 2], modifier: ctrl });
        }
    }

    /// Where a world point appears in the window, as the viewport was last drawn.
    pub fn screen_of(&self, world: DVec3) -> Option<egui::Pos2> {
        let size = [f64::from(self.rect.width()), f64::from(self.rect.height())];
        let p = self.camera.project(world, size)?;
        #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
        Some(egui::pos2(self.rect.left() + p[0] as f32, self.rect.top() + p[1] as f32))
    }

    /// The corner, edge or face under a point of the view (Measure's Edges and Faces priority): a corner
    /// of the face's outline within 8 pixels, else an outline edge within 6 pixels, else the flat face,
    /// else the point on the surface.
    pub fn pick_feature(
        &mut self,
        session: &Session,
        at: [f64; 2],
        size: [f64; 2],
    ) -> Option<(gc_session::CellItem, gc_session::Feature)> {
        use gc_session::Feature;
        let scene = Scene::of(session);
        let hit = self.picker.pick_surface(&scene, &self.camera.ray(at, size))?;
        let screen = |p: DVec3| self.camera.project(p, size);
        let pixels = |p: DVec3| {
            screen(p).map_or(f64::INFINITY, |s| ((s[0] - at[0]).powi(2) + (s[1] - at[1]).powi(2)).sqrt())
        };
        if let Some((centre, axis, radius, length)) = hit.cylinder {
            let feature =
                Feature::Cylinder { centre: centre.to_array(), axis: axis.to_array(), radius, length };
            return Some((hit.owner, feature));
        }
        let Some((normal, point, area, outline)) = hit.face else {
            return Some((hit.owner, Feature::Point(hit.point.to_array())));
        };
        let corner = outline
            .iter()
            .flat_map(|[a, b]| [*a, *b])
            .map(|p| (pixels(p), p))
            .min_by(|x, y| x.0.total_cmp(&y.0));
        if let Some((d, p)) = corner
            && d <= 8.0
        {
            return Some((hit.owner, Feature::Point(p.to_array())));
        }
        let edge = outline
            .iter()
            .filter_map(|[a, b]| Some((segment_distance(at, screen(*a)?, screen(*b)?), *a, *b)))
            .min_by(|x, y| x.0.total_cmp(&y.0));
        if let Some((d, a, b)) = edge
            && d <= 6.0
        {
            return Some((hit.owner, Feature::Edge(a.to_array(), b.to_array())));
        }
        Some((hit.owner, Feature::Face { point: point.to_array(), normal: normal.to_array(), area }))
    }

    /// The last pick made while [`Self::measure_features`] is on, once: `Some(None)` for a click on a
    /// blank area.
    pub fn take_measure_pick(&mut self) -> Option<Option<(gc_session::CellItem, gc_session::Feature)>> {
        self.measure_pick.take()
    }

    /// The item of the cell under a screen position: waypoints and frame origins first (they are small),
    /// then the nearest solid along the ray.
    pub fn pick_item(
        &mut self,
        session: &Session,
        at: [f64; 2],
        size: [f64; 2],
    ) -> Option<gc_session::CellItem> {
        use gc_session::CellItem;
        let scene = Scene::of(session);
        let near = |world: DVec3, within: f64| {
            self.camera
                .project(world, size)
                .is_some_and(|p| ((p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2)).sqrt() <= within)
        };
        if let Some(marker) = scene.waypoints.iter().find(|m| near(m.position, 9.0)) {
            return Some(CellItem::Waypoint(marker.id));
        }
        let frame =
            scene.frames.iter().zip(&scene.frame_items).find(|((_, pose), _)| near(pose.translation, 10.0));
        if let Some((_, (id, _))) = frame {
            return Some(CellItem::Frame(*id));
        }
        // A path by its trace: within a few pixels of the line on screen.
        let on_trace = |trace: &gc_scene::Trace| {
            let screen: Vec<[f64; 2]> =
                trace.points.iter().filter_map(|p| self.camera.project(*p, size)).collect();
            screen.windows(2).any(|pair| segment_distance(at, pair[0], pair[1]) <= 6.0)
        };
        if let Some(trace) = scene.traces.iter().find(|t| on_trace(t)) {
            return Some(CellItem::Path(trace.id));
        }
        self.picker.pick(&scene, &self.camera.ray(at, size)).map(|(_, item)| item)
    }

    /// Outside sketches: the item under the cursor is highlighted, a click selects it, a right click
    /// opens its context menu.
    fn cell_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        session: &mut Session,
    ) {
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let to_view = |p: egui::Pos2| [f64::from(p.x - rect.left()), f64::from(p.y - rect.top())];
        // The move and turn handles of the selected item come first: dragging them is not a selection.
        let on_handles = gizmo::ui(ui, response, rect, &self.camera, session, &mut self.gizmo);
        let hovered = if on_handles {
            None
        } else {
            response.hover_pos().and_then(|p| self.pick_item(session, to_view(p), size))
        };
        if session.hover_item(hovered) {
            ui.ctx().request_repaint();
        }
        // Free Move / Free Rotate: drag a component.
        if let Some(tool) = self.free_tool.filter(|_| !on_handles) {
            if response.drag_started_by(PointerButton::Primary)
                && let Some(press) = ui.input(|i| i.pointer.press_origin()).map(to_view)
                && let Some(item) = self.pick_item(session, press, size)
            {
                session.select_item(Some(item));
                self.free_drag = freemove::begin(&self.camera, size, session, item, press);
            }
            if let Some(drag) = self.free_drag {
                if response.dragged_by(PointerButton::Primary)
                    && let Some(at) = response.hover_pos().or_else(|| ui.input(|i| i.pointer.latest_pos()))
                {
                    freemove::update(&drag, tool, &self.camera, size, session, to_view(at));
                    ui.ctx().request_repaint();
                }
                if response.drag_stopped_by(PointerButton::Primary) {
                    freemove::end(&drag, session);
                    self.free_drag = None;
                }
            }
            if self.free_drag.is_some() {
                return;
            }
        }
        if self.constrain_mode && response.clicked_by(PointerButton::Primary) {
            let at = response.interact_pointer_pos().map(to_view);
            self.constrain_pick = at.and_then(|at| origin::pick(&self.camera, size, session, at, true));
        } else if self.measure_features && response.clicked_by(PointerButton::Primary) {
            let at = response.interact_pointer_pos().map(to_view);
            self.measure_pick = Some(at.and_then(|at| self.pick_feature(session, at, size)));
        } else if !on_handles && response.clicked_by(PointerButton::Primary) {
            // Origin planes, axes and points that are shown come before the solids behind them.
            let at = response.interact_pointer_pos().map(to_view);
            self.origin_selected = at.and_then(|at| origin::pick(&self.camera, size, session, at, false));
            let picked = if self.origin_selected.is_some() {
                None
            } else {
                response.interact_pointer_pos().and_then(|p| self.pick_item(session, to_view(p), size))
            };
            session.select_item(picked);
        }
    }

    /// Opens the in-place editor of a driving dimension of the sketch being edited.
    pub fn edit_dimension(&mut self, session: &Session, id: Id<gc_sketch::Dimension>) {
        let source = session
            .active_sketch()
            .and_then(|(_, sketch)| sketch.dimensions.get(&id))
            .filter(|dimension| dimension.driving)
            .map(|dimension| dimension.source.clone());
        if let Some(source) = source {
            self.dimension_edit = Some((id, source, true));
        }
    }

    /// The box in which the expression of an existing dimension is edited. Enter applies it; a refused
    /// expression keeps the box open (the status bar says why); Escape closes it.
    fn dimension_box(&mut self, ui: &mut egui::Ui, rect: egui::Rect, session: &mut Session, size: [f64; 2]) {
        let Some((id, mut text, fresh)) = self.dimension_edit.take() else { return };
        let Some((_, sketch)) = session.active_sketch() else { return };
        let Some(dimension) = sketch.dimensions.get(&id) else { return };
        let plane = plane_of(session, sketch);
        let at = self
            .camera
            .project(plane.world([dimension.text_pos[0], dimension.text_pos[1], 0.0]), size)
            .map_or(rect.center(), |p| egui::pos2(rect.left() + p[0] as f32, rect.top() + p[1] as f32));
        let (mut submit, mut close) = (false, false);
        egui::Area::new(egui::Id::unique("gc_dimension_edit"))
            .fixed_pos(at)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(130.0));
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::TextInput, true, "dimension expression")
                    });
                    if fresh {
                        response.request_focus();
                    } else if response.lost_focus() {
                        if ui.input(|i| i.key_pressed(Key::Enter)) {
                            submit = true;
                        } else {
                            close = true;
                        }
                    }
                });
            });
        // A refused expression keeps the box and gives it the keyboard back for another try.
        let refused = submit && session.edit_dimension(id, &text).is_err();
        if !(close || submit && !refused) {
            self.dimension_edit = Some((id, text, refused));
        }
    }

    /// Passes Escape and Delete to the tool. Call when no text field has the keyboard.
    pub fn keys(&mut self, ctx: &egui::Context, session: &mut Session) {
        // In a 3D sketch Tab switches the working plane instead of moving the keyboard focus.
        if session.sketch_is_spatial() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Tab)) {
            session.cycle_spatial_plane();
        }
        if ctx.input(|i| i.key_pressed(Key::F7))
            && session.active_sketch().is_some()
            && !session.sketch_is_spatial()
        {
            self.slice = !self.slice;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.nav_tool = None;
            self.free_tool = None;
        }
        if ctx.input(|i| i.key_pressed(Key::F8)) {
            self.show_constraints = true;
        }
        if ctx.input(|i| i.key_pressed(Key::F9)) {
            self.show_constraints = false;
        }
        let (escape, delete) = ctx.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::Delete)));
        // Outside sketches the keys are about the selected item of the cell.
        if session.active_sketch().is_none() {
            if escape {
                session.select_item(None);
                self.origin_selected = None;
            }
            if delete && let Some(item) = session.selected_item() {
                // A refusal (still in use) is explained in the status bar.
                drop(session.delete_item(item));
            }
            return;
        }
        if escape {
            self.hud.cancel();
            session.tool_event(ToolEvent::Key(ToolKey::Escape));
        }
        if delete {
            session.tool_event(ToolEvent::Key(ToolKey::Delete));
        }
    }
}

/// The context menu of an item of the cell: its name, show or hide, plan (a path), delete.
pub fn item_menu(ui: &mut egui::Ui, session: &mut Session, lang: Lang, item: gc_session::CellItem) {
    let Some(name) = session.item_name(item) else {
        ui.close();
        return;
    };
    ui.label(egui::RichText::new(&name).strong());
    ui.separator();
    if let Some(shown) = item.shown()
        && let Some(look) = session.shown_look(shown)
    {
        let label = if look.hidden { tr!(lang, CmdShow) } else { tr!(lang, CmdHide) };
        if ui.button(label).clicked() {
            drop(session.set_look(shown, gc_doc::Look { hidden: !look.hidden, ..look }));
            ui.close();
        }
    }
    if let gc_session::CellItem::Path(path) = item
        && ui.button(tr!(lang, CmdPlan)).clicked()
    {
        session.plan(path);
        ui.close();
    }
    if ui.button(tr!(lang, CmdDeleteItem, name)).clicked() {
        // A refusal (still in use) is explained in the status bar.
        drop(session.delete_item(item));
        ui.close();
    }
}

/// The distance on screen from `p` to the segment from `a` to `b`, in pixels.
fn segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let length = d[0] * d[0] + d[1] * d[1];
    let t = if length > 0.0 {
        (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let q = [a[0] + t * d[0] - p[0], a[1] + t * d[1] - p[1]];
    (q[0] * q[0] + q[1] * q[1]).sqrt()
}

/// Selects what a selection box takes: dragged from left to right only what lies wholly inside (window),
/// from right to left also what the box touches (crossing). With Ctrl or Shift the selection is kept and
/// each item found is toggled; otherwise it is replaced.
fn select_in_box(session: &mut Session, a: [f64; 2], b: [f64; 2], crossing: bool, keep: bool) {
    let (lo, hi) = ([a[0].min(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].max(b[1])]);
    let inside = |p: [f64; 3]| p[0] >= lo[0] && p[0] <= hi[0] && p[1] >= lo[1] && p[1] <= hi[1];
    let overlaps = |min: [f64; 2], max: [f64; 2]| {
        max[0] >= lo[0] && min[0] <= hi[0] && max[1] >= lo[1] && min[1] <= hi[1]
    };
    let wholly = |min: [f64; 2], max: [f64; 2]| {
        min[0] >= lo[0] && max[0] <= hi[0] && min[1] >= lo[1] && max[1] <= hi[1]
    };
    let Some((_, sketch)) = session.active_sketch() else { return };
    let mut found: Vec<gc_sketch::Item> = Vec::new();
    for id in sketch.points.keys() {
        if session.point_pos(*id).is_some_and(inside) {
            found.push(gc_sketch::Item::Point(*id));
        }
    }
    for (id, curve) in &sketch.curves {
        let pos = |p| session.point_pos(p);
        let bounds = match curve.geom {
            CurveGeom::Line { a, b } => pos(a).zip(pos(b)).map(|(p, q)| {
                ([p[0].min(q[0]), p[1].min(q[1])], [p[0].max(q[0]), p[1].max(q[1])], Some((p, q)))
            }),
            CurveGeom::Circle { c, .. } | CurveGeom::Arc { c, .. } => {
                let r = session.radius(*id).unwrap_or(0.0);
                pos(c).map(|c| ([c[0] - r, c[1] - r], [c[0] + r, c[1] + r], None))
            }
        };
        let Some((min, max, ends)) = bounds else { continue };
        let hit = if crossing {
            // A line is taken when an end is inside or it crosses the box; a round curve by its bounds.
            match ends {
                Some((p, q)) => inside(p) || inside(q) || segment_crosses_box(p, q, lo, hi),
                None => overlaps(min, max),
            }
        } else {
            wholly(min, max)
        };
        if hit {
            found.push(gc_sketch::Item::Curve(*id));
        }
    }
    if !keep {
        session.tool_event(ToolEvent::ClickItem { item: None, pos: [0.0; 2], modifier: false });
    }
    for item in found {
        session.tool_event(ToolEvent::ClickItem { item: Some(item), pos: [0.0; 2], modifier: true });
    }
}

/// Whether the segment `p`–`q` passes through the box `lo`–`hi` (Liang–Barsky).
fn segment_crosses_box(p: [f64; 3], q: [f64; 3], lo: [f64; 2], hi: [f64; 2]) -> bool {
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    let delta = [q[0] - p[0], q[1] - p[1]];
    for axis in 0..2 {
        let (origin, d) = (p[axis], delta[axis]);
        if d.abs() < 1e-15 {
            if origin < lo[axis] || origin > hi[axis] {
                return false;
            }
        } else {
            let (mut a, mut b) = ((lo[axis] - origin) / d, (hi[axis] - origin) / d);
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            t0 = t0.max(a);
            t1 = t1.min(b);
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}
