//! Draw-command equivalents of the remaining screenshot scenes of
//! `widgets/test_canvas.c` (`test_canvas_layer_complex_drawing`) and
//! `draw/test_draw_blend.c` (`test_xrgb8888`, `test_argb8888`, `test_rgb888`,
//! `test_rgb565`) at the pinned LVGL commit.
//!
//! The canvas widget API is not ported. A "canvas" here is a [`DrawBuf`]
//! that is filled byte by byte exactly like `lv_canvas_fill_bg()`, drawn into
//! through a layer whose `draw_buf` is that buffer (`lv_canvas_init_layer()`),
//! drained like `lv_canvas_finish_layer()` and finally shown by an object
//! whose `DRAW_MAIN` callback draws it as an image at the object's coordinates
//! (what the `lv_image` base class of `lv_canvas` does with default styles).
//!
//! The scenes of `test_draw_blur.c` (`draw_blur_corner_*.png`) and the other
//! `test_canvas.c` screenshots (`canvas_2`, `canvas_dropshadow`,
//! `canvas_nested_sub_layer`) are ported in `parity_blur.rs` and
//! `parity_primitives.rs`. `canvas_1.png` (`lv_canvas_set_px` / palettes on
//! indexed buffers) needs the canvas widget and is out of scope.

use std::rc::Rc;

use ferrovgl_core::draw::draw_buf::{DrawBuf, STRIDE_AUTO};
use ferrovgl_core::draw::grad::GradDir;
use ferrovgl_core::draw::image::DrawImageDsc;
use ferrovgl_core::draw::{LabelDsc, LayerId, RectDsc};
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::color::{OPA_30, OPA_50, OPA_60, OPA_COVER};
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::style::ImageSrc;
use ferrovgl_core::{Area, Color, ColorFormat, EventCode, FontRef, Obj, ObjFlags, Opa, Ui};
use ferrovgl_test::TestDisplay;

/// A zero-initialised buffer, like the `static` arrays behind
/// `LV_DRAW_BUF_DEFINE_STATIC` and the canvas buffers of `test_draw_blend.c`.
fn canvas_buf(w: u32, h: u32, cf: ColorFormat) -> DrawBuf {
    DrawBuf::create(w, h, cf, STRIDE_AUTO).expect("canvas buffer")
}

/// `lv_canvas_fill_bg()` for the colour formats these scenes use.
fn canvas_fill_bg(buf: &mut DrawBuf, color: Color, opa: Opa) {
    let cf = buf.header.cf;
    let (w, h) = (buf.header.w as usize, buf.header.h as usize);
    let stride = buf.header.stride as usize;
    let data = buf.data_mut().expect("writable canvas buffer");
    for y in 0..h {
        let row = &mut data[y * stride..];
        match cf {
            ColorFormat::Rgb565 => {
                let c16 = color.to_u16().to_le_bytes();
                for px in row[..w * 2].as_chunks_mut::<2>().0 {
                    px.copy_from_slice(&c16);
                }
            }
            ColorFormat::Xrgb8888 | ColorFormat::Argb8888 => {
                let mut c32 = color.to_u32();
                if cf == ColorFormat::Argb8888 {
                    c32 &= 0x00ff_ffff;
                    c32 |= u32::from(opa) << 24;
                }
                for px in row[..w * 4].as_chunks_mut::<4>().0 {
                    px.copy_from_slice(&c32.to_le_bytes());
                }
            }
            ColorFormat::Rgb888 => {
                for px in row[..w * 3].as_chunks_mut::<3>().0 {
                    px.copy_from_slice(&[color.blue, color.green, color.red]);
                }
            }
            other => panic!("lv_canvas_fill_bg: {other:?} not used by these scenes"),
        }
    }
}

/// `lv_canvas_init_layer()`: a layer drawing directly into the canvas buffer.
fn canvas_init_layer(ui: &mut Ui, buf: DrawBuf) -> LayerId {
    let area = Area::new(0, 0, i32::from(buf.header.w) - 1, i32::from(buf.header.h) - 1);
    let layer = ui.draw_layer_create(None, buf.header.cf, &area);
    ui.draw.layer_mut(layer).unwrap().draw_buf = Some(buf);
    layer
}

