//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_border.c`.
//!
//! Borders with side selection: plain rectangles when there is no radius
//! (`draw_border_simple`), otherwise straight parts plus corners masked by
//! an inner (inverted) and an outer radius mask (`draw_border_complex`).

#[cfg(feature = "complex")]
use alloc::vec;
#[cfg(feature = "complex")]
use alloc::vec::Vec;

use ferrovgl_core::draw::rect::BorderDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;
use ferrovgl_core::misc::color::{Color, OPA_MIN, Opa};
use ferrovgl_core::misc::style::BorderSide;

use crate::SwDrawUnit;
use crate::fill::{blend_dsc_zeroed, sw_blend};
#[cfg(feature = "complex")]
use crate::mask::{self, MaskParam, MaskRadiusParam, MaskRes};

/// `SPLIT_LIMIT`: below this core width the left and right corners are
/// drawn together.
#[cfg(feature = "complex")]
const SPLIT_LIMIT: i32 = 50;

/// `lv_draw_sw_border()`
pub fn border(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &BorderDsc, coords: &Area) {
    if dsc.opa <= OPA_MIN {
        return;
    }
    if dsc.width == 0 {
        return;
    }
    if dsc.side == BorderSide::NONE {
        return;
    }

    let coords_w = coords.width();
    let coords_h = coords.height();
    let mut rout = dsc.radius;
    let short_side = coords_w.min(coords_h);
    if rout > short_side >> 1 {
        rout = short_side >> 1;
    }

    /*Get the inner area*/
    let mut area_inner = *coords;
    area_inner.x1 += if dsc.side.intersects(BorderSide::LEFT) {
        dsc.width
    } else {
        -(dsc.width + rout)
    };
    area_inner.x2 -= if dsc.side.intersects(BorderSide::RIGHT) {
        dsc.width
    } else {
        -(dsc.width + rout)
    };
    area_inner.y1 += if dsc.side.intersects(BorderSide::TOP) {
        dsc.width
    } else {
        -(dsc.width + rout)
    };
    area_inner.y2 -= if dsc.side.intersects(BorderSide::BOTTOM) {
        dsc.width
    } else {
        -(dsc.width + rout)
    };

    let mut rin = rout - dsc.width;
    if rin < 0 {
        rin = 0;
    }

    if rout == 0 && rin == 0 {
        draw_border_simple(unit, ctx, t, coords, &area_inner, dsc.color, dsc.opa);
    } else {
        draw_border_complex(unit, ctx, t, coords, &area_inner, rout, rin, dsc.color, dsc.opa);
    }
}

/*
 *   STATIC FUNCTIONS
 */

