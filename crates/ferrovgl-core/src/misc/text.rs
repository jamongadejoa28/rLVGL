//! Ported from lvgl v9.6.0: `include/lvgl/font/lv_text.h`, `src/misc/lv_text_private.h`,
//! `src/misc/lv_text.c`, `src/misc/lv_text_ap.h`, `src/misc/lv_text_ap.c`.
//!
//! Text measurement, line breaking and the UTF-8 encoder/decoder
//! (`LV_TXT_ENC == LV_TXT_ENC_UTF8`).
//!
//! Upstream works on `'\0'` terminated `char *` buffers and byte indexes.
//! Here text is `&str` / `&[u8]` plus the same byte indexes: reading past the
//! end of the slice (or an embedded NUL byte) yields `0`, exactly where C
//! would read the terminator, so every loop keeps its upstream shape.

use crate::font::{self, Font};
use crate::misc::area::Point;
use crate::misc::types::coord;

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

/// `NO_BREAK_FOUND` (private define in `lv_text.c`).
const NO_BREAK_FOUND: u32 = u32::MAX;

/// `#if LV_TXT_LINE_BREAK_LONG_LEN > 0`: break inside words that are "long".
const LINE_BREAK_LONG_ENABLED: bool = TXT_LINE_BREAK_LONG_LEN != 0;

/// `LV_USE_ARABIC_PERSIAN_CHARS`: `tests/configs/full.defconfig` (the parity
/// config) sets it to `y`, so the `#if LV_USE_ARABIC_PERSIAN_CHARS` branches
/// are the ones ported.
pub const USE_ARABIC_PERSIAN_CHARS: bool = true;

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
#[non_exhaustive]
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
    #[must_use]
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
#[must_use]
pub fn is_break_char(letter: u32) -> bool {
    TXT_BREAK_CHARS.bytes().any(|c| letter == c as u32)
}

/// `lv_text_is_hanging_space()`
#[inline]
#[must_use]
pub const fn is_hanging_space(letter: u32) -> bool {
    letter == ' ' as u32 || letter == '\n' as u32 || letter == '\r' as u32
}

/// `lv_text_is_a_word()`: CJK and other characters that are words by themselves.
#[must_use]
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
#[must_use]
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
#[must_use]
pub fn font_get_bottom_trim(font: &dyn Font, trim: LeadingTrim) -> i32 {
    match trim {
        LeadingTrim::CapitalBaseline | LeadingTrim::LowerBaseline => font.base_line(),
        LeadingTrim::None | LeadingTrim::Capital | LeadingTrim::Lower => 0,
    }
}

/// `lv_font_get_top_trim_internal()` / `lv_font_get_top_trim()`
#[must_use]
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

/*
 * C string helpers: `txt[i]` on a '\0' terminated buffer.
 */

/// `txt[i]` of a C string: `0` at and past the end of the slice.
#[inline]
fn byte_at(txt: &[u8], i: u32) -> u8 {
    txt.get(i as usize).copied().unwrap_or(0)
}

/// `&txt[i]` of a C string: the empty string at and past the end.
#[inline]
fn tail(txt: &[u8], i: u32) -> &[u8] {
    txt.get(i as usize..).unwrap_or(&[])
}

/// `lv_strlen()`: bytes before the first NUL (or the whole slice).
#[inline]
fn strlen(txt: &[u8]) -> usize {
    txt.iter().position(|&c| c == 0).unwrap_or(txt.len())
}

/*
 * UTF-8 classification macros from `lv_text.c`.
 */

/// `LV_IS_ASCII()`
#[inline]
const fn is_ascii(value: u8) -> bool {
    (value & 0x80) == 0x00
}

/// `LV_IS_2BYTES_UTF8_CODE()`
#[inline]
const fn is_2bytes_utf8_code(value: u8) -> bool {
    (value & 0xE0) == 0xC0
}

/// `LV_IS_3BYTES_UTF8_CODE()`
#[inline]
const fn is_3bytes_utf8_code(value: u8) -> bool {
    (value & 0xF0) == 0xE0
}

/// `LV_IS_4BYTES_UTF8_CODE()`
#[inline]
const fn is_4bytes_utf8_code(value: u8) -> bool {
    (value & 0xF8) == 0xF0
}

/// `LV_IS_INVALID_UTF8_CODE()`
#[inline]
const fn is_invalid_utf8_code(value: u8) -> bool {
    (value & 0xC0) != 0x80
}

/*
 * Global functions.
 */

/// `lv_text_get_size()` / `lv_text_get_size_internal()`: size of `text`
/// rendered with `font`.
#[must_use]
pub fn get_size(
    text: &str,
    font: &dyn Font,
    letter_space: i32,
    line_space: i32,
    max_width: i32,
    flag: TextFlags,
) -> Point {
    let mut attrs = TextAttributes::new();
    attrs.line_space = line_space;
    attrs.max_width = max_width;
    attrs.text_flags = flag;
    attrs.letter_space = letter_space;
    get_size_attributes(text, font, &attrs)
}

/// `lv_text_get_size_attributes()`
///
/// Upstream writes `LV_COORD_MAX` back into `attributes->max_width` when
/// `LV_TEXT_FLAG_EXPAND` is set; here that happens on a private copy.
#[must_use]
pub fn get_size_attributes(text: &str, font: &dyn Font, attributes: &TextAttributes) -> Point {
    let text = text.as_bytes();
    let mut attributes = *attributes;
    let mut line_start: u32 = 0;
    let mut new_line_start: u32 = 0;
    let mut size_res = Point::new(0, 0);

    // `uint16_t letter_height = lv_font_get_line_height_internal(font);`
    let letter_height = font::get_line_height(font) as u16;

    if attributes.text_flags.contains(TextFlags::EXPAND) {
        attributes.max_width = coord::MAX;
    }

    /* Calc. the height and longest line */
    while byte_at(text, line_start) != 0 {
        new_line_start = new_line_start.wrapping_add(get_next_line_bytes(
            tail(text, line_start),
            TEXT_LEN_MAX,
            font,
            None,
            &mut attributes,
        ));

        // `(unsigned long)size_res->y + letter_height + line_space > LV_MAX_OF(int32_t)`
        // with the 64-bit `unsigned long` of the upstream reference builds: a
        // negative `line_space` becomes a huge value and trips the check.
        if (size_res.y as i64 as u64)
            .wrapping_add(u64::from(letter_height))
            .wrapping_add(attributes.line_space as i64 as u64)
            > i32::MAX as u64
        {
            #[cfg(feature = "log")]
            log::warn!("integer overflow while calculating text height");
            return size_res;
        } else {
            // Plain `int32_t +=` in C: it wraps on every target, and the
            // second add can bring `y` back below `INT32_MAX`.
            size_res.y = size_res.y.wrapping_add(i32::from(letter_height));
            size_res.y = size_res.y.wrapping_add(attributes.line_space);
        }

        /* Calculate the longest line */
        let act_line_length = get_line_width_bytes(
            tail(text, line_start),
            new_line_start.wrapping_sub(line_start),
            font,
            &attributes,
        );

        size_res.x = act_line_length.max(size_res.x);
        line_start = new_line_start;
    }

    /* Make the text one line taller if the last character is '\n' or '\r' */
    if line_start != 0 && (byte_at(text, line_start - 1) == b'\n' || byte_at(text, line_start - 1) == b'\r') {
        size_res.y = size_res
            .y
            .wrapping_add(i32::from(letter_height).wrapping_add(attributes.line_space));
    }

    /* Correction with the last line space or set the height manually if the text is empty */
    if size_res.y == 0 {
        size_res.y = i32::from(letter_height);
    } else {
        size_res.y = size_res.y.wrapping_sub(attributes.line_space);
    }

    size_res
}

/// `lv_text_is_cmd()`: advance the recolor command state machine with `c`.
/// Returns `true` if `c` is part of a command (should not be drawn).
#[must_use]
pub fn is_cmd(state: &mut TextCmdState, c: u32) -> bool {
    let mut ret = false;

    if c == u32::from(TXT_COLOR_CMD.as_bytes()[0]) {
        if *state == TextCmdState::Wait {
            /* Start char */
            *state = TextCmdState::Par;
            ret = true;
        }
        /* Upstream has a second `else if(*state == LV_TEXT_CMD_STATE_WAIT)`
         * branch here which can never be taken; it is omitted. */
        else if *state == TextCmdState::In {
            /* Command end */
            *state = TextCmdState::Wait;
            ret = true;
        }
    }

    /* Skip the color parameter and wait the space after it */
    if *state == TextCmdState::Par {
        if c == ' ' as u32 {
            *state = TextCmdState::In; /* After the parameter the text is in the command */
        }
        ret = true;
    }

    ret
}

