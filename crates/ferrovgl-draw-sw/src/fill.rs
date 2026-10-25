//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_fill.c`.
//!
//! Rounded rectangle fill with optional simple (vertical / horizontal) or
//! complex (linear / radial / conical) gradient.
//!
//! The `lv_draw_sw_blend_dsc_t` of upstream holds pointers into the mask
//! buffer and the gradient maps that are rewritten between blends; here the
//! descriptor state lives in plain locals and a [`BlendDsc`] borrowing the
//! current buffers is built right before every `lv_draw_sw_blend()` call.
//! The gradient colour map (`lv_color_t`, 3 bytes `B, G, R` in memory) is
//! handed to the blender as `RGB888` bytes, converted with
//! `colors_to_rgb888`.

#[cfg(feature = "complex")]
use alloc::vec;
#[cfg(any(feature = "complex", test))]
use alloc::vec::Vec;

use ferrovgl_core::draw::grad::GradDir;
use ferrovgl_core::draw::rect::FillDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;
use ferrovgl_core::misc::color::{Color, ColorFormat, OPA_COVER, OPA_MIN};
use ferrovgl_core::misc::style::BlendMode;

#[cfg(feature = "complex")]
use ferrovgl_core::misc::color::{OPA_MAX, Opa, opa_mix2};

use crate::SwDrawUnit;
use crate::blend::{self, BlendDsc, CustomBlendHandler, MaskRes};
#[cfg(feature = "complex")]
use crate::grad;
#[cfg(feature = "complex")]
use crate::mask::{self, MaskParam, MaskRadiusParam};

/// `lv_draw_sw_blend(t, dsc)`: blend into `t.target_layer` of `ctx`.
///
/// Shared by the `fill`, `border` and `letter` renderers of this crate.
/// `handlers` are the custom blend handlers of the unit
/// (`SwDrawUnit::blend_handlers`).
pub(crate) fn sw_blend(handlers: &[CustomBlendHandler], ctx: &mut DrawCtx, t: &DrawTask, dsc: &BlendDsc<'_>) {
    if let Some(layer) = ctx.layer_mut(t.target_layer) {
        blend::blend(t, layer, dsc, handlers);
    }
}

/// A `lv_memzero`ed `lv_draw_sw_blend_dsc_t` blending `blend_area`.
pub(crate) fn blend_dsc_zeroed<'a>(blend_area: Area) -> BlendDsc<'a> {
    BlendDsc {
        blend_area,
        src_buf: None,
        src_stride: 0,
        src_color_format: ColorFormat::Unknown,
        src_area: None,
        opa: 0,
        color: Color::make(0, 0, 0),
        mask_buf: None,
        mask_res: MaskRes::Transp,
        mask_area: None,
        mask_stride: 0,
        blend_mode: BlendMode::Normal,
    }
}

/// The bytes of an `lv_color_t` array (`B, G, R` per entry), i.e. the
/// array read as `LV_COLOR_FORMAT_RGB888` pixels.
#[cfg(any(feature = "complex", test))]
pub(crate) fn colors_to_rgb888(colors: &[Color], out: &mut Vec<u8>) {
    out.clear();
    out.reserve(colors.len() * 3);
    for c in colors {
        out.push(c.blue);
        out.push(c.green);
        out.push(c.red);
    }
}

/// `lv_draw_sw_fill()`: rounded rectangle fill with optional gradient.
pub fn fill(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &FillDsc, coords: &Area) {
    if dsc.opa <= OPA_MIN {
        return;
    }

    let bg_coords = *coords;

    let Some(clipped_coords) = Area::intersect(&bg_coords, &t.clip_area) else {
        return;
    };

    let grad_dir = dsc.grad.dir;
    let bg_color = if grad_dir == GradDir::None {
        dsc.color
    } else {
        dsc.grad.stops[0].color
    };

    /*Most simple case: just a plain rectangle*/
    if dsc.radius == 0 && grad_dir == GradDir::None {
        let mut blend_dsc = blend_dsc_zeroed(bg_coords);
        blend_dsc.color = bg_color;
        blend_dsc.opa = dsc.opa;
        sw_blend(&unit.blend_handlers, ctx, t, &blend_dsc);
        return;
    }

    /*Complex case: there is gradient, mask, or radius*/
    #[cfg(not(feature = "complex"))]
    {
        /*LV_LOG_WARN("Can't draw complex rectangle because LV_DRAW_SW_COMPLEX = 0")*/
        let _ = (clipped_coords, OPA_COVER);
    }
    #[cfg(feature = "complex")]
    fill_complex(unit, ctx, t, dsc, coords, bg_coords, clipped_coords, bg_color);
}

