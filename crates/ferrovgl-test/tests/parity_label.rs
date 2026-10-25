//! Exact scene ports from LVGL v9.6.0 `tests/src/test_cases/widgets/test_label.c`.
//! Run with `--include-ignored` after `cargo xtask refimgs sync`. References
//! are immutable upstream PNGs; actual pixels come from the Rust object pipeline.

use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::{StyleSelector, Ui};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Label, LabelLongMode};

fn scene() -> TestDisplay {
    let mut display = TestDisplay::with_default_theme().unwrap();
    display.ui.obj_clean(display.screen().unwrap());
    display
}

fn green_background(ui: &mut Ui, label: Label) {
    ui.obj_set_style_bg_color(label.obj(), palette::main(Palette::Green), StyleSelector::DEFAULT);
    ui.obj_set_style_bg_opa(label.obj(), 255, StyleSelector::DEFAULT);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_no_leading_space_after_line_wrap() {
    let mut display = scene();
    let screen = display.screen().unwrap();
    let label = Label::create(&mut display.ui, screen);
    label.set_text(&mut display.ui, "LongWord not!");
    display.ui.obj_set_size(label.obj(), 80, 100);
    green_background(&mut display.ui, label);
    display
        .screenshot_compare("widgets/label_no_leading_space.png")
        .unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_preserve_spaces_after_explicit_newline() {
    let mut display = scene();
    let screen = display.screen().unwrap();
    let label = Label::create(&mut display.ui, screen);
    label.set_text(&mut display.ui, "Hello\n   World\n\n   Indent");
    display.ui.obj_set_size(label.obj(), 200, 120);
    green_background(&mut display.ui, label);
    display
        .screenshot_compare("widgets/label_preserve_indent_after_newline.png")
        .unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_long_mode_clip() {
    let mut display = scene();
    let screen = display.screen().unwrap();
    for y in [10, 80] {
        let label = Label::create(&mut display.ui, screen);
        label.set_long_mode(&mut display.ui, LabelLongMode::Clip);
        label.set_text(
            &mut display.ui,
            "This is a long text that we will update while scrolling",
        );
        display.ui.obj_set_width(label.obj(), 150);
        display.ui.obj_set_pos(label.obj(), 10, y);
    }
    display.screenshot_compare("widgets/label_scroll_0.png").unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_wrap_mode_clip() {
    let mut display = scene();
    let screen = display.screen().unwrap();
    let label = Label::create(&mut display.ui, screen);
    label.set_long_mode(&mut display.ui, LabelLongMode::Wrap);
    label.set_text(
        &mut display.ui,
        "This is a long text that should be clipped vertically",
    );
    display.ui.obj_set_size(label.obj(), 150, 25);
    display.ui.obj_set_pos(label.obj(), 10, 10);
    display
        .ui
        .obj_set_style_border_width(label.obj(), 1, StyleSelector::DEFAULT);
    display
        .ui
        .obj_set_style_border_color(label.obj(), palette::main(Palette::Red), StyleSelector::DEFAULT);
    display.screenshot_compare("widgets/label_wrap_clip.png").unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_with_recolor_cmd() {
    let mut display = scene();
    let screen = display.screen().unwrap();
    let label = Label::create(&mut display.ui, screen);
    label.set_text(&mut display.ui, "Write a #ff0000 red# word");
    label.set_recolor(&mut display.ui, true);
    display.screenshot_compare("widgets/label_recolor.png").unwrap();
}
