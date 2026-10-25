//! Port of `examples/styles/lv_example_style_local.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Align, Color, State, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::{screen_active, styles_once};

/// `ACCENT`
const ACCENT: Color = Color::hex(0x6366f1);

/// `static lv_style_t style_shared;`
struct StyleShared;

/// `lv_example_style_local()`: local style properties overriding a shared style.
pub fn lv_example_style_local(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_shared] = styles_once::<StyleShared, 1>(ui, || {
        let mut style_shared = Style::new();
        style_shared.set_radius(14);
        style_shared.set_bg_opa((255 * 100 / 100) as u8);
        style_shared.set_bg_color(ACCENT);
        style_shared.set_border_width(3);
        style_shared.set_border_color(Color::hex(0x312e81));
        style_shared.set_pad_all(14);
        style_shared.set_text_color(Color::hex(0xffffff));
        [style_shared]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    let button_1 = Button::create(ui, screen).obj();
    ui.obj_set_width(button_1, 95);
    ui.obj_add_style(button_1, style_shared, sel);
    let label_1 = Label::create(ui, button_1);
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "Shared");

    /* Plain local override: fill always wins over the shared style */
    let button_2 = Button::create(ui, screen).obj();
    ui.obj_set_width(button_2, 95);
    ui.obj_set_style_bg_color(button_2, Color::hex(0xec4899), sel);
    ui.obj_add_style(button_2, style_shared, sel);
    let label_2 = Label::create(ui, button_2);
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "Override");

    /* Local state selector: overrides the fill only while pressed */
    let button_3 = Button::create(ui, screen).obj();
    ui.obj_set_width(button_3, 95);
    ui.obj_set_style_bg_color(button_3, Color::hex(0x10b981), State::PRESSED.into());
    ui.obj_add_style(button_3, style_shared, sel);
    let label_3 = Label::create(ui, button_3);
    ui.obj_set_align(label_3.obj(), Align::Center);
    label_3.set_text(ui, "On press");
}
