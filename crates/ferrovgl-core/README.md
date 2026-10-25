# ferrovgl-core

Core crate of [ferrovgl](https://github.com/jamongadejoa28/rLVGL), a pure-Rust
reimplementation of LVGL v9.6.0. `#![no_std]` with `alloc`.

Not affiliated with LVGL LLC. LVGL is a trademark of LVGL LLC.

## Object snapshots

With the `snapshot` feature and a registered draw unit, `Ui::snapshot_take(obj,
ColorFormat::Argb8888)` renders an object and its descendants into an owned
`DrawBuf`. It updates layout and includes the extended drawing area (for example,
shadows); a display refresh is not required first. `snapshot_create_draw_buf`,
`snapshot_reshape_draw_buf` and `snapshot_take_to_draw_buf` support allocation
reuse. `Drop` replaces C's deprecated snapshot-free API.

The source display and pending render queues are restored after rendering.
Nested calls, callback deletion and stalled queues have regression coverage.
`snapshot_take_to_draw_buf` actually reshapes to the requested color format,
following the C header's documented contract rather than its implementation's
old-format stride mismatch. Failure may leave partially rendered pixels.
Supported snapshot format values do not guarantee that the registered renderer
implements every format; the software renderer's packed ARGB formats are still
incomplete. See the module docs and [snapshot example](../ferrovgl-test/examples/object_snapshot.rs).

## Event and image ownership

Event descriptors support external data through
`Ui::event_desc_set_external_data`: a shared `Rc<dyn Any>` and an optional
release callback. Running callbacks see a retained `Event::ext_data`; descriptor
removal preserves LVGL's release-before-deferred-cleanup order. Callback addition
followed by nested object/display/input deletion has regression coverage.

`DrawBuf::to_image()` returns `Some(ImageDsc)` only for immutable static pixel
storage; other storage returns `None`. Use `ImageSrc::DrawBuf(Rc::new(buffer))`
for an owned/shared/mutable buffer so queued drawing retains the pixels.
Custom C draw-buffer allocator/cache/stride callbacks are not implemented.
