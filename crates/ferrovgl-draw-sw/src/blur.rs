//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_blur.c`.
//!
//! Four in-place IIR passes, including upstream's endpoint and pixel-repeat
//! optimizations. Three/four-byte formats blur only the first three bytes;
//! alpha is unchanged except when skipped rows are copied wholesale.

use ferrovgl_core::draw::blur::BlurDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;
use ferrovgl_core::misc::color::ColorFormat;
use ferrovgl_core::misc::math::sqrt32;
use ferrovgl_core::misc::style::BlurQuality;

use crate::SwDrawUnit;

/// `lv_draw_sw_blur()`
pub fn blur(_unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &BlurDsc, coords: &Area) {
    if dsc.blur_radius <= 0 {
        return;
    }
    let Some(layer) = ctx.layer_mut(t.target_layer) else {
        return;
    };
    let (x_offset, y_offset) = (layer.buf_area.x1, layer.buf_area.y1);
    let Some(mut clipped) = Area::intersect(coords, &t.clip_area) else {
        return;
    };
    clipped.move_by(-x_offset, -y_offset);
    let Some(buffer) = layer.draw_buf.as_mut() else {
        return;
    };
    let format = PixelFormat {
        size: usize::from(buffer.header.cf.size()),
        swapped: buffer.header.cf == ColorFormat::Rgb565Swapped,
    };
    // C's byte-oriented routine has no meaningful representation for packed
    // sub-byte formats. Reject those instead of indexing beyond their rows.
    if buffer.header.cf.bpp() < 8 || format.size == 0 {
        return;
    }
    let bounds = Area::new(
        0,
        0,
        i32::from(buffer.header.w) - 1,
        i32::from(buffer.header.h) - 1,
    );
    let Some(mut clipped) = Area::intersect(&clipped, &bounds) else {
        return;
    };
    let mut radius = dsc.blur_radius as u32;
    let skip = match dsc.quality {
        BlurQuality::Auto if radius >= 32 && dsc.corner_radius == 0 && clipped.size() > 160 * 160 => 3,
        BlurQuality::Auto if radius >= 8 => 2,
        BlurQuality::Speed if radius >= 24 => 3,
        BlurQuality::Speed => 2,
        _ => 1,
    };
    clipped.x1 = ((clipped.x1 + skip - 1) / skip) * skip;
    clipped.y1 = ((clipped.y1 + skip - 1) / skip) * skip;
    clipped.x2 = ((clipped.x2 - (skip - 1)) / skip) * skip;
    clipped.y2 = ((clipped.y2 - (skip - 1)) / skip) * skip;
    if clipped.width() <= 0 || clipped.height() <= 0 {
        return;
    }
    radius /= skip as u32;
    let intensity = INTENSITY_MAX.wrapping_mul(radius) / (radius + 4);
    let samples = (radius / 2).max(1) as usize;
    let corner = dsc.corner_radius.min(coords.width().min(coords.height()) >> 1);
    let stride = usize::from(buffer.header.stride);
    let origin = buffer.goto_xy(0, 0);
    let Some(data) = buffer.data_mut() else { return };
    let offset = |x: i32, y: i32| origin + y as usize * stride + x as usize * format.size;

    for x in (clipped.x1..=clipped.x2).step_by(skip as usize) {
        let edge = rounded_edge(coords.x1, coords.x2, x_offset + x, corner);
        let start = (coords.y1 - y_offset + edge).clamp(clipped.y1, clipped.y2) / skip * skip;
        let end = (coords.y2 - y_offset - edge).clamp(clipped.y1, clipped.y2) / skip * skip;
        if start > end {
            continue;
        }
        let count = ((end - start) / skip + 1) as usize;
        let step = (stride * skip as usize) as isize;
        pass(
            data,
            offset(x, start),
            step,
            count,
            samples.min(count),
            intensity,
            format,
        );
        pass(
            data,
            offset(x, end),
            -step,
            count,
            samples.min(count),
            intensity,
            format,
        );
    }

    for y in (clipped.y1..=clipped.y2).step_by(skip as usize) {
        let edge = rounded_edge(coords.y1, coords.y2, y_offset + y, corner);
        let start = (coords.x1 - x_offset + edge).clamp(clipped.x1, clipped.x2) / skip * skip;
        let end = (coords.x2 - x_offset - edge).clamp(clipped.x1, clipped.x2) / skip * skip;
        if start > end {
            continue;
        }
        let count = ((end - start) / skip + 1) as usize;
        let step = (format.size * skip as usize) as isize;
        // The 1- and 3-byte C loops begin their counter at start+skip but
        // their pointer at start: only RGB565 processes the final sample.
        let forward_count = if format.size == 2 { count } else { count - 1 };
        pass(
            data,
            offset(start, y),
            step,
            forward_count,
            samples.min(count),
            intensity,
            format,
        );
        pass(
            data,
            offset(end, y),
            -step,
            count,
            samples.min(count),
            intensity,
            format,
        );

        // Repeating the filtered samples after the pass is equivalent to C's
        // reverse-pass copies: the writes only touch skipped pixels.
        for x in (start..=end).step_by(skip as usize) {
            let source = offset(x, y);
            let bytes = format.size.min(3);
            for gap in 1..skip {
                if x + gap <= bounds.x2 {
                    data.copy_within(source..source + bytes, offset(x + gap, y));
                }
            }
        }
        let line_length = (end - start + skip).min(bounds.x2 - start + 1) as usize * format.size;
        let source = offset(start, y);
        for gap in 1..skip {
            if y + gap <= bounds.y2 {
                data.copy_within(source..source + line_length, offset(start, y + gap));
            }
        }
    }
}

