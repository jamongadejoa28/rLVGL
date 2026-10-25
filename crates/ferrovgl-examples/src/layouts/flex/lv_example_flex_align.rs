//! Port of `examples/layouts/flex/lv_example_flex_align.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Align, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Checkbox, Label, Switch};

use crate::util::screen_active;

/// `lv_example_flex_align()`: items of different heights aligned on the cross axis.
pub fn lv_example_flex_align(ui: &mut Ui) {
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_pad_column(screen, 24, StyleSelector::DEFAULT);

    /* Tall item: a label with a large font */
    let button = Button::create(ui, screen).obj();
    ui.obj_set_height(button, 64);
    ui.obj_set_width(button, 64);
    let label = Label::create(ui, button);
    label.set_text(ui, "64px");
    ui.obj_set_align(label.obj(), Align::Center);

    /* Medium item: a toggle switch */
    Switch::create(ui, screen);

    /* Another medium item: a checkbox */
    let checkbox = Checkbox::create(ui, screen);
    checkbox.set_text(ui, "Check Me!");
}
