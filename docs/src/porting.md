# Porting notes

- Each ported file starts with `//! Ported from lvgl v9.6.0: <upstream path>`.
- Integer math (`lv_sqrt`, `lv_trigo_sin`, `lv_cubic_bezier`, `lv_atan2`,
  `lv_color_mix`, `LV_OPA_MIX2/3`, `LV_UDIV255`) is ported with identical
  operation order so rounding matches.
- `lv_value_precise_t` is `f32` with the `float` feature (the test
  configuration) and `i32` without it.
- Coordinates keep LVGL's `i32` encoding: `LV_COORD_TYPE_SHIFT = 29`,
  `LV_SIZE_CONTENT`, `LV_PCT(x)`.
