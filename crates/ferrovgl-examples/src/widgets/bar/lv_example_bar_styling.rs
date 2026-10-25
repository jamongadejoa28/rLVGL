//! Port of `examples/widgets/bar/lv_example_bar_styling.c`.

use ferrovgl_core::draw::grad::GradDir;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::anim::ANIM_OFF;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Part, Style, StyleSelector, Ui};
use ferrovgl_widgets::Bar;

use crate::util::{screen_active, styles_once};

/// `style_bar_track`, `style_bar_indicator`.
struct Styles;

/// `lv_example_bar_styling()`: named styles for both parts and local overrides.
pub fn lv_example_bar_styling(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_bar_track, style_bar_indicator] = styles_once::<Styles, 2>(ui, || {
        let mut style_bar_track = Style::new();
        style_bar_track.set_bg_opa((255 * 100 / 100) as u8);
        style_bar_track.set_bg_color(Color::hex(0x1f2937));
        style_bar_track.set_radius(12);
        style_bar_track.set_border_color(Color::hex(0x0f172a));
        style_bar_track.set_border_width(1);

        let mut style_bar_indicator = Style::new();
        style_bar_indicator.set_bg_opa((255 * 100 / 100) as u8);
        style_bar_indicator.set_bg_color(Color::hex(0x10b981));
        style_bar_indicator.set_bg_grad_color(Color::hex(0x06b6d4));
        style_bar_indicator.set_bg_grad_dir(GradDir::Hor);
        style_bar_indicator.set_radius(12);
        style_bar_indicator.set_shadow_color(Color::hex(0x064e3b));
        style_bar_indicator.set_shadow_width(8);
        style_bar_indicator.set_shadow_spread(-2);
        style_bar_indicator.set_shadow_opa(180);
        [style_bar_track, style_bar_indicator]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 20, sel);

    /* Tall pill-shaped bar with named styles for both parts */
    let bar_1 = Bar::create(ui, screen);
    ui.obj_set_size(bar_1.obj(), coord::pct(90), 22);
    bar_1.set_min_value(ui, 0);
    bar_1.set_max_value(ui, 100);
    bar_1.set_value(ui, 65, ANIM_OFF);
    ui.obj_add_style(bar_1.obj(), style_bar_track, Part::MAIN.into());
    ui.obj_add_style(bar_1.obj(), style_bar_indicator, Part::INDICATOR.into());

    /* Same widget, local style_* overrides for a flat alternative look */
    let bar_2 = Bar::create(ui, screen);
    ui.obj_set_size(bar_2.obj(), coord::pct(90), 10);
    bar_2.set_min_value(ui, 0);
    bar_2.set_max_value(ui, 100);
    bar_2.set_value(ui, 35, ANIM_OFF);
    ui.obj_set_style_bg_color(bar_2.obj(), Color::hex(0xe5e7eb), sel);
    ui.obj_set_style_bg_opa(bar_2.obj(), (255 * 100 / 100) as u8, sel);
    ui.obj_set_style_radius(bar_2.obj(), 2, sel);
    ui.obj_set_style_border_color(bar_2.obj(), Color::hex(0xd1d5db), sel);
    ui.obj_set_style_border_width(bar_2.obj(), 1, sel);
}
