//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_letter.c`.
//!
//! `lv_draw_sw_letter()` and the glyph callback (`draw_letter_cb`) shared
//! with `lv_draw_sw_label()` (see [`crate::label`]).
//!
//! Glyph bitmaps are not fetched here: the core
//! (`ferrovgl_core::draw::label`) already rendered `A1`..`A8` glyphs into
//! the A8 scratch buffer [`DrawGlyphDsc::draw_buf`]
//! (`lv_font_get_glyph_bitmap(g, _draw_buf)`) and flags the static-bitmap
//! fast path with [`GlyphData::StaticBitmap`].
//!
//! Known gaps (upstream code that is active in the parity configuration,
//! `tests/configs/full.defconfig`, but cannot be ported yet because the
//! core lacks the APIs it depends on):
//!
//! * `LV_FONT_GLYPH_FORMAT_IMAGE` glyphs (`LV_USE_IMGFONT=y`): upstream
//!   fetches the image source in `draw_letter_cb()` with
//!   `lv_font_get_glyph_bitmap_internal(g, _draw_buf)` (`lv_imgfont`'s
//!   `get_glyph_bitmap`). `ferrovgl_core::font::Font` has no image-glyph
//!   API and the core never sets [`GlyphData::Image`], so an image glyph
//!   is drawn with [`ImageSrc::None`], i.e. nothing is drawn. Once the
//!   `Font` trait can return an image source, fetch it here (as upstream
//!   does) instead of relying on [`GlyphData::Image`].
//! * The FreeType outline path (`LV_USE_FREETYPE && LV_USE_VECTOR_GRAPHIC
//!   && LV_USE_THORVG`, all `y` in full.defconfig): the
//!   `LV_FONT_GLYPH_FORMAT_VECTOR` case of `draw_letter_cb()`
//!   (`draw_letter_outline()`: ThorVG renders the glyph paths into an
//!   ARGB8888 buffer that is then drawn with `lv_draw_sw_image()`) and
//!   `freetype_outline_event_cb()` with its one-time registration in
//!   `lv_draw_sw_label()` (see [`crate::label`]). This needs FreeType,
//!   vector graphics and a ThorVG backend, none of which exist yet and no
//!   cargo feature selects them. The core currently rewrites `VECTOR`
//!   glyphs to `NONE`, so the `Vector` arm below is a no-op. When a
//!   freetype/vector feature is added, port `draw_letter_outline()` and
//!   the `VECTOR` case behind it, and the event registration as per-unit
//!   state (not a `static`).
//!
//! The renderer needs the unit (custom blend handlers, circle cache) and the
//! task, which the glyph callback signature of the core does not carry: the
//! entry points capture them in a closure around `draw_letter_cb_impl`.

use alloc::rc::Rc;

use ferrovgl_core::draw::image::DrawImageDsc;
use ferrovgl_core::draw::label::{DrawGlyphDsc, GlyphData, LetterDsc};
use ferrovgl_core::draw::rect::{BorderDsc, FillDsc};
use ferrovgl_core::draw::{DrawBuf, DrawCtx, DrawDsc, DrawTask, DrawTaskRef, DrawTaskState};
use ferrovgl_core::font::{GlyphFormat, USE_FONT_PLACEHOLDER};
use ferrovgl_core::misc::area::{Area, Point};
use ferrovgl_core::misc::color::{ColorFormat, OPA_MIN};
use ferrovgl_core::misc::style::{ImageSrc, SCALE_NONE};

use crate::SwDrawUnit;
use crate::blend::{CustomBlendHandler, MaskRes};
use crate::fill::{blend_dsc_zeroed, sw_blend};

