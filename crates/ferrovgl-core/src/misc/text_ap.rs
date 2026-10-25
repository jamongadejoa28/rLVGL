//! Ported from LVGL v9.6.0: `src/misc/lv_text_ap.c`.
//!
//! Also covers `src/misc/lv_text_ap.h` (`LV_USE_ARABIC_PERSIAN_CHARS`).
//!
//! The Arabic/Persian shaping was ported earlier into `misc/text.rs`, where
//! `lv_text_set_text_vfmt()` and the widgets already use it; this module is
//! the `lv_text_ap.h` entry point and re-exports that implementation:
//! `lv_text_ap_strlen()` is [`ap_strlen`] ([`ap_strlen_bytes`] for bytes that
//! are not valid UTF-8) and `lv_text_ap_proc()` is [`ap_proc`] (it returns the
//! processed string instead of writing a caller buffer of
//! `lv_text_ap_strlen(txt) + 1` bytes; `lv_text_ap_proc(txt, NULL)` is a no-op
//! upstream and has no counterpart).

pub use crate::misc::text::{
    AP_ALPHABET_BASE_CODE, UNDEF_ARABIC_PERSIAN_CHARS, ap_proc, ap_strlen, ap_strlen_bytes,
};
