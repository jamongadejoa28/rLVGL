//! Port of `examples/get_started/get_started_hello_world/lv_example_get_started_hello_world.c`.

use ferrovgl_core::{Align, Color, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::screen_active;

/// `lv_example_get_started_hello_world()`: paint the screen background and
/// center a label on it.
pub fn lv_example_get_started_hello_world(ui: &mut Ui) {
    let screen = screen_active(ui);
    ui.obj_set_style_bg_color(screen, Color::hex(0x003a57), StyleSelector::DEFAULT);
    ui.obj_set_style_text_color(screen, Color::hex(0xffffff), StyleSelector::DEFAULT);

    let label = Label::create(ui, screen);
    ui.obj_set_align(label.obj(), Align::Center);
    label.set_text(ui, "Hello world");
}