/// `lv_text_get_next_word()`: get the next word of text. A word is delimited
/// by break characters.
///
/// If the word cannot fit in the `max_width` space, obey the
/// `LV_TXT_LINE_BREAK_LONG_*` rules. If the next word cannot fit anything,
/// return 0. If the first character is a break character, returns the next
/// index.
///
/// Example calls from [`get_next_line`] assuming sufficient `max_width` and
/// `txt = "Test text\n"`:
/// 1. Return `i=4`, pointing at breakchar `' '`, for the string `"Test"`
/// 2. Return `i=5`, since `i=4` was a breakchar.
/// 3. Return `i=9`, pointing at breakchar `'\n'`
/// 4. The parent detects the subsequent `'\0'`.
///
/// `word_w_ptr` receives the width (in pixels) of the parsed word. Returns
/// the byte index of the first char of the next word.
fn get_next_word(
    txt: &[u8],
    font: &dyn Font,
    letter_space: i32,
    mut max_width: i32,
    flag: TextFlags,
    mut word_w_ptr: Option<&mut u32>,
    cmd_state: &mut TextCmdState,
) -> u32 {
    if byte_at(txt, 0) == 0 {
        return 0;
    }

    if flag.contains(TextFlags::EXPAND) {
        max_width = coord::MAX;
    }

    let mut i: u32 = 0; /* Iterating index into txt */
    let mut i_next: u32 = 0;
    let mut i_next_next: u32;
    let mut letter: u32; /* Letter at i */
    let mut letter_next: u32 = 0; /* Letter at i_next */
    let mut letter_w: i32;
    let mut cur_w: i32 = 0; /* Pixel Width of traversed string */
    let mut word_len: u32 = 0; /* Number of characters in the traversed word */
    let mut break_index: u32 = NO_BREAK_FOUND; /* only used for "long" words */
    let mut break_letter_count: u32 = 0; /* Number of characters up to the long word break point */

    letter = encoded_next(txt, Some(&mut i_next));
    i_next_next = i_next;

    /* Obtain the full word, regardless if it fits or not in max_width */
    while byte_at(txt, i) != 0 {
        letter_next = encoded_next(txt, Some(&mut i_next_next));
        word_len = word_len.wrapping_add(1);

        /* Handle the recolor command */
        if flag.contains(TextFlags::RECOLOR) && is_cmd(cmd_state, letter) {
            i = i_next;
            i_next = i_next_next;
            letter = letter_next;
            continue; /* Skip the letter if it is part of a command */
        }

        letter_w = i32::from(font::get_glyph_width(font, letter, letter_next));
        cur_w = cur_w.wrapping_add(letter_w);

        if letter_w > 0 {
            cur_w = cur_w.wrapping_add(letter_space);
        }

        let is_break = letter == '\n' as u32 || letter == '\r' as u32 || is_break_char(letter);

        /* Test if this character fits within max_width */
        if !is_hanging_space(letter)
            && break_index == NO_BREAK_FOUND
            && cur_w.wrapping_sub(letter_space) > max_width
        {
            break_index = i;
            break_letter_count = word_len.wrapping_sub(1);
            if flag.contains(TextFlags::BREAK_ALL) {
                break;
            }
            /* break_index is now pointing at the character that doesn't fit */
        }

        /* Check for new line chars and breakchars */
        if is_break {
            /* Update the output width on the first character if it fits.
             * Must do this here in case first letter is a break character. */
            if i == 0
                && break_index == NO_BREAK_FOUND
                && let Some(w) = word_w_ptr.as_deref_mut()
            {
                *w = cur_w as u32;
            }
            word_len = word_len.wrapping_sub(1);
            break;
        } else if is_a_word(letter_next) || is_a_word(letter) {
            /* Found a word for single letter, usually true for CJK */
            if let Some(w) = word_w_ptr.as_deref_mut() {
                *w = cur_w as u32;
            }
            i = i_next;
            break;
        }

        /* Update the output width */
        if break_index == NO_BREAK_FOUND
            && let Some(w) = word_w_ptr.as_deref_mut()
        {
            *w = cur_w as u32;
        }

        i = i_next;
        i_next = i_next_next;
        letter = letter_next;
    }

    /* Entire Word fits in the provided space */
    if break_index == NO_BREAK_FOUND {
        if word_len == 0 || (letter == '\r' as u32 && letter_next == '\n' as u32) {
            i = i_next;
        }
        return i;
    }

    if LINE_BREAK_LONG_ENABLED {
        /* `#if LV_TXT_LINE_BREAK_LONG_LEN > 0` */
        return long_word_break(
            txt,
            flag,
            word_w_ptr,
            word_len,
            break_index,
            break_letter_count,
            TXT_LINE_BREAK_LONG_LEN,
            TXT_LINE_BREAK_LONG_PRE_MIN_LEN,
            TXT_LINE_BREAK_LONG_POST_MIN_LEN,
        );
    }

    /* `#else` */
    if flag.contains(TextFlags::BREAK_ALL) {
        return break_index;
    }
    if let Some(w) = word_w_ptr {
        *w = 0; /* Return no word */
    }
    let _ = break_letter_count;
    0
}

/// The `#if LV_TXT_LINE_BREAK_LONG_LEN > 0` tail of `lv_text_get_next_word()`:
/// a word that does not fit may be broken inside when it is "long" enough.
/// The limits are parameters so the (currently disabled) branch stays a
/// faithful, compilable port.
fn long_word_break(
    txt: &[u8],
    flag: TextFlags,
    word_w_ptr: Option<&mut u32>,
    word_len: u32,
    break_index: u32,
    break_letter_count: u32,
    long_len: u32,
    pre_min_len: u32,
    post_min_len: u32,
) -> u32 {
    /* Word doesn't fit in provided space, but isn't "long" */
    if word_len < long_len {
        if flag.contains(TextFlags::BREAK_ALL) {
            return break_index;
        }
        if let Some(w) = word_w_ptr {
            *w = 0; /* Return no word */
        }
        return 0;
    }

    /* Word is "long," but insufficient amounts can fit in provided space */
    if break_letter_count < pre_min_len {
        if flag.contains(TextFlags::BREAK_ALL) {
            return break_index;
        }
        if let Some(w) = word_w_ptr {
            *w = 0;
        }
        return 0;
    }

    /* Word is a "long", but letters may need to be better distributed */
    let mut i = break_index;
    // `int32_t n_move = POST_MIN_LEN - (word_len - break_letter_count);` (unsigned arithmetic)
    let mut n_move = post_min_len.wrapping_sub(word_len.wrapping_sub(break_letter_count)) as i32;
    /* Move pointer "i" backwards */
    while n_move > 0 {
        encoded_prev(txt, Some(&mut i));
        n_move -= 1;
    }
    i
}

/// `is_space_run()`: tell whether a consumed run of text is nothing but
/// spaces (non-rendered soft break opportunity).
fn is_space_run(txt: &[u8], len: u32) -> bool {
    if len == 0 {
        return false;
    }

    for i in 0..len {
        if byte_at(txt, i) != b' ' {
            return false;
        }
    }

    true
}

/// `lv_text_get_line_width()`: width of the first `length` bytes of `txt`,
/// ignoring trailing hanging spaces.
#[must_use]
pub fn get_line_width(txt: &str, length: u32, font: &dyn Font, attributes: &TextAttributes) -> i32 {
    get_line_width_bytes(txt.as_bytes(), length, font, attributes)
}

/// [`get_line_width`] on raw bytes.
fn get_line_width_bytes(txt: &[u8], length: u32, font: &dyn Font, attributes: &TextAttributes) -> i32 {
    /* length may reach past the end of the string, so find the real end first */
    let mut len: u32 = 0;
    while len < length && byte_at(txt, len) != 0 {
        len += 1;
    }

    while len > 0 && is_hanging_space(u32::from(byte_at(txt, len - 1))) {
        len -= 1;
    }

    get_width_bytes(txt, len, font, attributes)
}

/// `lv_text_get_next_line()`: byte index where the next line starts.
/// `used_width`, when given, receives the width of the line.
#[must_use]
pub fn get_next_line(
    txt: &str,
    len: u32,
    font: &dyn Font,
    used_width: Option<&mut i32>,
    attributes: &mut TextAttributes,
) -> u32 {
    get_next_line_bytes(txt.as_bytes(), len, font, used_width, attributes)
}

