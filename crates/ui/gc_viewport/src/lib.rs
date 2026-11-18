//! The viewport: view navigation, sketch tool input and drawing of the scene.
//!
//! Solids (bodies, robot links) come from the renderer's offscreen target when a GPU is present. The grid
//! is painted under that image and sketches, dimensions, traces and tool previews over it with egui's
//! painter, through the same camera. Without a GPU everything still works; only the solids are missing.

mod altdrag;
mod dof;
mod freemove;
mod gizmo;
mod grip;
mod hud;
mod origin;
mod paint;
mod place;
mod radial;
mod repr;
mod section;
mod target;
mod workfeat;

use egui::{Key, PointerButton, Sense, vec2};
use gc_core::Id;
use gc_doc::{GripReference, Sketch};
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
pub use target::{Background, VisualStyle};

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
    /// The work feature the last click picked for a relationship dialog.
    constrain_work: Option<gc_core::Id<gc_doc::WorkFeature>>,
    /// A face, edge, point, cylinder or cone clicked while a constraint dialog is picking.
    constrain_selection: Option<gc_doc::Selection>,
    /// What the pointer is over while a constraint dialog is picking (shown highlighted).
    constrain_hover: Option<gc_session::Feature>,
    /// The navigation tool chosen on the navigation bar: it works with the left button until it ends.
    nav_tool: Option<NavTool>,
    /// The origin element (plane, axis, point) of a component that is selected in the view or the browser.
    pub origin_selected: Option<(gc_doc::Occurrence, gc_doc::OriginElement)>,
    /// The work feature selected in the view (drawn heavier; Delete removes it, a double click edits it).
    pub work_selected: Option<Id<gc_doc::WorkFeature>>,
    /// Select Other: where the list stands and what lies under the cursor there, nearest first.
    select_other: Option<(egui::Pos2, Vec<gc_session::CellItem>)>,
    /// The heads-up display of the drawing tools.
    hud: hud::Hud,
    /// Whether the heads-up display (dynamic input) is on.
    pub hud_on: bool,
    /// Free Move or Free Rotate, while on.
    pub free_tool: Option<FreeTool>,
    /// A component being dragged with it.
    free_drag: Option<freemove::FreeDrag>,
    placing: Option<place::Placing>,
    /// A paste waiting for the click that gives its place.
    placing_paste: Option<place::PastePlacing>,
    /// What a plain click selects (the Select command's priority menu, opened with Shift + right click).
    pub select_priority: SelectPriority,
    /// Where the Select menu is open.
    select_menu: Option<egui::Pos2>,
    /// Where the view has stood (F5 goes back, Shift + F5 forward), the one shown now, and when the
    /// camera last changed.
    views: ViewHistory,
    /// Bend (3D sketch) is running: the lines picked so far and the radius typed.
    bend: Option<BendState>,
    /// Helical Curve (3D sketch): the dialog, then the start and end clicks.
    helix: Option<HelixState>,
    /// Insert Point is running on this spline: each click adds a fit point, Esc ends.
    insert_spline: Option<Id<gc_sketch::Curve>>,
    /// Whether Alt was held when the primary button last went down.
    alt_at_press: bool,
    /// An Alt + drag that places a Mate constraint.
    alt_drag: Option<altdrag::AltDrag>,
    /// A constraint Alt + drag asks for: the component's geometry, then the one it goes to.
    quick_constrain: Option<(gc_doc::Selection, gc_doc::Selection)>,
    /// A Grip Snap in progress.
    grip: Option<grip::Grip>,
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
    /// The visual style of the 3D view (View tab, Appearance panel).
    pub style: VisualStyle,
    /// The ground plane (the floor grid) is shown; a sketch's own grid stays (View tab, Appearance panel;
    /// on and off are session-based).
    pub show_ground: bool,
    /// "Create Path" (sketch roles, plan 9.1): the robot, the first point and the straight-line choice.
    route: Option<RouteState>,
    /// The path the last "Create Path" made; the application takes it.
    route_made: Option<Id<gc_doc::MotionPath>>,
    /// The sketch role selected in the browser: marked strongly in the view.
    pub role_selected: Option<Id<gc_doc::SketchRole>>,
    /// The browser row the pointer is over: its component's bounding box is drawn.
    pub browser_hover: Option<gc_core::AnyId>,
    /// The ViewCube is shown (View tab, Windows panel, User Interface); the navigation bar keeps its own.
    pub show_view_cube: bool,
    /// The navigation bar is shown.
    pub show_nav_bar: bool,
    /// Right-click opens the classic context menu (a list) instead of the radial marking menu.
    pub classic_menu: bool,
    /// The Home view the ViewCube's menu saved (Set Current View as Home); the default is the isometric view.
    pub home: Option<Camera>,
    /// Where the ViewCube's menu is open.
    cube_menu: Option<(egui::Pos2, u8)>,
    /// ViewCube options: fit the model to the window when a part of the cube is clicked.
    pub cube_fit_on_view_change: bool,
    /// Object Visibility (View tab, Visibility panel).
    pub visibility: ObjectVisibility,
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

/// The views the camera has rested in: F5 returns to the previous one, Shift + F5 goes to the next
/// (Inventor 2026 help, shortcut table GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E).
#[derive(Clone, Debug, Default)]
struct ViewHistory {
    stack: Vec<Camera>,
    at: usize,
    /// The camera of the last frame and when it last differed from the one before.
    seen: Option<(Camera, f64)>,
}

impl ViewHistory {
    /// How long the camera rests before the view counts as one to return to, seconds.
    const REST: f64 = 0.35;
    const MAX: usize = 50;

    /// Follows the camera: a view it has rested in is remembered.
    fn follow(&mut self, camera: &Camera, time: f64) {
        match &mut self.seen {
            Some((seen, since)) if seen == camera => {
                if time - *since >= Self::REST && self.stack.get(self.at) != Some(camera) {
                    self.stack.truncate(self.at + 1);
                    self.stack.push(*camera);
                    if self.stack.len() > Self::MAX {
                        self.stack.remove(0);
                    }
                    self.at = self.stack.len() - 1;
                }
            }
            seen => *seen = Some((*camera, time)),
        }
        if self.stack.is_empty() {
            self.stack.push(*camera);
            self.at = 0;
        }
    }

    /// The previous (`back`) or next view, when there is one.
    fn step(&mut self, back: bool) -> Option<Camera> {
        let to = if back { self.at.checked_sub(1)? } else { self.at + 1 };
        let camera = *self.stack.get(to)?;
        self.at = to;
        self.seen = None;
        Some(camera)
    }
}

/// Points along a curve for picking it, closed ones too (a circle, a whole ellipse, a closed spline): the
/// run of an open curve ([`Sketch::curve_run`]) or the outline of a closed one, in sketch coordinates.
fn curve_outline(sketch: &Sketch, curve: Id<gc_sketch::Curve>) -> Option<Vec<[f64; 3]>> {
    if let Some(run) = sketch.curve_run(curve) {
        return Some(run);
    }
    let geom = &sketch.curves.get(&curve)?.geom;
    match geom {
        CurveGeom::Circle { .. } => {
            let (centre, radius) = sketch.round(curve)?;
            let c = sketch.pos(centre)?;
            Some(
                (0..=72)
                    .map(|k| {
                        let a = f64::from(k) / 72.0 * std::f64::consts::TAU;
                        let (sin, cos) = gc_math::fm::sin_cos(a);
                        [c[0] + radius * cos, c[1] + radius * sin, c[2]]
                    })
                    .collect(),
            )
        }
        CurveGeom::Ellipse { .. } => sketch
            .ellipse_geom_outline(geom, 96)
            .map(|(points, _)| points.into_iter().map(|p| [p[0], p[1], 0.0]).collect()),
        CurveGeom::Spline { .. } => sketch.spline_outline3(geom, 16),
        _ => None,
    }
}

/// The body or robot an id stands for.
fn cell_item(any: gc_core::AnyId) -> Option<gc_session::CellItem> {
    match any.kind {
        gc_core::Kind::Body => Some(gc_session::CellItem::Body(gc_core::Id::from_raw(any.raw))),
        gc_core::Kind::RobotInstance => Some(gc_session::CellItem::Robot(gc_core::Id::from_raw(any.raw))),
        _ => None,
    }
}

/// A box around everything drawn for a body or robot, in the world.
fn item_bounds(session: &Session, item: gc_session::CellItem) -> Aabb {
    let scene = Scene::of(session);
    let mut bounds = Aabb::EMPTY;
    for solid in scene.solids.iter().filter(|solid| solid.owner == Some(item)) {
        let local = solid.shape.local_bounds();
        for x in [local.min.x, local.max.x] {
            for y in [local.min.y, local.max.y] {
                for z in [local.min.z, local.max.z] {
                    bounds = bounds.including(solid.pose * DVec3::new(x, y, z));
                }
            }
        }
    }
    bounds
}

/// Object Visibility (Inventor 2026 help, To set visibility of work features, annotations, and other display
/// elements, GUID-BA193171-FAFC-4150-9222-CC43D8D4AA54): one switch for all instances of a kind. The origin
/// kinds start off (as Inventor's origin folders do): on, every origin element of that kind shows, on top of
/// the ones shown one by one. The others start on and only hide: what was hidden one by one stays hidden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectVisibility {
    /// Origin planes of every component.
    pub origin_planes: bool,
    /// Origin axes.
    pub origin_axes: bool,
    /// Origin centre points.
    pub origin_points: bool,
    /// Work planes.
    pub work_planes: bool,
    /// Work axes.
    pub work_axes: bool,
    /// Work points.
    pub work_points: bool,
    /// Inactive 2D sketches.
    pub sketches: bool,
    /// Inactive 3D sketches.
    pub sketches_3d: bool,
}

impl Default for ObjectVisibility {
    fn default() -> Self {
        Self {
            origin_planes: false,
            origin_axes: false,
            origin_points: false,
            work_planes: true,
            work_axes: true,
            work_points: true,
            sketches: true,
            sketches_3d: true,
        }
    }
}

impl ObjectVisibility {
    /// Whether the origin element's kind is switched on.
    pub(crate) fn origin(&self, element: gc_doc::OriginElement) -> bool {
        match element {
            gc_doc::OriginElement::Plane(_) => self.origin_planes,
            gc_doc::OriginElement::Axis(_) => self.origin_axes,
            gc_doc::OriginElement::Point => self.origin_points,
        }
    }

    /// Whether the work feature's kind is switched on.
    pub(crate) fn work(&self, shape: &gc_session::WorkShape) -> bool {
        match shape {
            gc_session::WorkShape::Plane(_) => self.work_planes,
            gc_session::WorkShape::Axis { .. } => self.work_axes,
            gc_session::WorkShape::Point(_) => self.work_points,
        }
    }
}

