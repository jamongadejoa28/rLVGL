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
later releases against (CI, after 0.1.0). Cargo's rules for a 0.x crate
apply: a 0.1.x release must stay compatible with 0.1.0 (cargo-semver-checks
treats it like a 1.x minor release), and 0.2.0 is the next release that may
break the API. Items marked `#[doc(hidden)]` are outside that promise.

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
  so the functions of its sub-modules drop `draw_` (`lv_draw_layer_create`
  is `DrawCtx::layer_create`, `lv_draw_task_get_fill_dsc` is
  `ctx.task_get_fill_dsc`; `Ui::draw_layer_create` is the `Ui` wrapper).
  The `lv_draw_<primitive>()` entry points keep the verb, because the bare
  noun would read as an accessor and `lv_draw_layer` would collide with
  `DrawCtx::layer`: `lv_draw_rect` is `ctx.draw_rect`, `lv_draw_label` is
  `ctx.draw_label`, `lv_draw_layer` is `ctx.draw_layer` (likewise
  `draw_fill`, `draw_border`, `draw_box_shadow`, `draw_character`,
  `draw_letter`, `draw_image`, `draw_line`, `draw_line_iterate`,
  `draw_arc`, `draw_triangle`, `draw_blur`, `draw_mask_rect`).
  Rust-only accessors without a C counterpart are short nouns: `ui.obj(o)`,
  `ui.obj_mut(o)`, `ui.display(d)`, `ui.style(s)`, `ui.subject(s)`,
  `ui.observer(o)`, `ui.app::<T>()`.
- There are no convenience aliases: the generated style accessors exist
  only as `obj_set_style_<prop>` / `obj_get_style_<prop>`. A C function
  has a second Rust name only where the layers meet: the `Ui` wrapper of a
  `DrawCtx` method (`Ui::draw_layer_create` for `DrawCtx::layer_create`) or
  a layer-2 value method next to the free function (`StyleSelector::state`
  and `obj_style::get_selector_state`).
- **Layer 2, widget handles and value types**: methods on `Label`, `Slider`,
  ..., `Event`, `Anim`, `Style`, `Area`, `Color`, `DrawBuf` drop the
  `lv_<module>_` prefix *and* `get_` (C-GETTER): `lv_label_get_text(l)` is
  `l.text(ui)`, `lv_event_get_code(e)` is `e.code()`,
  `lv_anim_get_delay(&a)` is `a.delay()`, `lv_area_get_width(&a)` is
  `a.width()`. Setters keep `set_`. Lookups that take a key keep a
  `get_` + noun name like `HashMap::get` (`Style::get_prop(prop)`).
- Private struct fields of the C object (`lv_slider_t`, `lv_buttonmatrix_t`,
  ...) that the ported upstream tests read or write directly get a getter
  or setter named after the C field (`Buttonmatrix::btn_cnt`,
  `Bar::indic_area`, `Slider::set_left_knob_focus`). They are test hooks,
  not API, so they are `#[doc(hidden)]` (see Visibility).
- Free functions in a module drop the module part of the C name:
  `lv_sqrt()` is `misc::math::sqrt`, `lv_text_get_size()` is
  `misc::text::get_size`, `lv_obj_style_get_selector_state()` is
  `core::obj_style::get_selector_state`, `lv_obj_id_compare()` is
  `core::obj_id::compare`, `lv_draw_arc_get_area()` is
  `draw::arc::get_area`, `lv_event_free_user_data_cb()` is
  `misc::event::free_user_data_cb`. Two cases keep a prefix: a function
  that another C module contributes to the Rust module keeps that module's
  name (`lv_image_src_get_type` and `lv_image_decoder_get_info` in
  `draw::image`, `lv_font_get_top_trim` / `lv_font_get_bottom_trim` as
  `font_get_top_trim` / `font_get_bottom_trim` in `misc::text`), and a C
  file `static` keeps its static name (`draw_letter_cb` of `lv_draw_sw_letter.c`, the
  `lv_draw_mask_line` / `lv_draw_mask_radius` / ... statics of
  `lv_draw_sw_mask.c` as `mask_line`, `mask_radius`, ...).
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
  `ui.widget_static_mut::<T>()` (its lazy `Default` insert is not a reason
  to call it for nothing), `ui.subject(s)`, `dyn WidgetState::get` /
  `get_mut`, `ctx.task_get_fill_dsc(t)`, `e.param()`);
