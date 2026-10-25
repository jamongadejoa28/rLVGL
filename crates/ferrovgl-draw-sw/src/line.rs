//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_line.c`.
//!
//! Axis-aligned lines exclude the final endpoint. The upstream vertical dash
//! counter differs from the horizontal one; both are preserved. Skew lines use
//! line masks and intentionally ignore dash settings, as in upstream.

#[cfg(feature = "complex")]
use alloc::vec;
use ferrovgl_core::draw::line::LineDsc;
use ferrovgl_core::draw::rect::{FillDsc, RADIUS_CIRCLE};
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;
use ferrovgl_core::misc::color::OPA_MIN;

use crate::SwDrawUnit;
use crate::fill::{blend_dsc_zeroed, sw_blend};
#[cfg(feature = "complex")]
use crate::mask::{self, LineSide, MaskLineParam, MaskParam, MaskRes};

/// `lv_draw_sw_line()`.
#[allow(clippy::unnecessary_cast)] // ValuePrecise is either f32 or i32.
pub fn line(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &LineDsc) {
    if dsc.width == 0 || dsc.opa <= OPA_MIN || dsc.p1 == dsc.p2 {
        return;
    }
    let clip_line = Area::new(
        dsc.p1.x.min(dsc.p2.x) as i32 - dsc.width / 2,
        dsc.p1.y.min(dsc.p2.y) as i32 - dsc.width / 2,
        dsc.p1.x.max(dsc.p2.x) as i32 + dsc.width / 2,
        dsc.p1.y.max(dsc.p2.y) as i32 + dsc.width / 2,
    );
    if Area::intersect(&clip_line, &t.clip_area).is_none() {
        return;
    }
    if dsc.p1.y as i32 == dsc.p2.y as i32 {
        draw_line_hor(unit, ctx, t, dsc);
    } else if dsc.p1.x as i32 == dsc.p2.x as i32 {
        draw_line_ver(unit, ctx, t, dsc);
    } else {
        draw_line_skew(unit, ctx, t, dsc);
    }
    if dsc.round_start || dsc.round_end {
        let circle = FillDsc {
            color: dsc.color,
            radius: RADIUS_CIRCLE,
            opa: dsc.opa,
            ..FillDsc::new()
        };
        let radius = dsc.width >> 1;
        let correction = if dsc.width & 1 != 0 { 0 } else { 1 };
        for (draw, point) in [(dsc.round_start, dsc.p1), (dsc.round_end, dsc.p2)] {
            if draw {
                let area = Area::new(
                    point.x as i32 - radius,
                    point.y as i32 - radius,
                    point.x as i32 + radius - correction,
                    point.y as i32 + radius - correction,
                );
                crate::fill::fill(unit, ctx, t, &circle, &area);
            }
        }
    }
}

/// `draw_line_hor()`.
#[allow(clippy::unnecessary_cast)] // ValuePrecise is either f32 or i32.
pub fn draw_line_hor(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &LineDsc) {
    let width = dsc.width - 1;
    let half0 = width >> 1;
    let half1 = half0 + (width & 1);
    let area = Area::new(
        dsc.p1.x.min(dsc.p2.x) as i32,
        dsc.p1.y as i32 - half1,
        dsc.p1.x.max(dsc.p2.x) as i32 - 1,
        dsc.p1.y as i32 + half0,
    );
    let Some(area) = Area::intersect(&area, &t.clip_area) else {
        return;
    };
    if dsc.dash_gap == 0 || dsc.dash_width == 0 {
        let mut blend = blend_dsc_zeroed(area);
        blend.color = dsc.color;
        blend.opa = dsc.opa;
        sw_blend(&unit.blend_handlers, ctx, t, &blend);
    } else {
        #[cfg(feature = "complex")]
        draw_horizontal_dashes(unit, ctx, t, dsc, area);
    }
}

#[cfg(feature = "complex")]
fn draw_horizontal_dashes(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &LineDsc, area: Area) {
    let Some(period) = dsc.dash_gap.checked_add(dsc.dash_width).filter(|&p| p > 0) else {
        return;
    };
    if dsc.dash_width < 0 || dsc.dash_gap < 0 {
        return;
    }
    let width = area.width() as usize;
    let start = area.x1 % period;
    let mut mask_buf = vec![255; width];
    for y in area.y1..=area.y2 {
        mask_buf.fill(255);
        let mut count = start;
        let mut index = 0;
        while index < width {
            if count < dsc.dash_width {
                let diff = (dsc.dash_width - count) as i16;
                // Invalid huge/negative dash settings can fail to advance in C.
                if diff <= 0 {
                    return;
                }
                index += diff as usize;
                count += i32::from(diff);
            } else {
                mask_buf[index] = 0;
                index += 1;
                count += 1;
            }
            if count >= period {
                count = 0;
            }
        }
        let row = Area::new(area.x1, y, area.x2, y);
        let mut blend = blend_dsc_zeroed(row);
        blend.color = dsc.color;
        blend.opa = dsc.opa;
        blend.mask_buf = Some(&mask_buf);
        blend.mask_area = Some(row);
        blend.mask_res = MaskRes::Changed;
        sw_blend(&unit.blend_handlers, ctx, t, &blend);
    }
}

