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

/// The geometry a picked face, cylinder, cone, edge or point stands for, as the world sees it.
fn world_geometry_of(feature: &Feature) -> Option<Geometry> {
    match *feature {
        Feature::Face { point, normal, .. } => Some(Geometry::Plane { point, normal }),
        Feature::Cylinder { centre, axis, radius, .. } => {
            Some(Geometry::Cylinder { point: centre, direction: axis, radius })
        }
        Feature::Cone { apex, axis, half_angle } => {
            Some(Geometry::Cone { apex, direction: axis, half_angle })
        }
        Feature::Edge(a, b) => {
            let direction =
                (gc_math::DVec3::from_array(b) - gc_math::DVec3::from_array(a)).try_normalize()?;
            Some(Geometry::Axis { point: a, direction: direction.to_array() })
        }
        Feature::Point(p) => Some(Geometry::Point(p)),
        Feature::Circle { centre, normal, .. } => Some(Geometry::Circle { centre, normal }),
        Feature::Sphere { centre, radius } => Some(Geometry::Sphere { centre, radius }),
    }
}

/// The selection a pick of the component stands for: the geometry is turned from the world into the component's
/// own frame, as a Selection holds it (Inventor 2026 help, Place Constraints: faces, edges, points and axes are
/// picked in the graphics window).
pub(crate) fn selection_of(session: &Session, item: CellItem, feature: &Feature) -> Option<Selection> {
    use gc_math::DVec3;
    let any = crate::grip::component_of(item)?;
    let occurrence = Occurrence::Item(any);
    let pose = session.component_origin(occurrence)?.pose.inverse();
    let p = |v: [f64; 3]| (pose.translation + pose.rotation * DVec3::from_array(v)).to_array();
    let d = |v: [f64; 3]| (pose.rotation * DVec3::from_array(v)).to_array();
    let geometry = match world_geometry_of(feature)? {
        Geometry::Plane { point, normal } => Geometry::Plane { point: p(point), normal: d(normal) },
        Geometry::Axis { point, direction } => Geometry::Axis { point: p(point), direction: d(direction) },
        Geometry::Point(point) => Geometry::Point(p(point)),
        Geometry::Cylinder { point, direction, radius } => {
            Geometry::Cylinder { point: p(point), direction: d(direction), radius }
        }
        Geometry::Cone { apex, direction, half_angle } => {
            Geometry::Cone { apex: p(apex), direction: d(direction), half_angle }
        }
        Geometry::Circle { centre, normal } => Geometry::Circle { centre: p(centre), normal: d(normal) },
        Geometry::Sphere { centre, radius } => Geometry::Sphere { centre: p(centre), radius },
        other => other,
    };
    Some(Selection { occurrence, geometry })
}

/// Whether `second` can be constrained to `first` (another component, geometry that goes together).
pub(crate) fn offers(session: &Session, first: &Selection, second: &Selection) -> bool {
    first.occurrence != second.occurrence && !session.possible_relations(first, second).is_empty()
}
