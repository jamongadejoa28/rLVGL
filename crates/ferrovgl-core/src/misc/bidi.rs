//! Ported from LVGL v9.6.0: `src/misc/lv_bidi.c`.
//!
//! Also covers `include/lvgl/font/lv_bidi.h`, `src/misc/lv_bidi_private.h`
//! and `src/misc/lv_bidi_common.c` (the `LV_USE_BIDI == 1` branch of
//! `lv_bidi_calculate_align()`).
//!
//! `lv_base_dir_t` is [`BaseDir`] (defined beside `lv_text_align_t` in
//! `misc/text.rs` because the style system needs it without this feature).
//!
//! The file-scope `custom_neutrals` static of `lv_bidi.c` lives in
//! [`BidiState`], owned by `DrawCtx::bidi` so that both the draw code
//! (`lv_draw_label`) and the widgets (`ui.draw.bidi`) reach the same
//! instance. The functions taking a `const char *` take bytes that end at the
//! first NUL or at the end of the slice, like the other `misc/text.rs` ports.
//!
//! SKELETON: the types and public API are declared; the bodies are `todo!()`
//! until the module is ported. The existing callers (`draw/label.rs`,
//! `core/obj_style.rs`) still use their local `LV_USE_BIDI` stubs.

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

use alloc::vec::Vec;

pub use crate::misc::text::BaseDir;
use crate::misc::text::TextAlign;

/// `LV_BIDI_LRO`: left-to-right override (U+202D), a non printable strong
/// character that affects the run's direction.
pub const BIDI_LRO: &str = "\u{202D}";
/// `LV_BIDI_RLO`: right-to-left override (U+202E).
pub const BIDI_RLO: &str = "\u{202E}";

/// `LV_BIDI_BASE_DIR_DEF`: the Kconfig default `LV_BASE_DIR_AUTO`
/// (`tests/configs/full.defconfig` does not override it).
pub const BIDI_BASE_DIR_DEF: BaseDir = BaseDir::Auto;

/// `LV_BIDI_BRACKET_DEPTH` (private define in `lv_bidi.c`).
const BIDI_BRACKET_DEPTH: usize = 4;

/// `GET_POS(x)`: the position part of a 16-bit `pos_conv` value.
const fn get_pos(x: u16) -> u16 {
    x & 0x7FFF
}

/// `IS_RTL_POS(x)`: the highest bit of a `pos_conv` value marks RTL.
const fn is_rtl_pos(x: u16) -> bool {
    (x & 0x8000) != 0
}

/// `SET_RTL_POS(x, is_rtl)`
const fn set_rtl_pos(x: u16, is_rtl: bool) -> u16 {
    get_pos(x) | if is_rtl { 0x8000 } else { 0 }
}

/// `bracket_stack_t` (private in `lv_bidi.c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BracketStack {
    /// `bracket_pos`
    bracket_pos: u32,
    /// `dir`
    dir: BaseDir,
}

/// `lv_bidi_ctx_t` (private in `lv_bidi.c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BidiCtx {
    /// `br_stack`
    br_stack: [BracketStack; BIDI_BRACKET_DEPTH],
    /// `br_stack_p`
    br_stack_p: u8,
}

/// The file-scope state of `lv_bidi.c`. Every BiDi function is a method so
/// that `lv_bidi_letter_is_neutral()` sees the custom neutrals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BidiState {
    /// `custom_neutrals` (`NULL` = `None`: the built-in set is used).
    pub custom_neutrals: Option<&'static str>,
}

impl BidiState {
    /// The zero-initialized statics.
    pub const fn new() -> Self {
        Self {
            custom_neutrals: None,
        }
    }

    /// `lv_bidi_set_custom_neutrals_static()`: `None` restores the built-in
    /// set.
    pub fn set_custom_neutrals_static(&mut self, neutrals: Option<&'static str>) {
        todo!()
    }

    /// `lv_bidi_calculate_align()` (`lv_bidi_calculate_align_internal()`
    /// with `LV_USE_BIDI`): resolve `LV_BASE_DIR_AUTO` from `txt` and
    /// `LV_TEXT_ALIGN_AUTO` from the base direction.
    pub fn calculate_align(&self, align: &mut TextAlign, base_dir: &mut BaseDir, txt: &[u8]) {
        todo!()
    }

    /// `lv_bidi_process()`: reorder every paragraph of `str_in` into visual
    /// order. `str_out` must hold `strlen(str_in) + 1` bytes.
    pub fn process(&self, str_in: &[u8], str_out: &mut [u8], base_dir: BaseDir) {
        todo!()
    }

    /// `lv_bidi_detect_base_dir()`: `Rtl` if any strong RTL character is
    /// found, else `Ltr` if any strong LTR one is, else
    /// [`BIDI_BASE_DIR_DEF`] (`Ltr` for `Auto`).
    pub fn detect_base_dir(&self, txt: &[u8]) -> BaseDir {
        todo!()
    }

    /// `lv_bidi_get_logical_pos()`: the logical position of the character at
    /// `visual_pos` of a line of `len` bytes. `bidi_txt` receives the
    /// processed line (C: an `lv_malloc`ed buffer of `len + 1` bytes) and
    /// `is_rtl` the direction of the character's context. Returns
    /// `(uint16_t) -1` on allocation failure.
    pub fn get_logical_pos(
        &self,
        str_in: &[u8],
        bidi_txt: Option<&mut Vec<u8>>,
        len: u32,
        base_dir: BaseDir,
        visual_pos: u32,
        is_rtl: Option<&mut bool>,
    ) -> u16 {
        todo!()
    }

    /// `lv_bidi_get_visual_pos()`: the visual position of the character at
    /// `logical_pos` of a line of `len` bytes (`(uint16_t) -1` if not found).
    pub fn get_visual_pos(
        &self,
        str_in: &[u8],
        bidi_txt: Option<&mut Vec<u8>>,
        len: u16,
        base_dir: BaseDir,
        logical_pos: u32,
        is_rtl: Option<&mut bool>,
    ) -> u16 {
        todo!()
    }

    /// `lv_bidi_process_paragraph()`: reorder `len` bytes of one paragraph.
    /// `str_out` (`NULL` = `None`) receives the visual order, `pos_conv_out`
    /// the logical position of each character (`pos_conv_len` elements).
    pub fn process_paragraph(
        &self,
        str_in: &[u8],
        str_out: Option<&mut [u8]>,
        len: u32,
        base_dir: BaseDir,
        pos_conv_out: Option<&mut [u16]>,
        pos_conv_len: u16,
    ) {
        todo!()
    }
}
