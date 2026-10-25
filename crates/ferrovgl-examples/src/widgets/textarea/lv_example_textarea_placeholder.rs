//! Port of `examples/widgets/textarea/lv_example_textarea_placeholder.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{StyleSelector, Ui};
use ferrovgl_widgets::Textarea;

use crate::util::screen_active;

/// `lv_example_textarea_placeholder()`: an empty and a filled text area with placeholders.
pub fn lv_example_textarea_placeholder(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Empty: placeholder hint is shown */
    let textarea_1 = Textarea::create(ui, screen);
    ui.obj_set_width(textarea_1.obj(), coord::pct(60));
    textarea_1.set_one_line(ui, true);
    textarea_1.set_placeholder_text(ui, Some("Search..."));

    /* Filled: placeholder is hidden */
    let textarea_2 = Textarea::create(ui, screen);
    ui.obj_set_width(textarea_2.obj(), coord::pct(60));
    textarea_2.set_one_line(ui, true);
    textarea_2.set_placeholder_text(ui, Some("Search\u{2026}"));
    textarea_2.set_text(ui, "Hello world!");
}
