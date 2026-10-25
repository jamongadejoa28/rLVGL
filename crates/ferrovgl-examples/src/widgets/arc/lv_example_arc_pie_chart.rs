//! Port of `examples/widgets/arc/lv_example_arc_pie_chart.c`.

use ferrovgl_core::misc::types::ValuePrecise;
use ferrovgl_core::{Align, Color, Part, Style, Ui};
use ferrovgl_widgets::Arc;

use crate::util::{screen_active, styles_once};

/// `style_pie_main`, `style_pie_indicator`, `style_pie_knob`.
struct Styles;

/// `lv_example_arc_pie_chart()`: a pie chart built from five background arcs.
pub fn lv_example_arc_pie_chart(ui: &mut Ui) {
    let [style_pie_main, style_pie_indicator, style_pie_knob] = styles_once::<Styles, 3>(ui, || {
        let mut style_pie_main = Style::new();
        style_pie_main.set_width(200);
        style_pie_main.set_height(200);
        style_pie_main.set_align(Align::Center);
        style_pie_main.set_arc_width(100);
        style_pie_main.set_arc_rounded(false);

        let mut style_pie_indicator = Style::new();
        style_pie_indicator.set_arc_opa(0);

        let mut style_pie_knob = Style::new();
        style_pie_knob.set_bg_opa(0);
        [style_pie_main, style_pie_indicator, style_pie_knob]
    });

    let screen = screen_active(ui);

    /* 5 slices with uneven sweeps so the colours look like a real data breakdown.
    Angles measured clockwise from 3 o'clock; the seam between E and A is at 12 o'clock (270°). */
    for (start, end, color) in [
        (270, 325, 0xef4444),
        (325, 38, 0xf59e0b),
        (38, 162, 0x22c55e),
        (162, 190, 0x3b82f6),
        (190, 270, 0xa855f7),
    ] {
        let arc = Arc::create(ui, screen);
        ui.obj_set_clickable(arc.obj(), false);
        arc.set_bg_start_angle(ui, start as ValuePrecise);
        arc.set_bg_end_angle(ui, end as ValuePrecise);
        ui.obj_set_style_arc_color(arc.obj(), Color::hex(color), Part::MAIN.into());
        ui.obj_add_style(arc.obj(), style_pie_main, Part::MAIN.into());
        ui.obj_add_style(arc.obj(), style_pie_indicator, Part::INDICATOR.into());
        ui.obj_add_style(arc.obj(), style_pie_knob, Part::KNOB.into());
    }
}
