//! Port of `examples/styles/lv_example_style_blur.c`.

use ferrovgl_core::draw::grad::GradDir;
use ferrovgl_core::misc::text::TextAlign;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_glass;`
struct StyleGlass;

/// `lv_example_style_blur()`: a frosted-glass card blurring its backdrop.
pub fn lv_example_style_blur(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_glass] = styles_once::<StyleGlass, 1>(ui, || {
        let mut style_glass = Style::new();
        style_glass.set_radius(40);
        style_glass.set_bg_opa((255 * 40 / 100) as u8);
        style_glass.set_bg_color(Color::hex(0xffffff));
        style_glass.set_border_width(1);
        style_glass.set_border_color(Color::hex(0xffffff));
        style_glass.set_border_opa((255 * 60 / 100) as u8);
        style_glass.set_blur_backdrop(true);
        style_glass.set_blur_radius(18);
        style_glass.set_text_color(Color::hex(0xffffff));
        [style_glass]
    });

    let screen = screen_active(ui);
    ui.obj_set_style_bg_opa(screen, (255 * 100 / 100) as u8, sel);
    ui.obj_set_style_bg_color(screen, Color::hex(0x6366f1), sel);
    ui.obj_set_style_bg_grad_color(screen, Color::hex(0xec4899), sel);
    ui.obj_set_style_bg_grad_dir(screen, GradDir::Ver, sel);
    ui.obj_set_style_pad_all(screen, 10, sel);

    let label_1 = Label::create(ui, screen);
    ui.obj_set_align(label_1.obj(), Align::Center);
    ui.obj_set_width(label_1.obj(), coord::pct(100));
    ui.obj_set_style_text_align(label_1.obj(), TextAlign::Center, sel);
    label_1.set_text(
        ui,
        "Hey there, I'm a little text in the background which is blurred. You might seem that some of parts are sharp, but other under the glass cannot be read. ",
    );

    let button = Button::create(ui, screen).obj();
    ui.obj_set_align(button, Align::Center);
    ui.obj_set_style_pad_all(button, 16, sel);
    ui.obj_set_y(button, 55);
    let label_2 = Label::create(ui, button);
    label_2.set_text(ui, "Click me");

    let container = ui.obj_create(Some(screen));
    ui.obj_set_size(container, 210, 110);
    ui.obj_set_align(container, Align::Center);
    ui.obj_add_style(container, style_glass, sel);
    let label_3 = Label::create(ui, container);
    ui.obj_set_align(label_3.obj(), Align::Center);
    label_3.set_text(ui, "Frosted glass");
}
