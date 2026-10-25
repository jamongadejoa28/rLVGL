# ferrovgl-wasm

Part of [ferrovgl](https://github.com/jamongadejoa28/rLVGL) (independent project, not affiliated with
LVGL LLC). API documentation: <https://docs.rs/ferrovgl-wasm>.

Browser backend for ferrovgl. A display is rendered by the pure Rust
software renderer and copied into a `<canvas>` with Canvas2D
`ImageData`/`putImageData`; pointer, keyboard and wheel events drive real
LVGL input devices; a `requestAnimationFrame` loop feeds `lv_tick_inc()` from
the frame timestamps and runs `lv_timer_handler()`.

The demo page shows a gallery of all 13 v0.1 widgets on the default theme
and a picker that runs the
ported `lv_example_*` functions.

## Build and serve the demo

From the workspace root (`wasm32-unknown-unknown` target and
[wasm-pack](https://rustwasm.github.io/wasm-pack/) installed):

```sh
wasm-pack build crates/ferrovgl-wasm --target web --release --out-dir www/pkg -- --locked
python3 -m http.server 8000 --directory crates/ferrovgl-wasm/www
# open http://localhost:8000/
```

Any static file server works; the page only needs `index.html`, `main.js`
and the generated `pkg/` directory (git-ignored). ES modules and `.wasm`
are not loaded from `file://` URLs, so a server is required. wasm-pack
runs `wasm-opt -Os` (it downloads binaryen once); add `--no-opt` when
offline.

URL parameters of the demo page:

| Parameter | Values | Default |
|---|---|---|
| `scene` | `gallery` or an example name (`lv_example_...`) | `gallery` |
| `w`, `h` | display size in pixels | `800`, `480` |
| `cf` | `xrgb8888`, `argb8888`, `rgb888`, `rgb565` | `xrgb8888` |
| `mode` | `partial`, `direct`, `full` | `partial` |
| `dpi` | `lv_display_set_dpi()` value | `LV_DPI_DEF` (130) |
| `scale` | device pixels per display pixel | `round(devicePixelRatio)` |

For example `?mode=direct&cf=rgb565&scale=1`.

Controls: click/touch/drag the widgets, click the canvas and type into the
text area, Tab / Shift+Tab (or PageDown / PageUp) move the focus, Enter
activates, arrows change sliders, the mouse wheel rotates the encoder and
the middle button presses it.

## JavaScript API

```js
import init, { WebDisplay, scene_names, example_count, start_demo } from "./pkg/ferrovgl_wasm.js";
await init();
// canvas id, width, height, [color format], [render mode], [dpi]
const display = new WebDisplay("ferrovgl", 800, 480, "xrgb8888", "partial", 130);
display.show("gallery");     // or an lv_example_* name: fresh Ui per scene
display.start();             // listeners + requestAnimationFrame loop
display.set_pixel_scale(1);  // CSS size = width * scale / devicePixelRatio
display.step(performance.now()); // manual frame, without rAF
display.stop(); display.free();
```

`start_demo(canvasId, scene?)` does the first three steps for an 800x480
display.

## Design

Only `src/web.rs` touches the DOM. Everything else is plain Rust and is
tested on the host with `cargo test -p ferrovgl-wasm`:

- `backend`: `Backend` owns the `Ui`, the display (single or double
  buffered, any render mode), the flush driver, the input devices and the
  default focus group (`lv_group_set_default()`, attached to the keypad and
  encoder). `frame(now_ms, events)` runs `lv_tick_inc()`, the queued input
  events and `lv_timer_handler()`. Pixels leave through the `Surface` trait.
  The flush layout (render mode, color format, stride) is sampled right
  before each refresh, so an application may switch render modes at run
  time; rotation and display offsets are reported as errors.
- `convert`: locates the flushed area like `refr_area()` does (partial mode:
  buffer reshaped to the area with an automatic stride; direct/full mode:
  screen-sized buffer, area at its own coordinates) and converts it to RGBA.
  XRGB8888/RGB888/RGB565 become opaque; ARGB8888 keeps straight alpha.
  RGB565 is expanded with upstream's `* 2106 >> 8` / `* 1037 >> 8`.
- `geometry`: the canvas backing store is exactly the display size. Pointer
  positions are mapped with `getBoundingClientRect()` and truncated to
  `int16_t` like `lv_sdl_mouse.c`; the CSS size is
  `width * pixel_scale / devicePixelRatio` and is re-applied on `resize`
  (browser zoom and moving between screens change the ratio).
- `keys`: `keycode_to_ctrl_key()` of `lv_sdl_keyboard.c` for
  `KeyboardEvent.key`/`code` (arrows, Enter, Escape, Backspace, Delete,
  Tab/PageDown -> `LV_KEY_NEXT`, PageUp -> `LV_KEY_PREV`, Home, End, keypad
  `+`/`-` -> right/left), plus the `SDL_TEXTINPUT` part: one printable
  character, packed as UTF-8 bytes into a native `u32`. Shift+Tab sends
  `LV_KEY_PREV` (as in `ferrovgl-sdl`); Ctrl/Meta shortcuts do not type;
  IME composition events are skipped.
- `input`: event-mode pointer, keypad and encoder devices. Pointer down/move
  /up, `pointerleave`/`pointercancel` (release in place, like
  `SDL_WINDOWEVENT_LEAVE`), keys with a synthetic release, wheel steps of
  `+-1` per event (upstream's `__EMSCRIPTEN__` branch of
  `lv_sdl_mousewheel.c`), middle button = encoder button, canvas blur
  releases pointer and encoder.
- `clock`: whole milliseconds of `performance.now()` deltas for
  `lv_tick_inc()`, carrying the fraction to the next frame.
- `gallery`: the widget gallery and the scene list.

Browser events are queued by the listeners and applied at the start of the
next animation frame, so a listener never re-enters the `Ui`.

## Tests

```sh
cargo test -p ferrovgl-wasm --locked          # host unit + integration tests
cargo build -p ferrovgl-wasm --target wasm32-unknown-unknown --locked
node crates/ferrovgl-wasm/scripts/node-smoke.mjs   # after the wasm-pack build
FERROVGL_WASM_DUMP=gallery.ppm cargo test -p ferrovgl-wasm dump_gallery
```

The host tests render through a recording surface: every render mode and
color format fills the canvas, only invalidated areas are put, the gallery
is byte-identical in partial, direct and full mode, and pointer, keypad,
encoder, focus group, tick and display-teardown behaviour are checked.

`scripts/node-smoke.mjs` instantiates the real wasm-pack output in Node with
a minimal fake DOM (no npm packages): it renders the gallery through
`putImageData`, runs the rAF loop, clicks the button, sends keys and wheel
events and tears the display down.

Sizes (wasm-pack `--release`, gallery + all widgets, Montserrat 14):
`ferrovgl_wasm_bg.wasm` 953,755 bytes after `wasm-opt -Os` (368,597
gzipped), 1,162,247 bytes without `wasm-opt`.

Not implemented yet: multi-touch, IME composition, canvas resizing of the
display, rotation/offsets and the remaining color formats.

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