/// Which buffer `blend_dsc.mask_buf` points to.
#[cfg(feature = "complex")]
#[derive(Copy, Clone, PartialEq, Eq)]
enum MaskSel {
    /// `NULL`
    None,
    /// The radius mask line buffer (`mask_buf`, `NULL` without radius).
    MaskBuf,
    /// `grad_opa_map` (`NULL` when it is not set).
    GradOpa,
}

/// The per-draw state of a complex gradient (`dsc->grad.state`).
#[cfg(feature = "complex-gradients")]
enum ComplexState {
    Linear(grad::GradLinearState),
    Radial(grad::GradRadialState),
    Conical(grad::GradConicalState),
}

/// `lv_draw_sw_grad_*_get_line()` for the gradient type of `state`.
#[cfg(feature = "complex-gradients")]
fn complex_get_line(
    dsc: &FillDsc,
    state: &Option<ComplexState>,
    xp: i32,
    yp: i32,
    width: i32,
    result: &mut grad::GradCalc,
) {
    match state {
        Some(ComplexState::Linear(s)) => grad::linear_get_line(&dsc.grad, s, xp, yp, width, result),
        Some(ComplexState::Radial(s)) => grad::radial_get_line(&dsc.grad, s, xp, yp, width, result),
        Some(ComplexState::Conical(s)) => grad::conical_get_line(&dsc.grad, s, xp, yp, width, result),
        None => {}
    }
}

