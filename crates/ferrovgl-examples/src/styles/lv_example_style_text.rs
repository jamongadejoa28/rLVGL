//! Port of `examples/styles/lv_example_style_text.c`.

use ferrovgl_core::misc::text::{TextAlign, TextDecor};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `ACCENT`
const ACCENT: Color = Color::hex(0x6366f1);

/// `style_card`, `style_heading`.
struct Styles;

/// `lv_example_style_text()`: letter and line spacing with an underline.
pub fn lv_example_style_text(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_card, style_heading] = styles_once::<Styles, 2>(ui, || {
        let mut style_card = Style::new();
        style_card.set_radius(16);
        style_card.set_bg_opa((255 * 100 / 100) as u8);
        style_card.set_bg_color(Color::hex(0xffffff));
        style_card.set_pad_all(22);

        let mut style_heading = Style::new();
        style_heading.set_text_color(ACCENT);
        style_heading.set_text_letter_space(4);
        style_heading.set_text_line_space(14);
        style_heading.set_text_decor(TextDecor::UNDERLINE);
        [style_card, style_heading]
    });

    let screen = screen_active(ui);

    let container = ui.obj_create(Some(screen));
    ui.obj_set_size(container, 240, coord::SIZE_CONTENT);
    ui.obj_set_align(container, Align::Center);
    ui.obj_add_style(container, style_card, sel);
    let label = Label::create(ui, container);
    ui.obj_set_align(label.obj(), Align::Center);
    ui.obj_set_style_text_align(label.obj(), TextAlign::Center, sel);
    label.set_text(ui, "Custom\nTypography");
    ui.obj_add_style(label.obj(), style_heading, sel);
}
