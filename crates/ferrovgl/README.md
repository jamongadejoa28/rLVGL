# ferrovgl

A pure-Rust reimplementation of [LVGL](https://lvgl.io) v9.6.0, the embedded
GUI library: object tree, style system, events, flex/grid layouts, software
renderer, built-in fonts, widgets and themes, written in `#![no_std]` + `alloc`
Rust with a safe public API. No LVGL C code is compiled or linked.

> ferrovgl is an independent project. It is **not** affiliated with, endorsed
> by, or sponsored by LVGL LLC or LVGL Kft. "LVGL" is a trademark of LVGL LLC
> and is used here only to describe compatibility and provenance. See
> [TRADEMARK.md](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/TRADEMARK.md).

- Repository: <https://github.com/jamongadejoa28/rLVGL>
- API documentation: <https://docs.rs/ferrovgl>
- Changelog: [CHANGELOG.md](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/CHANGELOG.md)

**Status: 0.1, pre-alpha.** Upstream algorithms are ported 1:1 and checked
against LVGL's own reference images with zero RGB tolerance; see the
[parity status](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/docs/src/parity.md)
and the changelog for what is (and is not) implemented. The API will change
before 1.0.

## This crate

`ferrovgl` is the umbrella crate. It re-exports the workspace crates behind
feature flags:

| feature | default | re-export | crate |
|---|---|---|---|
| (always) | yes | `ferrovgl::core` | [`ferrovgl-core`](https://crates.io/crates/ferrovgl-core): object tree, styles, events, layouts, draw model, input, timers, animation |
| `draw-sw` | yes | `ferrovgl::draw_sw` | [`ferrovgl-draw-sw`](https://crates.io/crates/ferrovgl-draw-sw): software renderer |
| `fonts` | yes | `ferrovgl::fonts` | [`ferrovgl-fonts`](https://crates.io/crates/ferrovgl-fonts): Montserrat, UNSCII, DejaVu bitmap fonts |
| `widgets` | yes | `ferrovgl::widgets` | [`ferrovgl-widgets`](https://crates.io/crates/ferrovgl-widgets): 13 widgets, default/simple themes |
| `sdl` | no | `ferrovgl::sdl` | [`ferrovgl-sdl`](https://crates.io/crates/ferrovgl-sdl): SDL2 desktop simulator (needs the SDL2 library, implies `std`) |
| `embedded` | no | `ferrovgl::embedded` | [`ferrovgl-embedded`](https://crates.io/crates/ferrovgl-embedded): placeholder for the v0.2 embedded-graphics adapters |
| `std` | no | | enables `std` in the core, renderer and widgets |

The browser backend ([`ferrovgl-wasm`](https://crates.io/crates/ferrovgl-wasm))
and the ported `lv_example_*` functions
([`ferrovgl-examples`](https://crates.io/crates/ferrovgl-examples)) are separate
crates.

```toml
[dependencies]
ferrovgl = { version = "0.1", features = ["sdl"] }
```

```rust,ignore
use ferrovgl::core::{Color, FontRef, StyleSelector, UiConfig};
use ferrovgl::sdl::{DEFAULT_TITLE, SdlDisplay};
use ferrovgl::widgets::{Label, theme};

fn main() -> Result<(), String> {
    let mut display = SdlDisplay::new(
        DEFAULT_TITLE,
        800,
        480,
        UiConfig {
            font_default: Some(FontRef::Static(&ferrovgl::fonts::MONTSERRAT_14)),
            ..UiConfig::default()
        },
        Some(theme::default::init_display),
    )?;
    let screen = display.screen().ok_or("display has no screen")?;
    let label = Label::create(&mut display.ui, screen);
    label.set_text(&mut display.ui, "Hello, Rust!");
    display
        .ui
        .obj_set_style_text_color(label.obj(), Color::hex(0x1565c0), StyleSelector::DEFAULT);
    while display.step()? {
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    Ok(())
}
```

LVGL concepts map 1:1 onto the Rust API (`lv_obj_set_width(obj, 100)` is
`ui.obj_set_width(obj, 100)`); the generated
[mapping table](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/docs/src/mapping.md)
lists every upstream function.

## Minimum supported Rust version

Rust 1.97 (edition 2024).

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/LICENSE-APACHE) or
[MIT license](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/LICENSE-MIT) at your option. ferrovgl is derived from LVGL (MIT,
Copyright LVGL Kft); the upstream notice is reproduced in [NOTICE](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/NOTICE).
Bundled fonts carry their own licences (see `ferrovgl-fonts`).
