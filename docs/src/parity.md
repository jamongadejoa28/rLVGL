# Pixel parity with upstream

The parity goal is to render the scenarios of upstream's test cases
(`tests/src/test_cases/*.c`) and compare the result against upstream's
reference images (`tests/ref_imgs/**/*.png`) byte for byte.

## Upstream harness parameters (verified against lvgl v9.6.0, commit 80ca777)

Source: `src/debugging/test/lv_test_display.c`,
`src/debugging/test/lv_test_screenshot_compare.c`, `tests/src/lv_test_init.h`,
`tests/configs/*.defconfig`.

| parameter | value |
|---|---|
| resolution | 800 x 480 (`LV_TEST_DISPLAY_HOR_RES/VER_RES`) |
| display colour format | `LV_COLOR_FORMAT_XRGB8888` |
| render mode | `LV_DISPLAY_RENDER_MODE_DIRECT`, single full-size buffer |
| flush callback | no-op (`lv_display_flush_ready` immediately) |
| draw buffer alignment | `LV_DRAW_BUF_ALIGN = 852` in tests (deliberately not a power of two) |
| DPI | **130** (`run_tests.defconfig`, applied after `depth_32.defconfig`) |
| default font | Montserrat 14 (all Montserrat sizes compiled in) |
| `LV_USE_FLOAT` | yes (`lv_value_precise_t = float`) |
| `LV_USE_MATRIX` | yes |
| `LV_USE_DRAW_SW_COMPLEX_GRADIENTS` | yes |
| `LV_DRAW_SW_SHADOW_CACHE_SIZE` | 8 |
| `LV_CACHE_DEF_SIZE` | 10 MiB |
| `LV_OBJ_STYLE_CACHE` | yes |
| compare rule | per pixel, R/G/B channels each must match exactly (`REF_IMG_TOLERANCE = 0`); alpha ignored; the first mismatching pixel fails the image |
| before compare | `lv_obj_invalidate(lv_screen_active()); lv_refr_now(NULL);` |
| on failure | upstream writes `<name>_err.png`; ferrovgl writes `target/parity/<name>_{actual,expected,diff}.png` |

PNG reference images are RGBA; upstream converts the display buffer to
XRGB8888 (R,G,B,A byte order after `buf_to_xrgb8888`) before comparing.

## Status

The headless harness now exists in `ferrovgl-test`: `TestDisplay` owns a real
`Ui`, software draw unit and XRGB8888 direct-mode display. Integration tests
render rounded objects and Montserrat 14 text, update styles, delete objects,
and inspect the RGB framebuffer. `TestDisplay::with_default_theme()` adds the
light theme for upstream scenarios; `new()` remains unthemed. `create_inputs()`
installs timer-driven pointer, keypad and encoder devices; `SyntheticInput` also
supports button points.
Its safe buffer allocation uses the Rust draw-buffer defaults rather than
the upstream test-only 852-byte alignment setting.

`TestDisplay::screenshot_compare("widgets/example.png")` refreshes the display
and compares decoded RGB bytes exactly, ignoring alpha and row padding. A
missing reference, mismatching dimension, or single-channel difference fails.
Failures write nested `target/parity/<name>_{actual,expected,diff}.png` artifacts
where the expected image is available, plus a `.result` file recording
`matched`, `mismatch`, or `error`. Success removes stale diff images.

`uv run --locked cargo xtask parity-report` counts these stored results
recursively. No recorded comparisons means **0 run, parity unverified**, not
a passing suite. The report is the latest stored result per scenario and may
contain old runs; clear `target/parity` before a fresh complete run. Reference
images are fetched with `uv run --locked cargo xtask refimgs sync` (sparse
checkout, pinned to commit `80ca777`).

**2026-10-25: eighty-two reference images pass RGB byte-exact comparison.**
Five label scenes fulfilled M1; M2 adds nine flex, two grid, thirteen drawing,
nineteen additional animated-label frames, two scrollbar, two style-alignment
and four snapshot images. Button/theme scenes add five images, and Image
widget transforms/alignment add twenty-one. Frames use real objects, layouts,
Label, Button and Image widgets and the default theme.
Drawing tests reproduce the upstream primitive commands through callbacks or
offscreen layers; line, arc and canvas widget APIs remain unported.
Reference PNGs are unchanged.

