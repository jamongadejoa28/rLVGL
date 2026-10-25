//! Ported from LVGL v9.6.0: `tests/src/test_cases/test_bidi.c`,
//! `tests/src/test_cases/test_text_ap.c`, and the RTL screenshot cases of
//! `tests/src/test_cases/widgets/test_checkbox.c` (`test_checkbox_rtl`) and
//! `tests/src/test_cases/widgets/test_label.c` (`test_label_rtl_dot_long_mode`).
//!
//! Every upstream test function of `test_bidi.c` and `test_text_ap.c`, in
//! upstream order, with its values. The `lv_bidi.c` file-scope state
//! (`custom_neutrals`) is a [`BidiState`]; `test_bidi.c`'s `tearDown()`
//! restores the default neutrals, which a fresh `BidiState::new()` per test
//! is. `lv_malloc`ed guard buffers are `Vec`s; a write outside the C buffer
//! would panic on the slice bound here, and the guard bytes are checked too.

use ferrovgl_core::misc::bidi::{BaseDir, BidiState};
use ferrovgl_core::misc::color::OPA_100;
use ferrovgl_core::misc::text::TextAlign;
use ferrovgl_core::misc::text_ap::{ap_proc, ap_strlen, ap_strlen_bytes};
use ferrovgl_core::{Color, Font, FontRef, Obj, StyleSelector, Ui};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Checkbox, Label, LabelLongMode};

const MAIN: StyleSelector = StyleSelector::DEFAULT;

/// `&lv_font_dejavu_16_persian_hebrew`
fn dejavu() -> FontRef {
    FontRef::Static(&ferrovgl_fonts::DEJAVU_16_PERSIAN_HEBREW)
}

/*
 * test_bidi.c
 */

/// `GUARD_SIZE`
const GUARD_SIZE: usize = 16;
/// `GUARD_BYTE`
const GUARD_BYTE: u8 = 0xAA;

/// `bidi_process_check_overflow()`: run `lv_bidi_process_paragraph()` on a
/// `len + 1` byte output between two guard zones; `true` if no guard byte
/// changed.
fn bidi_process_check_overflow(s: &BidiState, str_in: &[u8], len: u32, base_dir: BaseDir) -> bool {
    /* Allocate buffer with guard zones before and after */
    let total = GUARD_SIZE + len as usize + 1 + GUARD_SIZE;
    /* Fill entire buffer with guard byte */
    let mut buf = vec![GUARD_BYTE; total];

    {
        /* str_out points to the usable region */
        let str_out = &mut buf[GUARD_SIZE..GUARD_SIZE + len as usize + 1];
        /* Clear the usable region */
        str_out.fill(0);

        s.process_paragraph(str_in, Some(str_out), len, base_dir, None, 0);
    }

    /* Check pre-guard and post-guard */
    buf[..GUARD_SIZE].iter().all(|&b| b == GUARD_BYTE)
        && buf[GUARD_SIZE + len as usize + 1..]
            .iter()
            .all(|&b| b == GUARD_BYTE)
}

/// `strlen()`
fn strlen(s: &[u8]) -> u32 {
    s.iter().position(|&b| b == 0).unwrap_or(s.len()) as u32
}

/// Test 1: pure Arabic text with RTL base direction, odd lengths.
#[test]
fn bidi_rtl_arabic_odd_len_no_overflow() {
    let s = BidiState::new();
    /* Arabic: U+0639 U+0631 U+0628 U+064A (8 bytes) */
    let arabic = b"\xd8\xb9\xd8\xb1\xd8\xa8\xd9\x8a";

    /* len=7: cuts the last 2-byte char in half */
    assert!(bidi_process_check_overflow(&s, arabic, 7, BaseDir::Rtl));
    /* len=5: cuts in the middle */
    assert!(bidi_process_check_overflow(&s, arabic, 5, BaseDir::Rtl));
    /* len=3 */
    assert!(bidi_process_check_overflow(&s, arabic, 3, BaseDir::Rtl));
    /* len=1: minimal case */
    assert!(bidi_process_check_overflow(&s, arabic, 1, BaseDir::Rtl));
}

/// Test 2: mixed ASCII + Arabic with RTL base direction.
#[test]
fn bidi_rtl_mixed_neutral_prefix_no_overflow() {
    let s = BidiState::new();
    /* Space + Arabic chars */
    let mixed = b" \xd8\xb9\xd8\xb1\xd8\xa8";

    /* len=6: space(1) + 2.5 Arabic chars, cuts mid-char */
    assert!(bidi_process_check_overflow(&s, mixed, 6, BaseDir::Rtl));
    /* len=4 */
    assert!(bidi_process_check_overflow(&s, mixed, 4, BaseDir::Rtl));
    /* len=2 */
    assert!(bidi_process_check_overflow(&s, mixed, 2, BaseDir::Rtl));
}

