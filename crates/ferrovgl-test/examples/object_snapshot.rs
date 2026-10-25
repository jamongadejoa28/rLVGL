//! Export one object and its descendants, including transparent shadow margins.

use std::fs::File;
use std::io::{self, BufWriter};
use std::path::PathBuf;

use ferrovgl_core::{Color, ColorFormat, StyleSelector};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::Label;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| "target/object-snapshot.png".into());
    if args.next().is_some() {
        return Err("usage: object_snapshot [output.png]".into());
    }
    let mut display = TestDisplay::with_default_theme()?;
    let screen = display
        .screen()
        .ok_or_else(|| io::Error::other("missing screen"))?;
    let ui = &mut display.ui;
    let card = ui.obj_create(Some(screen));
    let selector = StyleSelector::DEFAULT;
    ui.obj_set_size(card, 360, 140);
    ui.obj_center(card);
    ui.obj_set_style_bg_color(card, Color::hex(0xe3f2fd), selector);
    ui.obj_set_style_border_color(card, Color::hex(0x1565c0), selector);
    ui.obj_set_style_shadow_color(card, Color::hex(0x19334d), selector);
    ui.obj_set_style_shadow_width(card, 16, selector);
    ui.obj_set_style_shadow_opa(card, 120, selector);
    ui.obj_set_style_shadow_offset_y(card, 6, selector);
    let label = Label::create(ui, card);
    label.set_text(
        ui,
        "Object snapshot\n\nA card, its label and shadow.\nRendered offscreen in pure Rust.",
    );
    ui.obj_set_width(label.obj(), 310);
    // No display refresh is needed: the snapshot API updates layout itself.
    let buffer = ui
        .snapshot_take(card, ColorFormat::Argb8888)
        .ok_or_else(|| io::Error::other("snapshot failed"))?;
    let width = usize::from(buffer.header.w);
    let height = usize::from(buffer.header.h);
    let stride = usize::from(buffer.header.stride);
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for pixel in buffer.data()[y * stride..y * stride + width * 4]
            .as_chunks::<4>()
            .0
        {
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(&path)?), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&rgba)?;
    writer.finish()?;
    println!("{}x{} RGBA snapshot: {}", width, height, path.display());
    ui.deinit();
    Ok(())
}
