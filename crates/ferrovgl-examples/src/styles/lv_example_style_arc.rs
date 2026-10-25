//! Port of `examples/styles/lv_example_style_arc.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Color, Part, Style, StyleSelector, Ui};
use ferrovgl_widgets::Arc;

use crate::util::{screen_active, styles_once};

/// `ACCENT`
const ACCENT: Color = Color::hex(0x6366f1);

/// `style_bg`, `style_indicator`.
struct Styles;

/// `lv_example_style_arc()`: track and indicator arcs styled independently.
pub fn lv_example_style_arc(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_bg, style_indicator] = styles_once::<Styles, 2>(ui, || {
        let mut style_bg = Style::new();
        style_bg.set_arc_color(ACCENT);
        style_bg.set_arc_width(14);
        style_bg.set_arc_opa((255 * 20 / 100) as u8);
        style_bg.set_arc_rounded(true);

        let mut style_indicator = Style::new();
        style_indicator.set_arc_color(ACCENT);
        style_indicator.set_arc_width(14);
        style_indicator.set_arc_rounded(true);
        [style_bg, style_indicator]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 12, sel);

    let arc = Arc::create(ui, screen);
    ui.obj_set_size(arc.obj(), 160, 160);
    arc.set_min_value(ui, 0);
    arc.set_max_value(ui, 100);
    arc.set_value(ui, 68);
    ui.obj_add_style(arc.obj(), style_bg, Part::MAIN.into());
    ui.obj_add_style(arc.obj(), style_indicator, Part::INDICATOR.into());
}
