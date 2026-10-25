//! Port of `examples/styles/lv_example_style_padding.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Style, StyleSelector, Ui};

use crate::util::{screen_active, styles_once};

/// `ACCENT`
const ACCENT: Color = Color::hex(0x6366f1);

/// `static lv_style_t style_block;`
struct StyleBlock;

/// `lv_example_style_padding()`: padding shrinks the content area of a card.
pub fn lv_example_style_padding(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_block] = styles_once::<StyleBlock, 1>(ui, || {
        let mut style_block = Style::new();
        style_block.set_radius(10);
        style_block.set_bg_opa((255 * 100 / 100) as u8);
        style_block.set_bg_color(ACCENT);
        [style_block]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::SpaceEvenly, sel);

    let container_2 = ui.obj_create(Some(screen));
    ui.obj_set_size(container_2, 120, 120);
    ui.obj_set_style_pad_top(container_2, 26, sel);
    ui.obj_set_style_pad_left(container_2, 26, sel);
    ui.obj_set_style_pad_right(container_2, 8, sel);
    ui.obj_set_style_pad_bottom(container_2, 8, sel);
    let container_3 = ui.obj_create(Some(container_2));
    ui.obj_set_size(container_3, coord::pct(100), coord::pct(100));
    ui.obj_add_style(container_3, style_block, sel);

    let container_4 = ui.obj_create(Some(screen));
    ui.obj_set_size(container_4, 120, 120);
    ui.obj_set_style_pad_all(container_4, 18, sel);
    let container_5 = ui.obj_create(Some(container_4));
    ui.obj_set_size(container_5, coord::pct(100), coord::pct(100));
    ui.obj_add_style(container_5, style_block, sel);
}
