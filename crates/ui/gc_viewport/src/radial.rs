//! The marking menu of the graphics window: which commands sit in which sector, and what they do.
//! Reference: Inventor 2026 help, About Marking Menus (GUID-4B081B74-45EA-451E-84BE-9FFE0E2ACAE3): the
//! sketch environment offers Center Point Circle, Line, Two Point Rectangle and Finish Sketch, the
//! assembly environment Constrain, Place Component, Move Component and Rotate Component; a short context
//! menu (overflow) goes with it. The help does not say which sector holds what: north, east, south and
//! west are used for the four commands.

use gc_doc::{Occurrence, OriginElement, OriginPlane};
use gc_i18n::{Lang, tr};
use gc_session::{CellItem, Session};
use gc_tools::{Key as ToolKey, ToolEvent, ToolKind};
use gc_ui::marking::MarkingItem;

/// What the application has to do for the view because it needs more than the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewportRequest {
    /// Open the Constrain dialog.
    Constrain,
    /// Ask for a mesh file and place it as a component.
    PlaceComponent,
    /// Double click on a work feature: edit it.
    EditWorkFeature(gc_core::Id<gc_doc::WorkFeature>),
    /// Double click on a component: edit the unit that holds it in place (a subassembly as one).
    EditUnit(gc_doc::Occurrence),
}

/// What the right-click menu of a sketch offers for the spline under the cursor (Inventor 2026 help,
/// GUID-DB15C479-66F2-456A-9E54-40A60F83DC92): Split Spline at an inner fit point, Close Curve at the start
/// of an open spline, Insert Point on the curve.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SplineMenu {
    pub(crate) split: Option<(gc_core::Id<gc_sketch::Curve>, gc_core::Id<gc_sketch::SkPoint>)>,
    pub(crate) close: Option<gc_core::Id<gc_sketch::Curve>>,
    pub(crate) insert: Option<gc_core::Id<gc_sketch::Curve>>,
    /// The sketch point under the cursor that is not a waypoint yet ("Waypoint" role, plan 9.1).
    pub(crate) waypoint: Option<(gc_core::Id<gc_doc::Sketch>, gc_core::Id<gc_sketch::SkPoint>)>,
    /// The open curve under the cursor that is not in a path role yet; its chain becomes the path.
    pub(crate) path: Option<(gc_core::Id<gc_doc::Sketch>, gc_core::Id<gc_sketch::Curve>)>,
    /// The constraint glyph under the cursor (Delete Constraint).
    pub(crate) constraint: Option<gc_core::Id<gc_sketch::GeoConstraint>>,
}

/// A command of the marking menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Cmd {
    /// Switch the sketch tool.
    Tool(ToolKind),
    /// End the command in progress.
    Done,
    /// Delete the constraint whose glyph is under the cursor (docs/ux/inventor-sketch-parity.md: glyph interaction; Inventor
    /// 2026 help, To Work with Geometric Constraints, GUID-B06FA7B7-5CD5-4D58-B1C5-0C5ABAFFD84E).
    DeleteConstraint(gc_core::Id<gc_sketch::GeoConstraint>),
    /// Line: draw the closing segment back to where the chain started; the tool stays on (Inventor 2026 help, To Work
    /// with Sketch Profiles, GUID-4110FA02-0A8A-41F7-85B5-384819DAC528: right-click Close).
    CloseLine,
    /// Finish the sketch.
    FinishSketch,
    /// Constrain components.
    Constrain,
    /// Place a component.
    Place,
    /// Move a component (not yet there).
    Move,
    /// Rotate a component (not yet there).
    Rotate,
    /// Show or hide a body or robot, or any other item.
    ToggleVisible(CellItem),
    /// Plan a path.
    Plan(CellItem),
    /// Delete an item.
    Delete(CellItem),
    /// Start a sketch on an origin plane of a component.
    SketchHere(Occurrence, OriginPlane),
    /// Hide an origin element.
    HideOrigin(Occurrence, OriginElement),
    /// List what lies under the cursor (Select Other).
    SelectOther,
    /// Look At the selection.
    LookAt,
    /// Split a spline at one of its inner fit points (Inventor 2026 help, GUID-DB15C479-66F2-456A-9E54-40A60F83DC92:
    /// "right-click a point and choose Split Spline").
    SplitSpline(gc_core::Id<gc_sketch::Curve>, gc_core::Id<gc_sketch::SkPoint>),
    /// Insert Point: each click on the spline adds a fit point (Esc ends).
    InsertSplinePoint(gc_core::Id<gc_sketch::Curve>),
    /// Close Curve: an open spline of three or more fit points closes.
    CloseSpline(gc_core::Id<gc_sketch::Curve>),
    /// Isolate the component: everything else is hidden.
    Isolate(Occurrence),
    /// Undo Isolate.
    UndoIsolate,
    /// Make a sketch point a waypoint of the robot work.
    AssignWaypoint(gc_core::Id<gc_doc::Sketch>, gc_core::Id<gc_sketch::SkPoint>),
    /// Make the chain of curves around a curve a path of the robot work.
    AssignPath(gc_core::Id<gc_doc::Sketch>, gc_core::Id<gc_sketch::Curve>),
    /// Make a subassembly flexible, or rigid again (the flag is what it becomes).
    Flexible(gc_doc::SubassemblyId, bool),
    /// The isometric view.
    Home,
    /// Fit everything.
    FitAll,
}

