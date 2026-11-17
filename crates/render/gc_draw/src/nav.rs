//! Which mouse and key combination does what in the viewport.

/// The view navigation styles on offer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavPreset {
    /// Autodesk Inventor: middle drag pans, Shift + middle drag or F4 + left drag orbits, F2 / F3 + left
    /// drag pan / zoom.
    #[default]
    Inventor,
    /// SolidWorks: middle drag orbits, Ctrl + middle drag pans, Shift + middle drag zooms.
    SolidWorks,
}

/// What a drag in the viewport does to the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavAction {
    /// Slide the view.
    Pan,
    /// Turn the view about the pivot.
    Orbit,
    /// Zoom by vertical motion.
    Zoom,
}

/// The buttons and keys held during a drag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools, reason = "one flag per physical button or key")]
pub struct NavInput {
    /// Left mouse button.
    pub left: bool,
    /// Middle mouse button (wheel click).
    pub middle: bool,
    /// Shift key.
    pub shift: bool,
    /// Ctrl key.
    pub ctrl: bool,
    /// F2 key.
    pub f2: bool,
    /// F3 key.
    pub f3: bool,
    /// F4 key.
    pub f4: bool,
}

/// The navigation a drag performs, or `None` when the drag belongs to the active tool.
pub fn nav_action(preset: NavPreset, input: NavInput) -> Option<NavAction> {
    match preset {
        NavPreset::Inventor => {
            if input.middle {
                Some(if input.shift { NavAction::Orbit } else { NavAction::Pan })
            } else if input.left && input.f4 {
                Some(NavAction::Orbit)
            } else if input.left && input.f2 {
                Some(NavAction::Pan)
            } else if input.left && input.f3 {
                Some(NavAction::Zoom)
            } else {
                None
            }
        }
        NavPreset::SolidWorks => {
            if !input.middle {
                None
            } else if input.ctrl {
                Some(NavAction::Pan)
            } else if input.shift {
                Some(NavAction::Zoom)
            } else {
                Some(NavAction::Orbit)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventor_bindings() {
        let nav = |input| nav_action(NavPreset::Inventor, input);
        assert_eq!(nav(NavInput { middle: true, ..NavInput::default() }), Some(NavAction::Pan));
        assert_eq!(
            nav(NavInput { middle: true, shift: true, ..NavInput::default() }),
            Some(NavAction::Orbit)
        );
        assert_eq!(nav(NavInput { left: true, f4: true, ..NavInput::default() }), Some(NavAction::Orbit));
        assert_eq!(nav(NavInput { left: true, f2: true, ..NavInput::default() }), Some(NavAction::Pan));
        assert_eq!(nav(NavInput { left: true, f3: true, ..NavInput::default() }), Some(NavAction::Zoom));
        assert_eq!(
            nav(NavInput { left: true, ..NavInput::default() }),
            None,
            "a plain left drag is the tool's"
        );
    }

    #[test]
    fn solidworks_bindings() {
        let nav = |input| nav_action(NavPreset::SolidWorks, input);
        assert_eq!(nav(NavInput { middle: true, ..NavInput::default() }), Some(NavAction::Orbit));
        assert_eq!(nav(NavInput { middle: true, ctrl: true, ..NavInput::default() }), Some(NavAction::Pan));
        assert_eq!(nav(NavInput { middle: true, shift: true, ..NavInput::default() }), Some(NavAction::Zoom));
        assert_eq!(nav(NavInput { left: true, f4: true, ..NavInput::default() }), None);
    }
}