/// The task `t` (a copy taken out of the layer queue by `execute_drawing`)
/// as a [`DrawTaskRef`] for the core text iterators: `unit.task_act`, or,
/// if that is not set, the in-progress task of the target layer with the
/// same type and area.
pub(crate) fn task_ref(unit: &SwDrawUnit, ctx: &DrawCtx, t: &DrawTask) -> Option<DrawTaskRef> {
    if let Some(r) = unit.task_act
        && ctx.task(r).is_some()
    {
        return Some(r);
    }
    let layer = ctx.layer(t.target_layer)?;
    let index = layer
        .tasks
        .iter()
        .position(|x| x.state == DrawTaskState::InProgress && x.ty == t.ty && x.area == t.area)?;
    Some(DrawTaskRef {
        layer: t.target_layer,
        index,
    })
}

/// A copy of `task` without its descriptor: everything the blender reads.
fn task_header(task: &DrawTask) -> DrawTask {
    DrawTask {
        ty: task.ty,
        area: task.area,
        real_area: task.real_area,
        clip_area: task.clip_area,
        target_layer: task.target_layer,
        draw_unit: task.draw_unit,
        state: task.state,
        dsc: DrawDsc::None,
        opa: task.opa,
        preferred_draw_unit_id: task.preferred_draw_unit_id,
        preference_score: task.preference_score,
    }
}

/// `lv_draw_sw_letter()`
pub fn letter(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &LetterDsc, coords: &Area) {
    if dsc.opa <= OPA_MIN {
        return;
    }

    let mut glyph_dsc = DrawGlyphDsc::new();
    glyph_dsc.opa = dsc.opa;
    glyph_dsc.bg_coords = None;
    glyph_dsc.color = dsc.color;
    glyph_dsc.rotation = dsc.rotation;
    glyph_dsc.pivot = dsc.pivot;

    let Some(tref) = task_ref(unit, ctx, t) else {
        return;
    };
    /* `dsc->font` is dereferenced unconditionally upstream */
    let Some(font) = dsc.font.as_ref() else {
        return;
    };

    let mut cb =
        |ctx: &mut DrawCtx, _t: DrawTaskRef, g: &mut DrawGlyphDsc, fill: Option<(&FillDsc, &Area)>| {
            draw_letter_cb_impl(unit, ctx, t, g, fill);
        };
    ctx.unit_draw_letter(
        tref,
        &mut glyph_dsc,
        Point::new(coords.x1, coords.y1),
        font,
        dsc.unicode,
        &mut cb,
    );

    /*lv_draw_buf_destroy(glyph_dsc._draw_buf)*/
    glyph_dsc.draw_buf = None;
}

/// `draw_letter_cb()`: the glyph callback shared with `label`.
///
/// This stand-alone form renders with a fresh [`SwDrawUnit`] (no custom
/// blend handlers) and a copy of the task `t`; the renderer entry points
/// use `draw_letter_cb_impl` with their own unit.
pub fn draw_letter_cb(
    ctx: &mut DrawCtx,
    t: DrawTaskRef,
    glyph_draw_dsc: &mut DrawGlyphDsc,
    fill: Option<(&FillDsc, &Area)>,
) {
    let Some(task) = ctx.task(t).map(task_header) else {
        return;
    };
    let mut unit = SwDrawUnit::new(0);
    unit.task_act = Some(t);
    draw_letter_cb_impl(&mut unit, ctx, &task, glyph_draw_dsc, fill);
}

/// `draw_letter_bitmap()`: blend a non-rotated `A1`..`A8` glyph (the
/// `rotation % 3600 == 0` branch of `draw_letter_cb()`).
///
/// Uses no custom blend handlers, see [`draw_letter_cb`].
pub fn draw_letter_bitmap(ctx: &mut DrawCtx, t: DrawTaskRef, dsc: &DrawGlyphDsc) {
    let Some(task) = ctx.task(t).map(task_header) else {
        return;
    };
    draw_letter_bitmap_impl(&[], ctx, &task, dsc);
}