/// Test 3: Arabic text with LTR base direction.
#[test]
fn bidi_ltr_arabic_odd_len_no_overflow() {
    let s = BidiState::new();
    let arabic = b"\xd8\xb9\xd8\xb1\xd8\xa8\xd9\x8a";

    assert!(bidi_process_check_overflow(&s, arabic, 7, BaseDir::Ltr));
    assert!(bidi_process_check_overflow(&s, arabic, 5, BaseDir::Ltr));
    assert!(bidi_process_check_overflow(&s, arabic, 3, BaseDir::Ltr));
}

/// Test 4: multiple Arabic words separated by spaces create multiple runs.
#[test]
fn bidi_rtl_multi_run_boundary_no_overflow() {
    let s = BidiState::new();
    /* Arabic word + space + Arabic word */
    let multi = b"\xd8\xb9\xd8\xb1 \xd8\xa8\xd9\x8a";

    /* Sweep through all len values to catch any boundary issue */
    let full_len = strlen(multi);
    for len in 1..=full_len {
        assert!(bidi_process_check_overflow(&s, multi, len, BaseDir::Rtl));
    }
}

/// Test 5: Hebrew text (also 2-byte UTF-8) with RTL.
#[test]
fn bidi_rtl_hebrew_odd_len_no_overflow() {
    let s = BidiState::new();
    let hebrew = b"\xd7\xa9\xd7\x9c\xd7\x95\xd7\x9d";

    assert!(bidi_process_check_overflow(&s, hebrew, 7, BaseDir::Rtl));
    assert!(bidi_process_check_overflow(&s, hebrew, 5, BaseDir::Rtl));
    assert!(bidi_process_check_overflow(&s, hebrew, 3, BaseDir::Rtl));
    assert!(bidi_process_check_overflow(&s, hebrew, 1, BaseDir::Rtl));
}

/// Test 6: 3-byte UTF-8 characters (Chinese) with RTL base direction.
#[test]
fn bidi_rtl_chinese_non_aligned_len_no_overflow() {
    let s = BidiState::new();
    /* Chinese: U+4F60 U+597D (6 bytes) */
    let chinese = b"\xe4\xbd\xa0\xe5\xa5\xbd";

    assert!(bidi_process_check_overflow(&s, chinese, 5, BaseDir::Rtl));
    assert!(bidi_process_check_overflow(&s, chinese, 4, BaseDir::Rtl));
    assert!(bidi_process_check_overflow(&s, chinese, 2, BaseDir::Rtl));
    assert!(bidi_process_check_overflow(&s, chinese, 1, BaseDir::Rtl));
}

/// Test 7: digits (weak chars) mixed with Arabic (strong RTL).
#[test]
fn bidi_rtl_digits_arabic_no_overflow() {
    let s = BidiState::new();
    /* "123" + Arabic */
    let mixed = b"123\xd8\xb9\xd8\xb1";

    assert!(bidi_process_check_overflow(&s, mixed, 6, BaseDir::Rtl));
    assert!(bidi_process_check_overflow(&s, mixed, 5, BaseDir::Rtl));
    assert!(bidi_process_check_overflow(&s, mixed, 4, BaseDir::Rtl));
}

/// Test 8: exact character boundary len should produce valid output.
#[test]
fn bidi_rtl_exact_boundary_correct_output() {
    let s = BidiState::new();
    /* Two Arabic chars: U+0639 U+0631 (4 bytes) */
    let arabic = b"\xd8\xb9\xd8\xb1";
    let mut out = [0u8; 8];

    s.process_paragraph(arabic, Some(&mut out), 4, BaseDir::Rtl, None, 0);

    /* RTL reversal: chars should be swapped. U+0631 first, then U+0639 */
    assert_eq!(0xd8, out[0]);
    assert_eq!(0xb1, out[1]);
    assert_eq!(0xd8, out[2]);
    assert_eq!(0xb9, out[3]);
    assert_eq!(0x00, out[4]);
}

