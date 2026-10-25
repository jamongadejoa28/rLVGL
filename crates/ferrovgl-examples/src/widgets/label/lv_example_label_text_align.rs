//! Port of `examples/widgets/label/lv_example_label_text_align.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::text::TextAlign;
use ferrovgl_core::{StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::screen_active;

/// `lv_example_label_text_align()`: left, center and right aligned text.
pub fn lv_example_label_text_align(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    for (align, text) in [
        /* Left-aligned multi-line label */
        (TextAlign::Left, "Left aligned\nSecond line"),
        /* Center-aligned multi-line label */
        (TextAlign::Center, "Centered\nSecond line"),
        /* Right-aligned multi-line label */
        (TextAlign::Right, "Right aligned\nSecond line"),
    ] {
        let label = Label::create(ui, screen);
        ui.obj_set_width(label.obj(), 220);
        ui.obj_set_style_text_align(label.obj(), align, sel);
        label.set_text(ui, text);
        ui.obj_set_style_border_width(label.obj(), 1, sel);
    }
}
