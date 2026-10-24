//! Text helpers.
//!
//! Ported from lvgl v9.6.0: `src/misc/lv_text.c` (partial; the line breaking
//! engine lands with the label widget).

/// `lv_text_is_marker()`: control characters that occupy no space.
#[inline]
pub const fn is_marker(letter: u32) -> bool {
    letter < 0x20 || letter == 0xF8FF /* LV_SYMBOL_DUMMY */ || letter == 0x200C /* ZWNJ */
}