/// Test 9: brute-force sweep of every len of a realistic Arabic string.
#[test]
fn bidi_rtl_bruteforce_sweep_no_overflow() {
    let s = BidiState::new();
    /* Longer Arabic string simulating real HRV page text */
    let text: &[u8] = b"\xd8\xb9\xd8\xb1\xd8\xa8\xd9\x8a \xd8\xaa\xd8\xac\xd8\xb1\xd8\xa8\xd8\xa9 HRV 123";
    let full_len = strlen(text);

    for len in 1..=full_len {
        assert!(
            bidi_process_check_overflow(&s, text, len, BaseDir::Rtl),
            "Overflow detected in brute-force sweep"
        );
    }
}

/// Test 10: `NULL` `str_out` (direction detection only, no write).
#[test]
fn bidi_rtl_null_output_no_crash() {
    let s = BidiState::new();
    let arabic = b"\xd8\xb9\xd8\xb1\xd8\xa8";

    /* Should not crash — str_out is NULL */
    s.process_paragraph(arabic, None, 5, BaseDir::Rtl, None, 0);
    s.process_paragraph(arabic, None, 3, BaseDir::Rtl, None, 0);
    s.process_paragraph(arabic, None, 1, BaseDir::Rtl, None, 0);
}

/// Test 11: `lv_bidi_process()`: multi-paragraph processing.
#[test]
fn bidi_process_multi_paragraph() {
    let s = BidiState::new();
    /* Two paragraphs separated by newline, with Arabic text */
    let input = b"Hello\n\xd8\xb9\xd8\xb1\xd8\xa8";
    let mut out = [0u8; 32];

    s.process(input, &mut out, BaseDir::Ltr);

    /* First paragraph "Hello" should remain LTR */
    assert_eq!(b'H', out[0]);
    assert_eq!(b'e', out[1]);
    assert_eq!(b'l', out[2]);
    assert_eq!(b'l', out[3]);
    assert_eq!(b'o', out[4]);
    /* Newline preserved */
    assert_eq!(b'\n', out[5]);
    /* Null terminator at end */
    assert_eq!(b'\0', out[12]);
}

/// Test 12: `lv_bidi_process()` with leading newlines.
#[test]
fn bidi_process_leading_newlines() {
    let s = BidiState::new();
    let input = b"\n\rHello";
    let mut out = [0u8; 16];

    s.process(input, &mut out, BaseDir::Ltr);

    assert_eq!(b'\n', out[0]);
    assert_eq!(b'\r', out[1]);
    assert_eq!(b'H', out[2]);
}

/// Test 13: `lv_bidi_process()` with AUTO base direction.
#[test]
fn bidi_process_auto_dir() {
    let s = BidiState::new();
    /* Arabic first -> auto-detect RTL */
    let input = b"\xd8\xb9\xd8\xb1\xd8\xa8";
    let mut out = [0u8; 16];

    s.process(input, &mut out, BaseDir::Auto);

    /* Should not crash, output should be non-empty */
    assert_ne!(0, out[0]);
}

/// Test 14: `lv_bidi_process()` with RTL base direction and multiple
/// paragraphs.
#[test]
fn bidi_process_rtl_multi_paragraph() {
    let s = BidiState::new();
    /* 4 bytes, then \n\n, then 4 bytes => 10 bytes total */
    let input = b"\xd8\xb9\xd8\xb1\n\n\xd8\xa8\xd9\x8a";
    let mut out = [0u8; 32];

    s.process(input, &mut out, BaseDir::Rtl);

    /* Newlines should be preserved at byte offsets 4 and 5 */
    assert_eq!(b'\n', out[4]);
    assert_eq!(b'\n', out[5]);
    /* Null terminator */
    let full_len = strlen(input);
    assert_eq!(b'\0', out[full_len as usize]);
}

/// Test 15: `lv_bidi_set_custom_neutrals_static()`: set and use custom
/// neutrals.
#[test]
fn bidi_set_custom_neutrals() {
    let mut s = BidiState::new();
    /* Set custom neutrals to only space and period */
    s.set_custom_neutrals_static(Some(" ."));

    /* Process text with custom neutrals active */
    let input = b"AB . \xd8\xb9\xd8\xb1";
    let mut out = [0u8; 32];

    s.process_paragraph(input, Some(&mut out), strlen(input), BaseDir::Ltr, None, 0);

    /* Should not crash, output should be non-empty */
    assert_ne!(0, out[0]);

    /* tearDown(): lv_bidi_set_custom_neutrals_static(NULL) */
    s.set_custom_neutrals_static(None);
    assert_eq!(s, BidiState::new());
}

