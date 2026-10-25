//! LVGL v9.6.0 `tests/src/test_cases/test_snapshot.c` snapshot scenes.
//! Uses the actual Label and Image widgets, including image source sizing,
//! centered pivot, rotation and extended draw bounds.

use std::rc::Rc;

use ferrovgl_core::draw::DrawBuf;
use ferrovgl_core::misc::style::ImageSrc;
use ferrovgl_core::misc::types::coord::pct;
use ferrovgl_core::{Align, Color, ColorFormat, FontRef, Obj, StyleSelector, Ui};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Image, ImageAlign, Label};

const SELECTOR: StyleSelector = StyleSelector::DEFAULT;

fn snapshot_image(ui: &mut Ui, parent: Obj, buf: DrawBuf) -> Image {
    let image = Image::create(ui, parent);
    image.set_src(ui, ImageSrc::DrawBuf(Rc::new(buf)));
    image
}

fn label_scene(transformed: bool) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let label = Label::create(ui, screen);
    ui.obj_set_style_text_font(
        label.obj(),
        FontRef::Static(&ferrovgl_fonts::MONTSERRAT_28),
        SELECTOR,
    );
    label.set_text(ui, "Wubba lubba dub dub!");
    if transformed {
        ui.obj_set_style_transform_rotation(label.obj(), 450, SELECTOR);
    }
    let target = if transformed { screen } else { label.obj() };
    let buffer = ui.snapshot_take(target, ColorFormat::Argb8888).unwrap();
    if transformed {
        ui.obj_delete(label.obj());
        snapshot_image(ui, screen, buffer);
    } else {
        snapshot_image(ui, screen, buffer);
        ui.obj_delete(label.obj());
    }
    display
        .screenshot_compare(if transformed {
            "snapshot_2.png"
        } else {
            "snapshot_0.png"
        })
        .unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn snapshot_immediately_after_label_creation() {
    label_scene(false);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn snapshot_screen_with_transformed_label() {
    label_scene(true);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn snapshot_label_buffer_displayed_by_rotated_image_widget() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let label = Label::create(ui, screen);
    ui.obj_set_style_text_font(
        label.obj(),
        FontRef::Static(&ferrovgl_fonts::MONTSERRAT_28),
        SELECTOR,
    );
    label.set_text(ui, "Wubba lubba dub dub!");
    let buffer = ui.snapshot_take(label.obj(), ColorFormat::Argb8888).unwrap();
    let image = snapshot_image(ui, screen, buffer);
    ui.obj_delete(label.obj());
    // Upstream renders snapshot_0 before moving and rotating the same widget.
    // The separate immediate-creation test compares that first frame.
    display.snapshot().unwrap();
    let ui = &mut display.ui;
    ui.obj_center(image.obj());
    image.set_inner_align(ui, ImageAlign::Center);
    image.set_rotation(ui, 450);
    display.screenshot_compare("snapshot_1.png").unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn snapshot_multiple_objects_cover_hidden_and_opacity() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let obj_1 = ui.obj_create(Some(screen));
    ui.obj_set_size(obj_1, 200, 200);
    ui.obj_set_style_border_width(obj_1, 0, SELECTOR);
    ui.obj_set_style_bg_color(obj_1, Color::hex(0xffff00), SELECTOR);
    ui.obj_center(obj_1);

    let top = ui.obj_create(Some(screen));
    ui.obj_set_size(top, pct(100), pct(100));
    ui.obj_set_style_border_width(top, 0, SELECTOR);
    ui.obj_set_style_radius(top, 0, SELECTOR);
    ui.obj_set_style_bg_opa(top, 255, SELECTOR);
    ui.obj_set_style_pad_all(top, 0, SELECTOR);
    ui.obj_center(top);

    let obj_2 = ui.obj_create(Some(top));
    ui.obj_set_size(obj_2, 100, 100);
    ui.obj_set_style_border_width(obj_2, 0, SELECTOR);
    ui.obj_set_style_bg_color(obj_2, Color::hex(0xff0000), SELECTOR);
    ui.obj_center(obj_2);

    for (color, align, mode) in [
        (0x0000ff, Align::TopMid, 0),
        (0x00ffff, Align::BottomMid, 1),
        (0xffff00, Align::LeftMid, 2),
        (0xff00ff, Align::RightMid, 3),
    ] {
        let obj = ui.obj_create(Some(screen));
        ui.obj_set_size(obj, 100, 100);
        ui.obj_set_style_border_width(obj, 0, SELECTOR);
        ui.obj_set_style_bg_color(obj, Color::hex(color), SELECTOR);
        ui.obj_align(obj, align, 0, 0);
        match mode {
            1 => ui.obj_set_hidden(obj, true),
            2 => ui.obj_set_style_opa(obj, 0, SELECTOR),
            3 => ui.obj_set_style_opa_layered(obj, 0, SELECTOR),
            _ => {}
        }
    }
    let buffer = ui.snapshot_take(screen, ColorFormat::Argb8888).unwrap();
    let image = snapshot_image(ui, screen, buffer);
    ui.obj_center(image.obj());
    display.screenshot_compare("snapshot_3.png").unwrap();
}
