//! Port of `examples/styles/lv_example_style_multiple.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `BASE`
const BASE: Color = Color::hex(0x6366f1);
/// `WARN`
const WARN: Color = Color::hex(0xf59e0b);

/// `style_base`, `style_warning`.
struct Styles;

/// `lv_example_style_multiple()`: cascading styles; a second style repaints only colors.
pub fn lv_example_style_multiple(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_base, style_warning] = styles_once::<Styles, 2>(ui, || {
        let mut style_base = Style::new();
        style_base.set_radius(14);
        style_base.set_bg_opa((255 * 100 / 100) as u8);
        style_base.set_bg_color(BASE);
        style_base.set_border_width(2);
        style_base.set_border_color(Color::hex(0x4338ca));
        style_base.set_shadow_color(Color::hex(0x312e81));
        style_base.set_shadow_width(14);
        style_base.set_shadow_offset_y(5);
        style_base.set_shadow_opa(120);
        style_base.set_pad_all(16);
        style_base.set_text_color(Color::hex(0xffffff));

        let mut style_warning = Style::new();
        style_warning.set_bg_color(WARN);
        style_warning.set_border_color(Color::hex(0xb45309));
        style_warning.set_text_color(Color::hex(0x1f2937));
        [style_base, style_warning]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    let container_2 = ui.obj_create(Some(screen));
    ui.obj_set_size(container_2, 110, coord::SIZE_CONTENT);
    ui.obj_add_style(container_2, style_base, sel);
    let label_1 = Label::create(ui, container_2);
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "Base");

    let container_3 = ui.obj_create(Some(screen));
    ui.obj_set_size(container_3, 110, coord::SIZE_CONTENT);
    ui.obj_add_style(container_3, style_base, sel);
    ui.obj_add_style(container_3, style_warning, sel);
    let label_2 = Label::create(ui, container_3);
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "Warning");
}
