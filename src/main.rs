mod gui;
mod platform;
mod session;
mod terminal;

use iced::font;
use iced::{Color, Size};

use crate::gui::App;

// Embed DejaVu Sans font for better Unicode support (Box Drawing characters)
const DEJAVU_SANS: &[u8] = include_bytes!("../fonts/DejaVuSans.ttf");

fn main() -> iced::Result {
    iced::application("Rabbitty", App::update, App::view)
        .theme(|_state| iced::Theme::Dark)
        .style(|_, _| iced::application::Appearance {
            background_color: Color::from_rgb8(16, 16, 20),
            text_color: Color::WHITE,
        })
        .subscription(App::subscription)
        .font(DEJAVU_SANS)
        .default_font(iced::Font {
            family: font::Family::Name("DejaVu Sans"),
            ..iced::Font::DEFAULT
        })
        .window(iced::window::Settings {
            exit_on_close_request: false,
            size: Size::new(600.0, 350.0),

            #[cfg(target_os = "macos")]
            platform_specific: iced::window::settings::PlatformSpecific {
                title_hidden: true,
                titlebar_transparent: true,
                fullsize_content_view: true,
            },

            ..Default::default()
        })
        .run_with(|| {
            let app = App::new();

            #[cfg(target_os = "windows")]
            let init_task: iced::Task<gui::app::Message> = window::get_latest()
                .and_then(|id| {
                    window::run_with_handle(id, |handle| {
                        platform::apply_style(handle);
                    })
                })
                .discard();

            #[cfg(not(target_os = "windows"))]
            let init_task = iced::Task::none();

            (app, init_task)
        })
}
