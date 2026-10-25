//! `test_draw_blur.c`: identical offscreen draw commands and screen flex layout.
//! Canvas widgets are represented by 140x100 image-drawing objects whose
//! buffers are filled like `lv_canvas_fill_bg()` and drawn into like
//! `lv_canvas_init_layer()` / `lv_canvas_finish_layer()`; this tests the
//! rasterizer and compositing, not the unported canvas widget API.

mod common;

use std::rc::Rc;

use common::{canvas_buf, canvas_fill_bg, canvas_finish_layer, canvas_init_layer};
use ferrovgl_core::draw::blur::BlurDsc;
use ferrovgl_core::draw::draw_buf::DrawBuf;
use ferrovgl_core::draw::image::DrawImageDsc;
use ferrovgl_core::draw::{FillDsc, LabelDsc};
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::color::{OPA_30, OPA_COVER};
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::style::ImageSrc;
use ferrovgl_core::misc::text::{TextAlign, TextDecor};
use ferrovgl_core::{Area, Color, ColorFormat, EventCode, FontRef, ObjFlags, Ui};
use ferrovgl_test::TestDisplay;

fn render_canvas(ui: &mut Ui, name: &str, cf: ColorFormat, radius: i32, corner: i32) -> DrawBuf {
    /*lv_canvas_set_buffer(canvas, canvas_buf, CANVAS_WIDTH, CANVAS_HEIGHT, cf)*/
    let mut buf = canvas_buf(140, 100, cf);
    canvas_fill_bg(&mut buf, Color::hex3(0xccc), OPA_COVER);
    let layer = canvas_init_layer(ui, buf);

    let mut label = LabelDsc::new();
    label.color = palette::main(Palette::Red);
    label.font = Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14));
    label.decor = TextDecor::UNDERLINE;
    label.align = TextAlign::Center;
    label.text = Rc::from(concat!(
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ",
        "Curabitur sed velit sed neque tristique sagittis vel et sapien."
    ));
    ui.draw_label(layer, &label, &Area::new(10, 10, 130, 90));

    let mut blur = BlurDsc::new();
    blur.blur_radius = radius;
    blur.corner_radius = corner;
    let middle = Area::new(25, 20, 115, 80);
    ui.draw_blur(layer, &blur, &middle);
    let mut fill = FillDsc::new();
    fill.color = palette::lighten(Palette::Blue, 1);
    fill.radius = corner;
    fill.opa = OPA_30;
    ui.draw_fill(layer, &fill, &middle);

    let mut label = LabelDsc::new();
    label.color = Color::BLACK;
    label.font = Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14));
    label.align = TextAlign::Center;
    label.text = Rc::from(format!("{name}\nblur:{radius}"));
    ui.draw_label(layer, &label, &Area::new(30, 35, 110, 65));
    canvas_finish_layer(ui, layer)
}

fn scene(corner: i32) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    display.ui.obj_set_flex_flow(screen, FlexFlow::RowWrap);
    display.ui.obj_set_flex_align(
        screen,
        FlexAlign::SpaceEvenly,
        FlexAlign::Center,
        FlexAlign::SpaceEvenly,
    );
    for radius in [6, 12, 24, 60] {
        for (name, cf) in [
            ("l8", ColorFormat::L8),
            ("rgb565", ColorFormat::Rgb565),
            ("rgb888", ColorFormat::Rgb888),
            ("xrgb8888", ColorFormat::Xrgb8888),
            ("argb8888", ColorFormat::Argb8888),
        ] {
            let pixels = Rc::new(render_canvas(&mut display.ui, name, cf, radius, corner));
            let canvas = display.ui.obj_create(Some(screen));
            display.ui.obj_remove_style_all(canvas);
            display.ui.obj_remove_flag(canvas, ObjFlags::SCROLLABLE);
            display.ui.obj_set_size(canvas, 140, 100);
            display.ui.obj_add_event_cb(
                canvas,
                Rc::new(move |ui, event| {
                    let layer = event.layer().unwrap();
                    let mut image = DrawImageDsc::new();
                    image.src = ImageSrc::DrawBuf(pixels.clone());
                    ui.draw_image(layer, &image, &ui.obj_get_coords(canvas));
                }),
                EventCode::DRAW_MAIN,
                None,
            );
        }
    }
    display
        .screenshot_compare(format!("draw/draw_blur_corner_{corner}.png"))
        .unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn blur_square_corners() {
    scene(0);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn blur_radius_4_corners() {
    scene(4);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn blur_radius_16_corners() {
    scene(16);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn blur_radius_32_corners() {
    scene(32);
}
