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
//! first NUL or at the end of the slice, like the other `misc/text.rs` ports:
//! reading past the end of a slice yields `0`, exactly where C reads the
//! terminator. Output buffers (`char *` / `uint16_t *`) are mutable slices;
//! upstream writes past their end are undefined behaviour in C and panic here.

use alloc::vec;
use alloc::vec::Vec;

pub use crate::misc::text::BaseDir;
use crate::misc::text::{self, TextAlign};

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

/// `bracket_left[]` (static in `lv_bidi.c`), NUL terminated.
const BRACKET_LEFT: &[u8; 5] = b"<({[\0";
/// `bracket_right[]` (static in `lv_bidi.c`), NUL terminated.
const BRACKET_RIGHT: &[u8; 5] = b">)}]\0";

/// The built-in neutral characters of `lv_bidi_letter_is_neutral()`.
const DEFAULT_NEUTRALS: &str = " \t\n\r.,:;'\"`!?%/\\-=()[]{}<>@#&$|";

/// `weaks[]` of `lv_bidi_letter_is_weak()`.
const WEAKS: &[u8] = b"0123456789";

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

/// `lv_memcpy(dest, &src[ofs], n)`: bytes past the end of `src` read as the
/// C terminator (0).
#[inline]
fn copy_bytes(dest: &mut [u8], src: &[u8], ofs: u32, n: u32) {
    for (k, d) in dest[..n as usize].iter_mut().enumerate() {
        *d = byte_at(src, ofs.wrapping_add(k as u32));
    }
}

/// `lv_text_encoded_next(txt, &i)`
#[inline]
fn next(txt: &[u8], i: &mut u32) -> u32 {
    text::encoded_next(txt, Some(i))
}