/// `draw_line_ver()`.
#[allow(clippy::unnecessary_cast)] // ValuePrecise is either f32 or i32.
pub fn draw_line_ver(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &LineDsc) {
    let width = dsc.width - 1;
    let half0 = width >> 1;
    let half1 = half0 + (width & 1);
    let area = Area::new(
        dsc.p1.x as i32 - half1,
        dsc.p1.y.min(dsc.p2.y) as i32,
        dsc.p1.x as i32 + half0,
        dsc.p1.y.max(dsc.p2.y) as i32 - 1,
    );
    let Some(area) = Area::intersect(&area, &t.clip_area) else {
        return;
    };
    if dsc.dash_gap == 0 || dsc.dash_width == 0 {
        let mut blend = blend_dsc_zeroed(area);
        blend.color = dsc.color;
        blend.opa = dsc.opa;
        sw_blend(&unit.blend_handlers, ctx, t, &blend);
    } else {
        #[cfg(feature = "complex")]
        draw_vertical_dashes(unit, ctx, t, dsc, area);
    }
}

#[cfg(feature = "complex")]
fn draw_vertical_dashes(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &LineDsc, area: Area) {
    let Some(period) = dsc.dash_gap.checked_add(dsc.dash_width).filter(|&p| p > 0) else {
        return;
    };
    if dsc.dash_width < 0 || dsc.dash_gap < 0 {
        return;
    }
    let mut count = area.y1 % period;
    let mask_buf = vec![255; area.width() as usize];
    for y in area.y1..=area.y2 {
        let row = Area::new(area.x1, y, area.x2, y);
        let mut blend = blend_dsc_zeroed(row);
        blend.color = dsc.color;
        blend.opa = dsc.opa;
        blend.mask_buf = Some(&mask_buf);
        blend.mask_area = Some(row);
        // Upstream tests `>` and wraps BEFORE increment, unlike horizontal.
        blend.mask_res = if count > dsc.dash_width {
            MaskRes::Transp
        } else {
            MaskRes::FullCover
        };
        if count >= period {
            count = 0;
        }
        count += 1;
        sw_blend(&unit.blend_handlers, ctx, t, &blend);
    }
}

/// `draw_line_skew()`.
pub fn draw_line_skew(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &LineDsc) {
    #[cfg(feature = "complex")]
    draw_skew_complex(unit, ctx, t, dsc);
    #[cfg(not(feature = "complex"))]
    let _ = (unit, ctx, t, dsc);
}

