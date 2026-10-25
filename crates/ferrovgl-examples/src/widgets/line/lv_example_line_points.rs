//! Port of `examples/widgets/line/lv_example_line_points.c`.

use ferrovgl_core::misc::area::PointPrecise;
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::style::SharedRef;
use ferrovgl_core::{Style, StyleSelector, Ui};
use ferrovgl_widgets::Line;

use crate::util::{pp, screen_active, styles_reinit};

/// `static lv_point_precise_t line_points[]`: the points of the line.
static LINE_POINTS: [PointPrecise; 5] = [pp(5, 5), pp(70, 70), pp(120, 10), pp(180, 60), pp(240, 10)];

/// `static lv_style_t style_line;`
struct StyleLine;

/// `lv_example_line_points()`: a rounded polyline.
pub fn lv_example_line_points(ui: &mut Ui) {
    /*Create style*/
    let [style_line] = styles_reinit::<StyleLine, 1>(ui, || {
        let mut style_line = Style::new();
        style_line.set_line_width(8);
        style_line.set_line_color(palette::main(Palette::Blue));
        style_line.set_line_rounded(true);
        [style_line]
    });

    /*Create a line and apply the new style*/
    let line1 = Line::create(ui, screen_active(ui));
    line1.set_points(ui, Some(SharedRef::Static(&LINE_POINTS)), 5); /*Set the points*/
    ui.obj_add_style(line1.obj(), style_line, StyleSelector::DEFAULT);
    ui.obj_center(line1.obj());
}