/// `lv_text_encoded_prev(txt, &i)`
#[inline]
fn prev(txt: &[u8], i: &mut u32) -> u32 {
    text::encoded_prev(txt, Some(i))
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
        self.custom_neutrals = neutrals;
    }

    /// `lv_bidi_calculate_align()` (`lv_bidi_calculate_align_internal()`
    /// with `LV_USE_BIDI`): resolve `LV_BASE_DIR_AUTO` from `txt` and
    /// `LV_TEXT_ALIGN_AUTO` from the base direction.
    pub fn calculate_align(&self, align: &mut TextAlign, base_dir: &mut BaseDir, txt: &[u8]) {
        if *base_dir == BaseDir::Auto {
            *base_dir = self.detect_base_dir(txt);
        }

        if *align == TextAlign::Auto {
            if *base_dir == BaseDir::Rtl {
                *align = TextAlign::Right;
            } else {
                *align = TextAlign::Left;
            }
        }
    }

    /// `lv_bidi_process()`: reorder every paragraph of `str_in` into visual
    /// order. `str_out` must hold `strlen(str_in) + 1` bytes.
    pub fn process(&self, str_in: &[u8], str_out: &mut [u8], mut base_dir: BaseDir) {
        if base_dir == BaseDir::Auto {
            base_dir = self.detect_base_dir(str_in);
        }

        let mut par_start: u32 = 0;
        let mut par_len: u32;

        while byte_at(str_in, par_start) == b'\n' || byte_at(str_in, par_start) == b'\r' {
            str_out[par_start as usize] = byte_at(str_in, par_start);
            par_start += 1;
        }

        while byte_at(str_in, par_start) != 0 {
            par_len = Self::get_next_paragraph(tail(str_in, par_start));
            self.process_paragraph(
                tail(str_in, par_start),
                Some(&mut str_out[par_start as usize..]),
                par_len,
                base_dir,
                None,
                0,
            );
            par_start += par_len;

            while byte_at(str_in, par_start) == b'\n' || byte_at(str_in, par_start) == b'\r' {
                str_out[par_start as usize] = byte_at(str_in, par_start);
                par_start += 1;
            }
        }

        str_out[par_start as usize] = 0;
    }

    /// `lv_bidi_detect_base_dir()`: `Rtl` if any strong RTL character is
    /// found, else `Ltr` if any strong LTR one is, else
    /// [`BIDI_BASE_DIR_DEF`] (`Ltr` for `Auto`).
    pub fn detect_base_dir(&self, txt: &[u8]) -> BaseDir {
        let mut i: u32 = 0;
        let mut letter: u32;
        let mut found_ltr = false;
        while byte_at(txt, i) != 0 {
            letter = next(txt, &mut i);

            let dir = self.get_letter_dir(letter);
            if dir == BaseDir::Rtl {
                return BaseDir::Rtl;
            }
            if dir == BaseDir::Ltr {
                found_ltr = true;
            }
        }

        if found_ltr {
            return BaseDir::Ltr;
        }

        /*If there were no strong char earlier return with the default base dir*/
        if BIDI_BASE_DIR_DEF == BaseDir::Auto {
            BaseDir::Ltr
        } else {
            BIDI_BASE_DIR_DEF
        }
    }

    /// `lv_bidi_get_logical_pos()`: the logical position of the character at
    /// `visual_pos` of a line of `len` bytes. `bidi_txt` receives the
    /// processed line (C: an `lv_malloc`ed buffer of `len + 1` bytes) and
    /// `is_rtl` the direction of the character's context. Returns
    /// `(uint16_t) -1` on allocation failure (not reachable here).
    ///
    /// A `visual_pos` past the converted characters reads outside the C
    /// buffer; here such a read gives position 0 (LTR).
    pub fn get_logical_pos(
        &self,
        str_in: &[u8],
        bidi_txt: Option<&mut Vec<u8>>,
        len: u32,
        base_dir: BaseDir,
        visual_pos: u32,
        is_rtl: Option<&mut bool>,
    ) -> u16 {
        let pos_conv_len = Self::get_txt_len(str_in, len, None);
        let mut buf: Vec<u8> = vec![0; len as usize + 1];

        let mut pos_conv_buf: Vec<u16> = vec![0; pos_conv_len as usize];

        let has_bidi_txt = bidi_txt.is_some();
        self.process_paragraph(
            str_in,
            if has_bidi_txt { Some(&mut buf) } else { None },
            len,
            base_dir,
            Some(&mut pos_conv_buf),
            pos_conv_len as u16,
        );

        let conv = pos_conv_buf.get(visual_pos as usize).copied().unwrap_or(0);
        if let Some(is_rtl) = is_rtl {
            *is_rtl = is_rtl_pos(conv);
        }

        /*`if(bidi_txt) *bidi_txt = buf;`, else `lv_free(buf)`*/
        if let Some(bidi_txt) = bidi_txt {
            *bidi_txt = buf;
        }
        get_pos(conv)
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
        let pos_conv_len = Self::get_txt_len(str_in, u32::from(len), None);
        let mut buf: Vec<u8> = vec![0; usize::from(len) + 1];

        let mut pos_conv_buf: Vec<u16> = vec![0; pos_conv_len as usize];

        let has_bidi_txt = bidi_txt.is_some();
        self.process_paragraph(
            str_in,
            if has_bidi_txt { Some(&mut buf) } else { None },
            u32::from(len),
            base_dir,
            Some(&mut pos_conv_buf),
            pos_conv_len as u16,
        );

        if let Some(bidi_txt) = bidi_txt {
            *bidi_txt = buf;
        }

        let mut i: u16 = 0;
        while u32::from(i) < pos_conv_len {
            if u32::from(get_pos(pos_conv_buf[usize::from(i)])) == logical_pos {
                if let Some(is_rtl) = is_rtl {
                    *is_rtl = is_rtl_pos(pos_conv_buf[usize::from(i)]);
                }
                return i;
            }
            i = i.wrapping_add(1);
        }
        u16::MAX
    }

    /// `lv_bidi_process_paragraph()`: reorder `len` bytes of one paragraph.
    /// `str_out` (`NULL` = `None`) receives the visual order, `pos_conv_out`
    /// the logical position of each character (`pos_conv_len` elements).
    pub fn process_paragraph(
        &self,
        str_in: &[u8],
        mut str_out: Option<&mut [u8]>,
        mut len: u32,
        mut base_dir: BaseDir,
        mut pos_conv_out: Option<&mut [u16]>,
        pos_conv_len: u16,
    ) {
        let mut run_len: u32 = 0;
        let mut run_dir: BaseDir;
        let mut rd: u32 = 0;
        let mut wr: u32;
        let mut pos_conv_run_len: u16 = 0;
        let mut pos_conv_rd: u16 = 0;
        let mut pos_conv_wr: u16;

        /*A trailing partial or invalid character can't be processed and would make the byte
         *and character counts inconsistent, so drop its bytes and zero them in the output*/
        let mut aligned_len: u32 = 0;
        Self::get_txt_len(str_in, len, Some(&mut aligned_len));
        if let Some(out) = str_out.as_deref_mut() {
            out[aligned_len as usize..=len as usize].fill(0);
        }
        len = aligned_len;

        if base_dir == BaseDir::Auto {
            base_dir = self.detect_base_dir(str_in);
        }
        if base_dir == BaseDir::Rtl {
            wr = len;
            pos_conv_wr = pos_conv_len;
        } else {
            wr = 0;
            pos_conv_wr = 0;
        }

        let mut dir: BaseDir;

        /*Empty the bracket stack*/
        let mut ctx = BidiCtx::default();

        /*Process neutral chars in the beginning*/
        while rd < len {
            let letter = next(str_in, &mut rd);
            pos_conv_rd = pos_conv_rd.wrapping_add(1);
            dir = self.get_letter_dir(letter);
            if dir == BaseDir::Neutral {
                self.bracket_process(&mut ctx, str_in, rd, len, letter, base_dir);
            } else if dir != BaseDir::Weak {
                break;
            }
        }

        if rd != 0 && byte_at(str_in, rd) != 0 && rd < len {
            prev(str_in, &mut rd);
            pos_conv_rd = pos_conv_rd.wrapping_sub(1);
        }

        if rd != 0 {
            if base_dir == BaseDir::Ltr {
                if let Some(out) = str_out.as_deref_mut() {
                    copy_bytes(&mut out[wr as usize..], str_in, 0, rd);
                    wr = wr.wrapping_add(rd);
                }
                if let Some(pc) = pos_conv_out.as_deref_mut() {
                    Self::fill_pos_conv(&mut pc[usize::from(pos_conv_wr)..], pos_conv_rd, 0);
                    pos_conv_wr = pos_conv_wr.wrapping_add(pos_conv_rd);
                }
            } else {
                wr = wr.wrapping_sub(rd);
                pos_conv_wr = pos_conv_wr.wrapping_sub(pos_conv_rd);
                self.rtl_reverse(
                    str_out.as_deref_mut().map(|o| &mut o[wr as usize..]),
                    str_in,
                    rd,
                    pos_conv_out
                        .as_deref_mut()
                        .map(|p| &mut p[usize::from(pos_conv_wr)..]),
                    0,
                    pos_conv_rd,
                );
            }
        }

        /*Get and process the runs*/

        while rd < len && byte_at(str_in, rd) != 0 {
            run_dir = self.get_next_run(
                &mut ctx,
                tail(str_in, rd),
                base_dir,
                len - rd,
                &mut run_len,
                &mut pos_conv_run_len,
            );

            if base_dir == BaseDir::Ltr {
                if run_dir == BaseDir::Ltr {
                    if let Some(out) = str_out.as_deref_mut() {
                        copy_bytes(&mut out[wr as usize..], str_in, rd, run_len);
                    }
                    if let Some(pc) = pos_conv_out.as_deref_mut() {
                        Self::fill_pos_conv(
                            &mut pc[usize::from(pos_conv_wr)..],
                            pos_conv_run_len,
                            pos_conv_rd,
                        );
                    }
                } else {
                    self.rtl_reverse(
                        str_out.as_deref_mut().map(|o| &mut o[wr as usize..]),
                        tail(str_in, rd),
                        run_len,
                        pos_conv_out
                            .as_deref_mut()
                            .map(|p| &mut p[usize::from(pos_conv_wr)..]),
                        pos_conv_rd,
                        pos_conv_run_len,
                    );
                }
                wr = wr.wrapping_add(run_len);
                pos_conv_wr = pos_conv_wr.wrapping_add(pos_conv_run_len);
            } else {
                wr = wr.wrapping_sub(run_len);
                pos_conv_wr = pos_conv_wr.wrapping_sub(pos_conv_run_len);
                if run_dir == BaseDir::Ltr {
                    if let Some(out) = str_out.as_deref_mut() {
                        copy_bytes(&mut out[wr as usize..], str_in, rd, run_len);
                    }
                    if let Some(pc) = pos_conv_out.as_deref_mut() {
                        Self::fill_pos_conv(
                            &mut pc[usize::from(pos_conv_wr)..],
                            pos_conv_run_len,
                            pos_conv_rd,
                        );
                    }
                } else {
                    self.rtl_reverse(
                        str_out.as_deref_mut().map(|o| &mut o[wr as usize..]),
                        tail(str_in, rd),
                        run_len,
                        pos_conv_out
                            .as_deref_mut()
                            .map(|p| &mut p[usize::from(pos_conv_wr)..]),
                        pos_conv_rd,
                        pos_conv_run_len,
                    );
                }
            }

            rd = rd.wrapping_add(run_len);
            pos_conv_rd = pos_conv_rd.wrapping_add(pos_conv_run_len);
        }
    }

    /*
     * Static functions.
     */

    /// `lv_bidi_get_next_paragraph()`: the length of the current paragraph
    /// of `txt` in bytes.
    fn get_next_paragraph(txt: &[u8]) -> u32 {
        let mut i: u32 = 0;

        next(txt, &mut i);

        while byte_at(txt, i) != 0 && byte_at(txt, i) != b'\n' && byte_at(txt, i) != b'\r' {
            next(txt, &mut i);
        }

        i
    }

    /// `lv_bidi_get_letter_dir()`: `Rtl` / `Ltr` / `Weak` / `Neutral`.
    fn get_letter_dir(&self, letter: u32) -> BaseDir {
        if Self::letter_is_rtl(letter) {
            return BaseDir::Rtl;
        }
        if self.letter_is_neutral(letter) {
            return BaseDir::Neutral;
        }
        if Self::letter_is_weak(letter) {
            return BaseDir::Weak;
        }

        BaseDir::Ltr
    }

    /// `lv_bidi_letter_is_weak()`
    fn letter_is_weak(letter: u32) -> bool {
        let mut i: u32 = 0;

        loop {
            let x = next(WEAKS, &mut i);
            if letter == x {
                return true;
            }
            if byte_at(WEAKS, i) == 0 {
                break;
            }
        }

        false
    }

    /// `lv_bidi_letter_is_rtl()`
    fn letter_is_rtl(letter: u32) -> bool {
        if letter == 0x202E {
            return true; /*Unicode of LV_BIDI_RLO*/
        }

        /*Check for Persian and Arabic characters [https://en.wikipedia.org/wiki/Arabic_script_in_Unicode]*/
        if (0x600..=0x6FF).contains(&letter) {
            return true;
        }
        if (0xFB50..=0xFDFF).contains(&letter) {
            return true;
        }
        if (0xFE70..=0xFEFF).contains(&letter) {
            return true;
        }

        /*Check for Hebrew characters [https://en.wikipedia.org/wiki/Unicode_and_HTML_for_the_Hebrew_alphabet]*/
        if (0x590..=0x5FF).contains(&letter) {
            return true;
        }
        if (0xFB1D..=0xFB4F).contains(&letter) {
            return true;
        }

        false
    }

    /// `lv_bidi_letter_is_neutral()`. Like C, every byte of the (custom)
    /// neutrals string is compared as a `(uint32_t)` of a signed `char`.
    fn letter_is_neutral(&self, letter: u32) -> bool {
        let neutrals: &[u8] = match self.custom_neutrals {
            Some(custom) => custom.as_bytes(),
            None => DEFAULT_NEUTRALS.as_bytes(),
        };

        let mut i: u16 = 0;
        while byte_at(neutrals, u32::from(i)) != 0 {
            if letter == byte_at(neutrals, u32::from(i)) as i8 as i32 as u32 {
                return true;
            }
            i = i.wrapping_add(1);
        }

        false
    }

    /// `get_txt_len()`: the number of whole characters in the first
    /// `max_len` bytes; `byte_len` receives their byte count.
    fn get_txt_len(txt: &[u8], max_len: u32, byte_len: Option<&mut u32>) -> u32 {
        let mut len: u32 = 0;
        let mut i: u32 = 0;

        while i < max_len {
            let mut next_i = i;
            if next(txt, &mut next_i) == 0 || next_i > max_len {
                break;
            }
            i = next_i;
            len += 1;
        }

        if let Some(byte_len) = byte_len {
            *byte_len = i;
        }

        len
    }

    /// `fill_pos_conv()`
    fn fill_pos_conv(out: &mut [u16], len: u16, mut index: u16) {
        for o in &mut out[..usize::from(len)] {
            *o = set_rtl_pos(index, false);
            index = index.wrapping_add(1);
        }
    }

    /// `get_next_run()`
    fn get_next_run(
        &self,
        ctx: &mut BidiCtx,
        txt: &[u8],
        base_dir: BaseDir,
        max_len: u32,
        len: &mut u32,
        pos_conv_len: &mut u16,
    ) -> BaseDir {
        let mut i: u32 = 0;
        let mut letter: u32;

        let mut pos_conv_i: u16 = 0;

        letter = text::encoded_next(txt, None);
        let mut dir = self.get_letter_dir(letter);
        if dir == BaseDir::Neutral {
            dir = self.bracket_process(ctx, txt, 0, max_len, letter, base_dir);
        }

        /*Find the first strong char. Skip the neutrals*/
        while dir == BaseDir::Neutral || dir == BaseDir::Weak {
            letter = next(txt, &mut i);

            pos_conv_i = pos_conv_i.wrapping_add(1);
            dir = self.get_letter_dir(letter);
            if dir == BaseDir::Neutral {
                dir = self.bracket_process(ctx, txt, i, max_len, letter, base_dir);
            }

            if dir == BaseDir::Ltr || dir == BaseDir::Rtl {
                break;
            }

            if i >= max_len || byte_at(txt, i) == 0 || byte_at(txt, i) == b'\n' || byte_at(txt, i) == b'\r' {
                *len = i;
                *pos_conv_len = pos_conv_i;
                return base_dir;
            }
        }

        let run_dir = dir;

        let mut i_prev = i;
        let mut i_last_strong = i;
        let mut pos_conv_i_prev = pos_conv_i;
        let mut pos_conv_i_last_strong = pos_conv_i;

        /*Find the next char which has different direction*/
        let mut next_dir: BaseDir;
        while i_prev < max_len && byte_at(txt, i) != 0 && byte_at(txt, i) != b'\n' && byte_at(txt, i) != b'\r'
        {
            letter = next(txt, &mut i);
            pos_conv_i = pos_conv_i.wrapping_add(1);
            next_dir = self.get_letter_dir(letter);
            if next_dir == BaseDir::Neutral {
                next_dir = self.bracket_process(ctx, txt, i, max_len, letter, base_dir);
            }

            if next_dir == BaseDir::Weak && run_dir == BaseDir::Rtl && base_dir == BaseDir::Rtl {
                next_dir = BaseDir::Ltr;
            }

            /*New dir found?*/
            if (next_dir == BaseDir::Rtl || next_dir == BaseDir::Ltr) && next_dir != run_dir {
                /*Include neutrals if `run_dir == base_dir`*/
                if run_dir == base_dir {
                    *len = i_prev;
                    *pos_conv_len = pos_conv_i_prev;
                }
                /*Exclude neutrals if `run_dir != base_dir`*/
                else {
                    *len = i_last_strong;
                    *pos_conv_len = pos_conv_i_last_strong;
                }

                return run_dir;
            }

            if next_dir != BaseDir::Neutral {
                i_last_strong = i;
                pos_conv_i_last_strong = pos_conv_i;
            }

            i_prev = i;
            pos_conv_i_prev = pos_conv_i;
        }

        /*Handle end of of string. Apply `base_dir` on trailing neutrals*/

        /*Include neutrals if `run_dir == base_dir`*/
        if run_dir == base_dir {
            *len = i_prev;
            *pos_conv_len = pos_conv_i_prev;
        }
        /*Exclude neutrals if `run_dir != base_dir`*/
        else {
            *len = i_last_strong;
            *pos_conv_len = pos_conv_i_last_strong;
        }

        run_dir
    }

    /// `rtl_reverse()`
    fn rtl_reverse(
        &self,
        mut dest: Option<&mut [u8]>,
        src: &[u8],
        len: u32,
        mut pos_conv_out: Option<&mut [u16]>,
        pos_conv_rd_base: u16,
        pos_conv_len: u16,
    ) {
        let mut i: u32 = len;
        let mut wr: u32 = 0;
        let mut pos_conv_i: u16 = pos_conv_len;
        let mut pos_conv_wr: u16 = 0;

        while i != 0 {
            let mut letter = prev(src, &mut i);
            pos_conv_i = pos_conv_i.wrapping_sub(1);
            let pos_conv_letter: u16 = pos_conv_i;

            /*Keep weak letters (numbers) as LTR*/
            if Self::letter_is_weak(letter) {
                let last_weak = i;
                let mut first_weak = i;
                let pos_conv_last_weak = pos_conv_i;
                let mut pos_conv_first_weak = pos_conv_i;
                while i != 0 {
                    letter = prev(src, &mut i);
                    /*`pos_conv_letter = --pos_conv_i;`: `pos_conv_letter` is not read on this path*/
                    pos_conv_i = pos_conv_i.wrapping_sub(1);

                    /*No need to call `char_change_to_pair` because there not such chars here*/

                    /*Finish on non-weak char*/
                    /*but treat number and currency related chars as weak*/
                    if !Self::letter_is_weak(letter)
                        && letter != u32::from(b'.')
                        && letter != u32::from(b',')
                        && letter != u32::from(b'$')
                        && letter != u32::from(b'%')
                    {
                        next(src, &mut i); /*Rewind one letter*/
                        pos_conv_i = pos_conv_i.wrapping_add(1);
                        first_weak = i;
                        pos_conv_first_weak = pos_conv_i;
                        break;
                    }
                }
                if i == 0 {
                    first_weak = 0;
                    pos_conv_first_weak = 0;
                }

                let n = last_weak.wrapping_sub(first_weak).wrapping_add(1);
                if let Some(d) = dest.as_deref_mut() {
                    copy_bytes(&mut d[wr as usize..], src, first_weak, n);
                }
                let pos_n = pos_conv_last_weak
                    .wrapping_sub(pos_conv_first_weak)
                    .wrapping_add(1);
                if let Some(pc) = pos_conv_out.as_deref_mut() {
                    Self::fill_pos_conv(
                        &mut pc[usize::from(pos_conv_wr)..],
                        pos_n,
                        pos_conv_rd_base.wrapping_add(pos_conv_first_weak),
                    );
                }
                wr = wr.wrapping_add(n);
                pos_conv_wr = pos_conv_wr.wrapping_add(pos_n);
            }
            /*Simply store in reversed order*/
            else {
                let letter_size = u32::from(text::encoded_size(tail(src, i)));

                if wr + letter_size > len {
                    break;
                }

                /*Swap arithmetical symbols*/
                if letter_size == 1 {
                    letter = Self::char_change_to_pair(letter);
                    let new_letter = letter;
                    if let Some(d) = dest.as_deref_mut() {
                        d[wr as usize] = new_letter as u8;
                    }
                    if let Some(pc) = pos_conv_out.as_deref_mut() {
                        pc[usize::from(pos_conv_wr)] =
                            set_rtl_pos(pos_conv_rd_base.wrapping_add(pos_conv_letter), true);
                    }
                    wr += 1;
                    pos_conv_wr = pos_conv_wr.wrapping_add(1);
                }
                /*Just store the letter*/
                else {
                    if let Some(d) = dest.as_deref_mut() {
                        copy_bytes(&mut d[wr as usize..], src, i, letter_size);
                    }
                    if let Some(pc) = pos_conv_out.as_deref_mut() {
                        pc[usize::from(pos_conv_wr)] =
                            set_rtl_pos(pos_conv_rd_base.wrapping_add(pos_conv_i), true);
                    }
                    wr += letter_size;
                    pos_conv_wr = pos_conv_wr.wrapping_add(1);
                }
            }
        }
    }

    /// `char_change_to_pair()`: mirror a bracket.
    fn char_change_to_pair(letter: u32) -> u32 {
        let mut i: u8 = 0;
        while BRACKET_LEFT[usize::from(i)] != 0 {
            if letter == u32::from(BRACKET_LEFT[usize::from(i)]) {
                return u32::from(BRACKET_RIGHT[usize::from(i)]);
            }
            i += 1;
        }

        i = 0;
        while BRACKET_RIGHT[usize::from(i)] != 0 {
            if letter == u32::from(BRACKET_RIGHT[usize::from(i)]) {
                return u32::from(BRACKET_LEFT[usize::from(i)]);
            }
            i += 1;
        }

        letter
    }

    /// `bracket_process()`: the direction of a bracket (`Neutral` if it is
    /// not a bracket or the direction is not known).
    fn bracket_process(
        &self,
        ctx: &mut BidiCtx,
        txt: &[u8],
        next_pos: u32,
        len: u32,
        letter: u32,
        base_dir: BaseDir,
    ) -> BaseDir {
        let mut bracket_dir = BaseDir::Neutral;

        let mut i: u8 = 0;
        /*Is the letter an opening bracket?*/
        while BRACKET_LEFT[usize::from(i)] != 0 {
            if u32::from(BRACKET_LEFT[usize::from(i)]) == letter {
                /*If so find its matching closing bracket.
                 *If a char with base dir. direction is found then the brackets will have `base_dir` direction*/
                let mut txt_i = next_pos;
                while txt_i < len {
                    let letter_next = next(txt, &mut txt_i);
                    if letter_next == u32::from(BRACKET_RIGHT[usize::from(i)]) {
                        /*Closing bracket found*/
                        break;
                    } else {
                        /*Save the dir*/
                        let letter_dir = self.get_letter_dir(letter_next);
                        if letter_dir == base_dir {
                            bracket_dir = base_dir;
                        }
                    }
                }

                /*There were no matching closing bracket*/
                if txt_i > len {
                    return BaseDir::Neutral;
                }

                /*There where a strong char with base dir in the bracket so the dir is found.*/
                if bracket_dir != BaseDir::Neutral && bracket_dir != BaseDir::Weak {
                    break;
                }

                /*If there were no matching strong chars in the brackets then check the previous chars*/
                txt_i = next_pos;
                if txt_i != 0 {
                    prev(txt, &mut txt_i);
                }
                while txt_i > 0 {
                    let letter_next = prev(txt, &mut txt_i);
                    let letter_dir = self.get_letter_dir(letter_next);
                    if letter_dir == BaseDir::Ltr || letter_dir == BaseDir::Rtl {
                        bracket_dir = letter_dir;
                        break;
                    }
                }

                /*There where a previous strong char which can be used*/
                if bracket_dir != BaseDir::Neutral {
                    break;
                }

                /*There were no strong chars before the bracket, so use the base dir.*/
                if txt_i == 0 {
                    bracket_dir = base_dir;
                }

                break;
            }
            i += 1;
        }

        /*The letter was an opening bracket*/
        if BRACKET_LEFT[usize::from(i)] != 0 {
            if bracket_dir == BaseDir::Neutral || usize::from(ctx.br_stack_p) == BIDI_BRACKET_DEPTH {
                return BaseDir::Neutral;
            }

            let p = usize::from(ctx.br_stack_p);
            ctx.br_stack[p].bracket_pos = u32::from(i);
            ctx.br_stack[p].dir = bracket_dir;

            ctx.br_stack_p += 1;
            return bracket_dir;
        } else if ctx.br_stack_p > 0 {
            /*Is the letter a closing bracket of the last opening?*/
            let top = ctx.br_stack[usize::from(ctx.br_stack_p) - 1];
            if letter == u32::from(BRACKET_RIGHT[top.bracket_pos as usize]) {
                bracket_dir = top.dir;
                ctx.br_stack_p -= 1;
                return bracket_dir;
            }
        }

        BaseDir::Neutral
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pos_conv_macros() {
        assert_eq!(set_rtl_pos(5, true), 0x8005);
        assert!(is_rtl_pos(0x8005));
        assert_eq!(get_pos(0x8005), 5);
        assert_eq!(set_rtl_pos(0x8005, false), 5);
    }

    #[test]
    fn letter_classes() {
        let s = BidiState::new();
        assert_eq!(s.get_letter_dir('a' as u32), BaseDir::Ltr);
        assert_eq!(s.get_letter_dir('5' as u32), BaseDir::Weak);
        assert_eq!(s.get_letter_dir(' ' as u32), BaseDir::Neutral);
        assert_eq!(s.get_letter_dir(0x05D0), BaseDir::Rtl);
        assert_eq!(s.get_letter_dir(0x202E), BaseDir::Rtl);
        assert_eq!(BidiState::char_change_to_pair('(' as u32), ')' as u32);
        assert_eq!(BidiState::char_change_to_pair(']' as u32), '[' as u32);
        assert_eq!(BidiState::char_change_to_pair('a' as u32), 'a' as u32);
    }

    #[test]
    fn mixed_paragraph_reorders_runs() {
        let s = BidiState::new();
        /* "abc " + two Hebrew letters in an LTR paragraph: the RTL run is reversed in place */
        let input = "abc \u{05D0}\u{05D1}".as_bytes();
        let mut out = vec![0u8; input.len() + 1];
        s.process_paragraph(input, Some(&mut out), input.len() as u32, BaseDir::Ltr, None, 0);
        assert_eq!(&out[..input.len()], "abc \u{05D1}\u{05D0}".as_bytes());
        /* RTL paragraph: the LTR run moves to the end, the brackets are mirrored */
        let input = "\u{05D0} (x)".as_bytes();
        let mut out = vec![0u8; input.len() + 1];
        let mut pos = [0u16; 5];
        s.process_paragraph(
            input,
            Some(&mut out),
            input.len() as u32,
            BaseDir::Rtl,
            Some(&mut pos),
            5,
        );
        assert_eq!(&out[..input.len()], "(x) \u{05D0}".as_bytes());
        assert_eq!(pos, [0x8004, 3, 0x8002, 0x8001, 0x8000]);
    }
}
