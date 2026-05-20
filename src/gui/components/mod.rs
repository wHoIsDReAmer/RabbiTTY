pub mod button;
pub mod category_transition;
pub mod container;
pub mod context_menu;
pub mod hover_fade;
pub mod ime_wrapper;
pub mod tab_bar;

use crate::gui::theme::Palette;

pub fn button_primary(
    text: &str,
    palette: Palette,
) -> iced::widget::button::Button<'_, crate::gui::app::Message> {
    button::primary(text, palette)
}

pub fn button_secondary(
    text: &str,
    palette: Palette,
) -> iced::widget::button::Button<'_, crate::gui::app::Message> {
    button::secondary(text, palette)
}

pub use category_transition::CategoryTransition;
pub use container::panel;
pub use hover_fade::{HoverStyle, hover_fade};
pub use tab_bar::tab_bar;
