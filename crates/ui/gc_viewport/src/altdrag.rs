//! Alt + drag in an assembly (Inventor 2026 help, To Place Mate or Flush Constraints in Assemblies,
//! GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E, "To drag a component into position and place a Mate
//! constraint"; shortcut table GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E): hold Alt, press on a face or
//! cylinder of the component, drag onto a face or cylinder of another one; the constraint is previewed and
//! releasing places it. Planar faces give a Mate, cylinders a Mate along their axes.

use gc_doc::{Geometry, Occurrence, Selection};
use gc_session::{CellItem, Feature, Session};

/// An Alt + drag in progress.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AltDrag {
    /// What was pressed: the first selection.
    pub(crate) first: Selection,
    /// Where the press was, in view pixels.
    pub(crate) from: [f64; 2],
    /// The other component's geometry under the cursor that goes with the first: the second selection, and
    /// where it was picked (view pixels).
    pub(crate) target: Option<(Selection, [f64; 2])>,
}

/// The geometry a face or cylinder picked in the view stands for (edges and points are not used here).
pub(crate) fn geometry_of(feature: &Feature) -> Option<Geometry> {
    match *feature {
        Feature::Face { point, normal, .. } => Some(Geometry::Plane { point, normal }),
        Feature::Cylinder { centre, axis, radius, .. } => {
            Some(Geometry::Cylinder { point: centre, direction: axis, radius })
        }
        Feature::Cone { apex, axis, half_angle } => {
            Some(Geometry::Cone { apex, direction: axis, half_angle })
        }
        Feature::Point(_) | Feature::Edge(..) => None,
    }
}

/// The selection a pick of the component stands for.
pub(crate) fn selection_of(item: CellItem, feature: &Feature) -> Option<Selection> {
    let any = crate::grip::component_of(item)?;
    Some(Selection { occurrence: Occurrence::Item(any), geometry: geometry_of(feature)? })
}

/// Whether `second` can be constrained to `first` (another component, geometry that goes together).
pub(crate) fn offers(session: &Session, first: &Selection, second: &Selection) -> bool {
    first.occurrence != second.occurrence && !session.possible_relations(first, second).is_empty()
}
