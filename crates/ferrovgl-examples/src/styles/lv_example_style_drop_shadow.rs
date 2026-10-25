//! Port of `examples/styles/lv_example_style_drop_shadow.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::text::TextAlign;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Part, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Arc, Label};

use crate::util::{screen_active, styles_reinit};

/// `static lv_style_t style_drop;`
struct StyleDrop;

/// `lv_example_style_drop_shadow()`: a drop shadow following the arc's curve.
pub fn lv_example_style_drop_shadow(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_drop] = styles_reinit::<StyleDrop, 1>(ui, || {
        let mut style_drop = Style::new();
        style_drop.set_drop_shadow_color(Color::hex(0xf44336));
        style_drop.set_drop_shadow_radius(16);
        style_drop.set_drop_shadow_opa(255);
        style_drop.set_drop_shadow_offset_x(5);
        style_drop.set_drop_shadow_offset_y(10);
        [style_drop]
    });

    let scr = screen_active(ui);
    ui.obj_set_flex_flow(scr, FlexFlow::Column);
    ui.obj_set_style_flex_cross_place(scr, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(scr, 16, sel);

    let title = Label::create(ui, scr);
    ui.obj_set_width(title.obj(), coord::pct(100));
    ui.obj_set_style_text_align(title.obj(), TextAlign::Center, sel);
    title.set_text(ui, "Style: drop shadow");

    let arc = Arc::create(ui, scr);
    ui.obj_set_size(arc.obj(), 150, 150);
    arc.set_value(ui, 60);
    ui.obj_add_style(arc.obj(), style_drop, Part::INDICATOR.into());
}
