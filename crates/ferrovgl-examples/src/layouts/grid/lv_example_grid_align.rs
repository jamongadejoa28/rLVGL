//! Port of `examples/layouts/grid/lv_example_grid_align.c`.

use ferrovgl_core::layouts::LAYOUT_GRID;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::layouts::grid::{GRID_TEMPLATE_LAST, GridAlign};
use ferrovgl_core::misc::style::TracksRef;
use ferrovgl_core::{Color, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::screen_active;

/// `container_style_grid_column_dsc_array_0`
static COLUMN_DSC: [i32; 4] = [56, 56, 56, GRID_TEMPLATE_LAST];
/// `container_style_grid_row_dsc_array_1`
static ROW_DSC: [i32; 3] = [36, 36, GRID_TEMPLATE_LAST];

/// `lv_example_grid_align()`: track alignment inside a larger grid container.
pub fn lv_example_grid_align(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 8, sel);

    /* Grid container with explicit track alignment settings */
    let container = ui.obj_create(Some(screen));
    ui.obj_set_style_grid_column_dsc_array(container, TracksRef::Static(&COLUMN_DSC), sel);
    ui.obj_set_style_grid_row_dsc_array(container, TracksRef::Static(&ROW_DSC), sel);
    ui.obj_set_style_grid_column_align(container, GridAlign::SpaceBetween, sel);
    ui.obj_set_style_grid_row_align(container, GridAlign::End, sel);
    ui.obj_set_style_layout(container, LAYOUT_GRID as u16, sel);
    ui.obj_set_size(container, 300, 180);

    /* First row, then second row */
    for (row, color) in [(0, 0xd6eaf8), (1, 0xd5f5e3)] {
        for col in 0..3 {
            let label = Label::create(ui, container);
            let obj = label.obj();
            ui.obj_set_style_grid_cell_x_align(obj, GridAlign::Stretch, sel);
            ui.obj_set_style_grid_cell_column_pos(obj, col, sel);
            ui.obj_set_style_grid_cell_y_align(obj, GridAlign::Stretch, sel);
            ui.obj_set_style_grid_cell_row_pos(obj, row, sel);
            ui.obj_set_style_bg_color(obj, Color::hex(color), sel);
            ui.obj_set_style_bg_opa(obj, (255 * 100 / 100) as u8, sel);
            label.set_text_fmt(ui, format_args!("{col},{row}"));
        }
    }
}
