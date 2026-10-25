//! Port of `examples/scroll/lv_example_scroll_rtl.c`.

use ferrovgl_core::core::obj::ScrollbarMode;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::text::BaseDir;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, Part, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `lv_example_scroll_rtl()`: the scrollbar of a right-to-left panel is on the left.
pub fn lv_example_scroll_rtl(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 12, sel);

    let container = ui.obj_create(Some(screen));
    ui.obj_set_size(container, 220, 160);
    ui.obj_set_flex_flow(container, FlexFlow::Column);
    ui.obj_set_style_pad_row(container, 8, sel);
    ui.obj_set_style_base_dir(container, BaseDir::Rtl, sel);
    ui.obj_set_scrollbar_mode(container, ScrollbarMode::On);
    ui.obj_set_style_bg_color(container, Color::hex(0x9429ff), Part::SCROLLBAR.into());
    ui.obj_set_style_bg_opa(container, (255 * 100 / 100) as u8, Part::SCROLLBAR.into());

    for i in 1..=6 {
        let button = Button::create(ui, container).obj();
        ui.obj_set_width(button, coord::pct(100));
        let label = Label::create(ui, button);
        ui.obj_set_align(label.obj(), Align::Center);
        label.set_text_fmt(ui, format_args!("Row {i}"));
    }
}