/// [`get_next_line`] on raw bytes.
fn get_next_line_bytes(
    txt: &[u8],
    len: u32,
    font: &dyn Font,
    mut used_width: Option<&mut i32>,
    attributes: &mut TextAttributes,
) -> u32 {
    if let Some(w) = used_width.as_deref_mut() {
        *w = 0;
    }

    if byte_at(txt, 0) == 0 {
        return 0;
    }

    let mut line_w: i32 = 0;

    /* If max_width doesn't matter simply find the new line character
     * without thinking about word wrapping */
    if attributes.text_flags.contains(TextFlags::EXPAND) || attributes.text_flags.contains(TextFlags::FIT) {
        let mut i: u32 = 0;
        while i < len && byte_at(txt, i) != b'\n' && byte_at(txt, i) != b'\r' && byte_at(txt, i) != 0 {
            /* Just find the new line chars or string ends by incrementing `i` */
            i += 1;
        }
        if i < len && byte_at(txt, i) != 0 {
            i += 1; /* To go beyond `\n` */
        }
        if let Some(w) = used_width {
            *w = -1;
        }
        return i;
    }

    if attributes.text_flags.contains(TextFlags::EXPAND) {
        attributes.max_width = coord::MAX;
    }
    let mut cmd_state = TextCmdState::Wait;

    let mut i: u32 = 0; /* Iterating index into txt */
    let mut max_width = attributes.max_width as u32;
    let mut explicit_new_line = false;
    let mut hanging_w: u32 = 0; /* Width of the white space which may hang out of the line */

    while i < len && byte_at(txt, i) != 0 && max_width > hanging_w {
        let mut word_flag = attributes.text_flags;

        if i == 0 {
            word_flag |= TextFlags::BREAK_ALL;
        }

        let mut word_w: u32 = 0;
        let advance = get_next_word(
            tail(txt, i),
            font,
            attributes.letter_space,
            max_width.wrapping_sub(hanging_w) as i32,
            word_flag,
            Some(&mut word_w),
            &mut cmd_state,
        );

        if advance == 0 {
            break;
        }

        /* A space is only charged to the line if a word follows it on the same line */
        if is_space_run(tail(txt, i), advance) {
            hanging_w = hanging_w.wrapping_add(word_w);
        } else {
            max_width = max_width.wrapping_sub(word_w.wrapping_add(hanging_w));
            line_w = line_w.wrapping_add(word_w.wrapping_add(hanging_w) as i32);
            hanging_w = 0;
        }

        i = i.wrapping_add(advance);

        if byte_at(txt, 0) == b'\n' || byte_at(txt, 0) == b'\r' {
            explicit_new_line = true;
            break;
        }

        if byte_at(txt, i) == b'\n' || byte_at(txt, i) == b'\r' {
            i += 1; /* Include the following newline in the current line */
            explicit_new_line = true;
            break;
        }
    }

    /* The text ran out, not the line: lv_spangroup continues it with the next span */
    if i >= len || byte_at(txt, i) == 0 {
        line_w = line_w.wrapping_add(hanging_w as i32);
    }

    /* Always step at least one to avoid infinite loops */
    if i == 0 {
        let letter = encoded_next(txt, Some(&mut i));
        if used_width.is_some() {
            line_w = i32::from(font::get_glyph_width(font, letter, 0));
        }
    }

    if let Some(w) = used_width {
        *w = line_w;
    }

    /* Skip leading spaces of the next line only for automatic word wrapping */
    if !explicit_new_line {
        while i < len && byte_at(txt, i) == b' ' {
            i += 1;
        }
    }

    i
}

/// `lv_text_get_width()`: width of the first `length` bytes of `txt`.
#[must_use]
pub fn get_width(txt: &str, length: u32, font: &dyn Font, attributes: &TextAttributes) -> i32 {
    get_width_bytes(txt.as_bytes(), length, font, attributes)
}

/// [`get_width`] on raw bytes.
fn get_width_bytes(txt: &[u8], length: u32, font: &dyn Font, attributes: &TextAttributes) -> i32 {
    if byte_at(txt, 0) == 0 {
        return 0;
    }

    let mut i: u32 = 0;
    let mut width: i32 = 0;
    let mut cmd_state = TextCmdState::Wait;

    if length != 0 {
        while byte_at(txt, i) != 0 && i < length {
            let mut letter: u32 = 0;
            let mut letter_next: u32 = 0;

            encoded_letter_next_2_bytes(txt, &mut letter, &mut letter_next, &mut i);

            if attributes.text_flags.contains(TextFlags::RECOLOR) && is_cmd(&mut cmd_state, letter) {
                continue;
            }

            let char_width = i32::from(font::get_glyph_width(font, letter, letter_next));
            if char_width > 0 {
                width = width.wrapping_add(char_width);
                width = width.wrapping_add(attributes.letter_space);
            }
        }

        if width > 0 {
            /* Trim the last letter space. Important if the text is center aligned */
            width = width.wrapping_sub(attributes.letter_space);
        }
    }

    width
}

/// `lv_text_ins()`: insert `ins_txt` at character position `pos`.
///
/// Like upstream, both strings end at their first NUL byte (`lv_strlen()`).
#[allow(clippy::ptr_arg)] // insertion grows the buffer, a `&mut str` cannot
pub fn ins(txt_buf: &mut alloc::string::String, pos: u32, ins_txt: &str) {
    let ins_len = strlen(ins_txt.as_bytes());
    if ins_len == 0 {
        return;
    }

    /* Convert to byte index instead of letter index */
    let pos = encoded_get_byte_id(txt_buf.as_bytes(), pos) as usize;

    /* Copy the text into the new space */
    txt_buf.insert_str(pos, &ins_txt[..ins_len]);
}

/// `lv_text_cut()`: delete `len` characters starting at character `pos`.
///
/// Like upstream, the string ends at its first NUL byte (`lv_strlen()`).
#[allow(clippy::ptr_arg)] // removal shrinks the buffer, a `&mut str` cannot
pub fn cut(txt: &mut alloc::string::String, pos: u32, len: u32) {
    let old_len = strlen(txt.as_bytes()) as u32;

    /* Convert to byte index instead of letter index */
    let pos = encoded_get_byte_id(txt.as_bytes(), pos);
    if pos >= old_len {
        return;
    }

    let mut len = encoded_get_byte_id(tail(txt.as_bytes(), pos), len);
    if len > old_len - pos {
        len = old_len - pos; /* Don't cut more than what's left */
    }

    /* Move the second part down over the removed bytes */
    txt.replace_range(pos as usize..(pos + len) as usize, "");
}

/// `lv_text_set_text_vfmt()`: format `args` into a newly allocated string.
///
/// With [`USE_ARABIC_PERSIAN_CHARS`] (the `#if LV_USE_ARABIC_PERSIAN_CHARS`
/// branch) the formatted raw text is passed through `lv_text_ap_proc()`
/// ([`ap_proc`]), so Arabic/Persian letters come back in their contextual
/// presentation forms.
#[must_use]
pub fn set_text_vfmt(args: core::fmt::Arguments<'_>) -> alloc::string::String {
    if USE_ARABIC_PERSIAN_CHARS {
        /* Put together the text according to the format string */
        let raw_txt = alloc::fmt::format(args);

        /* Get the size of the Arabic text and process it */
        ap_proc(&raw_txt)
    } else {
        alloc::fmt::format(args)
    }
}

/// `lv_text_encoded_letter_next_2()`: read the letter at `ofs` and the one
/// after it, advancing `ofs`.
pub fn encoded_letter_next_2(txt: &str, letter: &mut u32, letter_next: &mut u32, ofs: &mut u32) {
    encoded_letter_next_2_bytes(txt.as_bytes(), letter, letter_next, ofs);
}

/// [`encoded_letter_next_2`] on raw bytes.
fn encoded_letter_next_2_bytes(txt: &[u8], letter: &mut u32, letter_next: &mut u32, ofs: &mut u32) {
    *letter = encoded_next(txt, Some(ofs));
    *letter_next = if *letter != 0 {
        encoded_next(tail(txt, *ofs), None)
    } else {
        0
    };
}

/*
 * UTF-8 encoder/decoder (`LV_TXT_ENC == LV_TXT_ENC_UTF8`).
 */

/// `lv_text_encoded_size()` / `lv_text_utf8_size()`: byte length of the
/// UTF-8 sequence starting at `txt[0]` (1, 2, 3 or 4), 0 on an invalid code.
#[must_use]
pub fn encoded_size(txt: &[u8]) -> u8 {
    let c = byte_at(txt, 0);
    if is_ascii(c) {
        1
    } else if is_2bytes_utf8_code(c) {
        2
    } else if is_3bytes_utf8_code(c) {
        3
    } else if is_4bytes_utf8_code(c) {
        4
    } else {
        0
    }
}

/// `lv_text_unicode_to_encoded()` / `lv_text_unicode_to_utf8()`: unicode code
/// point to UTF-8 bytes packed in native byte order ("Little Endian to be
/// compatible with C chars").
#[must_use]
pub fn unicode_to_encoded(letter_uni: u32) -> u32 {
    if letter_uni < 128 {
        return letter_uni;
    }
    let mut bytes = [0u8; 4];

    if letter_uni < 0x0800 {
        bytes[0] = (((letter_uni >> 6) & 0x1F) | 0xC0) as u8;
        bytes[1] = (((letter_uni >> 0) & 0x3F) | 0x80) as u8;
        bytes[2] = 0;
        bytes[3] = 0;
    } else if letter_uni < 0x010000 {
        bytes[0] = (((letter_uni >> 12) & 0x0F) | 0xE0) as u8;
        bytes[1] = (((letter_uni >> 6) & 0x3F) | 0x80) as u8;
        bytes[2] = (((letter_uni >> 0) & 0x3F) | 0x80) as u8;
        bytes[3] = 0;
    } else if letter_uni < 0x110000 {
        bytes[0] = (((letter_uni >> 18) & 0x07) | 0xF0) as u8;
        bytes[1] = (((letter_uni >> 12) & 0x3F) | 0x80) as u8;
        bytes[2] = (((letter_uni >> 6) & 0x3F) | 0x80) as u8;
        bytes[3] = (((letter_uni >> 0) & 0x3F) | 0x80) as u8;
    } else {
        return 0;
    }

    // `uint32_t * res_p = (uint32_t *)bytes; return *res_p;`
    u32::from_ne_bytes(bytes)
}

/// `lv_text_encoded_conv_wc()` / `lv_text_utf8_conv_wc()`: a wide character
/// (packed UTF-8 bytes, e.g. `'Á'` as a multi-char constant) to big endian.
#[must_use]
pub fn encoded_conv_wc(c: u32) -> u32 {
    /* Swap the bytes (UTF-8 is big endian, but the MCUs are little endian) */
    if cfg!(target_endian = "little") && (c & 0x80) != 0 {
        let c8 = c.to_ne_bytes();
        let mut swapped = (u32::from(c8[0]) << 24)
            .wrapping_add(u32::from(c8[1]) << 16)
            .wrapping_add(u32::from(c8[2]) << 8)
            .wrapping_add(u32::from(c8[3]));
        for _ in 0..4 {
            if (swapped & 0xFF) == 0 {
                swapped >>= 8; /* Ignore leading zeros (they were in the end originally) */
            }
        }
        return swapped;
    }
    c
}