/// `draw_border_complex()`: a border with rounded corners. Does nothing
/// without the `complex` feature (`LV_DRAW_SW_COMPLEX == 0`).
#[allow(clippy::too_many_arguments)] // upstream signature (+ unit / ctx)
fn draw_border_complex(
    unit: &mut SwDrawUnit,
    ctx: &mut DrawCtx,
    t: &DrawTask,
    outer_area: &Area,
    inner_area: &Area,
    rout: i32,
    rin: i32,
    color: Color,
    opa: Opa,
) {
    #[cfg(feature = "complex")]
    {
        /*Get clipped draw area which is the real draw area.
         *It is always the same or inside `coords`*/
        let Some(draw_area) = Area::intersect(outer_area, &t.clip_area) else {
            return;
        };
        let draw_area_w = draw_area.width();

        let SwDrawUnit {
            circle_cache,
            blend_handlers,
            ..
        } = unit;
        let handlers: &[crate::blend::CustomBlendHandler] = blend_handlers;

        let mut mask_buf: Vec<Opa> = vec![0; draw_area_w as usize];

        let mut mask_list: Vec<MaskParam<'static>> = Vec::with_capacity(2);

        /*Create mask for the inner mask*/
        let mut mask_rin_param = MaskRadiusParam::default();
        mask::radius_init(circle_cache, &mut mask_rin_param, inner_area, rin, true);
        mask_list.push(MaskParam::Radius(mask_rin_param));

        /*Create mask for the outer area*/
        if rout > 0 {
            let mut mask_rout_param = MaskRadiusParam::default();
            mask::radius_init(circle_cache, &mut mask_rout_param, outer_area, rout, false);
            mask_list.push(MaskParam::Radius(mask_rout_param));
        }

        let mut blend_area = Area::new(0, 0, 0, 0);
        let mut mask_res;

        macro_rules! do_blend {
            () => {{
                let mut blend_dsc = blend_dsc_zeroed(blend_area);
                blend_dsc.mask_buf = Some(&mask_buf[..]);
                blend_dsc.mask_area = Some(blend_area);
                blend_dsc.color = color;
                blend_dsc.opa = opa;
                blend_dsc.mask_res = mask_res;
                sw_blend(handlers, ctx, t, &blend_dsc);
            }};
        }

        /*Calculate the x and y coordinates where the straight parts area is*/
        let core_area = Area::new(
            (outer_area.x1 + rout).max(inner_area.x1),
            (outer_area.y1 + rout).max(inner_area.y1),
            (outer_area.x2 - rout).min(inner_area.x2),
            (outer_area.y2 - rout).min(inner_area.y2),
        );
        let core_w = core_area.width();

        let top_side = outer_area.y1 <= inner_area.y1;
        let bottom_side = outer_area.y2 >= inner_area.y2;

        /*No masks*/
        let left_side = outer_area.x1 <= inner_area.x1;
        let right_side = outer_area.x2 >= inner_area.x2;

        let mut split_hor = true;
        if left_side && right_side && top_side && bottom_side && core_w < SPLIT_LIMIT {
            split_hor = false;
        }

        mask_res = MaskRes::FullCover;
        /*Draw the straight lines first if they are long enough*/
        if top_side && split_hor {
            blend_area.x1 = core_area.x1;
            blend_area.x2 = core_area.x2;
            blend_area.y1 = outer_area.y1;
            blend_area.y2 = inner_area.y1 - 1;
            do_blend!();
        }

        if bottom_side && split_hor {
            blend_area.x1 = core_area.x1;
            blend_area.x2 = core_area.x2;
            blend_area.y1 = inner_area.y2 + 1;
            blend_area.y2 = outer_area.y2;
            do_blend!();
        }

        /*If the border is very thick and the vertical sides overlap horizontally draw a single rectangle*/
        if inner_area.x1 >= inner_area.x2 && left_side && right_side {
            blend_area.x1 = outer_area.x1;
            blend_area.x2 = outer_area.x2;
            blend_area.y1 = core_area.y1;
            blend_area.y2 = core_area.y2;
            do_blend!();
        } else {
            if left_side {
                blend_area.x1 = outer_area.x1;
                blend_area.x2 = inner_area.x1 - 1;
                blend_area.y1 = core_area.y1;
                blend_area.y2 = core_area.y2;
                do_blend!();
            }

            if right_side {
                blend_area.x1 = inner_area.x2 + 1;
                blend_area.x2 = outer_area.x2;
                blend_area.y1 = core_area.y1;
                blend_area.y2 = core_area.y2;
                do_blend!();
            }
        }

        /*Draw the corners*/

        /*Left and right corner together if they are close to each other*/
        if !split_hor {
            /*Calculate the top corner and mirror it to the bottom*/
            blend_area.x1 = draw_area.x1;
            blend_area.x2 = draw_area.x2;
            let max_h = rout.max(inner_area.y1 - outer_area.y1);
            for h in 0..max_h {
                let top_y = outer_area.y1 + h;
                let bottom_y = outer_area.y2 - h;
                if top_y < draw_area.y1 && bottom_y > draw_area.y2 {
                    continue; /*This line is clipped now*/
                }

                mask_buf.fill(0xff);
                mask_res = mask::apply(
                    circle_cache,
                    &mut mask_list,
                    &mut mask_buf,
                    blend_area.x1,
                    top_y,
                    draw_area_w,
                );

                if top_y >= draw_area.y1 {
                    blend_area.y1 = top_y;
                    blend_area.y2 = top_y;
                    do_blend!();
                }

                if bottom_y <= draw_area.y2 {
                    blend_area.y1 = bottom_y;
                    blend_area.y2 = bottom_y;
                    do_blend!();
                }
            }
        } else {
            /*Left corners*/
            blend_area.x1 = draw_area.x1;
            blend_area.x2 = draw_area.x2.min(core_area.x1 - 1);
            let blend_w = blend_area.width();
            if blend_w > 0 {
                if left_side || top_side {
                    for h in draw_area.y1..core_area.y1 {
                        blend_area.y1 = h;
                        blend_area.y2 = h;

                        mask_buf[..blend_w as usize].fill(0xff);
                        mask_res = mask::apply(
                            circle_cache,
                            &mut mask_list,
                            &mut mask_buf,
                            blend_area.x1,
                            h,
                            blend_w,
                        );
                        do_blend!();
                    }
                }

                if left_side || bottom_side {
                    for h in core_area.y2 + 1..=draw_area.y2 {
                        blend_area.y1 = h;
                        blend_area.y2 = h;

                        mask_buf[..blend_w as usize].fill(0xff);
                        mask_res = mask::apply(
                            circle_cache,
                            &mut mask_list,
                            &mut mask_buf,
                            blend_area.x1,
                            h,
                            blend_w,
                        );
                        do_blend!();
                    }
                }
            }

            /*Right corners*/
            blend_area.x1 = draw_area.x1.max(blend_area.x2 + 1); /*To not overlap with the left side*/
            blend_area.x1 = draw_area.x1.max(core_area.x2 + 1);

            blend_area.x2 = draw_area.x2;
            let blend_w = blend_area.width();

            if blend_w > 0 {
                if right_side || top_side {
                    for h in draw_area.y1..core_area.y1 {
                        blend_area.y1 = h;
                        blend_area.y2 = h;

                        mask_buf[..blend_w as usize].fill(0xff);
                        mask_res = mask::apply(
                            circle_cache,
                            &mut mask_list,
                            &mut mask_buf,
                            blend_area.x1,
                            h,
                            blend_w,
                        );
                        do_blend!();
                    }
                }

                if right_side || bottom_side {
                    for h in core_area.y2 + 1..=draw_area.y2 {
                        blend_area.y1 = h;
                        blend_area.y2 = h;

                        mask_buf[..blend_w as usize].fill(0xff);
                        mask_res = mask::apply(
                            circle_cache,
                            &mut mask_list,
                            &mut mask_buf,
                            blend_area.x1,
                            h,
                            blend_w,
                        );
                        do_blend!();
                    }
                }
            }
        }

        for p in mask_list.iter_mut() {
            mask::free_param(circle_cache, p);
        }
    }
    #[cfg(not(feature = "complex"))]
    {
        let _ = (unit, ctx, t, outer_area, inner_area, rout, rin, color, opa);
    }
}

