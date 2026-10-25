//! Port of `examples/styles/lv_example_style_margin.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `ACCENT`
const ACCENT: Color = Color::hex(0x6366f1);

/// `style_chip`, `style_chip_accent`.
struct Styles;

/// `lv_example_style_margin()`: margins push flex neighbours apart.
pub fn lv_example_style_margin(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_chip, style_chip_accent] = styles_once::<Styles, 2>(ui, || {
        let mut style_chip = Style::new();
        style_chip.set_radius(10);
        style_chip.set_bg_opa((255 * 100 / 100) as u8);
        style_chip.set_bg_color(Color::hex(0xe2e8f0));
        style_chip.set_pad_all(14);

        let mut style_chip_accent = Style::new();
        style_chip_accent.set_radius(10);
        style_chip_accent.set_bg_opa((255 * 100 / 100) as u8);
        style_chip_accent.set_bg_color(ACCENT);
        style_chip_accent.set_pad_all(14);
        style_chip_accent.set_text_color(Color::hex(0xffffff));
        [style_chip, style_chip_accent]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    let label_1 = Label::create(ui, screen);
    label_1.set_text(ui, "A");
    ui.obj_add_style(label_1.obj(), style_chip, sel);

    let label_2 = Label::create(ui, screen);
    label_2.set_text(ui, "B");
    ui.obj_set_style_margin_left(label_2.obj(), 48, sel);
    ui.obj_set_style_margin_right(label_2.obj(), 24, sel);
    ui.obj_add_style(label_2.obj(), style_chip_accent, sel);

    let label_3 = Label::create(ui, screen);
    label_3.set_text(ui, "C");
    ui.obj_add_style(label_3.obj(), style_chip, sel);
}
