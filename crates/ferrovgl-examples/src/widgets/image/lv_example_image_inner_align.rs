//! Port of `examples/widgets/image/lv_example_image_inner_align.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::style::ImageSrc;
use ferrovgl_core::{Color, Style, StyleSelector, Ui};
use ferrovgl_widgets::{Image, ImageAlign};

use crate::assets::IMG_EXAMPLE_LVGL_LOGO;
use crate::util::{screen_active, styles_once};

/// `static lv_style_t style_main;`
struct StyleMain;

/// `lv_example_image_inner_align()`: the image placed inside a larger widget.
pub fn lv_example_image_inner_align(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [style_main] = styles_once::<StyleMain, 1>(ui, || {
        let mut style_main = Style::new();
        style_main.set_bg_color(Color::hex(0xd5d5d5));
        style_main.set_bg_opa((255 * 100 / 100) as u8);
        style_main.set_border_width(2);
        style_main.set_border_color(Color::hex(0x9429ff));
        [style_main]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::SpaceEvenly, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_column(screen, 16, sel);

    for align in [ImageAlign::TopLeft, ImageAlign::Center, ImageAlign::BottomRight] {
        let image = Image::create(ui, screen);
        image.set_src(ui, ImageSrc::Static(&IMG_EXAMPLE_LVGL_LOGO));
        ui.obj_set_size(image.obj(), 80, 80);
        image.set_inner_align(ui, align);
        ui.obj_add_style(image.obj(), style_main, sel);
    }
}