/// `lv_canvas_finish_layer()`: dispatch until the queue is empty and hand the
/// canvas buffer back.
fn canvas_finish_layer(ui: &mut Ui, layer: LayerId) -> DrawBuf {
    for _ in 0..64 {
        if ui.draw.layer(layer).unwrap().tasks.is_empty() {
            break;
        }
        ui.draw_dispatch();
    }
    assert!(
        ui.draw.layer(layer).unwrap().tasks.is_empty(),
        "draw queue stalled"
    );
    let buf = ui.draw.layer_mut(layer).unwrap().draw_buf.take().unwrap();
    ui.draw_layer_delete(layer);
    buf
}

/// An `lv_canvas` showing `pixels`: content-sized, default (theme-less)
/// image styles, drawn as an image at its coordinates.
fn show_canvas(ui: &mut Ui, parent: Obj, pixels: DrawBuf) -> Obj {
    let canvas = ui.obj_create(Some(parent));
    ui.obj_remove_style_all(canvas);
    ui.obj_remove_flag(canvas, ObjFlags::SCROLLABLE);
    ui.obj_set_size(canvas, pixels.header.w.into(), pixels.header.h.into());
    let pixels = Rc::new(pixels);
    ui.obj_add_event_cb(
        canvas,
        Rc::new(move |ui, event| {
            let mut image = DrawImageDsc::new();
            image.src = ImageSrc::DrawBuf(pixels.clone());
            ui.draw_image(event.layer().unwrap(), &image, &ui.obj_get_coords(canvas));
        }),
        EventCode::DRAW_MAIN,
        None,
    );
    canvas
}

fn montserrat_14() -> Option<FontRef> {
    /*LV_FONT_DEFAULT of the test configuration*/
    Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14))
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn canvas_layer_complex_drawing() {
    // test_canvas_layer_complex_drawing: 100x100 ARGB8888 static (zeroed) buffer.
    let mut display = TestDisplay::with_default_theme().unwrap();
    let ui = &mut display.ui;
    let layer = canvas_init_layer(ui, canvas_buf(100, 100, ColorFormat::Argb8888));

    let mut rect = RectDsc::new();
    rect.bg_color = Color::hex(0xff0000);
    ui.draw_rect(layer, &rect, &Area::new(10, 10, 50, 50));

    // The C test sets `color = white` and then re-initialises the descriptor,
    // so the text is drawn with the default black colour.
    let mut label = LabelDsc::new();
    label.font = montserrat_14();
    label.text = Rc::from("Test");
    ui.draw_label(layer, &label, &Area::new(20, 20, 20 + 100, 20 + 50));

    let pixels = canvas_finish_layer(ui, layer);

    // lv_canvas_get_px(canvas, 30, 30) is opaque red.
    let at = pixels.goto_xy(30, 30);
    assert_eq!(&pixels.data()[at..at + 3], [0x00, 0x00, 0xff]);

    let screen = display.screen().unwrap();
    show_canvas(&mut display.ui, screen, pixels);
    display
        .screenshot_compare("widgets/canvas_layer_complex.png")
        .unwrap();
}

