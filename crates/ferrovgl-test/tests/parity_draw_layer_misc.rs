//! Ported from LVGL v9.6.0: `tests/src/test_cases/test_display.c`
//! (`test_display_layers`), `draw/test_clip_corner.c`,
//! `draw/test_draw_dropshadow.c`, `draw/test_draw_layer.c`,
//! `draw/test_layer_transform.c` and `libs/test_bin_decoder.c`.
//!
//! Every screenshot test keeps the upstream scenario, values and reference
//! name. Upstream runs the tests of one file as a single suite on one
//! display; where a test depends on state an earlier test of the same file
//! left on the active screen (styles and layout set on `lv_screen_active()`
//! survive `tearDown()`'s `lv_obj_clean()`), that state is reproduced
//! explicitly and the reason is noted at the test.
//!
//! Tests that are not ported here:
//!
//! - `test_display.c`: the non-screenshot tests are ported as unit tests in
//!   `ferrovgl-core/src/display/mod.rs`; `test_display_matrix_rotation` is
//!   `TEST_PASS()` without `LV_DRAW_TRANSFORM_USE_MATRIX` (off in the parity
//!   configuration).
//! - `test_draw_drop_shadow_basic` needs `lv_chart` and the
//!   `img_benchmark_lvgl_logo_argb` asset, neither ported.
//! - `test_draw_layer_bitmap_mask_from_image` and the file variants of
//!   `test_bin_decoder.c` (`test_bin_decoder_bin_file`, the `"A:..."` sources
//!   of `test_bin_decoder_transformed_partial_decode`) read from `lv_fs`;
//!   there is no test file system.
//! - `test_draw_layer_alloc_failed_no_deadlock` replaces
//!   `lv_draw_buf_handlers_t::buf_malloc_cb`, which has no ferrovgl seam.
//! - `test_bin_decoder_flush_cache` is compiled out
//!   (`LV_BIN_DECODER_RAM_LOAD == 0`); `test_bin_decoder_open_stride_zero_handling`
//!   calls `lv_bin_decoder_open()` directly, which ferrovgl does not expose.
//!
//! The cogwheel images are exact byte-array extractions from LVGL v9.6.0 commit
//! 80ca777e37a2b176770726a02e07a6fb79ef0b39, `tests/src/test_assets/`:
//!
//! | asset | C array | bytes | SHA-256 of the bytes |
//! |---|---|---|---|
//! | `cogwheel.a8` | `test_img_cogwheel_a8.c::test_image_cogwheel_a8_map` | 10,000 | `814b45c3317d7eddbd21e4b25cd470d53d9bd9262fa349b2c2d3297b1a3c295a` |
//! | `cogwheel.i4` | `test_img_cogwheel_i4.c::test_image_cogwheel_i4_map` | 5,064 | `61a5782d30939a80ec8a5cf6edb4839b74cd656b5333ff9c1db4ecda4b91ec7b` |
//! | `cogwheel.argb8888` | `test_img_cogwheel_argb8888.c::test_image_cogwheel_argb8888_map` | 40,000 | `84d0c262f55f9f52b664606b6c4451cf1f22fbeb4785d9702d937bf519b64b20` |
//!
//! The descriptors match the C ones, which leave `header.magic` at zero.

use ferrovgl_core::draw::draw_buf::{ImageDsc, ImageHeader};
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::style::{ImageSrc, SharedRef};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Area, Color, ColorFormat, FontRef, Obj, Part, State, Style, StyleSelector, Ui};
use ferrovgl_fonts::MONTSERRAT_24;
use ferrovgl_test::{RgbImage, TestDisplay, UPSTREAM_DIR, compare_screenshot, workspace_root};
use ferrovgl_widgets::{Arc, Image, ImageAlign, Label};

const MAIN: StyleSelector = StyleSelector::DEFAULT;
/// `LV_SCALE_NONE`
const SCALE_NONE: i32 = 256;

/// `test_image_cogwheel_a8`
static COGWHEEL_A8: ImageDsc = ImageDsc {
    header: ImageHeader {
        magic: 0,
        ..ImageHeader::new(ColorFormat::A8, 100, 100, 100)
    },
    data_size: 10000,
    data: include_bytes!("assets/draw-layer-misc/cogwheel.a8"),
};

