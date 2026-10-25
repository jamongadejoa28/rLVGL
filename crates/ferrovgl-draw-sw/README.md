# ferrovgl-draw-sw

The software renderer of ferrovgl: a 1:1 port of LVGL v9.6.0 `src/draw/sw`
(fill, border, box shadow, gradients including the complex
linear/radial/conic ones, masks, letters, images with transforms, lines,
arcs, triangles, blur and layer composition), `#![no_std]` with `alloc`.

Part of [ferrovgl](https://github.com/jamongadejoa28/rLVGL) (independent project, not affiliated with
LVGL LLC). API documentation: <https://docs.rs/ferrovgl-draw-sw>.

Register the draw unit with a `ferrovgl_core::Ui`; the desktop, browser and
headless backends of ferrovgl do this for you. Each colour format is behind a
`cf-*` feature so that unused blend paths are not compiled on small targets:

| feature | default | meaning |
|---|---|---|
| `complex` | yes | `LV_DRAW_SW_COMPLEX`: rounded corners, masks, shadows, arcs |
| `complex-gradients` | yes | `LV_USE_DRAW_SW_COMPLEX_GRADIENTS` (upstream template default is off; on in the reference-image configuration) |
| `cf-rgb565`, `cf-rgb565-swapped`, `cf-rgb888`, `cf-xrgb8888`, `cf-argb8888`, `cf-argb8888-premultiplied`, `cf-l8`, `cf-al88`, `cf-a8`, `cf-i1` | yes | render/blend support per `lv_color_format_t` |
| `std` | no | `std` support in `ferrovgl-core` |

Embedded builds usually start from `default-features = false` and enable the
display's colour format only. Rendering is checked byte-for-byte against
LVGL's reference images; the only `unsafe` code of the crate is in
`blend::words`.

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
