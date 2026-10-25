//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_triangle.c`.
//!
//! Three line masks cover the triangle; simple horizontal/vertical gradients
//! retain upstream's mask/opacity rounding. With `complex` off this is a no-op.

use crate::SwDrawUnit;
use ferrovgl_core::draw::triangle::TriangleDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};

#[cfg(feature = "complex")]
use {
    crate::fill::{blend_dsc_zeroed, colors_to_rgb888, sw_blend},
    crate::grad,
    crate::mask::{self, LineSide, MaskLineParam, MaskParam, MaskRes},
    alloc::{vec, vec::Vec},
    ferrovgl_core::draw::grad::GradDir,
    ferrovgl_core::misc::area::{Area, PointPrecise},
    ferrovgl_core::misc::color::{ColorFormat, OPA_MAX, opa_mix2},
};

/// `lv_draw_sw_triangle()`.
pub fn triangle(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &TriangleDsc) {
    #[cfg(not(feature = "complex"))]
    let _ = (unit, ctx, t, dsc);
    #[cfg(feature = "complex")]
    triangle_complex(unit, ctx, t, dsc);
}

#[cfg(feature = "complex")]
fn triangle_complex(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &TriangleDsc) {
    let mut p = dsc.p.map(PointPrecise::to_point);
    let tri_area = Area::new(
        p[0].x.min(p[1].x).min(p[2].x),
        p[0].y.min(p[1].y).min(p[2].y),
        p[0].x.max(p[1].x).max(p[2].x),
        p[0].y.max(p[1].y).max(p[2].y),
    );
    let Some(draw_area) = Area::intersect(&tri_area, &t.clip_area) else {
        return;
    };
    // Prefer a vertical side, comparing precise coordinates before truncation.
    if dsc.p[0].x == dsc.p[1].x {
        // Already in the right order.
    } else if dsc.p[0].x == dsc.p[2].x {
        p.swap(1, 2);
    } else if dsc.p[1].x == dsc.p[2].x {
        p = [p[1], p[2], p[0]];
    } else {
        if p[0].y > p[1].y {
            p.swap(0, 1);
        }
        if p[0].y > p[2].y {
            p.swap(0, 2);
        }
        if p[1].y < p[2].y {
            p.swap(1, 2);
        }
    }
    if p[0].y > p[2].y {
        p.swap(0, 2);
    }
    if p[0].y > p[1].y {
        p.swap(0, 1);
    }
    if p[1].y < p[2].y {
        p.swap(1, 2);
    }
    let right = (p[1].x - p[0].x)
        .wrapping_mul(p[2].y - p[0].y)
        .wrapping_sub((p[1].y - p[0].y).wrapping_mul(p[2].x - p[0].x))
        < 0;
    let mut left_mask = MaskLineParam::default();
    let mut right_mask = MaskLineParam::default();
    let mut bottom_mask = MaskLineParam::default();
    mask::line_points_init(
        &mut left_mask,
        p[0].x,
        p[0].y,
        p[1].x,
        p[1].y,
        if right { LineSide::Right } else { LineSide::Left },
    );
    mask::line_points_init(
        &mut right_mask,
        p[0].x,
        p[0].y,
        p[2].x,
        p[2].y,
        if right { LineSide::Left } else { LineSide::Right },
    );
    mask::line_points_init(
        &mut bottom_mask,
        p[1].x,
        p[1].y,
        p[2].x,
        p[2].y,
        if p[1].y == p[2].y {
            LineSide::Top
        } else if right {
            LineSide::Left
        } else {
            LineSide::Right
        },
    );
    let mut masks = [
        MaskParam::Line(left_mask),
        MaskParam::Line(right_mask),
        MaskParam::Line(bottom_mask),
    ];
    let width = draw_area.width();
    let mut mask_buf = vec![255; width as usize];
    let direction = dsc.grad.dir;
    // Invalid stop counts would index outside the C descriptor. Reject them.
    if direction != GradDir::None && !(1..=dsc.grad.stops.len()).contains(&(dsc.grad.stops_count as usize)) {
        return;
    }
    let gradient = grad::get(&dsc.grad, tri_area.width(), tri_area.height());
    let mut colors = Vec::new();
    let horizontal_offset = (draw_area.x1 - tri_area.x1) as usize;
    if direction == GradDir::Hor {
        colors_to_rgb888(&gradient.color_map, &mut colors);
    }
    for y in draw_area.y1..=draw_area.y2 {
        let row = Area::new(draw_area.x1, y, draw_area.x2, y);
        mask_buf.fill(255);
        let mut result = mask::apply(
            &mut unit.circle_cache,
            &mut masks,
            &mut mask_buf,
            draw_area.x1,
            y,
            width,
        );
        let mut mask_row = &mask_buf[..];
        let mut color = dsc.color;
        let mut opacity = dsc.opa;
        if direction == GradDir::Ver {
            color = gradient.color_map[(y - tri_area.y1) as usize];
            opacity = gradient.opa_map[(y - tri_area.y1) as usize];
            if dsc.opa < OPA_MAX {
                opacity = opa_mix2(opacity, dsc.opa);
            }
        } else if direction == GradDir::Hor {
            let gradient_opa = &gradient.opa_map[horizontal_offset..horizontal_offset + width as usize];
            match result {
                MaskRes::Changed => {
                    for (mask, &opa) in mask_buf.iter_mut().zip(gradient_opa) {
                        if opa < OPA_MAX {
                            *mask = opa_mix2(*mask, opa);
                        }
                    }
                    mask_row = &mask_buf;
                }
                MaskRes::FullCover => {
                    mask_row = gradient_opa;
                    result = MaskRes::Changed;
                }
                MaskRes::Transp => continue,
                _ => {}
            }
        }
        let mut blend = blend_dsc_zeroed(row);
        blend.color = color;
        blend.opa = opacity;
        blend.mask_buf = Some(mask_row);
        blend.mask_area = Some(row);
        blend.mask_res = result;
        if direction == GradDir::Hor {
            blend.src_area = Some(row);
            blend.src_color_format = ColorFormat::Rgb888;
            blend.src_buf = Some(&colors[horizontal_offset * 3..]);
        }
        sw_blend(&unit.blend_handlers, ctx, t, &blend);
    }
    // Line masks own no cache entries; gradient maps are released by Drop.
}

