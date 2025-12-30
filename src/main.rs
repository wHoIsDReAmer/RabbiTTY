mod config;
mod gui;
mod platform;
mod session;
mod terminal;

use iced::font;
#[cfg(target_os = "windows")]
use iced::window::raw_window_handle::HasWindowHandle;
use iced::{Color, Size};

use crate::config::AppConfig;
use crate::gui::App;

// Embed DejaVu Sans font for better Unicode support (Box Drawing characters)
const DEJAVU_SANS: &[u8] = include_bytes!("../fonts/DejaVuSans.ttf");

fn main() -> iced::Result {
    let app_config = AppConfig::load();
    let boot_config = app_config.clone();

    iced::application(
        move || {
            let app = App::new(boot_config.clone());

            #[cfg(target_os = "windows")]
            let init_task: iced::Task<gui::app::Message> = iced::window::latest()
                .and_then(|id| {
                    iced::window::run(id, |window| {
                        if let Ok(handle) = window.window_handle() {
                            platform::apply_style(handle);
                        }
                    })
                })
                .discard();

            #[cfg(not(target_os = "windows"))]
            let init_task = iced::Task::none();

            (app, init_task)
        },
        App::update,
        App::view,
    )
    .title("Rabbitty")
    .theme(iced::Theme::Dark)
    .style(|_, _| iced::theme::Style {
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
        size: Size::new(app_config.ui.window_width, app_config.ui.window_height),

        #[cfg(target_os = "macos")]
        platform_specific: iced::window::settings::PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            fullsize_content_view: true,
        },

        ..Default::default()
    })
    .run()
}
