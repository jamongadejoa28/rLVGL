//! Draw-command equivalents of `test_canvas.c` at the pinned LVGL commit.
//! Offscreen buffers and image callbacks exercise primitives and nested layers;
//! these tests do not claim coverage of the unported canvas widget API.

use std::rc::Rc;

use ferrovgl_core::draw::draw_buf::DrawBuf;
use ferrovgl_core::draw::image::DrawImageDsc;
use ferrovgl_core::draw::triangle::TriangleDsc;
use ferrovgl_core::draw::{FillDsc, LayerId, RectDsc};
use ferrovgl_core::misc::area::PointPrecise;
use ferrovgl_core::misc::style::ImageSrc;
use ferrovgl_core::{Area, Color, ColorFormat, EventCode, ObjFlags, Ui};
use ferrovgl_test::TestDisplay;

fn finish(ui: &mut Ui, layer: LayerId) -> DrawBuf {
    for _ in 0..32 {
        if ui.draw.layer(layer).unwrap().tasks.is_empty() {
            break;
        }
        ui.draw_dispatch();
    }
    assert!(
        ui.draw.layer(layer).unwrap().tasks.is_empty(),
        "draw queue stalled"
    );
    let pixels = ui
        .draw
        .layer(layer)
        .unwrap()
        .draw_buf
        .as_ref()
        .unwrap()
        .dup()
        .unwrap();
    ui.draw_layer_delete(layer);
    pixels
}

fn show_canvas(display: &mut TestDisplay, pixels: DrawBuf, centered: bool) {
    let screen = display.screen().unwrap();
    let canvas = display.ui.obj_create(Some(screen));
    display.ui.obj_remove_style_all(canvas);
    display.ui.obj_remove_flag(canvas, ObjFlags::SCROLLABLE);
    display
        .ui
        .obj_set_size(canvas, pixels.header.w.into(), pixels.header.h.into());
    if centered {
        display.ui.obj_center(canvas);
    }
    let pixels = Rc::new(pixels);
    display.ui.obj_add_event_cb(
        canvas,
        Rc::new(move |ui, event| {
            let mut image = DrawImageDsc::new();
            image.src = ImageSrc::DrawBuf(pixels.clone());
            ui.draw_image(event.layer().unwrap(), &image, &ui.obj_get_coords(canvas));
        }),
        EventCode::DRAW_MAIN,
        None,
    );
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn twelve_triangle_orientations() {
    // Exact geometry/color from test_canvas_triangles; 500x500 RGB888 buffer
    // is centered on the 800x480 display, clipping ten rows at each end.
    let mut display = TestDisplay::with_default_theme().unwrap();
    let area = Area::new(0, 0, 499, 499);
    let layer = display.ui.draw_layer_create(None, ColorFormat::Rgb888, &area);
    let mut fill = FillDsc::new();
    fill.color = Color::hex(0xafafaf);
    display.ui.draw_fill(layer, &fill, &area);
    for (color, points) in [
        (0xff0000, [(50, 50), (50, 100), (100, 100)]),
        (0x00ff00, [(150, 50), (200, 50), (200, 100)]),
        (0x0000ff, [(250, 50), (230, 100), (270, 100)]),
        (0xffff00, [(350, 50), (390, 50), (370, 100)]),
        (0xff00ff, [(50, 150), (100, 130), (100, 170)]),
        (0x00ffff, [(150, 130), (150, 170), (200, 150)]),
        (0x800080, [(250, 150), (300, 140), (280, 180)]),
        (0xffa500, [(350, 150), (330, 180), (370, 180)]),
        (0x008000, [(100, 250), (50, 280), (150, 280)]),
        (0x000080, [(250, 230), (240, 290), (260, 290)]),
        (0x800000, [(350, 230), (350, 280), (390, 290)]),
        (0x808000, [(100, 350), (100, 400), (150, 320)]),
    ] {
        let mut dsc = TriangleDsc::new();
        dsc.color = Color::hex(color);
        dsc.p = points.map(|(x, y)| PointPrecise::new(x as _, y as _));
        display.ui.draw_triangle(layer, &dsc);
    }
    let pixels = finish(&mut display.ui, layer);
    show_canvas(&mut display, pixels, true);
    display.screenshot_compare("widgets/canvas_2.png").unwrap();
}

fn shadow_canvas(ui: &mut Ui, nested: bool) -> DrawBuf {
    // test_canvas_draw_sub_layer / test_canvas_draw_nested_sub_layer.
    let area = Area::new(0, 0, 99, 99);
    let layer = ui.draw_layer_create(None, ColorFormat::Argb8888, &area);
    let mut fill = FillDsc::new();
    fill.color = Color::WHITE;
    ui.draw_fill(layer, &fill, &area);
    let sub_area = Area::new(10, 10, 90, 90);
    let target = if nested {
        ui.draw_layer_create(Some(layer), ColorFormat::Argb8888, &sub_area)
    } else {
        layer
    };
    let mut rect = RectDsc::new();
    rect.bg_color = Color::hex(if nested { 0x0000ff } else { 0xff0000 });
    rect.bg_opa = 255;
    rect.base.drop_shadow_opa = 255;
    rect.base.drop_shadow_blur_radius = if nested { 4 } else { 5 };
    let rect_area = if nested {
        Area::new(35, 35, 65, 65)
    } else {
        Area::new(30, 30, 70, 70)
    };
    ui.draw_rect(target, &rect, &rect_area);
    if nested {
        let mut image = DrawImageDsc::new();
        image.src = ImageSrc::Layer(target);
        ui.draw_layer(layer, &image, &sub_area);
    }
    finish(ui, layer)
}

#[test]
fn drop_shadow_tasks_render_and_release_nested_layers() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let before = display.ui.draw.layers.len();
    for nested in [false, true] {
        let pixels = shadow_canvas(&mut display.ui, nested);
        assert_eq!(display.ui.draw.layers.len(), before, "child layers leaked");
        let stride = pixels.header.stride as usize;
        let at = |x: usize, y: usize| &pixels.data()[y * stride + x * 4..y * stride + x * 4 + 4];
        assert_eq!(at(0, 0), [255, 255, 255, 255]);
        assert_eq!(
            at(50, 50),
            if nested {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            }
        );
        let shadow = at(if nested { 33 } else { 28 }, 50);
        assert!(shadow[0] < 255 && shadow[0] > 0, "missing blurred shadow");
        assert_eq!(shadow[0], shadow[1]);
        assert_eq!(shadow[1], shadow[2]);
    }
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn drop_shadow_and_nested_layer_reference_pixels() {
    for (nested, reference) in [(false, "canvas_dropshadow"), (true, "canvas_nested_sub_layer")] {
        let mut display = TestDisplay::with_default_theme().unwrap();
        let pixels = shadow_canvas(&mut display.ui, nested);
        show_canvas(&mut display, pixels, false);
        display
            .screenshot_compare(format!("widgets/{reference}.png"))
            .unwrap();
    }
}
