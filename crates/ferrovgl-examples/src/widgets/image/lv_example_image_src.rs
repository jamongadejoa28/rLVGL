//! Port of `examples/widgets/image/lv_example_image_src.c`.

use ferrovgl_core::misc::style::ImageSrc;
use ferrovgl_core::{Align, Ui};
use ferrovgl_widgets::Image;

use crate::assets::IMG_EXAMPLE_LVGL_LOGO;
use crate::util::screen_active;

/// `lv_example_image_src()`: an image from a C array.
pub fn lv_example_image_src(ui: &mut Ui) {
    let screen = screen_active(ui);

    let image = Image::create(ui, screen);
    image.set_src(ui, ImageSrc::Static(&IMG_EXAMPLE_LVGL_LOGO));
    ui.obj_set_align(image.obj(), Align::Center);
}
