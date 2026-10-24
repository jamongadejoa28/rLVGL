# ferrovgl

A pure-Rust reimplementation of [LVGL](https://lvgl.io) v9.6.0: the object
tree, style system, flex/grid layouts, software renderer and widgets, rewritten
in `#![no_std]` Rust with a safe public API. No C code is compiled or linked.

> ferrovgl is an independent project and is not affiliated with, endorsed by,
> or sponsored by LVGL LLC. LVGL is a trademark of LVGL LLC. See
> [TRADEMARK.md](TRADEMARK.md).

**Status: pre-alpha, milestone M0 (scaffolding) done.** Nothing renders yet.
What exists today: the workspace, CI definitions, the `misc` ports (areas,
colours, integer math, generational arena) with tests ported from upstream,
the `.lvfnt` font pipeline (`cargo xtask fonts`) with all 24 built-in bitmap
fonts and a zero-copy reader that decodes plain and RLE-compressed glyphs
identically to LVGL, and a verified description of the upstream pixel-parity
harness (`docs/src/parity.md`).

## Goals

- Track LVGL v9.6.0 semantics faithfully. Algorithms are ported 1:1 so that
  rendering is byte-identical to upstream's reference images
  (`tests/ref_imgs`, 800x480 XRGB8888). Parity is the headline metric.
- Rust-idiomatic API with a 1:1 concept mapping:
  `lv_obj_set_width(obj, 100)` becomes `ui.obj(obj).set_width(100)`.
- `no_std` + `alloc` core, stable Rust, `unsafe` confined to two audited
  modules.
- Desktop simulator (SDL2) and WebAssembly first, embedded HAL backends next.

## Crates

| crate | `no_std` | purpose |
|---|---|---|
| `ferrovgl-core` | yes | object tree, styles, events, layouts, draw model, fonts, input, timers, animation |
| `ferrovgl-draw-sw` | yes | software renderer (port of `src/draw/sw`) |
| `ferrovgl-fonts` | yes | built-in Montserrat and other bitmap fonts |
| `ferrovgl-widgets` | yes | widgets and themes |
| `ferrovgl-sdl` | no | SDL2 window, mouse and keyboard |
| `ferrovgl-wasm` | no | Canvas2D backend for the browser |
| `ferrovgl-embedded` | yes | embedded-graphics / embedded-hal adapters (v0.2) |
| `ferrovgl-test` | no | headless test display and screenshot comparison |
| `ferrovgl` | umbrella | re-exports everything behind feature flags |

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Derived from LVGL (MIT, Copyright
LVGL Kft); see [NOTICE](NOTICE).
