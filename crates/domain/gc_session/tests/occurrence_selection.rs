//! Selecting a component occurrence (lead: the frontend's Ground toggle needs it) and how it relates to the
//! cell item selection: bodies and robots are both; a link or tool selects its robot's panels; an
//! imported assembly or part has no cell item. Only components placed in the cell can be grounded.

#![allow(clippy::unwrap_used, reason = "test code")]

use gc_doc::{CellOp, Command, ComponentFlag, Occurrence, Placement, Solid, Tool};
use gc_session::{CellItem, Session};

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn occurrences_select_with_their_cell_items_and_the_ground_toggle_works_on_them() {
    let mut session = Session::new();
    let floor = session.add_body(Solid::Box { size: [2.0, 2.0, 0.1] }, Placement::default()).unwrap();
    let robot = session.add_robot("Robot1", &six_axis()).unwrap();
    let tool = Tool { solid: Solid::Box { size: [0.04; 3] }, placement: Placement::default() };
    session.execute(&Command::Cell(CellOp::SetTool { id: robot, tool: Some(tool) })).unwrap();

    // A cell item selects its occurrence.
    session.select_item(Some(CellItem::Robot(robot)));
    assert_eq!(session.selected_occurrence(), Some(Occurrence::Item(robot.erase())));
    // An occurrence selects its cell item: a link or tool its robot.
    assert!(session.select_occurrence(Some(Occurrence::Link { robot, index: 2 })));
    assert_eq!(session.selected_item(), Some(CellItem::Robot(robot)));
    assert_eq!(session.selected_occurrence(), Some(Occurrence::Link { robot, index: 2 }));
    assert!(session.select_occurrence(Some(Occurrence::Tool { robot })));
    assert_eq!(session.selected_item(), Some(CellItem::Robot(robot)));
    assert!(session.select_occurrence(Some(Occurrence::Item(floor.erase()))));
    assert_eq!(session.selected_item(), Some(CellItem::Body(floor)));
    // A missing occurrence is refused and the selection stays.
    assert!(!session.select_occurrence(Some(Occurrence::Link { robot, index: 999 })));
    assert_eq!(session.selected_item(), Some(CellItem::Body(floor)));

    // Ground toggle: the floor was the first component (grounded); toggling frees it, then grounds it,
    // one undo step each.
    assert!(session.can_ground(Occurrence::Item(floor.erase())));
    let depth = session.history().undo_depth();
    assert!(!session.toggle_ground_selected().unwrap());
    assert!(!session.doc().component_flag(Occurrence::Item(floor.erase()), ComponentFlag::Grounded));
    assert!(session.toggle_ground_selected().unwrap());
    assert_eq!(session.history().undo_depth(), depth + 2);
    // Links, tools and parts are not grounded on their own.
    session.select_occurrence(Some(Occurrence::Link { robot, index: 1 }));
    assert!(!session.can_ground(Occurrence::Link { robot, index: 1 }));
    assert!(session.toggle_ground_selected().is_err());
    assert!(
        session.set_component(Occurrence::Link { robot, index: 1 }, ComponentFlag::Grounded, true).is_err()
    );
    // A waypoint selection has no occurrence; deleting the selected body clears it.
    session.select_occurrence(Some(Occurrence::Item(floor.erase())));
    session.delete_item(CellItem::Body(floor)).unwrap();
    assert_eq!(session.selected_occurrence(), None);
    assert!(session.toggle_ground_selected().is_err());
    session.select_occurrence(None);
    assert_eq!(session.selected_item(), None);
}

#[test]
fn the_representations_folder_lists_views_and_positions_with_the_active_ones() {
    let mut session = Session::new();
    let body = session.add_body(Solid::Box { size: [0.2; 3] }, Placement::default()).unwrap();
    let folder = session.representation_folder();
    assert!(folder.views.is_empty());
    assert_eq!(folder.positions.len(), 1);
    assert_eq!((folder.positions[0].name.as_str(), folder.positions[0].active), ("Primary", true));

    session.add_view_representation("All", Some("camera".into())).unwrap();
    // A new view is the active one and takes display edits (Inventor 2026 help, To Create, Edit, and
    // Restore View Representations, GUID-397F9263-12C3-47A1-B6A6-16FC05EDEF47), so the body is hidden
    // after "Hidden" was made.
    session.add_view_representation("Hidden", None).unwrap();
    session.set_component(Occurrence::Item(body.erase()), ComponentFlag::Visible, false).unwrap();
    session.add_positional_representation("Open").unwrap();
    assert!(session.add_positional_representation("Primary").is_err());
    session.activate_view_representation(0).unwrap();
    session.activate_positional_representation(Some(0)).unwrap();
    let folder = session.representation_folder();
    let views: Vec<(&str, bool)> = folder.views.iter().map(|n| (n.name.as_str(), n.active)).collect();
    assert_eq!(views, [("All", true), ("Hidden", false)]);
    let positions: Vec<(&str, bool)> = folder.positions.iter().map(|n| (n.name.as_str(), n.active)).collect();
    assert_eq!(positions, [("Primary", false), ("Open", true)]);
    assert!(
        session.doc().component_flag(Occurrence::Item(body.erase()), ComponentFlag::Visible),
        "All shows it"
    );

    // Deleting the active view leaves none checked; deleting the active position goes back to Primary.
    session.delete_representation(Some(0), Some(0)).unwrap();
    let folder = session.representation_folder();
    assert_eq!(folder.views.len(), 1);
    assert!(!folder.views[0].active);
    assert!(folder.positions[0].active && folder.positions.len() == 1);
    session.undo();
    assert!(session.representation_folder().views[0].active);
}