#[cfg(feature = "complex")]
fn draw_skew_complex(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &LineDsc) {
    let (p1, p2) = if dsc.p1.y < dsc.p2.y {
        (dsc.p1.to_point(), dsc.p2.to_point())
    } else {
        (dsc.p2.to_point(), dsc.p1.to_point())
    };
    let dx = p2.x - p1.x;
    let dy = p2.y - p1.y;
    let flat = dx.abs() > dy.abs();
    const WIDTH_CORRECTION: [u8; 33] = [
        128, 128, 128, 129, 129, 130, 130, 131, 132, 133, 134, 135, 137, 138, 140, 141, 143, 145, 147, 149,
        151, 153, 155, 158, 160, 162, 165, 167, 170, 173, 175, 178, 181,
    ];
    let index = if flat {
        (dy.abs() << 5) / dx.abs()
    } else {
        (dx.abs() << 5) / dy.abs()
    };
    let width = (dsc.width * i32::from(WIDTH_CORRECTION[index as usize]) + 63) >> 7;
    let half0 = width >> 1;
    let half1 = half0 + (width & 1);
    let area = Area::new(
        p1.x.min(p2.x) - width,
        p1.y.min(p2.y) - width,
        p1.x.max(p2.x) + width,
        p1.y.max(p2.y) + width,
    );
    let Some(area) = Area::intersect(&area, &t.clip_area) else {
        return;
    };
    let mut left = MaskLineParam::default();
    let mut right = MaskLineParam::default();
    if flat {
        if dx > 0 {
            mask::line_points_init(&mut left, p1.x, p1.y - half0, p2.x, p2.y - half0, LineSide::Left);
            mask::line_points_init(
                &mut right,
                p1.x,
                p1.y + half1,
                p2.x,
                p2.y + half1,
                LineSide::Right,
            );
        } else {
            mask::line_points_init(&mut left, p1.x, p1.y + half1, p2.x, p2.y + half1, LineSide::Left);
            mask::line_points_init(
                &mut right,
                p1.x,
                p1.y - half0,
                p2.x,
                p2.y - half0,
                LineSide::Right,
            );
        }
    } else {
        mask::line_points_init(&mut left, p1.x + half1, p1.y, p2.x + half1, p2.y, LineSide::Left);
        mask::line_points_init(
            &mut right,
            p1.x - half0,
            p1.y,
            p2.x - half0,
            p2.y,
            LineSide::Right,
        );
    }
    let mut masks = vec![MaskParam::Line(left), MaskParam::Line(right)];
    if !dsc.raw_end {
        let mut top = MaskLineParam::default();
        let mut bottom = MaskLineParam::default();
        mask::line_points_init(&mut top, p1.x, p1.y, p1.x - dy, p1.y + dx, LineSide::Bottom);
        mask::line_points_init(&mut bottom, p2.x, p2.y, p2.x - dy, p2.y + dx, LineSide::Top);
        masks.push(MaskParam::Line(top));
        masks.push(MaskParam::Line(bottom));
    }
    // Upstream allocates min(area size, area width): for a nonempty clipped
    // area this is exactly one row, so every iteration flushes that row.
    let mut mask_buf = vec![255; area.width() as usize];
    for y in area.y1..=area.y2 {
        mask_buf.fill(255);
        let result = mask::apply(
            &mut unit.circle_cache,
            &mut masks,
            &mut mask_buf,
            area.x1,
            y,
            area.width(),
        );
        if result == MaskRes::Transp {
            mask_buf.fill(0);
        }
        let row = Area::new(area.x1, y, area.x2, y);
        let mut blend = blend_dsc_zeroed(row);
        blend.color = dsc.color;
        blend.opa = dsc.opa;
        blend.mask_buf = Some(&mask_buf);
        blend.mask_area = Some(row);
        blend.mask_res = MaskRes::Changed;
        sw_blend(&unit.blend_handlers, ctx, t, &blend);
    }
    for param in &mut masks {
        mask::free_param(&mut unit.circle_cache, param);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fill::tests::{H, W, WHITE, ctx_with_layer, px, task};
    use ferrovgl_core::draw::LayerId;
    use ferrovgl_core::misc::area::Point;

    fn run(dsc: &LineDsc, clip: Area) -> (DrawCtx, LayerId) {
        let (mut ctx, layer) = ctx_with_layer();
        let mut unit = SwDrawUnit::new(0);
        line(&mut unit, &mut ctx, &task(layer, clip), dsc);
        (ctx, layer)
    }
    fn clip() -> Area {
        Area::new(0, 0, W - 1, H - 1)
    }
    fn descriptor(p1: Point, p2: Point, width: i32) -> LineDsc {
        LineDsc {
            p1: p1.to_precise(),
            p2: p2.to_precise(),
            width,
            ..LineDsc::new()
        }
    }
    fn ink(ctx: &DrawCtx, layer: LayerId) -> usize {
        (0..H)
            .flat_map(|y| (0..W).map(move |x| (x, y)))
            .filter(|&(x, y)| px(ctx, layer, x, y) != WHITE)
            .count()
    }

    #[test]
    fn horizontal_and_vertical_even_width_exclude_final_endpoint() {
        let horizontal = descriptor(Point::new(5, 10), Point::new(20, 10), 4);
        let (ctx, layer) = run(&horizontal, clip());
        assert_eq!(ink(&ctx, layer), 15 * 4);
        for y in 8..=11 {
            assert_eq!(px(&ctx, layer, 5, y), [0; 3]);
            assert_eq!(px(&ctx, layer, 19, y), [0; 3]);
        }
        assert_eq!(px(&ctx, layer, 20, 10), WHITE);
        assert_eq!(px(&ctx, layer, 10, 12), WHITE);
        let vertical = descriptor(Point::new(10, 20), Point::new(10, 5), 4);
        let (ctx, layer) = run(&vertical, clip());
        assert_eq!(ink(&ctx, layer), 15 * 4);
        assert_eq!(px(&ctx, layer, 8, 5), [0; 3]);
        assert_eq!(px(&ctx, layer, 11, 19), [0; 3]);
        assert_eq!(px(&ctx, layer, 10, 20), WHITE);
    }

    #[cfg(feature = "complex")]
    #[test]
    fn horizontal_and_vertical_dashes_keep_different_upstream_counters() {
        let mut dsc = descriptor(Point::new(0, 10), Point::new(16, 10), 1);
        dsc.dash_width = 2;
        dsc.dash_gap = 2;
        let (ctx, layer) = run(&dsc, clip());
        for x in 0..16 {
            assert_eq!(px(&ctx, layer, x, 10) != WHITE, x % 4 < 2);
        }
        dsc.p1 = Point::new(10, 0).to_precise();
        dsc.p2 = Point::new(10, 16).to_precise();
        let (ctx, layer) = run(&dsc, clip());
        let expected = [
            true, true, true, false, false, true, true, false, false, true, true, false,
        ];
        for (y, expected) in expected.into_iter().enumerate() {
            assert_eq!(px(&ctx, layer, 10, y as i32) != WHITE, expected, "y={y}");
        }
        // C initializes the vertical dash counter from the clipped first row.
        let (ctx, layer) = run(&dsc, Area::new(0, 4, W - 1, H - 1));
        assert_eq!(px(&ctx, layer, 10, 3), WHITE);
        assert_eq!(px(&ctx, layer, 10, 4), [0; 3]);
        assert_eq!(px(&ctx, layer, 10, 6), [0; 3]);
        assert_eq!(px(&ctx, layer, 10, 7), WHITE);
    }

    #[cfg(feature = "complex")]
    #[test]
    fn skew_masks_are_antialiased_direction_invariant_and_ignore_dashes() {
        for end in [Point::new(50, 20), Point::new(20, 40), Point::new(2, 30)] {
            let mut dsc = descriptor(Point::new(10, 5), end, 5);
            let (ctx, layer) = run(&dsc, clip());
            assert!(ink(&ctx, layer) > 40);
            assert!((0..H).any(|y| (0..W).any(|x| {
                let p = px(&ctx, layer, x, y);
                p != WHITE && p != [0; 3]
            })));
            core::mem::swap(&mut dsc.p1, &mut dsc.p2);
            dsc.dash_width = 3;
            dsc.dash_gap = 5;
            let (reverse, other) = run(&dsc, clip());
            assert_eq!(
                ctx.layer(layer).unwrap().draw_buf.as_ref().unwrap().data(),
                reverse.layer(other).unwrap().draw_buf.as_ref().unwrap().data()
            );
        }
    }

    #[cfg(feature = "complex")]
    #[test]
    fn rounded_caps_and_raw_skew_ends_extend_only_the_expected_regions() {
        let mut dsc = descriptor(Point::new(10, 20), Point::new(40, 20), 8);
        dsc.round_start = true;
        dsc.round_end = true;
        let (ctx, layer) = run(&dsc, clip());
        assert_eq!(px(&ctx, layer, 7, 20), [0; 3]);
        assert_eq!(px(&ctx, layer, 42, 20), [0; 3]);
        assert_eq!(px(&ctx, layer, 5, 20), WHITE);
        assert_eq!(px(&ctx, layer, 44, 20), WHITE);
        let mut dsc = descriptor(Point::new(15, 15), Point::new(35, 35), 6);
        let (normal, first) = run(&dsc, clip());
        dsc.raw_end = true;
        let (raw, second) = run(&dsc, clip());
        assert!(ink(&raw, second) > ink(&normal, first));
        assert_eq!(px(&normal, first, 25, 25), px(&raw, second, 25, 25));
    }

    #[test]
    fn transparent_zero_length_and_clipped_lines_do_not_touch_pixels() {
        let mut dsc = descriptor(Point::new(10, 10), Point::new(10, 10), 8);
        dsc.round_start = true;
        dsc.round_end = true;
        let (ctx, layer) = run(&dsc, clip());
        assert_eq!(ink(&ctx, layer), 0);
        dsc.p2 = Point::new(30, 10).to_precise();
        dsc.opa = OPA_MIN;
        let (ctx, layer) = run(&dsc, clip());
        assert_eq!(ink(&ctx, layer), 0);
        dsc.opa = 255;
        let (ctx, layer) = run(&dsc, Area::new(50, 30, 63, 47));
        assert_eq!(ink(&ctx, layer), 0);
    }

    #[cfg(feature = "complex")]
    #[test]
    fn rounded_skew_endpoint_outside_buffer_is_clipped() {
        let mut dsc = descriptor(Point::new(0, 0), Point::new(W + 1, H + 1), 4);
        dsc.round_start = true;
        dsc.round_end = true;
        let (ctx, layer) = run(&dsc, clip());
        assert!(ink(&ctx, layer) > 100);
        assert_eq!(px(&ctx, layer, 0, 0), [0; 3]);
    }
}