/// Test 16: `lv_bidi_get_logical_pos()` with `bidi_txt == NULL`.
#[test]
fn bidi_get_logical_pos_null_bidi_txt() {
    let s = BidiState::new();
    let input = b"\xd8\xb9\xd8\xb1\xd8\xa8"; /* 3 Arabic chars, 6 bytes */
    let len = strlen(input);

    let pos = s.get_logical_pos(input, None, len, BaseDir::Rtl, 0, None);
    /* Should return a valid position (not -1) */
    assert_ne!(u16::MAX, pos);
}

/// Test 17: `lv_bidi_get_logical_pos()` with `is_rtl` output.
#[test]
fn bidi_get_logical_pos_with_is_rtl() {
    let s = BidiState::new();
    let input = b"\xd8\xb9\xd8\xb1"; /* 2 Arabic chars, 4 bytes */
    let len = strlen(input);
    let mut is_rtl = false;
    let mut bidi_txt = Vec::new();

    let pos = s.get_logical_pos(
        input,
        Some(&mut bidi_txt),
        len,
        BaseDir::Rtl,
        0,
        Some(&mut is_rtl),
    );
    assert_ne!(u16::MAX, pos);
    /* Arabic text in RTL context should be RTL */
    assert!(is_rtl);
    assert_eq!(bidi_txt, b"\xd8\xb1\xd8\xb9\0");
}

/// Test 18: `lv_bidi_get_visual_pos()` with `bidi_txt == NULL`.
#[test]
fn bidi_get_visual_pos_null_bidi_txt() {
    let s = BidiState::new();
    let input = b"\xd8\xb9\xd8\xb1\xd8\xa8"; /* 3 Arabic chars */
    let len = strlen(input) as u16;

    let pos = s.get_visual_pos(input, None, len, BaseDir::Rtl, 0, None);
    assert_ne!(u16::MAX, pos);
}

/// Test 19: `lv_bidi_get_visual_pos()` with `is_rtl` output.
#[test]
fn bidi_get_visual_pos_with_is_rtl() {
    let s = BidiState::new();
    let input = b"\xd8\xb9\xd8\xb1"; /* 2 Arabic chars */
    let len = strlen(input) as u16;
    let mut is_rtl = false;
    let mut bidi_txt = Vec::new();

    let pos = s.get_visual_pos(
        input,
        Some(&mut bidi_txt),
        len,
        BaseDir::Rtl,
        0,
        Some(&mut is_rtl),
    );
    assert_ne!(u16::MAX, pos);
    assert!(is_rtl);
}

/// Test 20: `lv_bidi_get_visual_pos()`: `logical_pos` not found.
#[test]
fn bidi_get_visual_pos_not_found() {
    let s = BidiState::new();
    let input = b"AB"; /* 2 LTR chars */
    let len = strlen(input) as u16;

    /* logical_pos = 999 doesn't exist */
    let pos = s.get_visual_pos(input, None, len, BaseDir::Ltr, 999, None);
    assert_eq!(u16::MAX, pos);
}

/// Test 21: `lv_bidi_calculate_align()` with AUTO base_dir and RTL text.
#[test]
fn bidi_calculate_align_auto_rtl() {
    let s = BidiState::new();
    let mut align = TextAlign::Auto;
    let mut dir = BaseDir::Auto;
    let arabic = b"\xd8\xb9\xd8\xb1\xd8\xa8";

    s.calculate_align(&mut align, &mut dir, arabic);

    assert_eq!(BaseDir::Rtl, dir);
    assert_eq!(TextAlign::Right, align);
}

/// Test 22: `lv_bidi_calculate_align()` with explicit LTR dir and AUTO align.
#[test]
fn bidi_calculate_align_ltr() {
    let s = BidiState::new();
    let mut align = TextAlign::Auto;
    let mut dir = BaseDir::Ltr;

    s.calculate_align(&mut align, &mut dir, b"Hello");

    assert_eq!(BaseDir::Ltr, dir);
    assert_eq!(TextAlign::Left, align);
}

/// Test 23: `lv_bidi_process_paragraph()` with AUTO base_dir.
#[test]
fn bidi_process_paragraph_auto_dir() {
    let s = BidiState::new();
    let input = b"\xd8\xb9\xd8\xb1 Hello";
    let mut out = [0u8; 32];

    s.process_paragraph(input, Some(&mut out), strlen(input), BaseDir::Auto, None, 0);
    /* Auto-detect should pick RTL from the first strong Arabic char */
    assert_ne!(0, out[0]);
}