/// `lv_text_encoded_next()` / `lv_text_utf8_next()`: decode the letter at
/// byte index `*i` and advance it. `None` reads `txt[0]`. Returns 0 on an
/// invalid UTF-8 code.
pub fn encoded_next(txt: &[u8], i: Option<&mut u32>) -> u32 {
    /*
     * Unicode to UTF-8
     * 00000000 00000000 00000000 0xxxxxxx -> 0xxxxxxx
     * 00000000 00000000 00000yyy yyxxxxxx -> 110yyyyy 10xxxxxx
     * 00000000 00000000 zzzzyyyy yyxxxxxx -> 1110zzzz 10yyyyyy 10xxxxxx
     * 00000000 000wwwzz zzzzyyyy yyxxxxxx -> 11110www 10zzzzzz 10yyyyyy 10xxxxxx
     */

    let mut result: u32 = 0;

    /* Dummy 'i' pointer is required */
    let mut i_tmp: u32 = 0;
    let i: &mut u32 = match i {
        Some(i) => i,
        None => &mut i_tmp,
    };

    /* Ensure the string is not at its end */
    if byte_at(txt, *i) == 0 {
        return result;
    }

    let c = byte_at(txt, *i);
    /* Normal ASCII */
    if is_ascii(c) {
        result = u32::from(c);
        *i += 1;
    }
    /* Real UTF-8 decode, 2 bytes UTF-8 code */
    else if is_2bytes_utf8_code(c) {
        result = u32::from(c & 0x1F) << 6;
        *i += 1;
        let c = byte_at(txt, *i);
        if is_invalid_utf8_code(c) {
            return 0;
        }
        result += u32::from(c & 0x3F);
        *i += 1;
    }
    /* 3 bytes UTF-8 code */
    else if is_3bytes_utf8_code(c) {
        result = u32::from(c & 0x0F) << 12;
        *i += 1;

        let c = byte_at(txt, *i);
        if is_invalid_utf8_code(c) {
            return 0;
        }
        result += u32::from(c & 0x3F) << 6;
        *i += 1;

        let c = byte_at(txt, *i);
        if is_invalid_utf8_code(c) {
            return 0;
        }
        result += u32::from(c & 0x3F);
        *i += 1;
    }
    /* 4 bytes UTF-8 code */
    else if is_4bytes_utf8_code(c) {
        result = u32::from(c & 0x07) << 18;
        *i += 1;

        let c = byte_at(txt, *i);
        if is_invalid_utf8_code(c) {
            return 0;
        }
        result += u32::from(c & 0x3F) << 12;
        *i += 1;

        let c = byte_at(txt, *i);
        if is_invalid_utf8_code(c) {
            return 0;
        }
        result += u32::from(c & 0x3F) << 6;
        *i += 1;

        let c = byte_at(txt, *i);
        if is_invalid_utf8_code(c) {
            return 0;
        }
        result += u32::from(c & 0x3F);
        *i += 1;
    } else {
        *i += 1; /* Not UTF-8 char. Go the next. */
    }
    result
}

/// `lv_text_encoded_prev()` / `lv_text_utf8_prev()`: decode the letter before
/// byte index `*i` and move it back. Returns 0 on an invalid UTF-8 code.
pub fn encoded_prev(txt: &[u8], i: Option<&mut u32>) -> u32 {
    let mut i_tmp_start: u32 = 0;
    let i: &mut u32 = match i {
        Some(i) => i,
        None => &mut i_tmp_start,
    };

    let mut c_size: u8;
    let mut cnt: u8 = 0;

    /* Try to find a !0 long UTF-8 char by stepping one character back */
    *i = i.wrapping_sub(1);
    loop {
        if cnt >= 4 {
            return 0; /* No UTF-8 char found before the initial */
        }

        c_size = encoded_size(tail(txt, *i));
        if c_size == 0 {
            if *i != 0 {
                *i -= 1;
            } else {
                return 0;
            }
        }
        cnt += 1;
        if c_size != 0 {
            break;
        }
    }

    let mut i_tmp = *i;
    encoded_next(txt, Some(&mut i_tmp)) /* Character found, get it */
}

/// `lv_text_encoded_get_byte_id()` / `lv_text_utf8_get_byte_id()`: character
/// index to byte index. E.g. in `"AÁRT"` `'R'` is the 2nd char but starts at
/// byte 3 because `'Á'` is 2 bytes long.
#[must_use]
pub fn encoded_get_byte_id(txt: &[u8], utf8_id: u32) -> u32 {
    let mut byte_cnt: u32 = 0;
    let mut i: u32 = 0;
    while i < utf8_id && byte_at(txt, byte_cnt) != 0 {
        let c_size = encoded_size(tail(txt, byte_cnt));
        /* If the char was invalid tell it's 1 byte long */
        byte_cnt = byte_cnt.wrapping_add(if c_size != 0 { u32::from(c_size) } else { 1 });
        i += 1;
    }

    byte_cnt
}

/// `lv_text_encoded_get_char_id()` / `lv_text_utf8_get_char_id()`: byte index
/// to character index. E.g. in `"AÁRT"` `'R'` is the 2nd char but starts at
/// byte 3 because `'Á'` is 2 bytes long.
///
/// Upstream never terminates for a `byte_id` past the end of the string;
/// here the count stops at the end of `txt` instead.
#[must_use]
pub fn encoded_get_char_id(txt: &[u8], byte_id: u32) -> u32 {
    let mut i: u32 = 0;
    let mut char_cnt: u32 = 0;

    while i < byte_id {
        if byte_at(txt, i) == 0 {
            break;
        }
        encoded_next(txt, Some(&mut i)); /* 'i' points to the next letter so use the prev. value */
        char_cnt += 1;
    }

    char_cnt
}

/// `lv_text_get_encoded_length()` / `lv_text_utf8_get_length()`: number of
/// characters (and NOT bytes) in `txt`. E.g. `"ÁBC"` is 3 characters (but 4
/// bytes).
#[must_use]
pub fn get_encoded_length(txt: &[u8]) -> u32 {
    let mut len: u32 = 0;
    let mut i: u32 = 0;

    while byte_at(txt, i) != 0 {
        encoded_next(txt, Some(&mut i));
        len += 1;
    }

    len
}

/*
 * Ported from LVGL v9.6.0: src/misc/lv_text_ap.h, src/misc/lv_text_ap.c
 * (`LV_USE_ARABIC_PERSIAN_CHARS == 1`).
 */

/// `LV_UNDEF_ARABIC_PERSIAN_CHARS`: "not an Arabic/Persian letter" index.
pub const UNDEF_ARABIC_PERSIAN_CHARS: u32 = u32::MAX;
/// `LV_AP_ALPHABET_BASE_CODE`: the code point of `char_offset == 0` in
/// `ap_chars_map[]`.
pub const AP_ALPHABET_BASE_CODE: u32 = 0x0622;

/// `ap_chars_map_t` (private in `lv_text_ap.c`).
#[derive(Copy, Clone)]
struct ApCharsMap {
    char_offset: u8,
    char_end_form: u16,
    char_beginning_form_offset: i8,
    char_middle_form_offset: i8,
    char_isolated_form_offset: i8,
    /// `ap_chars_conjunction.conj_to_previous`
    conj_to_previous: u8,
    /// `ap_chars_conjunction.conj_to_next`
    conj_to_next: u8,
}

/// One `ap_chars_map[]` row:
/// `{Key Offset, End, Beginning, Middle, Isolated, {conjunction}}`.
const fn ap(
    char_offset: u8,
    char_end_form: u16,
    char_beginning_form_offset: i8,
    char_middle_form_offset: i8,
    char_isolated_form_offset: i8,
    conj: (u8, u8),
) -> ApCharsMap {
    ApCharsMap {
        char_offset,
        char_end_form,
        char_beginning_form_offset,
        char_middle_form_offset,
        char_isolated_form_offset,
        conj_to_previous: conj.0,
        conj_to_next: conj.1,
    }
}