/// The commands of one menu: the radial ones by sector (north first, clockwise) and the overflow list.
#[derive(Clone, Debug, Default)]
pub(crate) struct Set {
    pub(crate) radial: Vec<Option<Cmd>>,
    pub(crate) overflow: Vec<Cmd>,
}

impl Set {
    /// The menu of a sketch being edited.
    pub(crate) fn sketch(spatial: bool, spline: SplineMenu, line_tool: bool) -> Self {
        let mut radial = vec![None; 8];
        radial[0] = Some(Cmd::Tool(ToolKind::Line));
        if !spatial {
            radial[2] = Some(Cmd::Tool(ToolKind::Circle));
            radial[6] = Some(Cmd::Tool(ToolKind::Rectangle));
        }
        radial[4] = Some(Cmd::FinishSketch);
        let mut overflow = Vec::new();
        if let Some(constraint) = spline.constraint {
            overflow.push(Cmd::DeleteConstraint(constraint));
        }
        // While a command runs, the menu starts with Done (and Close for the Line tool).
        if line_tool {
            overflow.push(Cmd::CloseLine);
        }
        if let Some((curve, point)) = spline.split {
            overflow.push(Cmd::SplitSpline(curve, point));
        }
        if let Some(curve) = spline.close {
            overflow.push(Cmd::CloseSpline(curve));
        }
        if let Some(curve) = spline.insert {
            overflow.push(Cmd::InsertSplinePoint(curve));
        }
        if let Some((sketch, point)) = spline.waypoint {
            overflow.push(Cmd::AssignWaypoint(sketch, point));
        }
        if let Some((sketch, curve)) = spline.path {
            overflow.push(Cmd::AssignPath(sketch, curve));
        }
        overflow.push(Cmd::LookAt);
        overflow.push(Cmd::Done);
        Self { radial, overflow }
    }

