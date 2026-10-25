//! Port of `examples/layouts/grid/lv_example_grid_cell_span.c`.

use ferrovgl_core::core::obj_style::StyleId;
use ferrovgl_core::layouts::LAYOUT_GRID;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::layouts::grid::{GRID_TEMPLATE_LAST, GridAlign};
use ferrovgl_core::misc::style::TracksRef;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, Obj, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_card;`
struct StyleCard;

/// `container_1_style_grid_column_dsc_array_0`
static COLUMN_DSC: [i32; 4] = [90, 90, 90, GRID_TEMPLATE_LAST];
/// `container_1_style_grid_row_dsc_array_1`
static ROW_DSC: [i32; 4] = [60, 60, 60, GRID_TEMPLATE_LAST];

/// A content-sized card aligned inside its cell.
fn aligned_card(ui: &mut Ui, parent: Obj, align: GridAlign, col: i32, text: &str, style_card: StyleId) {
    let sel = StyleSelector::DEFAULT;
    let container = ui.obj_create(Some(parent));
    ui.obj_set_style_grid_cell_x_align(container, align, sel);
    ui.obj_set_style_grid_cell_column_pos(container, col, sel);
    ui.obj_set_style_grid_cell_y_align(container, align, sel);
    ui.obj_set_style_grid_cell_row_pos(container, 0, sel);
    ui.obj_set_size(container, coord::SIZE_CONTENT, coord::SIZE_CONTENT);
    ui.obj_add_style(container, style_card, sel);
    let label = Label::create(ui, container);
    label.set_text(ui, text);
}

/// `lv_example_grid_cell_span()`: cell alignment and items spanning several tracks.
pub fn lv_example_grid_cell_span(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_card] = styles_once::<StyleCard, 1>(ui, || {
        let mut style_card = Style::new();
        style_card.set_bg_color(Color::hex(0xf6f6f6));
        style_card.set_bg_opa((255 * 100 / 100) as u8);
        style_card.set_pad_all(6);
        style_card.set_radius(6);
        [style_card]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    /* Main grid container for alignment and span cases */
    let container_1 = ui.obj_create(Some(screen));
    ui.obj_set_style_grid_column_dsc_array(container_1, TracksRef::Static(&COLUMN_DSC), sel);
    ui.obj_set_style_grid_row_dsc_array(container_1, TracksRef::Static(&ROW_DSC), sel);
    ui.obj_set_style_layout(container_1, LAYOUT_GRID as u16, sel);
    ui.obj_set_size(container_1, coord::pct(90), coord::SIZE_CONTENT);

    /* Cell aligned to start/start */
    aligned_card(ui, container_1, GridAlign::Start, 0, "start/start", style_card);
    /* Cell aligned to center/center */
    aligned_card(ui, container_1, GridAlign::Center, 1, "center", style_card);
    /* Cell aligned to end/end */
    aligned_card(ui, container_1, GridAlign::End, 2, "end/end", style_card);

    /* Item spanning two columns */
    let container_5 = ui.obj_create(Some(container_1));
    ui.obj_set_style_grid_cell_x_align(container_5, GridAlign::Stretch, sel);
    ui.obj_set_style_grid_cell_column_pos(container_5, 1, sel);
    ui.obj_set_style_grid_cell_column_span(container_5, 2, sel);
    ui.obj_set_style_grid_cell_y_align(container_5, GridAlign::Stretch, sel);
    ui.obj_set_style_grid_cell_row_pos(container_5, 1, sel);
    ui.obj_set_style_bg_color(container_5, Color::hex(0xd8ecff), sel);
    ui.obj_set_style_bg_opa(container_5, (255 * 100 / 100) as u8, sel);
    let label_4 = Label::create(ui, container_5);
    label_4.set_text(ui, "span 2 columns");
    ui.obj_set_align(label_4.obj(), Align::Center);

    /* Item spanning two rows */
    let container_6 = ui.obj_create(Some(container_1));
    ui.obj_set_style_grid_cell_x_align(container_6, GridAlign::Stretch, sel);
    ui.obj_set_style_grid_cell_column_pos(container_6, 0, sel);
    ui.obj_set_style_grid_cell_y_align(container_6, GridAlign::Stretch, sel);
    ui.obj_set_style_grid_cell_row_pos(container_6, 1, sel);
    ui.obj_set_style_grid_cell_row_span(container_6, 2, sel);
    ui.obj_set_style_bg_color(container_6, Color::hex(0xffe8cc), sel);
    ui.obj_set_style_bg_opa(container_6, (255 * 100 / 100) as u8, sel);
    let label_5 = Label::create(ui, container_6);
    label_5.set_text(ui, "span\n2 rows");
    ui.obj_set_align(label_5.obj(), Align::Center);
}
