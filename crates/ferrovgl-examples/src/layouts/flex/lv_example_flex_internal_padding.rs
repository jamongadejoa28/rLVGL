//! Port of `examples/layouts/flex/lv_example_flex_internal_padding.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_tag;`
struct StyleTag;

/// `lv_example_flex_internal_padding()`: row and column gaps of a wrapping row.
pub fn lv_example_flex_internal_padding(ui: &mut Ui) {
    let [style_tag] = styles_once::<StyleTag, 1>(ui, || {
        let mut style_tag = Style::new();
        style_tag.set_bg_opa((255 * 100 / 100) as u8);
        style_tag.set_pad_ver(12);
        style_tag.set_pad_hor(18);
        style_tag.set_radius(4);
        [style_tag]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_pad_row(screen, 10, StyleSelector::DEFAULT);

    /* Demo container with row/column gaps */
    let container = ui.obj_create(Some(screen));
    ui.obj_set_flex_flow(container, FlexFlow::RowWrap);
    ui.obj_set_style_pad_column(container, 10, StyleSelector::DEFAULT);
    ui.obj_set_style_pad_row(container, 30, StyleSelector::DEFAULT);
    ui.obj_set_size(container, coord::pct(90), coord::SIZE_CONTENT);

    for (color, text) in [
        (0x5e9ee3, "HTML"),
        (0xe67e22, "CSS"),
        (0x27ae60, "JavaScript"),
        (0x8e44ad, "Python"),
        (0xc0392b, "C++"),
        (0x16a085, "Rust"),
        (0xd8da19, "Java"),
        (0x18d99e, "PHP"),
        (0xeb4c9e, "Zig"),
    ] {
        let label = Label::create(ui, container);
        ui.obj_set_style_bg_color(label.obj(), Color::hex(color), StyleSelector::DEFAULT);
        label.set_text(ui, text);
        ui.obj_add_style(label.obj(), style_tag, StyleSelector::DEFAULT);
    }
}
