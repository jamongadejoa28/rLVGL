//! Port of `examples/layouts/flex/lv_example_flex_rtl.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::text::BaseDir;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Obj, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_label;`
struct StyleLabel;

/// The three colored labels of one row.
fn add_row_labels(ui: &mut Ui, container: Obj, style_label: ferrovgl_core::core::obj_style::StyleId) {
    for (color, text) in [(0x3498db, "First"), (0x2ecc71, "Second"), (0xe74c3c, "Third")] {
        let label = Label::create(ui, container);
        ui.obj_set_style_bg_color(label.obj(), Color::hex(color), StyleSelector::DEFAULT);
        label.set_text(ui, text);
        ui.obj_add_style(label.obj(), style_label, StyleSelector::DEFAULT);
    }
}

/// `lv_example_flex_rtl()`: the same row in left-to-right and right-to-left base direction.
pub fn lv_example_flex_rtl(ui: &mut Ui) {
    let [style_label] = styles_once::<StyleLabel, 1>(ui, || {
        let mut style_label = Style::new();
        style_label.set_bg_opa((255 * 100 / 100) as u8);
        style_label.set_text_color(Color::hex(0xffffff));
        style_label.set_pad_all(8);
        [style_label]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_pad_row(screen, 10, StyleSelector::DEFAULT);

    /* Default left-to-right row */
    let label_1 = Label::create(ui, screen);
    label_1.set_text(ui, "LTR (default):");

    let container_1 = ui.obj_create(Some(screen));
    ui.obj_set_flex_flow(container_1, FlexFlow::Row);
    ui.obj_set_size(container_1, coord::pct(90), coord::SIZE_CONTENT);
    add_row_labels(ui, container_1, style_label);

    /* Right-to-left row with the same children */
    let label_5 = Label::create(ui, screen);
    label_5.set_text(ui, "RTL:");

    let container_2 = ui.obj_create(Some(screen));
    ui.obj_set_flex_flow(container_2, FlexFlow::Row);
    ui.obj_set_style_base_dir(container_2, BaseDir::Rtl, StyleSelector::DEFAULT);
    ui.obj_set_size(container_2, coord::pct(90), coord::SIZE_CONTENT);
    add_row_labels(ui, container_2, style_label);
}
