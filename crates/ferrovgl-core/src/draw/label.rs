//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_label.h`, `src/draw/lv_draw_label_private.h`,
//! `src/draw/lv_draw_label.c`.
//!
//! Like `rect.rs`, every task creator exists as `DrawCtx::draw_*`
//! (unit-less finalization) and `Ui::draw_*` (full
//! `lv_draw_finalize_task_creation()`).
//!
//! Differences forced by the Rust data model:
//!
//! - `LV_FONT_DEFAULT` is not reachable from a bare descriptor, so the
//!   `font` of [`LabelDsc::new`] / [`LetterDsc::new`] is `None`
//!   (`lv_obj_init_draw_label_dsc()` sets it from the style).
//! - `lv_draw_glyph_dsc_t::g` is a value ([`DrawGlyphDsc::g`]), so
//!   [`DrawCtx::unit_draw_letter`] always takes the `dsc->g == NULL` path
//!   (look the glyph up in `font`); [`DrawCtx::label_iterate_characters`]
//!   hands its kerning-aware glyph to the shared internal function directly,
//!   which is the `dsc->g != NULL` path.
//! - The glyph pixels: `Font::fallback()` returns a borrowed `&dyn Font`,
//!   so the font that resolved a glyph cannot always be stored as a
//!   [`FontRef`] in [`DrawGlyphDsc::resolved_font`] (it is `None` for a
//!   fallback font). To keep fallback glyphs drawable, `unit_draw_letter`
//!   renders bitmap glyphs (`A1`..`A8`) into the A8 scratch buffer
//!   [`DrawGlyphDsc::draw_buf`] right away (`glyph_data = ScratchBuf`),
//!   exactly what the renderer would get from
//!   `lv_font_get_glyph_bitmap(g, _draw_buf)`. The static-bitmap fast path
//!   of `lv_draw_sw_letter.c` (`A8` glyph of a font with static bitmaps) is
//!   left to the renderer and flagged as `glyph_data = StaticBitmap`.
//! - BiDi (`LV_USE_BIDI`): the paragraph processing is stubbed (identity),
//!   only the base direction / alignment resolution is kept.

use alloc::boxed::Box;
use alloc::rc::Rc;

use crate::core::Ui;
use crate::draw::rect::{FillDsc, TaskSink, check_layer, set_task_dsc};
use crate::draw::{DrawBuf, DrawCtx, DrawDsc, DrawDscBase, DrawTaskRef, DrawTaskType, LayerId};
use crate::font::{self, Font, FontRef, GlyphDsc, GlyphFormat};
use crate::misc::area::{Area, Point};
use crate::misc::color::{Color, ColorFormat, OPA_COVER, OPA_MIN, Opa};
use crate::misc::palette::{self, Palette};
use crate::misc::style::{BlendMode, ImageSrc, SCALE_NONE};
use crate::misc::text::{
    self, BaseDir, TEXT_LEN_MAX, TXT_COLOR_CMD, TextAlign, TextAttributes, TextDecor, TextFlags,
};
use crate::misc::types::coord;

/// `LV_DRAW_LABEL_NO_TXT_SEL`
pub const DRAW_LABEL_NO_TXT_SEL: u32 = 0xFFFF;

/// `LABEL_RECOLOR_PAR_LENGTH`
const LABEL_RECOLOR_PAR_LENGTH: u32 = 6;

/// `LV_LABEL_HINT_UPDATE_TH`: update the "hint" if the label's y coordinates
/// have changed more then this.
const LV_LABEL_HINT_UPDATE_TH: i32 = 1024;

/// `cmd_state_t`: state of the recolor command parser of
/// `lv_draw_label_iterate_characters()`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum RecolorCmdState {
    /// `RECOLOR_CMD_STATE_WAIT_FOR_PARAMETER`
    WaitForParameter,
    /// `RECOLOR_CMD_STATE_PARAMETER`
    Parameter,
    /// `RECOLOR_CMD_STATE_TEXT_INPUT`
    TextInput,
}

/// `lv_draw_label_hint_t`: cache to speed up drawing of long texts.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct LabelHint {
    /// `line_start`: index of the first letter of the cached line.
    pub line_start: i32,
    /// `y`: y coordinate of the cached line.
    pub y: i32,
    /// `coord_y`: the `y1` of the text area when the hint was saved.
    pub coord_y: i32,
}

/// `lv_draw_label_dsc_t`
#[derive(Clone, Debug)]
pub struct LabelDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `text`
    pub text: Rc<str>,
    /// `text_size`
    pub text_size: Point,
    /// `font`
    pub font: Option<FontRef>,
    /// `color`
    pub color: Color,
    /// `line_space`
    pub line_space: i32,
    /// `letter_space`
    pub letter_space: i32,
    /// `ofs_x`
    pub ofs_x: i32,
    /// `ofs_y`
    pub ofs_y: i32,
    /// `rotation`
    pub rotation: i32,
    /// `sel_start`
    pub sel_start: u32,
    /// `sel_end`
    pub sel_end: u32,
    /// `sel_color`
    pub sel_color: Color,
    /// `sel_bg_color`
    pub sel_bg_color: Color,
    /// `text_length`: number of bytes to draw (0 = whole text).
    pub text_length: u32,
    /// `align`
    pub align: TextAlign,
    /// `bidi_dir`
    pub bidi_dir: BaseDir,
    /// `opa`
    pub opa: Opa,
    /// `outline_stroke_opa`
    pub outline_stroke_opa: Opa,
    /// `decor`
    pub decor: TextDecor,
    /// `flag`
    pub flag: TextFlags,
    /// `text_local`: the text was copied into the task.
    pub text_local: bool,
    /// `text_static`: the text is in static memory.
    pub text_static: bool,
    /// `has_bided`: the text already went through BiDi processing.
    pub has_bided: bool,
    /// `hint`
    pub hint: Option<Rc<core::cell::Cell<LabelHint>>>,
    /// `outline_stroke_color`
    pub outline_stroke_color: Color,
    /// `outline_stroke_width`
    pub outline_stroke_width: i32,
}

impl LabelDsc {
    /// `lv_draw_label_dsc_init()`
    ///
    /// `font` is `LV_FONT_DEFAULT` upstream; it starts as `None` here (see
    /// the module docs).
    pub fn new() -> Self {
        Self {
            base: DrawDscBase::new(),
            text: Rc::from(""),
            text_size: Point::new(0, 0),
            font: None,
            color: Color::black(),
            line_space: 0,
            letter_space: 0,
            ofs_x: 0,
            ofs_y: 0,
            rotation: 0,
            sel_start: DRAW_LABEL_NO_TXT_SEL,
            sel_end: DRAW_LABEL_NO_TXT_SEL,
            sel_color: Color::black(),
            sel_bg_color: palette::main(Palette::Blue),
            text_length: TEXT_LEN_MAX,
            align: TextAlign::Auto,
            bidi_dir: BaseDir::Ltr,
            opa: OPA_COVER,
            outline_stroke_opa: 0,
            decor: TextDecor::NONE,
            flag: TextFlags::NONE,
            text_local: false,
            text_static: false,
            has_bided: false,
            hint: None,
            outline_stroke_color: Color::BLACK,
            outline_stroke_width: 0,
        }
    }
}

