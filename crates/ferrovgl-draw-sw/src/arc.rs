//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_arc.c`.

use ferrovgl_core::draw::arc::ArcDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;

use crate::SwDrawUnit;

/// `lv_draw_sw_arc()`
pub fn arc(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &ArcDsc, coords: &Area) {
    #[cfg(feature = "complex")]
    draw_arc(unit, ctx, t, dsc, coords);
    #[cfg(not(feature = "complex"))]
    let _ = (unit, ctx, t, dsc, coords);
}

#[cfg(feature = "complex")]
use crate::fill::{blend_dsc_zeroed, sw_blend};
#[cfg(feature = "complex")]
use crate::mask::{self, CircleCache, MaskAngleParam, MaskParam, MaskRadiusParam, MaskRes};
#[cfg(feature = "complex")]
use alloc::{vec, vec::Vec};
#[cfg(feature = "complex")]
#[cfg(feature = "complex")]
use ferrovgl_core::draw::image::{DecodedImage, with_decoded_image};
#[cfg(feature = "complex")]
use ferrovgl_core::draw::rect::{BorderDsc, RADIUS_CIRCLE};
#[cfg(feature = "complex")]
use ferrovgl_core::misc::color::{ColorFormat, OPA_MIN, Opa, opa_mix2};
#[cfg(feature = "complex")]
use ferrovgl_core::misc::event::LvResult;
#[cfg(feature = "complex")]
use ferrovgl_core::misc::math::{TRIGO_SHIFT, trigo_cos, trigo_sin};
#[cfg(feature = "complex")]
use ferrovgl_core::misc::types::ValuePrecise;

#[cfg(feature = "complex")]
#[allow(clippy::unnecessary_cast)] // ValuePrecise is f32 or i32.
fn draw_arc(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, task: &DrawTask, dsc: &ArcDsc, coords: &Area) {
    if dsc.opa <= OPA_MIN || dsc.width == 0 || dsc.start_angle == dsc.end_angle {
        return;
    }
    let width = dsc.width.min(i32::from(dsc.radius));
    let Some(mut clipped) = Area::intersect(coords, &task.clip_area) else {
        return;
    };
    if dsc.img_src.is_none()
        && (dsc.start_angle + 360 as ValuePrecise == dsc.end_angle
            || dsc.start_angle == dsc.end_angle + 360 as ValuePrecise)
    {
        let border = BorderDsc {
            opa: dsc.opa,
            color: dsc.color,
            width,
            radius: RADIUS_CIRCLE,
            ..BorderDsc::new()
        };
        crate::border::border(unit, ctx, task, &border, coords);
        return;
    }
    let mut inner = *coords;
    inner.increase(-dsc.width, -dsc.width);
    let mut bounds = Area::default();
    ferrovgl_core::draw::arc::get_area(
        dsc.center.x,
        dsc.center.y,
        dsc.radius,
        dsc.start_angle,
        dsc.end_angle,
        width,
        dsc.rounded,
        &mut bounds,
    );
    let Some(intersection) = Area::intersect(&clipped, &bounds) else {
        return;
    };
    clipped = intersection;
    let mut start = dsc.start_angle as i32;
    let mut end = dsc.end_angle as i32;
    while start >= 360 {
        start -= 360;
    }
    while end >= 360 {
        end -= 360;
    }
    let mut angle = MaskAngleParam::default();
    mask::angle_init(&mut angle, dsc.center.x, dsc.center.y, start, end);
    let mut masks = vec![MaskParam::Angle(angle)];
    let mut outer = MaskRadiusParam::default();
    if mask::radius_init(&mut unit.circle_cache, &mut outer, coords, RADIUS_CIRCLE, false) != LvResult::Ok {
        return;
    }
    masks.push(MaskParam::Radius(outer));
    if inner.width() > 0 && inner.height() > 0 {
        let mut radius = MaskRadiusParam::default();
        if mask::radius_init(&mut unit.circle_cache, &mut radius, &inner, RADIUS_CIRCLE, true) != LvResult::Ok
        {
            for mask in &mut masks {
                mask::free_param(&mut unit.circle_cache, mask);
            }
            return;
        }
        masks.push(MaskParam::Radius(radius));
    }
    let caps = dsc
        .rounded
        .then(|| allocate_circle_mask(&mut unit.circle_cache, dsc, start, end, width))
        .flatten();
    if dsc.img_src.is_none() {
        draw_rows(unit, ctx, task, dsc, clipped, &mut masks, caps.as_ref(), None);
    } else {
        // The shared in-memory decoder matches open/decoded/close ownership.
        // Missing files, unsupported sources and partial-only decoders use
        // the descriptor color, as in LVGL's failed-image fallback.
        with_decoded_image(ctx, &dsc.img_src, |ctx, image| {
            draw_rows(unit, ctx, task, dsc, clipped, &mut masks, caps.as_ref(), image);
        });
    }
    for mask in &mut masks {
        mask::free_param(&mut unit.circle_cache, mask);
    }
}

