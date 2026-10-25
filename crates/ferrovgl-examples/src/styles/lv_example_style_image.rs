//! Port of `examples/styles/lv_example_style_image.c`.

use ferrovgl_core::misc::style::ImageSrc;
use ferrovgl_core::{Align, Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::Image;

use crate::assets::IMG_EXAMPLE_LVGL_LOGO;
use crate::util::{screen_active, styles_once};

/// `ACCENT`
const ACCENT: Color = Color::hex(0x2196f3);

/// `static lv_style_t style_img;`
struct StyleImg;

/// `lv_example_style_image()`: an image recolored and framed by a style.
pub fn lv_example_style_image(ui: &mut Ui) {
    let [style_img] = styles_once::<StyleImg, 1>(ui, || {
        let mut style_img = Style::new();
        style_img.set_radius(5);
        style_img.set_bg_opa((255 * 100 / 100) as u8);
        style_img.set_bg_color(Color::hex(0xf5f5f5));
        style_img.set_border_width(2);
        style_img.set_border_color(ACCENT);
        style_img.set_image_recolor(ACCENT);
        style_img.set_image_recolor_opa((255 * 50 / 100) as u8);
        [style_img]
    });

    let screen = screen_active(ui);

    let image = Image::create(ui, screen);
    image.set_src(ui, ImageSrc::Static(&IMG_EXAMPLE_LVGL_LOGO));
    ui.obj_set_align(image.obj(), Align::Center);
    ui.obj_add_style(image.obj(), style_img, StyleSelector::DEFAULT);
}
