//! Picking for a sketch dialog that stays open (Inventor's Mirror and Pattern dialogs: the active
//! selector takes what is clicked in the graphics window; Inventor 2026 help, To Create and Edit Sketch
//! Patterns, GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE).
//!
//! While a pick target is set the Select tool's clicks and boxes go to it: geometry is added or removed
//! by each click (no Ctrl needed, and a click on nothing keeps the picks), a line or point selector takes
//! one item and replaces what it had. The dialog reads the pick back from the selection.

use gc_sketch::Item;
use gc_tools::{Prompt, ToolKind};

use crate::Session;

/// What the open sketch dialog's active selector takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SketchPick {
    /// Curves, toggled one by one (the Geometry selector).
    Curves,
    /// One line (a Mirror Line or a pattern direction).
    Line,
    /// One point, or the centre of a clicked circle or arc (a circular pattern's axis).
    Point,
}

impl Session {
    /// The active selector of an open sketch dialog, if any.
    pub fn sketch_pick(&self) -> Option<SketchPick> {
        self.pick_target
    }

    /// Makes `pick` the active selector (the Select tool runs, and `current` — what the selector holds
    /// now — shows as the selection), or ends picking with `None` (the selection is cleared).
    pub fn set_sketch_pick(&mut self, pick: Option<SketchPick>, current: Vec<Item>) {
        if pick.is_some() && self.tool.kind() != ToolKind::Select {
            self.set_tool(ToolKind::Select);
        }
        self.pick_target = pick;
        self.picked = current.clone();
        self.selection = current.into_iter().collect();
    }

    /// The status-bar prompt while a selector is active.
    pub(crate) fn pick_prompt(&self) -> Option<Prompt> {
        Some(match self.pick_target? {
            SketchPick::Curves => Prompt::PickGeometry,
            SketchPick::Line => Prompt::PickLine,
            SketchPick::Point => Prompt::PickPoint,
        })
    }

    /// A Select-tool pick while a selector is active. Returns false when no selector is active (the
    /// ordinary selection rules apply).
    /// Ctrl makes no difference: geometry always toggles, a line or point always replaces.
    pub(crate) fn pick_select(&mut self, item: Option<Item>) -> bool {
        let Some(target) = self.pick_target else { return false };
        let Some(item) = item else { return true };
        let sketch = self.active_sketch().map(|(_, s)| s);
        let taken = match (target, item) {
            (SketchPick::Curves, Item::Curve(_)) => {
                self.picked.retain(|p| *p != item);
                if !self.selection.remove(&item) {
                    self.selection.insert(item);
                    self.picked.push(item);
                }
                return true;
            }
            (SketchPick::Line, Item::Curve(c)) if sketch.is_some_and(|s| s.line(c).is_some()) => item,
            (SketchPick::Point, Item::Point(_)) => item,
            (SketchPick::Point, Item::Curve(c)) => match sketch.and_then(|s| s.round(c)) {
                Some((centre, _)) => Item::Point(centre),
                None => return true,
            },
            _ => return true,
        };
        self.selection = [taken].into_iter().collect();
        self.picked = vec![taken];
        true
    }
}
