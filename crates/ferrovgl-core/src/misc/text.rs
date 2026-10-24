//! Ported from lvgl v9.6.0: `include/lvgl/font/lv_text.h`, `src/misc/lv_text_private.h`,
//! `src/misc/lv_text.c`.
//!
//! Text measurement and line breaking. Only the type definitions and the
//! header-level inline helpers are implemented here; the line breaking engine
//! (`lv_text.c`) is filled in by the text port.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::font::Font;
use crate::misc::area::Point;

/// `LV_TXT_COLOR_CMD`: the character that starts a recolor command.
pub const TXT_COLOR_CMD: &str = "#";
/// `LV_TXT_BREAK_CHARS`: characters after which a line can be broken.
pub const TXT_BREAK_CHARS: &str = " ,.;:-_)]}";
/// `LV_TXT_LINE_BREAK_LONG_LEN`: minimal word length to break anywhere
/// (0 disables breaking inside words).
pub const TXT_LINE_BREAK_LONG_LEN: u32 = 0;
/// `LV_TXT_LINE_BREAK_LONG_PRE_MIN_LEN`
pub const TXT_LINE_BREAK_LONG_PRE_MIN_LEN: u32 = 3;
/// `LV_TXT_LINE_BREAK_LONG_POST_MIN_LEN`
pub const TXT_LINE_BREAK_LONG_POST_MIN_LEN: u32 = 3;
/// `LV_TEXT_LEN_MAX`
pub const TEXT_LEN_MAX: u32 = u32::MAX;

bitflags::bitflags! {
    /// `lv_text_flag_t`
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct TextFlags: u8 {
        /// `LV_TEXT_FLAG_EXPAND`: ignore `max_width` to avoid automatic word wrapping.
        const EXPAND = 0x01;
        /// `LV_TEXT_FLAG_FIT`: `max_width` is already equal to the longest line.
        const FIT = 0x02;
        /// `LV_TEXT_FLAG_BREAK_ALL`: wrap anywhere, not only at break characters.
        const BREAK_ALL = 0x04;
        /// `LV_TEXT_FLAG_RECOLOR`: enable `#rrggbb text#` recolor commands.
        const RECOLOR = 0x08;
    }
}

impl TextFlags {
    /// `LV_TEXT_FLAG_NONE`
    pub const NONE: TextFlags = TextFlags::empty();
}

/// `lv_text_align_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextAlign {
    /// `LV_TEXT_ALIGN_AUTO`
    #[default]
    Auto = 0,
    /// `LV_TEXT_ALIGN_LEFT`
    Left = 1,
    /// `LV_TEXT_ALIGN_CENTER`
    Center = 2,
    /// `LV_TEXT_ALIGN_RIGHT`
    Right = 3,
}

bitflags::bitflags! {
    /// `lv_text_decor_t` (defined in `lv_style.h`)
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct TextDecor: u8 {
        /// `LV_TEXT_DECOR_UNDERLINE`
        const UNDERLINE = 0x01;
        /// `LV_TEXT_DECOR_STRIKETHROUGH`
        const STRIKETHROUGH = 0x02;
    }
}

impl TextDecor {
    /// `LV_TEXT_DECOR_NONE`
    pub const NONE: TextDecor = TextDecor::empty();
}

/// `lv_base_dir_t` (defined in `lv_bidi.h`)
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BaseDir {
    /// `LV_BASE_DIR_LTR`
    #[default]
    Ltr = 0x00,
    /// `LV_BASE_DIR_RTL`
    Rtl = 0x01,
    /// `LV_BASE_DIR_AUTO`
    Auto = 0x02,
    /// `LV_BASE_DIR_NEUTRAL`
    Neutral = 0x20,
    /// `LV_BASE_DIR_WEAK`
    Weak = 0x21,
}

/// `lv_text_leading_trim_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LeadingTrim {
    /// `LV_TEXT_LEADING_TRIM_NONE`
    #[default]
    None = 0,
    /// `LV_TEXT_LEADING_TRIM_CAPITAL_BASELINE`
    CapitalBaseline = 1,
    /// `LV_TEXT_LEADING_TRIM_LOWER_BASELINE`
    LowerBaseline = 2,
    /// `LV_TEXT_LEADING_TRIM_CAPITAL`
    Capital = 3,
    /// `LV_TEXT_LEADING_TRIM_LOWER`
    Lower = 4,
}