/// Test 24: LTR text with neutral prefix and `NULL` `str_out`.
#[test]
fn bidi_process_paragraph_ltr_null_str_out() {
    let s = BidiState::new();
    /* Space (neutral) + LTR text */
    let input = b" Hello";
    let len = strlen(input);
    let pos_conv_len: u16 = 6; /* 6 chars */
    let mut pos_conv = [0u16; 8];

    /* str_out is NULL but pos_conv_out is provided */
    s.process_paragraph(input, None, len, BaseDir::Ltr, Some(&mut pos_conv), pos_conv_len);
    /* Should not crash */
}

/// Test 25: RTL base_dir with neutral prefix and non-`NULL` `str_out`.
#[test]
fn bidi_process_paragraph_rtl_neutral_prefix_with_output() {
    let s = BidiState::new();
    /* Space (neutral) + Arabic */
    let input = b" \xd8\xb9\xd8\xb1";
    let len = strlen(input);
    let mut out = [0u8; 16];
    let pos_conv_len: u16 = 3; /* space + 2 Arabic chars */
    let mut pos_conv = [0u16; 8];

    s.process_paragraph(
        input,
        Some(&mut out),
        len,
        BaseDir::Rtl,
        Some(&mut pos_conv),
        pos_conv_len,
    );
    assert_ne!(0, out[0]);
}

/// Test 26: RTL base_dir with an embedded LTR run.
#[test]
fn bidi_process_paragraph_rtl_with_ltr_run() {
    let s = BidiState::new();
    /* Arabic + LTR word + Arabic */
    let input = b"\xd8\xb9 Hello \xd8\xb1";
    let len = strlen(input);
    let mut out = [0u8; 32];
    let pos_conv_len: u16 = 9; /* 1 + 1 + 5 + 1 + 1 */
    let mut pos_conv = [0u16; 16];

    s.process_paragraph(
        input,
        Some(&mut out),
        len,
        BaseDir::Rtl,
        Some(&mut pos_conv),
        pos_conv_len,
    );
    assert_ne!(0, out[0]);
}

/// Test 27: `lv_bidi_process()` with `\r` in trailing newlines.
#[test]
fn bidi_process_carriage_return() {
    let s = BidiState::new();
    let input = b"Hello\r\nWorld";
    let mut out = [0u8; 32];

    s.process(input, &mut out, BaseDir::Ltr);

    assert_eq!(b'H', out[0]);
    assert_eq!(b'\r', out[5]);
    assert_eq!(b'\n', out[6]);
    assert_eq!(b'W', out[7]);
}

/// Test 28: Arabic and Hebrew presentation forms are RTL.
#[test]
fn bidi_arabic_presentation_forms() {
    let s = BidiState::new();
    /* U+FB50 (Arabic Presentation Forms-A) */
    let pres_a = b"\xef\xad\x90\xef\xad\x91";
    let mut out_a = [0u8; 16];
    s.process_paragraph(pres_a, Some(&mut out_a), strlen(pres_a), BaseDir::Rtl, None, 0);
    assert_ne!(0, out_a[0]);

    /* U+FE70 (Arabic Presentation Forms-B) */
    let pres_b = b"\xef\xb9\xb0\xef\xb9\xb1";
    let mut out_b = [0u8; 16];
    s.process_paragraph(pres_b, Some(&mut out_b), strlen(pres_b), BaseDir::Rtl, None, 0);
    assert_ne!(0, out_b[0]);

    /* U+FB1D (Hebrew Presentation Forms) */
    let heb_pres = b"\xef\xac\x9d\xef\xac\x9e";
    let mut out_h = [0u8; 16];
    s.process_paragraph(
        heb_pres,
        Some(&mut out_h),
        strlen(heb_pres),
        BaseDir::Rtl,
        None,
        0,
    );
    assert_ne!(0, out_h[0]);
}

/// Test 29: RTL text with digits: weak chars in RTL context.
#[test]
fn bidi_rtl_weak_digits_in_rtl_context() {
    let s = BidiState::new();
    /* Arabic + digits + Arabic: triggers weak handling in RTL run */
    let input = b"\xd8\xb9123\xd8\xb1";
    let len = strlen(input);
    let mut out = [0u8; 32];
    let mut pos_conv = [0u16; 16];
    let pos_conv_len: u16 = 5; /* 1 Arabic + 3 digits + 1 Arabic */

    s.process_paragraph(
        input,
        Some(&mut out),
        len,
        BaseDir::Rtl,
        Some(&mut pos_conv),
        pos_conv_len,
    );
    assert_ne!(0, out[0]);
}

