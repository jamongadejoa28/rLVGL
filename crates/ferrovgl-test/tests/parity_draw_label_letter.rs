//! Exact scene ports from LVGL v9.6.0 `tests/src/test_cases/draw/test_draw_label.c`
//! and `tests/src/test_cases/draw/test_draw_letter.c`.
//!
//! Run with `--include-ignored` after `cargo xtask refimgs sync`. References are
//! immutable upstream PNGs; actual pixels come from the Rust object and draw
//! pipelines.
//!
//! Each upstream file runs as one suite whose `setUp()` puts the active screen
//! into a `ROW_WRAP` flex with `SPACE_EVENLY`/`CENTER`/`SPACE_EVENLY` alignment
//! and whose `tearDown()` cleans the screen. Every function here starts from a
//! fresh [`TestDisplay`] and applies that `setUp()`; nothing else carries over
//! between the upstream functions.
//!
//! The FreeType (`test_draw_letter_bitmap`, `test_draw_letter_outline`) and
//! image-font (`test_draw_letter_image_format_pivot`) scenarios need font
//! engines ferrovgl does not have and are not ported.

use std::rc::Rc;

use ferrovgl_core::draw::LayerId;
use ferrovgl_core::draw::draw_buf::DrawBuf;
use ferrovgl_core::draw::label::LetterDsc;
use ferrovgl_core::font::FmtTxtFont;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::color::{OPA_50, OPA_COVER};
use ferrovgl_core::misc::math::{atan2, trigo_sin};
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::style::ImageSrc;
use ferrovgl_core::misc::text::{TextAlign, TextDecor};
use ferrovgl_core::{Area, Color, ColorFormat, FontRef, Obj, Opa, Part, Point, Style, StyleSelector, Ui};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Image, Label};

/// `test_font_montserrat_ascii_1bpp` (`tests/src/test_assets/test_font_montserrat_ascii_1bpp.c`,
/// sha256 `763a82694059ef17d7ded961f9286f79f1e25d188e0f6eab3dfe9ec28b46b59d`).
/// Blob sha256 `be751e76c188670b78da74622f62ccfe2bcb3a3edb33267079e1d4551cd71e6a`.
static FONT_1BPP: FmtTxtFont<'static> = FmtTxtFont::from_bytes_const(include_bytes!(
    "assets/draw-label-letter/test_font_montserrat_ascii_1bpp.lvfnt"
));
/// `test_font_montserrat_ascii_2bpp` (`tests/src/test_assets/test_font_montserrat_ascii_2bpp.c`,
/// sha256 `deb9f5813f9565c29fb64cc59790c3c04eed4a90583d866ade10c155a7ceb3eb`).
/// Blob sha256 `b4f69f7be73575c8abf72cf0db7548cc90384f46d202067be86de80bfd135c3b`.
static FONT_2BPP: FmtTxtFont<'static> = FmtTxtFont::from_bytes_const(include_bytes!(
    "assets/draw-label-letter/test_font_montserrat_ascii_2bpp.lvfnt"
));
/// `test_font_montserrat_ascii_4bpp` (`tests/src/test_assets/test_font_montserrat_ascii_4bpp.c`,
/// sha256 `63097e9451aa1cb21874d25311fa2b0214a2ac9f282fa5c2602702b6a19daed7`).
/// Blob sha256 `0b5227b288417780d0374e629485e8e7ebee1b057a6d41d947dffc0fce259080`.
static FONT_4BPP: FmtTxtFont<'static> = FmtTxtFont::from_bytes_const(include_bytes!(
    "assets/draw-label-letter/test_font_montserrat_ascii_4bpp.lvfnt"
));
/// `test_font_montserrat_ascii_8bpp` (`tests/src/test_assets/test_font_montserrat_ascii_8bpp.c`,
/// sha256 `8c5bdfd863cdb273e958b355ded3da726a12d96dec0bfc48f215b4020f121b05`).
/// Blob sha256 `386b28796d58c249edb4106be9dd36718e3e57fb8a84d699d3a49db89de78bcb`.
static FONT_8BPP: FmtTxtFont<'static> = FmtTxtFont::from_bytes_const(include_bytes!(
    "assets/draw-label-letter/test_font_montserrat_ascii_8bpp.lvfnt"
));
/// `test_font_montserrat_ascii_3bpp_compressed`
/// (`tests/src/test_assets/test_font_montserrat_ascii_3bpp_compressed.c`,
/// sha256 `50953de15157169e3712f0f832c845f6bb9a541c4ab5dedee0506bf7a3485a54`).
/// Blob sha256 `2faa2111e545c277f02826f7d755f0eb4fae508377c485f5f6ba12ebc226d275`.
static FONT_3BPP_COMPRESSED: FmtTxtFont<'static> = FmtTxtFont::from_bytes_const(include_bytes!(
    "assets/draw-label-letter/test_font_montserrat_ascii_3bpp_compressed.lvfnt"
));

const DEFAULT: StyleSelector = StyleSelector::DEFAULT;

