//! The viewport: view navigation, sketch tool input and drawing of the scene.
//!
//! Solids (bodies, robot links) come from the renderer's offscreen target when a GPU is present. The grid
//! is painted under that image and sketches, dimensions, traces and tool previews over it with egui's
//! painter, through the same camera. Without a GPU everything still works; only the solids are missing.

mod paint;
mod target;

use egui::{Key, PointerButton, Sense};
use gc_core::Id;
use gc_doc::Sketch;
use gc_draw::{Camera, NavAction, NavInput, NavPreset, StandardView, nav_action};
use gc_i18n::Lang;
use gc_math::{Aabb, DPose3, DVec3, Plane};
use gc_scene::Scene;
use gc_session::Session;
use gc_sketch::Space;
use gc_tools::{Key as ToolKey, ToolEvent};

pub use paint::{format_angle, format_length};
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

/// The viewport widget and its view state.
pub struct Viewport {
    background: Background,
    /// The view camera.
    pub camera: Camera,
    /// The navigation style.
    pub preset: NavPreset,
    navigating: Option<NavAction>,
    viewed_sketch: Option<Id<Sketch>>,
    value_text: String,
    value_open: bool,
}

impl Viewport {
    /// A viewport that renders its background with `render_state` when there is one.
    pub fn new(render_state: Option<egui_wgpu::RenderState>) -> Self {
        Self {
            background: Background::new(render_state),
            camera: Camera { distance: 0.35, ..Camera::default() },
            preset: NavPreset::Inventor,
            navigating: None,
            viewed_sketch: None,
            value_text: String::new(),
            value_open: false,
        }
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
        self.camera.fit(&bounds, size);
    }

    /// Shows the viewport and feeds the user's input to the view and to the session's tool.
    pub fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session, lang: Lang) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let to_view = |p: egui::Pos2| [f64::from(p.x - rect.left()), f64::from(p.y - rect.top())];

        // Entering a sketch turns the view to face its plane, as CAD users expect.
        let active = session.active_sketch().map(|(id, sketch)| (id, plane_of(session, sketch)));
        if self.viewed_sketch != active.map(|(id, _)| id) {
            self.viewed_sketch = active.map(|(id, _)| id);
            if let Some((_, plane)) = active {
                self.camera.look_along(-plane.normal, plane.v);
            }
        }

        self.navigate(ui, &response, rect, size);

        let cursor = response
            .hover_pos()
            .or_else(|| ui.input(|i| i.pointer.latest_pos()))
            .filter(|p| rect.contains(*p));
        if let (Some((_, plane)), None) = (active, self.navigating) {
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
            }
            if session.is_dragging() {
                ui.ctx().request_repaint();
            }
        }

        let scene = Scene::of(session);
        paint::under(ui, rect, &self.camera, session);
        self.background.paint(ui, rect, &self.camera, &scene);
        paint::over(ui, rect, &self.camera, session, &scene, lang);
        self.value_box(ui, rect, session, size);
    }

    /// Wheel zoom and the drag bindings of the navigation preset.
    fn navigate(&mut self, ui: &egui::Ui, response: &egui::Response, rect: egui::Rect, size: [f64; 2]) {
        let to_view = |p: egui::Pos2| [f64::from(p.x - rect.left()), f64::from(p.y - rect.top())];
        let (scroll, delta, modifiers, keys, pointer) = ui.input(|i| {
            let keys = (i.key_down(Key::F2), i.key_down(Key::F3), i.key_down(Key::F4));
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
        let at = self
            .camera
            .project(plane.world([request.text_pos[0], request.text_pos[1], 0.0]), size)
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

    /// Passes Escape and Delete to the tool. Call when no text field has the keyboard.
    pub fn keys(&mut self, ctx: &egui::Context, session: &mut Session) {
        let (escape, delete) = ctx.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::Delete)));
        if escape {
            session.tool_event(ToolEvent::Key(ToolKey::Escape));
        }
        if delete {
            session.tool_event(ToolEvent::Key(ToolKey::Delete));
        }
    }
}
