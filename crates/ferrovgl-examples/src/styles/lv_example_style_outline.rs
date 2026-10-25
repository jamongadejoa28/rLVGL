//! Port of `examples/styles/lv_example_style_outline.c`.

use ferrovgl_core::{Align, Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `ACCENT`
const ACCENT: Color = Color::hex(0x6366f1);

/// `static lv_style_t style_focus;`
struct StyleFocus;

/// `lv_example_style_outline()`: an outline ring drawn outside the card.
pub fn lv_example_style_outline(ui: &mut Ui) {
    let [style_focus] = styles_once::<StyleFocus, 1>(ui, || {
        let mut style_focus = Style::new();
        style_focus.set_radius(14);
        style_focus.set_bg_opa((255 * 100 / 100) as u8);
        style_focus.set_bg_color(Color::hex(0xffffff));
        style_focus.set_border_width(1);
        style_focus.set_border_color(Color::hex(0xe2e8f0));
        style_focus.set_outline_color(ACCENT);
        style_focus.set_outline_width(3);
        style_focus.set_outline_pad(6);
        [style_focus]
    });

    let screen = screen_active(ui);

    let container = ui.obj_create(Some(screen));
    ui.obj_set_align(container, Align::Center);
    ui.obj_set_size(container, 200, 120);
    ui.obj_add_style(container, style_focus, StyleSelector::DEFAULT);
    let label = Label::create(ui, container);
    ui.obj_set_align(label.obj(), Align::Center);
    label.set_text(ui, "Focused");
}
