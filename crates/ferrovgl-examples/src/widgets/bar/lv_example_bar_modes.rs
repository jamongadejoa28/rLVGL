//! Port of `examples/widgets/bar/lv_example_bar_modes.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::anim::ANIM_OFF;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, StyleSelector, Ui};
use ferrovgl_widgets::{Bar, BarMode, Label};

use crate::util::screen_active;

/// `TEXT_COLOR`
const TEXT_COLOR: Color = Color::hex(0x013992);

/// `lv_example_bar_modes()`: normal, symmetrical and range bars.
pub fn lv_example_bar_modes(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Normal mode */
    let bar_1 = Bar::create(ui, screen);
    ui.obj_set_size(bar_1.obj(), coord::pct(90), 20);
    bar_1.set_mode(ui, BarMode::Normal);
    bar_1.set_min_value(ui, 0);
    bar_1.set_max_value(ui, 100);
    bar_1.set_value(ui, 70, ANIM_OFF);
    let label_1 = Label::create(ui, bar_1.obj());
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "normal");
    ui.obj_set_style_text_color(label_1.obj(), TEXT_COLOR, sel);

    /* Symmetrical mode around zero */
    let bar_2 = Bar::create(ui, screen);
    ui.obj_set_size(bar_2.obj(), coord::pct(90), 20);
    bar_2.set_mode(ui, BarMode::Symmetrical);
    bar_2.set_min_value(ui, -100);
    bar_2.set_max_value(ui, 100);
    bar_2.set_value(ui, 45, ANIM_OFF);
    let label_2 = Label::create(ui, bar_2.obj());
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "symmetrical");
    ui.obj_set_style_text_color(label_2.obj(), TEXT_COLOR, sel);

    /* Range mode with start and end values */
    let bar_3 = Bar::create(ui, screen);
    ui.obj_set_size(bar_3.obj(), coord::pct(90), 20);
    bar_3.set_mode(ui, BarMode::Range);
    bar_3.set_min_value(ui, 0);
    bar_3.set_max_value(ui, 100);
    bar_3.set_start_value(ui, 25, ANIM_OFF);
    bar_3.set_value(ui, 80, ANIM_OFF);
    let label_3 = Label::create(ui, bar_3.obj());
    ui.obj_set_align(label_3.obj(), Align::Center);
    label_3.set_text(ui, "range");
    ui.obj_set_style_text_color(label_3.obj(), TEXT_COLOR, sel);
}