/// `test_image_cogwheel_i4`
static COGWHEEL_I4: ImageDsc = ImageDsc {
    header: ImageHeader {
        magic: 0,
        ..ImageHeader::new(ColorFormat::I4, 100, 100, 50)
    },
    data_size: 5064,
    data: include_bytes!("assets/draw-layer-misc/cogwheel.i4"),
};

/// `test_image_cogwheel_argb8888`
static COGWHEEL_ARGB8888: ImageDsc = ImageDsc {
    header: ImageHeader {
        magic: 0,
        ..ImageHeader::new(ColorFormat::Argb8888, 100, 100, 400)
    },
    data_size: 40000,
    data: include_bytes!("assets/draw-layer-misc/cogwheel.argb8888"),
};

fn screen(display: &TestDisplay) -> Obj {
    display.screen().expect("active screen")
}

/// `lv_test_screenshot_compare_core()`: compare the active draw buffer as it
/// is, without `lv_obj_invalidate(lv_screen_active())` and `lv_refr_now()`.
fn screenshot_compare_core(display: &mut TestDisplay, name: &str) {
    let disp = display.display();
    let d = display.ui.display_mut(disp).expect("test display");
    let buf = match d.buf_act {
        1 => d.buf_1.as_ref(),
        2 => d.buf_2.as_ref(),
        3 => d.buf_3.as_ref(),
        _ => None,
    }
    .expect("active draw buffer");
    let actual = RgbImage::from_draw_buf(buf).unwrap();
    let root = workspace_root();
    compare_screenshot(
        &actual,
        root.join(UPSTREAM_DIR).join("tests/ref_imgs"),
        root.join("target/parity"),
        name,
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// test_display.c
// ---------------------------------------------------------------------------

/// `test_display_layers`. The earlier tests of `test_display.c` use their own
/// displays (or compare nothing) and leave the default display's screen and
/// layers empty (`tearDown()` cleans all four).
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn display_layers() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let ui = &mut display.ui;
    let parents = [
        ui.layer_bottom().unwrap(), /* Bottom layer should be covered by screen active */
        ui.screen_active().unwrap(),
        ui.layer_top().unwrap(),
        ui.layer_sys().unwrap(),
    ];
    let palettes = [Palette::Red, Palette::Green, Palette::Blue, Palette::Yellow];
    let layer_names = ["Layer Bottom", "Screen Active", "Layer Top", "Layer Sys"];

    for i in 0..parents.len() {
        let obj = ui.obj_create(Some(parents[i]));
        ui.obj_align(obj, Align::Center, 20 * i as i32, 20 * i as i32);
        ui.obj_set_style_opa(obj, 127, MAIN); // LV_OPA_50
        ui.obj_set_style_bg_color(obj, palette::main(palettes[i]), MAIN);

        let label = Label::create(ui, obj);
        label.set_text_fmt(ui, format_args!("{}", layer_names[i]));
        ui.obj_center(label.obj());
    }

    display.screenshot_compare("display_layers.png").unwrap();

    let ui = &display.ui;
    let disp = ui.display_get_default();
    assert!(disp.is_some());
    assert!(ui.display_get_screen_active(disp).is_some());
    assert!(ui.display_get_screen_prev(disp).is_none());
    assert!(ui.display_get_layer_top(disp).is_some());
    assert!(ui.display_get_layer_sys(disp).is_some());
    assert!(ui.display_get_layer_bottom(disp).is_some());
}

// ---------------------------------------------------------------------------
// draw/test_clip_corner.c
// ---------------------------------------------------------------------------

fn create_panel(ui: &mut Ui, radius: i32, transform: bool) -> Obj {
    let parent = ui.obj_create(ui.screen_active());
    ui.obj_set_style_pad_all(parent, 3, MAIN);
    ui.obj_set_style_radius(parent, radius, MAIN);
    ui.obj_set_style_clip_corner(parent, true, MAIN);
    ui.obj_set_style_shadow_color(parent, Color::hex(0x888888), MAIN);
    ui.obj_set_style_shadow_width(parent, 30, MAIN);
    ui.obj_set_style_shadow_spread(parent, 10, MAIN);
    ui.obj_set_style_outline_color(parent, Color::hex(0xff0000), MAIN);
    ui.obj_set_style_outline_width(parent, 2, MAIN);
    ui.obj_set_style_outline_pad(parent, 5, MAIN);
    if transform {
        ui.obj_set_style_transform_rotation(parent, 300, MAIN);
    }

    let label = Label::create(ui, parent);
    ui.obj_set_width(label.obj(), coord::pct(200));
    label.set_text(
        ui,
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Vivamus dignissim quam id eros iaculis dapibus. Mauris nisl orci, vulputate sed eleifend a, consectetur et nulla.",
    );
    ui.obj_set_style_text_font(label.obj(), FontRef::Static(&MONTSERRAT_24), MAIN);
    ui.obj_set_style_bg_color(label.obj(), palette::main(Palette::Red), MAIN);
    ui.obj_set_style_bg_opa(label.obj(), 51, MAIN); // LV_OPA_20

    ui.obj_update_layout(parent);
    ui.obj_scroll_by(parent, -15, -15, false);

    parent
}

/// `test_clip_corner_1`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn clip_corner_1() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let scr = screen(&display);
    let ui = &mut display.ui;
    ui.obj_set_flex_flow(scr, FlexFlow::RowWrap);
    ui.obj_set_flex_align(scr, FlexAlign::Center, FlexAlign::Center, FlexAlign::SpaceEvenly);
    ui.obj_set_style_pad_column(scr, 40, MAIN);

    create_panel(ui, 0, false);
    create_panel(ui, 10, false);
    create_panel(ui, 30, false);
    create_panel(ui, 100, false);

    let cont = create_panel(ui, 0, true);
    ui.obj_set_flex_in_new_track(cont, true);
    create_panel(ui, 10, true);
    create_panel(ui, 30, true);
    create_panel(ui, 100, true);

    display.screenshot_compare("draw/clip_corner_1.png").unwrap();
}