const INTENSITY_BITS: u32 = 12;
const INTENSITY_MAX: u32 = 1 << INTENSITY_BITS;

#[derive(Clone, Copy)]
struct PixelFormat {
    size: usize,
    swapped: bool,
}

impl PixelFormat {
    fn channels(self, data: &[u8], offset: usize) -> [u32; 3] {
        if self.size == 2 {
            let mut value = u16::from_le_bytes([data[offset], data[offset + 1]]);
            if self.swapped {
                value = value.swap_bytes();
            }
            [
                u32::from(value >> 11),
                u32::from((value >> 5) & 63),
                u32::from(value & 31),
            ]
        } else if self.size == 1 {
            [u32::from(data[offset]), 0, 0]
        } else {
            [
                u32::from(data[offset]),
                u32::from(data[offset + 1]),
                u32::from(data[offset + 2]),
            ]
        }
    }

    fn write(self, data: &mut [u8], offset: usize, sum: [u32; 3]) {
        if self.size == 2 {
            let r = (sum[0] + INTENSITY_MAX / 2) >> INTENSITY_BITS;
            let g = (sum[1] + INTENSITY_MAX / 2) >> INTENSITY_BITS;
            let b = (sum[2] + INTENSITY_MAX / 2) >> INTENSITY_BITS;
            let mut value = ((r << 11) | (g << 5) | b) as u16;
            if self.swapped {
                value = value.swap_bytes();
            }
            data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        } else {
            for channel in 0..self.size.min(3) {
                data[offset + channel] = (sum[channel] >> INTENSITY_BITS) as u8;
            }
        }
    }