#[cfg(feature = "complex")]
fn draw_rows(
    unit: &mut SwDrawUnit,
    ctx: &mut DrawCtx,
    task: &DrawTask,
    dsc: &ArcDsc,
    clipped: Area,
    masks: &mut [MaskParam<'_>],
    caps: Option<&CircleCaps>,
    decoded: Option<&DecodedImage<'_>>,
) {
    let blend_width = clipped.width();
    let mut buffer = vec![255; blend_width as usize];
    let image_area = decoded.map(|image| {
        let header = image.decoded.header;
        let offset = i32::from(header.w) / 2;
        let mut area = Area::new(0, 0, i32::from(header.w) - 1, i32::from(header.h) - 1);
        // Upstream intentionally uses half the image width on BOTH axes.
        area.move_by(dsc.center.x - offset, dsc.center.y - offset);
        area
    });
    for y in clipped.y1..=clipped.y2 {
        let row = Area::new(clipped.x1, y, clipped.x2, y);
        buffer.fill(255);
        let mut result = mask::apply(&mut unit.circle_cache, masks, &mut buffer, row.x1, y, blend_width);
        if let Some(caps) = caps {
            for circle in [&caps.first, &caps.last] {
                if y >= circle.y1 && y <= circle.y2 {
                    if result == MaskRes::Transp {
                        buffer.fill(0);
                        result = MaskRes::Changed;
                    }
                    add_circle(&caps.mask, &row, circle, &mut buffer, caps.width);
                }
            }
        }
        let mut blend = blend_dsc_zeroed(row);
        blend.opa = dsc.opa;
        // C's zeroed descriptor leaves color black when decoding succeeds.
        if let (Some(image), Some(image_area)) = (decoded, image_area) {
            let header = image.decoded.header;
            let bytes = image.decoded.data();
            blend.src_buf = Some(bytes);
            blend.src_stride = u32::from(header.stride);
            blend.src_color_format = header.cf;
            blend.src_area = Some(image_area);
            if header.cf == ColorFormat::Rgb565A8 {
                blend.src_color_format = ColorFormat::Rgb565;
                if result != MaskRes::Transp {
                    let alpha_offset = usize::from(header.stride) * usize::from(header.h);
                    let alpha = bytes.get(alpha_offset..).unwrap_or(&[]);
                    let alpha_stride = i64::from(header.stride / 2);
                    let start =
                        alpha_stride * i64::from(y - image_area.y1) + i64::from(row.x1 - image_area.x1);
                    for (index, pixel) in buffer.iter_mut().enumerate() {
                        // C assumes the image covers the arc; safely clip
                        // malformed or smaller alpha planes at the boundary.
                        let opacity = usize::try_from(start + index as i64)
                            .ok()
                            .and_then(|index| alpha.get(index))
                            .copied()
                            .unwrap_or(0);
                        *pixel = opa_mix2(*pixel, opacity);
                    }
                    if result == MaskRes::FullCover {
                        result = MaskRes::Changed;
                    }
                }
            }
        } else {
            blend.color = dsc.color;
        }
        blend.mask_buf = Some(&buffer);
        blend.mask_area = Some(row);
        blend.mask_res = result;
        sw_blend(&unit.blend_handlers, ctx, task, &blend);
    }
}

#[cfg(feature = "complex")]
fn add_circle(circle: &[Opa], row: &Area, circle_area: &Area, mask: &mut [Opa], width: i32) {
    let Some(common) = Area::intersect(circle_area, row) else {
        return;
    };
    let circle_offset = (width * (common.y1 - circle_area.y1) + common.x1 - circle_area.x1) as usize;
    let mask_offset = (common.x1 - row.x1) as usize;
    for index in 0..common.width() as usize {
        if let (Some(dest), Some(source)) = (
            mask.get_mut(mask_offset + index),
            circle.get(circle_offset + index),
        ) {
            *dest = dest.saturating_add(*source);
        }
    }
}

#[cfg(feature = "complex")]
fn get_rounded_area(angle: i16, radius: i32, thickness: u8) -> Area {
    let half = i32::from(thickness / 2);
    let correction = if thickness & 1 == 0 { 1 } else { 0 };
    let x = (radius - half).wrapping_mul(trigo_cos(angle)) >> (TRIGO_SHIFT - 8);
    let y = (radius - half).wrapping_mul(trigo_sin(angle)) >> (TRIGO_SHIFT - 8);
    let ends = |value: i32| {
        if value > 0 {
            let value = (value - 128) >> 8;
            (value - half + correction, value + half)
        } else {
            let value = (value + 128) >> 8;
            (value - half, value + half - correction)
        }
    };
    let (x1, x2) = ends(x);
    let (y1, y2) = ends(y);
    Area::new(x1, y1, x2, y2)
}

#[cfg(feature = "complex")]
struct CircleCaps {
    mask: Vec<Opa>,
    first: Area,
    last: Area,
    width: i32,
}

#[cfg(feature = "complex")]
fn allocate_circle_mask(
    cache: &mut CircleCache,
    dsc: &ArcDsc,
    start: i32,
    end: i32,
    width: i32,
) -> Option<CircleCaps> {
    let side = usize::try_from(width).ok().filter(|&width| width != 0)?;
    let size = side.checked_mul(side)?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(size).ok()?;
    pixels.resize(size, 255);
    let area = Area::new(0, 0, width - 1, width - 1);
    let mut radius = MaskRadiusParam::default();
    if mask::radius_init(cache, &mut radius, &area, width / 2, false) != LvResult::Ok {
        return None;
    }
    let mut masks = [MaskParam::Radius(radius)];
    for (y, row) in pixels.chunks_exact_mut(side).enumerate() {
        if mask::apply(cache, &mut masks, row, 0, y as i32, width) == MaskRes::Transp {
            row.fill(0);
        }
    }
    mask::free_param(cache, &mut masks[0]);
    // The uint8_t thickness argument is deliberate, matching upstream even
    // for widths above 255; the backing circle still has `width` pixels.
    let mut first = get_rounded_area(start as i16, i32::from(dsc.radius), width as u8);
    let mut last = get_rounded_area(end as i16, i32::from(dsc.radius), width as u8);
    first.move_by(dsc.center.x, dsc.center.y);
    last.move_by(dsc.center.x, dsc.center.y);
    Some(CircleCaps {
        mask: pixels,
        first,
        last,
        width,
    })
}

#[cfg(all(test, feature = "complex", feature = "cf-xrgb8888"))]
#[allow(clippy::unnecessary_cast)] // Test angles also support integer ValuePrecise.
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use ferrovgl_core::Point;
    use ferrovgl_core::draw::DrawTaskRef;
    use ferrovgl_core::draw::draw_buf::{DrawBuf, STRIDE_AUTO};
    use ferrovgl_core::misc::color::Color;
    use ferrovgl_core::misc::style::{ImageSrc, SharedRef};

    fn descriptor() -> ArcDsc {
        ArcDsc {
            center: Point::new(16, 16),
            radius: 10,
            width: 4,
            start_angle: 0 as ValuePrecise,
            end_angle: 90 as ValuePrecise,
            color: Color::WHITE,
            ..ArcDsc::new()
        }
    }

    fn render(dsc: &ArcDsc, clip: Area) -> Vec<u8> {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Xrgb8888, &Area::new(0, 0, 31, 31));
        assert!(ctx.layer_alloc_buf(layer));
        ctx.draw_arc(layer, dsc);
        if let Some(mut task) = ctx.task(DrawTaskRef { layer, index: 0 }).cloned() {
            task.clip_area = clip;
            let mut unit = SwDrawUnit::default();
            arc(&mut unit, &mut ctx, &task, dsc, &task.area);
            assert!(unit.circle_cache.entries.iter().all(|entry| entry.used_cnt == 0));
        }
        ctx.layer(layer)
            .unwrap()
            .draw_buf
            .as_ref()
            .unwrap()
            .data()
            .to_vec()
    }

    fn pixel(bytes: &[u8], x: usize, y: usize) -> u8 {
        bytes[(y * 32 + x) * 4]
    }

    #[test]
    fn arc_quadrant_clip_and_ring_hole_are_respected() {
        let image = render(&descriptor(), Area::new(0, 0, 31, 31));
        assert_eq!(pixel(&image, 23, 20), 255);
        for (x, y) in [(16, 16), (8, 20), (23, 10), (8, 10)] {
            assert_eq!(pixel(&image, x, y), 0);
        }
        let clipped = render(&descriptor(), Area::new(22, 18, 24, 22));
        for y in 0..32 {
            for x in 0..32 {
                assert_eq!(
                    pixel(&clipped, x, y),
                    if (22..=24).contains(&x) && (18..=22).contains(&y) {
                        pixel(&image, x, y)
                    } else {
                        0
                    }
                );
            }
        }
    }

    #[test]
    fn arc_round_caps_add_coverage_and_release_cache_references() {
        let mut dsc = descriptor();
        dsc.end_angle = 10 as ValuePrecise;
        let flat = render(&dsc, Area::new(0, 0, 31, 31));
        dsc.rounded = true;
        let round = render(&dsc, Area::new(0, 0, 31, 31));
        let count = |bytes: &[u8]| {
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| pixel[0] != 0)
                .count()
        };
        assert!(count(&round) > count(&flat));
        assert!(pixel(&round, 23, 14) > 0);
        assert_eq!(pixel(&flat, 23, 14), 0);
    }

    #[test]
    fn arc_missing_image_falls_back_to_color_and_full_turn_directions_match() {
        let mut dsc = descriptor();
        let plain = render(&dsc, Area::new(0, 0, 31, 31));
        dsc.img_src = ImageSrc::Path(SharedRef::Static("missing.arc"));
        assert_eq!(render(&dsc, Area::new(0, 0, 31, 31)), plain);
        dsc.img_src = ImageSrc::None;
        dsc.end_angle = 360 as ValuePrecise;
        let clockwise = render(&dsc, Area::new(0, 0, 31, 31));
        dsc.end_angle = -360 as ValuePrecise;
        assert_eq!(render(&dsc, Area::new(0, 0, 31, 31)), clockwise);
    }

    #[test]
    fn arc_rgb565a8_uses_padded_alpha_stride() {
        // Deliberate padding: RGB stride 44, A8 stride 22 for a 20px image.
        let mut image = DrawBuf::create(20, 20, ColorFormat::Rgb565A8, 44).unwrap();
        let bytes = image.data_mut().unwrap();
        bytes[..44 * 20].fill(255);
        for row in bytes[44 * 20..].chunks_mut(22).take(20) {
            row.fill(0);
            row[10..20].fill(255);
        }
        let mut dsc = descriptor();
        dsc.end_angle = 360 as ValuePrecise;
        dsc.img_src = ImageSrc::DrawBuf(Rc::new(image));
        let image = render(&dsc, Area::new(0, 0, 31, 31));
        assert_eq!(pixel(&image, 23, 20), 255);
        assert_eq!(pixel(&image, 8, 20), 0);
        assert_eq!(pixel(&image, 16, 16), 0);
    }

    #[test]
    fn arc_non_square_image_keeps_upstream_width_based_y_offset() {
        let mut image = DrawBuf::create(20, 12, ColorFormat::Xrgb8888, STRIDE_AUTO).unwrap();
        image.data_mut().unwrap().fill(255);
        let mut dsc = descriptor();
        dsc.start_angle = 0 as ValuePrecise;
        dsc.end_angle = 360 as ValuePrecise;
        dsc.img_src = ImageSrc::DrawBuf(Rc::new(image));
        let image = render(&dsc, Area::new(0, 0, 31, 31));
        assert!(pixel(&image, 23, 10) > 0);
        assert_eq!(pixel(&image, 23, 20), 0);
    }
}