- registrations whose returned id is the only way to reach what was
  registered: `ui.style_create`, `ui.theme_create`, `ui.subject_create`,
  `ui.layout_create` / `layout_register`, `ui.event_register_id`,
  `ui.style_register_prop`, `ui.draw_layer_create`,
  `ctx.add_task`, `ctx.layer_create`, `Arena::insert`. `ctx.add_task` is
  marked although the task stays in `Layer::tasks`: the new task has an
  empty descriptor, and the returned `DrawTaskRef` is how the caller stores
  the descriptor and finalizes the task, so dropping it leaves a task that
  is never finalized (the one in-tree test that wants exactly that writes
  `let _ =`);
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
- `ui.indev_find_scroll_obj`, which like upstream also accumulates
  `pointer.scroll_sum` and on a hit stores `scroll_dir` / `scroll_obj`;
- functions returning `Result` or an iterator: those types are already
  `#[must_use]` (`clippy::double_must_use`).

Inside the workspace, a deliberately ignored result is written `let _ = ...`.

## `#[non_exhaustive]`

`#[non_exhaustive]` is what lets an item gain variants or fields in a 0.1.x
release. Every exhaustive public enum, and every exhaustive public struct
whose fields are all public, can only grow in a breaking release (0.2.0),
and cargo-semver-checks reports the change (`enum_variant_added`,
`constructible_struct_adds_field`) against the 0.1.0 baseline.

On an **enum** when new variants are expected:

- C enums that grew between LVGL releases or are open-ended:
  `ColorFormat`, `Align`, `GlyphFormat` (already marked),
  `DrawTaskType`, `DrawTaskState` (v9.6 added `Blocked`/`Finished`/`Failed`),
  `BlendMode`, `BlurQuality`, `GradDir`, `ImageCompress`, `ScreenLoadAnim`,
  `SubjectType`, `IndevGestureType`, `IndevGestureState`, `LeadingTrim`,
  `LayerType` (new layer kinds), `StyleStateCmp` (an ordered scale that
  may gain levels); the widget modes `LabelLongMode`, `BarMode`,
  `SliderMode`, `ArcMode` and `ImageAlign`;
- Rust data enums that mirror one of those or a C union or `void *` of
  "any kind of value": `DrawDsc`, `GradParams`, `GlyphData`, `EventParam`,
  `EventTarget`, `ObserverTarget` (the `void * target` of an observer, like
  `EventTarget`), `StyleValue`, `ConstStyleValue`, `ImageSrc`,
  `ConstImageSrc`, `SubjectValue`, `ObserverUserCb`, `AnimExec`,
  `AnimPath`, `AnimTarget`, `BufStorage`, `ferrovgl-wasm`'s `InputEvent`.
  A two-way tagged choice is not such a union (`LinePoints`, below);
- error enums: `FontError`, `ferrovgl-wasm`'s `ConvertError`.

A downstream `match` on these needs a `_` arm; a renderer handles an
unknown variant the way C's `default:` does (for example the blend kernels
treat an unknown `BlendMode` like `Normal`).

Not on an enum whose exhaustive matching is the point or whose set is
closed by definition:

- result and state enums: `LvResult`, `CoverRes`, `StyleRes`, `FlushResult`,
  `IndevState`, `ObjTreeWalkRes`, `TextCmdState` (the three states of the
  recolor-command parser);