    fn packed(self, data: &[u8], offset: usize) -> u16 {
        if self.size == 1 {
            u16::from(data[offset])
        } else {
            u16::from_le_bytes([data[offset], data[offset + 1]])
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pass(
    data: &mut [u8],
    start: usize,
    step: isize,
    count: usize,
    samples: usize,
    intensity: u32,
    format: PixelFormat,
) {
    let mut sum = [0u32; 3];
    for index in 0..samples {
        let pixel = format.channels(data, start.wrapping_add_signed(step * index as isize));
        for channel in 0..3 {
            sum[channel] += pixel[channel];
        }
    }
    for value in &mut sum {
        *value = (*value << INTENSITY_BITS) / samples as u32;
    }
    let first = format.packed(data, start);
    let mut previous = if format.size == 1 {
        u16::from((first as u8).wrapping_add(1))
    } else {
        first.wrapping_add(1)
    };
    for index in 0..count {
        let offset = start.wrapping_add_signed(step * index as isize);
        if format.size <= 2 && previous == format.packed(data, offset) {
            continue;
        }
        let pixel = format.channels(data, offset);
        for channel in 0..3 {
            sum[channel] = (sum[channel].wrapping_mul(intensity) >> INTENSITY_BITS)
                + pixel[channel] * (INTENSITY_MAX - intensity);
        }
        format.write(data, offset, sum);
        if format.size <= 2 {
            previous = format.packed(data, offset);
        }
    }
}

fn rounded_edge(start: i32, end: i32, point: i32, radius: i32) -> i32 {
    let distance = if point < start + radius {
        radius - (point - start)
    } else if point > end - radius {
        radius - (end - point)
    } else {
        return 0;
    };
    radius - sqrt32((radius * radius - distance * distance) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::draw::DrawTaskType;
    use ferrovgl_core::draw::draw_buf::DrawBuf;

    fn render(cf: ColorFormat, case: usize) -> alloc::vec::Vec<u8> {
        let size = if case == 4 { (180, 180) } else { (19, 17) };
        let stride = size.0 * u32::from(cf.size()) + 4;
        let length = (stride * size.1) as usize;
        let mut buffer = DrawBuf::create(size.0, size.1, cf, stride).unwrap();
        for (index, byte) in buffer.data_mut().unwrap()[..length].iter_mut().enumerate() {
            *byte = (index * 37 + index / 7 * 13) as u8;
        }
        let bounds = Area::new(-3, 5, size.0 as i32 - 4, size.1 as i32 + 4);
        let area = if case == 4 {
            bounds
        } else {
            Area::new(-1, 6, 13, 19)
        };
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, cf, &bounds);
        ctx.layer_mut(layer).unwrap().draw_buf = Some(buffer);
        let task = ctx.add_task(layer, &area, DrawTaskType::Blur);
        let task = ctx.task(task).unwrap().clone();
        let mut dsc = BlurDsc::new();
        dsc.blur_radius = [6, 6, 12, 24, 32][case];
        dsc.corner_radius = if case == 0 || case == 4 { 0 } else { 3 };
        dsc.quality = match case {
            0 | 1 => BlurQuality::Precision,
            3 => BlurQuality::Speed,
            _ => BlurQuality::Auto,
        };
        blur(&mut SwDrawUnit::default(), &mut ctx, &task, &dsc, &area);
        ctx.layer(layer).unwrap().draw_buf.as_ref().unwrap().data()[..length].to_vec()
    }

    #[test]
    fn four_pass_bytes_match_pinned_c_across_formats_quality_corners_and_padding() {
        // FNV-1a of every byte (including row padding), generated by executing
        // the unchanged v9.6.0 lv_draw_sw_blur.c with the same deterministic
        // input as render(). Cases: precision square/rounded, auto skip=2,
        // speed skip=3 rounded, and large auto skip=3. No PNG-derived pixels.
        let cases = [
            (
                ColorFormat::L8,
                [
                    17285478512405801162,
                    7197548304240186249,
                    6120770226853328909,
                    577735864905562249,
                    12999436028610627557,
                ],
            ),
            (
                ColorFormat::Rgb565,
                [
                    17915790868355595027,
                    10710187290170163641,
                    18446616960912037147,
                    14757386363933106587,
                    16207437187809514489,
                ],
            ),
            (
                ColorFormat::Rgb888,
                [
                    14623521455367057023,
                    4735471052848507481,
                    15258507398458778391,
                    12070057775552161415,
                    6052534705303578552,
                ],
            ),
            (
                ColorFormat::Argb8888,
                [
                    1216977799925068647,
                    13825972957818427799,
                    11194014758319866088,
                    16225276976200683288,
                    16648356037691377155,
                ],
            ),
            (
                ColorFormat::Rgb565Swapped,
                [
                    8504902971715290943,
                    8549376744547107615,
                    7940990665535156947,
                    9728722855633860251,
                    875512956478449185,
                ],
            ),
        ];
        for (format, expected) in cases {
            for (case, expected) in expected.into_iter().enumerate() {
                let hash = render(format, case)
                    .into_iter()
                    .fold(14695981039346656037u64, |hash, byte| {
                        (hash ^ u64::from(byte)).wrapping_mul(1099511628211)
                    });
                assert_eq!(hash, expected, "{format:?}, case {case}");
            }
        }
    }

    #[test]
    fn precision_preserves_alpha_and_speed_repeats_whole_rows_like_c() {
        let original = |index: usize| (index * 37 + index / 7 * 13) as u8;
        let stride = 19 * 4 + 4;
        let precise = render(ColorFormat::Argb8888, 1);
        for y in 0..17 {
            for x in 0..19 {
                let offset = y * stride + x * 4 + 3;
                assert_eq!(precise[offset], original(offset));
            }
        }
        let quick = render(ColorFormat::Argb8888, 3);
        // The center row samples x=3,6,9,12 and fills through x=14.
        for y in [7, 8] {
            assert_eq!(
                &quick[y * stride + 3 * 4..y * stride + 15 * 4],
                &quick[6 * stride + 3 * 4..6 * stride + 15 * 4]
            );
        }
    }

    #[test]
    fn tiny_speed_buffer_stays_inside_the_allocated_pixel_area() {
        let mut ctx = DrawCtx::new();
        let area = Area::new(0, 0, 0, 0);
        let layer = ctx.layer_create(None, ColorFormat::Rgb888, &area);
        let mut buffer = DrawBuf::create(1, 1, ColorFormat::Rgb888, 0).unwrap();
        buffer.data_mut().unwrap()[..3].copy_from_slice(&[17, 90, 231]);
        ctx.layer_mut(layer).unwrap().draw_buf = Some(buffer);
        let task = ctx.add_task(layer, &area, DrawTaskType::Blur);
        let task = ctx.task(task).unwrap().clone();
        let mut dsc = BlurDsc::new();
        dsc.quality = BlurQuality::Speed;
        dsc.blur_radius = 24;
        blur(&mut SwDrawUnit::default(), &mut ctx, &task, &dsc, &area);
        assert_eq!(
            &ctx.layer(layer).unwrap().draw_buf.as_ref().unwrap().data()[..3],
            &[17, 90, 231]
        );
    }
}