/// `draw_border_simple()`: up to four plain rectangles.
#[allow(clippy::too_many_arguments)] // upstream signature (+ unit / ctx)
fn draw_border_simple(
    unit: &mut SwDrawUnit,
    ctx: &mut DrawCtx,
    t: &DrawTask,
    outer_area: &Area,
    inner_area: &Area,
    color: Color,
    opa: Opa,
) {
    let mut a = Area::new(0, 0, 0, 0);

    macro_rules! do_blend {
        () => {{
            let mut blend_dsc = blend_dsc_zeroed(a);
            blend_dsc.color = color;
            blend_dsc.opa = opa;
            sw_blend(&unit.blend_handlers, ctx, t, &blend_dsc);
        }};
    }

    let top_side = outer_area.y1 <= inner_area.y1;
    let bottom_side = outer_area.y2 >= inner_area.y2;
    let left_side = outer_area.x1 <= inner_area.x1;
    let right_side = outer_area.x2 >= inner_area.x2;

    /*Top*/
    a.x1 = outer_area.x1;
    a.x2 = outer_area.x2;
    a.y1 = outer_area.y1;
    a.y2 = inner_area.y1 - 1;
    if top_side {
        do_blend!();
    }

    /*Bottom*/
    a.y1 = inner_area.y2 + 1;
    a.y2 = outer_area.y2;
    if bottom_side {
        do_blend!();
    }

    /*Left*/
    a.x1 = outer_area.x1;
    a.x2 = inner_area.x1 - 1;
    a.y1 = if top_side { inner_area.y1 } else { outer_area.y1 };
    a.y2 = if bottom_side { inner_area.y2 } else { outer_area.y2 };
    if left_side {
        do_blend!();
    }

    /*Right*/
    a.x1 = inner_area.x2 + 1;
    a.x2 = outer_area.x2;
    if right_side {
        do_blend!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fill::tests::{H, W, WHITE, ctx_with_layer, px, task};

    const RED: [u8; 3] = [0, 0, 0xff];

    fn run(dsc: &BorderDsc, coords: Area, clip: Area) -> (DrawCtx, ferrovgl_core::draw::LayerId) {
        let (mut ctx, layer) = ctx_with_layer();
        let mut unit = SwDrawUnit::new(0);
        let t = task(layer, clip);
        border(&mut unit, &mut ctx, &t, dsc, &coords);
        assert!(unit.circle_cache.entries.iter().all(|e| e.used_cnt == 0));
        (ctx, layer)
    }

    fn red(width: i32) -> BorderDsc {
        let mut d = BorderDsc::new();
        d.color = Color::make(0xff, 0, 0);
        d.width = width;
        d
    }

    fn full() -> Area {
        Area::new(0, 0, W - 1, H - 1)
    }

    #[test]
    fn nothing_for_zero_width_none_side_or_transparent() {
        let c = Area::new(2, 2, 20, 20);
        let mut d = red(0);
        let (ctx, l) = run(&d, c, full());
        assert_eq!(px(&ctx, l, 2, 2), WHITE);
        d.width = 2;
        d.side = BorderSide::NONE;
        let (ctx, l) = run(&d, c, full());
        assert_eq!(px(&ctx, l, 2, 2), WHITE);
        d.side = BorderSide::FULL;
        d.opa = OPA_MIN;
        let (ctx, l) = run(&d, c, full());
        assert_eq!(px(&ctx, l, 2, 2), WHITE);
    }

    #[test]
    fn simple_full_border_is_a_ring() {
        let c = Area::new(2, 3, 20, 15);
        let (ctx, l) = run(&red(2), c, full());
        for y in 0..H {
            for x in 0..W {
                let in_outer = x >= c.x1 && x <= c.x2 && y >= c.y1 && y <= c.y2;
                let in_inner = x >= c.x1 + 2 && x <= c.x2 - 2 && y >= c.y1 + 2 && y <= c.y2 - 2;
                let expected = if in_outer && !in_inner { RED } else { WHITE };
                assert_eq!(px(&ctx, l, x, y), expected, "at {x},{y}");
            }
        }
    }

    #[test]
    fn simple_border_side_selection() {
        let c = Area::new(2, 3, 20, 15);
        let mut d = red(3);
        d.side = BorderSide::TOP | BorderSide::LEFT;
        let (ctx, l) = run(&d, c, full());
        assert_eq!(px(&ctx, l, 20, 3), RED); // top right
        assert_eq!(px(&ctx, l, 20, 5), RED);
        assert_eq!(px(&ctx, l, 20, 6), WHITE);
        assert_eq!(px(&ctx, l, 2, 15), RED); // bottom left
        assert_eq!(px(&ctx, l, 4, 15), RED);
        assert_eq!(px(&ctx, l, 5, 15), WHITE);
        assert_eq!(px(&ctx, l, 19, 14), WHITE);
    }

    #[cfg(feature = "complex")]
    #[test]
    fn rounded_border_is_symmetric_and_hollow() {
        for (c, r, w) in [
            (Area::new(4, 4, 59, 43), 10, 3),  // split horizontally
            (Area::new(4, 4, 33, 43), 8, 4),   // corners together (core_w < SPLIT_LIMIT)
            (Area::new(10, 10, 21, 21), 6, 6), // thick
        ] {
            let mut d = red(w);
            d.radius = r;
            let (ctx, l) = run(&d, c, full());
            assert_eq!(px(&ctx, l, c.x1, c.y1), WHITE);
            assert_eq!(px(&ctx, l, (c.x1 + c.x2) / 2, c.y1), RED);
            assert_eq!(px(&ctx, l, c.x1, (c.y1 + c.y2) / 2), RED);
            if c.width() > 2 * w + 2 {
                assert_eq!(px(&ctx, l, (c.x1 + c.x2) / 2, (c.y1 + c.y2) / 2), WHITE);
            }
            for h in 0..c.height() {
                for x in 0..c.width() {
                    let p = px(&ctx, l, c.x1 + x, c.y1 + h);
                    assert_eq!(p, px(&ctx, l, c.x1 + x, c.y2 - h), "{c:?} {x} {h}");
                    assert_eq!(p, px(&ctx, l, c.x2 - x, c.y1 + h), "{c:?} {x} {h}");
                }
            }
            assert_eq!(px(&ctx, l, c.x1 - 1, (c.y1 + c.y2) / 2), WHITE);
        }
    }

    #[cfg(feature = "complex")]
    #[test]
    fn rounded_border_clipped_matches_unclipped() {
        let c = Area::new(4, 4, 59, 43);
        let mut d = red(5);
        d.radius = 12;
        d.opa = 180;
        let (f, lf) = run(&d, c, full());
        let clip = Area::new(0, 20, 30, 47);
        let (p, lp) = run(&d, c, clip);
        for y in 0..H {
            for x in 0..W {
                let inside = x >= clip.x1 && x <= clip.x2 && y >= clip.y1 && y <= clip.y2;
                let expected = if inside { px(&f, lf, x, y) } else { WHITE };
                assert_eq!(px(&p, lp, x, y), expected, "at {x},{y}");
            }
        }
    }
}
