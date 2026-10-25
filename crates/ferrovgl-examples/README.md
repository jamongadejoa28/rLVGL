# ferrovgl-examples

Ports of the LVGL v9.6.0 `lv_example_*` functions (`examples/` in the
upstream repository) to the ferrovgl API: get started, styles, animations,
events, flex/grid layouts, scrolling and the 13 widgets of ferrovgl 0.1.
`#![no_std]` with `alloc`.

Part of [ferrovgl](https://github.com/jamongadejoa28/rLVGL) (independent project, not affiliated with
LVGL LLC). API documentation: <https://docs.rs/ferrovgl-examples>.

Each example keeps its C name and builds its UI on the active screen of the
default display, exactly like the C function. C `static` variables live in
per-`Ui` storage. `ferrovgl_examples::EXAMPLES` lists them all and
`ferrovgl_examples::find(name)` looks one up; the
[SDL](https://crates.io/crates/ferrovgl-sdl) and
[browser](https://crates.io/crates/ferrovgl-wasm) demos use this crate to run
them. They are also a reading aid: put a Rust example next to the C one to
see how an LVGL program translates.

The `complex-gradients` feature (default) builds the
`LV_USE_DRAW_SW_COMPLEX_GRADIENTS` branch of the gradient examples; enable it
together with the same feature of `ferrovgl-draw-sw`. Examples that need
chart, keyboard, table, `lv_anim_timeline` or large image assets are not
ported yet.

The tests in `tests/` need the unpublished `ferrovgl-test` harness and run
from the repository workspace only.

## About ferrovgl

This crate is part of [ferrovgl](https://github.com/jamongadejoa28/rLVGL), a pure-Rust reimplementation of
LVGL v9.6.0. ferrovgl is an independent project and is **not** affiliated
with, endorsed by, or sponsored by LVGL LLC or LVGL Kft. "LVGL" is a trademark
of LVGL LLC, used here only to describe compatibility and provenance (see
[TRADEMARK.md](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/TRADEMARK.md)).

- Repository: <https://github.com/jamongadejoa28/rLVGL>
- Changelog: [CHANGELOG.md](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/CHANGELOG.md)
- Minimum supported Rust version: 1.97

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/LICENSE-APACHE)
or [MIT license](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/LICENSE-MIT) at your option. ferrovgl is derived from
LVGL (MIT, Copyright LVGL Kft); the upstream notice is reproduced in
[NOTICE](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/NOTICE), which ships in this package.