impl Default for LabelDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_letter_dsc_t`
#[derive(Clone, Debug)]
pub struct LetterDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `unicode`
    pub unicode: u32,
    /// `font`
    pub font: Option<FontRef>,
    /// `color`
    pub color: Color,
    /// `rotation`
    pub rotation: i32,
    /// `scale_x`
    pub scale_x: i32,
    /// `scale_y`
    pub scale_y: i32,
    /// `skew_x`
    pub skew_x: i32,
    /// `skew_y`
    pub skew_y: i32,
    /// `pivot`
    pub pivot: Point,
    /// `opa`
    pub opa: Opa,
    /// `decor`
    pub decor: TextDecor,
    /// `blend_mode`
    pub blend_mode: BlendMode,
    /// `outline_stroke_opa`
    pub outline_stroke_opa: Opa,
    /// `outline_stroke_width`
    pub outline_stroke_width: i32,
    /// `outline_stroke_color`
    pub outline_stroke_color: Color,
}

impl LetterDsc {
    /// `lv_draw_letter_dsc_init()`
    ///
    /// `font` is `LV_FONT_DEFAULT` upstream; it starts as `None` here (see
    /// the module docs).
    pub fn new() -> Self {
        Self {
            base: DrawDscBase::new(),
            unicode: 0,
            font: None,
            color: Color::black(),
            rotation: 0,
            scale_x: SCALE_NONE,
            scale_y: SCALE_NONE,
            skew_x: 0,
            skew_y: 0,
            pivot: Point::new(0, 0),
            opa: OPA_COVER,
            decor: TextDecor::NONE,
            blend_mode: BlendMode::Normal,
            outline_stroke_opa: 0,
            outline_stroke_width: 0,
            outline_stroke_color: Color::BLACK,
        }
    }
}

impl Default for LetterDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `glyph_data` of `lv_draw_glyph_dsc_t`: where the glyph pixels are.
///
/// Owned / indexed (no borrowed lifetime) so that the renderer callback can
/// set it after fetching the bitmap lazily, exactly like
/// `lv_draw_sw_letter.c` does.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum GlyphData {
    /// `NULL`: nothing to draw (placeholder or missing glyph).
    #[default]
    None,
    /// The glyph was rendered into [`DrawGlyphDsc::draw_buf`] (A8) with
    /// `Font::glyph_bitmap_a8` (`lv_font_get_glyph_bitmap(g, _draw_buf)`).
    ScratchBuf,
    /// `req_raw_bitmap` fast path: the font has a static bitmap, read it
    /// through `Font::glyph_static_bitmap` on [`DrawGlyphDsc::resolved_font`].
    StaticBitmap,
    /// `LV_FONT_GLYPH_FORMAT_IMAGE`: the glyph is an image source.
    Image(ImageSrc),
}

/// `lv_draw_glyph_dsc_t`: per-glyph data handed to the renderer callback.
pub struct DrawGlyphDsc {
    /// `glyph_data`: the rendered glyph, see [`GlyphData`].
    pub glyph_data: GlyphData,
    /// `_draw_buf`: the reusable A8 scratch buffer the glyph is rendered
    /// into (`lv_draw_buf_reshape` / `lv_draw_buf_create_ex(font_draw_buf_handlers, ...)`
    /// per glyph in `lv_draw_unit_draw_letter`). Do not read it directly,
    /// use `glyph_data`.
    pub draw_buf: Option<DrawBuf>,
    /// `g->resolved_font`: the font that actually has the glyph (after
    /// fallback resolution); `None` for a placeholder.
    pub resolved_font: Option<FontRef>,
    /// `format`
    pub format: GlyphFormat,
    /// `letter_coords`
    pub letter_coords: Area,
    /// `bg_coords`: background area (selection).
    pub bg_coords: Option<Area>,
    /// `g`: glyph metrics.
    pub g: GlyphDsc,
    /// `color`
    pub color: Color,
    /// `opa`
    pub opa: Opa,
    /// `outline_stroke_color`
    pub outline_stroke_color: Color,
    /// `outline_stroke_opa`
    pub outline_stroke_opa: Opa,
    /// `outline_stroke_width`
    pub outline_stroke_width: i32,
    /// `rotation`
    pub rotation: i32,
    /// `pivot`
    pub pivot: Point,
}

impl DrawGlyphDsc {
    /// `lv_draw_glyph_dsc_init()`
    pub fn new() -> Self {
        Self {
            glyph_data: GlyphData::None,
            draw_buf: None,
            resolved_font: None,
            format: GlyphFormat::None,
            letter_coords: Area::new(0, 0, 0, 0),
            bg_coords: None,
            g: GlyphDsc::default(),
            color: Color::BLACK,
            opa: 0,
            outline_stroke_color: Color::BLACK,
            outline_stroke_opa: 0,
            outline_stroke_width: 0,
            rotation: 0,
            pivot: Point::new(0, 0),
        }
    }
}

impl Default for DrawGlyphDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_glyph_cb_t`: called for every glyph (and for underline /
/// strikethrough fills, when `fill_dsc` is given).
///
/// When the last argument is `Some((fill_dsc, fill_area))` the call is a
/// fill request (upstream passes `glyph_draw_dsc == NULL`) and the glyph
/// descriptor must be ignored.
pub type DrawGlyphCallback<'a> =
    &'a mut dyn FnMut(&mut DrawCtx, DrawTaskRef, &mut DrawGlyphDsc, Option<(&FillDsc, &Area)>);

/*
 * Helpers.
 */

/// `txt[i]` of a C string: 0 at and past the end.
#[inline]
fn byte_at(txt: &[u8], i: u32) -> u8 {
    txt.get(i as usize).copied().unwrap_or(0)
}

/// `&txt[i]` of a C string: the empty string at and past the end (or when
/// `i` is not on a character boundary, which the line breaker never
/// produces).
#[inline]
fn str_tail(txt: &str, i: u32) -> &str {
    txt.get(i as usize..).unwrap_or("")
}

/// `lv_strndup(txt, n)`: at most `n` bytes, stopping at a NUL, cut back to
/// a character boundary so that the copy stays valid UTF-8.
fn strndup(txt: &str, n: u32) -> Rc<str> {
    let bytes = txt.as_bytes();
    let mut len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    len = len.min(n as usize);
    while len > 0 && !txt.is_char_boundary(len) {
        len -= 1;
    }
    Rc::from(&txt[..len])
}

/// Address of a font, for identity comparison with [`FontRef::addr`].
#[inline]
fn font_addr(f: &dyn Font) -> usize {
    (f as *const dyn Font).cast::<u8>() as usize
}

/// `g.resolved_font` as a [`FontRef`]: only the font the lookup started
/// with can be named (see the module docs).
fn resolved_font_ref(font: &FontRef, resolved: Option<&dyn Font>) -> Option<FontRef> {
    match resolved {
        Some(r) if font_addr(r) == font.addr() => Some(font.clone()),
        _ => None,
    }
}

/// `lv_bidi_calculate_align_internal()` (`LV_USE_BIDI == 0`)
#[cfg(not(feature = "bidi"))]
fn bidi_calculate_align(align: &mut TextAlign, base_dir: &mut BaseDir, txt: &str) {
    let _ = (txt, base_dir);
    if *align == TextAlign::Auto {
        *align = TextAlign::Left;
    }
}

/// `lv_bidi_calculate_align_internal()` (`LV_USE_BIDI == 1`). The BiDi
/// algorithm is not ported: `lv_bidi_detect_base_dir()` is stubbed to
/// `LV_BIDI_BASE_DIR_DEF` (LTR).
#[cfg(feature = "bidi")]
fn bidi_calculate_align(align: &mut TextAlign, base_dir: &mut BaseDir, txt: &str) {
    let _ = txt;
    if *base_dir == BaseDir::Auto {
        *base_dir = BaseDir::Ltr;
    }

    if *align == TextAlign::Auto {
        if *base_dir == BaseDir::Rtl {
            *align = TextAlign::Right;
        } else {
            *align = TextAlign::Left;
        }
    }
}

/// `hex_char_to_num()`: convert a hexadecimal character to a number
/// (0..15), 0 on error.
fn hex_char_to_num(hex: u8) -> u8 {
    let mut hex = hex;
    if hex.is_ascii_digit() {
        return hex - b'0';
    }
    /*Convert to upper case. (A signed C `char` >= 0x80 is negative, so it
     *is never converted; the result is 0 either way.)*/
    if (b'a'..0x80).contains(&hex) {
        hex -= b'a' - b'A';
    }
    if (b'A'..=b'F').contains(&hex) {
        hex - b'A' + 10
    } else {
        0
    }
}

/// `LV_ROUND_UP(x, round_to)`
#[inline]
fn round_up(x: u32, round_to: u32) -> u32 {
    x.div_ceil(round_to) * round_to
}

/*
 * Task creation.
 */

/// `lv_draw_label()`
fn draw_label_impl<S: TaskSink + ?Sized>(s: &mut S, layer: LayerId, dsc: &LabelDsc, coords: &Area) {
    if !check_layer(s.ctx(), layer) {
        return;
    }

    if dsc.opa <= OPA_MIN {
        return;
    }
    if dsc.text.is_empty() || dsc.text.as_bytes()[0] == 0 {
        return;
    }
    if dsc.font.is_none() {
        #[cfg(feature = "log")]
        log::warn!("dsc->font == NULL");
        return;
    }
    if coords.x1 > coords.x2 {
        /* Attempting to draw a label too small (negative width), cancel to avoid error */
        return;
    }

    if dsc.base.drop_shadow_opa != 0
        && let Some(ds_layer) = s.ctx().layer_create_drop_shadow(layer, &dsc.base, coords)
    {
        let mut ds_dsc = dsc.clone();
        ds_dsc.base.drop_shadow_opa = 0; /*Disable drop shadow so rendering below will render plain label*/
        draw_label_impl(s, ds_layer, &ds_dsc, coords);
        s.ctx().layer_finish_drop_shadow(ds_layer, &dsc.base);
    }

    let t = s.ctx().add_task(layer, coords, DrawTaskType::Label);

    let mut new_dsc = dsc.clone();

    /*The text is stored in a local variable so malloc memory for it*/
    if dsc.text_local {
        new_dsc.text = strndup(&dsc.text, dsc.text_length);
    }

    set_task_dsc(s.ctx(), t, DrawDsc::Label(Box::new(new_dsc)));

    s.finalize(t);
}

/// `lv_draw_character()`
fn draw_character_impl<S: TaskSink + ?Sized>(
    s: &mut S,
    layer: LayerId,
    dsc: &mut LabelDsc,
    point: Point,
    unicode_letter: u32,
) {
    if !check_layer(s.ctx(), layer) {
        return;
    }

    if dsc.opa <= OPA_MIN {
        return;
    }
    let Some(font_ref) = dsc.font.clone() else {
        #[cfg(feature = "log")]
        log::warn!("dsc->font == NULL");
        return;
    };

    if text::is_marker(unicode_letter) {
        return;
    }

    let (g, resolved) = font::get_glyph_dsc(font_ref.as_font(), unicode_letter, 0);

    let a = Area::new(
        point.x,
        point.y,
        point.x + i32::from(g.adv_w),
        point.y + font::get_line_height(resolved.unwrap_or(font_ref.as_font())),
    );

    /*lv_draw_label needs UTF8 text so convert the Unicode character to an UTF8 string */
    let letter_buf = text::unicode_to_encoded(unicode_letter).to_ne_bytes();
    let mut start = 0;
    if cfg!(target_endian = "big") {
        while start < letter_buf.len() && letter_buf[start] == 0 {
            start += 1;
        }
    }
    let end = letter_buf[start..]
        .iter()
        .position(|&b| b == 0)
        .map_or(letter_buf.len(), |p| start + p);
    let letter_str = core::str::from_utf8(&letter_buf[start..end]).unwrap_or("");

    dsc.text = Rc::from(letter_str);
    dsc.text_local = true;

    draw_label_impl(s, layer, dsc, &a);
}

/// `lv_draw_letter()`
fn draw_letter_impl<S: TaskSink + ?Sized>(s: &mut S, layer: LayerId, dsc: &mut LetterDsc, point: Point) {
    if !check_layer(s.ctx(), layer) {
        return;
    }

    if dsc.opa <= OPA_MIN {
        return;
    }
    let Some(font_ref) = dsc.font.clone() else {
        #[cfg(feature = "log")]
        log::warn!("dsc->font == NULL");
        return;
    };

    let (g, resolved) = font::get_glyph_dsc(font_ref.as_font(), dsc.unicode, 0);

    let font: &dyn Font = resolved.unwrap_or(font_ref.as_font());

    let a = Area::new(
        point.x,
        point.y,
        point.x + i32::from(g.adv_w),
        point.y + font::get_line_height(font),
    );

    dsc.pivot.x = i32::from(g.adv_w) / 2;
    dsc.pivot.y = font.line_height() - font.base_line();

    let t = s.ctx().add_task(layer, &a, DrawTaskType::Letter);

    set_task_dsc(s.ctx(), t, DrawDsc::Letter(Box::new(dsc.clone())));

    s.finalize(t);
}

/*
 * Glyph iteration.
 */

/// `lv_draw_label_iterate_characters()`
fn label_iterate_characters_impl(
    ctx: &mut DrawCtx,
    t: DrawTaskRef,
    dsc: &LabelDsc,
    coords: &Area,
    cb: DrawGlyphCallback<'_>,
) {
    let Some(task) = ctx.task(t) else {
        crate::misc::check_arg_failed("t != NULL");
        return;
    };
    let clip_area = task.clip_area;
    let base_obj = task.dsc.base().and_then(|b| b.obj);
    let Some(font_ref) = dsc.font.clone() else {
        crate::misc::check_arg_failed("dsc->font != NULL");
        return;
    };
    let font: &dyn Font = font_ref.as_font();
    let txt: &str = &dsc.text;
    let bytes = txt.as_bytes();
    let w: i32;

    if Area::intersect(coords, &clip_area).is_none() {
        return;
    }

    let mut align = dsc.align;
    let mut base_dir = dsc.bidi_dir;

    bidi_calculate_align(&mut align, &mut base_dir, txt);

    if !dsc.flag.contains(TextFlags::EXPAND) {
        /*Normally use the label's width as width*/
        w = coords.width();
    } else {
        /*If EXPAND is enabled then not limit the text's width to the object's width*/
        /*`lv_obj_is_send_draw_task_events(obj)` needs the object tree, which a
         *renderer does not have: an object's `text_size` is trusted.*/
        if base_obj.is_some() {
            w = dsc.text_size.x;
        } else {
            let mut attributes = TextAttributes::new();

            attributes.letter_space = dsc.letter_space;
            attributes.line_space = dsc.line_space;
            attributes.max_width = coord::MAX;
            attributes.text_flags = dsc.flag;

            let p = text::get_size_attributes(txt, font, &attributes);
            w = p.x;
        }
    }

    let line_height_font = font::get_line_height(font);
    let line_height = line_height_font + dsc.line_space;

    /*Init variables for the first line*/
    let mut pos = Point::new(coords.x1, coords.y1);

    let x_ofs = dsc.ofs_x;
    let y_ofs = dsc.ofs_y;
    pos.y += y_ofs;

    let mut line_start: u32 = 0;
    let mut last_line_start: i32 = -1;

    /*Check the hint to use the cached info*/
    if let Some(hint) = &dsc.hint
        && y_ofs == 0
        && coords.y1 < 0
    {
        /*If the label changed too much recalculate the hint.*/
        let mut h = hint.get();
        if h.coord_y.wrapping_sub(coords.y1).wrapping_abs() > LV_LABEL_HINT_UPDATE_TH - 2 * line_height {
            h.line_start = -1;
            hint.set(h);
        }
        last_line_start = h.line_start;
    }

    /*Use the hint if it's valid*/
    if let Some(hint) = &dsc.hint
        && last_line_start >= 0
    {
        line_start = last_line_start as u32;
        pos.y += hint.get().y;
    }

    let mut remaining_len = dsc.text_length;
    let mut attributes = TextAttributes::new();
    attributes.letter_space = dsc.letter_space;
    attributes.text_flags = dsc.flag;
    attributes.max_width = w;

    let mut line_end = line_start.wrapping_add(text::get_next_line(
        str_tail(txt, line_start),
        remaining_len,
        font,
        None,
        &mut attributes,
    ));

    /*Go the first visible line*/
    while pos.y + line_height_font < clip_area.y1 {
        /*Go to next line*/
        remaining_len = remaining_len.wrapping_sub(line_end.wrapping_sub(line_start));
        line_start = line_end;
        line_end = line_end.wrapping_add(text::get_next_line(
            str_tail(txt, line_start),
            remaining_len,
            font,
            None,
            &mut attributes,
        ));
        pos.y += line_height;

        /*Save at the threshold coordinate*/
        if let Some(hint) = &dsc.hint
            && pos.y >= -LV_LABEL_HINT_UPDATE_TH
            && hint.get().line_start < 0
        {
            hint.set(LabelHint {
                line_start: line_start as i32,
                y: pos.y - coords.y1,
                coord_y: coords.y1,
            });
        }

        if byte_at(bytes, line_start) == 0 {
            return;
        }
    }

    /*Align to middle*/
    if align == TextAlign::Center {
        let line_width = text::get_line_width(
            str_tail(txt, line_start),
            line_end.wrapping_sub(line_start),
            font,
            &attributes,
        );
        pos.x += (coords.width() - line_width) / 2;
    }
    /*Align to the right*/
    else if align == TextAlign::Right {
        let line_width = text::get_line_width(
            str_tail(txt, line_start),
            line_end.wrapping_sub(line_start),
            font,
            &attributes,
        );
        pos.x += coords.width() - line_width;
    }

    let mut sel_start = dsc.sel_start;
    let mut sel_end = dsc.sel_end;
    if sel_start > sel_end {
        core::mem::swap(&mut sel_start, &mut sel_end);
    }

    let mut draw_letter_dsc = DrawGlyphDsc::new();
    draw_letter_dsc.opa = dsc.opa;
    draw_letter_dsc.bg_coords = Some(Area::new(0, 0, 0, 0));
    draw_letter_dsc.color = dsc.color;
    draw_letter_dsc.rotation = dsc.rotation;

    /* Set letter outline stroke attributes */
    draw_letter_dsc.outline_stroke_width = dsc.outline_stroke_width;
    draw_letter_dsc.outline_stroke_opa = dsc.outline_stroke_opa;
    draw_letter_dsc.outline_stroke_color = dsc.outline_stroke_color;

    let mut fill_dsc = FillDsc::new();
    fill_dsc.opa = dsc.opa;
    let underline_thickness = i32::from(font.underline_thickness());
    let underline_width = if underline_thickness != 0 {
        underline_thickness
    } else {
        1
    };
    let mut recolor_command_start_index: u32 = 0;

    let mut recolor_cmd_state = RecolorCmdState::WaitForParameter;
    let mut recolor = Color::black(); /* Holds the selected color inside the recolor command */
    let mut is_first_space_after_cmd = false;

    let cmd_char = u32::from(TXT_COLOR_CMD.as_bytes()[0]);
    let has_recolor = dsc.flag.contains(TextFlags::RECOLOR);
    let sel_enabled = sel_start != DRAW_LABEL_NO_TXT_SEL && sel_end != DRAW_LABEL_NO_TXT_SEL;

    /*Write out all lines*/
    while remaining_len != 0 && byte_at(bytes, line_start) != 0 {
        pos.x += x_ofs;
        let line_start_x = pos.x;
        let mut decor_end_x = pos.x - 1;

        /*Write all letter of a line*/
        let mut next_char_offset: u32 = 0;
        /*`LV_USE_BIDI`: `lv_bidi_process_paragraph()` is stubbed to a copy,
         *so the logical and visual orders are the same.*/
        let bidi_txt: &str = str_tail(txt, line_start);
        let bidi_bytes = bidi_txt.as_bytes();

        while next_char_offset < remaining_len && next_char_offset < line_end.wrapping_sub(line_start) {
            let mut logical_char_pos: u32 = 0;

            /* Check if the text selection is enabled */
            if sel_enabled {
                logical_char_pos =
                    text::encoded_get_char_id(bytes, line_start.wrapping_add(next_char_offset));
            }

            let mut letter: u32 = 0;
            let mut letter_next: u32 = 0;
            text::encoded_letter_next_2(bidi_txt, &mut letter, &mut letter_next, &mut next_char_offset);

            /* If recolor is enabled */
            if has_recolor {
                if letter == cmd_char {
                    /* Handle the recolor command marker depending of the current recolor state */

                    match recolor_cmd_state {
                        RecolorCmdState::WaitForParameter => {
                            recolor_command_start_index = next_char_offset;
                            recolor_cmd_state = RecolorCmdState::Parameter;
                            continue;
                        }
                        /*Other start char in parameter escaped cmd. char*/
                        RecolorCmdState::Parameter => {
                            recolor_cmd_state = RecolorCmdState::WaitForParameter;
                        }
                        /* If letter is LV_TXT_COLOR_CMD and we were in the CMD_STATE_IN then the recolor close marked has been found */
                        RecolorCmdState::TextInput => {
                            recolor_cmd_state = RecolorCmdState::WaitForParameter;
                            continue;
                        }
                    }
                }

                /* Find the first space (aka ' ') after the recolor command parameter, we need to skip rendering it */
                is_first_space_after_cmd = recolor_cmd_state == RecolorCmdState::Parameter
                    && letter == u32::from(b' ')
                    && !is_first_space_after_cmd;

                /* Skip the color parameter and wait the space after it
                 * Once we have reach the space ' ', then we will extract the color information
                 * and store it into the recolor variable */
                if recolor_cmd_state == RecolorCmdState::Parameter {
                    /* Not an space? Continue with the next character */
                    if letter != u32::from(b' ') {
                        continue;
                    }

                    /*Get the recolor parameter*/
                    if next_char_offset.wrapping_sub(recolor_command_start_index)
                        == LABEL_RECOLOR_PAR_LENGTH + 1
                    {
                        /* Temporary buffer to hold the recolor information */
                        let mut buf = [0u8; LABEL_RECOLOR_PAR_LENGTH as usize + 1];
                        for (k, b) in buf.iter_mut().take(LABEL_RECOLOR_PAR_LENGTH as usize).enumerate() {
                            *b = byte_at(bidi_bytes, recolor_command_start_index + k as u32);
                        }
                        buf[LABEL_RECOLOR_PAR_LENGTH as usize] = 0;

                        let r = (hex_char_to_num(buf[0]) << 4) + hex_char_to_num(buf[1]);
                        let g = (hex_char_to_num(buf[2]) << 4) + hex_char_to_num(buf[3]);
                        let b = (hex_char_to_num(buf[4]) << 4) + hex_char_to_num(buf[5]);

                        recolor = Color::make(r, g, b);
                    } else {
                        recolor.red = dsc.color.red;
                        recolor.blue = dsc.color.blue;
                        recolor.green = dsc.color.green;
                    }

                    /*After the parameter the text is in the command*/
                    recolor_cmd_state = RecolorCmdState::TextInput;
                }

                /* Don't draw the first space after the recolor command */
                if is_first_space_after_cmd {
                    continue;
                }
            }

            /* If we're in the CMD_STATE_IN state then we need to subtract the recolor command length */
            if has_recolor && recolor_cmd_state == RecolorCmdState::TextInput {
                logical_char_pos = logical_char_pos.wrapping_sub(LABEL_RECOLOR_PAR_LENGTH + 1);
            }

            let (glyph_dsc, resolved) = font::get_glyph_dsc(font, letter, letter_next);
            let letter_w = if text::is_marker(letter) {
                0
            } else {
                i32::from(glyph_dsc.adv_w)
            };

            if !text::is_hanging_space(letter) {
                decor_end_x = pos.x + letter_w - 1;
            }

            /*Always set the bg_coordinates for placeholder drawing*/
            let bg_coords = Area::new(
                pos.x - dsc.letter_space / 2,
                pos.y,
                pos.x + letter_w - 1 + (dsc.letter_space + 1) / 2,
                pos.y + line_height - 1,
            );
            draw_letter_dsc.bg_coords = Some(bg_coords);

            if next_char_offset >= line_end.wrapping_sub(line_start) && decor_end_x >= line_start_x {
                if dsc.decor.contains(TextDecor::UNDERLINE) {
                    let y1 =
                        pos.y + font.line_height() - font.base_line() - i32::from(font.underline_position());
                    let fill_area = Area::new(line_start_x, y1, decor_end_x, y1 + underline_width - 1);

                    fill_dsc.color = dsc.color;
                    cb(ctx, t, &mut draw_letter_dsc, Some((&fill_dsc, &fill_area)));
                }
                if dsc.decor.contains(TextDecor::STRIKETHROUGH) {
                    let y1 =
                        pos.y + (font.line_height() - font.base_line()) * 2 / 3 + underline_thickness / 2;
                    let fill_area = Area::new(line_start_x, y1, decor_end_x, y1 + underline_width - 1);

                    fill_dsc.color = dsc.color;
                    cb(ctx, t, &mut draw_letter_dsc, Some((&fill_dsc, &fill_area)));
                }
            }

            /* Handle text selection */
            if sel_enabled && logical_char_pos >= sel_start && logical_char_pos < sel_end {
                draw_letter_dsc.color = dsc.sel_color;
                fill_dsc.color = dsc.sel_bg_color;
                cb(ctx, t, &mut draw_letter_dsc, Some((&fill_dsc, &bg_coords)));
            } else if recolor_cmd_state == RecolorCmdState::TextInput {
                draw_letter_dsc.color = recolor;
            } else {
                draw_letter_dsc.color = dsc.color;
            }

            unit_draw_letter_internal(
                ctx,
                t,
                &mut draw_letter_dsc,
                pos,
                &font_ref,
                letter,
                Some((glyph_dsc, resolved)),
                &mut *cb,
            );

            if letter_w > 0 {
                pos.x += letter_w + dsc.letter_space;
            }
        }

        let mut text_attributes = TextAttributes::new();
        text_attributes.letter_space = dsc.letter_space;
        text_attributes.text_flags = dsc.flag;
        text_attributes.max_width = w;

        /*Go to next line*/
        remaining_len = remaining_len.wrapping_sub(line_end.wrapping_sub(line_start));
        line_start = line_end;
        if remaining_len != 0 {
            line_end = line_end.wrapping_add(text::get_next_line(
                str_tail(txt, line_start),
                remaining_len,
                font,
                None,
                &mut text_attributes,
            ));
        }

        pos.x = coords.x1;
        /*Align to middle*/
        if align == TextAlign::Center {
            let line_width = text::get_line_width(
                str_tail(txt, line_start),
                line_end.wrapping_sub(line_start),
                font,
                &text_attributes,
            );

            pos.x += (coords.width() - line_width) / 2;
        }
        /*Align to the right*/
        else if align == TextAlign::Right {
            let line_width = text::get_line_width(
                str_tail(txt, line_start),
                line_end.wrapping_sub(line_start),
                font,
                &text_attributes,
            );
            pos.x += coords.width() - line_width;
        }

        /*Go the next line position*/
        pos.y += line_height;

        let clip_y2 = ctx.task(t).map_or(clip_area.y2, |task| task.clip_area.y2);
        if pos.y > clip_y2 {
            break;
        }
    }

    /*lv_draw_buf_destroy(draw_letter_dsc._draw_buf)*/
    draw_letter_dsc.draw_buf = None;
}

/// `lv_draw_unit_draw_letter_internal()`
///
/// `given` is `dsc->g` when it is set: the glyph metrics and the font that
/// resolved them. `None` looks the glyph up in `font` (`dsc->g == NULL`).
fn unit_draw_letter_internal(
    ctx: &mut DrawCtx,
    t: DrawTaskRef,
    dsc: &mut DrawGlyphDsc,
    pos: Point,
    font: &FontRef,
    letter: u32,
    given: Option<(GlyphDsc, Option<&dyn Font>)>,
    cb: DrawGlyphCallback<'_>,
) {
    if text::is_marker(letter) {
        /*Markers are valid letters but should not be rendered.*/
        return;
    }

    let (g, resolved) = match given {
        /*If the glyph dsc is set then use it*/
        Some(given) => given,
        /*If the glyph dsc is not set then get it from the font*/
        None => {
            let (g, resolved) = font::get_glyph_dsc(font.as_font(), letter, 0);
            #[cfg(feature = "log")]
            if resolved.is_none() && g.is_placeholder {
                /*Add warning if the dsc is not found*/
                log::warn!("lv_draw_letter: glyph dsc. not found for U+{:X}", letter);
            }
            (g, resolved)
        }
    };
    dsc.g = g;
    dsc.resolved_font = resolved_font_ref(font, resolved);

    /*Don't draw anything if the character is empty. E.g. space*/
    if g.box_h == 0 || g.box_w == 0 {
        return;
    }

    let f = font.as_font();
    let mut letter_coords = Area::new(0, 0, 0, 0);
    letter_coords.x1 = pos.x + i32::from(g.ofs_x);
    letter_coords.x2 = letter_coords.x1 + i32::from(g.box_w) - 1;
    letter_coords.y1 = pos.y + (f.line_height() - f.base_line()) - i32::from(g.box_h) - i32::from(g.ofs_y);
    letter_coords.y2 = letter_coords.y1 + i32::from(g.box_h) - 1;
    letter_coords.move_by(-dsc.pivot.x, -dsc.pivot.y);

    /*If the letter is completely out of mask don't draw it*/
    let clip_area = match ctx.task(t) {
        Some(task) => task.clip_area,
        None => return,
    };
    if letter_coords.is_out(&clip_area, 0)
        && let Some(bg_coords) = &dsc.bg_coords
        && bg_coords.is_out(&clip_area, 0)
    {
        return;
    }

    if let Some(resolved) = resolved {
        let mut has_draw_buf = false;
        if (GlyphFormat::None as u8) < (g.format as u8) && (g.format as u8) < (GlyphFormat::Image as u8) {
            /*Only check draw buf for bitmap glyph*/
            let reshaped = match dsc.draw_buf.as_mut() {
                Some(buf) => buf.reshape(
                    ColorFormat::Unknown,
                    u32::from(g.box_w),
                    u32::from(g.box_h),
                    0, /*LV_STRIDE_AUTO*/
                ),
                None => false,
            };
            if !reshaped {
                dsc.draw_buf = None;

                let h = round_up(u32::from(g.box_h), 32); /*Assume a larger size to avoid many reallocations*/
                match DrawBuf::create(u32::from(g.box_w), h, ColorFormat::A8, 0) {
                    Some(mut draw_buf) => {
                        draw_buf.header.h = g.box_h;
                        dsc.draw_buf = Some(draw_buf);
                    }
                    None => {
                        #[cfg(feature = "log")]
                        log::warn!("Failed to allocate memory for glyph draw buffer");
                        return;
                    }
                }
            }
            has_draw_buf = true;
        }

        dsc.format = g.format;
        dsc.glyph_data = GlyphData::None;

        if g.format == GlyphFormat::Vector {
            /*Load the outline of the glyph, even if the function says bitmap.
             *`Font` has no outline API, so there is never glyph data.*/
            dsc.format = GlyphFormat::None;
        } else if has_draw_buf {
            let static_fast_path = g.format == GlyphFormat::A8
                && dsc
                    .resolved_font
                    .as_ref()
                    .is_some_and(|rf| rf.as_font().has_static_bitmap());
            if static_fast_path {
                dsc.glyph_data = GlyphData::StaticBitmap;
            } else if let Some(buf) = dsc.draw_buf.as_mut() {
                /*lv_font_get_glyph_bitmap(g, _draw_buf), see the module docs*/
                let stride = usize::from(buf.header.stride);
                if let Some(data) = buf.data_mut()
                    && resolved.glyph_bitmap_a8(&g, data, stride)
                {
                    dsc.glyph_data = GlyphData::ScratchBuf;
                }
            }
        }
    } else {
        dsc.format = GlyphFormat::None;
    }

    dsc.letter_coords = letter_coords;
    cb(ctx, t, dsc, None);

    /*lv_font_glyph_release_draw_data_internal(): nothing to release*/
}

impl DrawCtx {
    /// `lv_draw_label()`
    pub fn draw_label(&mut self, layer: LayerId, dsc: &LabelDsc, coords: &Area) {
        draw_label_impl(self, layer, dsc, coords);
    }

    /// `lv_draw_character()`
    pub fn draw_character(&mut self, layer: LayerId, dsc: &mut LabelDsc, point: Point, unicode_letter: u32) {
        draw_character_impl(self, layer, dsc, point, unicode_letter);
    }

    /// `lv_draw_letter()`
    pub fn draw_letter(&mut self, layer: LayerId, dsc: &mut LetterDsc, point: Point) {
        draw_letter_impl(self, layer, dsc, point);
    }

    /// `lv_draw_task_get_label_dsc()`
    pub fn task_get_label_dsc(&mut self, task: DrawTaskRef) -> Option<&mut LabelDsc> {
        let task = self.task_mut(task)?;
        if task.ty != DrawTaskType::Label {
            return None;
        }
        match &mut task.dsc {
            DrawDsc::Label(d) => Some(d),
            _ => None,
        }
    }

    /// `lv_draw_label_iterate_characters()`: the shared text layout loop
    /// used by every renderer.
    pub fn label_iterate_characters(
        &mut self,
        t: DrawTaskRef,
        dsc: &LabelDsc,
        coords: &Area,
        cb: DrawGlyphCallback<'_>,
    ) {
        label_iterate_characters_impl(self, t, dsc, coords, cb);
    }

    /// `lv_draw_unit_draw_letter()`: resolve and draw a single glyph through `cb`.
    ///
    /// The glyph is always looked up in `font` (the `dsc->g == NULL` path,
    /// see the module docs).
    pub fn unit_draw_letter(
        &mut self,
        t: DrawTaskRef,
        dsc: &mut DrawGlyphDsc,
        pos: Point,
        font: &FontRef,
        letter: u32,
        cb: DrawGlyphCallback<'_>,
    ) {
        if self.task(t).is_none() {
            crate::misc::check_arg_failed("t != NULL");
            return;
        }
        unit_draw_letter_internal(self, t, dsc, pos, font, letter, None, cb);
    }
}

impl Ui {
    /// `lv_draw_label()` with the full `lv_draw_finalize_task_creation()`.
    pub fn draw_label(&mut self, layer: LayerId, dsc: &LabelDsc, coords: &Area) {
        draw_label_impl(self, layer, dsc, coords);
    }

    /// `lv_draw_character()` with the full task finalization.
    pub fn draw_character(&mut self, layer: LayerId, dsc: &mut LabelDsc, point: Point, unicode_letter: u32) {
        draw_character_impl(self, layer, dsc, point, unicode_letter);
    }

    /// `lv_draw_letter()` with the full task finalization.
    pub fn draw_letter(&mut self, layer: LayerId, dsc: &mut LetterDsc, point: Point) {
        draw_letter_impl(self, layer, dsc, point);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    /// Line height 12, base line 2. Letters are 6 px wide (5 x 7 box, image
    /// format so that no scratch buffer is needed), space and new line are
    /// 4 px wide and empty, `'~'` is missing (placeholder).
    struct TestFont;

    impl Font for TestFont {
        fn line_height(&self) -> i32 {
            12
        }
        fn base_line(&self) -> i32 {
            2
        }
        fn underline_position(&self) -> i8 {
            -1
        }
        fn underline_thickness(&self) -> i8 {
            1
        }
        fn glyph_dsc(&self, out: &mut GlyphDsc, letter: u32, _letter_next: u32) -> bool {
            if letter == u32::from('~') {
                return false;
            }
            let empty = letter == u32::from(' ') || letter == u32::from('\n');
            *out = GlyphDsc {
                adv_w: if empty { 4 } else { 6 },
                box_w: if empty { 0 } else { 5 },
                box_h: if empty { 0 } else { 7 },
                ofs_x: 0,
                ofs_y: 0,
                format: GlyphFormat::Image,
                ..GlyphDsc::default()
            };
            true
        }
        fn glyph_bitmap_a8(&self, _g: &GlyphDsc, _out: &mut [u8], _stride: usize) -> bool {
            false
        }
    }

    fn test_font() -> FontRef {
        FontRef::Shared(Rc::new(TestFont))
    }

    #[derive(Clone, Debug, PartialEq)]
    enum Call {
        Glyph {
            coords: Area,
            color: Color,
            format: GlyphFormat,
        },
        Fill {
            area: Area,
            color: Color,
        },
    }

    fn glyph(x1: i32, y1: i32, color: Color) -> Call {
        Call::Glyph {
            coords: Area::new(x1, y1, x1 + 4, y1 + 6),
            color,
            format: GlyphFormat::Image,
        }
    }

    fn ctx_with_layer(clip: Area) -> (DrawCtx, LayerId) {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Argb8888, &Area::new(-100, -100, 299, 299));
        ctx.layer_mut(layer).unwrap().clip_area = clip;
        (ctx, layer)
    }

    /// Add `dsc` as a label task and iterate it, recording the callbacks.
    fn run_clipped(dsc: &LabelDsc, coords: Area, clip: Area) -> Vec<Call> {
        let (mut ctx, layer) = ctx_with_layer(clip);
        ctx.draw_label(layer, dsc, &coords);
        let t = DrawTaskRef { layer, index: 0 };
        let task_dsc = ctx.task_get_label_dsc(t).unwrap().clone();
        let mut calls = Vec::new();
        let mut cb =
            |_: &mut DrawCtx, _: DrawTaskRef, g: &mut DrawGlyphDsc, fill: Option<(&FillDsc, &Area)>| {
                match fill {
                    Some((f, a)) => calls.push(Call::Fill {
                        area: *a,
                        color: f.color,
                    }),
                    None => calls.push(Call::Glyph {
                        coords: g.letter_coords,
                        color: g.color,
                        format: g.format,
                    }),
                }
            };
        ctx.label_iterate_characters(t, &task_dsc, &coords, &mut cb);
        calls
    }

    fn run(dsc: &LabelDsc) -> Vec<Call> {
        run_clipped(dsc, Area::new(0, 0, 99, 49), Area::new(-100, -100, 299, 299))
    }

    fn label(txt: &str) -> LabelDsc {
        let mut dsc = LabelDsc::new();
        dsc.text = Rc::from(txt);
        dsc.font = Some(test_font());
        dsc
    }

    const BLACK: Color = Color::BLACK;

    #[test]
    fn label_dsc_init_defaults() {
        let d = LabelDsc::new();
        assert_eq!(d.opa, OPA_COVER);
        assert_eq!(d.color, Color::black());
        assert_eq!(d.text_length, TEXT_LEN_MAX);
        assert_eq!(d.sel_start, DRAW_LABEL_NO_TXT_SEL);
        assert_eq!(d.sel_end, DRAW_LABEL_NO_TXT_SEL);
        assert_eq!(d.sel_color, Color::black());
        assert_eq!(d.sel_bg_color, Color::hex(0x2196F3));
        assert_eq!(d.bidi_dir, BaseDir::Ltr);
        assert_eq!(d.align, TextAlign::Auto);
        assert_eq!(d.flag, TextFlags::NONE);
        assert_eq!(d.decor, TextDecor::NONE);
        assert_eq!(d.line_space, 0);
        assert_eq!(d.letter_space, 0);
        assert_eq!(d.rotation, 0);
        assert!(d.font.is_none());
        assert!(d.text.is_empty());
        assert!(!d.text_local && !d.text_static && !d.has_bided);
        assert!(d.hint.is_none());
        assert_eq!(d.outline_stroke_width, 0);
        assert_eq!(d.outline_stroke_opa, 0);

        let l = LetterDsc::new();
        assert_eq!(l.opa, OPA_COVER);
        assert_eq!(l.color, Color::black());
        assert_eq!(l.scale_x, SCALE_NONE);
        assert_eq!(l.scale_y, SCALE_NONE);
        assert_eq!(l.rotation, 0);
        assert_eq!(l.blend_mode, BlendMode::Normal);
        assert!(l.font.is_none());

        let g = DrawGlyphDsc::new();
        assert_eq!(g.glyph_data, GlyphData::None);
        assert_eq!(g.format, GlyphFormat::None);
        assert!(g.draw_buf.is_none() && g.resolved_font.is_none() && g.bg_coords.is_none());
        assert_eq!(g.opa, 0);
    }

    #[test]
    fn draw_label_skips_invisible_labels() {
        let coords = Area::new(0, 0, 49, 19);
        let count = |dsc: &LabelDsc, coords: &Area| {
            let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
            ctx.draw_label(layer, dsc, coords);
            ctx.layer(layer).unwrap().tasks.len()
        };
        assert_eq!(count(&label("A"), &coords), 1);
        let mut d = label("A");
        d.opa = OPA_MIN;
        assert_eq!(count(&d, &coords), 0);
        assert_eq!(count(&label(""), &coords), 0);
        assert_eq!(count(&label("\0A"), &coords), 0);
        let mut d = label("A");
        d.font = None;
        assert_eq!(count(&d, &coords), 0);
        /*Negative width*/
        assert_eq!(count(&label("A"), &Area::new(10, 0, 9, 19)), 0);
        /*Zero height is still drawn*/
        assert_eq!(count(&label("A"), &Area::new(0, 5, 9, 4)), 1);
    }

    #[test]
    fn draw_label_copies_a_local_text() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let coords = Area::new(1, 2, 30, 40);
        let mut d = label("Hello");
        d.text_local = true;
        d.text_length = 3;
        ctx.draw_label(layer, &d, &coords);
        let t = DrawTaskRef { layer, index: 0 };
        assert_eq!(ctx.task_get_area(t), coords);
        let l = ctx.task_get_label_dsc(t).unwrap();
        assert_eq!(&*l.text, "Hel");
        assert!(!Rc::ptr_eq(&l.text, &d.text));
        assert_eq!(l.base.layer, Some(layer));

        /*Not local: shared as is, text_length kept*/
        d.text_local = false;
        ctx.draw_label(layer, &d, &coords);
        let l = ctx.task_get_label_dsc(DrawTaskRef { layer, index: 1 }).unwrap();
        assert!(Rc::ptr_eq(&l.text, &d.text));
        assert_eq!(l.text_length, 3);

        /*A cut inside a multi-byte character is moved back*/
        assert_eq!(&*strndup("aé", 2), "a");
        assert_eq!(&*strndup("ab\0cd", 10), "ab");
    }

    #[test]
    fn draw_character_converts_to_utf8() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let mut d = label("");
        ctx.draw_character(layer, &mut d, Point::new(5, 6), u32::from('A'));
        assert_eq!(&*d.text, "A");
        assert!(d.text_local);
        let t = DrawTaskRef { layer, index: 0 };
        /*adv_w and line height are added without -1, like upstream*/
        assert_eq!(ctx.task_get_area(t), Area::new(5, 6, 11, 18));
        assert_eq!(&*ctx.task_get_label_dsc(t).unwrap().text, "A");

        ctx.draw_character(layer, &mut d, Point::new(0, 0), 0x00E9);
        assert_eq!(&*d.text, "é");

        /*Markers are not drawn*/
        let n = ctx.layer(layer).unwrap().tasks.len();
        ctx.draw_character(layer, &mut d, Point::new(0, 0), 0x200B);
        assert_eq!(ctx.layer(layer).unwrap().tasks.len(), n);
    }

    #[test]
    fn draw_letter_sets_the_pivot() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let mut d = LetterDsc::new();
        d.font = Some(test_font());
        d.unicode = u32::from('X');
        ctx.draw_letter(layer, &mut d, Point::new(10, 20));
        assert_eq!(d.pivot, Point::new(3, 10));
        let l = ctx.layer(layer).unwrap();
        assert_eq!(l.tasks.len(), 1);
        assert_eq!(l.tasks[0].ty, DrawTaskType::Letter);
        assert_eq!(l.tasks[0].area, Area::new(10, 20, 16, 32));
        match &l.tasks[0].dsc {
            DrawDsc::Letter(ld) => {
                assert_eq!(ld.pivot, Point::new(3, 10));
                assert_eq!(ld.base.layer, Some(layer));
            }
            _ => panic!("not a letter descriptor"),
        }

        d.font = None;
        ctx.draw_letter(layer, &mut d, Point::new(10, 20));
        assert_eq!(ctx.layer(layer).unwrap().tasks.len(), 1);
    }

    #[test]
    fn iterate_places_glyphs_left_aligned() {
        /*'A' at x = 0: y1 = 0 + (12 - 2) - 7 - 0; the space has no box but advances*/
        assert_eq!(
            run(&label("AB C")),
            vec![glyph(0, 3, BLACK), glyph(6, 3, BLACK), glyph(16, 3, BLACK)]
        );

        let mut d = label("AB");
        d.letter_space = 2;
        d.ofs_x = 1;
        d.ofs_y = 2;
        assert_eq!(run(&d), vec![glyph(1, 5, BLACK), glyph(9, 5, BLACK)]);
    }

    #[test]
    fn iterate_aligns_center_and_right() {
        let mut d = label("AB");
        d.align = TextAlign::Center;
        /*(100 - 12) / 2*/
        assert_eq!(run(&d), vec![glyph(44, 3, BLACK), glyph(50, 3, BLACK)]);
        d.align = TextAlign::Right;
        assert_eq!(run(&d), vec![glyph(88, 3, BLACK), glyph(94, 3, BLACK)]);
        /*Hanging spaces are not counted*/
        let mut d = label("AB ");
        d.align = TextAlign::Right;
        assert_eq!(run(&d), vec![glyph(88, 3, BLACK), glyph(94, 3, BLACK)]);
    }

    #[test]
    fn iterate_breaks_lines() {
        let mut d = label("AB\nCD");
        d.line_space = 3;
        assert_eq!(
            run(&d),
            vec![
                glyph(0, 3, BLACK),
                glyph(6, 3, BLACK),
                glyph(0, 18, BLACK),
                glyph(6, 18, BLACK)
            ]
        );

        /*Wrapping at the area width: "AB CD" does not fit in 20 px*/
        let calls = run_clipped(&label("AB CD"), Area::new(0, 0, 19, 49), Area::new(0, 0, 99, 99));
        assert_eq!(
            calls,
            vec![
                glyph(0, 3, BLACK),
                glyph(6, 3, BLACK),
                glyph(0, 15, BLACK),
                glyph(6, 15, BLACK)
            ]
        );
    }

    #[test]
    fn iterate_skips_invisible_lines() {
        let d = label("AB\nCD\nEF");
        /*The first line ends above the clip area (0 + 12 < 13), the third starts below it*/
        let calls = run_clipped(&d, Area::new(0, 0, 99, 49), Area::new(0, 13, 99, 23));
        assert_eq!(calls, vec![glyph(0, 15, BLACK), glyph(6, 15, BLACK)]);

        /*Nothing visible at all*/
        assert!(run_clipped(&d, Area::new(0, 0, 99, 49), Area::new(0, 60, 99, 70)).is_empty());
    }

    #[test]
    fn iterate_draws_underline_and_strikethrough() {
        let mut d = label("AB");
        d.decor = TextDecor::UNDERLINE | TextDecor::STRIKETHROUGH;
        d.color = Color::make(1, 2, 3);
        let c = Color::make(1, 2, 3);
        assert_eq!(
            run(&d),
            vec![
                glyph(0, 3, c),
                /*y1 = 12 - 2 - (-1)*/
                Call::Fill {
                    area: Area::new(0, 11, 11, 11),
                    color: c
                },
                /*y1 = (12 - 2) * 2 / 3 + 1 / 2*/
                Call::Fill {
                    area: Area::new(0, 6, 11, 6),
                    color: c
                },
                glyph(6, 3, c),
            ]
        );

        /*A trailing space is not underlined*/
        let mut d = label("A ");
        d.decor = TextDecor::UNDERLINE;
        assert_eq!(
            run(&d),
            vec![
                glyph(0, 3, BLACK),
                Call::Fill {
                    area: Area::new(0, 11, 5, 11),
                    color: BLACK
                },
            ]
        );
    }

    #[test]
    fn iterate_highlights_the_selection() {
        let mut d = label("ABC");
        /*Swapped on purpose*/
        d.sel_start = 2;
        d.sel_end = 1;
        d.sel_color = Color::make(255, 255, 255);
        d.sel_bg_color = Color::make(0, 0, 200);
        d.letter_space = 2;
        assert_eq!(
            run(&d),
            vec![
                glyph(0, 3, BLACK),
                /*bg: x1 = 8 - 2 / 2, x2 = 8 + 6 - 1 + 3 / 2, y2 = 12 - 1*/
                Call::Fill {
                    area: Area::new(7, 0, 14, 11),
                    color: Color::make(0, 0, 200)
                },
                glyph(8, 3, Color::make(255, 255, 255)),
                glyph(16, 3, BLACK),
            ]
        );
    }

    #[test]
    fn iterate_parses_recolor_commands() {
        let red = Color::make(0xFF, 0, 0);
        let mut d = label("#ff0000 R# G");
        d.flag = TextFlags::RECOLOR;
        /*The command and the first space are not drawn; the space after the
         *closing '#' only advances*/
        assert_eq!(run(&d), vec![glyph(0, 3, red), glyph(10, 3, BLACK)]);

        /*Lower and upper case hex digits*/
        let mut d = label("#0aF0b0 X#");
        d.flag = TextFlags::RECOLOR;
        assert_eq!(run(&d), vec![glyph(0, 3, Color::make(0x0A, 0xF0, 0xB0))]);

        /*A parameter of the wrong length falls back to the label color*/
        let mut d = label("#f00 X#");
        d.flag = TextFlags::RECOLOR;
        d.color = Color::make(1, 2, 3);
        assert_eq!(run(&d), vec![glyph(0, 3, Color::make(1, 2, 3))]);

        /*"##" escapes the command character*/
        let mut d = label("A##");
        d.flag = TextFlags::RECOLOR;
        assert_eq!(run(&d), vec![glyph(0, 3, BLACK), glyph(6, 3, BLACK)]);

        /*Without the flag the command is plain text*/
        assert_eq!(run(&label("#f")).len(), 2);
    }

    #[test]
    fn missing_glyph_is_a_placeholder() {
        let calls = run(&label("~"));
        /*box_w = 12 / 2, box_h = 12: y1 = 0 + 10 - 12*/
        assert_eq!(
            calls,
            vec![Call::Glyph {
                coords: Area::new(0, -2, 5, 9),
                color: BLACK,
                format: GlyphFormat::None,
            }]
        );
    }

    #[test]
    fn hint_is_saved_and_reused() {
        let txt = "A\n".repeat(200);
        let mut d = label(&txt);
        let hint = Rc::new(core::cell::Cell::new(LabelHint {
            line_start: -1,
            y: 0,
            coord_y: 0,
        }));
        d.hint = Some(hint.clone());
        /*The label starts far above the clip area*/
        let coords = Area::new(0, -2000, 99, 400);
        let calls = run_clipped(&d, coords, Area::new(0, 0, 99, 11));
        /*Line 167 is at y = -2000 + 167 * 12 = 4; line 166 (y = -8) ends in the clip area*/
        assert_eq!(calls, vec![glyph(0, -5, BLACK), glyph(0, 7, BLACK)]);
        let h = hint.get();
        /*Saved at the first line at or below -1024: 82 * 12 - 2000 = -1016*/
        assert_eq!(h.line_start, 82 * 2);
        assert_eq!(h.y, 82 * 12);
        assert_eq!(h.coord_y, -2000);

        /*Reused: same result*/
        assert_eq!(run_clipped(&d, coords, Area::new(0, 0, 99, 11)), calls);
        assert_eq!(hint.get(), h);
    }

    #[test]
    fn unit_draw_letter_looks_the_glyph_up() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let t = ctx.add_task(layer, &Area::new(0, 0, 99, 99), DrawTaskType::Label);
        let font = test_font();
        let mut dsc = DrawGlyphDsc::new();
        dsc.pivot = Point::new(1, 2);
        let mut seen = Vec::new();
        let mut cb = |_: &mut DrawCtx, _: DrawTaskRef, g: &mut DrawGlyphDsc, _: Option<(&FillDsc, &Area)>| {
            seen.push((g.letter_coords, g.format, g.resolved_font.is_some()));
        };
        ctx.unit_draw_letter(t, &mut dsc, Point::new(10, 10), &font, u32::from('A'), &mut cb);
        /*Empty glyphs and markers are not drawn*/
        ctx.unit_draw_letter(t, &mut dsc, Point::new(10, 10), &font, u32::from(' '), &mut cb);
        ctx.unit_draw_letter(t, &mut dsc, Point::new(10, 10), &font, 0x200B, &mut cb);
        /*Out of the clip area*/
        dsc.bg_coords = Some(Area::new(200, 200, 210, 210));
        ctx.unit_draw_letter(t, &mut dsc, Point::new(200, 200), &font, u32::from('A'), &mut cb);
        /*Out of the clip area, but its background is not*/
        dsc.bg_coords = Some(Area::new(90, 90, 110, 110));
        ctx.unit_draw_letter(t, &mut dsc, Point::new(200, 200), &font, u32::from('A'), &mut cb);
        assert_eq!(
            seen,
            vec![
                (Area::new(9, 11, 13, 17), GlyphFormat::Image, true),
                (Area::new(199, 201, 203, 207), GlyphFormat::Image, true),
            ]
        );
        assert_eq!(dsc.g.adv_w, 6);
    }

    #[test]
    fn hex_and_round_helpers() {
        assert_eq!(hex_char_to_num(b'0'), 0);
        assert_eq!(hex_char_to_num(b'9'), 9);
        assert_eq!(hex_char_to_num(b'a'), 10);
        assert_eq!(hex_char_to_num(b'F'), 15);
        assert_eq!(hex_char_to_num(b'g'), 0);
        assert_eq!(hex_char_to_num(b'G'), 0);
        assert_eq!(hex_char_to_num(0xE6), 0);
        assert_eq!(round_up(1, 32), 32);
        assert_eq!(round_up(32, 32), 32);
        assert_eq!(round_up(33, 32), 64);
    }
}
