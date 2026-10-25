//! Port of `examples/scroll/lv_example_scroll_scrollbar_style.c`.

use ferrovgl_core::core::obj::ScrollbarMode;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, Part, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::{screen_active, styles_once};

/// `ACCENT`
const ACCENT: Color = Color::hex(0x9429ff);

/// `static lv_style_t style_scrollbar;`
struct StyleScrollbar;

/// `lv_example_scroll_scrollbar_style()`: a styled `LV_PART_SCROLLBAR`.
pub fn lv_example_scroll_scrollbar_style(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_scrollbar] = styles_once::<StyleScrollbar, 1>(ui, || {
        let mut style_scrollbar = Style::new();
        style_scrollbar.set_width(10);
        style_scrollbar.set_radius(5);
        style_scrollbar.set_bg_color(ACCENT);
        style_scrollbar.set_bg_opa((255 * 100 / 100) as u8);
        style_scrollbar.set_pad_right(4);
        style_scrollbar.set_pad_top(4);
        [style_scrollbar]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 12, sel);

    let container = ui.obj_create(Some(screen));
    ui.obj_set_size(container, 240, 160);
    ui.obj_set_flex_flow(container, FlexFlow::Column);
    ui.obj_set_style_pad_row(container, 8, sel);
    ui.obj_set_scrollbar_mode(container, ScrollbarMode::On);
    ui.obj_add_style(container, style_scrollbar, Part::SCROLLBAR.into());

    for i in 1..=6 {
        let button = Button::create(ui, container).obj();
        ui.obj_set_width(button, coord::pct(100));
        let label = Label::create(ui, button);
        ui.obj_set_align(label.obj(), Align::Center);
        label.set_text_fmt(ui, format_args!("Row {i}"));
    }
}