// ---------------------------------------------------------------------------
// draw/test_draw_dropshadow.c
// ---------------------------------------------------------------------------

/// `test_draw_drop_shadow_invalidate`.
///
/// `test_draw_drop_shadow_basic` runs first in the same suite and leaves the
/// screen as a 32 px padded `LV_FLEX_FLOW_ROW_WRAP` container, so the flex
/// layout, not `lv_obj_set_pos(obj, 10, 10)`, places the arc. That state is
/// reproduced here; the basic test's children are deleted by `tearDown()`.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn draw_drop_shadow_invalidate() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let scr = screen(&display);
    let ui = &mut display.ui;
    /* Left over from test_draw_drop_shadow_basic() */
    ui.obj_set_flex_flow(scr, FlexFlow::RowWrap);
    ui.obj_set_style_pad_all(scr, 32, MAIN);
    ui.obj_set_style_pad_row(scr, 32, MAIN);
    ui.obj_set_style_pad_column(scr, 32, MAIN);

    let mut style = Style::new();
    style.set_drop_shadow_color(palette::main(Palette::Red));
    style.set_drop_shadow_radius(16);
    style.set_drop_shadow_opa(255);
    style.set_drop_shadow_offset_x(5);
    style.set_drop_shadow_offset_y(10);
    let style = ui.style_create(style);

    /*Create an object with the new style*/
    let obj = Arc::create(ui, scr);
    ui.obj_add_style(obj.obj(), style, Part::INDICATOR.into());
    ui.obj_set_pos(obj.obj(), 10, 10);
    obj.set_value(ui, 50);

    /*Don't use TEST_ASSERT_EQUAL_SCREENSHOT as it invalidates the screen removing all
     *manual partial invalidations below*/
    let path = "draw/draw_drop_shadow_invalidate.png";
    display.ui.refr_now(None);
    screenshot_compare_core(&mut display, path);

    for value in [10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 50] {
        obj.set_value(&mut display.ui, value);
        display.ui.refr_now(None);
    }

    screenshot_compare_core(&mut display, path);

    let a = Area::new(20, 20, 200, 60);
    display.ui.obj_invalidate_area(obj.obj(), &a);
    display.ui.refr_now(None);
    screenshot_compare_core(&mut display, path);
}

// ---------------------------------------------------------------------------
// draw/test_draw_layer.c
// ---------------------------------------------------------------------------

const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Quisque suscipit risus nec pharetra pulvinar. In hac habitasse platea dictumst. Proin placerat congue massa eu luctus. Suspendisse risus nulla, consectetur eget odio ut, mollis sollicitudin magna. Suspendisse volutpat consequat laoreet. Aenean sodales suscipit leo, vitae pulvinar lorem pulvinar eu. Nullam molestie hendrerit est sit amet imperdiet.";