/// The `LV_DRAW_SW_COMPLEX` part of `lv_draw_sw_fill()`.
#[cfg(feature = "complex")]
#[allow(clippy::too_many_arguments)] // the locals of the single upstream function
fn fill_complex(
    unit: &mut SwDrawUnit,
    ctx: &mut DrawCtx,
    t: &DrawTask,
    dsc: &FillDsc,
    coords: &Area,
    bg_coords: Area,
    clipped_coords: Area,
    bg_color: Color,
) {
    let SwDrawUnit {
        circle_cache,
        blend_handlers,
        ..
    } = unit;
    let handlers: &[CustomBlendHandler] = blend_handlers;

    let grad_dir = dsc.grad.dir;
    let opa: Opa = if dsc.opa >= OPA_MAX { OPA_COVER } else { dsc.opa };

    /*Get the real radius. Can't be larger than the half of the shortest side */
    let coords_bg_w = bg_coords.width();
    let coords_bg_h = bg_coords.height();
    let short_side = coords_bg_w.min(coords_bg_h);
    let rout = dsc.radius.min(short_side >> 1);

    /*Add a radius mask if there is a radius*/
    let clipped_w = clipped_coords.width();
    let mut mask_buf: Vec<Opa> = Vec::new();
    let has_mask_buf = rout > 0;
    let mut mask_list: Vec<MaskParam<'static>> = Vec::new();
    if rout > 0 {
        mask_buf = vec![0; clipped_w as usize];
        let mut mask_rout_param = MaskRadiusParam::default();
        mask::radius_init(circle_cache, &mut mask_rout_param, &bg_coords, rout, false);
        mask_list.push(MaskParam::Radius(mask_rout_param));
    }

    let mut blend_area = Area::new(clipped_coords.x1, 0, clipped_coords.x2, 0);

    /* `blend_dsc` fields (`blend_area` / `mask_area` are `&blend_area`) */
    let mut b_color = bg_color;
    let mut b_opa: Opa = OPA_COVER;
    let mut b_mask_res = MaskRes::Transp;
    let mut b_mask_sel = MaskSel::MaskBuf;
    /* `src_buf` as an offset into the colour map, `None` for `NULL` */
    let mut b_src_ofs: Option<usize> = None;

    /*Get gradient if appropriate*/
    #[cfg_attr(not(feature = "complex-gradients"), allow(unused_mut))] // `get_line` writes it
    let mut grad = grad::get(&dsc.grad, coords_bg_w, coords_bg_h);
    let has_grad = grad_dir != GradDir::None; /* `grad != NULL` */
    /* `grad_opa_map` as an offset into `grad.opa_map`, `None` for `NULL` */
    let mut grad_opa_ofs: Option<usize> = None;
    let mut transp = false;
    if has_grad && grad_dir as u8 >= GradDir::Hor as u8 {
        b_src_ofs = Some((clipped_coords.x1 - bg_coords.x1) as usize);
        for s in 0..dsc.grad.stops_count as usize {
            if dsc.grad.stops[s].opa != OPA_COVER {
                transp = true;
                break;
            }
        }
        if grad_dir == GradDir::Hor && transp {
            grad_opa_ofs = Some((clipped_coords.x1 - bg_coords.x1) as usize);
        }
    }

    #[cfg(feature = "complex-gradients")]
    let mut complex_state: Option<ComplexState> = None;

    /*Prepare complex gradient*/
    #[cfg(feature = "complex-gradients")]
    if grad_dir as u8 >= GradDir::Linear as u8 {
        complex_state = Some(match grad_dir {
            GradDir::Linear => ComplexState::Linear(grad::linear_setup(&dsc.grad, coords)),
            GradDir::Radial => ComplexState::Radial(grad::radial_setup(&dsc.grad, coords)),
            GradDir::Conical => ComplexState::Conical(grad::conical_setup(&dsc.grad, coords)),
            _ => {
                /*LV_LOG_WARN("Gradient type is not supported")*/
                for p in mask_list.iter_mut() {
                    mask::free_param(circle_cache, p);
                }
                return;
            }
        });
        /* For complex gradients we reuse the color map buffer for the pixel data */
        b_src_ofs = Some(0);
        grad_opa_ofs = Some(0);
    }
    #[cfg(not(feature = "complex-gradients"))]
    let _ = coords;

    /* `src_buf` bytes: the colour map as RGB888 */
    let mut src_bytes: Vec<u8> = Vec::new();
    if b_src_ofs.is_some() {
        colors_to_rgb888(&grad.color_map, &mut src_bytes);
    }

    macro_rules! do_blend {
        () => {{
            let mask: Option<&[u8]> = match b_mask_sel {
                MaskSel::None => None,
                MaskSel::MaskBuf => has_mask_buf.then_some(&mask_buf[..]),
                MaskSel::GradOpa => grad_opa_ofs.and_then(|o| grad.opa_map.get(o..)),
            };
            let src: Option<&[u8]> = b_src_ofs.and_then(|o| src_bytes.get(o * 3..));
            let mut blend_dsc = blend_dsc_zeroed(blend_area);
            blend_dsc.color = b_color;
            blend_dsc.opa = b_opa;
            blend_dsc.mask_buf = mask;
            blend_dsc.mask_res = b_mask_res;
            blend_dsc.mask_area = Some(blend_area);
            if src.is_some() {
                blend_dsc.src_buf = src;
                blend_dsc.src_area = Some(blend_area);
                blend_dsc.src_color_format = ColorFormat::Rgb888;
            }
            sw_blend(handlers, ctx, t, &blend_dsc);
        }};
    }

    /* Refresh `src_bytes` after a `get_line` (only the first `clipped_w`
     * entries are read by the blender). */
    #[cfg(feature = "complex-gradients")]
    macro_rules! refresh_src {
        () => {{
            let n = (clipped_w as usize).min(grad.color_map.len());
            colors_to_rgb888(&grad.color_map[..n], &mut src_bytes);
        }};
    }

    /* `mask_buf[i] = (mask_buf[i] * grad_opa_map[i]) >> 8` */
    macro_rules! preblend_mask {
        () => {{
            if let Some(o) = grad_opa_ofs {
                let gom = &grad.opa_map[o..];
                for i in 0..clipped_w as usize {
                    if gom[i] < OPA_MAX {
                        mask_buf[i] = ((mask_buf[i] as u32 * gom[i] as u32) >> 8) as Opa;
                    }
                }
            }
        }};
    }

    /* Draw the top of the rectangle line by line and mirror it to the bottom. */
    for h in 0..rout {
        let top_y = bg_coords.y1 + h;
        let bottom_y = bg_coords.y2 - h;
        if top_y < clipped_coords.y1 && bottom_y > clipped_coords.y2 {
            continue; /*This line is clipped now*/
        }

        let mut preblend = false;

        /* Initialize the mask to opa instead of 0xFF and blend with LV_OPA_COVER.
         * It saves calculating the final opa in lv_draw_sw_blend*/
        mask_buf.fill(opa);
        b_mask_res = mask::apply(
            circle_cache,
            &mut mask_list,
            &mut mask_buf,
            blend_area.x1,
            top_y,
            clipped_w,
        );
        if b_mask_res == MaskRes::FullCover {
            b_mask_res = MaskRes::Changed;
        }

        let mut hor_grad_processed = false;
        if top_y >= clipped_coords.y1 {
            blend_area.y1 = top_y;
            blend_area.y2 = top_y;

            match grad_dir {
                GradDir::Ver => {
                    b_color = grad.color_map[(top_y - bg_coords.y1) as usize];
                    b_opa = grad.opa_map[(top_y - bg_coords.y1) as usize];
                }
                GradDir::Hor => {
                    hor_grad_processed = true;
                    preblend = grad_opa_ofs.is_some();
                }
                #[cfg(feature = "complex-gradients")]
                GradDir::Linear | GradDir::Radial | GradDir::Conical => {
                    complex_get_line(
                        dsc,
                        &complex_state,
                        clipped_coords.x1 - bg_coords.x1,
                        top_y - bg_coords.y1,
                        coords_bg_w,
                        &mut grad,
                    );
                    refresh_src!();
                    preblend = true;
                }
                _ => {}
            }
            /* pre-blend the mask */
            if preblend {
                preblend_mask!();
                b_mask_res = MaskRes::Changed;
            }
            do_blend!();
        }

        if bottom_y <= clipped_coords.y2 {
            blend_area.y1 = bottom_y;
            blend_area.y2 = bottom_y;

            match grad_dir {
                GradDir::Ver => {
                    b_color = grad.color_map[(bottom_y - bg_coords.y1) as usize];
                    b_opa = grad.opa_map[(bottom_y - bg_coords.y1) as usize];
                }
                GradDir::Hor => {
                    preblend = !hor_grad_processed && grad_opa_ofs.is_some();
                }
                #[cfg(feature = "complex-gradients")]
                GradDir::Linear | GradDir::Radial | GradDir::Conical => {
                    complex_get_line(
                        dsc,
                        &complex_state,
                        clipped_coords.x1 - bg_coords.x1,
                        bottom_y - bg_coords.y1,
                        coords_bg_w,
                        &mut grad,
                    );
                    refresh_src!();
                    preblend = true;
                }
                _ => {}
            }
            /* pre-blend the mask */
            if preblend {
                if grad_dir as u8 >= GradDir::Linear as u8 {
                    /*Need to generate the mask again, because we have mixed in the upper part of the gradient*/
                    mask_buf.fill(opa);
                    /* Upstream stores the result (FULL_COVER -> CHANGED) in
                     * `blend_dsc.mask_res`, which is overwritten right below. */
                    let _ = mask::apply(
                        circle_cache,
                        &mut mask_list,
                        &mut mask_buf,
                        blend_area.x1,
                        top_y,
                        clipped_w,
                    );
                }
                preblend_mask!();
                b_mask_res = MaskRes::Changed;
            }
            do_blend!();
        }
    }

    /* Draw the center of the rectangle.*/

    /*If no gradient, the center is a simple rectangle*/
    if grad_dir == GradDir::None {
        blend_area.y1 = bg_coords.y1 + rout;
        blend_area.y2 = bg_coords.y2 - rout;
        b_opa = opa;
        b_mask_sel = MaskSel::None;
        do_blend!();
    }
    /*With gradient draw line by line*/
    else {
        b_opa = opa;
        match grad_dir {
            GradDir::Ver => {
                b_mask_res = MaskRes::FullCover;
            }
            GradDir::Hor => {
                b_mask_res = MaskRes::Changed;
                b_mask_sel = MaskSel::GradOpa;
            }
            GradDir::Linear | GradDir::Radial | GradDir::Conical => {
                b_mask_res = if transp {
                    MaskRes::Changed
                } else {
                    MaskRes::FullCover
                };
                b_mask_sel = MaskSel::GradOpa;
            }
            _ => {}
        }

        let h_start = (bg_coords.y1 + rout).max(clipped_coords.y1);
        let h_end = (bg_coords.y2 - rout).min(clipped_coords.y2);
        for h in h_start..=h_end {
            blend_area.y1 = h;
            blend_area.y2 = h;

            match grad_dir {
                GradDir::Ver => {
                    b_color = grad.color_map[(h - bg_coords.y1) as usize];
                    if opa >= OPA_MAX {
                        b_opa = grad.opa_map[(h - bg_coords.y1) as usize];
                    } else {
                        b_opa = opa_mix2(grad.opa_map[(h - bg_coords.y1) as usize], opa);
                    }
                }
                #[cfg(feature = "complex-gradients")]
                GradDir::Linear | GradDir::Radial | GradDir::Conical => {
                    complex_get_line(
                        dsc,
                        &complex_state,
                        clipped_coords.x1 - bg_coords.x1,
                        h - bg_coords.y1,
                        coords_bg_w,
                        &mut grad,
                    );
                    refresh_src!();
                }
                _ => {}
            }
            do_blend!();
        }
    }

    if has_mask_buf {
        for p in mask_list.iter_mut() {
            mask::free_param(circle_cache, p);
        }
    }
    if has_grad {
        grad::cleanup(grad);
    }
    #[cfg(feature = "complex-gradients")]
    match complex_state {
        Some(ComplexState::Linear(s)) => grad::linear_cleanup(s),
        Some(ComplexState::Radial(s)) => grad::radial_cleanup(s),
        Some(ComplexState::Conical(s)) => grad::conical_cleanup(s),
        None => {}
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ferrovgl_core::draw::{DrawDsc, DrawTaskState, DrawTaskType, LayerId};

    pub(crate) const W: i32 = 64;
    pub(crate) const H: i32 = 48;

    /// A white XRGB8888 layer of `W` x `H` at (0, 0).
    pub(crate) fn ctx_with_layer() -> (DrawCtx, LayerId) {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Xrgb8888, &Area::new(0, 0, W - 1, H - 1));
        assert!(ctx.layer_alloc_buf(layer));
        let buf = ctx.layer_mut(layer).unwrap().draw_buf.as_mut().unwrap();
        buf.data_mut().unwrap().fill(0xff);
        (ctx, layer)
    }

    pub(crate) fn task(layer: LayerId, clip_area: Area) -> DrawTask {
        DrawTask {
            ty: DrawTaskType::Fill,
            area: clip_area,
            real_area: clip_area,
            clip_area,
            target_layer: layer,
            draw_unit: None,
            state: DrawTaskState::InProgress,
            dsc: DrawDsc::None,
            opa: OPA_COVER,
            preferred_draw_unit_id: 0,
            preference_score: 0,
        }
    }

    /// `[b, g, r]` of the pixel at `(x, y)`.
    pub(crate) fn px(ctx: &DrawCtx, layer: LayerId, x: i32, y: i32) -> [u8; 3] {
        let buf = ctx.layer(layer).unwrap().draw_buf.as_ref().unwrap();
        let stride = buf.header.stride as usize;
        let i = y as usize * stride + x as usize * 4;
        let d = buf.data();
        [d[i], d[i + 1], d[i + 2]]
    }

    pub(crate) const WHITE: [u8; 3] = [0xff, 0xff, 0xff];

    fn full_clip() -> Area {
        Area::new(0, 0, W - 1, H - 1)
    }

    fn run(dsc: &FillDsc, coords: Area, clip: Area) -> (DrawCtx, LayerId) {
        let (mut ctx, layer) = ctx_with_layer();
        let mut unit = SwDrawUnit::new(0);
        let t = task(layer, clip);
        fill(&mut unit, &mut ctx, &t, dsc, &coords);
        // Every cached circle was released.
        assert!(unit.circle_cache.entries.iter().all(|e| e.used_cnt == 0));
        (ctx, layer)
    }

    #[test]
    fn colors_to_rgb888_is_bgr_in_memory() {
        let mut out = Vec::new();
        colors_to_rgb888(&[Color::make(1, 2, 3), Color::make(4, 5, 6)], &mut out);
        assert_eq!(out, [3, 2, 1, 6, 5, 4]);
    }

    #[test]
    fn transparent_fill_draws_nothing() {
        let mut dsc = FillDsc::new();
        dsc.color = Color::make(0xff, 0, 0);
        dsc.opa = OPA_MIN;
        let (ctx, layer) = run(&dsc, Area::new(2, 2, 20, 20), full_clip());
        assert_eq!(px(&ctx, layer, 10, 10), WHITE);
    }

    #[test]
    fn plain_rect_is_clipped() {
        let mut dsc = FillDsc::new();
        dsc.color = Color::make(0xff, 0, 0);
        let (ctx, layer) = run(&dsc, Area::new(4, 4, 30, 20), Area::new(0, 0, 10, 10));
        assert_eq!(px(&ctx, layer, 4, 4), [0, 0, 0xff]);
        assert_eq!(px(&ctx, layer, 10, 10), [0, 0, 0xff]);
        assert_eq!(px(&ctx, layer, 11, 10), WHITE);
        assert_eq!(px(&ctx, layer, 10, 11), WHITE);
        assert_eq!(px(&ctx, layer, 3, 4), WHITE);
    }

    #[cfg(feature = "complex")]
    #[test]
    fn rounded_rect_is_mirrored_and_antialiased() {
        let mut dsc = FillDsc::new();
        dsc.color = Color::make(0, 0, 0xff);
        dsc.radius = 8;
        let c = Area::new(5, 5, 40, 30);
        let (ctx, layer) = run(&dsc, c, full_clip());
        // Outer corner untouched, middle of the edges covered.
        assert_eq!(px(&ctx, layer, c.x1, c.y1), WHITE);
        assert_eq!(px(&ctx, layer, c.x2, c.y2), WHITE);
        assert_eq!(px(&ctx, layer, 22, c.y1), [0xff, 0, 0]);
        assert_eq!(px(&ctx, layer, c.x1, 17), [0xff, 0, 0]);
        assert_eq!(px(&ctx, layer, 22, 17), [0xff, 0, 0]);
        // Some anti-aliased pixel exists on the corner arc.
        let mut aa = false;
        for h in 0..8 {
            for x in c.x1..c.x1 + 8 {
                let p = px(&ctx, layer, x, c.y1 + h);
                if p != WHITE && p != [0xff, 0, 0] {
                    aa = true;
                }
            }
        }
        assert!(aa);
        // Top rows are mirrored to the bottom and left to right.
        for h in 0..c.height() {
            for x in 0..c.width() {
                assert_eq!(
                    px(&ctx, layer, c.x1 + x, c.y1 + h),
                    px(&ctx, layer, c.x1 + x, c.y2 - h)
                );
                assert_eq!(
                    px(&ctx, layer, c.x1 + x, c.y1 + h),
                    px(&ctx, layer, c.x2 - x, c.y1 + h)
                );
            }
        }
        // Nothing outside the coordinates.
        assert_eq!(px(&ctx, layer, c.x1 - 1, 17), WHITE);
        assert_eq!(px(&ctx, layer, 22, c.y2 + 1), WHITE);
    }

    #[cfg(feature = "complex")]
    #[test]
    fn rounded_rect_clipped_matches_unclipped() {
        let mut dsc = FillDsc::new();
        dsc.color = Color::make(0x20, 0x80, 0x40);
        dsc.radius = 10;
        dsc.opa = 200;
        let c = Area::new(3, 3, 50, 40);
        let (full, lf) = run(&dsc, c, full_clip());
        let clip = Area::new(10, 0, 30, 20);
        let (part, lp) = run(&dsc, c, clip);
        for y in 0..H {
            for x in 0..W {
                let inside = x >= clip.x1 && x <= clip.x2 && y >= clip.y1 && y <= clip.y2;
                let expected = if inside { px(&full, lf, x, y) } else { WHITE };
                assert_eq!(px(&part, lp, x, y), expected, "at {x},{y}");
            }
        }
    }

    #[cfg(feature = "complex")]
    fn two_stops(dir: GradDir, c0: Color, c1: Color) -> FillDsc {
        use ferrovgl_core::draw::grad::GradStop;
        let mut dsc = FillDsc::new();
        dsc.grad.dir = dir;
        dsc.grad.stops_count = 2;
        dsc.grad.stops[0] = GradStop {
            color: c0,
            opa: OPA_COVER,
            frac: 0,
        };
        dsc.grad.stops[1] = GradStop {
            color: c1,
            opa: OPA_COVER,
            frac: 255,
        };
        dsc
    }

    #[cfg(feature = "complex")]
    #[test]
    fn vertical_gradient_goes_top_to_bottom() {
        let dsc = two_stops(GradDir::Ver, Color::make(0xff, 0, 0), Color::make(0, 0, 0xff));
        let c = Area::new(0, 0, 9, 39);
        let (ctx, layer) = run(&dsc, c, full_clip());
        assert_eq!(px(&ctx, layer, 5, 0), [0, 0, 0xff]);
        assert_eq!(px(&ctx, layer, 5, 39), [0xff, 0, 0]);
        let mut prev = 0xff;
        for y in 0..40 {
            let r = px(&ctx, layer, 5, y)[2];
            assert!(r <= prev);
            prev = r;
            // Rows are uniform.
            assert_eq!(px(&ctx, layer, 0, y), px(&ctx, layer, 9, y));
        }
    }

    #[cfg(feature = "complex")]
    #[test]
    fn horizontal_gradient_with_radius() {
        let dsc = {
            let mut d = two_stops(GradDir::Hor, Color::make(0xff, 0, 0), Color::make(0, 0xff, 0));
            d.radius = 5;
            d
        };
        let c = Area::new(0, 0, 49, 29);
        let (ctx, layer) = run(&dsc, c, full_clip());
        assert_eq!(px(&ctx, layer, 0, 15), [0, 0, 0xff]);
        assert_eq!(px(&ctx, layer, 49, 15), [0, 0xff, 0]);
        assert_eq!(px(&ctx, layer, 0, 0), WHITE);
        // Columns are uniform away from the corners.
        for y in 5..25 {
            assert_eq!(px(&ctx, layer, 20, y), px(&ctx, layer, 20, 15));
        }
        assert_eq!(px(&ctx, layer, 20, 0), px(&ctx, layer, 20, 15));
    }

    #[cfg(feature = "complex")]
    #[test]
    fn horizontal_gradient_with_transparent_stop() {
        let mut dsc = two_stops(GradDir::Hor, Color::make(0, 0, 0), Color::make(0, 0, 0));
        dsc.grad.stops[0].opa = 0;
        let c = Area::new(0, 0, 50, 9);
        let (ctx, layer) = run(&dsc, c, full_clip());
        assert_eq!(px(&ctx, layer, 0, 5), WHITE);
        assert_eq!(px(&ctx, layer, 50, 5), [0, 0, 0]);
        assert!(px(&ctx, layer, 25, 5)[0] > 0x40 && px(&ctx, layer, 25, 5)[0] < 0xc0);
    }

    #[cfg(feature = "complex-gradients")]
    #[test]
    fn linear_gradient_matches_horizontal_direction() {
        use ferrovgl_core::draw::grad::GradParams;
        use ferrovgl_core::misc::area::Point;
        let mut dsc = two_stops(GradDir::Linear, Color::make(0xff, 0, 0), Color::make(0, 0, 0xff));
        dsc.grad.params = GradParams::Linear {
            start: Point::new(0, 0),
            end: Point::new(63, 0),
        };
        dsc.radius = 6;
        let c = Area::new(0, 0, 63, 31);
        let (ctx, layer) = run(&dsc, c, full_clip());
        assert_eq!(px(&ctx, layer, 0, 16), [0, 0, 0xff]);
        assert_eq!(px(&ctx, layer, 63, 16), [0xff, 0, 0]);
        assert_eq!(px(&ctx, layer, 0, 0), WHITE);
        // Columns are uniform, including the rounded rows (mirrored mask).
        for y in 0..32 {
            assert_eq!(px(&ctx, layer, 31, y), px(&ctx, layer, 31, 16), "row {y}");
        }
        let mut prev = 0xff;
        for x in 0..64 {
            let r = px(&ctx, layer, x, 16)[2];
            assert!(r <= prev);
            prev = r;
        }
    }
}