- closed C choices: `Rotation`, `RenderMode`, `TextAlign`, `BaseDir`,
  `Subpx`, `Kerning`, `IndevType`, `IndevMode` (timer or event driven),
  `GroupRefocusPolicy` (next or previous), `ScrollbarMode`, `ScrollSnap`,
  `FlexAlign`, `FlexFlow`, `GridAlign`, `GradExtend`, `Palette`, the
  orientation enums, `ObjClassEditable`/`ObjClassGroupDef` (tri-states);
- `ImageSrcType` (`lv_image_src_t`): `LV_IMAGE_SRC_UNKNOWN` is its
  catch-all, and the Rust-only `ImageSrc` variants (`DrawBuf`, `Layer`,
  `Shared`) all map to one of its four values, so `ImageSrc` can grow
  without it;
- `ObjPointTransformFlag`: a C bit-flag set modelled as an enum with all
  four combinations of its two bits;
- Rust data enums with a fixed set of cases: `FontRef`, `SharedRef`,
  `ObjStyleSrc` (the `is_local` / `is_trans` bits plus shared or constant),
  `AsyncJob` (`Call` carries any other job), `LinePoints` (the
  `point_array_is_mutable` choice), `SliderValueTarget` (the two bar values
  `value_to_set` can point to; a hidden test type anyway);
- the `ferrovgl-draw-sw` mask enums (`MaskRes`, `MaskType`, `LineSide`,
  `MaskParam`, `CircleRef`).

Numeric values stay `#[repr]`-identical to C either way, and `as` casts
keep working on `#[non_exhaustive]` enums.

`EventCode`, `StyleProp`, `Part`, `State`, `StyleSelector`, `ObjFlags` and
`ferrovgl-widgets`' `ButtonmatrixCtrl` are newtypes over the C integer with
associated constants, because applications register custom event codes and
style properties and combine control bits; they are open by construction
and need no attribute.

On a **struct** when the library creates it and applications only read or
modify it through a reference, so adding a field in a 0.1.x release must
not break them: `ObjData` (`lv_obj_t`), `SpecAttr`, `ObjStyle`,
`DisplayData` (`lv_display_t`), `Layer` (`lv_layer_t`), `TimerData`
(`lv_timer_t`), `EventDsc`, `Event` (`lv_event_t`), `DrawBuf`
(`lv_draw_buf_t`, built with `DrawBuf::create`, `init` or the other
constructors), `IndevPointer` and `IndevKeypad` (the `pointer` / `keypad`
parts of `lv_indev_t`), `SubjectIncrementDsc` (created by
`obj_add_subject_increment_event`), `DecodedImage`, `BidiState`, the
software renderer's state and caches (`SwDrawUnit`, `CircleCache`,
`RadiusCircleDsc`, `ShadowCache`, `GradCalc` and its `GradLinearState` /
`GradRadialState` / `GradConicalState`), `ferrovgl-examples`' `Example`
and `ferrovgl-wasm`'s `FlushConfig`. Structs that callbacks fill through
`&mut` are in this list too, because field assignment still works on a
`#[non_exhaustive]` struct and their C structs are known to grow:
`IndevReadData` (`lv_indev_data_t`, filled by `IndevDriver::read`; v9.6
already has gesture fields that are not ported yet) and `GlyphDsc`
(`lv_font_glyph_dsc_t`, filled by `Font::glyph_dsc`). A driver or font
writes `data.key = k;` or starts from `GlyphDsc::default()`. (`Ui`,
`DrawCtx`, `GroupData`, `Subject`, `Observer` and `Style` have private
fields, which has the same effect.)

`#[non_exhaustive]` does not stop field writes, so it is not enough for a
field with an invariant: such bookkeeping is private or `pub(crate)`
(`Style`'s `props` / `has_group`, `Event`'s `frame` / `deleted`), or
`#[doc(hidden)]` where another ferrovgl crate needs it (see Visibility).