/// `lv_text_cmd_state_t`: state of the recolor command parser.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextCmdState {
    /// `LV_TEXT_CMD_STATE_WAIT`: waiting for a command.
    #[default]
    Wait = 0,
    /// `LV_TEXT_CMD_STATE_PAR`: processing the parameter.
    Par = 1,
    /// `LV_TEXT_CMD_STATE_IN`: processing the command text.
    In = 2,
}

/// `lv_text_attributes_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct TextAttributes {
    /// `letter_space`
    pub letter_space: i32,
    /// `line_space`
    pub line_space: i32,
    /// `max_width`
    pub max_width: i32,
    /// `text_flags`
    pub text_flags: TextFlags,
}

impl TextAttributes {
    /// `lv_text_attributes_init()`: all zero / no flags.
    pub const fn new() -> Self {
        Self {
            letter_space: 0,
            line_space: 0,
            max_width: 0,
            text_flags: TextFlags::NONE,
        }
    }
}

/// `lv_text_is_break_char()`
#[inline]
pub fn is_break_char(letter: u32) -> bool {
    TXT_BREAK_CHARS.bytes().any(|c| letter == c as u32)
}

/// `lv_text_is_hanging_space()`
#[inline]
pub const fn is_hanging_space(letter: u32) -> bool {
    letter == ' ' as u32 || letter == '\n' as u32 || letter == '\r' as u32
}

/// `lv_text_is_a_word()`: CJK and other characters that are words by themselves.
pub const fn is_a_word(letter: u32) -> bool {
    if letter == 0 {
        return false;
    }
    if letter >= 0x4E00 && letter <= 0x9FFF {
        return true;
    }
    if letter >= 0xFF01 && letter <= 0xFF5E {
        return true;
    }
    if letter >= 0x3000 && letter <= 0x303F {
        return true;
    }
    if letter >= 0x2E80 && letter <= 0x2EFF {
        return true;
    }
    if letter >= 0x31C0 && letter <= 0x31EF {
        return true;
    }
    if letter >= 0x3040 && letter <= 0x30FF {
        return true;
    }
    if letter >= 0xFE10 && letter <= 0xFE1F {
        return true;
    }
    if letter >= 0xFE30 && letter <= 0xFE4F {
        return true;
    }
    false
}

/// `lv_text_is_marker()`: control characters that occupy no space.
pub const fn is_marker(letter: u32) -> bool {
    if letter < 0x20 {
        return true;
    }
    if letter == 0x061C {
        return true;
    }
    if letter == 0x115F {
        return true;
    }
    if letter == 0x1160 {
        return true;
    }
    if letter >= 0x180B && letter <= 0x180E {
        return true;
    }
    if letter >= 0x200B && letter <= 0x200F {
        return true;
    }
    if letter >= 0x2028 && letter <= 0x202F {
        return true;
    }
    if letter >= 0x205F && letter <= 0x206F {
        return true;
    }
    if letter == 0xFEFF {
        return true;
    }
    if letter == 0xF8FF {
        return true;
    }
    false
}

/// `lv_font_get_bottom_trim_internal()` / `lv_font_get_bottom_trim()`
pub fn font_get_bottom_trim(font: &dyn Font, trim: LeadingTrim) -> i32 {
    match trim {
        LeadingTrim::CapitalBaseline | LeadingTrim::LowerBaseline => font.base_line(),
        LeadingTrim::None | LeadingTrim::Capital | LeadingTrim::Lower => 0,
    }
}

/// `lv_font_get_top_trim_internal()` / `lv_font_get_top_trim()`
pub fn font_get_top_trim(font: &dyn Font, trim: LeadingTrim) -> i32 {
    match trim {
        LeadingTrim::CapitalBaseline | LeadingTrim::Capital => {
            (font.line_height() - font.base_line()) - font.cap_height()
        }
        LeadingTrim::LowerBaseline | LeadingTrim::Lower => {
            (font.line_height() - font.base_line()) - font.x_height()
        }
        LeadingTrim::None => 0,
    }
}

/// `lv_text_get_size()`: size of `text` rendered with `font`.
pub fn get_size(
    text: &str,
    font: &dyn Font,
    letter_space: i32,
    line_space: i32,
    max_width: i32,
    flag: TextFlags,
) -> Point {
    todo!()
}

