# ferrovgl-test

Headless integration and RGB screenshot comparison for the pure Rust renderer.
`TestDisplay::new()` creates an 800×480 XRGB8888 display in direct mode with
one full-size buffer, DPI 130, Montserrat 14, and the software draw unit.
`with_resolution()` provides smaller displays for focused tests.

`TestDisplay::new()` leaves the display unthemed for explicit styling.
`TestDisplay::with_default_theme()` installs the light theme's screen, base
object and button styles. Every constructor creates the pointer, keypad and
encoder devices right after the display, in `lv_test_indev_create_all()` order;
`create_inputs()` returns handles to those same devices. Label parity
tests use real widgets from `ferrovgl-widgets`; a separate low-level glyph
regression still tests an object's `DRAW_MAIN` callback.

```rust
use ferrovgl_core::{Color, StyleSelector};
use ferrovgl_test::TestDisplay;

let mut display = TestDisplay::new()?;
let screen = display.screen().expect("active screen");
display.ui.obj_set_style_bg_color(screen, Color::WHITE, StyleSelector::DEFAULT);
display.ui.obj_set_style_bg_opa(screen, 255, StyleSelector::DEFAULT);
let image = display.snapshot()?;
assert_eq!(image.pixel(0, 0), Some([255, 255, 255]));
# Ok::<(), std::io::Error>(())
```

`snapshot()` invalidates the screen, runs the real refresh pipeline and returns
RGB bytes. `wait(ms)` advances the deterministic clock and timers once per
millisecond, then refreshes even for `wait(0)`. `screenshot_compare("widgets/example.png")` compares a new frame
against the pinned upstream PNG. It never creates or replaces references.

Comparison ignores alpha, as upstream does, and has zero RGB tolerance. A single
different channel value fails. A missing reference or different dimensions also
fails. Differences produce `target/parity/widgets/example_{actual,expected,diff}.png`.
The diff is white for every differing pixel and black elsewhere, so one-bit
errors remain visible. `example.result` starts with `matched`, `mismatch` or
`error`. It describes the latest stored comparison, not necessarily a comparison
performed by the current test command. Successful reruns delete obsolete PNG
failure artifacts. Comparison unit tests use temporary directories and do not
contribute to the upstream parity report.

Changing the display through the public `ui` field is allowed. As upstream
`lv_test_display_create()` does, a colour-format or resolution change reallocates
the draw buffer, and `screenshot_compare` converts every format upstream's
`buf_to_xrgb8888` handles (RGB565, RGB565 swapped, ARGB8888, premultiplied
ARGB8888, XRGB8888, RGB888, L8, AL88, I1). Unsupported formats or a malformed
buffer header return an error before comparing.
An attempted screenshot is marked incomplete before rendering, so rendering
failure cannot retain a previous `matched` result.

Generate a standalone first frame, without claiming upstream parity:

```sh
uv run --locked cargo run --locked -p ferrovgl-test --example first_frame -- target/first-frame.png
```

The example styles normal objects, uses real label widgets and Montserrat 14,
and exports a real 800×480 frame.

## Verified upstream scenes

These five scenes pass **RGB byte-exact parity**. Each reference lives under
`tests/upstream/lvgl/tests/ref_imgs/widgets/`. Their source is
`tests/upstream/lvgl/tests/src/test_cases/widgets/test_label.c`. Each listed
function cleans the screen itself; porting that cleanup avoids the four labels
created by the C file's shared `setUp()`.

| Reference | C scenario | Required behavior |
|---|---|---|
| `label_no_leading_space.png` | `test_label_no_leading_space_after_line_wrap` | 80×100 green label, `LongWord not!`, wrapping drops a leading space |
| `label_preserve_indent_after_newline.png` | `test_label_preserve_spaces_after_explicit_newline` | 200×120 green label, explicit newlines and indentation retained |
| `label_scroll_0.png` | `test_label_long_mode_clip` | Two 150-pixel-wide labels at (10,10)/(10,80); clip mode reuses the initial scrolling reference without animation timing |
| `label_wrap_clip.png` | `test_label_wrap_mode_clip` | 150×25 label at (10,10), wrap mode, vertical clipping, one-pixel red border |
| `label_recolor.png` | `test_label_with_recolor_cmd` | Automatic content size and `#ff0000 red#` inline recoloring |

