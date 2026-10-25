//! Port of `examples/styles/lv_example_style_shadow.c`.

use ferrovgl_core::{Align, Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_elevated;`
struct StyleElevated;

/// `lv_example_style_shadow()`: an elevated card with an offset shadow.
pub fn lv_example_style_shadow(ui: &mut Ui) {
    let [style_elevated] = styles_once::<StyleElevated, 1>(ui, || {
        let mut style_elevated = Style::new();
        style_elevated.set_radius(20);
        style_elevated.set_bg_opa((255 * 100 / 100) as u8);
        style_elevated.set_bg_color(Color::hex(0xffffff));
        style_elevated.set_border_width(0);
        style_elevated.set_shadow_color(Color::hex(0x312e81));
        style_elevated.set_shadow_width(30);
        style_elevated.set_shadow_offset_y(12);
        style_elevated.set_shadow_opa(80);
        [style_elevated]
    });

    let screen = screen_active(ui);

    let container = ui.obj_create(Some(screen));
    ui.obj_set_align(container, Align::Center);
    ui.obj_set_size(container, 210, 130);
    ui.obj_add_style(container, style_elevated, StyleSelector::DEFAULT);
    let label = Label::create(ui, container);
    ui.obj_set_align(label.obj(), Align::Center);
    label.set_text(ui, "Elevated");
}
