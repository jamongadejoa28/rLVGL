//! Port of `examples/widgets/bar/lv_example_bar_value_range.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::anim::ANIM_OFF;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, StyleSelector, Ui};
use ferrovgl_widgets::{Bar, Label};

use crate::util::screen_active;

/// `TEXT_COLOR`
const TEXT_COLOR: Color = Color::hex(0x013992);

/// `lv_example_bar_value_range()`: the default and a custom value range.
pub fn lv_example_bar_value_range(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Default 0..100 range, then a custom range that does not start at zero */
    for (min, max, value, text) in [(0, 100, 30, "0..100"), (-20, 80, 35, "-20..80")] {
        let bar = Bar::create(ui, screen);
        ui.obj_set_size(bar.obj(), coord::pct(90), 20);
        bar.set_min_value(ui, min);
        bar.set_max_value(ui, max);
        bar.set_value(ui, value, ANIM_OFF);
        let label = Label::create(ui, bar.obj());
        ui.obj_set_align(label.obj(), Align::Center);
        label.set_text(ui, text);
        ui.obj_set_style_text_color(label.obj(), TEXT_COLOR, sel);
    }
}