/// `setUp()` of both files.
fn set_up() -> TestDisplay {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    display.ui.obj_set_flex_flow(screen, FlexFlow::RowWrap);
    display.ui.obj_set_flex_align(
        screen,
        FlexAlign::SpaceEvenly,
        FlexAlign::Center,
        FlexAlign::SpaceEvenly,
    );
    display
}

// ---------------------------------------------------------------------------
// test_draw_label.c
// ---------------------------------------------------------------------------

/// `label_create()`
fn label_create(
    ui: &mut Ui,
    screen: Obj,
    font: &'static FmtTxtFont<'static>,
    style: Option<ferrovgl_core::core::obj_style::StyleId>,
    text_base: &str,
) -> Label {
    let label = Label::create(ui, screen);
    label.set_text_fmt(
        ui,
        format_args!("{text_base}: the quick brown fox jumps over the lazy dog"),
    );
    ui.obj_set_style_text_font(label.obj(), FontRef::Static(font), DEFAULT);
    if let Some(style) = style {
        ui.obj_add_style(label.obj(), style, DEFAULT);
    }
    label
}

/// `all_labels_create()`
fn all_labels_create(name: &str, style: Option<Style>) {
    let mut display = set_up();
    let screen = display.screen().unwrap();
    let style = style.map(|style| display.ui.style_create(style));
    let ui = &mut display.ui;
    label_create(ui, screen, &FONT_1BPP, style, "1bpp font");
    label_create(ui, screen, &FONT_2BPP, style, "2bpp font");
    label_create(ui, screen, &FONT_4BPP, style, "4bpp font");
    label_create(ui, screen, &FONT_8BPP, style, "8bpp font");
    label_create(ui, screen, &FONT_3BPP_COMPRESSED, style, "3bpp compressed font");
    display
        .screenshot_compare(format!("draw/label_{name}.png"))
        .unwrap();
}

