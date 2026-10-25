# ferrovgl

A pure-Rust reimplementation of [LVGL](https://lvgl.io) v9.6.0: the object
tree, style system, flex/grid layouts, software renderer and widgets, rewritten
in `#![no_std]` Rust with a safe public API. No LVGL C code is compiled or
linked; the desktop backend uses the platform SDL2 library.

> ferrovgl is an independent project and is not affiliated with, endorsed by,
> or sponsored by LVGL LLC. LVGL is a trademark of LVGL LLC. See
> [TRADEMARK.md](TRADEMARK.md).

**Status: 0.1.0 release candidate (pre-alpha API).** All 13 v0.1 widgets
(label, button, image, bar, slider, switch, checkbox, arc, line, led, spinner,
buttonmatrix, textarea), the default and simple themes, the observer system,
BiDi/Arabic text, flex/grid, scrolling, all input device types, snapshots and the
software renderer for every colour format upstream's SW renderer supports are
implemented. Backends: SDL2 desktop simulator and a Canvas2D browser backend.
132 upstream `lv_example_*` functions are ported (`ferrovgl-examples`).

**Pixel parity:** every in-scope upstream reference image matches byte-exactly
(zero RGB tolerance): **472/472** in scope, 474 compared in total, out of 976
reference images in LVGL v9.6.0. Out-of-scope images (widgets planned for v0.2,
vector graphics/SVG, FreeType and other font engines, image decoder libraries,
file-system features, tests compiled out by the reference configuration) are
listed with reasons in [`parity_scope.toml`](crates/ferrovgl-test/parity_scope.toml);
`cargo xtask parity-report` prints the coverage. See the
[progress record](docs/src/status.md), the [parity notes](docs/src/parity.md)
and the generated [LVGL to ferrovgl API mapping](docs/src/mapping.md).

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

With SDL2 development libraries installed, run the desktop demo or any ported
upstream example:

```sh
uv run --locked cargo run --locked -p ferrovgl-sdl --example interactive
uv run --locked cargo run --locked -p ferrovgl-sdl --example lv_examples -- --list
uv run --locked cargo run --locked -p ferrovgl-sdl --example lv_examples -- lv_example_flex_align
```

The browser demo (widget gallery and example picker) is described in
[crates/ferrovgl-wasm/README.md](crates/ferrovgl-wasm/README.md).

The demo uses real buttons and shared image buffers; activation rotates the
button's arrow and updates its ASCII-only status label.

Default builds omit debug information and retain incremental compilation to
reduce `target` size while preserving fast edits. `--profile dev-debug` restores
full debug information when needed; see [build cache guidance](CONTRIBUTING.md#build-cache-and-disk-use).

## Goals

- Track LVGL v9.6.0 semantics faithfully. Algorithms are ported 1:1 so that
  rendering is byte-identical to upstream's reference images
  (`tests/ref_imgs`, 800x480 XRGB8888). Parity is the headline metric.
- Rust-idiomatic API with a 1:1 concept mapping: one `Ui` context owns every
  object, objects are `Copy` handles, and `lv_obj_set_width(obj, 100)` becomes
  `ui.obj_set_width(obj, 100)`; widget functions live on their handles
  (`Label::create(&mut ui, parent)`, `label.set_text(&mut ui, "Hi")`).
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
| `ferrovgl-wasm` | no | Canvas2D browser backend and demo |
| `ferrovgl-examples` | yes | ports of the upstream `lv_example_*` functions |
| `ferrovgl-embedded` | yes | placeholder for the v0.2 embedded-graphics / embedded-hal adapters |
| `ferrovgl-test` | no | headless display, synthetic input and screenshot comparison |
| `ferrovgl` | umbrella | re-exports everything behind feature flags |

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Derived from LVGL (MIT, Copyright
LVGL Kft); see [NOTICE](NOTICE).
