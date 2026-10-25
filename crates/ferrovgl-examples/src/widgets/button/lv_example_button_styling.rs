//! Port of `examples/widgets/button/lv_example_button_styling.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Align, Color, State, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::{screen_active, styles_once};

/// `style_button`, `style_button_pressed`, `style_button_checked`.
struct Styles;

/// `lv_example_button_styling()`: resting, pressed and checked button styles.
pub fn lv_example_button_styling(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_button, style_button_pressed, style_button_checked] = styles_once::<Styles, 3>(ui, || {
        let mut style_button = Style::new();
        style_button.set_bg_opa((255 * 100 / 100) as u8);
        style_button.set_bg_color(Color::hex(0x3b82f6));
        style_button.set_radius(12);
        style_button.set_shadow_color(Color::hex(0x1e3a8a));
        style_button.set_shadow_width(14);
        style_button.set_shadow_offset_y(4);
        style_button.set_shadow_opa(120);
        style_button.set_text_color(Color::hex(0xffffff));
        style_button.set_pad_all(10);

        let mut style_button_pressed = Style::new();
        style_button_pressed.set_bg_color(Color::hex(0x1d4ed8));
        style_button_pressed.set_shadow_offset_y(1);
        style_button_pressed.set_shadow_width(6);
        style_button_pressed.set_outline_color(Color::hex(0x60a5fa));
        style_button_pressed.set_outline_width(3);
        style_button_pressed.set_outline_pad(2);
        style_button_pressed.set_outline_opa((255 * 100 / 100) as u8);

        let mut style_button_checked = Style::new();
        style_button_checked.set_bg_color(Color::hex(0x16a34a));
        style_button_checked.set_shadow_color(Color::hex(0x14532d));
        style_button_checked.set_shadow_width(20);
        style_button_checked.set_shadow_opa(160);
        [style_button, style_button_pressed, style_button_checked]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 20, sel);

    /* Default button (resting style only) */
    let button_1 = Button::create(ui, screen).obj();
    ui.obj_set_size(button_1, 180, 48);
    ui.obj_add_style(button_1, style_button, sel);
    ui.obj_add_style(button_1, style_button_pressed, State::PRESSED.into());
    let label_1 = Label::create(ui, button_1);
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "Press me");

    /* Checkable button: swaps to the checked style on toggle */
    let button_2 = Button::create(ui, screen).obj();
    ui.obj_set_size(button_2, 180, 48);
    ui.obj_set_checkable(button_2, true);
    ui.obj_add_style(button_2, style_button, sel);
    ui.obj_add_style(button_2, style_button_pressed, State::PRESSED.into());
    ui.obj_add_style(button_2, style_button_checked, State::CHECKED.into());
    let label_2 = Label::create(ui, button_2);
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "Toggle me");

    /* Local style_* overrides: pure-CSS-style attribute styling, no named block needed */
    let button_3 = Button::create(ui, screen).obj();
    ui.obj_set_size(button_3, 180, 48);
    ui.obj_set_style_bg_color(button_3, Color::hex(0xf59e0b), sel);
    ui.obj_set_style_bg_opa(button_3, (255 * 100 / 100) as u8, sel);
    ui.obj_set_style_radius(button_3, 24, sel);
    ui.obj_set_style_border_color(button_3, Color::hex(0x78350f), sel);
    ui.obj_set_style_border_width(button_3, 2, sel);
    ui.obj_set_style_text_color(button_3, Color::hex(0x451a03), sel);
    ui.obj_set_style_pad_all(button_3, 10, sel);
    let label_3 = Label::create(ui, button_3);
    ui.obj_set_align(label_3.obj(), Align::Center);
    label_3.set_text(ui, "Local props");
}