All five use the actual label class/state/event implementation and the default
theme's screen/font/text styles. They do not exercise flex/grid, input devices,
custom test fonts, image decoding or RTL text; the result does not establish
parity for those paths.
Expected pixels must only come from the pinned upstream references; actual
pixels must be rendered from the ported C scenario through the Rust pipeline.

Run the current integration and comparison checks from the workspace root:

```sh
uv run --locked cargo test -p ferrovgl-test --locked
uv run --locked cargo xtask refimgs sync
uv run --locked cargo test --locked -p ferrovgl-test -- --include-ignored
uv run --locked cargo xtask parity-report
```

The upstream tests are explicitly ignored in ordinary workspace runs
because the reference checkout is not versioned here. The dedicated command
and CI parity job execute them; missing reference files then fail the tests.

## Synthetic input and layouts

`let inputs = display.create_inputs();` returns the timer-driven pointer, keypad
and encoder devices the constructor created. `TestDisplay` also has the upstream
helpers directly (`mouse_move_to`, `mouse_click_at`, `key_hit`, `encoder_turn`,
`wait`, `fast_forward`, `clean_screen`); `wait` ends with the pointer, encoder and
keypad reads of `lv_test_wait()`. One `TestDisplay` per `#[test]` models the first
test of an upstream suite; see the crate docs for state upstream carries between tests. `inputs.set_group(&mut display.ui, Some(group))` connects focus
navigation. `mouse_click_at`, `key_hit`, `encoder_turn` and `encoder_click` follow
upstream's 50 ms wait sequences. Individual `SyntheticInput` setters update the
next sample; `display.wait(ms)` drives real input timers, long presses and
animations. `SyntheticInput::new` also supports hardware-button points.

The integration suite checks press-state RGB changes, click ordering, focus/key
navigation, one-time encoder steps, button-triggered deletion and drag scrolling.
`parity_flex` and `parity_grid` reproduce upstream layout scenes with the same
reference-image rules; see [the complete parity table](../../docs/src/parity.md).
The reference harness uses **DPI 130**: `run_tests.defconfig`, loaded last by
upstream `tests/main.py`, overrides `depth_32.defconfig`'s 160. The latter is not
the final screenshot configuration.

`parity_scroll` adds the upstream horizontal/vertical scrollbar scenes and
20 animated label frames, with 783 ms between text changes. Frame zero shares
the existing label reference. It also checks rotated-child dragging and deletion
during a scroll animation. `parity_style` covers two margin-alignment PNGs and
behavioral state, transform-invalidation, transition and shared-style regressions.

## Drawing primitives and layers

`parity_line`, `parity_arc`, `parity_blur` and `parity_primitives` add thirteen
upstream images: two line, four arc, four blur, one triangle and two drop-shadow
scenes. These reproduce C draw commands through objects' DRAW_MAIN callbacks
and offscreen buffers. Line, arc and canvas widget APIs remain unported.
The non-ignored nested-shadow regression also requires draw queues to drain,
checks actual shadow pixels and verifies child-layer reclamation.

The harness enables RGB565, RGB888, L8 and A8 alongside the display formats so
standalone parity runs exercise every offscreen format used by these scenes.
Arc source-image bytes come from the pinned C asset, with provenance and hash
in the test; expected PNGs are never used to generate actual frames.

## Object snapshots

`parity_snapshot` compares the four upstream `snapshot_0..3.png` scenes. These
exercise the real snapshot API, Label (Montserrat 28), base objects and Image
widgets. Snapshot buffers are retained by the image source after the original
objects are deleted. The regular `snapshot` suite
checks allocation reuse, formats, transparent margins, nested calls, callbacks,
queue restoration and cleanup after stalls.

Export a real object snapshot with alpha, independently of upstream parity:

```sh
uv run --locked cargo run --locked -p ferrovgl-test --example object_snapshot -- target/object-snapshot.png
```

## Button and image widgets

`parity_button_theme` reproduces upstream press-lock/scroll and minimum-size
scenes with actual Buttons (five distinct PNGs). Its regular tests use synthetic
keyboard activation to check focus, checked/disabled behavior and default-theme
press/release transitions; simple-theme checks inspect rendered colors.

`parity_image` compares twenty-one upstream Image widget scenes: rotation,
independent scales, pivots, nine inner alignments, offsets, automatic sizing,
tiling and font symbols. The pinned logo PNG is decoded only inside the test
harness into an owned ARGB DrawBuf. This is not production PNG/file decoder
support. `image` additionally tests source ownership, invalidation, hit/cover
queries and deletion inside callbacks.