/// `ap_chars_map[]` (private in `lv_text_ap.c`), terminated by
/// `LV_AP_END_CHARS_LIST`.
#[rustfmt::skip]
static AP_CHARS_MAP: [ApCharsMap; 53] = [
    ap(0, 0xFE81, 0, 0, 0, (0, 0)),     // أ
    ap(1, 0xFE84, -1, 0, -1, (1, 0)),   // أ
    ap(2, 0xFE86, -1, 0, -1, (1, 0)),   // ؤ
    ap(3, 0xFE88, -1, 0, -1, (1, 0)),   // ﺇ
    ap(4, 0xFE8A, 1, 2, -1, (1, 1)),    // ئ
    ap(5, 0xFE8E, -1, 0, -1, (1, 0)),   // آ
    ap(6, 0xFE90, 1, 2, -1, (1, 1)),    // ب
    ap(92, 0xFB57, 1, 2, -1, (1, 1)),   // پ
    ap(8, 0xFE96, 1, 2, -1, (1, 1)),    // ت
    ap(9, 0xFE9A, 1, 2, -1, (1, 1)),    // ث
    ap(10, 0xFE9E, 1, 2, -1, (1, 1)),   // ج
    ap(100, 0xFB7B, 1, 2, -1, (1, 1)),  // چ
    ap(11, 0xFEA2, 1, 2, -1, (1, 1)),   // ح
    ap(12, 0xFEA6, 1, 2, -1, (1, 1)),   // خ
    ap(13, 0xFEAA, -1, 0, -1, (1, 0)),  // د
    ap(14, 0xFEAC, -1, 0, -1, (1, 0)),  // ذ
    ap(15, 0xFEAE, -1, 0, -1, (1, 0)),  // ر
    ap(16, 0xFEB0, -1, 0, -1, (1, 0)),  // ز
    ap(118, 0xFB8B, -1, 0, -1, (1, 0)), // ژ
    ap(17, 0xFEB2, 1, 2, -1, (1, 1)),   // س
    ap(18, 0xFEB6, 1, 2, -1, (1, 1)),   // ش
    ap(19, 0xFEBA, 1, 2, -1, (1, 1)),   // ص
    ap(20, 0xFEBE, 1, 2, -1, (1, 1)),   // ض
    ap(21, 0xFEC2, 1, 2, -1, (1, 1)),   // ط
    ap(22, 0xFEC6, 1, 2, -1, (1, 1)),   // ظ
    ap(23, 0xFECA, 1, 2, -1, (1, 1)),   // ع
    ap(24, 0xFECE, 1, 2, -1, (1, 1)),   // غ
    ap(30, 0x0640, 0, 0, 0, (1, 1)),    // - (mad, hyphen)
    ap(31, 0xFED2, 1, 2, -1, (1, 1)),   // ف
    ap(32, 0xFED6, 1, 2, -1, (1, 1)),   // ق
    ap(135, 0xFB8F, 1, 2, -1, (1, 1)),  // ک
    ap(33, 0xFEDA, 1, 2, -1, (1, 1)),   // ﻙ
    ap(141, 0xFB93, 1, 2, -1, (1, 1)),  // گ
    ap(34, 0xFEDE, 1, 2, -1, (1, 1)),   // ل
    ap(35, 0xFEE2, 1, 2, -1, (1, 1)),   // م
    ap(36, 0xFEE6, 1, 2, -1, (1, 1)),   // ن
    ap(38, 0xFEEE, -1, 0, -1, (1, 0)),  // و
    ap(37, 0xFEEA, 1, 2, -1, (1, 1)),   // ه
    ap(39, 0xFEF0, 0, 0, -1, (1, 0)),   // ى
    ap(40, 0xFEF2, 1, 2, -1, (1, 1)),   // ي
    ap(170, 0xFBFD, 1, 2, -1, (1, 1)),  // ی
    ap(7, 0xFE94, -1, 2, -1, (1, 0)),   // ة
    ap(206, 0x06F0, -1, 2, 0, (0, 0)),  // ۰
    ap(207, 0x06F1, 0, 0, 0, (0, 0)),   // ۱
    ap(208, 0x06F2, 0, 0, 0, (0, 0)),   // ۲
    ap(209, 0x06F3, 0, 0, 0, (0, 0)),   // ۳
    ap(210, 0x06F4, 0, 0, 0, (0, 0)),   // ۴
    ap(211, 0x06F5, 0, 0, 0, (0, 0)),   // ۵
    ap(212, 0x06F6, 0, 0, 0, (0, 0)),   // ۶
    ap(213, 0x06F7, 0, 0, 0, (0, 0)),   // ۷
    ap(214, 0x06F8, 0, 0, 0, (0, 0)),   // ۸
    ap(215, 0x06F9, 0, 0, 0, (0, 0)),   // ۹
    ap(0, 0, 0, 0, 0, (0, 0)),          // LV_AP_END_CHARS_LIST
];

/// `lv_text_ap_strlen()`: number of bytes the Arabic/Persian processed
/// version of `txt` needs, excluding the NUL terminator.
///
/// Every Arabic/Persian letter is counted with the size of its end form.
/// Like upstream, the string ends at its first NUL byte.
#[must_use]
pub fn ap_strlen(txt: &str) -> usize {
    ap_strlen_bytes(txt.as_bytes())
}

/// `lv_text_ap_strlen()` on raw bytes ([`ap_strlen`] for text that is not
/// valid UTF-8: an invalid byte counts as one byte).
#[must_use]
pub fn ap_strlen_bytes(txt: &[u8]) -> usize {
    let mut chars_cnt: u32 = 0;

    let txt_length = get_encoded_length(txt);

    let mut i: u32 = 0;
    let mut j: u32 = 0;
    while i < txt_length {
        let mut ch_enc = encoded_next(txt, Some(&mut j));
        let current_ap_idx = ap_get_char_index(ch_enc as u16);

        if current_ap_idx != UNDEF_ARABIC_PERSIAN_CHARS {
            ch_enc = u32::from(AP_CHARS_MAP[current_ap_idx as usize].char_end_form);
        }

        if ch_enc < 0x80 {
            chars_cnt = chars_cnt.wrapping_add(1);
        } else if ch_enc < 0x0800 {
            chars_cnt = chars_cnt.wrapping_add(2);
        } else if ch_enc < 0x01_0000 {
            chars_cnt = chars_cnt.wrapping_add(3);
        } else {
            chars_cnt = chars_cnt.wrapping_add(4);
        }

        i += 1;
    }

    chars_cnt as usize
}

/// `lv_text_ap_proc()`: replace the Arabic/Persian letters of `txt` with
/// their contextual (isolated, initial, medial, final) presentation forms.
/// Other characters are copied unchanged.
///
/// Upstream writes into a caller allocated `txt_out` of
/// `lv_text_ap_strlen(txt) + 1` bytes; here the result is returned. When a
/// lam-alef ligature merges two letters, upstream pads `txt_out` with NUL
/// bytes before the terminator; the returned string ends at the first of
/// them, which is where every C reader of `txt_out` stops. Like upstream,
/// `txt` ends at its first NUL byte.
#[must_use]
pub fn ap_proc(txt: &str) -> alloc::string::String {
    let txt = txt.as_bytes();

    let txt_length = get_encoded_length(txt);

    let mut ch_enc: alloc::vec::Vec<u32> = alloc::vec![0; txt_length as usize + 1];
    let mut ch_fin: alloc::vec::Vec<u32> = alloc::vec![0; txt_length as usize + 1];

    let mut i: u32 = 0;
    let mut j: u32 = 0;
    while j < txt_length {
        ch_enc[j as usize] = encoded_next(txt, Some(&mut i));
        j += 1;
    }

    ch_enc[j as usize] = 0;

    /* Upstream reads `ch_enc[i + 2]` only when `ch_enc[i + 1]` is a vowel
     * (so not the terminator) and stays in bounds; reading 0 past the end
     * keeps the port panic free whatever the input. */
    let enc_at = |ch_enc: &[u32], k: u32| -> u32 { ch_enc.get(k as usize).copied().unwrap_or(0) };

    i = 0;
    j = 0;
    let mut idx_previous = UNDEF_ARABIC_PERSIAN_CHARS;
    while i < txt_length {
        let index_current = ap_get_char_index(enc_at(&ch_enc, i) as u16);
        let mut idx_next = ap_get_char_index(enc_at(&ch_enc, i + 1) as u16);

        if text_is_arabic_vowel(enc_at(&ch_enc, i) as u16) {
            /* Current character is a vowel */
            ch_fin[j as usize] = enc_at(&ch_enc, i);
            i += 1;
            j += 1;
            continue; /* Skip this character */
        } else if text_is_arabic_vowel(enc_at(&ch_enc, i + 1) as u16) {
            /* Next character is a vowel:
             * skip the vowel character to join with the character after it */
            idx_next = ap_get_char_index(enc_at(&ch_enc, i + 2) as u16);
        }

        if index_current == UNDEF_ARABIC_PERSIAN_CHARS {
            ch_fin[j as usize] = enc_at(&ch_enc, i);
            j += 1;
            i += 1;
            idx_previous = UNDEF_ARABIC_PERSIAN_CHARS;
            continue;
        }

        let conjunction_to_previous: u8 = if i == 0 || idx_previous == UNDEF_ARABIC_PERSIAN_CHARS {
            0
        } else {
            AP_CHARS_MAP[idx_previous as usize].conj_to_next
        };
        let conjunction_to_next: u8 =
            if i == txt_length.wrapping_sub(1) || idx_next == UNDEF_ARABIC_PERSIAN_CHARS {
                0
            } else {
                AP_CHARS_MAP[idx_next as usize].conj_to_previous
            };

        let mut lam_alef = text_lam_alef(index_current, idx_next);
        if lam_alef != 0 {
            if conjunction_to_previous != 0 {
                lam_alef += 1;
            }
            ch_fin[j as usize] = lam_alef;
            idx_previous = UNDEF_ARABIC_PERSIAN_CHARS;
            i += 2;
            j += 1;
            continue;
        }

        let cur = &AP_CHARS_MAP[index_current as usize];
        let end_form = i32::from(cur.char_end_form);
        ch_fin[j as usize] = if conjunction_to_previous != 0 && conjunction_to_next != 0 {
            (end_form + i32::from(cur.char_middle_form_offset)) as u32
        } else if conjunction_to_previous == 0 && conjunction_to_next != 0 {
            (end_form + i32::from(cur.char_beginning_form_offset)) as u32
        } else if conjunction_to_previous != 0 && conjunction_to_next == 0 {
            end_form as u32
        } else {
            (end_form + i32::from(cur.char_isolated_form_offset)) as u32
        };
        idx_previous = index_current;
        i += 1;
        j += 1;
    }
    ch_fin[j as usize] = 0;
    for k in 0..txt_length as usize {
        ch_enc[k] = 0;
    }
    ch_enc[..j as usize].copy_from_slice(&ch_fin[..j as usize]);
    drop(ch_fin);

    let mut txt_out: alloc::vec::Vec<u8> = alloc::vec::Vec::with_capacity(ap_strlen_bytes(txt) + 1);

    for &c in &ch_enc[..txt_length as usize] {
        if c < 0x80 {
            txt_out.push((c & 0xFF) as u8);
        } else if c < 0x0800 {
            txt_out.push((((c >> 6) & 0x1F) | 0xC0) as u8);
            txt_out.push(((c & 0x3F) | 0x80) as u8);
        } else if c < 0x01_0000 {
            txt_out.push((((c >> 12) & 0x0F) | 0xE0) as u8);
            txt_out.push((((c >> 6) & 0x3F) | 0x80) as u8);
            txt_out.push(((c & 0x3F) | 0x80) as u8);
        } else if c < 0x11_0000 {
            txt_out.push((((c >> 18) & 0x07) | 0xF0) as u8);
            txt_out.push((((c >> 12) & 0x3F) | 0x80) as u8);
            txt_out.push((((c >> 6) & 0x3F) | 0x80) as u8);
            txt_out.push(((c & 0x3F) | 0x80) as u8);
        }
    }

    /* `*(txt_out_temp) = '\0'`: the C string ends at its first NUL byte */
    txt_out.truncate(strlen(&txt_out));
    match alloc::string::String::from_utf8(txt_out) {
        Ok(s) => s,
        /* Not reached: every code point written is a Unicode scalar value */
        Err(e) => alloc::string::String::from_utf8_lossy(e.as_bytes()).into_owned(),
    }
}