| Reference | Covered behavior |
|---|---|
| `widgets/label_no_leading_space.png` | automatic wrapping removes leading spaces |
| `widgets/label_preserve_indent_after_newline.png` | explicit newline indentation |
| `widgets/label_scroll_0.png` | two clipped labels and the initial animated-scroll frame |
| `widgets/label_scroll_{1..19}.png` | Scroll/ScrollCircular text changes every 783 ms preserve animation position |
| `widgets/label_wrap_clip.png` | vertical clipping with a border |
| `widgets/label_recolor.png` | content sizing and inline text colour |
| `flex_grow.png` | 45 tracks combining grow ratios and fixed widths |
| `align_flex_center{1,2}.png` | centering one/two children |
| `align_flex_space_around{1,2}.png` | space-around rounding with one/two children |
| `align_flex_space_between{1,2}.png` | space-between with one/two children |
| `align_flex_space_evenly{1,2}.png` | space-evenly with one/two children |
| `grid_fr.png` | fractional tracks and remainders across the screen |
| `margin_grid_0.png` | centered/stretched cells with asymmetric margins |
| `widgets/line_{1,2}.png` | source-derived line-widget geometry, dashed/rounded/polyline primitive rendering |
| `draw/render/xrgb8888/demo_render_arc_{normal,image}_opa_{255,128}.png` | 64 arcs per image: wrap, quadrants, widths, caps, full rings and source image |
| `draw/draw_blur_corner_{0,4,16,32}.png` | four radii across L8/RGB565/RGB888/XRGB8888/ARGB8888 offscreen layers |
| `widgets/canvas_2.png` | twelve triangle orientations in a 500×500 RGB888 layer |
| `widgets/canvas_dropshadow.png` | A8 silhouette, blur and parent-layer composition |
| `widgets/canvas_nested_sub_layer.png` | drop shadow inside an ARGB child layer, then parent composition |
| `scrollbar_{horizontal,vertical}.png` | scrollbar geometry for content extents from 100 to 4,000,000 pixels |
| `margin_align_{0,1}.png` | margin-aligned objects before/after radius, border and padding style resets |
| `snapshot_{0,1,2,3}.png` | owned snapshots displayed through real Image widgets; Montserrat 28 labels, rotation, child coverage, hidden and opacity |
| `scroll_{initial,after}.png` | real Button press-lock, drag scrolling and themed shadow |
| `widgets/obj_pos_{no,fixed,content}_min_size.png` | content-sized parent, percentage Button and minimum style sizes |
| `widgets/image_{rotate,scale,scale_x,scale_y,rotate_and_scale}_pivot_{center,top_left}.png` | Image rotation/scaling with two pivot positions, ten scenes |
| `widgets/image_{normal,transform}_align{,_offset}.png` | nine alignments, transforms and offsets, four scenes |
| `widgets/image_symbol_normal_align{,_offset}.png` | font symbol alignment/offset, two scenes |
| `widgets/image_{stretch,contain,contain_downscale,cover,tile}.png` | automatic scaling and tiling, five scenes |

```sh
uv run --locked cargo test --locked -p ferrovgl-test -- --include-ignored
uv run --locked cargo xtask parity-report
```

The 52 upstream test functions are ignored in ordinary workspace runs because
a fresh checkout has no upstream assets. The parity command and CI job run them
and fail on missing assets. Several functions compare multiple frames; the
animated-label test reuses label_scroll_0 and the button drag test revisits
scroll_initial, so there are 84 comparison calls and 82 distinct reference PNGs.
Eighty-two matched images do not imply all 976
upstream PNGs pass or establish RTL text or all input/scroll image parity.
State transitions, transform invalidation, shared styles, input events and
callback deletion also have behavioral/framebuffer regressions.

`test_style.c` itself has no screenshot assertions. The new style PNGs come
from `test_margin_align.c`; default-theme PNGs involving unported widgets remain
unverified. The current theme subset additionally checks dark/light palettes,
DPI rounding, size boundaries and selector order against C. Button tests cover
keyboard focus, checked/disabled state, press growth and delayed release.
Simple-theme framebuffer tests cover only implemented classes; these are
behavioral regressions, not additional upstream reference images. Snapshot PNGs
now use the actual Image widget to retain and redisplay the snapshot buffer.

Image tests decode the pinned `test_img_lvgl_logo.png` fixture into an ARGB
buffer using the test harness's PNG dependency. Its bytes match the C asset;
source and SHA-256 are recorded in `parity_image.rs`. Production file/PNG
decoders are not implemented. Symbol scenes use the configured bitmap font.

The arc image fixture is extracted byte-for-byte from the pinned upstream
`demos/render/assets/img_render_arc_bg.c`, with its commit and SHA-256 recorded
in `parity_arc.rs`; no reference-image pixels are used as scene inputs. Blur
also has 25 small C-derived byte-hash fixtures across five formats and quality,
radius and corner combinations. This is selected coverage, not complete
renderer or widget fidelity.

Already verified byte-exact against upstream data (M0):

- `misc::math`: `trigo_sin`, `cubic_bezier`, `sqrt32`, `atan2`, `map` against
  the expectations of `tests/src/test_cases/test_math.c`.
- `misc::area`: `Area::diff` against `test_area.c`, `coord::pct` encoding.
- `misc::color`: conversions and `lighten`/`darken` against `test_color.c`.
- `font::fmt_txt`: glyph metrics, class kerning and 4-bpp bitmaps of
  Montserrat 14 against the C arrays; the RLE-compressed Montserrat 28 decodes
  to the same bitmaps as the plain Montserrat 28.

## Reference DPI correction during M2

The original M1 audit stopped at `depth_32.defconfig` (DPI 160). Upstream
`tests/main.py` builds test configuration lists ending in `run_tests.defconfig`;
that final file explicitly states that reference images were rendered at DPI 130.
The shared harness now uses 130. The five label images still match; the grid
fractional-track scene exposed the error through 245 scrollbar pixels. Correcting
the common configuration removes the mismatch without touching rendering or
reference pixels. This supersedes the historical DPI statement in the M1 record.