/// `canvas_basic_render()`: render the 180x180 small canvas in `render_cf`.
fn canvas_basic_render(ui: &mut Ui, render_cf: ColorFormat, name_main: &str, name_sub: &str) -> DrawBuf {
    let mut buf = canvas_buf(180, 180, render_cf);
    canvas_fill_bg(&mut buf, palette::lighten(Palette::LightBlue, 2), OPA_COVER);

    let layer = canvas_init_layer(ui, buf);

    let mut label = LabelDsc::new();
    label.font = montserrat_14();
    label.text = Rc::from(format!("{name_main}_to_{name_sub}"));
    let mut area = Area::new(3, 5, 180, 25);
    ui.draw_label(layer, &label, &area);

    let mut rect = RectDsc::new();
    rect.radius = 10;
    rect.bg_color = palette::main(Palette::Red);
    rect.bg_opa = OPA_COVER;
    rect.outline_color = palette::main(Palette::Indigo);
    rect.outline_width = 10;
    rect.outline_pad = -5;
    rect.outline_opa = OPA_60;
    rect.shadow_color = palette::main(Palette::Orange);
    rect.shadow_width = 10;
    rect.shadow_offset_x = 5;
    rect.shadow_offset_y = 10;

    area.x1 = 10;
    area.x2 = 170;
    area.y1 = 30;
    area.y2 = 60;
    ui.draw_rect(layer, &rect, &area);

    area.y1 = 80;
    area.y2 = 110;
    rect.bg_grad.dir = GradDir::Hor;
    rect.bg_grad.stops_count = 2;
    rect.bg_grad.stops[0].color = palette::main(Palette::Red);
    rect.bg_grad.stops[0].opa = OPA_COVER;
    rect.bg_grad.stops[0].frac = 0;
    rect.bg_grad.stops[1].color = palette::main(Palette::Green);
    rect.bg_grad.stops[1].opa = OPA_COVER;
    rect.bg_grad.stops[1].frac = 255;
    ui.draw_rect(layer, &rect, &area);

    area.y1 = 130;
    area.y2 = 160;
    rect.bg_grad.stops[0].opa = OPA_30;
    ui.draw_rect(layer, &rect, &area);

    canvas_finish_layer(ui, layer)
}

/// `canvas_blend_test()`: draw the small canvas twice (cover and 50% opacity)
/// into the large canvas.
fn canvas_blend_test(
    ui: &mut Ui,
    large: DrawBuf,
    name_main: &str,
    name_sub: &str,
    small_render_cf: ColorFormat,
    idx: i32,
) -> DrawBuf {
    let img = Rc::new(canvas_basic_render(ui, small_render_cf, name_main, name_sub));

    let mut area = Area::new(0, 0, 0, 0);
    area.x1 = (idx % 2) * 380 + 10;
    area.x2 = area.x1 + 179;
    area.y1 = (idx / 2) * 190 + 10;
    area.y2 = area.y1 + 179;

    let mut img_dsc = DrawImageDsc::new();
    img_dsc.src = ImageSrc::DrawBuf(img);

    img_dsc.opa = OPA_COVER;
    let layer = canvas_init_layer(ui, large);
    ui.draw_image(layer, &img_dsc, &area);
    let large = canvas_finish_layer(ui, layer);

    area.move_by(190, 0);

    img_dsc.opa = OPA_50;
    let layer = canvas_init_layer(ui, large);
    ui.draw_image(layer, &img_dsc, &area);
    canvas_finish_layer(ui, layer)
}

/// `canvas_draw()` of `test_draw_blend.c`.
fn canvas_draw(name: &str, large_render_cf: ColorFormat) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    /*setUp()*/
    display.ui.obj_set_flex_flow(screen, FlexFlow::RowWrap);
    display.ui.obj_set_flex_align(
        screen,
        FlexAlign::SpaceEvenly,
        FlexAlign::Center,
        FlexAlign::SpaceEvenly,
    );
    display.clean_screen();

    let ui = &mut display.ui;
    let mut large = canvas_buf(768, 390, large_render_cf);
    canvas_fill_bg(&mut large, palette::lighten(Palette::BlueGrey, 2), OPA_COVER);

    for (idx, (main, cf)) in [
        ("rgb565", ColorFormat::Rgb565),
        ("rgb888", ColorFormat::Rgb888),
        ("xrgb8888", ColorFormat::Xrgb8888),
        ("argb8888", ColorFormat::Argb8888),
    ]
    .into_iter()
    .enumerate()
    {
        large = canvas_blend_test(ui, large, main, name, cf, idx as i32);
    }

    show_canvas(ui, screen, large);
    display
        .screenshot_compare(format!("draw/blend_to_{name}.png"))
        .unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn blend_to_xrgb8888() {
    canvas_draw("xrgb8888", ColorFormat::Xrgb8888);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn blend_to_argb8888() {
    canvas_draw("argb8888", ColorFormat::Argb8888);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn blend_to_rgb888() {
    canvas_draw("rgb888", ColorFormat::Rgb888);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn blend_to_rgb565() {
    canvas_draw("rgb565", ColorFormat::Rgb565);
}
