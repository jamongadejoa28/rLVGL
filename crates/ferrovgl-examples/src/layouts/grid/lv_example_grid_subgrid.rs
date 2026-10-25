//! Port of `examples/layouts/grid/lv_example_grid_subgrid.c`.

use ferrovgl_core::layouts::LAYOUT_GRID;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::layouts::grid::{GRID_TEMPLATE_LAST, GridAlign};
use ferrovgl_core::misc::style::TracksRef;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_tile;`
struct StyleTile;

/// `container_1_style_grid_column_dsc_array_0`
static COLUMN_DSC: [i32; 4] = [60, 60, 60, GRID_TEMPLATE_LAST];
/// `container_1_style_grid_row_dsc_array_1`
static ROW_DSC: [i32; 3] = [40, 40, GRID_TEMPLATE_LAST];

/// `lv_example_grid_subgrid()`: a nested grid inheriting the parent's tracks.
pub fn lv_example_grid_subgrid(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_tile] = styles_once::<StyleTile, 1>(ui, || {
        let mut style_tile = Style::new();
        style_tile.set_bg_opa((255 * 100 / 100) as u8);
        style_tile.set_text_color(Color::hex(0xffffff));
        style_tile.set_pad_all(6);
        style_tile.set_radius(4);
        [style_tile]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 8, sel);

    /* Parent grid container */
    let container_1 = ui.obj_create(Some(screen));
    ui.obj_set_style_grid_column_dsc_array(container_1, TracksRef::Static(&COLUMN_DSC), sel);
    ui.obj_set_style_grid_row_dsc_array(container_1, TracksRef::Static(&ROW_DSC), sel);
    ui.obj_set_style_layout(container_1, LAYOUT_GRID as u16, sel);
    ui.obj_set_size(container_1, 280, coord::SIZE_CONTENT);

    /* Label in the parent grid: left tile */
    let label_1 = Label::create(ui, container_1);
    ui.obj_set_style_grid_cell_column_pos(label_1.obj(), 0, sel);
    ui.obj_set_style_grid_cell_row_pos(label_1.obj(), 0, sel);
    ui.obj_set_style_bg_color(label_1.obj(), Color::hex(0x1f77b4), sel);
    label_1.set_text(ui, "P0,0");
    ui.obj_add_style(label_1.obj(), style_tile, sel);

    /* Sub-grid container spanning 2 columns and 2 rows
    Do not set the grid descriptor to let them inherit the parent tracks */
    let container_2 = ui.obj_create(Some(container_1));
    ui.obj_set_style_grid_cell_x_align(container_2, GridAlign::Stretch, sel);
    ui.obj_set_style_grid_cell_column_pos(container_2, 1, sel);
    ui.obj_set_style_grid_cell_column_span(container_2, 2, sel);
    ui.obj_set_style_grid_cell_y_align(container_2, GridAlign::Stretch, sel);
    ui.obj_set_style_grid_cell_row_pos(container_2, 0, sel);
    ui.obj_set_style_grid_cell_row_span(container_2, 2, sel);
    ui.obj_set_style_layout(container_2, LAYOUT_GRID as u16, sel);
    ui.obj_set_style_pad_all(container_2, 4, sel);

    /* Sub-grid labels at local cells (0,0), (1,0), (0,1), (1,1) */
    for (col, row, color, text) in [
        (0, 0, 0x2ca02c, "S0,0"),
        (1, 0, 0xff7f0e, "S1,0"),
        (0, 1, 0xd62728, "S0,1"),
        (1, 1, 0x9467bd, "S1,1"),
    ] {
        let label = Label::create(ui, container_2);
        ui.obj_set_style_grid_cell_column_pos(label.obj(), col, sel);
        ui.obj_set_style_grid_cell_row_pos(label.obj(), row, sel);
        ui.obj_set_style_bg_color(label.obj(), Color::hex(color), sel);
        label.set_text(ui, text);
        ui.obj_add_style(label.obj(), style_tile, sel);
    }
}