/// Test 30: RTL reverse with `NULL` dest but non-`NULL` `pos_conv`.
#[test]
fn bidi_rtl_reverse_null_dest_with_pos_conv() {
    let s = BidiState::new();
    let input = b"\xd8\xb9\xd8\xb1";
    let len = strlen(input);
    let pos_conv_len: u16 = 2;
    let mut pos_conv = [0u16; 4];

    /* str_out=NULL, pos_conv_out=non-NULL */
    s.process_paragraph(input, None, len, BaseDir::Ltr, Some(&mut pos_conv), pos_conv_len);
    /* Should not crash, pos_conv should be filled */
}

/// Test 31: brackets with no strong char inside.
#[test]
fn bidi_brackets_no_strong_inside() {
    let s = BidiState::new();
    /* Arabic + bracket with only neutrals inside + Arabic */
    let input = b"\xd8\xb9( . )\xd8\xb1";
    let len = strlen(input);
    let mut out = [0u8; 32];

    s.process_paragraph(input, Some(&mut out), len, BaseDir::Rtl, None, 0);
    assert_ne!(0, out[0]);
}

/// Test 32: brackets at the very start of text.
#[test]
fn bidi_brackets_at_start() {
    let s = BidiState::new();
    /* Opening bracket at start with neutrals inside, then Arabic */
    let input = b"( . )\xd8\xb9\xd8\xb1";
    let len = strlen(input);
    let mut out = [0u8; 32];

    s.process_paragraph(input, Some(&mut out), len, BaseDir::Rtl, None, 0);
    assert_ne!(0, out[0]);
}

/// Test 33: bracket with no matching closing bracket.
#[test]
fn bidi_bracket_no_closing() {
    let s = BidiState::new();
    /* Opening bracket with no closing */
    let input = b"\xd8\xb9(\xd8\xb1";
    let len = strlen(input);
    let mut out = [0u8; 16];

    s.process_paragraph(input, Some(&mut out), len, BaseDir::Rtl, None, 0);
    assert_ne!(0, out[0]);
}

/// Test 34: deeply nested brackets overflowing the bracket stack.
#[test]
fn bidi_bracket_stack_overflow() {
    let s = BidiState::new();
    /* 5 nested opening brackets with Arabic inside each — exceeds LV_BIDI_BRACKET_DEPTH (4) */
    let input = b"([\xd8\xb9{<\xd8\xb1(\xd8\xa8";
    let len = strlen(input);
    let mut out = [0u8; 32];

    s.process_paragraph(input, Some(&mut out), len, BaseDir::Ltr, None, 0);
    /* Should not crash even with stack overflow */
    assert_ne!(0, out[0]);
}

/// Test 35: U+202E (RLO) character.
#[test]
fn bidi_rlo_character() {
    let s = BidiState::new();
    /* U+202E (RLO) + LTR text */
    let input = b"\xe2\x80\xaeHello";
    let len = strlen(input);
    let mut out = [0u8; 16];

    s.process_paragraph(input, Some(&mut out), len, BaseDir::Ltr, None, 0);
    assert_ne!(0, out[0]);
}

/// Test 36: an RTL run followed by trailing neutrals in an LTR paragraph.
#[test]
fn bidi_ltr_base_rtl_run_trailing_neutrals() {
    let s = BidiState::new();
    /* Arabic + trailing spaces */
    let input = b"\xd8\xb9\xd8\xb1  ";
    let len = strlen(input);
    let mut out = [0u8; 16];

    s.process_paragraph(input, Some(&mut out), len, BaseDir::Ltr, None, 0);
    assert_ne!(0, out[0]);
}

/// Test 37: direction change with `run_dir != base_dir`.
#[test]
fn bidi_dir_change_exclude_neutrals() {
    let s = BidiState::new();
    /* Arabic + space + LTR text, base_dir=LTR */
    let input = b"\xd8\xb9\xd8\xb1 Hello";
    let len = strlen(input);
    let mut out = [0u8; 32];

    s.process_paragraph(input, Some(&mut out), len, BaseDir::Ltr, None, 0);
    assert_ne!(0, out[0]);
}

/// Test 38: a partial character right before the string terminator is
/// dropped.
#[test]
fn bidi_trailing_partial_char_dropped() {
    let s = BidiState::new();
    /* Lone UTF-8 lead byte of a 2 byte character */
    let partial = b"\xc3";
    let mut out = [0x55u8; 8];

    s.process_paragraph(partial, Some(&mut out), 1, BaseDir::Ltr, None, 0);
    assert_eq!(0x00, out[0]);

    out.fill(0x55);
    s.process_paragraph(partial, Some(&mut out), 1, BaseDir::Rtl, None, 0);
    assert_eq!(0x00, out[0]);
}

