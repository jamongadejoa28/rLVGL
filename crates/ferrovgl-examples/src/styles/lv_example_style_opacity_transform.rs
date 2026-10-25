//! Port of `examples/styles/lv_example_style_opacity_transform.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::{screen_active, styles_once};

/// `ACCENT`
const ACCENT: Color = Color::hex(0x6366f1);

/// `static lv_style_t style_btn;`
struct StyleBtn;

/// `lv_example_style_opacity_transform()`: opacity and transform style properties.
pub fn lv_example_style_opacity_transform(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_btn] = styles_once::<StyleBtn, 1>(ui, || {
        let mut style_btn = Style::new();
        style_btn.set_radius(10);
        style_btn.set_bg_opa((255 * 100 / 100) as u8);
        style_btn.set_bg_color(ACCENT);
        style_btn.set_pad_all(12);
        style_btn.set_text_color(Color::hex(0xffffff));
        [style_btn]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 12, sel);

    let button_1 = Button::create(ui, screen).obj();
    ui.obj_set_width(button_1, 130);
    ui.obj_add_style(button_1, style_btn, sel);
    let label_1 = Label::create(ui, button_1);
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "Normal");

    let button_2 = Button::create(ui, screen).obj();
    ui.obj_set_width(button_2, 130);
    ui.obj_set_style_opa(button_2, 128, sel);
    ui.obj_add_style(button_2, style_btn, sel);
    let label_2 = Label::create(ui, button_2);
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "Opa 50%");

    let button_3 = Button::create(ui, screen).obj();
    ui.obj_set_width(button_3, 130);
    ui.obj_set_style_opa(button_3, 128, sel);
    ui.obj_set_style_transform_rotation(button_3, 110, sel);
    ui.obj_set_style_transform_pivot_x(button_3, coord::pct(50), sel);
    ui.obj_set_style_transform_pivot_y(button_3, coord::pct(50), sel);
    ui.obj_set_style_transform_scale_x(button_3, 420, sel);
    ui.obj_add_style(button_3, style_btn, sel);
    let label_3 = Label::create(ui, button_3);
    ui.obj_set_align(label_3.obj(), Align::Center);
    label_3.set_text(ui, "Transformed");
}