/// The body of `draw_letter_cb()` with the unit and the task of the
/// renderer entry point. `fill` set is the `glyph_draw_dsc == NULL` call
/// (underline, strikethrough, selection background).
pub(crate) fn draw_letter_cb_impl(
    unit: &mut SwDrawUnit,
    ctx: &mut DrawCtx,
    t: &DrawTask,
    glyph_draw_dsc: &mut DrawGlyphDsc,
    fill: Option<(&FillDsc, &Area)>,
) {
    if fill.is_none() {
        match glyph_draw_dsc.format {
            GlyphFormat::None => {
                if USE_FONT_PLACEHOLDER && let Some(bg_coords) = glyph_draw_dsc.bg_coords {
                    /* Draw a placeholder rectangle*/
                    let mut border_draw_dsc = BorderDsc::new();
                    border_draw_dsc.opa = glyph_draw_dsc.opa;
                    border_draw_dsc.color = glyph_draw_dsc.color;
                    border_draw_dsc.width = 1;
                    crate::border::border(unit, ctx, t, &border_draw_dsc, &bg_coords);
                }
            }
            GlyphFormat::A1
            | GlyphFormat::A2
            | GlyphFormat::A3
            | GlyphFormat::A4
            | GlyphFormat::A8
            | GlyphFormat::Image => {
                if glyph_draw_dsc.rotation % 3600 == 0 && glyph_draw_dsc.format != GlyphFormat::Image {
                    draw_letter_bitmap_impl(&unit.blend_handlers, ctx, t, glyph_draw_dsc);
                } else {
                    draw_letter_image(unit, ctx, t, glyph_draw_dsc);
                }
            }
            GlyphFormat::Vector => {
                /* `draw_letter_outline(t, glyph_draw_dsc)` under
                 * `LV_USE_FREETYPE && LV_USE_VECTOR_GRAPHIC && LV_USE_THORVG`:
                 * not ported, see the module docs (known gap). */
            }
            _ => {}
        }
    }

    if let Some((fill_draw_dsc, fill_area)) = fill {
        crate::fill::fill(unit, ctx, t, fill_draw_dsc, fill_area);
    }
}

/// The non-rotated bitmap branch of `draw_letter_cb()`.
fn draw_letter_bitmap_impl(
    handlers: &[CustomBlendHandler],
    ctx: &mut DrawCtx,
    t: &DrawTask,
    glyph_draw_dsc: &DrawGlyphDsc,
) {
    let mut mask_area = glyph_draw_dsc.letter_coords;
    let g = &glyph_draw_dsc.g;

    let static_font = glyph_draw_dsc
        .resolved_font
        .as_ref()
        .filter(|f| f.as_font().has_static_bitmap());
    if let Some(font) = static_font
        && g.format == GlyphFormat::A8
    {
        /*req_raw_bitmap = 1*/
        let bitmap = font.as_font().glyph_static_bitmap(g);
        let mut blend_dsc = blend_dsc_zeroed(glyph_draw_dsc.letter_coords);
        blend_dsc.color = glyph_draw_dsc.color;
        blend_dsc.opa = glyph_draw_dsc.opa;
        blend_dsc.mask_buf = bitmap;
        blend_dsc.mask_area = Some(mask_area);
        blend_dsc.mask_stride = i32::from(g.stride);
        blend_dsc.mask_res = MaskRes::Changed;
        sw_blend(handlers, ctx, t, &blend_dsc);
    } else {
        /* `glyph_data = lv_font_get_glyph_bitmap(g, _draw_buf)`: done by the core */
        let draw_buf = match (&glyph_draw_dsc.glyph_data, glyph_draw_dsc.draw_buf.as_ref()) {
            (GlyphData::ScratchBuf, Some(b)) => b,
            _ => {
                /*LV_LOG_WARN("Couldn't get the bitmap of a glyph")*/
                return;
            }
        };

        mask_area.x2 =
            mask_area.x1 + DrawBuf::width_to_stride(mask_area.width() as u32, ColorFormat::A8) as i32 - 1;
        let mut blend_dsc = blend_dsc_zeroed(glyph_draw_dsc.letter_coords);
        blend_dsc.color = glyph_draw_dsc.color;
        blend_dsc.opa = glyph_draw_dsc.opa;
        blend_dsc.mask_buf = Some(draw_buf.data());
        blend_dsc.mask_area = Some(mask_area);
        blend_dsc.mask_stride = i32::from(draw_buf.header.stride);
        blend_dsc.mask_res = MaskRes::Changed;
        sw_blend(handlers, ctx, t, &blend_dsc);
    }
}

