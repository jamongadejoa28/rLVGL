//! Port of `examples/widgets/label/lv_example_label_long_mode.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Label, LabelLongMode};

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_label_bg;`
struct StyleLabelBg;

/// `lv_example_label_long_mode()`: wrap, dots, scroll, circular scroll and clip.
pub fn lv_example_label_long_mode(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_label_bg] = styles_once::<StyleLabelBg, 1>(ui, || {
        let mut style_label_bg = Style::new();
        style_label_bg.set_bg_opa((255 * 100 / 100) as u8);
        style_label_bg.set_radius(6);
        [style_label_bg]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 12, sel);

    /* Wrap mode example */
    let label_1 = Label::create(ui, screen);
    ui.obj_set_width(label_1.obj(), 170);
    label_1.set_long_mode(ui, LabelLongMode::Wrap);
    ui.obj_set_style_bg_color(label_1.obj(), Color::hex(0xd6eaf8), sel);
    label_1.set_text(
        ui,
        "Wrap keeps all text visible by splitting long lines into multiple rows.",
    );
    ui.obj_add_style(label_1.obj(), style_label_bg, sel);

    /* Dots mode example */
    let label_2 = Label::create(ui, screen);
    ui.obj_set_size(label_2.obj(), 170, 32);
    label_2.set_long_mode(ui, LabelLongMode::Dots);
    ui.obj_set_style_bg_color(label_2.obj(), Color::hex(0xd5f5e3), sel);
    label_2.set_text(
        ui,
        "Dots shortens overflowing text and replaces the tail with '...'",
    );
    ui.obj_add_style(label_2.obj(), style_label_bg, sel);

    /* Scroll mode example */
    let label_3 = Label::create(ui, screen);
    ui.obj_set_width(label_3.obj(), 170);
    label_3.set_long_mode(ui, LabelLongMode::Scroll);
    ui.obj_set_style_bg_color(label_3.obj(), Color::hex(0xfdebd0), sel);
    label_3.set_text(
        ui,
        "Scroll moves overflowing text back and forth inside the label area.",
    );
    ui.obj_add_style(label_3.obj(), style_label_bg, sel);

    /* Circular scroll mode example */
    let label_4 = Label::create(ui, screen);
    ui.obj_set_width(label_4.obj(), 170);
    label_4.set_long_mode(ui, LabelLongMode::ScrollCircular);
    ui.obj_set_style_bg_color(label_4.obj(), Color::hex(0xfadbd8), sel);
    label_4.set_text(ui, "Scroll circular continuously loops overflowing text forever.");
    ui.obj_add_style(label_4.obj(), style_label_bg, sel);

    /* Clip mode example */
    let label_5 = Label::create(ui, screen);
    ui.obj_set_width(label_5.obj(), 170);
    label_5.set_long_mode(ui, LabelLongMode::Clip);
    ui.obj_set_style_bg_color(label_5.obj(), Color::hex(0xebdef0), sel);
    label_5.set_text(
        ui,
        "Clip simply cuts any part that falls outside the label width.",
    );
    ui.obj_add_style(label_5.obj(), style_label_bg, sel);
}
