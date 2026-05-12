mod config;
#[macro_use]
mod i18n;
mod gui;
mod keychain;
mod platform;
mod session;
mod ssh;
mod terminal;

use iced::Size;
use iced::font;

use crate::config::AppConfig;
use crate::gui::App;

// Embed DejaVu Sans font for better Unicode support (Box Drawing characters)
const DEJAVU_SANS: &[u8] = include_bytes!("../fonts/DejaVuSans.ttf");
const APP_ICON_PNG: &[u8] = include_bytes!("../assets/logo.png");

fn main() -> iced::Result {
    let app_config = AppConfig::load();
    i18n::set_locale(app_config.ui.language.as_deref());
    let boot_config = app_config.clone();

    iced::application(
        move || {
            let app = App::new(boot_config.clone());

            let init_task = iced::Task::perform(
                async {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                },
                |_| gui::app::Message::ApplyWindowStyle,
            );

            (app, init_task)
        },
        App::update,
        App::view,
    )
    .title("Rabbitty")
    .theme(iced::Theme::Dark)
    .style(|state, _| state.window_style())
    .subscription(App::subscription)
    .font(DEJAVU_SANS)
    .default_font(iced::Font {
        family: font::Family::Name("DejaVu Sans"),
        ..iced::Font::DEFAULT
    })
    .window(iced::window::Settings {
        exit_on_close_request: false,
        size: Size::new(app_config.ui.window_width, app_config.ui.window_height),
        transparent: true,
        icon: iced::window::icon::from_file_data(APP_ICON_PNG, None).ok(),

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
