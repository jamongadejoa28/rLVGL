//! Button and image widgets with mouse, keyboard, wheel and flex/grid layout.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use ferrovgl_core::draw::DrawBuf;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::layouts::grid::{GRID_TEMPLATE_LAST, GridAlign, grid_fr};
use ferrovgl_core::misc::style::{ImageSrc, TracksRef};
use ferrovgl_core::{ColorFormat, EventCode, FontRef, StyleSelector, UiConfig};
use ferrovgl_sdl::SdlDisplay;
use ferrovgl_widgets::{Button, Image, Label, theme};

fn arrow_source() -> Result<ImageSrc, String> {
    let mut buffer = DrawBuf::create(24, 24, ColorFormat::Argb8888, 0).ok_or("unable to allocate icon")?;
    let stride = usize::from(buffer.header.stride);
    let pixels = buffer.data_mut().ok_or("icon is not writable")?;
    pixels.fill(0);
    for y in 4usize..20 {
        for x in 4usize..20 {
            // A right-facing arrow, drawn once and shared by all six images.
            let tip = 19 - y.abs_diff(12);
            if (10..15).contains(&y) && x < 13 || (12..=tip).contains(&x) {
                let offset = y * stride + x * 4;
                pixels[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
    }
    Ok(ImageSrc::DrawBuf(Rc::new(buffer)))
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let frames: Option<u32> = match args.next().as_deref() {
        None => None,
        Some("--frames") => {
            let n = args
                .next()
                .ok_or("--frames requires a count")?
                .parse()
                .map_err(|_| "invalid count")?;
            if n == 0 || args.next().is_some() {
                return Err("usage: interactive [--frames N], N > 0".into());
            }
            Some(n)
        }
        _ => return Err("usage: interactive [--frames N]".into()),
    };
    let mut window = SdlDisplay::new(
        "ferrovgl - buttons, images and input",
        800,
        480,
        UiConfig {
            dpi_def: 160,
            font_default: Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14)),
            ..UiConfig::default()
        },
        Some(theme::default::init_display),
    )?;
    let screen = window.screen().ok_or("missing screen")?;
    let sel = StyleSelector::DEFAULT;
    window.ui.obj_set_flex_flow(screen, FlexFlow::Column);
    window
        .ui
        .obj_set_flex_align(screen, FlexAlign::Start, FlexAlign::Center, FlexAlign::Center);
    window.ui.obj_set_style_pad_all(screen, 28, sel);
    let title = Label::create(&mut window.ui, screen);
    title.set_text(&mut window.ui, "Buttons + images + input, in pure Rust");
    let instructions = Label::create(&mut window.ui, screen);
    instructions.set_text(
        &mut window.ui,
        "Click to rotate. Tab / Shift+Tab or wheel to focus. Enter or middle-click to activate.",
    );
    let grid = window.ui.obj_create(Some(screen));
    window.ui.obj_set_size(grid, 720, 270);
    static COLUMNS: [i32; 4] = [grid_fr(1), grid_fr(1), grid_fr(1), GRID_TEMPLATE_LAST];
    static ROWS: [i32; 3] = [grid_fr(1), grid_fr(1), GRID_TEMPLATE_LAST];
    window
        .ui
        .obj_set_grid_dsc_array(grid, TracksRef::Static(&COLUMNS), TracksRef::Static(&ROWS));
    let status = Label::create(&mut window.ui, screen);
    status.set_text(&mut window.ui, "No activations yet");
    let count = Rc::new(Cell::new(0u32));
    let group = window.ui.group_create();
    window.set_group(Some(group));
    let icon_source = arrow_source()?;
    for index in 0..6 {
        let tile = Button::create(&mut window.ui, grid).obj();
        window.ui.obj_set_grid_cell(
            tile,
            GridAlign::Stretch,
            index % 3,
            1,
            GridAlign::Stretch,
            index / 3,
            1,
        );
        window.ui.obj_set_flex_flow(tile, FlexFlow::Column);
        window
            .ui
            .obj_set_flex_align(tile, FlexAlign::Center, FlexAlign::Center, FlexAlign::Center);
        let icon = Image::create(&mut window.ui, tile);
        icon.set_src(&mut window.ui, icon_source.clone());
        icon.set_rotation(&mut window.ui, index * 450);
        let label = Label::create(&mut window.ui, tile);
        label.set_text_fmt(&mut window.ui, format_args!("Button {}", index + 1));
        let count = count.clone();
        window.ui.obj_add_event_cb(
            tile,
            Rc::new(move |ui, _| {
                count.set(count.get().saturating_add(1));
                icon.set_rotation(ui, icon.rotation(ui) + 450);
                status.set_text_fmt(
                    ui,
                    format_args!("Button {} activated - total {}", index + 1, count.get()),
                );
            }),
            EventCode::CLICKED,
            None,
        );
        window.ui.group_add_obj(group, tile);
    }
    let mut iterations = 0;
    while frames.is_none_or(|n| iterations < n) && window.step()? {
        iterations += 1;
        std::thread::sleep(Duration::from_millis(16));
    }
    println!(
        "{iterations} event/render iterations, {} presentations",
        window.presented_frames()
    );
    window.ui.deinit();
    Ok(())
}
