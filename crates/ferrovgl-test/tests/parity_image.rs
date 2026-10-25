//! Actual Image widget scenes from LVGL v9.6.0 `widgets/test_image.c`.
//!
//! Production PNG/file decoders are not ported. The fixture is decoded in this
//! test into an in-memory ARGB8888 DrawBuf; every widget operation and draw uses
//! the public Rust API. No reference pixels are used as input or adjusted.
//! The pinned `tests/src/test_assets/test_img_lvgl_logo.png` is byte-identical
//! to `test_img_lvgl_logo_png_map` in the corresponding C source (6878 bytes,
//! SHA256 91ff89868cba8e79474b22e3223f873869035d89fdb12a595eca80fae63a70f8).

use std::fs::File;
use std::io::BufReader;
use std::rc::Rc;

use ferrovgl_core::draw::DrawBuf;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::style::{ImageSrc, SharedRef};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, ColorFormat, Obj, StyleSelector, Ui};
use ferrovgl_test::{TestDisplay, UPSTREAM_DIR, workspace_root};
use ferrovgl_widgets::{Image, ImageAlign};

const SELECTOR: StyleSelector = StyleSelector::DEFAULT;
const ALIGNMENTS: [ImageAlign; 9] = [
    ImageAlign::TopLeft,
    ImageAlign::TopMid,
    ImageAlign::TopRight,
    ImageAlign::LeftMid,
    ImageAlign::Center,
    ImageAlign::RightMid,
    ImageAlign::BottomLeft,
    ImageAlign::BottomMid,
    ImageAlign::BottomRight,
];

fn logo() -> ImageSrc {
    let path = workspace_root()
        .join(UPSTREAM_DIR)
        .join("tests/src/test_assets/test_img_lvgl_logo.png");
    let mut decoder = png::Decoder::new(BufReader::new(File::open(path).unwrap()));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap();
    let mut rgba = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut rgba).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!((info.width, info.height), (105, 40));
    let mut buf = DrawBuf::create(info.width, info.height, ColorFormat::Argb8888, 0).unwrap();
    let stride = usize::from(buf.header.stride);
    let pixels = buf.data_mut().unwrap();
    for y in 0..info.height as usize {
        for x in 0..info.width as usize {
            let src = (y * info.width as usize + x) * 4;
            let dst = y * stride + x * 4;
            pixels[dst..dst + 4].copy_from_slice(&[rgba[src + 2], rgba[src + 1], rgba[src], rgba[src + 3]]);
        }
    }
    ImageSrc::DrawBuf(Rc::new(buf))
}

fn image_create(ui: &mut Ui, screen: Obj, src: &ImageSrc) -> Image {
    let image = Image::create(ui, screen);
    image.set_src(ui, src.clone());
    ui.obj_set_style_bg_opa(image.obj(), 51, SELECTOR); // LV_OPA_20
    ui.obj_set_style_bg_color(image.obj(), Color::BLACK, SELECTOR);
    ui.obj_set_style_shadow_width(image.obj(), 10, SELECTOR);
    ui.obj_set_style_shadow_color(image.obj(), Color::hex(0xff0000), SELECTOR);
    image
}

#[derive(Clone, Copy)]
enum Transform {
    Rotate,
    Scale,
    ScaleX,
    ScaleY,
    Combined,
}

fn transform_scene(mode: Transform, top_left: bool, name: &str) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let src = logo();
    for i in 0..8 {
        let image = image_create(ui, screen, &src);
        let (x, y) = match (mode, top_left) {
            (Transform::Rotate, false) => (100 + i % 4 * 160, 150 + i / 4 * 150),
            (Transform::Rotate, true) => (120 + i % 4 * 180, 100 + i / 4 * 300),
            (Transform::Combined, true) => (120 + i % 4 * 180, 120 + i / 4 * 220),
            (_, true) => (10 + i % 4 * 190, 150 + i / 4 * 150),
            _ => (40 + i % 4 * 200, 150 + i / 4 * 150),
        };
        ui.obj_set_pos(image.obj(), x, y);
        let scale = (64 + i * 64) as u32;
        match mode {
            Transform::Rotate => image.set_rotation(ui, i * 450),
            Transform::Scale => image.set_scale(ui, scale),
            Transform::ScaleX => image.set_scale_x(ui, scale),
            Transform::ScaleY => image.set_scale_y(ui, scale),
            Transform::Combined => {
                image.set_scale_x(ui, scale);
                image.set_scale_y(ui, (32 + i * 96) as u32);
                image.set_rotation(ui, 200 + i * 333);
            }
        }
        if top_left {
            image.set_pivot(ui, 0, 0);
        }
    }
    display.screenshot_compare(format!("widgets/{name}.png")).unwrap();
}