/// `lv_ap_get_char_index()` (static in `lv_text_ap.c`). Takes a `uint16_t`
/// like upstream, so callers truncate the code point.
#[allow(clippy::if_same_then_else)] // keep the upstream if / else-if shape
fn ap_get_char_index(c: u16) -> u32 {
    let c = i32::from(c);
    let mut i: u8 = 0;
    while AP_CHARS_MAP[i as usize].char_end_form != 0 {
        let m = &AP_CHARS_MAP[i as usize];
        let end = i32::from(m.char_end_form);
        if c == i32::from(m.char_offset) + AP_ALPHABET_BASE_CODE as i32 {
            return u32::from(i);
        } else if c == end // is it an End form
            || c == end + i32::from(m.char_beginning_form_offset) // is it a Beginning form
            || c == end + i32::from(m.char_middle_form_offset) // is it a middle form
            || c == end + i32::from(m.char_isolated_form_offset)
        // is it an isolated form
        {
            return u32::from(i);
        }
        i += 1;
    }
    UNDEF_ARABIC_PERSIAN_CHARS
}

/// `lv_text_lam_alef()` (static in `lv_text_ap.c`): the lam-alef ligature
/// for the `ap_chars_map[]` indexes `ch_curr` and `ch_next`, or 0.
fn text_lam_alef(ch_curr: u32, ch_next: u32) -> u32 {
    if AP_CHARS_MAP[ch_curr as usize].char_offset != 34 {
        return 0;
    }
    if ch_next == UNDEF_ARABIC_PERSIAN_CHARS {
        return 0;
    }
    let ch_code = u32::from(AP_CHARS_MAP[ch_next as usize].char_offset) + AP_ALPHABET_BASE_CODE;
    if ch_code == 0x0622 {
        return 0xFEF5; /* (lam-alef) mad */
    }
    if ch_code == 0x0623 {
        return 0xFEF7; /* (lam-alef) top hamza */
    }
    if ch_code == 0x0625 {
        return 0xFEF9; /* (lam-alef) bot hamza */
    }
    if ch_code == 0x0627 {
        return 0xFEFB; /* (lam-alef) alef */
    }
    0
}