Never on a struct that users build with a struct literal or functional
update: `#[non_exhaustive]` forbids even `..Default::default()` outside the
defining crate. That covers `UiConfig`, every draw descriptor (`RectDsc`,
`LabelDsc`, `DrawImageDsc`, `LineDsc`, ...), `DrawDscBase`, `DrawTask` (draw
units and their tests build tasks), `ObjClass` (widget crates define
`static` classes), `Theme`, `Anim`, `StyleTransitionDsc`,
`ObjStyleTransitionDsc` (the input of `ui.obj_style_create_transition`),
`ImageHeader`, `ImageDsc`, `HitTestInfo` and `CoverCheckInfo` (built by
whoever sends `LV_EVENT_HIT_TEST` / `LV_EVENT_COVER_CHECK`), `LabelHint`
and `TextAttributes` (widgets keep and fill them), the geometry and colour
values, and `ferrovgl-wasm`'s `BackendConfig`, `SourceLayout`, `Modifiers`,
`KeyMapping` and `CssRect`. Their field lists mirror the C structs; build
them with `Type::new()` / `Default::default()` plus field assignment, or
with `..Type::new()`. Adding a field to one of them waits for 0.2.0.

## Derives (C-COMMON-TRAITS, C-DEBUG)

- `Debug` on every public type (`missing_debug_implementations` is clean).
  Types that hold callbacks (`Rc<dyn Fn ..>`) or trait objects implement it
  by hand: callbacks print as `<callback>`; `dyn WidgetState`,
  `dyn DisplayDriver` and `dyn IndevDriver` print a placeholder with the
  trait name (`WidgetState { .. }`: a trait object cannot name its
  concrete type), `dyn DrawUnit` also prints the unit's `name()`; `Ui`,
  `SdlDisplay`, `Backend` and `WebDisplay` print a summary (configuration,
  object/display/timer counts) instead of their whole arenas. Pixel and
  mask buffers print their length, not their bytes (`BufStorage`, the
  `ferrovgl-draw-sw` blend descriptors, `MaskMapParam`).
- Handles (`Obj`, `ObjId`, `DisplayId`, `IndevId`, `GroupId`, `TimerId`,
  `AnimId`, `StyleId`, `LayerId`, `SubjectId`, `ObserverId`, `ThemeId`,
  `EventDscId`, `StyleTransId`, `DrawTaskRef` and every widget handle) are
  `Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord`. `Ord` lets
  `no_std` applications key a `BTreeMap` by handle; the order is arena-key
  order (for `DrawTaskRef`: layer key, then queue position) and means
  nothing else. `DrawTaskRef` is a position in `Layer::tasks`, not a
  generational key, so it is only valid within the event or dispatch that
  handed it out.
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
  outside their crate are `pub(crate)` (or private). In 0.1: the
  `lv_refr.c` statics (`refr_area`, `refr_invalid_areas`, `refr_area_part`,
  `refr_obj_and_children`, `refr_join_areas`), the `rotate90/180/270_*`
  helpers of `draw::utils`, the built-in layout callbacks (`flex_update`,
  `flex_get_min_size`, `grid_update`, `grid_get_min_size`,
  `builtin_layouts`), `Ui::obj_pos_invalidate_expand_blur` (the body of the
  public `Ui::obj_invalidate_expand_blur`), the literal `lv_obj.c` bodies
  of `obj_add_child` / `obj_remove_child` in `core::obj` (compiled for
  their unit tests only; the API is `Ui::obj_add_child` /
  `Ui::obj_remove_child`), the `lv_draw_sw_line.c` /
  `lv_draw_sw_box_shadow.c` / `lv_draw_sw_img.c` / `lv_draw_sw_letter.c`
  statics (`draw_line_hor/ver/skew`, `shadow_blur_corner`, `img_draw_core`,
  `draw_letter_cb_impl`), `PointTransfDsc` and `execute_drawing`; the
  state types behind private `Ui` fields (`TimerState`, `AnimState`,
  `TickState`, `EventFrame`, `StyleTrans`, `ClassInfo`); the `Ui` fields
  that have accessor methods (`timers`, `anims`, `tick`, `groups`,
  `group_default`, `disp_default`, `indev_active`, `layouts`, `rand`, `app`,
  `inited`, `event_stack`, `style_trans`, `objid_array`, ...); the fields
  with invariants named above (`Style::props` / `has_group`, read with
  `Style::props()` / `has_group()`; `Event::frame` / `deleted`); and
  `ferrovgl-wasm`'s `Inputs` (like `ferrovgl-sdl`'s, reached through
  `Backend::pointer()` / `keypad()` / `encoder()` / `handle_input()`).
