//! Port of `examples/layouts/grid/lv_example_grid_internal_padding.c`.

use ferrovgl_core::layouts::LAYOUT_GRID;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::layouts::grid::GRID_TEMPLATE_LAST;
use ferrovgl_core::misc::style::TracksRef;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_chip;`
struct StyleChip;

/// `container_style_grid_column_dsc_array_0`
static COLUMN_DSC: [i32; 4] = [50, 50, 50, GRID_TEMPLATE_LAST];
/// `container_style_grid_row_dsc_array_1`
static ROW_DSC: [i32; 3] = [40, 40, GRID_TEMPLATE_LAST];

/// `lv_example_grid_internal_padding()`: explicit row and column gaps.
pub fn lv_example_grid_internal_padding(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_chip] = styles_once::<StyleChip, 1>(ui, || {
        let mut style_chip = Style::new();
        style_chip.set_bg_opa((255 * 100 / 100) as u8);
        style_chip.set_text_color(Color::hex(0xffffff));
        style_chip.set_pad_all(6);
        style_chip.set_radius(4);
        [style_chip]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 8, sel);

    /* Grid container with explicit row and column gaps */
    let container = ui.obj_create(Some(screen));
    ui.obj_set_style_grid_column_dsc_array(container, TracksRef::Static(&COLUMN_DSC), sel);
    ui.obj_set_style_grid_row_dsc_array(container, TracksRef::Static(&ROW_DSC), sel);
    ui.obj_set_style_pad_column(container, 0, sel);
    ui.obj_set_style_pad_row(container, 66, sel);
    ui.obj_set_style_layout(container, LAYOUT_GRID as u16, sel);
    ui.obj_set_size(container, 280, coord::SIZE_CONTENT);

    for (col, row, color, text) in [
        (0, 0, 0x1f77b4, "One"),
        (1, 0, 0xff7f0e, "Two"),
        (2, 0, 0x2ca02c, "Three"),
        (0, 1, 0xd62728, "Four"),
        (1, 1, 0x9467bd, "Five"),
        (2, 1, 0x17becf, "Six"),
    ] {
        let label = Label::create(ui, container);
        ui.obj_set_style_grid_cell_column_pos(label.obj(), col, sel);
        ui.obj_set_style_grid_cell_row_pos(label.obj(), row, sel);
        ui.obj_set_style_bg_color(label.obj(), Color::hex(color), sel);
        label.set_text(ui, text);
        ui.obj_add_style(label.obj(), style_chip, sel);
    }
}
