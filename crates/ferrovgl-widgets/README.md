# ferrovgl-widgets

Part of [ferrovgl](https://github.com/jamongadejoa28/rLVGL) (independent project, not affiliated with
LVGL LLC). API documentation: <https://docs.rs/ferrovgl-widgets>.

LVGL v9.6.0 widget classes and themes in `no_std + alloc` Rust. Each widget
is behind a feature of the same name (all on by default): `label`, `button`,
`image`, `bar`, `slider`, `switch`, `checkbox`, `arc`, `line`, `led`,
`spinner`, `buttonmatrix` and `textarea`, plus `theme-default`,
`theme-simple`, `observer`, `float`, `bidi` and `arabic-persian-chars`.
Handles are copyable; state and callbacks belong to `Ui`.

```rust
use ferrovgl_widgets::{Label, LabelLongMode};
# fn example(ui: &mut ferrovgl_core::Ui, parent: ferrovgl_core::Obj) {
let label = Label::create(ui, parent);
label.set_text(ui, "Hello, Rust!");
label.set_long_mode(ui, LabelLongMode::Wrap);
ui.obj_set_width(label.obj(), 200);
# }
```

Labels support wrapping, dots, clipping, back-and-forth/circular scrolling,
recolor commands, selection, maximum lines, owned/static text, formatting,
UTF-8 position queries, insertion and cutting. Text size and refresh follow the
upstream layout-completed callback. Styles/position use the base `Obj` handle.

With the `bidi` and `arabic-persian-chars` features, labels use the ported
`lv_bidi.c` and `lv_text_ap.c`. The `observer` feature adds the subject
bindings (`Label::bind_text`, `Arc`/`Bar`/`Slider::bind_value`,
`Image::bind_src`). Translation (`lv_translation`) is not implemented.
Dotted-label insertion restores the
original text before mutation to preserve valid Rust UTF-8; this intentionally
differs from C's in-place operations that can produce invalid UTF-8. The dots
cutting path preserves C's hidden bytes and restores them without truncation.

Buttons inherit the base object's input and checkable state behavior. Their
class sizes to its contents, joins the default focus group, disables scrolling
and enables scroll-on-focus. Use `Button::create(ui, parent)` and its `obj()`
handle for flags, styles, children and event callbacks.

Images accept owned/shared in-memory `ImageSrc` buffers and symbol text.
`Image::set_rotation` uses tenths of a degree; `set_scale` uses 256 for 100%.
Pivot, offsets, inner alignment, tiling, antialiasing and recoloring follow the
image class. Keep a `DrawBuf` in `Rc` with `ImageSrc::DrawBuf` to retain its
pixels through queued drawing. Production file/PNG decoding, decoder registry
and image property reflection are not implemented. The PNG
loader used by the parity tests belongs only to the test harness. Bitmap masks
are retained and forwarded to draw descriptors; the current direct software
image path, like the pinned C path, does not apply them (layer composition does).

The `theme-default` feature provides `theme::default::init_display`, suitable
for `Ui::display_create_ex`. Configure `UiConfig::font_default` first. It
implements the upstream styles of the screen, cards, scrollbars and the 13
widgets above, light/dark colours, DPI scaling and resolution updates.
Buttons have pressed, checked, disabled and keyboard-focus styles, including
grow and release transitions. Ordinary labels and images receive no separate
theme styles, as upstream. `theme-simple` provides
`theme::simple::init_display` with the upstream fixed-pixel styles. Theme
branches of widgets that are not ported yet (chart, keyboard, table, ...) are
absent.

Default and simple themes are owned per display rather than through C's single global
pointer. Their shared styles remain alive through display deletion callbacks
and can be reused by another display or theme copy. `Ui::deinit` releases all
resources. For repeated display creation/deletion within one context, call
`theme::default::deinit(ui, theme_id)` or `theme::simple::deinit(ui, theme_id)`
after deleting/detaching the display;
it refuses to release a theme still referenced by a display, child theme or
shallow copy. Application theme user data is independent of internal state.

Upstream label, button and image scenes are verified through `ferrovgl-test`;
see [the parity table](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/docs/src/parity.md). The font must be
configured to render labels; the widgets crate does not force a built-in font
into `no_std` consumers.

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
