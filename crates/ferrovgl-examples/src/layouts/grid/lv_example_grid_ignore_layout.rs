//! Port of `examples/layouts/grid/lv_example_grid_ignore_layout.c`.

use ferrovgl_core::layouts::LAYOUT_GRID;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::layouts::grid::GRID_TEMPLATE_LAST;
use ferrovgl_core::misc::style::TracksRef;
use ferrovgl_core::{Align, Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_cell;`
struct StyleCell;

/// `container_style_grid_column_dsc_array_0`
static COLUMN_DSC: [i32; 4] = [54, 54, 54, GRID_TEMPLATE_LAST];
/// `container_style_grid_row_dsc_array_1`
static ROW_DSC: [i32; 3] = [42, 42, GRID_TEMPLATE_LAST];

/// `lv_example_grid_ignore_layout()`: an item that the grid skips.
pub fn lv_example_grid_ignore_layout(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_cell] = styles_once::<StyleCell, 1>(ui, || {
        let mut style_cell = Style::new();
        style_cell.set_bg_opa((255 * 100 / 100) as u8);
        style_cell.set_text_color(Color::hex(0xffffff));
        style_cell.set_pad_all(6);
        style_cell.set_radius(4);
        [style_cell]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 8, sel);

    /* Main grid container */
    let container = ui.obj_create(Some(screen));
    ui.obj_set_style_grid_column_dsc_array(container, TracksRef::Static(&COLUMN_DSC), sel);
    ui.obj_set_style_grid_row_dsc_array(container, TracksRef::Static(&ROW_DSC), sel);
    ui.obj_set_style_layout(container, LAYOUT_GRID as u16, sel);
    ui.obj_set_size(container, 280, 120);

    for (col, row, color, text) in [
        (0, 0, 0x3498db, "A"),
        (1, 0, 0x2ecc71, "B"),
        (2, 0, 0xe67e22, "C"),
        (0, 1, 0x8e44ad, "D"),
    ] {
        let label = Label::create(ui, container);
        ui.obj_set_style_grid_cell_column_pos(label.obj(), col, sel);
        ui.obj_set_style_grid_cell_row_pos(label.obj(), row, sel);
        ui.obj_set_style_bg_color(label.obj(), Color::hex(color), sel);
        label.set_text(ui, text);
        ui.obj_add_style(label.obj(), style_cell, sel);
    }

    let label_5 = Label::create(ui, container);
    ui.obj_set_ignore_layout(label_5.obj(), true);
    ui.obj_set_align(label_5.obj(), Align::Center);
    ui.obj_set_style_bg_color(label_5.obj(), Color::hex(0xe74c3c), sel);
    label_5.set_text(ui, "FREE");
    ui.obj_add_style(label_5.obj(), style_cell, sel);
}
