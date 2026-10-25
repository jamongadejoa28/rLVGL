//! Port of `examples/layouts/grid/lv_example_grid_descriptors.c`.

use ferrovgl_core::layouts::LAYOUT_GRID;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::layouts::grid::{GRID_TEMPLATE_LAST, GridAlign};
use ferrovgl_core::misc::style::TracksRef;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::screen_active;

/// `container_style_grid_column_dsc_array_0`
static COLUMN_DSC: [i32; 4] = [70, 70, 70, GRID_TEMPLATE_LAST];
/// `container_style_grid_row_dsc_array_1`
static ROW_DSC: [i32; 3] = [44, 44, GRID_TEMPLATE_LAST];

/// `lv_example_grid_descriptors()`: fixed column and row descriptors.
pub fn lv_example_grid_descriptors(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    /* Main grid container with fixed descriptors */
    let container = ui.obj_create(Some(screen));
    ui.obj_set_style_grid_column_dsc_array(container, TracksRef::Static(&COLUMN_DSC), sel);
    ui.obj_set_style_grid_row_dsc_array(container, TracksRef::Static(&ROW_DSC), sel);
    ui.obj_set_style_layout(container, LAYOUT_GRID as u16, sel);
    ui.obj_set_size(container, 400, coord::SIZE_CONTENT);

    /* Row 0 cells, then row 1 cells */
    for (row, color) in [(0, 0x4a90d9), (1, 0x27ae60)] {
        for col in 0..3 {
            let label = Label::create(ui, container);
            let obj = label.obj();
            ui.obj_set_style_grid_cell_x_align(obj, GridAlign::Stretch, sel);
            ui.obj_set_style_grid_cell_column_pos(obj, col, sel);
            ui.obj_set_style_grid_cell_y_align(obj, GridAlign::Stretch, sel);
            ui.obj_set_style_grid_cell_row_pos(obj, row, sel);
            ui.obj_set_style_bg_color(obj, Color::hex(color), sel);
            ui.obj_set_style_bg_opa(obj, (255 * 100 / 100) as u8, sel);
            ui.obj_set_style_text_color(obj, Color::hex(0xffffff), sel);
            label.set_text_fmt(ui, format_args!("{col},{row}"));
        }
    }
}