- Items that other ferrovgl crates, the test harness or the fuzz targets
  reach into stay `pub` but `#[doc(hidden)]`, with a note on the item:
  the `Ui` fields `objs`, `displays`, `disp_refresh`, `indevs`, `themes`,
  `styles`, `event_dscs`, `draw_units`, `observer` (use `ui.subject(s)`,
  `ui.observer(o)` and the `subject_*` / `observer_*` methods); the
  `DrawCtx` fields `layers`, `used_memory_for_layers`, `dispatch_req`,
  `task_running`, `unit_cnt` (use `ctx.layer(l)` / `layer_mut(l)` and
  `ui.draw_get_unit_count()`); the inner key of every handle (`Obj`,
  `ObjId`, `DisplayId`, `IndevId`, `GroupId`, `TimerId`, `AnimId`,
  `StyleId`, `StyleTransId`, `LayerId`, `SubjectId`, `ObserverId`,
  `ThemeId`, `EventDscId`) and the `layer` / `index` fields of
  `DrawTaskRef`, so the arena representation is not API;
  `core::observer::log_warn`; `ferrovgl_draw_sw::blend::words`; and the
  widget test hooks for private C fields that upstream tests read or
  write (`Buttonmatrix::btn_cnt` / `button_areas` / `set_btn_id_sel`,
  `Slider::left_knob_area` / `right_knob_area` / `value_to_set` /
  `left_knob_focus` / `set_left_knob_focus` and `SliderValueTarget`,
  `Switch::anim_state`, `Bar::indic_area`). They may change in any
  release; use the accessor methods.
- `Ui::draw` stays public and documented: `ui.draw.task_get_*_dsc(t)` is
  how `LV_EVENT_DRAW_TASK_ADDED` handlers edit draw tasks.
- The rest of the C-derived surface (`DrawCtx`, `Layer`, `DrawTask`, the
  software renderer modules, `ObjClass`, `WidgetState`, `obj_event_base`)
  stays public: separate widget and draw-unit crates need it, as C widgets
  and draw units use LVGL's private headers.

## Errors (C-GOOD-ERR)

- Error types implement `Debug`, `Display` and `core::error::Error`
  (`FontError`, `ferrovgl-wasm`'s `ConvertError`), so they work with `?`
  and `Box<dyn Error>`.
- Known 0.1 limitation: the backend constructors and frame functions of
  `ferrovgl-sdl` (`SdlDisplay::new`, `step`, `request_quit`) and
  `ferrovgl-wasm` (`Backend::new`, `frame`, `refresh_now`,
  `FlushConfig::sample`) return `Result<_, String>`. A typed error
  (`SdlError`, `BackendError`) replaces it in 0.2.0, which is a breaking
  change.

## Re-exports and the prelude (C-REEXPORT)

`ferrovgl::prelude::*` gives a typical application what it needs:
`Ui`, `UiConfig`, `Obj`, `ObjFlags`, the handles `DisplayId`, `IndevId`,
`GroupId`, `TimerId`, `AnimId`, `StyleId`, `ThemeId`, `LayerId`,
`EventDscId`, `SubjectId` and `ObserverId`, `Event`,
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
cargo clippy -p ferrovgl-core -p ferrovgl-draw-sw -p ferrovgl-widgets -p ferrovgl-wasm -p ferrovgl-examples \
    --lib --all-features -- -W clippy::exhaustive_enums -W clippy::exhaustive_structs
```

The only remaining `must_use_candidate` hits are `text::encoded_next` and
`encoded_prev`, left off on purpose (see above).
