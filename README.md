# ferrovgl

A pure-Rust reimplementation of [LVGL](https://lvgl.io) v9.6.0: the object
tree, style system, flex/grid layouts, software renderer and widgets, rewritten
in `#![no_std]` Rust with a safe public API. No LVGL C code is compiled or
linked; the desktop backend uses the platform SDL2 library.

> ferrovgl is an independent project and is not affiliated with, endorsed by,
> or sponsored by LVGL LLC. LVGL is a trademark of LVGL LLC. See
> [TRADEMARK.md](TRADEMARK.md).

**Status: pre-alpha, M1 complete and M2 input/layout/rendering work implemented.** Real
Label, Button and Image widgets, their default/simple theme subsets, a headless
harness and a fixed-size SDL display are implemented. Pointer/keypad/encoder/
button processing, focus groups, flex/grid, synthetic inputs and SDL mouse/
keyboard/wheel input now work through the core. Lines, arcs, triangles and blur
are implemented, including nested drop-shadow compositing. Object snapshots
support owned and reusable buffers, and three coverage-guided fuzz targets are
available. Upstream widget, layout, drawing, snapshot and scroll scenes are
compared with zero RGB tolerance. Remaining widgets, theme coverage, advanced gestures and the wasm backend are
unfinished; this is not M2 completion.
See the [progress record](docs/src/status.md) and [parity status](docs/src/parity.md)
for the verified scope and limitations.

## Development

The project-local uv environment launches development commands; Rust
dependencies remain locked by `Cargo.lock`.

```sh
uv sync --locked
uv run --locked cargo test --locked --workspace --all-features
uv run --locked cargo test --locked -p ferrovgl-test
uv run --locked cargo run --locked -p ferrovgl-test --example first_frame
uv run --locked cargo xtask refimgs sync
uv run --locked cargo test --locked -p ferrovgl-test -- --include-ignored
uv run --locked cargo xtask parity-report
```

The headless example writes `target/first-frame.png` (800×480).
To export an individual object with its children and transparent shadow margins:

```sh
uv run --locked cargo run --locked -p ferrovgl-test --example object_snapshot
```

This writes `target/object-snapshot.png` (RGBA). The [fuzzing guide](fuzz/README.md)
describes the separate locked fuzz workspace and bounded local campaigns.

With SDL2 development libraries installed, run the desktop example:

```sh
uv run --locked cargo run --locked -p ferrovgl-sdl --example interactive
```

The demo uses real buttons and shared image buffers; activation rotates the
button's arrow and updates its ASCII-only status label.

Default builds omit debug information and retain incremental compilation to
reduce `target` size while preserving fast edits. `--profile dev-debug` restores
full debug information when needed; see [build cache guidance](CONTRIBUTING.md#build-cache-and-disk-use).

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
| `ferrovgl-sdl` | no | SDL2 display, mouse, keyboard and wheel input |
| `ferrovgl-wasm` | no | Canvas2D backend for the browser (scaffolding) |
| `ferrovgl-embedded` | yes | embedded-graphics / embedded-hal adapters (v0.2) |
| `ferrovgl-test` | no | headless display, synthetic input and screenshot comparison |
| `ferrovgl` | umbrella | re-exports everything behind feature flags |

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Derived from LVGL (MIT, Copyright
LVGL Kft); see [NOTICE](NOTICE).
