//! Port of `examples/layouts/grid/lv_example_grid_rtl.c`.

use ferrovgl_core::core::obj_style::StyleId;
use ferrovgl_core::layouts::LAYOUT_GRID;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::layouts::grid::GRID_TEMPLATE_LAST;
use ferrovgl_core::misc::style::TracksRef;
use ferrovgl_core::misc::text::BaseDir;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Obj, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_badge;`
struct StyleBadge;

/// `container_1_style_grid_column_dsc_array_0`
static COLUMN_DSC_1: [i32; 4] = [40, 40, 40, GRID_TEMPLATE_LAST];
/// `container_1_style_grid_row_dsc_array_1`
static ROW_DSC_1: [i32; 2] = [38, GRID_TEMPLATE_LAST];
/// `container_2_style_grid_column_dsc_array_2`
static COLUMN_DSC_2: [i32; 4] = [40, 40, 40, GRID_TEMPLATE_LAST];
/// `container_2_style_grid_row_dsc_array_3`
static ROW_DSC_2: [i32; 2] = [38, GRID_TEMPLATE_LAST];

/// The three numbered badges of one grid.
fn add_badges(ui: &mut Ui, container: Obj, style_badge: StyleId) {
    let sel = StyleSelector::DEFAULT;
    for (col, color, text) in [(0, 0x3498db, "0"), (1, 0x2ecc71, "1"), (2, 0xe74c3c, "2")] {
        let label = Label::create(ui, container);
        ui.obj_set_style_grid_cell_column_pos(label.obj(), col, sel);
        ui.obj_set_style_grid_cell_row_pos(label.obj(), 0, sel);
        ui.obj_set_style_bg_color(label.obj(), Color::hex(color), sel);
        label.set_text(ui, text);
        ui.obj_add_style(label.obj(), style_badge, sel);
    }
}

/// `lv_example_grid_rtl()`: the same grid in left-to-right and right-to-left base direction.
pub fn lv_example_grid_rtl(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_badge] = styles_once::<StyleBadge, 1>(ui, || {
        let mut style_badge = Style::new();
        style_badge.set_bg_opa((255 * 100 / 100) as u8);
        style_badge.set_text_color(Color::hex(0xffffff));
        style_badge.set_pad_all(6);
        style_badge.set_radius(4);
        [style_badge]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 8, sel);

    /* LTR reference block */
    let label_1 = Label::create(ui, screen);
    label_1.set_text(ui, "LTR (default):");

    let container_1 = ui.obj_create(Some(screen));
    ui.obj_set_style_grid_column_dsc_array(container_1, TracksRef::Static(&COLUMN_DSC_1), sel);
    ui.obj_set_style_grid_row_dsc_array(container_1, TracksRef::Static(&ROW_DSC_1), sel);
    ui.obj_set_style_layout(container_1, LAYOUT_GRID as u16, sel);
    ui.obj_set_size(container_1, 280, coord::SIZE_CONTENT);
    add_badges(ui, container_1, style_badge);

    /* RTL block with identical children */
    let label_5 = Label::create(ui, screen);
    label_5.set_text(ui, "RTL:");

    let container_2 = ui.obj_create(Some(screen));
    ui.obj_set_style_base_dir(container_2, BaseDir::Rtl, sel);
    ui.obj_set_style_grid_column_dsc_array(container_2, TracksRef::Static(&COLUMN_DSC_2), sel);
    ui.obj_set_style_grid_row_dsc_array(container_2, TracksRef::Static(&ROW_DSC_2), sel);
    ui.obj_set_style_layout(container_2, LAYOUT_GRID as u16, sel);
    ui.obj_set_size(container_2, 280, coord::SIZE_CONTENT);
    add_badges(ui, container_2, style_badge);
}
