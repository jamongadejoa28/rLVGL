# ferrovgl-widgets

The `label`, `button` and `image` features implement LVGL v9.6.0 widget
classes in `no_std + alloc` Rust. Handles are copyable; state and callbacks
belong to `Ui`.

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

Observer bindings, translation, Arabic shaping and BiDi paragraph reordering
are not implemented. The existing core BiDi fallback is LTR/identity; enabling
the feature does not provide RTL parity. Dotted-label insertion restores the
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
pixels through queued drawing. Production file/PNG decoding, decoder registry,
observer bindings and image property reflection are not implemented. The PNG
loader used by the parity tests belongs only to the test harness. Bitmap masks
are retained and forwarded to draw descriptors; the current direct software
image path, like the pinned C path, does not apply them (layer composition does).

The `theme-default` feature provides `theme::default::init_display`, suitable
for `Ui::display_create_ex`. Configure `UiConfig::font_default` first. This
subset implements screen/card/scrollbar/scrolled and button styles, light/dark
colours, DPI scaling and resolution updates. Buttons have pressed, checked,
disabled and keyboard-focus styles, including grow and release transitions.
Ordinary labels and images receive no separate theme styles. `theme-simple`
provides `theme::simple::init_display` with the upstream fixed-pixel screen,
base-object, scrollbar and button styles. Other widget/theme branches remain
scaffolding.

Default and simple themes are owned per display rather than through C's single global
pointer. Their shared styles remain alive through display deletion callbacks
and can be reused by another display or theme copy. `Ui::deinit` releases all
resources. For repeated display creation/deletion within one context, call
`theme::default::deinit(ui, theme_id)` or `theme::simple::deinit(ui, theme_id)`
after deleting/detaching the display;
it refuses to release a theme still referenced by a display, child theme or
shallow copy. Application theme user data is independent of internal state.

Upstream label, button and image scenes are verified through `ferrovgl-test`;
see [the parity table](../../docs/src/parity.md). The font must be
configured to render labels; the widgets crate does not force a built-in font
into `no_std` consumers.
