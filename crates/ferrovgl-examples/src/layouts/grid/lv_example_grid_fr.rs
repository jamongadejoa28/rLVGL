//! Port of `examples/layouts/grid/lv_example_grid_fr.c`.

use ferrovgl_core::layouts::LAYOUT_GRID;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::layouts::grid::{GRID_TEMPLATE_LAST, GridAlign, grid_fr};
use ferrovgl_core::misc::style::TracksRef;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::screen_active;

/// `container_1_style_grid_column_dsc_array_0`
static COLUMN_DSC: [i32; 4] = [80, grid_fr(1), grid_fr(2), GRID_TEMPLATE_LAST];
/// `container_1_style_grid_row_dsc_array_1`
static ROW_DSC: [i32; 3] = [44, grid_fr(1), GRID_TEMPLATE_LAST];

/// The colors and texts of the three columns.
const COLUMNS: [(u32, &str); 3] = [(0xb8d8f8, "80px"), (0x7ec8ff, "1fr"), (0x4daff7, "2fr")];

/// `lv_example_grid_fr()`: fixed and free-unit (`LV_GRID_FR`) tracks.
pub fn lv_example_grid_fr(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    /* Main grid container with fixed and FR tracks */
    let container_1 = ui.obj_create(Some(screen));
    ui.obj_set_style_grid_column_dsc_array(container_1, TracksRef::Static(&COLUMN_DSC), sel);
    ui.obj_set_style_grid_row_dsc_array(container_1, TracksRef::Static(&ROW_DSC), sel);
    ui.obj_set_style_layout(container_1, LAYOUT_GRID as u16, sel);
    ui.obj_set_size(container_1, coord::pct(100), 190);

    /* Header row showing column proportions */
    for (col, (color, text)) in (0..).zip(COLUMNS) {
        let label = Label::create(ui, container_1);
        let obj = label.obj();
        ui.obj_set_style_grid_cell_x_align(obj, GridAlign::Stretch, sel);
        ui.obj_set_style_grid_cell_column_pos(obj, col, sel);
        ui.obj_set_style_grid_cell_y_align(obj, GridAlign::Stretch, sel);
        ui.obj_set_style_grid_cell_row_pos(obj, 0, sel);
        ui.obj_set_style_bg_color(obj, Color::hex(color), sel);
        ui.obj_set_style_bg_opa(obj, (255 * 100 / 100) as u8, sel);
        label.set_text(ui, text);
    }

    /* Second row with matching column tracks */
    for (col, (color, text)) in (0..).zip(COLUMNS) {
        let container = ui.obj_create(Some(container_1));
        ui.obj_set_style_grid_cell_x_align(container, GridAlign::Stretch, sel);
        ui.obj_set_style_grid_cell_column_pos(container, col, sel);
        ui.obj_set_style_grid_cell_y_align(container, GridAlign::Stretch, sel);
        ui.obj_set_style_grid_cell_row_pos(container, 1, sel);
        ui.obj_set_style_bg_color(container, Color::hex(color), sel);
        ui.obj_set_style_bg_opa(container, (255 * 100 / 100) as u8, sel);
        let label = Label::create(ui, container);
        label.set_text(ui, text);
        ui.obj_set_align(label.obj(), Align::Center);
    }
}
