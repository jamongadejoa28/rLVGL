//! Port of `examples/styles/lv_example_style_size_position.c`.

use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `ACCENT`
const ACCENT: Color = Color::hex(0x6366f1);

/// `static lv_style_t style_card;`
struct StyleCard;

/// `lv_example_style_size_position()`: fixed widths with content-sized heights.
pub fn lv_example_style_size_position(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_card] = styles_once::<StyleCard, 1>(ui, || {
        let mut style_card = Style::new();
        style_card.set_radius(16);
        style_card.set_bg_opa((255 * 100 / 100) as u8);
        style_card.set_bg_color(Color::hex(0xffffff));
        style_card.set_border_width(0);
        style_card.set_shadow_color(ACCENT);
        style_card.set_shadow_width(22);
        style_card.set_shadow_offset_y(6);
        style_card.set_shadow_opa(55);
        [style_card]
    });

    let screen = screen_active(ui);

    let container_1 = ui.obj_create(Some(screen));
    ui.obj_set_size(container_1, 140, coord::SIZE_CONTENT);
    ui.obj_set_align(container_1, Align::Center);
    ui.obj_set_y(container_1, -40);
    ui.obj_set_style_pad_all(container_1, 12, sel);
    ui.obj_add_style(container_1, style_card, sel);
    let label_1 = Label::create(ui, container_1);
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "140 wide");

    let container_2 = ui.obj_create(Some(screen));
    ui.obj_set_size(container_2, 260, coord::SIZE_CONTENT);
    ui.obj_set_align(container_2, Align::Center);
    ui.obj_set_y(container_2, 40);
    ui.obj_set_style_pad_all(container_2, 28, sel);
    ui.obj_add_style(container_2, style_card, sel);
    let label_2 = Label::create(ui, container_2);
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "260 wide, roomy padding");
}
