//! Port of `examples/scroll/lv_example_scroll_scrollbar_mode.c`.

use ferrovgl_core::core::obj::ScrollbarMode;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, Obj, Part, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// A title label followed by the buttons `A` to `D`.
fn fill_panel(ui: &mut Ui, panel: Obj, title: &str) {
    let label = Label::create(ui, panel);
    label.set_text(ui, title);
    for text in ["A", "B", "C", "D"] {
        let button = Button::create(ui, panel).obj();
        ui.obj_set_width(button, coord::pct(100));
        let label = Label::create(ui, button);
        ui.obj_set_align(label.obj(), Align::Center);
        label.set_text(ui, text);
    }
}

/// `lv_example_scroll_scrollbar_mode()`: scrollbar modes `off` and `on`.
pub fn lv_example_scroll_scrollbar_mode(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 12, sel);

    let container_1 = ui.obj_create(Some(screen));
    ui.obj_set_size(container_1, coord::pct(100), 160);
    ui.obj_set_flex_flow(container_1, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(container_1, FlexAlign::Center, sel);
    ui.obj_set_style_pad_column(container_1, 16, sel);

    /* scrollbar_mode="off": scrolls, bar never shown */
    let container_2 = ui.obj_create(Some(container_1));
    ui.obj_set_size(container_2, 130, coord::pct(100));
    ui.obj_set_flex_flow(container_2, FlexFlow::Column);
    ui.obj_set_style_pad_row(container_2, 8, sel);
    ui.obj_set_scrollbar_mode(container_2, ScrollbarMode::Off);
    fill_panel(ui, container_2, "mode: off");

    /* scrollbar_mode="on": bar always visible */
    let container_3 = ui.obj_create(Some(container_1));
    ui.obj_set_size(container_3, 130, coord::pct(100));
    ui.obj_set_flex_flow(container_3, FlexFlow::Column);
    ui.obj_set_style_pad_row(container_3, 8, sel);
    ui.obj_set_scrollbar_mode(container_3, ScrollbarMode::On);
    ui.obj_set_style_bg_color(container_3, Color::hex(0x9429ff), Part::SCROLLBAR.into());
    ui.obj_set_style_bg_opa(container_3, (255 * 100 / 100) as u8, Part::SCROLLBAR.into());
    fill_panel(ui, container_3, "mode: on");
}
