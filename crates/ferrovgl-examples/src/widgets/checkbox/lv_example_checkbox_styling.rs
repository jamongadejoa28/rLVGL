//! Port of `examples/widgets/checkbox/lv_example_checkbox_styling.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Color, Part, State, Style, StyleSelector, Ui};
use ferrovgl_widgets::Checkbox;

use crate::util::{screen_active, styles_once};

/// `style_cb_indicator`, `style_cb_indicator_checked`.
struct Styles;

/// `lv_example_checkbox_styling()`: styled indicator with a checked state.
pub fn lv_example_checkbox_styling(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_cb_indicator, style_cb_indicator_checked] = styles_once::<Styles, 2>(ui, || {
        let mut style_cb_indicator = Style::new();
        style_cb_indicator.set_bg_opa((255 * 100 / 100) as u8);
        style_cb_indicator.set_bg_color(Color::hex(0xf9fafb));
        style_cb_indicator.set_radius(6);
        style_cb_indicator.set_border_color(Color::hex(0x9ca3af));
        style_cb_indicator.set_border_width(2);
        style_cb_indicator.set_pad_all(4);

        let mut style_cb_indicator_checked = Style::new();
        style_cb_indicator_checked.set_bg_color(Color::hex(0x16a34a));
        style_cb_indicator_checked.set_border_color(Color::hex(0x14532d));
        style_cb_indicator_checked.set_outline_color(Color::hex(0x16a34a));
        style_cb_indicator_checked.set_outline_width(4);
        style_cb_indicator_checked.set_outline_pad(2);
        style_cb_indicator_checked.set_outline_opa(80);
        [style_cb_indicator, style_cb_indicator_checked]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 14, sel);

    /* Unchecked: only the default indicator style applies */
    let checkbox_1 = Checkbox::create(ui, screen);
    checkbox_1.set_text(ui, "Default look");
    ui.obj_set_style_text_color(checkbox_1.obj(), Color::hex(0x111827), sel);
    ui.obj_set_style_pad_column(checkbox_1.obj(), 10, sel);
    ui.obj_add_style(checkbox_1.obj(), style_cb_indicator, Part::INDICATOR.into());
    ui.obj_add_style(
        checkbox_1.obj(),
        style_cb_indicator_checked,
        Part::INDICATOR | State::CHECKED,
    );

    /* Pre-checked so the checked-state style is visible at load time */
    let checkbox_2 = Checkbox::create(ui, screen);
    checkbox_2.set_text(ui, "Pre-checked");
    ui.obj_set_state(checkbox_2.obj(), State::CHECKED, true);
    ui.obj_set_style_text_color(checkbox_2.obj(), Color::hex(0x111827), sel);
    ui.obj_set_style_pad_column(checkbox_2.obj(), 10, sel);
    ui.obj_add_style(checkbox_2.obj(), style_cb_indicator, Part::INDICATOR.into());
    ui.obj_add_style(
        checkbox_2.obj(),
        style_cb_indicator_checked,
        Part::INDICATOR | State::CHECKED,
    );
}
