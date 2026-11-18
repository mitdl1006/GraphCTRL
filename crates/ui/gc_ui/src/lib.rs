//! The user-interface kit: fonts, theme, vector icons and the ribbon widgets.
//!
//! Third-party widget crates are used only from here, so replacing one touches a single crate.

#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

pub mod fonts;
mod icons;
pub mod marking;
pub mod panel;
mod ribbon;
mod theme;

pub use icons::Icon;
pub use ribbon::{
    SplitItem, StackRow, ribbon_button, ribbon_drop_button, ribbon_group, ribbon_small_button,
    ribbon_split_button, ribbon_stack, ribbon_stack_responses, ribbon_tab,
};
pub use theme::{colors, install as install_theme, panel_fill, panel_frame};
