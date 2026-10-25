//! Port of `examples/widgets/checkbox/lv_example_checkbox_text.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{StyleSelector, Ui};
use ferrovgl_widgets::Checkbox;

use crate::util::screen_active;

/// `lv_example_checkbox_text()`: short, long and multi-line checkbox texts.
pub fn lv_example_checkbox_text(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Short label, longer label, multi-line label */
    for text in [
        "Enable",
        "Send anonymous usage data",
        "I agree to the terms\nand the privacy policy",
    ] {
        let checkbox = Checkbox::create(ui, screen);
        checkbox.set_text(ui, text);
    }
}
