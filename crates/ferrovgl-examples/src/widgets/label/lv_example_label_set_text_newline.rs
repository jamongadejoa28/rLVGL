//! Port of `examples/widgets/label/lv_example_label_set_text_newline.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::screen_active;

/// `lv_example_label_set_text_newline()`: explicit newline characters in a label.
pub fn lv_example_label_set_text_newline(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Label with explicit newline characters */
    let label_1 = Label::create(ui, screen);
    label_1.set_text(ui, "First line\nSecond line\n\nFourth line");

    /* Plain label for comparison */
    let label_2 = Label::create(ui, screen);
    label_2.set_text(ui, "Just a normal single line text.");
}
