//! Integration coverage of public object, refresh, software draw and teardown paths.

use std::rc::Rc;

use ferrovgl_core::draw::label::LabelDsc;
use ferrovgl_core::{Area, Color, ColorFormat, EventCode, FontRef, RenderMode, StyleSelector};
use ferrovgl_test::{DISPLAY_DPI, RgbImage, TestDisplay};

#[test]
fn public_display_object_refresh_and_delete_render_pixels() {
    let mut display = TestDisplay::with_resolution(64, 48).unwrap();
    let screen = display.screen().unwrap();
    let selector = StyleSelector::DEFAULT;
    assert_eq!(display.ui.display_get_dpi(Some(display.display())), DISPLAY_DPI);
    assert_eq!(
        display.ui.display_get_render_mode(display.display()),
        RenderMode::Direct
    );
    assert_eq!(
        display.ui.display_get_color_format(Some(display.display())),
        ColorFormat::Xrgb8888
    );
    assert!(display.ui.display_get_theme(Some(display.display())).is_none());
    display.ui.obj_set_style_bg_color(screen, Color::WHITE, selector);
    display.ui.obj_set_style_bg_opa(screen, 255, selector);

    let rectangle = display.ui.obj_create(Some(screen));
    display.ui.obj_set_pos(rectangle, 10, 8);
    display.ui.obj_set_size(rectangle, 30, 24);
    display
        .ui
        .obj_set_style_bg_color(rectangle, Color::hex(0x123456), selector);
    display.ui.obj_set_style_bg_opa(rectangle, 255, selector);
    display.ui.obj_set_style_radius(rectangle, 6, selector);
    display.ui.obj_set_style_border_width(rectangle, 0, selector);

    let image = display.snapshot().unwrap();
    assert_eq!(image.pixel(0, 0), Some([255, 255, 255]));
    assert_eq!(image.pixel(24, 20), Some([0x12, 0x34, 0x56]));
    assert_eq!(image.pixel(10, 8), Some([255, 255, 255]));
    assert_eq!(image.pixel(40, 20), Some([255, 255, 255]));
    assert_eq!(
        display.snapshot().unwrap(),
        image,
        "repeat refresh must be stable"
    );

    display
        .ui
        .obj_set_style_bg_color(rectangle, Color::hex(0x654321), selector);
    let restyled = display.snapshot().unwrap();
    assert_eq!(restyled.pixel(24, 20), Some([0x65, 0x43, 0x21]));
    assert_eq!(restyled.pixel(0, 0), Some([255; 3]));

    display.ui.obj_delete(rectangle);
    let cleared = display.snapshot().unwrap();
    assert!(cleared.pixels().iter().all(|&v| v == 255));
    display.ui.deinit();
    assert!(!display.ui.is_initialized());
    assert!(display.ui.objs.is_empty());
    assert!(display.snapshot().is_err());
}

#[test]
fn draw_event_renders_montserrat_into_the_display() {
    let mut display = TestDisplay::with_resolution(128, 32).unwrap();
    let screen = display.screen().unwrap();
    display
        .ui
        .obj_set_style_bg_color(screen, Color::WHITE, StyleSelector::DEFAULT);
    display
        .ui
        .obj_set_style_bg_opa(screen, 255, StyleSelector::DEFAULT);
    display.ui.obj_add_event_cb(
        screen,
        Rc::new(|ui, event| {
            let layer = event.layer().expect("DRAW_MAIN layer");
            let mut label = LabelDsc::new();
            label.font = Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14));
            label.text = Rc::from("Hello Rust");
            label.color = Color::BLACK;
            ui.draw_label(layer, &label, &Area::new(4, 4, 123, 27));
        }),
        EventCode::DRAW_MAIN,
        None,
    );
    let image = display.snapshot().unwrap();
    assert_eq!(image.pixel(0, 0), Some([255, 255, 255]));
    let ink = image
        .pixels()
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|p| p[0] < 255)
        .count();
    assert!(ink > 100, "expected actual glyph ink, got {ink} pixels");
    assert!(ink < 1000, "text must not fill the display: {ink} pixels");
    assert_eq!(display.snapshot().unwrap(), image);
    display.ui.deinit();
}

#[test]
fn default_display_matches_upstream_configuration() {
    let mut display = TestDisplay::new().unwrap();
    let image = display.snapshot().unwrap();
    assert_eq!((image.width(), image.height()), (800, 480));
    display.wait(3);
    assert_eq!(display.ui.tick_get(), 3);
    display.ui.deinit();
}

