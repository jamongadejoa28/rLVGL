# API guidelines (0.1)

This page is the API policy of the published crates (`ferrovgl`,
`ferrovgl-core`, `ferrovgl-draw-sw`, `ferrovgl-fonts`, `ferrovgl-widgets`,
`ferrovgl-sdl`, `ferrovgl-wasm`, `ferrovgl-examples`, `ferrovgl-embedded`).
It applies the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)
(the `C-*` items cited below) to a library whose first rule is to keep
LVGL's names, numeric values and behaviour (`docs/PLAN.md` sections 2 and 4).
When the two disagree, the LVGL mapping wins and the reason is written
here. New public items follow these rules; reviewers check them.

The 0.1.0 API as published is the baseline that `cargo-semver-checks` compares
later releases against (CI, after 0.1.0).

## Naming (C-CASE, C-GETTER, C-CONV)

ferrovgl has two API layers (see [the mapping page](mapping.md#using-the-rust-api)),
and they name things differently on purpose:

- **Layer 1, `Ui` methods** (and the methods of the other context types
  `DrawCtx` and `BidiState`): the C function name without `lv_`, verbatim,
  module prefix and `get_` included: `lv_obj_get_width(obj)` is
  `ui.obj_get_width(obj)`, `lv_display_get_dpi(d)` is
  `ui.display_get_dpi(d)`, `lv_draw_task_get_fill_dsc(t)` is
  `ui.draw.task_get_fill_dsc(t)`. Keeping `get_` here is a deliberate
  exception to C-GETTER: the method is greppable from the C name and the
  module prefix (`obj_`, `display_`, `indev_`, `group_`, `timer_`,
  `anim_`, `style_`, `subject_`, `observer_`, `theme_`, `draw_`, `tick_`,
  `delay_`, `refr_`, `snapshot_`, `event_`, `layout_`) groups the
  methods in the documentation. `DrawCtx` is the `lv_draw_*` module itself,
  so its methods drop `draw_` (`lv_draw_layer_create` is
  `DrawCtx::layer_create`, `Ui::draw_layer_create` is the `Ui` wrapper).
  Rust-only accessors without a C counterpart are short nouns: `ui.obj(o)`,
  `ui.obj_mut(o)`, `ui.display(d)`, `ui.style(s)`, `ui.app::<T>()`.
- **Layer 2, widget handles and value types**: methods on `Label`, `Slider`,
  ..., `Event`, `Anim`, `Style`, `Area`, `Color`, `DrawBuf` drop the
  `lv_<module>_` prefix *and* `get_` (C-GETTER): `lv_label_get_text(l)` is
  `l.text(ui)`, `lv_event_get_code(e)` is `e.code()`,
  `lv_anim_get_delay(&a)` is `a.delay()`, `lv_area_get_width(&a)` is
  `a.width()`. Setters keep `set_`. Lookups that take a key keep a
  `get_` + noun name like `HashMap::get` (`Style::get_prop(prop)`).
- Struct fields of the C object (`lv_slider_t`, `lv_buttonmatrix_t`, ...)
  that upstream code reads directly keep the C field name as a getter
  (`Buttonmatrix::btn_cnt`, `Bar::indic_area`).
- Free functions in a module drop the module part of the C name:
  `lv_sqrt()` is `misc::math::sqrt`, `lv_text_get_size()` is
  `misc::text::get_size`.
- Conversions follow C-CONV (`from_u8`, `as_u8`, `to_u32`, `to_hsv`).
- Types are `UpperCamelCase` forms of the C type without `lv_` and `_t`
  (`lv_label_long_mode_t` is `LabelLongMode`), variants of the C constants
  without the type prefix (`LV_LABEL_LONG_MODE_WRAP` is `LabelLongMode::Wrap`),
  and `#define` constants keep their C name without `LV_`
  (`LV_OPA_COVER` is `OPA_COVER`). `lv_result_t` is `LvResult` so it does not
  shadow `core::result::Result`.

Every rename keeps the doc comment that starts with the C name, so
`cargo xtask gen-mapping` still maps it; the mapping table is the
authoritative C-to-Rust index.

## `#[must_use]`

`#[must_use]` goes on a function when ignoring its result is always a
mistake. The candidate list is `clippy::must_use_candidate` (pedantic),
which misses every method taking `&Ui` because `Ui` holds cells, so the
audit also took every public function without a `&mut` argument that
returns a value, from the rustdoc JSON of each crate, and judged the rest.

On:

- pure getters and queries, including the generated style getters
  (`ui.obj_get_style_bg_color(o, part)`), the flag and state getters
  (`ui.obj_is_checked(o)`), widget getters (`label.text(ui)`), the
  `Event`, `Anim`, `Style`, `Area`, `Color`, `DrawBuf` value methods and the
  pure math / text / colour helpers;
- queries that need `&mut Ui` only for a lazy layout update or a hit test
  (`obj_get_self_width`, `obj_get_scroll_bottom`, `obj_hit_test`,
  `obj_area_is_visible`, `indev_search_obj`, `Image::src_width`, ...);
- constructors and builders of plain values (`Area::new`, `Style::new`,
  `RectDsc::new`, `Anim::new`, `Color::hex`, `ConstStyleEntry::bg_color`,
  `FmtTxtFont::with_fallback`, `DrawBuf::create`);
- accessors returning a reference (`ui.obj_mut(o)`, `ui.widget_mut::<T>(o)`,
  `ctx.task_get_fill_dsc(t)`, `e.param()`);
- registrations whose returned id is the only way to reach what was
  registered: `ui.style_create`, `ui.theme_create`, `ui.subject_create`,
  `ui.layout_create` / `layout_register`, `ui.event_register_id`,
  `ui.style_register_prop`, `ui.draw_layer_create`,
  `ctx.add_task`, `ctx.layer_create`, `Arena::insert`;
- status results whose neglect is a bug: `DrawBuf::reshape`,
  `DrawCtx::layer_alloc_buf`, `Font::glyph_dsc`, `Font::glyph_bitmap_a8`,
  the software mask functions (`MaskRes`), and `ui.obj_event_base`, which
  carries a message because a widget's event handler must return when the
  base class reports `LvResult::Invalid`.

Off:

- setters, including the chainable `Anim::set_*(&mut self) -> &mut Self`;
- functions whose point is the side effect even though they return a
  handle: `ui.obj_create`, `Label::create` and the other widget
  constructors, `ui.display_create`, `ui.indev_create`, `ui.group_create`,
  `ui.timer_create`, `ui.anim_start`, `ui.obj_add_event_cb`, the
  `obj_bind_*` observer bindings. The created object or timer lives on in
  the `Ui` and stays reachable (tree walk, `display_get_default`,
  `group_get_default`, ...), and fire-and-forget use is normal LVGL style
  (`Switch::create(ui, screen);`, `lv_timer_create(cb, 500, NULL);`);
- removal and update functions that report whether anything happened
  (`obj_remove_event_cb`, `anim_delete`, `Style::remove_prop`), like
  `HashMap::remove`;
- `LvResult`-returning functions in general (`obj_send_event`,
  `obj_invalidate`): `LvResult` itself is not `#[must_use]`, because
  most callers rightly ignore it; only `obj_event_base` is marked;
- `text::encoded_next` / `encoded_prev`, which C code calls just to advance
  the index;
- functions returning `Result` or an iterator: those types are already
  `#[must_use]` (`clippy::double_must_use`).

Inside the workspace, a deliberately ignored result is written `let _ = ...`.

## `#[non_exhaustive]`

On an **enum** when new variants are expected in a minor release:

- C enums that grew between LVGL releases or are open-ended:
  `ColorFormat`, `Align`, `GlyphFormat` (already marked),
  `DrawTaskType`, `DrawTaskState` (v9.6 added `Blocked`/`Finished`/`Failed`),
  `BlendMode`, `BlurQuality`, `GradDir`, `ImageCompress`, `ScreenLoadAnim`,
  `SubjectType`, `IndevGestureType`, `IndevGestureState`, `LeadingTrim`;
  the widget modes `LabelLongMode`, `BarMode`, `SliderMode`, `ArcMode` and
  `ImageAlign`;
- Rust data enums that mirror one of those or a C union of "any kind of
  value": `DrawDsc`, `GradParams`, `GlyphData`, `EventParam`, `EventTarget`,
  `StyleValue`, `ConstStyleValue`, `ImageSrc`, `ConstImageSrc`,
  `SubjectValue`, `ObserverUserCb`, `AnimExec`, `AnimPath`, `AnimTarget`,
  `BufStorage`, `ferrovgl-wasm`'s `InputEvent`;
- error enums: `FontError`, `ferrovgl-wasm`'s `ConvertError`.

A downstream `match` on these needs a `_` arm; a renderer handles an
unknown variant the way C's `default:` does (for example the blend kernels
treat an unknown `BlendMode` like `Normal`).

Not on an enum whose exhaustive matching is the point or whose set is
closed by definition: `LvResult`, `IndevState`, `Rotation`, `RenderMode`,
`FlushResult`, `TextAlign`, `BaseDir`, `Subpx`, `Kerning`, the
orientation enums, `ObjClassEditable`/`ObjClassGroupDef` (tri-states),
`ObjTreeWalkRes`, `ScrollbarMode`, `ScrollSnap`, `FlexAlign`, `FlexFlow`,
`GridAlign`, `GradExtend`, `Palette`, `FontRef`, `SharedRef`, the
`ferrovgl-draw-sw` mask enums. Numeric values stay `#[repr]`-identical to C
either way, and `as` casts keep working on `#[non_exhaustive]` enums.

`EventCode`, `StyleProp`, `Part`, `State`, `StyleSelector` and `ObjFlags`
are newtypes over the C integer with associated constants, because
applications register custom event codes and style properties; they are
open by construction and need no attribute.

On a **struct** only when the library creates it and applications only read
or modify it through a reference, so adding a field in a minor release must
not break them: `ObjData` (`lv_obj_t`), `SpecAttr`, `ObjStyle`,
`ObjStyleTransitionDsc`, `StyleTrans`, `DisplayData` (`lv_display_t`),
`Layer` (`lv_layer_t`), `TimerData` (`lv_timer_t`), `TimerState`,
`TickState`, `EventDsc` and `Event` (`lv_event_t`). (`Ui`, `DrawCtx`,
`GroupData`, `IndevData`, `Subject`, `Observer` and `AnimState` already have
private fields, which has the same effect.)

Never on a struct that users build with a struct literal or functional
update: `#[non_exhaustive]` forbids even `..Default::default()` outside the
defining crate. That covers `UiConfig`, every draw descriptor (`RectDsc`,
`LabelDsc`, `DrawImageDsc`, `LineDsc`, ...), `DrawDscBase`, `DrawTask` (draw
units and their tests build tasks), `GlyphDsc` (filled by `Font`
implementations), `IndevReadData` (filled by input drivers), `ObjClass`
(widget crates define `static` classes), `Theme`, `Anim`,
`StyleTransitionDsc`, `ImageHeader`, `ImageDsc`, `DrawBuf`, the geometry and
colour values, and `ferrovgl-wasm`'s `BackendConfig`. Their field lists
mirror the C structs and may grow in a minor release before 1.0; build them
with `Type::new()` / `Default::default()` plus field assignment, or with
`..Type::new()`, and expect to recompile.

## Derives (C-COMMON-TRAITS, C-DEBUG)

- `Debug` on every public type (`missing_debug_implementations` is clean).
  Types that hold callbacks (`Rc<dyn Fn ..>`) or trait objects implement it
  by hand: callbacks print as `<callback>`, `dyn WidgetState`,
  `dyn DisplayDriver`, `dyn IndevDriver` and `dyn DrawUnit` print their
  type (a draw unit also its name), and `Ui`, `SdlDisplay`, `Backend` and
  `WebDisplay` print a summary (configuration, object/display/timer
  counts) instead of their whole arenas.
- Handles (`Obj`, `ObjId`, `DisplayId`, `IndevId`, `GroupId`, `TimerId`,
  `AnimId`, `StyleId`, `LayerId`, `SubjectId`, `ObserverId`, `ThemeId`,
  `EventDscId`, `StyleTransId`, `DrawTaskRef` and every widget handle) are
  `Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord`. `Ord` lets
  `no_std` applications key a `BTreeMap` by handle; the order is arena order
  and means nothing else.
- Plain C enums and value types derive `Copy, Clone, Debug, PartialEq, Eq,
  Hash` where their fields allow (`f32` fields rule out `Eq`/`Hash`).
- `Default` exists where C has a zero or init value (the `*_dsc_init()`
  descriptors, `UiConfig`, the zero-valued variant of the C enums,
  `EventFilter`, `ConstStyle`). Handles have no `Default`: there is no null
  handle, `Option<Handle>` is.
- `Copy` is not added to larger descriptors (`DrawDscBase`,
  `IndevReadData`, the mask parameters) so they can gain non-`Copy` fields,
  nor to `Rand`, whose copies would silently repeat a random sequence.

## Visibility (C-STRUCT-PRIVATE)

- Items that are implementation details of the port and have no user
  outside their crate are `pub(crate)`. In 0.1: the `lv_refr.c` statics
  (`refr_invalid_areas`, `refr_area_part`, `refr_obj_and_children`,
  `refr_join_areas`), the `rotate90/180/270_*` helpers of
  `draw::utils`, the built-in layout callbacks (`flex_update`,
  `flex_get_min_size`, `grid_update`, `grid_get_min_size`,
  `builtin_layouts`), `Ui::obj_pos_invalidate_expand_blur` (the body of the
  public `Ui::obj_invalidate_expand_blur`), the `lv_draw_sw_line.c` /
  `lv_draw_sw_box_shadow.c` / `lv_draw_sw_img.c` statics
  (`draw_line_hor/ver/skew`, `shadow_blur_corner`, `img_draw_core`),
  `PointTransfDsc` and `execute_drawing`, and the `Ui` fields that have
  accessor methods (`timers`, `anims`, `tick`, `groups`, `group_default`,
  `disp_default`, `indev_active`, `layouts`, `rand`, `app`, `inited`, ...).
- Items that other ferrovgl crates, the test harness or the fuzz targets
  reach into stay `pub` but `#[doc(hidden)]`, with a note on the item:
  the `Ui` fields `objs`, `displays`, `disp_refresh`, `indevs`, `themes`,
  `styles`, `event_dscs`, `draw_units`, `observer`, and
  `ferrovgl_draw_sw::blend::words`. They may change in any release; use
  the accessor methods.
- `Ui::draw` stays public and documented: `ui.draw.task_get_*_dsc(t)` is
  how `LV_EVENT_DRAW_TASK_ADDED` handlers edit draw tasks.
- The rest of the C-derived surface (`DrawCtx`, `Layer`, `DrawTask`, the
  software renderer modules, `ObjClass`, `WidgetState`, `obj_event_base`)
  stays public: separate widget and draw-unit crates need it, as C widgets
  and draw units use LVGL's private headers.

## Re-exports and the prelude (C-REEXPORT)

`ferrovgl::prelude::*` gives a typical application what it needs:
`Ui`, `UiConfig`, `Obj`, `ObjFlags`, the `*Id` handles, `Event`,
`EventCode`, `EventParam`, `LvResult`, `Style`, `StyleId`, `StyleSelector`,
`StyleProp`, `StyleValue`, `Part`, `State`, the geometry and colour types,
`ColorFormat`, `RenderMode`, `FontRef`, `ImageSrc`, `Align`, `Dir`,
`TextAlign`, `ScrollbarMode`, `ScrollSnap`, `FlexFlow`, `FlexAlign`,
`GridAlign`, `Palette`, `Anim`, `AnimId`, `AnimPath`, `AnimExec`,
`AnimTarget`, the coordinate helpers (`coord`, `pct`, `SIZE_CONTENT`,
`grid_fr`, `GRID_CONTENT`, `GRID_TEMPLATE_LAST`), `OPA_COVER`, `OPA_TRANSP`,
`ANIM_ON`, `ANIM_OFF`, `ANIM_REPEAT_INFINITE` and, with `widgets`, every
widget handle with its mode enums. Backend, renderer and font setup stay
behind their module paths (`ferrovgl::draw_sw::init`,
`ferrovgl::fonts::MONTSERRAT_14`, `ferrovgl::sdl::SdlDisplay`). Each
crate root re-exports its most used types (`ferrovgl_core::{Ui, Obj, ..}`,
`ferrovgl_widgets::{Label, ..}`).

## Checking

```sh
# candidates for #[must_use], Debug and builder attributes
cargo clippy -p ferrovgl-core -p ferrovgl-draw-sw -p ferrovgl-widgets --lib --all-features -- \
    -W clippy::must_use_candidate -W clippy::return_self_not_must_use -W missing_debug_implementations
# public enums and structs without #[non_exhaustive], to review against the lists above
cargo clippy -p ferrovgl-core --lib --all-features -- -W clippy::exhaustive_enums -W clippy::exhaustive_structs
```

The only remaining `must_use_candidate` hits are `text::encoded_next` and
`encoded_prev`, left off on purpose (see above).
