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

/// A command of the marking menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Cmd {
    /// Switch the sketch tool.
    Tool(ToolKind),
    /// End the command in progress.
    Done,
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
    pub(crate) fn sketch(spatial: bool) -> Self {
        let mut radial = vec![None; 8];
        radial[0] = Some(Cmd::Tool(ToolKind::Line));
        if !spatial {
            radial[2] = Some(Cmd::Tool(ToolKind::Circle));
            radial[6] = Some(Cmd::Tool(ToolKind::Rectangle));
        }
        radial[4] = Some(Cmd::FinishSketch);
        Self { radial, overflow: vec![Cmd::Done] }
    }

    /// The menu of the assembly environment: the four component commands, and what the item under the
    /// cursor offers.
    pub(crate) fn assembly(item: Option<CellItem>, origin: Option<(Occurrence, OriginElement)>) -> Self {
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
        Cmd::Home => return Effect::Home,
        Cmd::FitAll => return Effect::Fit,
    }
    Effect::None
}
