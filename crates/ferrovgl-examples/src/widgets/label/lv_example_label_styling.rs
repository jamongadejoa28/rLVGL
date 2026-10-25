//! Port of `examples/widgets/label/lv_example_label_styling.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_badge;`
struct StyleBadge;

/// `lv_example_label_styling()`: a named pill style and a locally styled call-out.
pub fn lv_example_label_styling(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_badge] = styles_once::<StyleBadge, 1>(ui, || {
        let mut style_badge = Style::new();
        style_badge.set_bg_opa((255 * 100 / 100) as u8);
        style_badge.set_bg_color(Color::hex(0x6366f1));
        style_badge.set_radius(100);
        style_badge.set_pad_hor(14);
        style_badge.set_pad_ver(6);
        style_badge.set_text_color(Color::hex(0xffffff));
        [style_badge]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Named-style pill badge */
    let label_1 = Label::create(ui, screen);
    label_1.set_text(ui, "NEW");
    ui.obj_add_style(label_1.obj(), style_badge, sel);

    /* Local-style call-out: border + outline halo + shadow + padding */
    let label_2 = Label::create(ui, screen);
    let obj = label_2.obj();
    label_2.set_text(ui, "Heads up");
    ui.obj_set_style_bg_opa(obj, (255 * 100 / 100) as u8, sel);
    ui.obj_set_style_bg_color(obj, Color::hex(0xfef3c7), sel);
    ui.obj_set_style_text_color(obj, Color::hex(0x92400e), sel);
    ui.obj_set_style_border_color(obj, Color::hex(0xf59e0b), sel);
    ui.obj_set_style_border_width(obj, 2, sel);
    ui.obj_set_style_radius(obj, 8, sel);
    ui.obj_set_style_pad_all(obj, 10, sel);
    ui.obj_set_style_outline_color(obj, Color::hex(0xf59e0b), sel);
    ui.obj_set_style_outline_width(obj, 3, sel);
    ui.obj_set_style_outline_pad(obj, 3, sel);
    ui.obj_set_style_outline_opa(obj, 80, sel);
    ui.obj_set_style_shadow_color(obj, Color::hex(0x78350f), sel);
    ui.obj_set_style_shadow_width(obj, 14, sel);
    ui.obj_set_style_shadow_offset_y(obj, 3, sel);
    ui.obj_set_style_shadow_opa(obj, 100, sel);
}