/// `test_obj_create()`
fn test_obj_create(ui: &mut Ui, mask_src: &ImageSrc, scale: i32, angle: i32) -> Obj {
    let obj = ui.obj_create(ui.screen_active());
    ui.obj_set_size(obj, 200, 200);
    ui.obj_set_style_bg_color(obj, Color::hex3(0xf88), MAIN);
    ui.obj_set_style_bitmap_mask_src(obj, mask_src.clone(), MAIN);
    ui.obj_set_style_transform_scale(obj, scale, MAIN);
    ui.obj_set_style_transform_rotation(obj, angle, MAIN);
    ui.obj_center(obj);

    let label = Label::create(ui, obj);
    ui.obj_set_width(label.obj(), coord::pct(100));
    label.set_text(ui, LOREM);
    ui.obj_center(label.obj());

    obj
}

/// The four objects of the `test_draw_layer_bitmap_mask_*` tests.
fn bitmap_mask_scene(ui: &mut Ui, mask_src: &ImageSrc) {
    let scr = ui.screen_active().unwrap();
    ui.obj_set_flex_flow(scr, FlexFlow::Row);
    ui.obj_set_flex_align(scr, FlexAlign::SpaceEvenly, FlexAlign::Center, FlexAlign::Center);

    test_obj_create(ui, mask_src, SCALE_NONE, 0);
    test_obj_create(ui, mask_src, 300, 0);
    test_obj_create(ui, mask_src, SCALE_NONE, 200);
    test_obj_create(ui, mask_src, 80, 600);
}

/// `test_draw_layer_bitmap_mask_from_variable`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn draw_layer_bitmap_mask_from_variable() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    bitmap_mask_scene(&mut display.ui, &ImageSrc::Static(&COGWHEEL_A8));
    display
        .screenshot_compare("draw/draw_layer_bitmap_mask.png")
        .unwrap();
}

/// `test_draw_layer_bitmap_mask_from_image_not_found`: a mask source that
/// cannot be opened leaves the layers unmasked.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn draw_layer_bitmap_mask_from_image_not_found() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    bitmap_mask_scene(
        &mut display.ui,
        &ImageSrc::Path(SharedRef::Static("A:nonexistent/file")),
    );
    display
        .screenshot_compare("draw/draw_layer_bitmap_mask_not_masked.png")
        .unwrap();
}

/// `test_draw_layer_dispatch`.
///
/// The earlier tests of `test_draw_layer.c` leave the screen as an
/// `LV_FLEX_FLOW_ROW` container with `SPACE_EVENLY, CENTER, CENTER`
/// alignment; that state is reproduced here.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn draw_layer_dispatch() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let scr = screen(&display);
    let ui = &mut display.ui;
    /* Left over from the test_draw_layer_bitmap_mask_*() tests */
    ui.obj_set_flex_flow(scr, FlexFlow::Row);
    ui.obj_set_flex_align(scr, FlexAlign::SpaceEvenly, FlexAlign::Center, FlexAlign::Center);

    let obj = ui.obj_create(Some(scr));
    ui.obj_set_style_opa_layered(obj, 127, MAIN); // LV_OPA_50
    ui.obj_set_style_opa(obj, 0, MAIN); // LV_OPA_0
    ui.obj_center(obj);

    /*Verify whether this scenario will result in a busy loop.*/
    display
        .screenshot_compare("draw/draw_layer_dispatch.png")
        .unwrap();
}

// ---------------------------------------------------------------------------
// draw/test_layer_transform.c
// ---------------------------------------------------------------------------

/// `test_no_residual_border_on_scale_down`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn no_residual_border_on_scale_down() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let scr = screen(&display);
    let ui = &mut display.ui;
    let obj = ui.obj_create(Some(scr));
    ui.obj_set_size(obj, 100, 100);
    ui.obj_center(obj);
    ui.obj_set_style_border_color(obj, Color::hex3(0xf00), MAIN);
    ui.obj_set_style_bg_color(obj, Color::hex3(0x0f0), MAIN);
    display.screenshot_compare("draw/layer_transform_1.png").unwrap();

    let pivot_x = [
        coord::pct(50),
        coord::pct(8),
        coord::pct(0),
        coord::pct(106),
        coord::pct(-16),
    ];
    let pivot_y = [
        coord::pct(50),
        coord::pct(12),
        coord::pct(0),
        coord::pct(113),
        coord::pct(-27),
    ];
    let scale = [256 * 4, 412, 569, 288, 711];

    for i in 0..5 {
        let ui = &mut display.ui;
        ui.obj_set_style_transform_pivot_x(obj, pivot_x[i], MAIN);
        ui.obj_set_style_transform_pivot_y(obj, pivot_y[i], MAIN);
        ui.obj_set_style_transform_scale(obj, scale[i], MAIN);
        ui.refr_now(None);

        /*Should be the same as the original without any artifacts*/
        ui.obj_set_style_transform_scale(obj, 256, MAIN);
        display.screenshot_compare("draw/layer_transform_1.png").unwrap();
    }
}

