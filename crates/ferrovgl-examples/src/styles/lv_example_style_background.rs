//! Port of `examples/styles/lv_example_style_background.c`.

use ferrovgl_core::draw::grad::GradDir;
use ferrovgl_core::{Align, Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Label;

use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_grad;`
struct StyleGrad;

/// `lv_example_style_background()`: a vertical gradient background with a shadow.
pub fn lv_example_style_background(ui: &mut Ui) {
    let [style_grad] = styles_once::<StyleGrad, 1>(ui, || {
        let mut style_grad = Style::new();
        style_grad.set_radius(20);
        style_grad.set_bg_opa((255 * 100 / 100) as u8);
        style_grad.set_bg_color(Color::hex(0x6366f1));
        style_grad.set_bg_grad_color(Color::hex(0xec4899));
        style_grad.set_bg_grad_dir(GradDir::Ver);
        style_grad.set_bg_main_stop(80);
        style_grad.set_bg_grad_stop(220);
        style_grad.set_shadow_color(Color::hex(0x6366f1));
        style_grad.set_shadow_width(26);
        style_grad.set_shadow_offset_y(10);
        style_grad.set_shadow_opa(70);
        style_grad.set_text_color(Color::hex(0xffffff));
        [style_grad]
    });

    let screen = screen_active(ui);

    let container = ui.obj_create(Some(screen));
    ui.obj_set_size(container, 210, 150);
    ui.obj_set_align(container, Align::Center);
    ui.obj_add_style(container, style_grad, StyleSelector::DEFAULT);
    let label = Label::create(ui, container);
    ui.obj_set_align(label.obj(), Align::Center);
    label.set_text(ui, "Gradient");
}