/// Create Path: two sketch points are clicked and a path of the robot between them is made and planned.
#[derive(Clone, Debug)]
struct RouteState {
    robot: Id<gc_doc::Robot>,
    first: Option<(Id<gc_doc::Sketch>, Id<gc_sketch::SkPoint>)>,
    /// The second move in a straight line of the tool tip.
    linear: bool,
}

/// The Bend command of a 3D sketch (Inventor 2026 help, To Create Lines, Arcs, and Bends,
/// GUID-4F861674-F74C-474E-869A-4C190ED6EC4B): two lines are clicked, then the radius is typed.
#[derive(Clone, Debug)]
struct BendState {
    lines: Vec<Id<gc_sketch::Curve>>,
    radius: String,
    fresh: bool,
    /// The Equal check box (default on): the next bends take the radius of the first one.
    equal: bool,
    /// The arc of the first bend made, and the radius typed for it: what Equal follows.
    first: Option<(Id<gc_sketch::Curve>, String)>,
}

/// The Helical Curve command of a 3D sketch (Inventor 2026 help, To Draw 3D Curves,
/// GUID-37BA1DCE-6BC4-41CD-A36D-577F31B304FC): the dialog's values as typed, then the start and end
/// points clicked.
#[derive(Clone, Debug)]
struct HelixState {
    /// 0 Pitch and Revolution, 1 Revolution and Height, 2 Pitch and Height, 3 Spiral.
    definition: usize,
    pitch: String,
    revolutions: String,
    height: String,
    diameter: String,
    taper: String,
    right_hand: bool,
    /// The dialog is done; the start and end are being clicked.
    placing: bool,
    start: Option<[f64; 3]>,
    hover: Option<[f64; 3]>,
}

impl HelixState {
    fn new() -> Self {
        Self {
            definition: 0,
            pitch: "10 mm".to_owned(),
            revolutions: "5".to_owned(),
            height: "50 mm".to_owned(),
            diameter: "20 mm".to_owned(),
            taper: "0 deg".to_owned(),
            right_hand: true,
            placing: false,
            start: None,
            hover: None,
        }
    }

    /// The operation for an axis from `start` to `end`; `None` while a value does not evaluate.
    fn op(&self, doc: &gc_doc::Document, start: [f64; 3], end: [f64; 3]) -> Option<gc_doc::SketchOp> {
        use gc_doc::{HelixShape, Unit, evaluate_value};
        let length = |text: &str| evaluate_value(doc, text, Unit::Mm);
        let number = |text: &str| evaluate_value(doc, text, Unit::Scalar);
        let shape = match self.definition {
            0 => HelixShape::PitchRevolution {
                pitch: length(&self.pitch)?,
                revolutions: number(&self.revolutions)?,
            },
            1 => HelixShape::RevolutionHeight {
                revolutions: number(&self.revolutions)?,
                height: length(&self.height)?,
            },
            2 => HelixShape::PitchHeight { pitch: length(&self.pitch)?, height: length(&self.height)? },
            _ => HelixShape::Spiral { pitch: length(&self.pitch)?, revolutions: number(&self.revolutions)? },
        };
        let taper = if self.definition == 3 { 0.0 } else { evaluate_value(doc, &self.taper, Unit::Deg)? };
        Some(gc_doc::SketchOp::AddHelix {
            start,
            end,
            diameter: length(&self.diameter)?,
            shape,
            taper,
            right_hand: self.right_hand,
        })
    }
}

