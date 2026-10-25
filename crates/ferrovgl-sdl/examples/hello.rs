//! Rounded rectangle and real label widgets in the SDL display.

use std::time::Duration;

use ferrovgl_core::{Color, FontRef, StyleSelector, UiConfig};
use ferrovgl_sdl::{DEFAULT_TITLE, SdlDisplay};
use ferrovgl_widgets::{Label, theme};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let limit = match args.next().as_deref() {
        None => None,
        Some("--frames") => {
            let count: u32 = args
                .next()
                .ok_or("--frames requires a positive count")?
                .parse()
                .map_err(|_| "invalid frame count")?;
            if count == 0 || args.next().is_some() {
                return Err("usage: hello [--frames N], where N > 0".into());
            }
            Some(count)
        }
        Some("--help" | "-h") => {
            println!(
                "Usage: hello [--frames N]\nClose the window to exit. --frames bounds the event/render loop for smoke checks.\nFor headless execution, set SDL_VIDEODRIVER=dummy."
            );
            return Ok(());
        }
        Some(_) => return Err("usage: hello [--frames N]".into()),
    };

    let mut display = SdlDisplay::new(
        DEFAULT_TITLE,
        800,
        480,
        UiConfig {
            dpi_def: 160,
            font_default: Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14)),
            ..UiConfig::default()
        },
        Some(theme::default::init_display),
    )?;
    let screen = display.screen().ok_or("display has no screen")?;
    let selector = StyleSelector::DEFAULT;
    let card = display.ui.obj_create(Some(screen));
    display.ui.obj_set_pos(card, 80, 100);
    display.ui.obj_set_size(card, 640, 240);
    display
        .ui
        .obj_set_style_bg_color(card, Color::hex(0x1565c0), selector);
    display.ui.obj_set_style_bg_opa(card, 255, selector);
    display.ui.obj_set_style_radius(card, 24, selector);
    display.ui.obj_set_style_border_width(card, 0, selector);

    let title = Label::create(&mut display.ui, card);
    title.set_text(&mut display.ui, "Hello, Rust!");
    display.ui.obj_set_pos(title.obj(), 24, 32);
    display.ui.obj_set_width(title.obj(), 550);
    display
        .ui
        .obj_set_style_text_color(title.obj(), Color::WHITE, selector);

    let detail = Label::create(&mut display.ui, card);
    detail.set_text(&mut display.ui, "LVGL objects, styles and software rendering in pure Rust.\nMontserrat 14, a rounded rectangle and SDL2 presentation.");
    display.ui.obj_set_pos(detail.obj(), 24, 76);
    display.ui.obj_set_width(detail.obj(), 550);
    display
        .ui
        .obj_set_style_text_color(detail.obj(), Color::WHITE, selector);

    let mut iterations = 0;
    while limit.is_none_or(|limit| iterations < limit) && display.step()? {
        iterations += 1;
        if limit.is_none_or(|limit| iterations < limit) {
            std::thread::sleep(Duration::from_millis(16));
        }
    }
    println!(
        "{iterations} event/render iterations, {} SDL presentations",
        display.presented_frames()
    );
    display.ui.deinit();
    Ok(())
}