    /// The menu of the assembly environment: the four component commands, and what the item under the
    /// cursor offers.
    pub(crate) fn assembly(
        item: Option<CellItem>,
        origin: Option<(Occurrence, OriginElement)>,
        unit: Option<(gc_doc::SubassemblyId, bool)>,
        isolated: bool,
    ) -> Self {
        let mut radial = vec![None; 8];
        radial[0] = Some(Cmd::Constrain);
        radial[2] = Some(Cmd::Move);
        radial[4] = Some(Cmd::Rotate);
        radial[6] = Some(Cmd::Place);
        let mut overflow = Vec::new();
        if let Some((occurrence, element)) = origin {
            if let OriginElement::Plane(plane) = element {
                overflow.push(Cmd::SketchHere(occurrence, plane));
            }
            overflow.push(Cmd::HideOrigin(occurrence, element));
        } else if let Some(item) = item {
            overflow.push(Cmd::ToggleVisible(item));
            if let CellItem::Path(_) = item {
                overflow.push(Cmd::Plan(item));
            }
            overflow.push(Cmd::Delete(item));
        }
        // Right-clicking a placed subassembly offers Flexible (GUID-30817F08-A5C2-44B7-B237-4791ACA3F854).
        if let Some((id, flexible)) = unit {
            overflow.push(Cmd::Flexible(id, !flexible));
        }
        // Isolate (GUID-36AE9264-912D-4D3C-BB24-7EC16FA8777E): the component under the cursor stays alone;
        // while something is isolated, Undo Isolate brings the rest back.
        if let Some(any) = item.and_then(crate::grip::component_of) {
            overflow.push(Cmd::Isolate(Occurrence::Item(any)));
        }
        if isolated {
            overflow.push(Cmd::UndoIsolate);
        }
        overflow.push(Cmd::SelectOther);
        overflow.push(Cmd::LookAt);
        overflow.push(Cmd::Home);
        overflow.push(Cmd::FitAll);
        Self { radial, overflow }
    }
}

impl Cmd {
    /// The name and whether the command can be used.
    pub(crate) fn item(self, session: &Session, lang: Lang) -> MarkingItem {
        let label = match self {
            Self::Tool(ToolKind::Line) => tr!(lang, ToolLine).to_owned(),
            Self::Tool(ToolKind::Circle) => tr!(lang, ToolCircle).to_owned(),
            Self::Tool(ToolKind::Rectangle) => tr!(lang, SplitRectangle).to_owned(),
            Self::Tool(_) => String::new(),
            Self::Done => tr!(lang, CmdDone).to_owned(),
            Self::CloseLine => tr!(lang, CmdCloseLine).to_owned(),
            Self::DeleteConstraint(_) => tr!(lang, CmdDeleteConstraint).to_owned(),
            Self::FinishSketch => tr!(lang, CmdFinishSketch).to_owned(),
            Self::Constrain => tr!(lang, ConstrainTitle).to_owned(),
            Self::Place => tr!(lang, MarkPlace).to_owned(),
            Self::Move => tr!(lang, MarkMove).to_owned(),
            Self::Rotate => tr!(lang, MarkRotate).to_owned(),
            Self::ToggleVisible(item) => {
                let hidden =
                    item.shown().and_then(|shown| session.shown_look(shown)).is_some_and(|look| look.hidden);
                if hidden { tr!(lang, CmdShow).to_owned() } else { tr!(lang, CmdHide).to_owned() }
            }
            Self::Plan(_) => tr!(lang, CmdPlan).to_owned(),
            Self::Delete(item) => {
                tr!(lang, CmdDeleteItem, session.item_name(item).unwrap_or_default())
            }
            Self::SketchHere(..) => tr!(lang, CmdSketchHere).to_owned(),
            Self::HideOrigin(..) => tr!(lang, CmdHide).to_owned(),
            Self::SplitSpline(..) => tr!(lang, CmdSplitSpline).to_owned(),
            Self::InsertSplinePoint(_) => tr!(lang, CmdInsertSplinePoint).to_owned(),
            Self::CloseSpline(_) => tr!(lang, CmdCloseSpline).to_owned(),
            Self::SelectOther => tr!(lang, SelOther).to_owned(),
            Self::LookAt => tr!(lang, NavLookAt).to_owned(),
            // The check mark shows the state it has now (the command switches it).
            Self::Flexible(_, becomes) => {
                format!("{}{}", if becomes { "" } else { "✓ " }, tr!(lang, CmdFlexible))
            }
            Self::Isolate(_) => tr!(lang, CmdIsolate).to_owned(),
            Self::UndoIsolate => tr!(lang, CmdUndoIsolate).to_owned(),
            Self::AssignWaypoint(..) => tr!(lang, CmdAssignWaypoint).to_owned(),
            Self::AssignPath(..) => tr!(lang, CmdAssignPath).to_owned(),
            Self::Home => tr!(lang, NavHome).to_owned(),
            Self::FitAll => tr!(lang, NavZoomAll).to_owned(),
        };
        MarkingItem { label, enabled: true }
    }
}

