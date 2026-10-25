//! Port of `examples/widgets/image/lv_example_image_transformations.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::style::ImageSrc;
use ferrovgl_core::{StyleSelector, Ui};
use ferrovgl_widgets::Image;

use crate::assets::IMG_EXAMPLE_LVGL_LOGO;
use crate::util::screen_active;

/// `lv_example_image_transformations()`: an unmodified, a rotated and a scaled image.
pub fn lv_example_image_transformations(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::SpaceEvenly, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    let image_1 = Image::create(ui, screen);
    image_1.set_src(ui, ImageSrc::Static(&IMG_EXAMPLE_LVGL_LOGO));

    let image_2 = Image::create(ui, screen);
    image_2.set_src(ui, ImageSrc::Static(&IMG_EXAMPLE_LVGL_LOGO));
    image_2.set_rotation(ui, 450);

    let image_3 = Image::create(ui, screen);
    image_3.set_src(ui, ImageSrc::Static(&IMG_EXAMPLE_LVGL_LOGO));
    image_3.set_scale_x(ui, 160);
    image_3.set_scale_y(ui, 160);
}