/// `test_update_layer_type_on_state_change`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn update_layer_type_on_state_change() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let scr = screen(&display);
    let ui = &mut display.ui;
    let mut style = Style::new();
    style.set_transform_rotation(90);
    style.set_transform_scale_x(100);
    let style = ui.style_create(style);

    let obj = ui.obj_create(Some(scr));
    ui.obj_center(obj);
    ui.obj_add_style(obj, style, State::CHECKED.into());
    ui.refr_now(None);

    ui.obj_add_state(obj, State::CHECKED);
    display.screenshot_compare("draw/layer_transform_2.png").unwrap();
}

// ---------------------------------------------------------------------------
// libs/test_bin_decoder.c
// ---------------------------------------------------------------------------

/// `create_image()`
fn create_image(ui: &mut Ui, src: &ImageSrc) {
    let img = Image::create(ui, ui.screen_active());
    img.set_src(ui, src.clone());
    ui.obj_center(img.obj());
}

/// `create_image_tile()`
fn create_image_tile(ui: &mut Ui, src: &ImageSrc) {
    let img = Image::create(ui, ui.screen_active());
    img.set_src(ui, src.clone());
    ui.obj_center(img.obj());
    ui.obj_set_size(img.obj(), 275, 175);
    img.set_inner_align(ui, ImageAlign::Tile);
}

/// `bin_decoder()` / `bin_decoder_tile()`. `lv_image_cache_drop()` is a no-op
/// (ferrovgl has no image cache) and `TEST_ASSERT_MEM_LEAK_LESS_THAN` has no
/// `lv_mem` counterpart; the object count must return to its starting value.
fn bin_decoder(src: &ImageSrc, screenshot: &str, create: fn(&mut Ui, &ImageSrc)) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let scr = screen(&display);
    create(&mut display.ui, src);
    display.screenshot_compare(screenshot).unwrap();
    display.ui.obj_clean(scr);

    for _ in 0..20 {
        display.ui.obj_clean(scr);
        create(&mut display.ui, src);

        display.ui.obj_invalidate(scr);
        display.ui.refr_now(None);
    }
    display.screenshot_compare(screenshot).unwrap();
    display.ui.obj_clean(scr);
    assert_eq!(display.ui.obj_get_child_count(scr), 0);
}

/// `test_bin_decoder_i4`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn bin_decoder_i4() {
    bin_decoder(
        &ImageSrc::Static(&COGWHEEL_I4),
        "libs/bin_decoder_1.png",
        create_image,
    );
}

/// `test_bin_decoder_i4_tile`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn bin_decoder_i4_tile() {
    bin_decoder(
        &ImageSrc::Static(&COGWHEEL_I4),
        "libs/bin_decoder_2.png",
        create_image_tile,
    );
}

/// `test_bin_decoder_argb8888`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn bin_decoder_argb8888() {
    bin_decoder(
        &ImageSrc::Static(&COGWHEEL_ARGB8888),
        "libs/bin_decoder_3.png",
        create_image,
    );
}

/// `test_bin_decoder_argb8888_tile`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn bin_decoder_argb8888_tile() {
    bin_decoder(
        &ImageSrc::Static(&COGWHEEL_ARGB8888),
        "libs/bin_decoder_4.png",
        create_image_tile,
    );
}

/// `test_bin_decoder_transformed_partial_decode`, variable source only: the
/// rotated and scaled I4 image must render (and be dropped) without error.
#[test]
fn bin_decoder_transformed_partial_decode() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let scr = screen(&display);
    let ui = &mut display.ui;
    let img = Image::create(ui, scr);
    img.set_src(ui, ImageSrc::Static(&COGWHEEL_I4));
    ui.obj_center(img.obj());
    img.set_pivot(ui, 0, 0);
    img.set_rotation(ui, 300);
    img.set_scale(ui, 400);
    ui.refr_now(None);

    ui.obj_clean(scr);
}