/// Test 39: a partial character at the end must not corrupt the valid text
/// before it.
#[test]
fn bidi_trailing_partial_char_keeps_valid_text() {
    let s = BidiState::new();
    /* Hebrew alef (U+05D0) followed by a lone lead byte */
    let input = b"\xd7\x90\xc3";
    let mut out = [0x55u8; 8];

    s.process_paragraph(input, Some(&mut out), 3, BaseDir::Rtl, None, 0);
    assert_eq!(0xd7, out[0]);
    assert_eq!(0x90, out[1]);
    assert_eq!(0x00, out[2]);

    out.fill(0x55);
    s.process_paragraph(input, Some(&mut out), 3, BaseDir::Ltr, None, 0);
    assert_eq!(0xd7, out[0]);
    assert_eq!(0x90, out[1]);
    assert_eq!(0x00, out[2]);
}

/// Test 40: the byte and character counts stay consistent when the text
/// ends with a partial character.
#[test]
fn bidi_trailing_partial_char_pos_conv() {
    let s = BidiState::new();
    /* Hebrew alef and bet followed by a lone lead byte */
    let input = b"\xd7\x90\xd7\x91\xc3";
    let len = strlen(input) as u16;

    /* Only the two Hebrew characters are mapped, and RTL reverses them */
    assert_eq!(1, s.get_visual_pos(input, None, len, BaseDir::Rtl, 0, None));
    assert_eq!(0, s.get_visual_pos(input, None, len, BaseDir::Rtl, 1, None));
    assert_eq!(
        1,
        s.get_logical_pos(input, None, u32::from(len), BaseDir::Rtl, 0, None)
    );
    assert_eq!(
        0,
        s.get_logical_pos(input, None, u32::from(len), BaseDir::Rtl, 1, None)
    );
}

/*
 * test_text_ap.c (`tearDown()` frees `output`: the `String` drop)
 */

/// An empty string should not crash and return 0.
#[test]
fn ap_strlen_empty_string_returns_zero() {
    assert_eq!(0, ap_strlen(""));
}

/// Pure ASCII has no Arabic/Persian chars: the result equals the byte count.
#[test]
fn ap_strlen_ascii_only() {
    let txt = "Hello World";
    let result = ap_strlen(txt);
    assert_eq!(txt.len(), result);
}

/// U+0622 ARABIC LETTER ALEF WITH MADDA ABOVE (`LV_AP_ALPHABET_BASE_CODE`).
#[test]
fn ap_strlen_single_arabic_char() {
    let txt = "\u{0622}";
    let result = ap_strlen(txt);
    assert_eq!(3, result);
}

/// Simple Arabic sentence: "مرحبا" (Marhaba / Hello).
#[test]
fn ap_strlen_arabic_sentence() {
    let txt = "\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}";
    let result = ap_strlen(txt);
    assert_eq!(15, result);
}

/// Mixed text: "abc مرحبا xyz".
#[test]
fn ap_strlen_mixed_arabic_latin() {
    let txt = "abc \u{0645}\u{0631}\u{062D}\u{0628}\u{0627} xyz";
    let result = ap_strlen(txt);
    assert_eq!(23, result);
}

/// Invalid UTF-8 bytes count as one byte each.
#[test]
fn ap_strlen_treats_invalid_utf8_as_raw_bytes() {
    let txt = b"\xFF\xFE"; /* Invalid UTF-8 */
    let result = ap_strlen_bytes(txt);
    assert_eq!(2, result);
}

/// `lv_text_ap_proc(input, NULL)` returns without writing; the Rust API
/// returns its result instead of writing a caller buffer, so there is no
/// `NULL` output to pass. Processing must still not crash.
#[test]
fn ap_proc_null_output() {
    let input = "Hello";
    let _ = ap_proc(input);
    /* Should get here without crashing */
}

#[test]
fn ap_proc_empty_string() {
    let input = "";
    let s_output = ap_proc(input);
    assert_eq!("", s_output);
}

#[test]
fn ap_proc_ascii_passthrough() {
    let input = "Hello";
    let s_output = ap_proc(input);
    assert_eq!(input, s_output);
}

/// "مرحبا" becomes its contextual presentation forms.
#[test]
fn ap_proc_output_not_null_for_arabic() {
    let input = "\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}";
    let output = ap_proc(input);
    assert_eq!(
        output.as_bytes(),
        b"\xEF\xBB\xA3\xEF\xBA\xAE\xEF\xBA\xA3\xEF\xBA\x92\xEF\xBA\x8E"
    );
}

