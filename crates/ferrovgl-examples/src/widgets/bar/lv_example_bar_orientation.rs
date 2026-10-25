//! Port of `examples/widgets/bar/lv_example_bar_orientation.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::anim::ANIM_OFF;
use ferrovgl_core::misc::text::TextAlign;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, StyleSelector, Ui};
use ferrovgl_widgets::{Bar, BarOrientation, Label};

use crate::util::screen_active;

/// `TEXT_COLOR`
const TEXT_COLOR: Color = Color::hex(0x013992);

/// `lv_example_bar_orientation()`: an automatic horizontal and an explicit vertical bar.
pub fn lv_example_bar_orientation(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Horizontal bar (auto orientation from width > height) */
    let bar_1 = Bar::create(ui, screen);
    ui.obj_set_size(bar_1.obj(), coord::pct(90), 20);
    bar_1.set_min_value(ui, 0);
    bar_1.set_max_value(ui, 100);
    bar_1.set_value(ui, 60, ANIM_OFF);
    let label_1 = Label::create(ui, bar_1.obj());
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "Horizontal");
    ui.obj_set_style_text_color(label_1.obj(), TEXT_COLOR, sel);

    /* Explicit vertical orientation */
    let bar_2 = Bar::create(ui, screen);
    ui.obj_set_size(bar_2.obj(), 20, 130);
    bar_2.set_orientation(ui, BarOrientation::Vertical);
    bar_2.set_min_value(ui, 0);
    bar_2.set_max_value(ui, 100);
    bar_2.set_value(ui, 60, ANIM_OFF);
    let label_2 = Label::create(ui, bar_2.obj());
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "V\ne\nr");
    ui.obj_set_style_text_align(label_2.obj(), TextAlign::Center, sel);
    ui.obj_set_style_text_color(label_2.obj(), TEXT_COLOR, sel);
}