/// `lv_text_get_size_attributes()`
pub fn get_size_attributes(text: &str, font: &dyn Font, attributes: &TextAttributes) -> Point {
    todo!()
}

/// `lv_text_get_width()`: width of the first `length` bytes of `txt`.
pub fn get_width(txt: &str, length: u32, font: &dyn Font, attributes: &TextAttributes) -> i32 {
    todo!()
}

/// `lv_text_get_line_width()`
pub fn get_line_width(txt: &str, length: u32, font: &dyn Font, attributes: &TextAttributes) -> i32 {
    todo!()
}

/// `lv_text_is_cmd()`: advance the recolor command state machine with `c`.
/// Returns `true` if `c` is part of a command (should not be drawn).
pub fn is_cmd(state: &mut TextCmdState, c: u32) -> bool {
    todo!()
}

/// `lv_text_get_next_line()`: byte index where the next line starts.
/// `used_width`, when given, receives the width of the line.
pub fn get_next_line(
    txt: &str,
    len: u32,
    font: &dyn Font,
    used_width: Option<&mut i32>,
    attributes: &mut TextAttributes,
) -> u32 {
    todo!()
}

/// `lv_text_ins()`: insert `ins_txt` at character position `pos`.
#[allow(clippy::ptr_arg)] // insertion grows the buffer, a `&mut str` cannot
pub fn ins(txt_buf: &mut alloc::string::String, pos: u32, ins_txt: &str) {
    todo!()
}

/// `lv_text_cut()`: delete `len` characters starting at character `pos`.
#[allow(clippy::ptr_arg)] // removal shrinks the buffer, a `&mut str` cannot
pub fn cut(txt: &mut alloc::string::String, pos: u32, len: u32) {
    todo!()
}

/// `lv_text_encoded_letter_next_2()`: read the letter at `ofs` and the one
/// after it, advancing `ofs`.
pub fn encoded_letter_next_2(txt: &str, letter: &mut u32, letter_next: &mut u32, ofs: &mut u32) {
    todo!()
}

/// `lv_text_encoded_size()`: byte length of the UTF-8 sequence starting at `txt[0]`.
pub fn encoded_size(txt: &[u8]) -> u8 {
    todo!()
}

/// `lv_text_unicode_to_encoded()`: unicode code point to packed UTF-8 bytes.
pub fn unicode_to_encoded(letter_uni: u32) -> u32 {
    todo!()
}

/// `lv_text_encoded_conv_wc()`: packed UTF-8 bytes to a unicode code point.
pub fn encoded_conv_wc(c: u32) -> u32 {
    todo!()
}

/// `lv_text_encoded_next()`: decode the letter at byte index `*i` and advance it.
pub fn encoded_next(txt: &[u8], i: Option<&mut u32>) -> u32 {
    todo!()
}

/// `lv_text_encoded_prev()`: decode the letter before byte index `*i` and move it back.
pub fn encoded_prev(txt: &[u8], i: Option<&mut u32>) -> u32 {
    todo!()
}

/// `lv_text_encoded_get_byte_id()`: character index to byte index.
pub fn encoded_get_byte_id(txt: &[u8], utf8_id: u32) -> u32 {
    todo!()
}

/// `lv_text_encoded_get_char_id()`: byte index to character index.
pub fn encoded_get_char_id(txt: &[u8], byte_id: u32) -> u32 {
    todo!()
}

/// `lv_text_get_encoded_length()`: number of characters in `txt`.
pub fn get_encoded_length(txt: &[u8]) -> u32 {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn break_chars_and_markers() {
        for c in TXT_BREAK_CHARS.bytes() {
            assert!(is_break_char(c as u32));
        }
        assert!(!is_break_char('a' as u32));
        assert!(is_marker(0x0A));
        assert!(is_marker(0x200C));
        assert!(is_marker(0xF8FF));
        assert!(!is_marker('A' as u32));
        assert!(is_a_word(0x4E2D));
        assert!(!is_a_word('x' as u32));
        assert!(is_hanging_space(' ' as u32));
    }

    #[test]
    fn enum_values_match_upstream() {
        assert_eq!(TextAlign::Right as u8, 3);
        assert_eq!(BaseDir::Weak as u8, 0x21);
        assert_eq!(LeadingTrim::Lower as u8, 4);
        assert_eq!(TextFlags::RECOLOR.bits(), 0x08);
        assert_eq!(TextDecor::STRIKETHROUGH.bits(), 0x02);
    }
}
