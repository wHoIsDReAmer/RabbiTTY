mod gui;
mod session;
mod terminal;

use iced::Size;

use crate::gui::App;

fn main() -> iced::Result {
    iced::application("Rabbitty", App::update, App::view)
        .subscription(App::subscription)
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
        .run()
}
