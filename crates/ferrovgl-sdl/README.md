# ferrovgl-sdl

A fixed-size SDL2 window for the pure Rust renderer. The backend owns a `Ui`,
installs the software draw unit, and displays a single full-size XRGB8888 buffer
in direct mode. Mouse, keyboard and wheel events feed real pointer, keypad and
encoder devices; close, expose and focus loss are handled.

Install the SDL2 development library for your platform, then run from the
workspace root:

```sh
uv run --locked cargo run --locked -p ferrovgl-sdl --example interactive
```

`interactive` uses real Button, Image and Label widgets, flex/grid and a focus
group. Click a button to rotate its arrow by 45 degrees, use Tab/Shift+Tab or
the wheel to change focus, and press Enter/middle-click to activate. Buttons
use the default theme's pressed and keyboard-focus states. The six images
share one owned ARGB buffer.
The activation status uses an ASCII hyphen (`activated - total`) because the
bundled Montserrat 14 subset has no em-dash glyph (U+2014).
`hello` remains the static rounded-rectangle demo. For a bounded,
headless smoke run:

```sh
SDL_VIDEODRIVER=dummy uv run --locked cargo run --locked -p ferrovgl-sdl --example interactive -- --frames 3
```

`--frames` limits event/render loop iterations. SDL presentations happen when a
dirty area is flushed, so an unchanged scene may be presented only once.

`SdlDisplay::new(title, width, height, config, theme_hook)` creates a window.
Use its public `ui` to create objects, and call `step()` on the creating thread.
`step()` processes close/expose events, advances ticks from a monotonic clock,
runs timers, and refreshes pending changes. It returns `false` on quit or display
deletion, and reports SDL presentation failures as errors. The caller controls
frame pacing; the example sleeps for 16 ms between iterations. Call
`display.ui.deinit()` when normal LVGL deletion callbacks are needed on exit.

This first backend uses SDL's software renderer and creates a scoped streaming
texture for each flush. It does not resize the framebuffer or support multiple
buffers, other pixel formats, rotation, or display offsets. Those configuration
changes are rejected by `step()`. The texture copy ignores the unused X byte so
XRGB pixels remain opaque. The crate adds no unsafe code.

The SDL smoke test starts an isolated process with the dummy video driver. It
checks the renderer's RGB readback, expose redraw, quit handling and teardown:

```sh
uv run --locked cargo test --locked -p ferrovgl-sdl
```

`pointer()`, `keypad()` and `encoder()` expose the UI-owned input handles.
`set_group(Some(group))` attaches keyboard/wheel navigation to your group.
The SDL adapters follow upstream event mode, including synthetic key release,
native-u32 UTF-8 key packing and `-wheel.y` encoder steps. Shift+Tab navigation
and releasing mouse/encoder on focus loss are host conveniences beyond the
upstream mappings. Touch/multi-touch, IME composition and resizing are not yet
implemented.

Input regression tests inject real SDL mouse, control-key and wheel events in
the isolated dummy process. TextInput is fed through the same adapter directly:
sdl2 0.37's safe `push_event` cannot serialize that event type. The tests also
cover UTF-8 keys, window filtering, focus loss and UI deletion inside an input
callback. Human GUI interaction is not part of this automated verification.
