//! Port of `examples/layouts/flex/lv_example_flex_new_track.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_day;`
struct StyleDay;

/// `lv_example_flex_new_track()`: force an item onto a new flex track.
pub fn lv_example_flex_new_track(ui: &mut Ui) {
    let [style_day] = styles_once::<StyleDay, 1>(ui, || {
        let mut style_day = Style::new();
        style_day.set_bg_opa((255 * 100 / 100) as u8);
        style_day.set_text_color(Color::hex(0xffffff));
        style_day.set_pad_all(6);
        [style_day]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);

    /* Demo container showing explicit new-track start */
    let container = ui.obj_create(Some(screen));
    ui.obj_set_flex_flow(container, FlexFlow::RowWrap);
    ui.obj_set_size(container, coord::pct(90), coord::SIZE_CONTENT);

    /* Track 1 */
    for text in ["Mon", "Tue", "Wed", "Thu", "Fri"] {
        let label = Label::create(ui, container);
        ui.obj_set_style_bg_color(label.obj(), Color::hex(0x3498db), StyleSelector::DEFAULT);
        label.set_text(ui, text);
        ui.obj_add_style(label.obj(), style_day, StyleSelector::DEFAULT);
    }

    /* Track 2 — forced onto a new track */
    let label_6 = Label::create(ui, container);
    ui.obj_set_flex_in_new_track(label_6.obj(), true);
    ui.obj_set_style_bg_color(label_6.obj(), Color::hex(0xe74c3c), StyleSelector::DEFAULT);
    label_6.set_text(ui, "Sat");
    ui.obj_add_style(label_6.obj(), style_day, StyleSelector::DEFAULT);

    let label_7 = Label::create(ui, container);
    ui.obj_set_style_bg_color(label_7.obj(), Color::hex(0xe74c3c), StyleSelector::DEFAULT);
    label_7.set_text(ui, "Sun");
    ui.obj_add_style(label_7.obj(), style_day, StyleSelector::DEFAULT);
}
