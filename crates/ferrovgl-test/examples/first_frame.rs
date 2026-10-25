//! Render a real 800×480 UI frame to PNG without SDL or an upstream reference.
//!
//! Uses the default theme, real label widgets and Montserrat 14. All shapes and
//! text pass through the object/event/layout/refresh pipeline.

use ferrovgl_core::{Color, Obj, StyleSelector, Ui};
use ferrovgl_test::{DISPLAY_DPI, TestDisplay, workspace_root};
use ferrovgl_widgets::Label;
use std::io;
use std::path::PathBuf;

fn text(ui: &mut Ui, obj: Obj, value: &str, x: i32, y: i32, color: Color) {
    let label = Label::create(ui, obj);
    label.set_text(ui, value);
    ui.obj_set_pos(label.obj(), x, y);
    ui.obj_set_style_text_color(label.obj(), color, StyleSelector::DEFAULT);
}

fn main() -> io::Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target/first-frame.png"));
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut display = TestDisplay::with_default_theme()?;
    let screen = display.screen().expect("the display creates an active screen");
    let selector = StyleSelector::DEFAULT;
    display
        .ui
        .obj_set_style_bg_color(screen, Color::hex(0xeaf0f6), selector);
    display.ui.obj_set_style_bg_opa(screen, 255, selector);

    let card = display.ui.obj_create(Some(screen));
    display.ui.obj_set_pos(card, 96, 100);
    display.ui.obj_set_size(card, 608, 280);
    display.ui.obj_set_style_bg_color(card, Color::WHITE, selector);
    display.ui.obj_set_style_bg_opa(card, 255, selector);
    display.ui.obj_set_style_radius(card, 24, selector);
    display.ui.obj_set_style_border_width(card, 1, selector);
    display.ui.obj_set_style_pad_all(card, 0, selector);
    display
        .ui
        .obj_set_style_border_color(card, Color::hex(0xcbd5e1), selector);

    let panel = display.ui.obj_create(Some(card));
    display.ui.obj_set_pos(panel, 31, 96);
    display.ui.obj_set_size(panel, 544, 88);
    display
        .ui
        .obj_set_style_bg_color(panel, Color::hex(0x2563eb), selector);
    display.ui.obj_set_style_bg_opa(panel, 255, selector);
    display.ui.obj_set_style_radius(panel, 16, selector);
    display.ui.obj_set_style_border_width(panel, 0, selector);
    display.ui.obj_set_style_pad_all(panel, 0, selector);

    text(
        &mut display.ui,
        card,
        "ferrovgl / FIRST FRAME",
        31,
        31,
        Color::hex(0x0f172a),
    );
    text(
        &mut display.ui,
        card,
        "Pure Rust objects, styles and software rendering",
        31,
        56,
        Color::hex(0x475569),
    );
    text(&mut display.ui, panel, "Hello, Rust!", 24, 21, Color::WHITE);
    text(
        &mut display.ui,
        panel,
        "Rounded rectangle + Montserrat 14",
        24,
        46,
        Color::hex(0xdbeafe),
    );
    text(
        &mut display.ui,
        card,
        &format!("800 x 480   |   XRGB8888   |   {DISPLAY_DPI} DPI   |   direct mode"),
        31,
        220,
        Color::hex(0x475569),
    );

    display.snapshot()?.write_png(&output)?;
    display.ui.deinit();
    println!("Wrote {}", output.display());
    Ok(())
}
