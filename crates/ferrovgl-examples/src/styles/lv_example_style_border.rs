//! Port of `examples/styles/lv_example_style_border.c`.

use ferrovgl_core::{Align, Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `ACCENT`
const ACCENT: Color = Color::hex(0x6366f1);

/// `static lv_style_t style_framed;`
struct StyleFramed;

/// `lv_example_style_border()`: a semi-transparent border around a card.
pub fn lv_example_style_border(ui: &mut Ui) {
    let [style_framed] = styles_once::<StyleFramed, 1>(ui, || {
        let mut style_framed = Style::new();
        style_framed.set_radius(20);
        style_framed.set_bg_opa((255 * 100 / 100) as u8);
        style_framed.set_bg_color(Color::hex(0xffffff));
        style_framed.set_border_color(ACCENT);
        style_framed.set_border_width(4);
        style_framed.set_border_opa((255 * 60 / 100) as u8);
        [style_framed]
    });

    let screen = screen_active(ui);

    let container = ui.obj_create(Some(screen));
    ui.obj_set_size(container, 210, 140);
    ui.obj_set_align(container, Align::Center);
    ui.obj_add_style(container, style_framed, StyleSelector::DEFAULT);
    let label = Label::create(ui, container);
    ui.obj_set_align(label.obj(), Align::Center);
    label.set_text(ui, "Bordered card");
}
