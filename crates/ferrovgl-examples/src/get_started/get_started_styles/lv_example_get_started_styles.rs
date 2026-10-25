//! Port of `examples/get_started/get_started_styles/lv_example_get_started_styles.c`.

use ferrovgl_core::draw::grad::GradDir;
use ferrovgl_core::{Align, Color, State, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::{screen_active, styles_once};

/// `style_button`, `style_button_pressed`, `style_button_red`.
struct Styles;

/// `lv_example_get_started_styles()`: styles from scratch for buttons.
pub fn lv_example_get_started_styles(ui: &mut Ui) {
    let [style_button, style_button_pressed, style_button_red] = styles_once::<Styles, 3>(ui, || {
        let mut style_button = Style::new();
        style_button.set_radius(10);
        style_button.set_bg_opa((255 * 100 / 100) as u8);
        style_button.set_bg_color(Color::hex(0xeeeeee));
        style_button.set_bg_grad_color(Color::hex(0x9e9e9e));
        style_button.set_bg_grad_dir(GradDir::Ver);
        style_button.set_border_color(Color::hex(0x000000));
        style_button.set_border_opa((255 * 20 / 100) as u8);
        style_button.set_border_width(2);
        style_button.set_text_color(Color::hex(0x000000));

        let mut style_button_pressed = Style::new();
        style_button_pressed.set_bg_color(Color::hex(0xbdbdbd));
        style_button_pressed.set_bg_grad_color(Color::hex(0x757575));

        let mut style_button_red = Style::new();
        style_button_red.set_bg_color(Color::hex(0xf44336));
        style_button_red.set_bg_grad_color(Color::hex(0xffcdd2));
        [style_button, style_button_pressed, style_button_red]
    });

    let screen = screen_active(ui);

    /* Base style + a pressed-state override */
    let button_1 = Button::create(ui, screen).obj();
    ui.obj_set_align(button_1, Align::Center);
    ui.obj_set_y(button_1, -35);
    ui.obj_set_size(button_1, 120, 50);
    ui.obj_add_style(button_1, style_button, StyleSelector::DEFAULT);
    ui.obj_add_style(button_1, style_button_pressed, State::PRESSED.into());
    let label_1 = Label::create(ui, button_1);
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "Button");

    /* Base + accent variant + a local circular radius */
    let button_2 = Button::create(ui, screen).obj();
    ui.obj_set_align(button_2, Align::Center);
    ui.obj_set_y(button_2, 35);
    ui.obj_set_size(button_2, 120, 50);
    ui.obj_set_style_radius(button_2, 25, StyleSelector::DEFAULT);
    ui.obj_add_style(button_2, style_button, StyleSelector::DEFAULT);
    ui.obj_add_style(button_2, style_button_red, StyleSelector::DEFAULT);
    let label_2 = Label::create(ui, button_2);
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "Button 2");
}