/// What carrying out a command needs from the view besides the session.
pub(crate) enum Effect {
    /// Nothing more.
    None,
    /// The application is asked to do something.
    Request(ViewportRequest),
    /// Show the Select Other list at the cursor.
    SelectOther,
    /// Turn the view to the selection.
    LookAt,
    /// Start inserting fit points into a spline.
    InsertSplinePoint(gc_core::Id<gc_sketch::Curve>),
    /// Turn the view to the isometric one.
    Home,
    /// Fit everything in the view.
    Fit,
    /// Switch Free Move or Free Rotate on.
    Free(crate::FreeTool),
}

/// Carries a command out.
pub(crate) fn run(cmd: Cmd, session: &mut Session) -> Effect {
    match cmd {
        Cmd::Tool(kind) => session.set_tool(kind),
        Cmd::Done => session.tool_event(ToolEvent::Key(ToolKey::Escape)),
        Cmd::DeleteConstraint(id) => {
            session.tool_event(ToolEvent::ClickItem {
                item: Some(gc_sketch::Item::Constraint(id)),
                pos: [0.0; 2],
                modifier: false,
            });
            session.tool_event(ToolEvent::Key(ToolKey::Delete));
        }
        Cmd::CloseLine => session.tool_event(ToolEvent::Command(gc_tools::ToolCommand::Close)),
        Cmd::FinishSketch => session.finish_sketch(),
        Cmd::Constrain => return Effect::Request(ViewportRequest::Constrain),
        Cmd::Place => return Effect::Request(ViewportRequest::PlaceComponent),
        Cmd::Move => return Effect::Free(crate::FreeTool::Move),
        Cmd::Rotate => return Effect::Free(crate::FreeTool::Rotate),
        Cmd::ToggleVisible(item) => {
            if let Some(shown) = item.shown()
                && let Some(look) = session.shown_look(shown)
            {
                drop(session.set_look(shown, gc_doc::Look { hidden: !look.hidden, ..look }));
            }
        }
        Cmd::Plan(CellItem::Path(path)) => drop(session.plan(path)),
        Cmd::Plan(_) => {}
        // A refusal (still in use) is explained in the status bar.
        Cmd::Delete(item) => drop(session.delete_item(item)),
        Cmd::SketchHere(occurrence, plane) => drop(session.create_sketch_on_component(occurrence, plane)),
        Cmd::HideOrigin(occurrence, element) => {
            drop(session.set_origin_visible(occurrence, &[element], false))
        }
        // A refusal is explained in the status bar.
        Cmd::SplitSpline(curve, point) => {
            drop(session.edit_active_sketch(gc_doc::SketchOp::SplitSpline { curve, point }))
        }
        Cmd::InsertSplinePoint(curve) => return Effect::InsertSplinePoint(curve),
        // A refusal is explained in the status bar.
        Cmd::CloseSpline(curve) => drop(session.edit_active_sketch(gc_doc::SketchOp::CloseSpline { curve })),
        // A refusal (a point that already is one, curves that do not join) is explained in the status bar.
        Cmd::AssignWaypoint(sketch, point) => drop(session.assign_roles(sketch, &[point], &[])),
        Cmd::AssignPath(sketch, curve) => {
            let chain = session.chain_at(sketch, curve);
            if !chain.is_empty() {
                drop(session.assign_roles(sketch, &[], &chain));
            }
        }
        // A refusal is explained in the status bar.
        Cmd::Isolate(occurrence) => drop(session.isolate(&[occurrence])),
        Cmd::UndoIsolate => drop(session.undo_isolate()),
        Cmd::SelectOther => return Effect::SelectOther,
        Cmd::LookAt => return Effect::LookAt,
        // A refusal is explained in the status bar.
        Cmd::Flexible(id, flexible) => drop(session.set_flexible(id, flexible)),
        Cmd::Home => return Effect::Home,
        Cmd::FitAll => return Effect::Fit,
    }
    Effect::None
}
