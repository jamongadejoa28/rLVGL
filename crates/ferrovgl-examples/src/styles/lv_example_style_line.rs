//! Port of `examples/styles/lv_example_style_line.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::area::PointPrecise;
use ferrovgl_core::misc::style::SharedRef;
use ferrovgl_core::misc::text::TextAlign;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Label, Line};

use crate::util::{pp, screen_active, styles_reinit};

/// `static lv_style_t style_stroke;`
struct StyleStroke;

/// `static lv_point_precise_t pts[]`
static PTS: [PointPrecise; 4] = [pp(10, 30), pp(60, 90), pp(130, 20), pp(200, 70)];

/// `lv_example_style_line()`: a rounded line stroke.
pub fn lv_example_style_line(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_stroke] = styles_reinit::<StyleStroke, 1>(ui, || {
        let mut style_stroke = Style::new();
        style_stroke.set_line_color(Color::hex(0x9e9e9e));
        style_stroke.set_line_width(6);
        style_stroke.set_line_rounded(true);
        [style_stroke]
    });

    let scr = screen_active(ui);
    ui.obj_set_flex_flow(scr, FlexFlow::Column);
    ui.obj_set_style_flex_cross_place(scr, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(scr, 16, sel);

    let title = Label::create(ui, scr);
    ui.obj_set_width(title.obj(), coord::pct(100));
    ui.obj_set_style_text_align(title.obj(), TextAlign::Center, sel);
    title.set_text(ui, "Style: line stroke");

    let line = Line::create(ui, scr);
    line.set_points(ui, Some(SharedRef::Static(&PTS)), PTS.len() as u32);
    ui.obj_add_style(line.obj(), style_stroke, sel);
}