#[cfg(all(test, feature = "complex", feature = "cf-xrgb8888"))]
mod tests {
    use super::*;
    use ferrovgl_core::draw::DrawTaskType;
    use ferrovgl_core::draw::draw_buf::DrawBuf;
    use ferrovgl_core::misc::color::Color;
    use ferrovgl_core::misc::types::ValuePrecise;

    const WIDTH: usize = 28;
    const HEIGHT: usize = 26;
    const BACKGROUND: [u8; 4] = [20, 30, 40, 77];

    fn render(dsc: &TriangleDsc, clip: Area) -> Vec<u8> {
        let bounds = Area::new(0, 0, WIDTH as i32 - 1, HEIGHT as i32 - 1);
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Xrgb8888, &bounds);
        let mut buffer = DrawBuf::create(WIDTH as u32, HEIGHT as u32, ColorFormat::Xrgb8888, 0).unwrap();
        for pixel in buffer.data_mut().unwrap()[..WIDTH * HEIGHT * 4]
            .as_chunks_mut::<4>()
            .0
        {
            pixel.copy_from_slice(&BACKGROUND);
        }
        ctx.layer_mut(layer).unwrap().draw_buf = Some(buffer);
        let task = ctx.add_task(layer, &bounds, DrawTaskType::Triangle);
        let mut task = ctx.task(task).unwrap().clone();
        task.clip_area = clip;
        triangle(&mut SwDrawUnit::default(), &mut ctx, &task, dsc);
        ctx.layer(layer).unwrap().draw_buf.as_ref().unwrap().data()[..WIDTH * HEIGHT * 4].to_vec()
    }

    fn descriptor(points: [(i32, i32); 3], direction: GradDir) -> TriangleDsc {
        let mut dsc = TriangleDsc::new();
        dsc.p = points.map(|(x, y)| PointPrecise::new(x as ValuePrecise, y as ValuePrecise));
        dsc.opa = 173;
        dsc.color = Color::make(240, 60, 20);
        dsc.grad.dir = direction;
        dsc.grad.stops[0].color = Color::make(220, 20, 30);
        dsc.grad.stops[0].opa = 70;
        dsc.grad.stops[1].color = Color::make(10, 190, 240);
        dsc.grad.stops[1].opa = 225;
        dsc
    }

    #[test]
    fn triangle_point_permutations_preserve_masks_and_gradient_direction() {
        let bounds = Area::new(0, 0, WIDTH as i32 - 1, HEIGHT as i32 - 1);
        for points in [
            [(-5, 3), (23, 5), (8, 24)],
            [(3, 3), (3, 22), (24, 12)],
            [(3, 21), (24, 21), (11, 2)],
        ] {
            for direction in [GradDir::None, GradDir::Hor, GradDir::Ver] {
                let mut dsc = descriptor(points, direction);
                let expected = render(&dsc, bounds);
                assert!(
                    expected
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|pixel| pixel[..3] != BACKGROUND[..3])
                );
                for order in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
                    dsc.p = order.map(|index| {
                        PointPrecise::new(points[index].0 as ValuePrecise, points[index].1 as ValuePrecise)
                    });
                    assert_eq!(
                        render(&dsc, bounds),
                        expected,
                        "{points:?} {direction:?} {order:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn clipped_partial_opacity_gradients_match_the_full_render_crop() {
        let bounds = Area::new(0, 0, WIDTH as i32 - 1, HEIGHT as i32 - 1);
        // Keep the right AA edge in each row. An entirely interior clip can
        // switch C's mask from Changed to FullCover, changing opacity rounding
        // by one LSB (255*opa >> 8 versus opa) even for a constant fill.
        let clip = Area::new(7, 8, 27, 19);
        for direction in [GradDir::None, GradDir::Hor, GradDir::Ver] {
            let dsc = descriptor([(-5, 3), (23, 5), (8, 24)], direction);
            let full = render(&dsc, bounds);
            let clipped = render(&dsc, clip);
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let offset = (y * WIDTH + x) * 4;
                    let inside = x >= clip.x1 as usize
                        && x <= clip.x2 as usize
                        && y >= clip.y1 as usize
                        && y <= clip.y2 as usize;
                    let expected = if inside {
                        &full[offset..offset + 4]
                    } else {
                        &BACKGROUND
                    };
                    assert_eq!(&clipped[offset..offset + 4], expected, "{direction:?} ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn single_gradient_stop_is_a_constant_color_as_in_c() {
        let bounds = Area::new(0, 0, WIDTH as i32 - 1, HEIGHT as i32 - 1);
        for direction in [GradDir::Hor, GradDir::Ver] {
            let mut dsc = descriptor([(2, 2), (25, 2), (12, 24)], direction);
            dsc.grad.stops[0].frac = 90;
            dsc.grad.stops_count = 1;
            let single = render(&dsc, bounds);
            dsc.grad.stops_count = 2;
            dsc.grad.stops[1] = dsc.grad.stops[0];
            dsc.grad.stops[1].frac = 255;
            assert_eq!(single, render(&dsc, bounds));
            let center = (10 * WIDTH + 12) * 4;
            assert_ne!(&single[center..center + 3], &BACKGROUND[..3]);
        }
    }

    #[test]
    fn fully_covered_clip_preserves_c_mask_opacity_rounding_difference() {
        let dsc = descriptor([(-5, 3), (23, 5), (8, 24)], GradDir::None);
        let full = render(&dsc, Area::new(0, 0, WIDTH as i32 - 1, HEIGHT as i32 - 1));
        let interior = render(&dsc, Area::new(7, 8, 17, 19));
        let offset = (8 * WIDTH + 7) * 4;
        // lv_draw_sw_blend.c drops FULL_COVER's mask. In
        // lv_draw_sw_blend_to_rgb888.c a Changed row instead applies
        // LV_OPA_MIX2(173,255)=172. The red result is therefore
        // (240*172+40*83)>>8=174 versus (240*173+40*82)>>8=175.
        assert_eq!(&full[offset..offset + 4], &[19, 50, 174, 77]);
        assert_eq!(&interior[offset..offset + 4], &[19, 50, 175, 77]);
    }
}