/// What a plain click selects (Inventor 2026 help, About Component Selection Modes and Sets,
/// GUID-E3E49BD5-3F55-4836-8CD1-2C1855CF9986: the Select command's priority).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectPriority {
    /// Components: bodies, robots, waypoints and the rest (the default).
    #[default]
    Component,
    /// Work features and origin geometry only.
    Feature,
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
            constrain_work: None,
            constrain_selection: None,
            constrain_hover: None,
            nav_tool: None,
            origin_selected: None,
            work_selected: None,
            select_other: None,
            hud: hud::Hud::default(),
            hud_on: true,
            free_tool: None,
            free_drag: None,
            placing: None,
            placing_paste: None,
            bend: None,
            helix: None,
            insert_spline: None,
            alt_at_press: false,
            alt_drag: None,
            quick_constrain: None,
            grip: None,
            views: ViewHistory::default(),
            select_priority: SelectPriority::default(),
            select_menu: None,
            marking: gc_ui::marking::MarkingMenu::default(),
            marking_set: radial::Set::default(),
            request: None,
            section: None,
            slice: false,
            show_constraints: true,
            style: VisualStyle::default(),
            show_ground: true,
            route: None,
            route_made: None,
            role_selected: None,
            browser_hover: None,
            show_view_cube: true,
            show_nav_bar: true,
            classic_menu: false,
            home: None,
            cube_menu: None,
            cube_fit_on_view_change: false,
            visibility: ObjectVisibility::default(),
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

    /// Find in Window (browser right-click; Inventor 2026 help, Frame Generator browser,
    /// GUID-36AE9264-912D-4D3C-BB24-7EC16FA8777E: "the selected objects are found, zoomed, and centered in the
    /// graphics window"): frames the body or robot. False when it has nothing to show.
    pub fn find_in_window(&mut self, session: &Session, any: gc_core::AnyId) -> bool {
        let Some(item) = cell_item(any) else { return false };
        let bounds = item_bounds(session, item);
        if bounds.is_empty() {
            return false;
        }
        self.camera.pivot = bounds.center();
        self.camera.fit(&bounds, [1000.0, 700.0]);
        true
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
                if let Some(curve) = self.insert_spline {
                    // Insert Point: every click on the spline adds a fit point; Esc ends.
                    if ui.input(|i| i.key_pressed(Key::Escape)) {
                        self.insert_spline = None;
                    } else if response.clicked_by(PointerButton::Primary)
                        && let Some(pos) = response.interact_pointer_pos().and_then(on_sketch)
                        && near_spline(session, curve, pos, gc_tools::tolerance::CURVE_PX * px)
                    {
                        // Only a click on the spline (within the pick distance) inserts a point; a refusal
                        // is explained in the status bar.
                        drop(session.edit_active_sketch(gc_doc::SketchOp::InsertSplinePoint {
                            curve,
                            at: [pos[0], pos[1]],
                        }));
                    }
                    // The hint is a label (it has an accessibility node), not painted text.
                    egui::Area::new(egui::Id::unique("gc_insert_spline_hint"))
                        .fixed_pos(self.rect.center_top() + egui::vec2(0.0, 14.0))
                        .pivot(egui::Align2::CENTER_TOP)
                        .order(egui::Order::Foreground)
                        .interactable(false)
                        .show(ui.ctx(), |ui| {
                            ui.label(
                                egui::RichText::new(tr!(lang, SplineInsertHint))
                                    .size(13.0)
                                    .color(colors::ACCENT),
                            );
                        });
                } else if response.clicked_by(PointerButton::Primary)
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
            self.cell_input(ui, &response, rect, session, lang);
        }

        self.select_other_list(ui, session);
        self.select_menu(ui, session, size, lang);
        self.bend_box(ui, session, rect, lang);
        self.helix_dialog(ui, session, lang);

        // The marking menu: a right press opens it (in a sketch or among the components); a quick mark
        // toward a command picks it, otherwise the radial menu and its overflow list show.
        let secondary_pressed = ui.input(|i| i.pointer.button_pressed(PointerButton::Secondary));
        // Shift + right click lists what lies under the cursor (Select Other), as Inventor's Select menu does
        // (docs/ux/inventor-parity.md: Shift+우클릭 Select 메뉴).
        let shift_menu = secondary_pressed
            && ui.input(|i| {
                i.modifiers.shift
                    || i.events.iter().any(|event| {
                        matches!(
                            event,
                            egui::Event::PointerButton {
                                button: PointerButton::Secondary,
                                pressed: true,
                                modifiers,
                                ..
                            } if modifiers.shift
                        )
                    })
            });
        if shift_menu
            && active.is_none()
            && let Some(at) = response.hover_pos()
        {
            self.select_menu = Some(at);
        }
        // The ViewCube has a menu of its own.
        let over_cube = self.cube_centre.is_some_and(|c| {
            ui.input(|i| i.pointer.latest_pos())
                .is_some_and(|p| egui::Rect::from_center_size(c, egui::Vec2::splat(124.0)).contains(p))
        });
        if secondary_pressed && over_cube && !shift_menu {
            self.cube_menu = ui.input(|i| i.pointer.latest_pos()).map(|p| (p, 0));
        }
        if secondary_pressed
            && !shift_menu
            && !over_cube
            && response.hovered()
            && self.navigating.is_none()
            && !self.marking.is_open()
        {
            let at = ui.input(|i| i.pointer.press_origin().or(i.pointer.interact_pos()));
            self.marking_set = if let Some((_, plane)) = active.as_ref() {
                // What the spline under the cursor offers: Split Spline at an inner fit point, Close Curve at
                // the start of an open one, Insert Point on the curve.
                let mut menu = radial::SplineMenu::default();
                if let Some(at) = at {
                    let at = to_view(at);
                    let near_point = |point: Id<gc_sketch::SkPoint>| -> bool {
                        session
                            .point_pos(point)
                            .and_then(|local| self.camera.project(plane.world(local), size))
                            .is_some_and(|s| {
                                gc_math::fm::hypot(s[0] - at[0], s[1] - at[1])
                                    <= gc_tools::tolerance::POINT_PX
                            })
                    };
                    // Sketch roles (plan 9.1): the point or the open curve under the cursor can become a
                    // waypoint or a path of the robot work.
                    if let Some((sketch_id, sketch)) = session.active_sketch() {
                        menu.waypoint = sketch
                            .points
                            .keys()
                            .copied()
                            .find(|p| near_point(*p) && session.waypoint_role(sketch_id, *p).is_none())
                            .map(|p| (sketch_id, p));
                        if menu.waypoint.is_none() {
                            let mut best: Option<(f64, Id<gc_sketch::Curve>)> = None;
                            for curve in sketch.curves.keys() {
                                if session.path_role(sketch_id, *curve).is_some() {
                                    continue;
                                }
                                let Some(run) = curve_outline(sketch, *curve) else { continue };
                                let on_screen: Vec<[f64; 2]> = run
                                    .iter()
                                    .filter_map(|p| self.camera.project(plane.world(*p), size))
                                    .collect();
                                let distance = on_screen
                                    .windows(2)
                                    .map(|w| segment_distance(at, w[0], w[1]))
                                    .fold(f64::INFINITY, f64::min);
                                if distance <= 6.0 && best.is_none_or(|(d, _)| distance < d) {
                                    best = Some((distance, *curve));
                                }
                            }
                            menu.path = best
                                .filter(|(_, curve)| !session.chain_at(sketch_id, *curve).is_empty())
                                .map(|(_, curve)| (sketch_id, curve));
                        }
                    }
                    if let Some((_, sketch)) = session.active_sketch() {
                        for (curve, c) in &sketch.curves {
                            let gc_sketch::CurveGeom::Spline { points } = &c.geom else { continue };
                            let closed = points.len() > 1 && points.first() == points.last();
                            if let Some(inner) = points.get(1..points.len().saturating_sub(1))
                                && let Some(point) = inner.iter().find(|p| near_point(**p))
                            {
                                menu.split = Some((*curve, *point));
                            }
                            if !closed && points.len() >= 3 && points.first().is_some_and(|p| near_point(*p))
                            {
                                menu.close = Some(*curve);
                            }
                            // On the curve itself: the outline in pixels.
                            if let Some(outline) = sketch.spline_outline(&c.geom, 16) {
                                let on_screen: Vec<[f64; 2]> = outline
                                    .iter()
                                    .filter_map(|p| self.camera.project(plane.world([p[0], p[1], 0.0]), size))
                                    .collect();
                                let near =
                                    on_screen.windows(2).any(|w| segment_distance(at, w[0], w[1]) <= 6.0);
                                if near {
                                    menu.insert = Some(*curve);
                                }
                            }
                        }
                    }
                }
                radial::Set::sketch(session.sketch_is_spatial(), menu)
            } else {
                // What is under the cursor is selected, and decides the short menu.
                let view = at.map(to_view);
                let origin =
                    view.and_then(|p| origin::pick(&self.visibility, &self.camera, size, session, p, false));
                let item =
                    if origin.is_some() { None } else { view.and_then(|p| self.pick_item(session, p, size)) };
                self.origin_selected = origin;
                session.select_item(item);
                // The rigid or flexible subassembly the item sits in, for the Flexible command.
                let unit = item
                    .and_then(grip::component_of)
                    .and_then(|any| session.pick_unit(gc_doc::Occurrence::Item(any)))
                    .and_then(|key| match key {
                        gc_doc::TreeKey::Subassembly(id) => session
                            .doc()
                            .subassemblies()
                            .iter()
                            .find(|s| s.id == id)
                            .map(|s| (id, s.flexible)),
                        gc_doc::TreeKey::Component(_) => None,
                    });
                radial::Set::assembly(item, origin, unit, session.is_isolated())
            };
            if let Some(at) = at {
                self.marking.press(at, ui.input(|i| i.time));
            }
        }
        // While the menu offers "Make Path", the chain it would take is drawn thick, so that the user sees what
        // becomes the path before choosing.
        if self.marking.is_open()
            && let Some(radial::Cmd::AssignPath(sketch_id, curve)) =
                self.marking_set.overflow.iter().copied().find(|c| matches!(c, radial::Cmd::AssignPath(..)))
            && let Some(sketch) = session.doc().get(sketch_id)
        {
            let plane = plane_of(session, sketch);
            let painter = ui.painter_at(rect);
            for member in session.chain_at(sketch_id, curve) {
                let Some(run) = curve_outline(sketch, member) else { continue };
                let points: Vec<egui::Pos2> = run
                    .iter()
                    .filter_map(|p| self.camera.project(plane.world(*p), size))
                    .map(|q| {
                        #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
                        egui::pos2(rect.left() + q[0] as f32, rect.top() + q[1] as f32)
                    })
                    .collect();
                for w in points.windows(2) {
                    painter.line_segment([w[0], w[1]], egui::Stroke::new(4.0, colors::SELECTED));
                }
            }
        }
        if self.marking.is_open() {
            // The classic context menu lists every command, the radial ones first (Inventor 2026 help, Reference for
            // Marking Menu Tab of Customize Dialog, GUID-1A2766FB-EB3E-476B-9C44-D500A5137478: Use Classic Context Menu).
            let classic = self.classic_menu;
            let listed: Vec<radial::Cmd> = if classic {
                self.marking_set
                    .radial
                    .iter()
                    .flatten()
                    .chain(self.marking_set.overflow.iter())
                    .copied()
                    .collect()
            } else {
                Vec::new()
            };
            let radial: Vec<Option<gc_ui::marking::MarkingItem>> = if classic {
                Vec::new()
            } else {
                self.marking_set.radial.iter().map(|cmd| cmd.map(|cmd| cmd.item(session, lang))).collect()
            };
            let overflow: Vec<gc_ui::marking::MarkingItem> = if classic {
                listed.iter().map(|cmd| cmd.item(session, lang)).collect()
            } else {
                self.marking_set.overflow.iter().map(|cmd| cmd.item(session, lang)).collect()
            };
            let picked = self.marking.show(ui.ctx(), &radial, &overflow);
            let cmd = picked.and_then(|pick| match pick {
                gc_ui::marking::MarkingPick::Radial(sector) => {
                    self.marking_set.radial.get(sector).copied().flatten()
                }
                gc_ui::marking::MarkingPick::Overflow(index) if classic => listed.get(index).copied(),
                gc_ui::marking::MarkingPick::Overflow(index) => self.marking_set.overflow.get(index).copied(),
            });
            if let Some(cmd) = cmd {
                match radial::run(cmd, session) {
                    radial::Effect::None => {}
                    radial::Effect::Request(request) => self.request = Some(request),
                    radial::Effect::InsertSplinePoint(curve) => self.insert_spline = Some(curve),
                    radial::Effect::LookAt => {
                        self.look_at(session);
                    }
                    radial::Effect::SelectOther => {
                        if let Some(at) = ui.input(|i| i.pointer.latest_pos()) {
                            self.open_select_other(session, at, size, &to_view);
                        }
                    }
                    radial::Effect::Home => self.go_home(),
                    radial::Effect::Fit => self.fit(session, [1000.0, 700.0]),
                    radial::Effect::Free(tool) => self.free_tool = Some(tool),
                }
            }
        }

        let scene = Scene::of(session);
        paint::under(ui, rect, &self.camera, session, self.show_ground);
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
        self.background.paint(ui, rect, &self.camera, &scene, slice, self.style);
        paint::over(
            ui,
            rect,
            &self.camera,
            session,
            &scene,
            self.background.has_gpu(),
            self.show_constraints,
            &self.visibility,
        );
        {
            let camera = self.camera;
            let screen = |world: DVec3| {
                let p = camera.project(world, size)?;
                #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
                Some(egui::pos2(rect.left() + p[0] as f32, rect.top() + p[1] as f32))
            };
            origin::paint(
                &self.visibility,
                &ui.painter_at(rect),
                &screen,
                session,
                self.origin_selected,
                self.constrain_mode,
            );
            dof::paint(&ui.painter_at(rect), &screen, session, camera.distance * 0.12);
            if let Some(feature) = &self.constrain_hover {
                paint_pick(&ui.painter_at(rect), &screen, feature, camera.distance * 0.1);
            }
            workfeat::paint(
                &self.visibility,
                &ui.painter_at(rect),
                &screen,
                session,
                camera.distance * 0.6,
                self.work_selected,
            );
        }
        // Sketch roles: a waypoint is a diamond with the tool's pointing axis, a path a line with an arrow
        // head; the one selected in the browser is drawn thick.
        {
            let painter = ui.painter_at(rect);
            let screen = |world: [f64; 3]| {
                self.camera.project(DVec3::from_array(world), size).map(|q| {
                    #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
                    egui::pos2(rect.left() + q[0] as f32, rect.top() + q[1] as f32)
                })
            };
            for (id, role) in session.doc().roles().iter() {
                if !session.doc().is_visible(role.sketch.erase()) || session.doc().is_inactive(id.erase()) {
                    continue;
                }
                let chosen = self.role_selected == Some(id);
                let color = if chosen { colors::SELECTED } else { colors::ACCENT.gamma_multiply(0.8) };
                match role.kind {
                    gc_doc::RoleKind::Waypoint { .. } => {
                        if let Some(pose) = session.role_pose(id)
                            && let Some(at) = screen(pose.translation.to_array())
                        {
                            let r = if chosen { 7.0 } else { 5.0 };
                            painter.add(egui::Shape::convex_polygon(
                                vec![
                                    at + egui::vec2(0.0, -r),
                                    at + egui::vec2(r, 0.0),
                                    at + egui::vec2(0.0, r),
                                    at + egui::vec2(-r, 0.0),
                                ],
                                color.gamma_multiply(0.35),
                                egui::Stroke::new(if chosen { 2.0 } else { 1.2 }, color),
                            ));
                            // The tool's pointing direction (its Z).
                            let tip =
                                pose.translation + pose.rotation * DVec3::Z * (self.camera.distance * 0.06);
                            if let Some(end) = screen(tip.to_array()) {
                                painter.line_segment([at, end], egui::Stroke::new(1.2, color));
                            }
                        }
                    }
                    gc_doc::RoleKind::Path { .. } => {
                        if let Some(line) = session.role_polyline(id) {
                            let points: Vec<egui::Pos2> = line.iter().filter_map(|p| screen(*p)).collect();
                            let stroke = egui::Stroke::new(if chosen { 3.5 } else { 2.0 }, color);
                            for w in points.windows(2) {
                                painter.line_segment([w[0], w[1]], stroke);
                            }
                            // An arrow head at the far end shows the direction.
                            if let [.., a, b] = points[..] {
                                let d = (b - a).normalized();
                                let n = egui::vec2(-d.y, d.x);
                                painter.add(egui::Shape::convex_polygon(
                                    vec![b, b - d * 11.0 + n * 5.0, b - d * 11.0 - n * 5.0],
                                    color,
                                    egui::Stroke::NONE,
                                ));
                            }
                        }
                    }
                }
            }
        }
        // Hovering a browser row shows the component's bounding box in the graphics.
        if let Some(item) = self.browser_hover.and_then(cell_item) {
            let bounds = item_bounds(session, item);
            if !bounds.is_empty() {
                let painter = ui.painter_at(rect);
                let corner = |i: usize| {
                    let p = DVec3::new(
                        if i & 1 == 0 { bounds.min.x } else { bounds.max.x },
                        if i & 2 == 0 { bounds.min.y } else { bounds.max.y },
                        if i & 4 == 0 { bounds.min.z } else { bounds.max.z },
                    );
                    self.camera.project(p, size).map(|q| {
                        #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
                        egui::pos2(rect.left() + q[0] as f32, rect.top() + q[1] as f32)
                    })
                };
                let stroke = egui::Stroke::new(1.2, colors::ACCENT);
                for (a, b) in [
                    (0, 1),
                    (2, 3),
                    (4, 5),
                    (6, 7),
                    (0, 2),
                    (1, 3),
                    (4, 6),
                    (5, 7),
                    (0, 4),
                    (1, 5),
                    (2, 6),
                    (3, 7),
                ] {
                    if let (Some(a), Some(b)) = (corner(a), corner(b)) {
                        painter.line_segment([a, b], stroke);
                    }
                }
            }
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
        self.elastic_bands(ui, rect, session, size);
        if self.show_view_cube {
            self.view_cube(ui, rect, session, lang);
        } else {
            self.cube_centre = None;
        }
        if self.show_nav_bar {
            self.nav_bar(ui, rect, session, lang);
        }
        if self.perf_overlay {
            self.perf(ui, rect, lang);
        }
        self.value_box(ui, rect, session, size);
        self.dimension_box(ui, rect, session, size, lang);
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

    /// The work feature the last click picked for the Constrain or Assemble dialog, once (Inventor 2026
    /// help, GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E: a work feature is selectable geometry).
    /// The face, edge, point, cylinder or cone a click picked for a constraint dialog, in its component's frame.
    pub fn take_constrain_selection(&mut self) -> Option<gc_doc::Selection> {
        self.constrain_selection.take()
    }

    pub fn take_constrain_work(&mut self) -> Option<gc_core::Id<gc_doc::WorkFeature>> {
        self.constrain_work.take()
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
        let can_look = self.can_look_at(session);
        let mut act = None;
        for (index, (icon, label, tool)) in tools.iter().enumerate() {
            let cell =
                egui::Rect::from_min_size(top_left + vec2(0.0, button * index as f32), vec2(button, button));
            let enabled = !(index == 4 && !can_look);
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
                self.look_at(session);
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
                self.go_home();
            }
        }
    }

    /// What Look At is used on, from the current selection (Inventor 2026 help, Navigation Bar Reference,
    /// GUID-3B926737-92BA-46CF-B722-97F68651772B: "zooms and rotates the model to display the selected
    /// element planar to the screen or a selected edge or line horizontal to the screen"): a work feature, an
    /// origin plane, a selected curve of the sketch being edited, else that sketch.
    fn look_item(&self, session: &Session) -> Option<gc_session::LookItem> {
        use gc_session::LookItem;
        if let Some(id) = self.work_selected {
            return Some(LookItem::WorkFeature(id));
        }
        if let Some((occurrence, gc_doc::OriginElement::Plane(plane))) = self.origin_selected {
            return Some(LookItem::ComponentPlane(occurrence, plane));
        }
        let (sketch, _) = session.active_sketch()?;
        if session.sketch_is_spatial() {
            return None;
        }
        let curve = session.selection().iter().find_map(|item| match item {
            gc_sketch::Item::Curve(id) => Some(*id),
            _ => None,
        });
        Some(match curve {
            Some(curve) => LookItem::SketchCurve { sketch, curve },
            None => LookItem::Sketch(sketch),
        })
    }

    /// Where Look At would turn the view now, if there is anything to look at.
    fn look_target(&self, session: &Session) -> Option<gc_session::LookTarget> {
        session.look_target(self.look_item(session)?, self.camera.forward())
    }

    /// Whether Look At has something to turn to.
    pub fn can_look_at(&self, session: &Session) -> bool {
        self.look_target(session).is_some()
    }

    /// Look At: turns the view to show the selected element square to the screen (a line level) and centres
    /// and zooms on it. False when nothing selected can be looked at.
    pub fn look_at(&mut self, session: &Session) -> bool {
        let Some(target) = self.look_target(session) else { return false };
        self.camera.look_along(-target.toward_eye, target.up);
        self.camera.pivot = target.centre;
        if let Some(reach) = target.reach.filter(|r| *r > 1e-6) {
            let half = DVec3::splat(reach);
            self.camera
                .fit(&Aabb::from_points([target.centre - half, target.centre + half]), [1000.0, 700.0]);
        }
        true
    }

    /// The view cube in the top-right corner: shows how the view is turned; clicking a face, an edge or a
    /// corner looks at the model from there.
    fn view_cube(&mut self, ui: &mut egui::Ui, rect: egui::Rect, session: &Session, lang: Lang) {
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
            if self.cube_fit_on_view_change {
                self.fit(session, [1000.0, 700.0]);
            }
        }
        // The ViewCube's menu (Inventor 2026 help, To Reorient the View, GUID-94F44C80-7313-4529-85D1-E64ABF732700):
        // Set Current View as Home, Go Home, and the Fit-to-View option. (Front and Reset Front are not offered:
        // the cube's face directions are fixed.)
        if let Some((at, age)) = self.cube_menu.take() {
            let mut keep = true;
            let area = egui::Area::new(egui::Id::unique("gc_cube_menu"))
                .fixed_pos(at)
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        if ui.button(tr!(lang, CubeSetHome)).clicked() {
                            self.home = Some(self.camera);
                            keep = false;
                        }
                        if ui.button(tr!(lang, CubeGoHome)).clicked() {
                            self.go_home();
                            keep = false;
                        }
                        ui.checkbox(&mut self.cube_fit_on_view_change, tr!(lang, CubeFitOnChange));
                    });
                });
            // (The menu is not laid out yet on the frame that opens it.)
            let outside =
                age > 0 && ui.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer();
            if keep && !outside && !ui.input(|i| i.key_pressed(Key::Escape)) {
                self.cube_menu = Some((at, age.saturating_add(1)));
            }
        }
    }

    /// The relationships of components that Free Move or Free Rotate left out of place, as elastic bands from each
    /// component to where its relationship wants it (Inventor 2026 help, Move assembly components,
    /// GUID-9796C22B-23C1-4D69-BC2E-54056E46F255: "The Free Move command displays component relationships as an
    /// elastic band").
    fn elastic_bands(&self, ui: &egui::Ui, rect: egui::Rect, session: &Session, size: [f64; 2]) {
        if !session.needs_update() {
            return;
        }
        let painter = ui.painter_at(rect);
        for band in session.relationship_bands() {
            let [a, b] = band.ends;
            let (Some(a), Some(b)) = (self.camera.project(a, size), self.camera.project(b, size)) else {
                continue;
            };
            let at = |p: [f64; 2]| {
                #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
                egui::pos2(rect.left() + p[0] as f32, rect.top() + p[1] as f32)
            };
            let colour = if band.satisfied { colors::SELECTED } else { colors::ERROR };
            painter.line_segment([at(a), at(b)], egui::Stroke::new(1.5, colour));
        }
    }

    /// Go Home: the saved Home view, else the isometric view.
    pub fn go_home(&mut self) {
        match self.home {
            Some(home) => self.camera = home,
            None => self.camera.look(StandardView::Iso),
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
        // Lines, and arcs as short segments, for picking.
        let lines: Vec<(Id<gc_sketch::Curve>, DVec3, DVec3)> = sketch
            .curves
            .iter()
            .flat_map(|(id, curve)| {
                let at = |p| session.point_pos(p).map(DVec3::from_array);
                let segments: Vec<(Id<gc_sketch::Curve>, DVec3, DVec3)> = match curve.geom {
                    CurveGeom::Line { a, b } => {
                        at(a).zip(at(b)).map(|(a, b)| vec![(*id, a, b)]).unwrap_or_default()
                    }
                    CurveGeom::Arc { c, s, e } => (|| {
                        let arc = gc_sketch::Arc3::new(
                            session.point_pos(c)?,
                            session.point_pos(s)?,
                            session.point_pos(e)?,
                        )?;
                        let along = |i: u32| DVec3::from_array(arc.at(f64::from(i) / 24.0));
                        Some((0..24).map(|i| (*id, along(i), along(i + 1))).collect())
                    })()
                    .unwrap_or_default(),
                    CurveGeom::Circle { .. } | CurveGeom::Ellipse { .. } | CurveGeom::Spline { .. } => {
                        Vec::new()
                    }
                };
                segments
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
        if let Some(bend) = self.bend.as_mut() {
            // Bend: the nearest line or arc under the click is one of the two.
            if let Some(at) = clicked
                && bend.lines.len() < 2
            {
                let screen = to_view(at);
                let hit = lines
                    .iter()
                    .filter_map(|(id, a, b)| {
                        let (a, b) = (on_screen(*a)?, on_screen(*b)?);
                        Some((segment_distance(screen, a, b), *id))
                    })
                    .filter(|(d, _)| *d <= gc_tools::tolerance::CURVE_PX)
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, id)) = hit
                    && !bend.lines.contains(&id)
                {
                    bend.lines.push(id);
                }
            }
        } else if self.helix.as_ref().is_some_and(|h| h.placing) {
            // Helical Curve: the start point, then the end point that gives the axis direction.
            let anchor = self.helix.as_ref().and_then(|h| h.start).map(DVec3::from_array);
            let plane_axis = session.spatial_plane();
            let place = |at: egui::Pos2| {
                let ray = camera.ray(to_view(at), size);
                let ray = gc_math::Ray { origin: local(ray.origin), dir: turned(ray.dir) };
                gc_tools::spatial::place(&ray, anchor, plane_axis, &points, &away(to_view(at)))
            };
            if let Some(helix) = self.helix.as_mut() {
                helix.hover = cursor.and_then(place).map(|placed| placed.pos.to_array());
                if let Some(placed) = clicked.and_then(place) {
                    let at = placed.pos.to_array();
                    match helix.start {
                        None => helix.start = Some(at),
                        Some(start) => {
                            // A refusal (a value that does not evaluate, a zero-length axis) shows in the
                            // status bar and the command stays open.
                            let op = helix.op(session.doc(), start, at);
                            if let Some(op) = op
                                && session.edit_active_sketch(op).is_ok()
                            {
                                self.helix = None;
                            }
                        }
                    }
                }
            }
            if let Some(helix) = &self.helix
                && let (Some(from), Some(to)) = (helix.start, helix.hover)
                && let (Some(a), Some(b)) =
                    (on_screen(DVec3::from_array(from)), on_screen(DVec3::from_array(to)))
            {
                #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
                let pt = |p: [f64; 2]| egui::pos2(rect.left() + p[0] as f32, rect.top() + p[1] as f32);
                ui.painter().line_segment([pt(a), pt(b)], egui::Stroke::new(1.6, colors::PREVIEW));
            }
        } else if matches!(
            session.tool(),
            gc_tools::ToolKind::Line
                | gc_tools::ToolKind::Arc
                | gc_tools::ToolKind::Arc3
                | gc_tools::ToolKind::Spline
        ) {
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
        self.pick_feature_with(session, at, size, false)
    }

    /// [`Viewport::pick_feature`], with conical faces reported as `Feature::Cone` when `cones` is set (the picks
    /// that become constraint geometry; Measure does not measure cones).
    pub fn pick_feature_with(
        &mut self,
        session: &Session,
        at: [f64; 2],
        size: [f64; 2],
        cones: bool,
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
        if let Some((apex, axis, half_angle)) = hit.cone.filter(|_| cones) {
            let feature = Feature::Cone { apex: apex.to_array(), axis: axis.to_array(), half_angle };
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

    /// Opens the Select Other list at `at` (window pixels) when more than one component lies under it.
    fn open_select_other(
        &mut self,
        session: &Session,
        at: egui::Pos2,
        size: [f64; 2],
        to_view: &impl Fn(egui::Pos2) -> [f64; 2],
    ) {
        let scene = Scene::of(session);
        let ray = self.camera.ray(to_view(at), size);
        let found: Vec<gc_session::CellItem> =
            self.picker.pick_all(&scene, &ray).into_iter().map(|(_, item)| item).collect();
        self.select_other = (found.len() > 1).then_some((at, found));
    }

    /// The Select command's menu (Shift + right click; Inventor 2026 help, shortcut table
    /// GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E: "Activates the Select command menu"): the selection
    /// priority, and Select Other. Esc or a click elsewhere closes it.
    fn select_menu(&mut self, ui: &egui::Ui, session: &Session, size: [f64; 2], lang: Lang) {
        let Some(at) = self.select_menu.take() else { return };
        let mut open_other = false;
        let mut picked = false;
        let area = egui::Area::new(egui::Id::unique("gc_select_menu"))
            .fixed_pos(at)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    for (priority, label) in [
                        (SelectPriority::Component, tr!(lang, SelPriorityComponent)),
                        (SelectPriority::Feature, tr!(lang, SelPriorityFeature)),
                    ] {
                        if ui.selectable_label(self.select_priority == priority, label).clicked() {
                            self.select_priority = priority;
                            picked = true;
                        }
                    }
                    ui.separator();
                    open_other = ui.button(tr!(lang, SelOther)).clicked();
                });
            });
        let rect = self.rect;
        let to_view = |p: egui::Pos2| [f64::from(p.x - rect.left()), f64::from(p.y - rect.top())];
        if open_other {
            self.open_select_other(session, at, size, &to_view);
            return;
        }
        let outside = ui.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer();
        if !(outside || picked || ui.input(|i| i.key_pressed(Key::Escape))) {
            self.select_menu = Some(at);
        }
    }

    /// The Select Other list: what lies under the cursor, nearest first. Passing over a row highlights that
    /// component in the view; a click selects it; Esc or a click elsewhere closes the list.
    fn select_other_list(&mut self, ui: &egui::Ui, session: &mut Session) {
        let Some((at, items)) = self.select_other.take() else { return };
        let mut chosen = None;
        let mut hovered = None;
        let area = egui::Area::new(egui::Id::unique("gc_select_other"))
            .fixed_pos(at)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    for item in &items {
                        let name = session.item_name(*item).unwrap_or_else(|| "?".to_owned());
                        let row = ui.button(&name);
                        if row.hovered() {
                            hovered = Some(*item);
                        }
                        if row.clicked() {
                            chosen = Some(*item);
                        }
                    }
                });
            });
        session.hover_item(hovered);
        let outside = ui.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer();
        if let Some(item) = chosen {
            session.select_item(Some(item));
        } else if !(outside || ui.input(|i| i.key_pressed(Key::Escape))) {
            self.select_other = Some((at, items));
        }
    }

    /// Starts placing a component: it hangs on the cursor until Esc or a right click. `copy` marks one
    /// that is dropped when placing ends without a click.
    pub fn start_placing(&mut self, item: gc_session::CellItem, copy: bool) {
        self.placing = Some(place::Placing { item, copy });
        self.free_tool = None;
    }

    /// The constraint the last Alt + drag asked for, once: the dragged component's geometry and the
    /// geometry it was dropped on.
    pub fn take_quick_constrain(&mut self) -> Option<(gc_doc::Selection, gc_doc::Selection)> {
        self.quick_constrain.take()
    }

    /// Alt + drag: press on a face or cylinder with Alt held, drag onto another component's face or
    /// cylinder, release to place the Mate (see [`altdrag`]). True while it runs.
    fn alt_drag_step(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        session: &mut Session,
        size: [f64; 2],
        to_view: &impl Fn(egui::Pos2) -> [f64; 2],
    ) -> bool {
        if self.alt_drag.is_none() {
            // Alt is read when the button goes down: the drag starts a few frames later.
            ui.input(|i| {
                for event in &i.events {
                    if let egui::Event::PointerButton {
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers,
                        ..
                    } = event
                    {
                        self.alt_at_press = modifiers.alt || i.modifiers.alt;
                    }
                }
            });
            let alt = self.alt_at_press;
            if alt
                && response.drag_started_by(PointerButton::Primary)
                && let Some(press) = ui.input(|i| i.pointer.press_origin()).map(to_view)
                && let Some((item, feature)) = self.pick_feature_with(session, press, size, true)
                && let Some(first) = altdrag::selection_of(session, item, &feature)
            {
                self.alt_drag = Some(altdrag::AltDrag { first, from: press, target: None });
            }
            return self.alt_drag.is_some();
        }
        let Some(mut drag) = self.alt_drag else { return false };
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.alt_drag = None;
            return true;
        }
        if let Some(at) = response.hover_pos().or_else(|| ui.input(|i| i.pointer.latest_pos())) {
            let at = to_view(at);
            drag.target = self
                .pick_feature_with(session, at, size, true)
                .and_then(|(item, feature)| altdrag::selection_of(session, item, &feature))
                .filter(|second| altdrag::offers(session, &drag.first, second))
                .map(|second| (second, at));
            ui.ctx().request_repaint();
        }
        if response.drag_stopped_by(PointerButton::Primary) {
            if let Some((second, _)) = drag.target {
                self.quick_constrain = Some((drag.first, second));
            }
            self.alt_drag = None;
            return true;
        }
        // The pull from the pressed geometry to the cursor, and the target it would be constrained to.
        let painter = ui.painter_at(self.rect);
        let origin = self.rect.min.to_vec2();
        #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
        let pos = |p: [f64; 2]| egui::pos2(p[0] as f32, p[1] as f32) + origin;
        let colour = if drag.target.is_some() {
            egui::Color32::from_rgb(0x2e, 0x9e, 0x4f)
        } else {
            egui::Color32::from_rgb(0x88, 0x88, 0x88)
        };
        let stroke = egui::Stroke::new(1.6, colour);
        let end = drag.target.map(|(_, at)| at).or_else(|| response.hover_pos().map(to_view));
        if let Some(end) = end {
            painter.line_segment([pos(drag.from), pos(end)], stroke);
            painter.circle_stroke(pos(drag.from), 6.0, stroke);
            if drag.target.is_some() {
                painter.circle_stroke(pos(end), 9.0, stroke);
            }
        }
        self.alt_drag = Some(drag);
        true
    }

    /// Starts Bend in the 3D sketch being edited: click two lines that meet, then type the radius.
    pub fn start_bend(&mut self) {
        self.bend = Some(BendState {
            lines: Vec::new(),
            radius: "10 mm".to_owned(),
            fresh: true,
            equal: true,
            first: None,
        });
    }

    /// How many lines the running Bend has picked so far.
    pub fn bend_picked(&self) -> usize {
        self.bend.as_ref().map_or(0, |bend| bend.lines.len())
    }

    /// Whether Bend is running.
    pub fn is_bending(&self) -> bool {
        self.bend.is_some()
    }

    /// Bend's prompt, and the radius box once both lines are picked. Enter makes the bend, Esc ends.
    fn bend_box(&mut self, ui: &egui::Ui, session: &mut Session, rect: egui::Rect, lang: Lang) {
        let Some(mut bend) = self.bend.take() else { return };
        if bend.lines.len() < 2 {
            // The radius box takes the keyboard each time it comes up.
            bend.fresh = true;
        }
        if !session.sketch_is_spatial() || ui.input(|i| i.key_pressed(Key::Escape)) {
            return;
        }
        let prompt = match bend.lines.len() {
            0 => tr!(lang, BendPickFirst),
            1 => tr!(lang, BendPickSecond),
            _ => tr!(lang, BendRadius),
        };
        let mut make = false;
        // In the bottom-left corner, out of the way of the lines being picked.
        egui::Area::new(egui::Id::unique("gc_bend"))
            .fixed_pos(rect.left_bottom() + egui::vec2(14.0, -14.0))
            .pivot(egui::Align2::LEFT_BOTTOM)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.strong(tr!(lang, ToolBend));
                    ui.label(prompt);
                    ui.checkbox(&mut bend.equal, tr!(lang, BendEqual));
                    if bend.lines.len() == 2 {
                        let field = ui.add(egui::TextEdit::singleline(&mut bend.radius).desired_width(90.0));
                        field.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::TextInput, true, tr!(lang, BendRadius))
                        });
                        if std::mem::take(&mut bend.fresh) {
                            field.request_focus();
                        }
                        if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            make = true;
                        }
                    }
                });
            });
        if make && let [a, b] = bend.lines[..] {
            // With Equal on and the radius unchanged, a bend after the first takes the first one's radius
            // (Bend Equal); otherwise it is a Bend of the radius typed, and becomes the one to follow. A refusal
            // (lines that do not meet, a radius too big) is explained in the status bar; the command stays
            // open to try again, and after a bend it stays open for the next one (Esc ends).
            let follow = bend
                .first
                .as_ref()
                .filter(|(_, radius)| bend.equal && *radius == bend.radius)
                .map(|(arc, _)| *arc);
            let op = match follow {
                Some(to) => gc_doc::SketchOp::BendEqual { lines: [a, b], to },
                None => gc_doc::SketchOp::Bend { lines: [a, b], radius: bend.radius.clone() },
            };
            if let Ok(made) = session.edit_active_sketch(op) {
                if follow.is_none()
                    && let Some(arc) = made.iter().find_map(|c| match c {
                        gc_doc::Created::Curve(id) => Some(*id),
                        _ => None,
                    })
                {
                    bend.first = Some((arc, bend.radius.clone()));
                }
                bend.lines.clear();
            }
        }
        self.bend = Some(bend);
    }

    /// Starts Create Path for a robot: click a point of any visible sketch, then a second one (Esc ends).
    pub fn start_route(&mut self, robot: Id<gc_doc::Robot>) {
        self.route = Some(RouteState { robot, first: None, linear: false });
    }

    /// Whether Create Path is running.
    pub fn is_routing(&self) -> bool {
        self.route.is_some()
    }

    /// The path Create Path made since the last call.
    pub fn take_route_made(&mut self) -> Option<Id<gc_doc::MotionPath>> {
        self.route_made.take()
    }

    /// Create Path outside sketches: the prompt, the straight-line check box, and the two clicks.
    fn route_step(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        session: &mut Session,
        size: [f64; 2],
        to_view: &impl Fn(egui::Pos2) -> [f64; 2],
        rect: egui::Rect,
        lang: Lang,
    ) -> bool {
        let Some(mut route) = self.route.take() else { return false };
        if ui.input(|i| i.key_pressed(Key::Escape)) || response.secondary_clicked() {
            return true;
        }
        let prompt =
            if route.first.is_none() { tr!(lang, RoutePickFirst) } else { tr!(lang, RoutePickSecond) };
        egui::Area::new(egui::Id::unique("gc_route"))
            .fixed_pos(rect.left_bottom() + egui::vec2(14.0, -14.0))
            .pivot(egui::Align2::LEFT_BOTTOM)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.strong(tr!(lang, CmdRouteBetween));
                    ui.label(prompt);
                    ui.checkbox(&mut route.linear, tr!(lang, RouteLinear));
                });
            });
        if response.clicked_by(PointerButton::Primary)
            && let Some(at) = response.interact_pointer_pos()
        {
            let screen = to_view(at);
            // The nearest point of the sketches that are shown.
            let mut best: Option<(f64, Id<gc_doc::Sketch>, Id<gc_sketch::SkPoint>)> = None;
            for (sketch_id, sketch) in session.doc().sketches().iter() {
                if !session.doc().is_visible(sketch_id.erase()) {
                    continue;
                }
                let plane = plane_of(session, sketch);
                for point in sketch.points.keys() {
                    let Some(pos) = sketch.pos(*point) else { continue };
                    let Some(p) = self.camera.project(plane.world(pos), size) else { continue };
                    let d = gc_math::fm::hypot(p[0] - screen[0], p[1] - screen[1]);
                    if d <= gc_tools::tolerance::POINT_PX * 1.5 && best.is_none_or(|(b, ..)| d < b) {
                        best = Some((d, sketch_id, *point));
                    }
                }
            }
            if let Some((_, sketch, point)) = best {
                match route.first {
                    None => route.first = Some((sketch, point)),
                    Some(from) if from != (sketch, point) => {
                        let planner = session.planner_choices().first().map(|(id, ..)| id.clone());
                        // A refusal is explained in the status bar; the command stays for another try.
                        if let Some(planner) = planner
                            && let Ok(path) = session.route_between(
                                route.robot,
                                from,
                                (sketch, point),
                                &planner,
                                route.linear,
                            )
                        {
                            self.route_made = Some(path);
                            return true;
                        }
                    }
                    Some(_) => {}
                }
            }
        }
        self.route = Some(route);
        true
    }

    /// Starts Helical Curve in the 3D sketch being edited: the dialog first.
    pub fn start_helix(&mut self) {
        self.helix = Some(HelixState::new());
    }

    /// Whether Helical Curve is running.
    pub fn is_helixing(&self) -> bool {
        self.helix.is_some()
    }

    /// The Helical Curve dialog (Helical Shape tab: Constant, a Definition, values, Rotation) while it is
    /// open; once OK is pressed, the prompt for the two clicks. Esc ends.
    fn helix_dialog(&mut self, ui: &egui::Ui, session: &Session, lang: Lang) {
        let Some(mut helix) = self.helix.take() else { return };
        if !session.sketch_is_spatial() || ui.input(|i| i.key_pressed(Key::Escape)) {
            return;
        }
        let mut cancel = false;
        if helix.placing {
            let prompt =
                if helix.start.is_none() { tr!(lang, HelixPickStart) } else { tr!(lang, HelixPickEnd) };
            egui::Area::new(egui::Id::unique("gc_helix_prompt"))
                .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(prompt));
                });
        } else {
            let mut ok = false;
            egui::Window::new(tr!(lang, ToolHelix))
                .id(egui::Id::unique("gc_helix_dialog"))
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ui.ctx(), |ui| {
                    for (index, name) in [
                        tr!(lang, HelixPitchRevolution),
                        tr!(lang, HelixRevolutionHeight),
                        tr!(lang, HelixPitchHeight),
                        tr!(lang, HelixSpiral),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        ui.radio_value(&mut helix.definition, index, name);
                    }
                    ui.separator();
                    let d = helix.definition;
                    let field = |ui: &mut egui::Ui, name: &str, text: &mut String, on: bool| {
                        if on {
                            ui.horizontal(|ui| {
                                ui.label(name);
                                let edit = ui.add(egui::TextEdit::singleline(text).desired_width(90.0));
                                edit.widget_info(|| {
                                    egui::WidgetInfo::labeled(egui::Role::TextInput, true, name)
                                });
                            });
                        }
                    };
                    field(ui, tr!(lang, HelixDiameter), &mut helix.diameter, true);
                    field(ui, tr!(lang, HelixPitch), &mut helix.pitch, d != 1);
                    field(ui, tr!(lang, HelixRevolutions), &mut helix.revolutions, d != 2);
                    field(ui, tr!(lang, HelixHeight), &mut helix.height, d == 1 || d == 2);
                    field(ui, tr!(lang, HelixTaper), &mut helix.taper, d != 3);
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut helix.right_hand, false, tr!(lang, HelixLeftHand));
                        ui.radio_value(&mut helix.right_hand, true, tr!(lang, HelixRightHand));
                    });
                    ui.horizontal(|ui| {
                        ok = ui.button(tr!(lang, Ok)).clicked();
                        cancel = ui.button(tr!(lang, Cancel)).clicked();
                    });
                });
            if ok {
                helix.placing = true;
            }
        }
        if !cancel {
            self.helix = Some(helix);
        }
    }

    /// Starts Grip Snap: pick an edge, then two points.
    pub fn start_grip_snap(&mut self) {
        self.grip = Some(grip::Grip::new());
        self.free_tool = None;
    }

    /// Whether a Grip Snap is running.
    pub fn is_grip_snapping(&self) -> bool {
        self.grip.is_some()
    }

    /// Grip Snap's clicks and its panel. True while it is running, so that nothing else takes the click.
    fn grip_step(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        session: &mut Session,
        size: [f64; 2],
        to_view: &impl Fn(egui::Pos2) -> [f64; 2],
        lang: Lang,
    ) -> bool {
        let Some(mut grip) = self.grip else { return false };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) || response.secondary_clicked() {
            grip::cancel(session, &grip);
            self.grip = None;
            return true;
        }
        let pointer = response.hover_pos().map(to_view);
        let clicked = response.clicked_by(PointerButton::Primary);
        if let Some(target) = grip.target {
            if grip.from.is_none() {
                if clicked && let Some(at) = pointer {
                    let scene = Scene::of(session);
                    let hit = self.picker.pick_surface(&scene, &self.camera.ray(at, size)).map(|h| h.point);
                    grip.from = hit.or_else(|| {
                        grip::on_view_plane(&self.camera, size, grip::anchor(&target.reference), at)
                    });
                }
            } else if let (Some(from), false) = (grip.from, grip.fixed) {
                let to = pointer.and_then(|at| {
                    grip::end_point(&self.camera, size, &target.reference, grip.motion, from, at)
                });
                if let Some(to) = to {
                    grip.to = Some(to);
                    grip.pose = session.grip_snap_pose_on(
                        target.any,
                        grip.motion,
                        target.reference,
                        from.to_array(),
                        to.to_array(),
                    );
                    grip::show(session, &target, grip.pose.as_ref().or(Some(&target.start)));
                    ui.ctx().request_repaint();
                    if clicked && grip.pose.is_some() {
                        grip.fixed = true;
                    }
                }
            }
        } else if clicked
            && let Some(at) = pointer
            && let Some((item, feature)) = self.pick_feature(session, at, size)
            && let Some(any) = grip::component_of(item)
            && let Some(reference) = grip::reference_of(&feature)
            && let Some((start, _)) = gizmo::pose_of(session, item)
        {
            session.select_item(Some(item));
            grip.motion = grip::motions(&reference)[0];
            grip.target = Some(grip::Target { item, any, reference, start });
        }
        // The panel: what to pick next, move or rotate, the freedom report, and the ways to end.
        let mut end: Option<Option<bool>> = None;
        let mut motion = grip.motion;
        let anchor = self.rect.center_top() + egui::vec2(0.0, 12.0);
        egui::Area::new(egui::Id::unique("gc_grip_snap"))
            .fixed_pos(anchor)
            .pivot(egui::Align2::CENTER_TOP)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.strong(tr!(lang, GripTitle));
                    if let Some(target) = &grip.target {
                        ui.horizontal_wrapped(|ui| {
                            for option in grip::motions(&target.reference) {
                                ui.selectable_value(&mut motion, option, grip::motion_name(option, lang));
                            }
                        });
                    }
                    let hint = match (&grip.target, grip.from, grip.fixed) {
                        (None, _, _) => tr!(lang, GripPickEdge),
                        (Some(_), None, _) => tr!(lang, GripPickFrom),
                        (Some(_), Some(_), false) => tr!(lang, GripPickTo),
                        (Some(_), Some(_), true) => tr!(lang, GripReady),
                    };
                    ui.label(hint);
                    if let Some(target) = &grip.target {
                        // With both points: the snap itself; before: a line is judged by its direction alone.
                        let allowed = match (grip.from, grip.to) {
                            (Some(from), Some(to)) => session.grip_snap_allowed(
                                target.any,
                                motion,
                                target.reference,
                                from.to_array(),
                                to.to_array(),
                            ),
                            _ => match target.reference {
                                GripReference::Line(line) => session.grip_allows(target.any, motion, line),
                                _ => true,
                            },
                        };
                        let text = if allowed { tr!(lang, GripAllowed) } else { tr!(lang, GripBlocked) };
                        let colour = if allowed {
                            egui::Color32::from_rgb(0x2e, 0x9e, 0x4f)
                        } else {
                            egui::Color32::from_rgb(0xc9, 0x3b, 0x2f)
                        };
                        ui.colored_label(colour, text);
                    }
                    ui.horizontal(|ui| {
                        let ready = grip.pose.is_some();
                        if ui.add_enabled(ready, egui::Button::new(tr!(lang, GripDone))).clicked() {
                            end = Some(Some(false));
                        }
                        if ui.add_enabled(ready, egui::Button::new(tr!(lang, GripGround))).clicked() {
                            end = Some(Some(true));
                        }
                        if ui.button(tr!(lang, GripCancel)).clicked() {
                            end = Some(None);
                        }
                    });
                });
            });
        if motion != grip.motion {
            // A different snap starts again from its first point.
            if let Some(target) = &grip.target {
                grip::show(session, target, None);
            }
            grip = grip::Grip { motion, from: None, to: None, pose: None, fixed: false, ..grip };
        }
        match end {
            Some(Some(ground)) => {
                if let (Some(target), Some(pose)) = (&grip.target, &grip.pose) {
                    // The preview goes first; the commit is one undo step.
                    grip::show(session, target, None);
                    // A component of a rigid subassembly takes the rest of it along.
                    let moves = session.unit_moves(target.any, *pose);
                    session.grip_snap_commit(&moves, ground).ok();
                }
                self.grip = None;
            }
            Some(None) => {
                grip::cancel(session, &grip);
                self.grip = None;
            }
            None => self.grip = Some(grip),
        }
        true
    }

    /// Starts a paste (Ctrl+V): the copied components are pasted where the next click says, as one undo
    /// step; Esc or a right click does nothing. False when nothing copied can be placed.
    pub fn start_paste(&mut self, session: &Session) -> bool {
        let Some(anchor) = place::clipboard_anchor(session) else { return false };
        self.placing_paste = Some(place::PastePlacing { anchor });
        self.free_tool = None;
        true
    }

    /// Whether a component hangs on the cursor.
    pub fn is_placing(&self) -> bool {
        self.placing.is_some() || self.placing_paste.is_some()
    }

    /// Another copy of the component just set down, to hang on the cursor next. Only bodies are copied.
    fn attach_copy(&self, session: &mut Session, item: gc_session::CellItem) -> Option<place::Placing> {
        let gc_session::CellItem::Body(body) = item else { return None };
        let before: Vec<gc_core::AnyId> = session.doc().bodies().iter().map(|(id, _)| id.erase()).collect();
        session.copy_components(&[body.erase()], gc_doc::Placement::default(), false, false).ok()?;
        let copy =
            session.doc().bodies().iter().map(|(id, _)| id).find(|id| !before.contains(&id.erase()))?;
        Some(place::Placing { item: gc_session::CellItem::Body(copy), copy: true })
    }

    /// Outside sketches: the item under the cursor is highlighted, a click selects it, a right click
    /// opens its context menu.
    fn cell_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        session: &mut Session,
        lang: Lang,
    ) {
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let to_view = |p: egui::Pos2| [f64::from(p.x - rect.left()), f64::from(p.y - rect.top())];
        // The move and turn handles of the selected item come first: dragging them is not a selection.
        // (Not during Grip Snap or Free Move / Free Rotate: their drags are not drags of the handles.)
        let on_handles = self.grip.is_none()
            && self.free_tool.is_none()
            && gizmo::ui(ui, response, rect, &self.camera, session, &mut self.gizmo);
        let hovered = if on_handles {
            None
        } else {
            response.hover_pos().and_then(|p| self.pick_item(session, to_view(p), size))
        };
        if session.hover_item(hovered) {
            ui.ctx().request_repaint();
        }
        if !on_handles && self.alt_drag_step(ui, response, session, size, &to_view) {
            return;
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
            if let Some(mut drag) = self.free_drag {
                if response.dragged_by(PointerButton::Primary)
                    && let Some(at) = response.hover_pos().or_else(|| ui.input(|i| i.pointer.latest_pos()))
                {
                    freemove::update(&mut drag, tool, &self.camera, size, session, to_view(at));
                    self.free_drag = Some(drag);
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
        // Select Other (Ctrl+A over the view): the components under the cursor in a list, nearest first
        // (Inventor 2026 help, Predefined Command Alias and Shortcut Reference,
        // GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E: Ctrl+A activates Select Other).
        let select_other_key = ui.input(|i| {
            i.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Key { key: egui::Key::A, pressed: true, modifiers, .. } if modifiers.command
                )
            })
        });
        if select_other_key && let Some(at) = response.hover_pos() {
            self.open_select_other(session, at, size, &to_view);
        }
        if self.grip_step(ui, response, session, size, &to_view, lang) {
            return;
        }
        if self.route.is_some() && self.route_step(ui, response, session, size, &to_view, rect, lang) {
            return;
        }
        // Paste: a see-through ghost of the copy follows the cursor (nothing is made yet); a click pastes
        // there (one undo step), Esc or a right click does nothing.
        if let Some(paste) = self.placing_paste {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) || response.secondary_clicked() {
                session.end_paste_preview();
                self.placing_paste = None;
                return;
            }
            if let Some(at) = response.hover_pos()
                && let Some(point) = place::ground_hit(&self.camera, size, to_view(at), paste.anchor)
            {
                let by = point - paste.anchor;
                drop(session.preview_paste(gc_doc::Placement { xyz: by.to_array(), rpy: [0.0; 3] }));
                ui.ctx().request_repaint();
            }
            if response.clicked_by(PointerButton::Primary)
                && let Some(at) = response.interact_pointer_pos()
                && let Some(point) = place::ground_hit(&self.camera, size, to_view(at), paste.anchor)
            {
                let by = point - paste.anchor;
                // A refusal is explained in the status bar.
                drop(session.paste(gc_doc::Placement { xyz: by.to_array(), rpy: [0.0; 3] }));
                session.end_paste_preview();
                self.placing_paste = None;
            }
            return;
        }
        // Place Component: the component hangs on the cursor; a click sets it down and attaches a copy.
        if let Some(placing) = self.placing {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) || response.secondary_clicked() {
                // Ends: the copy that was still hanging is dropped (the first component stays).
                if placing.copy {
                    gizmo::commit(session, placing.item);
                    drop(session.delete_item(placing.item));
                }
                self.placing = None;
                return;
            }
            if let Some(at) = response.hover_pos() {
                place::follow(&placing, &self.camera, size, session, to_view(at));
                ui.ctx().request_repaint();
            }
            if response.clicked_by(PointerButton::Primary) {
                place::set_down(&placing, session);
                self.placing = self.attach_copy(session, placing.item);
            }
            return;
        }
        // A double click on a work feature edits it.
        if response.double_clicked_by(PointerButton::Primary)
            && !self.constrain_mode
            && let Some(at) = response.interact_pointer_pos().map(to_view)
        {
            let screen = |p: gc_math::DVec3| self.camera.project(p, size);
            if let Some(id) =
                workfeat::pick(&self.visibility, session, &screen, at, self.camera.distance * 0.6)
            {
                self.request = Some(ViewportRequest::EditWorkFeature(id));
                return;
            }
        }
        // A double click on a component edits it in place; a subassembly is edited as one unit.
        if response.double_clicked_by(PointerButton::Primary)
            && !self.constrain_mode
            && let Some(at) = response.interact_pointer_pos().map(to_view)
            && let Some(item) = self.pick_item(session, at, size)
            && let gc_session::CellItem::Body(_) | gc_session::CellItem::Robot(_) = item
        {
            let any = match item {
                gc_session::CellItem::Body(id) => id.erase(),
                gc_session::CellItem::Robot(id) => id.erase(),
                _ => return,
            };
            self.request = Some(ViewportRequest::EditUnit(gc_doc::Occurrence::Item(any)));
        }
        self.constrain_hover = if self.constrain_mode {
            response
                .hover_pos()
                .map(to_view)
                .and_then(|at| self.pick_feature_with(session, at, size, true))
                .map(|(_, feature)| feature)
        } else {
            None
        };
        if self.constrain_mode && response.clicked_by(PointerButton::Primary) {
            let at = response.interact_pointer_pos().map(to_view);
            // Work features that are shown come before the origin geometry.
            let screen = |p: gc_math::DVec3| self.camera.project(p, size);
            self.constrain_work = at.and_then(|at| {
                workfeat::pick(&self.visibility, session, &screen, at, self.camera.distance * 0.6)
            });
            self.constrain_pick = if self.constrain_work.is_some() {
                None
            } else {
                at.and_then(|at| origin::pick(&self.visibility, &self.camera, size, session, at, true))
            };
            // A face, edge, point, cylinder or cone of a component under the cursor wins over an origin plane that
            // this mode only offers (origin planes of every component are pickable here); a plane the user has
            // shown stays pickable.
            if let Some(at) = at
                && self.constrain_work.is_none()
                && matches!(self.constrain_pick, None | Some((_, gc_doc::OriginElement::Plane(_))))
            {
                let shown = origin::pick(&self.visibility, &self.camera, size, session, at, false);
                if shown.is_some() {
                    self.constrain_pick = shown;
                } else if let Some((item, feature)) = self.pick_feature_with(session, at, size, true)
                    && let Some(selection) = altdrag::selection_of(session, item, &feature)
                {
                    self.constrain_selection = Some(selection);
                    self.constrain_pick = None;
                }
            }
        } else if self.measure_features && response.clicked_by(PointerButton::Primary) {
            let at = response.interact_pointer_pos().map(to_view);
            self.measure_pick = Some(at.and_then(|at| self.pick_feature(session, at, size)));
        } else if !on_handles && response.clicked_by(PointerButton::Primary) {
            // Origin planes, axes and points that are shown come before the solids behind them.
            let at = response.interact_pointer_pos().map(to_view);
            // Work features that are shown come first, as the origin geometry does.
            let screen = |p: gc_math::DVec3| self.camera.project(p, size);
            self.work_selected = at.and_then(|at| {
                workfeat::pick(&self.visibility, session, &screen, at, self.camera.distance * 0.6)
            });
            self.origin_selected = if self.work_selected.is_some() {
                None
            } else {
                at.and_then(|at| origin::pick(&self.visibility, &self.camera, size, session, at, false))
            };
            let picked = if self.origin_selected.is_some()
                || self.work_selected.is_some()
                || self.select_priority == SelectPriority::Feature
            {
                None
            } else {
                response.interact_pointer_pos().and_then(|p| self.pick_item(session, to_view(p), size))
            };
            // A click on a role's mark (a waypoint's diamond, a path's line) selects the role when nothing else is
            // there; any other click lets go of it.
            let role = if picked.is_none() && self.origin_selected.is_none() && self.work_selected.is_none() {
                response.interact_pointer_pos().and_then(|p| self.pick_role(session, to_view(p), size))
            } else {
                None
            };
            self.role_selected = role;
            session.select_item(picked);
        }
    }

    /// The role whose mark is under a screen position: a waypoint's diamond or a path's line.
    fn pick_role(&self, session: &Session, at: [f64; 2], size: [f64; 2]) -> Option<Id<gc_doc::SketchRole>> {
        let screen = |world: [f64; 3]| self.camera.project(DVec3::from_array(world), size);
        let mut best: Option<(f64, Id<gc_doc::SketchRole>)> = None;
        for (id, role) in session.doc().roles().iter() {
            if !session.doc().is_visible(role.sketch.erase()) || session.doc().is_inactive(id.erase()) {
                continue;
            }
            let distance = match role.kind {
                gc_doc::RoleKind::Waypoint { .. } => session
                    .role_pose(id)
                    .and_then(|pose| screen(pose.translation.to_array()))
                    .map(|p| gc_math::fm::hypot(p[0] - at[0], p[1] - at[1]))
                    .filter(|d| *d <= 9.0),
                gc_doc::RoleKind::Path { .. } => session.role_polyline(id).and_then(|line| {
                    let points: Vec<[f64; 2]> = line.iter().filter_map(|p| screen(*p)).collect();
                    points
                        .windows(2)
                        .map(|w| segment_distance(at, w[0], w[1]))
                        .fold(None, |best: Option<f64>, d| Some(best.map_or(d, |b| b.min(d))))
                        .filter(|d| *d <= 6.0)
                }),
            };
            if let Some(d) = distance
                && best.is_none_or(|(b, _)| d < b)
            {
                best = Some((d, id));
            }
        }
        best.map(|(_, id)| id)
    }

    /// Opens the in-place editor of a dimension of the sketch being edited: the expression of a driving
    /// dimension, and the Driven check box of any.
    pub fn edit_dimension(&mut self, session: &Session, id: Id<gc_sketch::Dimension>) {
        // Default Parameter Input Display (Document Settings, Units tab): the box shows the expression as
        // typed, or the value to the document's precision.
        let settings = session.doc_settings();
        let source = session.active_sketch().and_then(|(_, sketch)| {
            let dimension = sketch.dimensions.get(&id)?;
            if settings.input_as_expression {
                return Some(dimension.source.clone());
            }
            let value = sketch.measure(&dimension.kind)?;
            Some(if matches!(dimension.kind, gc_sketch::DimKind::Angle { .. }) {
                settings.format_angle(value)
            } else {
                settings.format_length(value)
            })
        });
        if let Some(source) = source {
            self.dimension_edit = Some((id, source, true));
        }
    }

    /// The text of the dimension edit box that is open, if any.
    pub fn dimension_edit_text(&self) -> Option<&str> {
        self.dimension_edit.as_ref().map(|(_, text, _)| text.as_str())
    }

    /// The box in which the expression of an existing dimension is edited. Enter applies it; a refused
    /// expression keeps the box open (the status bar says why); Escape closes it.
    fn dimension_box(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        session: &mut Session,
        size: [f64; 2],
        lang: Lang,
    ) {
        let Some((id, mut text, fresh)) = self.dimension_edit.take() else { return };
        let Some((_, sketch)) = session.active_sketch() else { return };
        let Some(dimension) = sketch.dimensions.get(&id) else { return };
        let plane = plane_of(session, sketch);
        let at = self
            .camera
            .project(plane.world([dimension.text_pos[0], dimension.text_pos[1], 0.0]), size)
            .map_or(rect.center(), |p| egui::pos2(rect.left() + p[0] as f32, rect.top() + p[1] as f32));
        let driving = dimension.driving;
        let (mut submit, mut close) = (false, false);
        let mut set_driving = None;
        egui::Area::new(egui::Id::unique("gc_dimension_edit"))
            .fixed_pos(at)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    if driving {
                        let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(130.0));
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::TextInput, true, "dimension expression")
                        });
                        if fresh {
                            response.request_focus();
                        } else if response.lost_focus() && !ui.ui_contains_pointer() {
                            if ui.input(|i| i.key_pressed(Key::Enter)) {
                                submit = true;
                            } else {
                                close = true;
                            }
                        } else if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            submit = true;
                        }
                    } else if ui.input(|i| i.key_pressed(Key::Escape)) {
                        close = true;
                    }
                    // Driven Dimension (a reference dimension): measures without constraining (Inventor 2026
                    // help, To Add Dimensions, GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185).
                    let mut driven = !driving;
                    if ui.checkbox(&mut driven, tr!(lang, DimDriven)).changed() {
                        set_driving = Some(!driven);
                    }
                });
            });
        if let Some(driving) = set_driving {
            // Going back to driving is refused when it would over-constrain; the status bar says why.
            if session.edit_active_sketch(gc_doc::SketchOp::SetDriving { id, driving }).is_ok() {
                return;
            }
        }
        // A refused expression keeps the box and gives it the keyboard back for another try.
        let refused = submit && !apply_dimension_text(session, id, &text);
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
        // Page Up turns the view to the selected element (Look At).
        if ctx.input(|i| i.key_pressed(Key::PageUp)) {
            self.look_at(session);
        }
        // F5 returns to the previous view, Shift + F5 goes to the next.
        let time = ctx.input(|i| i.time);
        self.views.follow(&self.camera, time);
        let f5 = ctx.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Key { key: Key::F5, pressed: true, repeat: false, modifiers, .. } => {
                    Some(modifiers.shift)
                }
                _ => None,
            })
        });
        if let Some(shift) = f5
            && let Some(camera) = self.views.step(!shift)
        {
            self.camera = camera;
            ctx.request_repaint();
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
                self.work_selected = None;
            }
            if delete && let Some(id) = self.work_selected.take() {
                drop(session.execute(&gc_doc::Command::Cell(gc_doc::CellOp::DeleteWorkFeature(id))));
            } else if delete && let Some(item) = session.selected_item() {
                // A refusal (still in use) is explained in the status bar.
                drop(session.delete_item(item));
            }
            return;
        }
        if escape {
            self.hud.cancel();
            session.tool_event(ToolEvent::Key(ToolKey::Escape));
        }
        // Enter finishes a 3D spline through the fit points clicked.
        if session.sketch_is_spatial()
            && session.tool() == gc_tools::ToolKind::Spline
            && ctx.input(|i| i.key_pressed(Key::Enter))
        {
            session.spatial_finish();
        }
        if delete {
            session.tool_event(ToolEvent::Key(ToolKey::Delete));
        }
    }
}