/// The byte count reported by calc must match what proc actually writes.
#[test]
fn ap_proc_output_length_matches_calc() {
    let input = "\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}";
    let expected_len = ap_strlen(input);
    let output = ap_proc(input);
    assert_eq!(expected_len, output.len());
}

/// Running proc twice on ASCII must yield the same result.
#[test]
fn ap_proc_idempotent_ascii() {
    let input = "Test 123";
    let out1 = ap_proc(input);
    let out2 = ap_proc(input);
    assert_eq!(out1, out2);
}

/// Stress: 10 repetitions of "مرحبا". The C test checks a sentinel after
/// the `lv_text_ap_strlen() + 1` byte buffer; here the result must fit it.
#[test]
fn ap_proc_does_not_overflow_with_long_arabic() {
    let word = "\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}";
    let input = word.repeat(10);

    let len = ap_strlen(&input);
    let output = ap_proc(&input); /* Must not crash / overwrite sentinel */
    assert_ne!(0, output.as_bytes()[0]);
    assert_eq!(len, output.len());
}

/*
 * Screenshot tests
 */

/// The white 800x480 container that `test_checkbox_rtl` and
/// `test_label_rtl_dot_long_mode` create on the active screen.
fn white_screen(ui: &mut Ui, active_screen: Obj) -> Obj {
    let screen = ui.obj_create(Some(active_screen));
    ui.obj_remove_style_all(screen);
    ui.obj_set_size(screen, 800, 480);
    ui.obj_center(screen);
    ui.obj_set_style_bg_color(screen, Color::white(), MAIN);
    ui.obj_set_style_bg_opa(screen, OPA_100, MAIN);
    ui.obj_set_style_pad_all(screen, 0, MAIN);
    screen
}

/// `test_checkbox_rtl` (`test_checkbox.c`). Earlier tests of that file only
/// create checkboxes on the active screen (cleaned by `tearDown()`) and set
/// `active_screen = lv_screen_active()`; none of them changes the screen's
/// styles, so a fresh default-theme display is the inherited state.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn checkbox_rtl() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let active_screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let message = "מעבד, או בשמו המלא יחידת עיבוד מרכזית (באנגלית: CPU - Central Processing Unit).";

    let _screen = white_screen(ui, active_screen);

    let test_checkbox = Checkbox::create(ui, active_screen);

    test_checkbox.set_text(ui, message);
    ui.obj_set_style_text_font(test_checkbox.obj(), dejavu(), MAIN);
    ui.obj_center(test_checkbox.obj());
    ui.obj_set_style_base_dir(test_checkbox.obj(), BaseDir::Rtl, MAIN);

    display.screenshot_compare("widgets/checkbox_rtl_1.png").unwrap();
}

/// `test_label_rtl_dot_long_mode` (`test_label.c`). `setUp()` creates four
/// labels on the active screen before every test (the white container
/// covers them); the earlier tests only restyle those labels, which
/// `tearDown()` deletes, so the screen itself is not changed.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn label_rtl_dot_long_mode() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let active_screen = display.screen().unwrap();
    let ui = &mut display.ui;

    /* setUp() */
    let _label = Label::create(ui, active_screen);
    let long_label = Label::create(ui, active_screen);
    let long_label_multiline = Label::create(ui, active_screen);
    let empty_label = Label::create(ui, active_screen);
    long_label.set_text(
        ui,
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Cras malesuada ultrices magna in rutrum.",
    );
    long_label_multiline.set_text(
        ui,
        "Lorem ipsum dolor sit amet,\nconsectetur adipiscing elit.\nCras malesuada ultrices magna in rutrum.\n",
    );
    empty_label.set_text(ui, "");

    let message = "מעבד, או בשמו המלא יחידת עיבוד מרכזית (באנגלית: CPU - Central Processing Unit).";

    let screen = white_screen(ui, active_screen);

    let test_label = Label::create(ui, screen);
    ui.obj_set_style_text_font(test_label.obj(), dejavu(), MAIN);
    test_label.set_long_mode(ui, LabelLongMode::Dots);
    ui.obj_set_style_base_dir(test_label.obj(), BaseDir::Rtl, MAIN);
    ui.obj_set_size(
        test_label.obj(),
        300,
        ferrovgl_fonts::DEJAVU_16_PERSIAN_HEBREW.line_height(),
    );
    test_label.set_text(ui, message);
    ui.obj_center(test_label.obj());

    display
        .screenshot_compare("widgets/label_rtl_dot_long_mode.png")
        .unwrap();
}
