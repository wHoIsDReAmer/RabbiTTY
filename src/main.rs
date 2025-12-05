mod gui;

use crate::gui::App;

fn main() -> iced::Result {
    iced::application("foo", App::update, App::view)
        .subscription(App::subscription)
        .run()
}
