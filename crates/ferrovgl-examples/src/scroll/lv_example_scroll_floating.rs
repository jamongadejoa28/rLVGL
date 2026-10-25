//! Port of `examples/scroll/lv_example_scroll_floating.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_fab;`
struct StyleFab;

/// `lv_example_scroll_floating()`: a floating button that ignores scrolling.
pub fn lv_example_scroll_floating(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_fab] = styles_once::<StyleFab, 1>(ui, || {
        let mut style_fab = Style::new();
        style_fab.set_radius(22);
        [style_fab]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 12, sel);

    let container = ui.obj_create(Some(screen));
    ui.obj_set_size(container, 240, 180);
    ui.obj_set_flex_flow(container, FlexFlow::Column);
    ui.obj_set_style_pad_row(container, 8, sel);

    for text in ["Track 1", "Track 2", "Track 3", "Track 4", "Track 5", "Track 6"] {
        let button = Button::create(ui, container).obj();
        ui.obj_set_width(button, coord::pct(100));
        let label = Label::create(ui, button);
        ui.obj_set_align(label.obj(), Align::Center);
        label.set_text(ui, text);
    }

    /* Pinned to bottom-right, unaffected by the list's scroll */
    let button_7 = Button::create(ui, container).obj();
    ui.obj_set_size(button_7, 44, 44);
    ui.obj_set_align(button_7, Align::BottomRight);
    ui.obj_set_floating(button_7, true);
    ui.obj_set_style_bg_color(button_7, Color::hex(0x9429ff), sel);
    ui.obj_add_style(button_7, style_fab, sel);
    let label_7 = Label::create(ui, button_7);
    ui.obj_set_align(label_7.obj(), Align::Center);
    label_7.set_text(ui, "+");
}
