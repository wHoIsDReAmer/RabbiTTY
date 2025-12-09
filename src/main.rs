mod gui;

use iced::Size;

use crate::gui::App;

fn main() -> iced::Result {
    iced::application("Rabbitty", App::update, App::view)
        .subscription(App::subscription)
        .window(iced::window::Settings {
            exit_on_close_request: false,
            size: Size::new(600.0, 350.0),
            ..Default::default()
        })
        .run()
}