macro_rules! transform_test {
    ($name:ident, $mode:ident, $top_left:expr) => {
        #[test]
        #[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
        fn $name() {
            transform_scene(Transform::$mode, $top_left, stringify!($name));
        }
    };
}
transform_test!(image_rotate_pivot_center, Rotate, false);
transform_test!(image_rotate_pivot_top_left, Rotate, true);
transform_test!(image_scale_pivot_center, Scale, false);
transform_test!(image_scale_pivot_top_left, Scale, true);
transform_test!(image_scale_x_pivot_center, ScaleX, false);
transform_test!(image_scale_x_pivot_top_left, ScaleX, true);
transform_test!(image_scale_y_pivot_center, ScaleY, false);
transform_test!(image_scale_y_pivot_top_left, ScaleY, true);
transform_test!(image_rotate_and_scale_pivot_center, Combined, false);
transform_test!(image_rotate_and_scale_pivot_top_left, Combined, true);

fn alignment_scene(transformed: bool, offset: bool, symbol: bool, name: &str) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    if symbol {
        ui.obj_set_flex_flow(screen, FlexFlow::RowWrap);
        ui.obj_set_flex_align(
            screen,
            FlexAlign::SpaceEvenly,
            FlexAlign::Center,
            FlexAlign::SpaceEvenly,
        );
    }
    let src = logo();
    for (i, align) in ALIGNMENTS.into_iter().enumerate() {
        let image = image_create(ui, screen, &src);
        if symbol {
            image.set_src(ui, ImageSrc::Symbol(SharedRef::Static("\u{f03e}")));
        }
        ui.obj_set_size(image.obj(), 200, 120);
        ui.obj_set_pos(image.obj(), 30 + i as i32 % 3 * 260, 40 + i as i32 / 3 * 150);
        if transformed && !offset {
            image.set_scale_x(ui, 300);
            image.set_scale_y(ui, 200);
            image.set_rotation(ui, 200);
        }
        image.set_inner_align(ui, align);
        if offset {
            image.set_offset_x(ui, if symbol { 10 } else { 15 });
            image.set_offset_y(ui, if symbol { 15 } else { 20 });
        }
        if transformed && offset {
            image.set_scale_x(ui, 300);
            image.set_scale_y(ui, 200);
            image.set_rotation(ui, 200);
        }
    }
    display.screenshot_compare(format!("widgets/{name}.png")).unwrap();
}

macro_rules! alignment_test {
    ($name:ident, $transformed:expr, $offset:expr, $symbol:expr) => {
        #[test]
        #[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
        fn $name() {
            alignment_scene($transformed, $offset, $symbol, stringify!($name));
        }
    };
}
alignment_test!(image_normal_align, false, false, false);
alignment_test!(image_normal_align_offset, false, true, false);
alignment_test!(image_transform_align, true, false, false);
alignment_test!(image_transform_align_offset, true, true, false);
alignment_test!(image_symbol_normal_align, false, false, true);
alignment_test!(image_symbol_normal_align_offset, false, true, true);

fn automatic_scene(align: ImageAlign, name: &str) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let src = logo();
    for i in 0..9 {
        let image = image_create(ui, screen, &src);
        let w = [105 / 2, 105, 105 * 2][i / 3];
        let h = [40 / 2, 40, 40 * 2][i % 3];
        ui.obj_set_size(image.obj(), w, h);
        ui.obj_set_pos(image.obj(), 30 + i as i32 % 3 * 260, 40 + i as i32 / 3 * 150);
        image.set_inner_align(ui, align);
        if align == ImageAlign::Stretch {
            assert!((image.transformed_width(ui) - w).abs() <= 1);
            assert!((image.transformed_height(ui) - h).abs() <= 1);
        } else {
            let scale = image.scale(ui);
            assert_eq!(
                image.transformed_width(ui) / image.transformed_height(ui),
                105 / 40
            );
            assert!((image.transformed_width(ui) - ((105 * scale) >> 8)).abs() <= 1);
            assert!((image.transformed_height(ui) - ((40 * scale) >> 8)).abs() <= 1);
        }
    }
    display.screenshot_compare(format!("widgets/{name}.png")).unwrap();
}

macro_rules! automatic_test {
    ($name:ident, $align:ident) => {
        #[test]
        #[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
        fn $name() {
            automatic_scene(ImageAlign::$align, stringify!($name));
        }
    };
}
automatic_test!(image_stretch, Stretch);
automatic_test!(image_contain, Contain);
automatic_test!(image_contain_downscale, ContainDownscale);
automatic_test!(image_cover, Cover);

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn image_tile() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let src = logo();
    for (w, h, x, y, ofs_x, ofs_y) in [
        (350, coord::SIZE_CONTENT, 20, 20, 0, 0),
        (coord::SIZE_CONTENT, 150, 420, 20, 0, 0),
        (coord::SIZE_CONTENT, coord::SIZE_CONTENT, 20, 220, -20, 20),
        (150, 150, 220, 220, 0, 0),
        (150, 150, 420, 220, -2000, 2000),
    ] {
        let image = image_create(ui, screen, &src);
        ui.obj_set_size(image.obj(), w, h);
        image.set_inner_align(ui, ImageAlign::Tile);
        ui.obj_set_pos(image.obj(), x, y);
        image.set_offset_x(ui, ofs_x);
        image.set_offset_y(ui, ofs_y);
    }
    display.screenshot_compare("widgets/image_tile.png").unwrap();
}