/// Whether `pos` (sketch coordinates) is within `reach` of the spline `curve`.
fn near_spline(session: &Session, curve: Id<gc_sketch::Curve>, pos: [f64; 2], reach: f64) -> bool {
    let Some((_, sketch)) = session.active_sketch() else { return false };
    let Some(line) = sketch.curves.get(&curve).and_then(|c| sketch.spline_outline(&c.geom, 16)) else {
        return false;
    };
    line.windows(2).any(|w| {
        let (dx, dy) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
        let length = dx * dx + dy * dy;
        let t = if length > 0.0 {
            (((pos[0] - w[0][0]) * dx + (pos[1] - w[0][1]) * dy) / length).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (x, y) = (w[0][0] + dx * t - pos[0], w[0][1] + dy * t - pos[1]);
        x * x + y * y <= reach * reach
    })
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
            // Bounded by the box of the points along it.
            CurveGeom::Spline { .. } => sketch.spline_outline(&curve.geom, 16).and_then(|line| {
                let first = *line.first()?;
                let (min, max) = line.iter().fold((first, first), |(lo, hi), p| {
                    ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
                });
                Some((min, max, None))
            }),
            // Bounded by the circle of its larger radius.
            CurveGeom::Ellipse { c, major, minor, .. } => pos(c).zip(pos(major)).map(|(c, m)| {
                let r = ((m[0] - c[0]).powi(2) + (m[1] - c[1]).powi(2)).sqrt().max(minor.abs());
                ([c[0] - r, c[1] - r], [c[0] + r, c[1] + r], None)
            }),
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

/// Applies the text of the dimension box. `NAME = value` makes (or, when the name exists, changes) the
/// parameter and lets the dimension refer to it (Inventor 2026 help, To Add Dimensions,
/// GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185); anything else is an expression for the dimension itself.
/// Returns whether it was accepted; a refusal is explained in the status bar.
fn apply_dimension_text(session: &mut Session, id: Id<gc_sketch::Dimension>, text: &str) -> bool {
    if let Some((name, value)) = text.split_once('=')
        && !value.starts_with('=')
    {
        let (name, value) = (name.trim(), value.trim());
        let identifier = name.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_alphanumeric() || c == '_');
        if identifier && !value.is_empty() {
            let existing = session.doc().params().iter().find(|(_, p)| p.name == name).map(|(id, _)| id);
            // A new name is added and used in one undo step; a name that is not allowed (a digit first, a
            // reserved word) is refused whole.
            return match existing {
                None => session.edit_dimension_with_param(id, name, value).is_ok(),
                Some(param) => {
                    let command = gc_doc::Command::SetParam { id: param, source: value.to_owned() };
                    session.execute(&command).is_ok() && session.edit_dimension(id, name).is_ok()
                }
            };
        }
    }
    session.edit_dimension(id, text).is_ok()
}

/// The highlight of what a constraint dialog would pick: an arrow out of a face, the axis of a cylinder or cone, an
/// edge, a point.
fn paint_pick(
    painter: &egui::Painter,
    screen: &dyn Fn(DVec3) -> Option<egui::Pos2>,
    feature: &gc_session::Feature,
    length: f64,
) {
    use gc_session::Feature;
    let colour = colors::SELECTED;
    let stroke = egui::Stroke::new(2.5, colour);
    let line = |a: DVec3, b: DVec3| {
        if let (Some(a), Some(b)) = (screen(a), screen(b)) {
            painter.line_segment([a, b], stroke);
        }
    };
    let v = DVec3::from_array;
    match *feature {
        Feature::Point(p) => {
            if let Some(at) = screen(v(p)) {
                painter.circle(at, 5.0, colour, egui::Stroke::new(1.5, egui::Color32::WHITE));
            }
        }
        Feature::Edge(a, b) => line(v(a), v(b)),
        Feature::Cylinder { centre, axis, length: l, .. } => {
            line(v(centre) - v(axis) * l / 2.0, v(centre) + v(axis) * l / 2.0);
        }
        Feature::Cone { apex, axis, .. } => line(v(apex), v(apex) + v(axis) * length * 3.0),
        // A circular edge or a spherical face: its centre, ringed (the picker that finds them comes later).
        Feature::Circle { centre, .. } | Feature::Sphere { centre, .. } => {
            if let Some(at) = screen(v(centre)) {
                painter.circle_stroke(at, 8.0, stroke);
                painter.circle(at, 3.0, colour, egui::Stroke::NONE);
            }
        }
        Feature::Face { point, normal, .. } => {
            if let (Some(a), Some(b)) = (screen(v(point)), screen(v(point) + v(normal) * length)) {
                painter.arrow(a, b - a, stroke);
            }
            if let Some(at) = screen(v(point)) {
                painter.circle_stroke(at, 6.0, stroke);
            }
        }
    }
}
