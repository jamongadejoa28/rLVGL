//! Run one of the ported LVGL `lv_example_*` functions in an SDL window.
//!
//! ```sh
//! cargo run -p ferrovgl-sdl --example lv_examples -- --list
//! cargo run -p ferrovgl-sdl --example lv_examples -- lv_example_get_started_button
//! cargo run -p ferrovgl-sdl --example lv_examples -- get_started_button --frames 60
//! ```
//!
//! The window is the upstream 800x480 test configuration (130 DPI,
//! Montserrat 14, default theme). For headless execution set
//! `SDL_VIDEODRIVER=dummy` and bound the loop with `--frames N`.
//!
//! `lv_example_style_gradient_{buttons,conic,radial}` follow
//! `LV_USE_DRAW_SW_COMPLEX_GRADIENTS` through the `complex-gradients` feature
//! of `ferrovgl-examples`. This crate enables it neither there nor on its
//! `ferrovgl-draw-sw`, so a `cargo run -p ferrovgl-sdl` build shows the
//! upstream `#else` label "LV_USE_DRAW_SW_COMPLEX_GRADIENTS is not enabled";
//! a build that unifies both features (e.g. a `--workspace` build, which also
//! compiles `ferrovgl-test`) renders the gradients.

use std::time::Duration;

use ferrovgl_core::{FontRef, UiConfig};
use ferrovgl_examples::{EXAMPLES, Example};
use ferrovgl_sdl::SdlDisplay;
use ferrovgl_widgets::theme;

const USAGE: &str = "usage: lv_examples --list | lv_examples <name> [--frames N]\n\
    <name> is an upstream function name, with or without the `lv_example_` prefix.";

/// What the command line asks for.
enum Command {
    List,
    Help,
    Run {
        example: &'static Example,
        frames: Option<u32>,
    },
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let first = args.next().ok_or(USAGE)?;
    match first.as_str() {
        "--list" | "-l" => {
            if args.next().is_some() {
                return Err(USAGE.into());
            }
            return Ok(Command::List);
        }
        "--help" | "-h" => return Ok(Command::Help),
        _ => {}
    }
    let example = ferrovgl_examples::find(&first)
        .ok_or_else(|| format!("unknown example `{first}`; run with --list to see the ported examples"))?;
    let frames = match args.next().as_deref() {
        None => None,
        Some("--frames") => {
            let n: u32 = args
                .next()
                .ok_or("--frames requires a count")?
                .parse()
                .map_err(|_| "invalid frame count")?;
            if n == 0 {
                return Err("--frames requires a count > 0".into());
            }
            Some(n)
        }
        Some(_) => return Err(USAGE.into()),
    };
    if args.next().is_some() {
        return Err(USAGE.into());
    }
    Ok(Command::Run { example, frames })
}

fn main() -> Result<(), String> {
    let (example, frames) = match parse(std::env::args().skip(1))? {
        Command::List => {
            for example in EXAMPLES {
                println!("{}", example.name);
            }
            return Ok(());
        }
        Command::Help => {
            println!("{USAGE}\nClose the window to exit.");
            return Ok(());
        }
        Command::Run { example, frames } => (example, frames),
    };

    let mut window = SdlDisplay::new(
        &format!("ferrovgl - {}", example.name),
        800,
        480,
        UiConfig {
            dpi_def: 130,
            font_default: Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14)),
            ..UiConfig::default()
        },
        Some(theme::default::init_display),
    )?;
    (example.run)(&mut window.ui);

    let mut iterations = 0;
    while frames.is_none_or(|n| iterations < n) && window.step()? {
        iterations += 1;
        std::thread::sleep(Duration::from_millis(16));
    }
    println!(
        "{}: {iterations} event/render iterations, {} presentations",
        example.name,
        window.presented_frames()
    );
    window.ui.deinit();
    Ok(())
}