/// The rotated / `LV_FONT_GLYPH_FORMAT_IMAGE` branch of `draw_letter_cb()`:
/// draw the glyph as an image with `lv_draw_sw_image()`.
fn draw_letter_image(
    unit: &mut SwDrawUnit,
    ctx: &mut DrawCtx,
    t: &DrawTask,
    glyph_draw_dsc: &mut DrawGlyphDsc,
) {
    /*glyph_data = lv_font_get_glyph_bitmap_internal(g, _draw_buf)*/
    /* Known gap: for `LV_FONT_GLYPH_FORMAT_IMAGE` upstream gets the image
     * source here from the font (imgfont). `Font` has no such API and the
     * core never sets `GlyphData::Image`, so such glyphs draw nothing; see
     * the module docs. */
    if glyph_draw_dsc.glyph_data == GlyphData::StaticBitmap {
        /* Without `req_raw_bitmap` the bitmap is rendered into the draw buffer */
        let g = glyph_draw_dsc.g;
        if let (Some(font), Some(buf)) = (
            glyph_draw_dsc.resolved_font.as_ref(),
            glyph_draw_dsc.draw_buf.as_mut(),
        ) {
            let stride = usize::from(buf.header.stride);
            if let Some(data) = buf.data_mut()
                && font.as_font().glyph_bitmap_a8(&g, data, stride)
            {
                glyph_draw_dsc.glyph_data = GlyphData::ScratchBuf;
            } else {
                glyph_draw_dsc.glyph_data = GlyphData::None;
            }
        } else {
            glyph_draw_dsc.glyph_data = GlyphData::None;
        }
    }
    let src = match &glyph_draw_dsc.glyph_data {
        GlyphData::Image(src) => src.clone(),
        GlyphData::ScratchBuf => match glyph_draw_dsc.draw_buf.take() {
            Some(buf) => ImageSrc::DrawBuf(Rc::new(buf)),
            None => ImageSrc::None,
        },
        /*`GlyphData::None`, `GlyphData::StaticBitmap`*/
        _ => ImageSrc::None,
    };

    let mut img_dsc = DrawImageDsc::new();
    img_dsc.rotation = glyph_draw_dsc.rotation;
    img_dsc.scale_x = SCALE_NONE;
    img_dsc.scale_y = SCALE_NONE;
    img_dsc.opa = glyph_draw_dsc.opa;
    img_dsc.src = src;
    img_dsc.recolor = glyph_draw_dsc.color;
    img_dsc.pivot = Point::new(
        glyph_draw_dsc.pivot.x - i32::from(glyph_draw_dsc.g.ofs_x),
        i32::from(glyph_draw_dsc.g.box_h) + i32::from(glyph_draw_dsc.g.ofs_y),
    );
    crate::img::image(unit, ctx, t, &img_dsc, &glyph_draw_dsc.letter_coords);

    /* Hand the scratch buffer back so that it is reused for the next glyph */
    if let ImageSrc::DrawBuf(rc) = core::mem::take(&mut img_dsc.src)
        && glyph_draw_dsc.glyph_data == GlyphData::ScratchBuf
        && let Ok(buf) = Rc::try_unwrap(rc)
    {
        glyph_draw_dsc.draw_buf = Some(buf);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::fill::tests::{WHITE, ctx_with_layer, px};
    use alloc::vec;
    use alloc::vec::Vec;
    use ferrovgl_core::draw::DrawTaskType;
    use ferrovgl_core::draw::LayerId;
    use ferrovgl_core::font::{Font, FontRef, GlyphDsc};
    use ferrovgl_core::misc::color::Color;

    /// Line height 10, base line 2. `'A'` is a 3 x 4 A4 glyph rendered as a
    /// full block (A8 `0xff`) except a transparent first pixel, space is
    /// empty, everything else is missing (placeholder). With `static_a8`
    /// the glyphs are static A8 bitmaps.
    pub(crate) struct TestFont {
        pub(crate) static_a8: bool,
        pub(crate) bitmap: Vec<u8>,
    }

    impl TestFont {
        pub(crate) fn new(static_a8: bool) -> Self {
            let mut bitmap = vec![0xff; 12];
            bitmap[0] = 0;
            Self { static_a8, bitmap }
        }
    }

    impl Font for TestFont {
        fn line_height(&self) -> i32 {
            10
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
        fn has_static_bitmap(&self) -> bool {
            self.static_a8
        }
        fn glyph_dsc(&self, out: &mut GlyphDsc, letter: u32, _letter_next: u32) -> bool {
            let empty = letter == u32::from(' ');
            if !empty && letter != u32::from('A') {
                return false;
            }
            let mut dsc = GlyphDsc::default();
            dsc.adv_w = 4;
            dsc.box_w = if empty { 0 } else { 3 };
            dsc.box_h = if empty { 0 } else { 4 };
            dsc.ofs_x = 0;
            dsc.ofs_y = 0;
            dsc.format = if self.static_a8 {
                GlyphFormat::A8
            } else {
                GlyphFormat::A4
            };
            dsc.stride = if self.static_a8 { 3 } else { 0 };
            *out = dsc;
            true
        }
        fn glyph_bitmap_a8(&self, g: &GlyphDsc, out: &mut [u8], stride: usize) -> bool {
            for y in 0..g.box_h as usize {
                for x in 0..g.box_w as usize {
                    out[y * stride + x] = self.bitmap[y * 3 + x];
                }
            }
            true
        }
        fn glyph_static_bitmap(&self, _g: &GlyphDsc) -> Option<&[u8]> {
            self.static_a8.then_some(&self.bitmap[..])
        }
    }

    pub(crate) fn font(static_a8: bool) -> FontRef {
        FontRef::Shared(Rc::new(TestFont::new(static_a8)))
    }

    pub(crate) const BLUE: [u8; 3] = [0xff, 0, 0];

    /// Add a letter task for `unicode` at (10, 10) and render it.
    fn run_letter(unicode: u32, static_a8: bool) -> (DrawCtx, LayerId) {
        let (mut ctx, layer) = ctx_with_layer();
        let mut dsc = LetterDsc::new();
        dsc.font = Some(font(static_a8));
        dsc.color = Color::make(0, 0, 0xff);
        dsc.unicode = unicode;
        let coords = Area::new(10, 10, 13, 19);
        let tref = ctx.add_task(layer, &coords, DrawTaskType::Letter);
        let t = {
            let task = ctx.task_mut(tref).unwrap();
            task.dsc = DrawDsc::Letter(alloc::boxed::Box::new(dsc.clone()));
            task.state = DrawTaskState::InProgress;
            task.clone()
        };
        let mut unit = SwDrawUnit::new(0);
        unit.task_act = Some(tref);
        letter(&mut unit, &mut ctx, &t, &dsc, &coords);
        (ctx, layer)
    }

    /// The glyph box: y1 = 10 + (10 - 2) - 4 = 14.
    fn check_glyph(ctx: &DrawCtx, layer: LayerId) {
        assert_eq!(px(ctx, layer, 10, 14), WHITE); // transparent bitmap pixel
        for (x, y) in [(11, 14), (12, 14), (10, 15), (12, 17)] {
            assert_eq!(px(ctx, layer, x, y), BLUE, "at {x},{y}");
        }
        assert_eq!(px(ctx, layer, 13, 15), WHITE);
        assert_eq!(px(ctx, layer, 11, 13), WHITE);
        assert_eq!(px(ctx, layer, 11, 18), WHITE);
    }

    #[test]
    fn letter_blends_the_scratch_buffer() {
        let (ctx, layer) = run_letter(u32::from('A'), false);
        check_glyph(&ctx, layer);
    }

    #[test]
    fn letter_blends_a_static_a8_bitmap() {
        let (ctx, layer) = run_letter(u32::from('A'), true);
        check_glyph(&ctx, layer);
    }

    #[test]
    fn letter_without_bg_coords_draws_no_placeholder() {
        let (ctx, layer) = run_letter(u32::from('~'), false);
        for y in 0..20 {
            for x in 0..20 {
                assert_eq!(px(&ctx, layer, x, y), WHITE);
            }
        }
    }

    #[test]
    fn fill_request_fills_and_ignores_the_glyph() {
        let (mut ctx, layer) = ctx_with_layer();
        let coords = Area::new(0, 0, 30, 30);
        let tref = ctx.add_task(layer, &coords, DrawTaskType::Label);
        let mut g = DrawGlyphDsc::new();
        g.format = GlyphFormat::None;
        g.bg_coords = Some(Area::new(0, 0, 5, 5));
        g.opa = 255;
        let mut f = FillDsc::new();
        f.color = Color::make(0, 0, 0xff);
        draw_letter_cb(&mut ctx, tref, &mut g, Some((&f, &Area::new(8, 8, 9, 9))));
        assert_eq!(px(&ctx, layer, 8, 8), BLUE);
        assert_eq!(px(&ctx, layer, 9, 9), BLUE);
        assert_eq!(px(&ctx, layer, 0, 0), WHITE); // no placeholder
        // Without the fill the placeholder border is drawn on `bg_coords`.
        draw_letter_cb(&mut ctx, tref, &mut g, None);
        assert_eq!(px(&ctx, layer, 0, 0), [0, 0, 0]);
        assert_eq!(px(&ctx, layer, 5, 3), [0, 0, 0]);
        assert_eq!(px(&ctx, layer, 2, 2), WHITE);
    }

    /// `LV_FONT_GLYPH_FORMAT_VECTOR` has its own case upstream
    /// (`draw_letter_outline`, not ported): it never falls into the `NONE`
    /// placeholder branch, so nothing is drawn even with `bg_coords` set.
    #[test]
    fn vector_glyph_draws_no_placeholder() {
        let (mut ctx, layer) = ctx_with_layer();
        let coords = Area::new(0, 0, 30, 30);
        let tref = ctx.add_task(layer, &coords, DrawTaskType::Label);
        let mut g = DrawGlyphDsc::new();
        g.format = GlyphFormat::Vector;
        g.bg_coords = Some(Area::new(0, 0, 5, 5));
        g.letter_coords = Area::new(0, 0, 5, 5);
        g.opa = 255;
        draw_letter_cb(&mut ctx, tref, &mut g, None);
        for y in 0..10 {
            for x in 0..10 {
                assert_eq!(px(&ctx, layer, x, y), WHITE);
            }
        }
    }

    #[test]
    fn task_ref_falls_back_to_the_in_progress_task() {
        let (mut ctx, layer) = ctx_with_layer();
        let a = Area::new(0, 0, 9, 9);
        let _ = ctx.add_task(layer, &a, DrawTaskType::Fill);
        let r = ctx.add_task(layer, &a, DrawTaskType::Label);
        ctx.task_mut(r).unwrap().state = DrawTaskState::InProgress;
        let t = task_header(ctx.task(r).unwrap());
        let unit = SwDrawUnit::new(0);
        assert_eq!(task_ref(&unit, &ctx, &t), Some(r));
    }
}
