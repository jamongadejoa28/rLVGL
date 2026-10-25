//! Port of `examples/widgets/arc/lv_example_arc_styling.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Color, Part, Style, StyleSelector, Ui};
use ferrovgl_widgets::Arc;

use crate::util::{screen_active, styles_once};

/// `style_arc_bg`, `style_arc_indicator`, `style_arc_knob`.
struct Styles;

/// `lv_example_arc_styling()`: styled main, indicator and knob parts.
pub fn lv_example_arc_styling(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_arc_bg, style_arc_indicator, style_arc_knob] = styles_once::<Styles, 3>(ui, || {
        let mut style_arc_bg = Style::new();
        style_arc_bg.set_arc_color(Color::hex(0xced8e4));
        style_arc_bg.set_arc_width(22);
        style_arc_bg.set_arc_rounded(true);
        style_arc_bg.set_arc_opa(180);
        style_arc_bg.set_bg_color(Color::hex(0xffffff));
        style_arc_bg.set_bg_opa((255 * 100 / 100) as u8);
        style_arc_bg.set_pad_all(16);
        style_arc_bg.set_radius(16);
        style_arc_bg.set_shadow_color(Color::hex(0xcacaca));
        style_arc_bg.set_shadow_offset_y(10);
        style_arc_bg.set_shadow_width(21);

        let mut style_arc_indicator = Style::new();
        style_arc_indicator.set_arc_color(Color::hex(0x6366f1));
        style_arc_indicator.set_arc_width(14);
        style_arc_indicator.set_arc_rounded(true);
        style_arc_indicator.set_pad_all(4);

        let mut style_arc_knob = Style::new();
        style_arc_knob.set_bg_opa((255 * 100 / 100) as u8);
        style_arc_knob.set_bg_color(Color::hex(0x6366f1));
        style_arc_knob.set_border_color(Color::hex(0xffffff));
        style_arc_knob.set_border_width(3);
        style_arc_knob.set_pad_all(6);
        style_arc_knob.set_shadow_color(Color::hex(0x312e81));
        style_arc_knob.set_shadow_width(10);
        style_arc_knob.set_shadow_opa(160);
        [style_arc_bg, style_arc_indicator, style_arc_knob]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 20, sel);

    let arc = Arc::create(ui, screen);
    ui.obj_set_size(arc.obj(), 180, 180);
    arc.set_min_value(ui, 0);
    arc.set_max_value(ui, 100);
    arc.set_value(ui, 65);
    ui.obj_add_style(arc.obj(), style_arc_bg, Part::MAIN.into());
    ui.obj_add_style(arc.obj(), style_arc_indicator, Part::INDICATOR.into());
    ui.obj_add_style(arc.obj(), style_arc_knob, Part::KNOB.into());
}