#[test]
fn dimensions_cannot_truncate_the_draw_buffer_header() {
    for (width, height) in [
        (0, 1),
        (-1, 1),
        (1, 0),
        (16384, 1),
        (65536, 1),
        (1, 65536),
        (i32::MAX, 1),
    ] {
        let result = TestDisplay::with_resolution(width, height);
        assert!(matches!(result, Err(error) if error.kind() == std::io::ErrorKind::InvalidInput));
    }
}

#[test]
fn wait_zero_flushes_pending_style_changes() {
    let mut display = TestDisplay::with_resolution(8, 8).unwrap();
    let screen = display.screen().unwrap();
    display
        .ui
        .obj_set_style_bg_opa(screen, 255, StyleSelector::DEFAULT);
    display
        .ui
        .obj_set_style_bg_color(screen, Color::hex(0xff0000), StyleSelector::DEFAULT);
    display.wait(0);
    let buffer = display.ui.display_get_buf_active(display.display()).unwrap();
    assert_eq!(&buffer.data()[..3], &[0, 0, 255]);
    assert_eq!(display.ui.tick_get(), 0);
}

#[test]
fn changed_display_configuration_reallocates_the_buffer_like_upstream() {
    let mut display = TestDisplay::with_resolution(8, 8).unwrap();
    let disp = display.display();
    let screen = display.screen().unwrap();
    display
        .ui
        .obj_set_style_bg_opa(screen, 255, StyleSelector::DEFAULT);
    display
        .ui
        .obj_set_style_bg_color(screen, Color::hex(0xff0000), StyleSelector::DEFAULT);
    // `lv_test_display_create()`: `4 * (hor_res + LV_DRAW_BUF_STRIDE_ALIGN - 1)
    // * ver_res + LV_DRAW_BUF_ALIGN` bytes through `lv_draw_buf_init()`.
    let buffer = display.ui.display_get_buf_active(disp).unwrap();
    assert_eq!(buffer.data_size, 4 * 8 * 8 + 4);
    assert!(buffer.header.flags.is_empty());

    // `buf_changed_event_cb()` on `LV_EVENT_COLOR_FORMAT_CHANGED`.
    display.ui.display_set_color_format(disp, ColorFormat::Rgb565);
    let buffer = display.ui.display_get_buf_active(disp).unwrap();
    assert_eq!(buffer.header.cf, ColorFormat::Rgb565);
    assert_eq!(buffer.header.stride, 16);
    assert_eq!(display.snapshot().unwrap().pixel(7, 7), Some([255, 0, 0]));

    // ... and on `LV_EVENT_RESOLUTION_CHANGED`.
    display.ui.display_set_resolution(disp, 9, 8);
    let image = display.snapshot().unwrap();
    assert_eq!((image.width(), image.height()), (9, 8));
    assert_eq!(image.pixel(8, 7), Some([255, 0, 0]));
    display.ui.display_set_resolution(disp, 8, 8);
    display.ui.display_set_color_format(disp, ColorFormat::Xrgb8888);
    assert_eq!(
        display.ui.display_get_buf_active(disp).unwrap().data_size,
        4 * 8 * 8 + 4
    );

    // Upstream reads the active buffer whatever the render mode.
    display.ui.display_set_render_mode(disp, RenderMode::Partial);
    assert_eq!(display.snapshot().unwrap().pixel(0, 0), Some([255, 0, 0]));
    display.ui.display_set_render_mode(disp, RenderMode::Direct);

    // A header that does not fit the bytes, or a format `buf_to_xrgb8888()`
    // does not convert, is an error, not a panic.
    let buffer = display.ui.display_get_buf_active(disp).unwrap();
    assert!(RgbImage::from_buffer(8, 8, u32::from(u16::MAX), ColorFormat::Xrgb8888, buffer.data()).is_err());
    assert!(RgbImage::from_buffer(8, 8, 32, ColorFormat::A8, buffer.data()).is_err());
}

#[test]
fn callback_deleting_display_during_refresh_returns_an_error() {
    let mut display = TestDisplay::with_resolution(8, 8).unwrap();
    let handle = display.display();
    display.ui.display_add_event_cb(
        handle,
        Rc::new(move |ui, _| ui.display_delete(handle)),
        EventCode::REFR_READY,
        None,
    );
    assert!(display.snapshot().unwrap_err().to_string().contains("deleted"));
}
