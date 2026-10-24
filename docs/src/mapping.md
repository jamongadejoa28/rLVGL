# LVGL to ferrovgl mapping

| LVGL (C) | ferrovgl (Rust) |
|---|---|
| `lv_init()` | `Ui::new(UiConfig::default())` |
| `lv_tick_inc(ms)` | `ui.tick_inc(ms)` |
| `lv_timer_handler()` | `ui.timer_handler()` |
| `lv_obj_t *` | `Obj` (Copy handle) |
| `lv_obj_create(parent)` | `Obj::create(&mut ui, parent)` |
| `lv_obj_set_width(obj, w)` | `ui.obj(obj).set_width(w)` or `obj.set_width(&mut ui, w)` |
| `lv_label_create(parent)` | `Label::create(&mut ui, parent)` |
| `lv_label_set_text(l, s)` | `ui.label(l).set_text(s)` |
| `lv_obj_add_event_cb(obj, cb, code, ud)` | `ui.obj(obj).add_event_cb(code, move \|ui, e\| { .. })` |
| `lv_obj_delete(obj)` | `ui.obj(obj).delete()` |
| `lv_style_t` | `StyleId` (owned by `Ui`) or `&'static ConstStyle` |
| `LV_PART_MAIN \| LV_STATE_PRESSED` | `Selector::new(Part::Main, State::PRESSED)` |

The table is extended as modules land.