fn red_text_style() -> Style {
    let mut style = Style::new();
    style.set_text_color(palette::main(Palette::Red));
    style
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn draw_label_normal() {
    all_labels_create("normal", None);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn draw_label_color() {
    all_labels_create("color", Some(red_text_style()));
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn draw_label_opa() {
    // Upstream sets only the red text colour here as well.
    all_labels_create("opa", Some(red_text_style()));
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn draw_label_color_and_opa() {
    let mut style = red_text_style();
    style.set_text_opa(OPA_50);
    all_labels_create("color_and_opa", Some(style));
}

/// Selection colours shared by the decor and selection scenarios.
fn selection_colors(ui: &mut Ui, label: Label) {
    let sel_bg_color = palette::lighten(Palette::Red, 4);
    let sel_color = palette::darken(Palette::Red, 4);
    ui.obj_set_style_bg_color(label.obj(), sel_bg_color, Part::SELECTED.into());
    ui.obj_set_style_text_color(label.obj(), sel_color, Part::SELECTED.into());
}

/// `decor_label_create()`
fn decor_label_create(ui: &mut Ui, screen: Obj, decor: TextDecor, align: TextAlign, opa: Opa) -> Label {
    let color = palette::main(Palette::Blue);
    let label = Label::create(ui, screen);
    label.set_text(ui, "Hi,\nTesting the\nlabels.");
    ui.obj_set_style_text_decor(label.obj(), decor, DEFAULT);
    ui.obj_set_style_text_color(label.obj(), color, DEFAULT);
    ui.obj_set_style_text_opa(label.obj(), opa, DEFAULT);
    ui.obj_set_style_text_align(label.obj(), align, DEFAULT);
    selection_colors(ui, label);
    label.set_text_selection_start(ui, 10);
    label.set_text_selection_end(ui, 19);
    label
}

/// `all_decor_labels_create()`
fn all_decor_labels_create(ui: &mut Ui, screen: Obj, decor: TextDecor) {
    let label = decor_label_create(ui, screen, decor, TextAlign::Left, OPA_COVER);
    ui.obj_set_flex_in_new_track(label.obj(), true);
    decor_label_create(ui, screen, decor, TextAlign::Center, OPA_COVER);
    decor_label_create(ui, screen, decor, TextAlign::Right, OPA_COVER);
    decor_label_create(ui, screen, decor, TextAlign::Left, OPA_50);
    decor_label_create(ui, screen, decor, TextAlign::Center, OPA_50);
    decor_label_create(ui, screen, decor, TextAlign::Right, OPA_50);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_decor() {
    let mut display = set_up();
    let screen = display.screen().unwrap();
    for decor in [
        TextDecor::NONE,
        TextDecor::UNDERLINE,
        TextDecor::STRIKETHROUGH,
        TextDecor::UNDERLINE | TextDecor::STRIKETHROUGH,
    ] {
        all_decor_labels_create(&mut display.ui, screen, decor);
    }
    display.screenshot_compare("draw/label_decor.png").unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_selection_and_recolor() {
    let mut display = set_up();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let label = Label::create(ui, screen);
    label.set_recolor(ui, true);
    label.set_text(ui, "Hi, Testing the #00ff00 colored labels.#");
    ui.obj_set_style_text_decor(label.obj(), TextDecor::NONE, DEFAULT);
    ui.obj_set_style_text_color(label.obj(), palette::main(Palette::Blue), DEFAULT);
    ui.obj_set_style_text_opa(label.obj(), OPA_COVER, DEFAULT);
    ui.obj_set_style_text_align(label.obj(), TextAlign::Left, DEFAULT);
    selection_colors(ui, label);
    label.set_text_selection_start(ui, 10);
    label.set_text_selection_end(ui, 25);
    display
        .screenshot_compare("draw/label_selection_and_recolor.png")
        .unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_selection_with_letter_space() {
    let mut display = set_up();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let label = Label::create(ui, screen);
    label.set_text(ui, "Testing selection with letter space label.");
    ui.obj_set_style_text_color(label.obj(), palette::main(Palette::Blue), DEFAULT);
    ui.obj_set_style_text_opa(label.obj(), OPA_COVER, DEFAULT);
    ui.obj_set_style_text_align(label.obj(), TextAlign::Left, DEFAULT);
    selection_colors(ui, label);
    ui.obj_set_style_text_letter_space(label.obj(), 10, DEFAULT);
    label.set_text_selection_start(ui, 10);
    label.set_text_selection_end(ui, 25);
    display
        .screenshot_compare("draw/label_selection_letter_space.png")
        .unwrap();
}

// ---------------------------------------------------------------------------
// test_draw_letter.c
// ---------------------------------------------------------------------------

/// `lv_canvas_finish_layer()`: dispatch until the layer's queue is empty.
fn finish_layer(ui: &mut Ui, layer: LayerId) -> DrawBuf {
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

/// `canvas_create()` + `lv_canvas_set_draw_buf()`: a canvas is an image
/// widget showing its ARGB8888 draw buffer.
fn canvas_show(ui: &mut Ui, screen: Obj, pixels: DrawBuf) -> Image {
    let canvas = Image::create(ui, screen);
    ui.obj_set_size(canvas.obj(), pixels.header.w.into(), pixels.header.h.into());
    canvas.set_src(ui, ImageSrc::DrawBuf(Rc::new(pixels)));
    canvas
}

/// `CURVE1_X(t)`
fn curve1_x(t: i32) -> i32 {
    t * 2 + 20
}

/// `CURVE1_Y(t)`
fn curve1_y(t: i32) -> i32 {
    trigo_sin(t as i16) * 40 / 32767 + 80
}

/// `CURVE2_X(t)`
fn curve2_x(t: i32) -> i32 {
    t * 3 + 20
}

/// `CURVE2_Y(t)`
fn curve2_y(t: i32) -> i32 {
    trigo_sin((t * 4) as i16) * 40 / 32767 + 230
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn draw_sin_wave() {
    let mut display = set_up();
    let screen = display.screen().unwrap();
    let string = b"lol~ I'm wavvvvvvving~";
    let string_len = string.len();

    // canvas_create(500, 360): cleared ARGB8888 buffer drawn through a layer.
    let area = Area::new(0, 0, 499, 359);
    let layer = display.ui.draw_layer_create(None, ColorFormat::Argb8888, &area);

    let mut letter_dsc = LetterDsc::new();
    letter_dsc.color = Color::hex(0xff0000);
    letter_dsc.font = Some(FontRef::Static(&FONT_4BPP));

    {
        let mut pre_x = curve1_x(-1);
        let mut pre_y = curve1_y(-1);
        for i in 0..30i16 {
            let angle = i32::from(i) * 10;
            let x = curve1_x(angle);
            let y = curve1_y(angle);
            letter_dsc.unicode = u32::from(string[i as usize % string_len]);
            letter_dsc.rotation = i32::from(atan2(y - pre_y, x - pre_x));
            letter_dsc.rotation = (if letter_dsc.rotation > 180 {
                letter_dsc.rotation - 360
            } else {
                letter_dsc.rotation
            }) * 5;
            display.ui.draw_letter(layer, &mut letter_dsc, Point::new(x, y));
            pre_x = x;
            pre_y = y;
        }
    }

    {
        let mut pre_x = curve2_x(-1);
        let mut pre_y = curve2_y(-1);
        for i in 0..30i16 {
            let angle = i32::from(i) * 5;
            let x = curve2_x(angle);
            let y = curve2_y(angle);
            letter_dsc.unicode = u32::from(string[i as usize % string_len]);
            letter_dsc.rotation = i32::from(atan2(y - pre_y, x - pre_x)) * 10;
            letter_dsc.color = Color::from_hsv((i * 10) as u16, 100, 100);
            display.ui.draw_letter(layer, &mut letter_dsc, Point::new(x, y));
            pre_x = x;
            pre_y = y;
        }
    }

    let pixels = finish_layer(&mut display.ui, layer);
    canvas_show(&mut display.ui, screen, pixels);
    display.screenshot_compare("draw/letter_0.png").unwrap();
}