/// `lv_text_is_arabic_vowel()` (static in `lv_text_ap.c`).
const fn text_is_arabic_vowel(c: u16) -> bool {
    (c >= 0x064B) && (c <= 0x0652)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::GlyphDsc;
    use alloc::string::String;

    /// A monospace test font: letters are 10 px wide, spaces 5 px, markers 0.
    struct TestFont;

    impl Font for TestFont {
        fn line_height(&self) -> i32 {
            20
        }
        fn base_line(&self) -> i32 {
            4
        }
        fn cap_height(&self) -> i32 {
            14
        }
        fn x_height(&self) -> i32 {
            10
        }
        fn underline_position(&self) -> i8 {
            -2
        }
        fn underline_thickness(&self) -> i8 {
            1
        }
        fn glyph_dsc(&self, out: &mut GlyphDsc, letter: u32, _letter_next: u32) -> bool {
            out.adv_w = if letter == ' ' as u32 { 5 } else { 10 };
            out.box_w = out.adv_w;
            out.box_h = 20;
            true
        }
        fn glyph_bitmap_a8(&self, _g: &GlyphDsc, _out: &mut [u8], _stride: usize) -> bool {
            false
        }
    }

    fn attrs(max_width: i32, flags: TextFlags) -> TextAttributes {
        TextAttributes {
            letter_space: 0,
            line_space: 0,
            max_width,
            text_flags: flags,
        }
    }

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
        assert_eq!(TextCmdState::In as u8, 2);
    }

    /* test_txt.c */

    #[test]
    fn txt_should_insert_string_into_another() {
        let mut target = String::from("Hello ");
        ins(&mut target, 6, "World");
        assert_eq!(target, "Hello World");
    }

    #[test]
    fn txt_should_handle_empty_insert() {
        /* `lv_text_ins(target, msg_len, NULL)` leaves the text alone */
        let mut target = String::from("Hello ");
        ins(&mut target, 6, "");
        assert_eq!(target, "Hello ");
    }

    #[test]
    fn txt_cut_happy_path() {
        let mut msg = String::from("Hello World");
        cut(&mut msg, 0, 6);
        assert_eq!(msg, "World");
    }

    #[test]
    fn txt_cut_should_handle_len_longer_than_string_length() {
        let mut msg = String::from("Hello World");
        cut(&mut msg, 0, 30);
        assert_eq!(msg, "");
    }

    #[test]
    fn txt_cut_should_handle_len_longer_than_remaining_string() {
        let mut msg = String::from("Hello World");
        cut(&mut msg, 6, 30);
        assert_eq!(msg, "Hello ");
    }

    #[test]
    fn txt_cut_should_handle_pos_beyond_string_length() {
        let mut msg = String::from("Hello World");
        cut(&mut msg, 30, 1);
        assert_eq!(msg, "Hello World");
    }

    #[test]
    fn txt_get_encoded_next_should_decode_valid_ascii() {
        assert_eq!(encoded_next(b"Hello World!", None), 'H' as u32);
    }

    #[test]
    fn txt_get_encoded_next_detect_valid_2_byte_input() {
        assert_eq!(encoded_next(b"\xc3\xb1", None), 241);
    }

    #[test]
    fn txt_get_encoded_next_detect_invalid_2_byte_input() {
        assert_eq!(encoded_next(b"\xc3\x28", None), 0);
    }

    #[test]
    fn txt_get_encoded_next_detect_valid_3_byte_input() {
        assert_eq!(encoded_next(b"\xe2\x82\xa1", None), 8353);
    }

    #[test]
    fn txt_get_encoded_next_detect_invalid_3_byte_input() {
        assert_eq!(encoded_next(b"\xe2\x28\xa1", None), 0);
    }

    #[test]
    fn txt_get_encoded_next_detect_valid_4_byte_input() {
        assert_eq!(encoded_next(b"\xf0\x90\x8c\xbc", None), 66364);
    }

    #[test]
    fn txt_get_encoded_next_detect_invalid_4_byte_input() {
        assert_eq!(encoded_next(b"\xf0\x28\x8c\x28", None), 0);
    }

    /* See #2615 for more information */
    #[test]
    fn txt_next_line_should_handle_empty_string() {
        let mut attributes = attrs(0, TextFlags::NONE);
        let next_line = get_next_line("", 0, &TestFont, None, &mut attributes);
        assert_eq!(next_line, 0);
    }

    #[test]
    fn encoded_letter_next_2_should_handle_empty_string() {
        let (mut letter, mut letter_next, mut ofs) = (100, 101, 0);
        encoded_letter_next_2("", &mut letter, &mut letter_next, &mut ofs);
        assert_eq!(letter, 0);
        assert_eq!(letter_next, 0);
        assert_eq!(ofs, 0);
    }

    #[test]
    fn encoded_letter_next_2_should_handle_single_ascii_character() {
        let (mut letter, mut letter_next, mut ofs) = (0, 0, 0);
        encoded_letter_next_2("A", &mut letter, &mut letter_next, &mut ofs);
        assert_eq!(letter, 'A' as u32);
        assert_eq!(letter_next, 0);
        assert_eq!(ofs, 1);
    }

    #[test]
    fn encoded_letter_next_2_should_handle_utf8_multibyte_character() {
        let (mut letter, mut letter_next, mut ofs) = (0, 0, 0);
        encoded_letter_next_2("é", &mut letter, &mut letter_next, &mut ofs);
        assert_eq!(letter, 0xE9);
        assert_eq!(letter_next, 0);
        assert_eq!(ofs, 2);
    }

    #[test]
    fn encoded_letter_next_2_should_handle_two_utf8_characters() {
        let (mut letter, mut letter_next, mut ofs) = (0, 0, 0);
        encoded_letter_next_2("éA", &mut letter, &mut letter_next, &mut ofs);
        assert_eq!(letter, 0xE9);
        assert_eq!(letter_next, 'A' as u32);
        assert_eq!(ofs, 2);
    }

    /* Invariants derived from lv_text.c */

    #[test]
    fn utf8_index_conversions() {
        let txt = "AÁRT".as_bytes();
        assert_eq!(encoded_get_byte_id(txt, 0), 0);
        assert_eq!(encoded_get_byte_id(txt, 1), 1);
        assert_eq!(encoded_get_byte_id(txt, 2), 3);
        assert_eq!(encoded_get_byte_id(txt, 4), 5);
        assert_eq!(encoded_get_byte_id(txt, 10), 5); /* stops at the end */
        assert_eq!(encoded_get_char_id(txt, 3), 2);
        assert_eq!(encoded_get_char_id(txt, 5), 4);
        assert_eq!(encoded_get_char_id(txt, 0), 0);
        assert_eq!(get_encoded_length("ÁBC".as_bytes()), 3);
        assert_eq!(get_encoded_length(b""), 0);
        assert_eq!(encoded_size("Á".as_bytes()), 2);
        assert_eq!(encoded_size("中".as_bytes()), 3);
        assert_eq!(encoded_size("😀".as_bytes()), 4);
        assert_eq!(encoded_size(b"A"), 1);
        assert_eq!(encoded_size(b"\x80"), 0); /* continuation byte */
    }

    #[test]
    fn utf8_prev_walks_backwards() {
        let txt = "éA中".as_bytes(); /* 2 + 1 + 3 bytes */
        let mut i = txt.len() as u32;
        assert_eq!(encoded_prev(txt, Some(&mut i)), 0x4E2D);
        assert_eq!(i, 3);
        assert_eq!(encoded_prev(txt, Some(&mut i)), 'A' as u32);
        assert_eq!(i, 2);
        assert_eq!(encoded_prev(txt, Some(&mut i)), 0xE9);
        assert_eq!(i, 0);
        /* Nothing but continuation bytes before `i`: give up after 4 steps */
        let mut j = 5;
        assert_eq!(encoded_prev(b"\x80\x80\x80\x80\x80\x80", Some(&mut j)), 0);
    }

    #[test]
    fn unicode_to_encoded_and_back() {
        assert_eq!(unicode_to_encoded('A' as u32), 'A' as u32);
        assert_eq!(unicode_to_encoded(0xC1), u32::from_ne_bytes([0xC3, 0x81, 0, 0]));
        assert_eq!(
            unicode_to_encoded(0x4E2D),
            u32::from_ne_bytes([0xE4, 0xB8, 0xAD, 0])
        );
        assert_eq!(
            unicode_to_encoded(0x1F600),
            u32::from_ne_bytes([0xF0, 0x9F, 0x98, 0x80])
        );
        assert_eq!(unicode_to_encoded(0x110000), 0);

        /* The C multi-char constant 'Á' (0xC3 0x81) read as a little endian int */
        if cfg!(target_endian = "little") {
            assert_eq!(encoded_conv_wc(unicode_to_encoded(0xC1)), 0xC381);
            assert_eq!(encoded_conv_wc(unicode_to_encoded(0x4E2D)), 0xE4B8AD);
            assert_eq!(encoded_conv_wc(unicode_to_encoded(0x1F600)), 0xF09F9880);
        }
        assert_eq!(encoded_conv_wc('A' as u32), 'A' as u32);
    }

    #[test]
    fn is_cmd_state_machine() {
        let mut state = TextCmdState::Wait;
        assert!(!is_cmd(&mut state, 'a' as u32));
        assert_eq!(state, TextCmdState::Wait);
        assert!(is_cmd(&mut state, '#' as u32));
        assert_eq!(state, TextCmdState::Par);
        assert!(is_cmd(&mut state, 'f' as u32));
        assert_eq!(state, TextCmdState::Par);
        assert!(is_cmd(&mut state, ' ' as u32));
        assert_eq!(state, TextCmdState::In);
        assert!(!is_cmd(&mut state, 'H' as u32));
        assert_eq!(state, TextCmdState::In);
        assert!(is_cmd(&mut state, '#' as u32));
        assert_eq!(state, TextCmdState::Wait);
    }

    #[test]
    fn get_width_counts_glyphs_and_letter_space() {
        let a = TextAttributes {
            letter_space: 2,
            ..attrs(0, TextFlags::NONE)
        };
        assert_eq!(get_width("", 5, &TestFont, &a), 0);
        assert_eq!(get_width("abc", 0, &TestFont, &a), 0);
        /* 3 letters: 10+2 each, last letter space trimmed */
        assert_eq!(get_width("abc", 3, &TestFont, &a), 34);
        assert_eq!(get_width("abc", 2, &TestFont, &a), 22);
        /* length past the end is fine */
        assert_eq!(get_width("abc", 100, &TestFont, &a), 34);
        /* markers (e.g. '\n') have no width and no letter space */
        assert_eq!(get_width("a\nb", 3, &TestFont, &a), 22);
        /* "é" is 2 bytes, one glyph */
        assert_eq!(get_width("éa", 3, &TestFont, &a), 22);
    }

    #[test]
    fn get_width_skips_recolor_commands() {
        let plain = attrs(0, TextFlags::NONE);
        let recolor = attrs(0, TextFlags::RECOLOR);
        assert_eq!(get_width("#ff0000 Hi#", 11, &TestFont, &plain), 105);
        assert_eq!(get_width("#ff0000 Hi#", 11, &TestFont, &recolor), 20);
    }

    #[test]
    fn get_line_width_trims_hanging_spaces() {
        let a = attrs(0, TextFlags::NONE);
        assert_eq!(get_line_width("ab  ", 4, &TestFont, &a), 20);
        assert_eq!(get_line_width("ab\n", 3, &TestFont, &a), 20);
        assert_eq!(get_line_width("   ", 3, &TestFont, &a), 0);
        assert_eq!(get_line_width("ab cd", 3, &TestFont, &a), 20);
    }

    #[test]
    fn next_line_follows_the_documented_example() {
        /* "Test text\n" with plenty of room: one line of 10 bytes, 85 px wide */
        let mut a = attrs(1000, TextFlags::NONE);
        let mut w = 0;
        let next = get_next_line("Test text\n", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a);
        assert_eq!(next, 10);
        assert_eq!(w, 40 + 5 + 40);
    }

    #[test]
    fn next_line_wraps_at_break_chars() {
        let mut a = attrs(60, TextFlags::NONE);
        let mut w = 0;
        let next = get_next_line("Hello World", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a);
        /* "Hello " is consumed, the hanging space is not charged to the line */
        assert_eq!(next, 6);
        assert_eq!(w, 50);
        let mut w = 0;
        let next = get_next_line("World", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a);
        assert_eq!(next, 5);
        assert_eq!(w, 50);
    }

    #[test]
    fn next_line_trailing_spaces_count_when_text_ends() {
        let mut a = attrs(1000, TextFlags::NONE);
        let mut w = 0;
        let next = get_next_line("ab  ", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a);
        assert_eq!(next, 4);
        assert_eq!(w, 30);
    }

    #[test]
    fn next_line_breaks_a_too_long_first_word_anywhere() {
        /* The first word of a line always gets LV_TEXT_FLAG_BREAK_ALL */
        let mut a = attrs(25, TextFlags::NONE);
        let mut w = 0;
        let next = get_next_line("abcdef", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a);
        assert_eq!(next, 2);
        assert_eq!(w, 20);
    }

    #[test]
    fn next_line_break_all_wraps_inside_words() {
        let mut a = attrs(55, TextFlags::BREAK_ALL);
        let mut w = 0;
        let next = get_next_line("abc defgh", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a);
        /* "abc" (30) + " " (5) + "de" (20) = 55 */
        assert_eq!(next, 6);
        assert_eq!(w, 55);
    }

    #[test]
    fn next_line_always_advances_one_letter() {
        /* Nothing fits: still consume one (multi-byte) letter */
        let mut a = attrs(5, TextFlags::NONE);
        let mut w = 0;
        let next = get_next_line("éa", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a);
        assert_eq!(next, 2);
        assert_eq!(w, 10);
    }

    #[test]
    fn next_line_handles_explicit_new_lines() {
        let mut a = attrs(1000, TextFlags::NONE);
        let mut w = 0;
        /* A leading newline is a line of its own */
        assert_eq!(
            get_next_line("\nabc", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a),
            1
        );
        assert_eq!(w, 0);
        /* "\r\n" is consumed as one break */
        assert_eq!(
            get_next_line("ab\r\ncd", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a),
            4
        );
        assert_eq!(w, 20);
        /* Leading spaces of the next line are kept after an explicit break... */
        assert_eq!(
            get_next_line("ab\n  cd", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a),
            3
        );
        /* ...but skipped after automatic wrapping */
        let mut a = attrs(20, TextFlags::NONE);
        assert_eq!(
            get_next_line("ab   cd", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a),
            5
        );
        assert_eq!(w, 20);
    }

    #[test]
    fn next_line_respects_len() {
        let mut a = attrs(1000, TextFlags::NONE);
        let mut w = 0;
        assert_eq!(get_next_line("abc def", 3, &TestFont, Some(&mut w), &mut a), 3);
        assert_eq!(w, 30);
    }

    #[test]
    fn next_line_expand_and_fit_only_look_for_new_lines() {
        for flags in [TextFlags::EXPAND, TextFlags::FIT] {
            let mut a = attrs(5, flags);
            let mut w = 0;
            assert_eq!(
                get_next_line("ab\ncd", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a),
                3
            );
            assert_eq!(w, -1);
            assert_eq!(
                get_next_line("cd", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a),
                2
            );
            assert_eq!(get_next_line("abcdef", 2, &TestFont, Some(&mut w), &mut a), 2);
        }
    }

    #[test]
    fn next_line_cjk_letters_are_words() {
        let mut a = attrs(15, TextFlags::NONE);
        let mut w = 0;
        let next = get_next_line("中文", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a);
        assert_eq!(next, 3);
        assert_eq!(w, 10);
        let mut a = attrs(1000, TextFlags::NONE);
        assert_eq!(
            get_next_line("中文", TEXT_LEN_MAX, &TestFont, Some(&mut w), &mut a),
            6
        );
        assert_eq!(w, 20);
    }

    #[test]
    fn get_size_measures_lines() {
        let f = &TestFont;
        /* Empty text is one line tall */
        assert_eq!(get_size("", f, 0, 0, 1000, TextFlags::NONE), Point::new(0, 20));
        assert_eq!(
            get_size("abc", f, 0, 0, 1000, TextFlags::NONE),
            Point::new(30, 20)
        );
        /* Explicit lines, line space applied between lines only */
        assert_eq!(
            get_size("a\nbc", f, 0, 3, 1000, TextFlags::NONE),
            Point::new(20, 43)
        );
        /* A trailing newline adds an empty line */
        assert_eq!(
            get_size("a\n", f, 0, 3, 1000, TextFlags::NONE),
            Point::new(10, 43)
        );
        /* Automatic wrapping */
        assert_eq!(
            get_size("Hello World", f, 0, 0, 60, TextFlags::NONE),
            Point::new(50, 40)
        );
        /* EXPAND ignores max_width */
        assert_eq!(
            get_size("Hello World", f, 0, 0, 60, TextFlags::EXPAND),
            Point::new(105, 20)
        );
        /* Letter space */
        assert_eq!(get_size("ab", f, 4, 0, 1000, TextFlags::NONE), Point::new(24, 20));
    }

    #[test]
    fn get_size_attributes_matches_get_size() {
        let a = TextAttributes {
            letter_space: 1,
            line_space: 2,
            max_width: 60,
            text_flags: TextFlags::NONE,
        };
        assert_eq!(
            get_size_attributes("Hello World", &TestFont, &a),
            get_size("Hello World", &TestFont, 1, 2, 60, TextFlags::NONE)
        );
    }

    #[test]
    fn font_trims() {
        let f = &TestFont;
        assert_eq!(font_get_bottom_trim(f, LeadingTrim::None), 0);
        assert_eq!(font_get_bottom_trim(f, LeadingTrim::CapitalBaseline), 4);
        assert_eq!(font_get_bottom_trim(f, LeadingTrim::LowerBaseline), 4);
        assert_eq!(font_get_bottom_trim(f, LeadingTrim::Capital), 0);
        assert_eq!(font_get_top_trim(f, LeadingTrim::None), 0);
        assert_eq!(font_get_top_trim(f, LeadingTrim::Capital), 20 - 4 - 14);
        assert_eq!(font_get_top_trim(f, LeadingTrim::LowerBaseline), 20 - 4 - 10);
    }

    #[test]
    fn ins_and_cut_use_character_positions() {
        let mut s = String::from("AÁRT");
        ins(&mut s, 2, "é");
        assert_eq!(s, "AÁéRT");
        cut(&mut s, 1, 2);
        assert_eq!(s, "ART");
        /* NUL terminates like a C string */
        let mut s = String::from("ab\0cd");
        ins(&mut s, 5, "X");
        assert_eq!(s, "abX\0cd");
        assert_eq!(set_text_vfmt(format_args!("{}-{}", 1, "x")), "1-x");
    }

    /// A font whose `line_height` is the largest `uint16_t letter_height`.
    struct TallFont;

    impl Font for TallFont {
        fn line_height(&self) -> i32 {
            65535
        }
        fn base_line(&self) -> i32 {
            0
        }
        fn underline_position(&self) -> i8 {
            0
        }
        fn underline_thickness(&self) -> i8 {
            0
        }
        fn glyph_dsc(&self, out: &mut GlyphDsc, _letter: u32, _letter_next: u32) -> bool {
            out.adv_w = 10;
            true
        }
        fn glyph_bitmap_a8(&self, _g: &GlyphDsc, _out: &mut [u8], _stride: usize) -> bool {
            false
        }
    }

    #[test]
    fn get_size_wraps_intermediate_height_like_c() {
        /* line 32949 (0-based): y = 32949 * 65174 = 2147418126 passes the
         * guard (y + 65535 - 361 <= INT32_MAX) but y + 65535 alone exceeds
         * INT32_MAX; C wraps there and the `line_space` add brings it back.
         * The next line trips the guard and returns early. */
        let text = "\n".repeat(32951);
        let a = TextAttributes {
            letter_space: 0,
            line_space: -361,
            max_width: coord::MAX,
            text_flags: TextFlags::NONE,
        };
        let size = get_size_attributes(&text, &TallFont, &a);
        assert_eq!(size, Point::new(0, 2147483300));
    }

    /* lv_text_ap.c */

    #[test]
    fn ap_proc_shapes_contextual_forms() {
        /* ba + alef: initial ba, final alef */
        assert_eq!(ap_proc("\u{0628}\u{0627}"), "\u{FE91}\u{FE8E}");
        /* ba ba ba: initial, medial, final */
        assert_eq!(ap_proc("\u{0628}\u{0628}\u{0628}"), "\u{FE91}\u{FE92}\u{FE90}");
        /* a lone ba is isolated; a space breaks the joining */
        assert_eq!(ap_proc("\u{0628} \u{0628}"), "\u{FE8F} \u{FE8F}");
        /* a vowel is copied and skipped when joining */
        assert_eq!(ap_proc("\u{0628}\u{064E}\u{0628}"), "\u{FE91}\u{064E}\u{FE90}");
        /* non Arabic/Persian text is unchanged */
        assert_eq!(ap_proc("Hello, ÁÉ 0x1F"), "Hello, ÁÉ 0x1F");
        assert_eq!(ap_proc(""), "");
    }

    #[test]
    fn ap_proc_lam_alef_ligature() {
        assert_eq!(ap_proc("\u{0644}\u{0627}"), "\u{FEFB}");
        /* joined to a previous letter: the final ligature form */
        assert_eq!(ap_proc("\u{0628}\u{0644}\u{0627}"), "\u{FE91}\u{FEFC}");
        /* upstream quirk: lam + vowel + alef emits the ligature, drops the
         * vowel and then shapes the alef again on its own */
        assert_eq!(ap_proc("\u{0644}\u{064E}\u{0627}"), "\u{FEFB}\u{FE8D}");
    }

    #[test]
    fn ap_proc_matches_upstream_quirks() {
        /* `lv_ap_get_char_index(uint16_t)` truncates: U+1FE90 looks like ba */
        assert_eq!(ap_proc("\u{1FE90}"), "\u{FE8F}");
        /* the string ends at its first NUL byte */
        assert_eq!(ap_proc("ab\0cd"), "ab");
        /* already shaped forms are recognized and shaped again */
        assert_eq!(ap_proc("\u{FE91}\u{FE8E}"), "\u{FE91}\u{FE8E}");
    }

    #[test]
    fn ap_strlen_counts_end_forms() {
        assert_eq!(ap_strlen("abc"), 3);
        /* 2-byte letters counted with their 3-byte end forms */
        assert_eq!(ap_strlen("\u{0628}\u{0627}"), 6);
        assert_eq!(ap_strlen("\u{0644}\u{0627}"), 6);
        /* the vowel stays 2 bytes; U+1FE90 is counted as 0xFE90 */
        assert_eq!(ap_strlen("\u{064E}\u{1FE90}"), 5);
        assert_eq!(ap_strlen("ab\0cd"), 2);
        /* the output always fits the buffer upstream allocates */
        for s in [
            "\u{0644}\u{064E}\u{0627}",
            "\u{0628}\u{0644}\u{0627}x",
            "\u{06F0}\u{0640}",
        ] {
            assert!(ap_proc(s).len() <= ap_strlen(s));
        }
    }

    #[test]
    fn set_text_vfmt_applies_arabic_persian_processing() {
        const { assert!(USE_ARABIC_PERSIAN_CHARS) };
        assert_eq!(
            set_text_vfmt(format_args!("{}", "\u{0628}\u{0627}")),
            "\u{FE91}\u{FE8E}"
        );
        assert_eq!(set_text_vfmt(format_args!("{}%", 42)), "42%");
    }
}
