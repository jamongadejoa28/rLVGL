//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_utils.h`, `src/draw/lv_draw_utils.c`.
//!
//! Pixel buffer helpers: I1 conversion, RGB565 byte swapping and buffer
//! rotation used by the display flush path.
//!
//! Buffers are byte slices. Upstream's `uint32_t *` / `uint16_t *` views
//! become 4 / 2 byte chunks addressed with the very same index arithmetic
//! (strides are divided by the pixel size with integer division, exactly like
//! `src_stride /= sizeof(uint32_t)`), so odd strides truncate the same way.
//! A `uint32_t` colour store is a native-endian 4 byte store. The optional
//! draw unit accelerators (`LV_DRAW_RGB565_SWAP`, `LV_DRAW_ROTATE*_*`) are
//! not defined for the software renderer, so every function runs upstream's
//! generic path.

use crate::display::Rotation;
use crate::misc::color::ColorFormat;

/// `lv_draw_i1_to_argb8888()`: convert an I1 buffer to ARGB8888.
///
/// `width` must be a multiple of 8 (only `width / 8` bytes of every row are
/// read), the strides are in bytes and `index0_color` / `index1_color` are
/// the `uint32_t` colours of the 0 and 1 bits.
pub fn i1_to_argb8888(
    buf_i1: &[u8],
    buf_argb8888: &mut [u8],
    width: u32,
    height: u32,
    buf_i1_stride: u32,
    buf_argb8888_stride: u32,
    index0_color: u32,
    index1_color: u32,
) {
    /*Extract the bits of I1 px_map and convert them to ARGB8888*/
    let mut src = 0usize; /*byte offset of the current source row*/
    let mut dst = 0usize; /*byte offset of the current destination row*/
    let i1_row_byte_count = (width / 8) as usize;
    for _row in 0..height {
        let mut dst_p = dst;
        for i in 0..i1_row_byte_count {
            let byte = buf_i1[src + i];
            /*From MSB to LSB (pixel 0 to pixel 7 in a byte)*/
            for bit in (0..8).rev() {
                let color = if (byte >> bit) & 1 != 0 {
                    index1_color
                } else {
                    index0_color
                };
                buf_argb8888[dst_p..dst_p + 4].copy_from_slice(&color.to_ne_bytes());
                dst_p += 4;
            }
        }
        src += buf_i1_stride as usize;
        /*`dst += buf_argb8888_stride / 4` on a `uint32_t *`*/
        dst += (buf_argb8888_stride / 4) as usize * 4;
    }
}

/// `lv_draw_rgb565_swap()`: swap the upper and lower byte of `buf_size_px`
/// RGB565 pixels in place.
///
/// Upstream swaps 32 bit words where the pointer alignment allows and handles
/// the odd leading / trailing pixel on its own; the outcome is every pixel's
/// two bytes exchanged, which is what the byte view does directly.
pub fn rgb565_swap(buf: &mut [u8], buf_size_px: u32) {
    for px in buf.as_chunks_mut::<2>().0.iter_mut().take(buf_size_px as usize) {
        px.swap(0, 1);
    }
}

/// `lv_draw_i1_invert()`: invert `buf_size` bytes of an I1 buffer in place.
///
/// Upstream inverts the unaligned head byte-wise, the middle in `uint32_t`
/// words and the tail byte-wise again; every byte ends up complemented.
pub fn i1_invert(buf: &mut [u8], buf_size: u32) {
    for byte in buf.iter_mut().take(buf_size as usize) {
        *byte = !*byte;
    }
}

/// `lv_draw_i1_convert_to_vtiled()`: convert an I1 buffer from htiled
/// (row-wise) to vtiled (column-wise) layout.
///
/// `width` and `height` must be multiples of 8, `buf_size >= width / 8 *
/// height` and `out_buf_size >= buf_size`, otherwise nothing is done
/// (`LV_CHECK_ARG`). The first `out_buf_size` bytes of `out_buf` are cleared
/// first.
pub fn i1_convert_to_vtiled(
    buf: &[u8],
    buf_size: u32,
    width: u32,
    height: u32,
    out_buf: &mut [u8],
    out_buf_size: u32,
    bit_order_lsb: bool,
) {
    if !(width.is_multiple_of(8) && height.is_multiple_of(8)) {
        return;
    }
    if buf_size < (width / 8).wrapping_mul(height) {
        return;
    }
    if out_buf_size < buf_size {
        return;
    }

    out_buf[..out_buf_size as usize].fill(0); /*lv_memset(out_buf, 0, out_buf_size)*/

    for y in 0..height {
        for x in 0..width {
            let src_index = y.wrapping_mul(width).wrapping_add(x);
            let dst_index = x.wrapping_mul(height).wrapping_add(y);
            let bit = (buf[(src_index / 8) as usize] >> (7 - (src_index % 8))) & 0x01;
            if bit_order_lsb {
                out_buf[(dst_index / 8) as usize] |= bit << (dst_index % 8);
            } else {
                out_buf[(dst_index / 8) as usize] |= bit << (7 - (dst_index % 8));
            }
        }
    }
}

/// `lv_draw_rotate()`: rotate `src` into `dest`.
///
/// `src_width` / `src_height` are in pixels, the strides in bytes. Only
/// `L8`, `RGB565`, `RGB888` and `XRGB8888` / `ARGB8888` /
/// `ARGB8888_PREMULTIPLIED` are handled; other formats and
/// [`Rotation::R0`] leave `dest` untouched.
pub fn rotate(
    src: &[u8],
    dest: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
    rotation: Rotation,
    color_format: ColorFormat,
) {
    if rotation == Rotation::R90 {
        match color_format {
            ColorFormat::L8 => rotate90_l8(src, dest, src_width, src_height, src_stride, dest_stride),
            ColorFormat::Rgb565 => rotate90_rgb565(src, dest, src_width, src_height, src_stride, dest_stride),
            ColorFormat::Rgb888 => rotate90_rgb888(src, dest, src_width, src_height, src_stride, dest_stride),
            ColorFormat::Xrgb8888 | ColorFormat::Argb8888 | ColorFormat::Argb8888Premultiplied => {
                rotate90_argb8888(src, dest, src_width, src_height, src_stride, dest_stride)
            }
            _ => {}
        }

        return;
    }

    if rotation == Rotation::R180 {
        match color_format {
            ColorFormat::L8 => rotate180_l8(src, dest, src_width, src_height, src_stride, dest_stride),
            ColorFormat::Rgb565 => {
                rotate180_rgb565(src, dest, src_width, src_height, src_stride, dest_stride)
            }
            ColorFormat::Rgb888 => {
                rotate180_rgb888(src, dest, src_width, src_height, src_stride, dest_stride)
            }
            ColorFormat::Xrgb8888 | ColorFormat::Argb8888 | ColorFormat::Argb8888Premultiplied => {
                rotate180_argb8888(src, dest, src_width, src_height, src_stride, dest_stride)
            }
            _ => {}
        }

        return;
    }

    if rotation == Rotation::R270 {
        match color_format {
            ColorFormat::L8 => rotate270_l8(src, dest, src_width, src_height, src_stride, dest_stride),
            ColorFormat::Rgb565 => {
                rotate270_rgb565(src, dest, src_width, src_height, src_stride, dest_stride)
            }
            ColorFormat::Rgb888 => {
                rotate270_rgb888(src, dest, src_width, src_height, src_stride, dest_stride)
            }
            ColorFormat::Xrgb8888 | ColorFormat::Argb8888 | ColorFormat::Argb8888Premultiplied => {
                rotate270_argb8888(src, dest, src_width, src_height, src_stride, dest_stride)
            }
            _ => {}
        }
    }
}

/**********************
 *   STATIC FUNCTIONS
 **********************/

/// Copy one `N` byte pixel: `dst[dst_index] = src[src_index]` on `N` byte
/// element views of the slices.
#[inline(always)]
fn copy_px<const N: usize>(src: &[u8], src_index: i32, dst: &mut [u8], dst_index: i32) {
    let s = src_index as usize * N;
    let d = dst_index as usize * N;
    dst[d..d + N].copy_from_slice(&src[s..s + N]);
}

/// Body of `rotate270_argb8888()` / `rotate270_rgb565()` / `rotate270_l8()`
/// on `N` byte pixels; the strides are already in pixel units.
fn rotate270_px<const N: usize>(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dst_stride: i32,
) {
    for x in 0..src_width {
        let dst_index = x * dst_stride;
        let mut src_index = x;
        for y in 0..src_height {
            copy_px::<N>(src, src_index, dst, dst_index + (src_height - y - 1));
            src_index += src_stride;
        }
    }
}

/// Body of `rotate180_argb8888()` / `rotate180_rgb565()` / `rotate180_l8()`
/// on `N` byte pixels; the strides are already in pixel units.
fn rotate180_px<const N: usize>(
    src: &[u8],
    dst: &mut [u8],
    width: i32,
    height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    for y in 0..height {
        let dst_index = (height - y - 1) * dest_stride;
        let src_index = y * src_stride;
        for x in 0..width {
            copy_px::<N>(src, src_index + x, dst, dst_index + width - x - 1);
        }
    }
}

/// Body of `rotate90_argb8888()` / `rotate90_rgb565()` / `rotate90_l8()`
/// on `N` byte pixels; the strides are already in pixel units.
fn rotate90_px<const N: usize>(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dst_stride: i32,
) {
    for x in 0..src_width {
        let dst_index = src_width - x - 1;
        let mut src_index = x;
        for y in 0..src_height {
            copy_px::<N>(src, src_index, dst, dst_index * dst_stride + y);
            src_index += src_stride;
        }
    }
}

/// `rotate270_argb8888()`: strides in bytes (`/= sizeof(uint32_t)`).
pub fn rotate270_argb8888(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    rotate270_px::<4>(src, dst, src_width, src_height, src_stride / 4, dest_stride / 4);
}

/// `rotate180_argb8888()`: strides in bytes (`/= sizeof(uint32_t)`).
pub fn rotate180_argb8888(
    src: &[u8],
    dst: &mut [u8],
    width: i32,
    height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    rotate180_px::<4>(src, dst, width, height, src_stride / 4, dest_stride / 4);
}

/// `rotate90_argb8888()`: strides in bytes (`/= sizeof(uint32_t)`).
pub fn rotate90_argb8888(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    rotate90_px::<4>(src, dst, src_width, src_height, src_stride / 4, dest_stride / 4);
}

/// `rotate90_rgb888()`: strides in bytes.
pub fn rotate90_rgb888(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    for x in 0..src_width {
        for y in 0..src_height {
            let src_index = (y * src_stride + x * 3) as usize;
            let dst_index = ((src_width - x - 1) * dest_stride + y * 3) as usize;
            dst[dst_index] = src[src_index]; /*Red*/
            dst[dst_index + 1] = src[src_index + 1]; /*Green*/
            dst[dst_index + 2] = src[src_index + 2]; /*Blue*/
        }
    }
}

/// `rotate180_rgb888()`: strides in bytes.
pub fn rotate180_rgb888(
    src: &[u8],
    dst: &mut [u8],
    width: i32,
    height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    for y in 0..height {
        for x in 0..width {
            let src_index = (y * src_stride + x * 3) as usize;
            let dst_index = ((height - y - 1) * dest_stride + (width - x - 1) * 3) as usize;
            dst[dst_index] = src[src_index];
            dst[dst_index + 1] = src[src_index + 1];
            dst[dst_index + 2] = src[src_index + 2];
        }
    }
}

/// `rotate270_rgb888()`: strides in bytes.
pub fn rotate270_rgb888(
    src: &[u8],
    dst: &mut [u8],
    width: i32,
    height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    for x in 0..width {
        for y in 0..height {
            let src_index = (y * src_stride + x * 3) as usize;
            let dst_index = (x * dest_stride + (height - y - 1) * 3) as usize;
            dst[dst_index] = src[src_index]; /*Red*/
            dst[dst_index + 1] = src[src_index + 1]; /*Green*/
            dst[dst_index + 2] = src[src_index + 2]; /*Blue*/
        }
    }
}

/// `rotate270_rgb565()`: strides in bytes (`/= sizeof(uint16_t)`).
pub fn rotate270_rgb565(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    rotate270_px::<2>(src, dst, src_width, src_height, src_stride / 2, dest_stride / 2);
}

/// `rotate180_rgb565()`: strides in bytes (`/= sizeof(uint16_t)`).
pub fn rotate180_rgb565(
    src: &[u8],
    dst: &mut [u8],
    width: i32,
    height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    rotate180_px::<2>(src, dst, width, height, src_stride / 2, dest_stride / 2);
}

/// `rotate90_rgb565()`: strides in bytes (`/= sizeof(uint16_t)`).
pub fn rotate90_rgb565(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    rotate90_px::<2>(src, dst, src_width, src_height, src_stride / 2, dest_stride / 2);
}

/// `rotate90_l8()`: strides in bytes.
pub fn rotate90_l8(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    rotate90_px::<1>(src, dst, src_width, src_height, src_stride, dest_stride);
}

/// `rotate180_l8()`: strides in bytes.
pub fn rotate180_l8(src: &[u8], dst: &mut [u8], width: i32, height: i32, src_stride: i32, dest_stride: i32) {
    rotate180_px::<1>(src, dst, width, height, src_stride, dest_stride);
}

/// `rotate270_l8()`: strides in bytes.
pub fn rotate270_l8(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    rotate270_px::<1>(src, dst, src_width, src_height, src_stride, dest_stride);
}

/// Ported from `tests/src/test_cases/draw/test_draw_sw_post_process.c`
/// (the rotate / invert / vtile cases) plus small invariants.
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    /// A `uint16_t[]` as the bytes it occupies in memory.
    fn u16s(v: &[u16]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_ne_bytes()).collect()
    }

    /// A `uint32_t[]` as the bytes it occupies in memory.
    fn u32s(v: &[u32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_ne_bytes()).collect()
    }

    #[test]
    fn test_rotate90_rgb565() {
        let src = u16s(&[0x1110, 0x2220, 0x3330, 0x4440, 0x5550, 0x6660]);
        let mut dst = vec![0u8; 2 * 3 * 2];
        let expected = u16s(&[0x3330, 0x6660, 0x2220, 0x5550, 0x1110, 0x4440]);
        rotate(
            &src,
            &mut dst,
            3,
            2,
            3 * 2,
            2 * 2,
            Rotation::R90,
            ColorFormat::Rgb565,
        );
        assert_eq!(dst, expected);
    }

    #[test]
    fn test_rotate180_rgb565() {
        let src = u16s(&[0x1110, 0x2220, 0x3330, 0x4440, 0x5550, 0x6660]);
        let mut dst = vec![0u8; 3 * 2 * 2];
        let expected = u16s(&[0x6660, 0x5550, 0x4440, 0x3330, 0x2220, 0x1110]);
        rotate(
            &src,
            &mut dst,
            3,
            2,
            3 * 2,
            3 * 2,
            Rotation::R180,
            ColorFormat::Rgb565,
        );
        assert_eq!(dst, expected);
    }

    #[test]
    fn test_rotate270_rgb565() {
        let src = u16s(&[0x1110, 0x2220, 0x3330, 0x4440, 0x5550, 0x6660]);
        let mut dst = vec![0u8; 2 * 3 * 2];
        let expected = u16s(&[0x4440, 0x1110, 0x5550, 0x2220, 0x6660, 0x3330]);
        rotate(
            &src,
            &mut dst,
            3,
            2,
            3 * 2,
            2 * 2,
            Rotation::R270,
            ColorFormat::Rgb565,
        );
        assert_eq!(dst, expected);
    }

    const RGB888_SRC: [u8; 18] = [
        0x11, 0x1A, 0x1B, 0x22, 0x2A, 0x2B, 0x33, 0x3A, 0x3B, //
        0x44, 0x4A, 0x4B, 0x55, 0x5A, 0x5B, 0x66, 0x6A, 0x6B,
    ];

    #[test]
    fn test_rotate90_rgb888() {
        let mut dst = [0u8; 2 * 3 * 3];
        let expected: [u8; 18] = [
            0x33, 0x3A, 0x3B, 0x66, 0x6A, 0x6B, //
            0x22, 0x2A, 0x2B, 0x55, 0x5A, 0x5B, //
            0x11, 0x1A, 0x1B, 0x44, 0x4A, 0x4B,
        ];
        rotate(
            &RGB888_SRC,
            &mut dst,
            3,
            2,
            3 * 3,
            2 * 3,
            Rotation::R90,
            ColorFormat::Rgb888,
        );
        assert_eq!(dst, expected);
    }

    #[test]
    fn test_rotate180_rgb888() {
        let mut dst = [0u8; 3 * 2 * 3];
        let expected: [u8; 18] = [
            0x66, 0x6A, 0x6B, 0x55, 0x5A, 0x5B, 0x44, 0x4A, 0x4B, //
            0x33, 0x3A, 0x3B, 0x22, 0x2A, 0x2B, 0x11, 0x1A, 0x1B,
        ];
        /*Upstream passes 2x3 with a 6 byte stride*/
        rotate(
            &RGB888_SRC,
            &mut dst,
            2,
            3,
            2 * 3,
            2 * 3,
            Rotation::R180,
            ColorFormat::Rgb888,
        );
        assert_eq!(dst, expected);
    }

    #[test]
    fn test_rotate270_rgb888() {
        let mut dst = [0u8; 2 * 3 * 3];
        let expected: [u8; 18] = [
            0x44, 0x4A, 0x4B, 0x11, 0x1A, 0x1B, //
            0x55, 0x5A, 0x5B, 0x22, 0x2A, 0x2B, //
            0x66, 0x6A, 0x6B, 0x33, 0x3A, 0x3B,
        ];
        rotate(
            &RGB888_SRC,
            &mut dst,
            3,
            2,
            3 * 3,
            2 * 3,
            Rotation::R270,
            ColorFormat::Rgb888,
        );
        assert_eq!(dst, expected);
    }

    #[test]
    fn test_rotate90_argb8888() {
        let src = u32s(&[
            0x111A1B1C, 0x222A2B2C, 0x333A3B3C, 0x444A4B4C, 0x555A5B5C, 0x666A6B6C,
        ]);
        let mut dst = vec![0u8; 2 * 3 * 4];
        let expected = u32s(&[
            0x333A3B3C, 0x666A6B6C, 0x222A2B2C, 0x555A5B5C, 0x111A1B1C, 0x444A4B4C,
        ]);
        rotate(
            &src,
            &mut dst,
            3,
            2,
            3 * 4,
            2 * 4,
            Rotation::R90,
            ColorFormat::Argb8888,
        );
        assert_eq!(dst, expected);
    }

    #[test]
    fn test_rotate180_argb8888() {
        let src = u32s(&[
            0xFF0000FF, 0xFF00FF00, 0xFFFF0000, 0xFFFFFFFF, 0xFF00FFFF, 0xFFFFFF00,
        ]);
        let mut dst = vec![0u8; 3 * 2 * 4];
        let expected = u32s(&[
            0xFFFFFF00, 0xFF00FFFF, 0xFFFFFFFF, 0xFFFF0000, 0xFF00FF00, 0xFF0000FF,
        ]);
        rotate(
            &src,
            &mut dst,
            2,
            3,
            2 * 4,
            2 * 4,
            Rotation::R180,
            ColorFormat::Argb8888,
        );
        assert_eq!(dst, expected);
    }

    #[test]
    fn test_rotate270_argb8888() {
        let src = u32s(&[
            0x111A1B1C, 0x222A2B2C, 0x333A3B3C, 0x444A4B4C, 0x555A5B5C, 0x666A6B6C,
        ]);
        let mut dst = vec![0u8; 2 * 3 * 4];
        let expected = u32s(&[
            0x444A4B4C, 0x111A1B1C, 0x555A5B5C, 0x222A2B2C, 0x666A6B6C, 0x333A3B3C,
        ]);
        rotate(
            &src,
            &mut dst,
            3,
            2,
            3 * 4,
            2 * 4,
            Rotation::R270,
            ColorFormat::Argb8888,
        );
        assert_eq!(dst, expected);
    }

    const L8_SRC: [u8; 6] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];

    #[test]
    fn test_rotate90_l8() {
        let mut dst = [0u8; 2 * 3];
        rotate(&L8_SRC, &mut dst, 3, 2, 3, 2, Rotation::R90, ColorFormat::L8);
        assert_eq!(dst, [0x33, 0x66, 0x22, 0x55, 0x11, 0x44]);
    }

    #[test]
    fn test_rotate180_l8() {
        let mut dst = [0u8; 3 * 2];
        rotate(&L8_SRC, &mut dst, 3, 2, 3, 3, Rotation::R180, ColorFormat::L8);
        assert_eq!(dst, [0x66, 0x55, 0x44, 0x33, 0x22, 0x11]);
    }

    #[test]
    fn test_rotate270_l8() {
        let mut dst = [0u8; 2 * 3];
        rotate(&L8_SRC, &mut dst, 3, 2, 3, 2, Rotation::R270, ColorFormat::L8);
        assert_eq!(dst, [0x44, 0x11, 0x55, 0x22, 0x66, 0x33]);
    }

    #[test]
    fn rotate_handles_padded_strides_and_premultiplied_alias() {
        /*2x2 XRGB8888 with a 12 byte source stride (one padding pixel) and
         *a 12 byte destination stride; padding must stay untouched*/
        let src = u32s(&[1, 2, 0xAAAA, 3, 4, 0xBBBB]);
        let mut dst = u32s(&[0, 0, 0xCCCC, 0, 0, 0xDDDD]);
        rotate(
            &src,
            &mut dst,
            2,
            2,
            12,
            12,
            Rotation::R90,
            ColorFormat::Argb8888Premultiplied,
        );
        assert_eq!(dst, u32s(&[2, 4, 0xCCCC, 1, 3, 0xDDDD]));
        let mut dst = u32s(&[0, 0, 0xCCCC, 0, 0, 0xDDDD]);
        rotate(
            &src,
            &mut dst,
            2,
            2,
            12,
            12,
            Rotation::R180,
            ColorFormat::Xrgb8888,
        );
        assert_eq!(dst, u32s(&[4, 3, 0xCCCC, 2, 1, 0xDDDD]));
        let mut dst = u32s(&[0, 0, 0xCCCC, 0, 0, 0xDDDD]);
        rotate(
            &src,
            &mut dst,
            2,
            2,
            12,
            12,
            Rotation::R270,
            ColorFormat::Argb8888,
        );
        assert_eq!(dst, u32s(&[3, 1, 0xCCCC, 4, 2, 0xDDDD]));

        /*Rotation 0 and unsupported formats are no-ops*/
        let mut dst = u32s(&[0, 0, 0, 0, 0, 0]);
        rotate(&src, &mut dst, 2, 2, 12, 12, Rotation::R0, ColorFormat::Argb8888);
        assert_eq!(dst, u32s(&[0, 0, 0, 0, 0, 0]));
        rotate(&src, &mut dst, 2, 2, 12, 12, Rotation::R90, ColorFormat::I1);
        assert_eq!(dst, u32s(&[0, 0, 0, 0, 0, 0]));
    }

    #[test]
    fn test_invert() {
        let expected_buf: [u8; 10] = [0xff, 0xfe, 0xfd, 0xfc, 0xfb, 0xfa, 0xf9, 0xf8, 0xf7, 0xf6];
        let init: [u8; 10] = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];

        /*Aligned start and aligned end*/
        let mut buf1 = init;
        i1_invert(&mut buf1, 8);
        assert_eq!(buf1[..8], expected_buf[..8]);
        assert_eq!(buf1[8..], init[8..]); /*not touched*/

        /*Unaligned start and unaligned end*/
        let mut buf2 = init;
        i1_invert(&mut buf2[3..], 6);
        assert_eq!(buf2[3..9], expected_buf[3..9]);
        assert_eq!(buf2[9], init[9]);

        /*Small buffer*/
        let mut buf3 = init;
        i1_invert(&mut buf3[3..], 2);
        assert_eq!(buf3[3..5], expected_buf[3..5]);
        assert_eq!(buf3[5..], init[5..]);
    }

    #[test]
    fn test_vtile_small() {
        let src_buf: [u8; 8] = [0x3C, 0x42, 0x81, 0xA5, 0x81, 0x81, 0x42, 0x3C];
        let mut dst_buf = [0u8; 8];

        let expected_buf_msb: [u8; 8] = [0x3C, 0x42, 0x91, 0x81, 0x81, 0x91, 0x42, 0x3C];
        let expected_buf_lsb: [u8; 8] = [0x3C, 0x42, 0x89, 0x81, 0x81, 0x89, 0x42, 0x3C];

        i1_convert_to_vtiled(&src_buf, 8, 8, 8, &mut dst_buf, 8, false);
        assert_eq!(dst_buf, expected_buf_msb);

        i1_convert_to_vtiled(&src_buf, 8, 8, 8, &mut dst_buf, 8, true);
        assert_eq!(dst_buf, expected_buf_lsb);
    }

    #[test]
    fn test_vtile_rectangular() {
        let src_buf: [u8; 80] = [
            0x00, 0x00, 0x00, 0x00, 0x00, //
            0x40, 0x80, 0x83, 0xE1, 0x00, //
            0x40, 0x80, 0x84, 0x01, 0x00, //
            0x40, 0x41, 0x08, 0x01, 0x00, //
            0x40, 0x41, 0x10, 0x01, 0x00, //
            0x40, 0x41, 0x20, 0x01, 0x00, //
            0x40, 0x41, 0x20, 0x01, 0x00, //
            0x40, 0x41, 0x20, 0x01, 0x00, //
            0x40, 0x41, 0x20, 0x01, 0x00, //
            0x40, 0x23, 0x20, 0x01, 0x00, //
            0x40, 0x22, 0x20, 0xF1, 0x00, //
            0x40, 0x14, 0x20, 0x11, 0x00, //
            0x40, 0x14, 0x10, 0x11, 0x00, //
            0x40, 0x08, 0x08, 0x11, 0x00, //
            0x7E, 0x08, 0x07, 0xF1, 0xF8, //
            0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut dst_buf = [0u8; 80];

        /*Upstream only compares the first 8 bytes of its 80 byte tables (its
         *MSB table is wrong further on). Column 15 is checked by hand instead:
         *the LSB of the second source byte of the 16 rows is
         *0,0,0,1,1,1,1,1, 1,1,0,0,0,0,0,0.*/
        let expected_buf_msb: [u8; 8] = [0x00, 0x00, 0x7F, 0xFE, 0x00, 0x02, 0x00, 0x02];
        let expected_buf_lsb: [u8; 8] = [0x00, 0x00, 0xFE, 0x7F, 0x00, 0x40, 0x00, 0x40];

        i1_convert_to_vtiled(&src_buf, 80, 40, 16, &mut dst_buf, 80, false);
        assert_eq!(dst_buf[..8], expected_buf_msb);
        assert_eq!(dst_buf[30..32], [0x1F, 0xC0]);

        i1_convert_to_vtiled(&src_buf, 80, 40, 16, &mut dst_buf, 80, true);
        assert_eq!(dst_buf[..8], expected_buf_lsb);
        assert_eq!(dst_buf[30..32], [0xF8, 0x03]);
    }

    #[test]
    fn vtile_rejects_bad_arguments_without_touching_the_output() {
        let src = [0xFFu8; 8];
        let mut dst = [0xAAu8; 8];
        i1_convert_to_vtiled(&src, 8, 7, 8, &mut dst, 8, false); /*width % 8*/
        i1_convert_to_vtiled(&src, 8, 8, 12, &mut dst, 8, false); /*height % 8*/
        i1_convert_to_vtiled(&src, 4, 8, 8, &mut dst, 8, false); /*buf_size too small*/
        i1_convert_to_vtiled(&src, 8, 8, 8, &mut dst, 4, false); /*out_buf_size < buf_size*/
        assert_eq!(dst, [0xAA; 8]);
        /*A valid call clears out_buf_size bytes first*/
        let mut dst = [0xAAu8; 10];
        i1_convert_to_vtiled(&[0u8; 8], 8, 8, 8, &mut dst, 9, false);
        assert_eq!(dst, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0xAA]);
    }

    #[test]
    fn i1_to_argb8888_expands_msb_first_and_honours_strides() {
        /*Two rows, 8 px wide, source stride 2 bytes (1 byte padding),
         *destination stride 36 bytes (one padding pixel)*/
        let src: [u8; 4] = [0b1010_0000, 0xEE, 0b0000_0001, 0xEE];
        let mut dst = vec![0u8; 2 * 36];
        i1_to_argb8888(&src, &mut dst, 8, 2, 2, 36, 0xFF000000, 0xFFFFFFFF);
        let px = |i: usize| u32::from_ne_bytes(dst[i * 4..i * 4 + 4].try_into().unwrap());
        let row0: Vec<u32> = (0..8).map(px).collect();
        assert_eq!(
            row0,
            [
                0xFFFFFFFF, 0xFF000000, 0xFFFFFFFF, 0xFF000000, 0xFF000000, 0xFF000000, 0xFF000000,
                0xFF000000
            ]
        );
        assert_eq!(px(8), 0); /*padding pixel untouched*/
        let row1: Vec<u32> = (9..17).map(px).collect();
        assert_eq!(
            row1,
            [
                0xFF000000, 0xFF000000, 0xFF000000, 0xFF000000, 0xFF000000, 0xFF000000, 0xFF000000,
                0xFFFFFFFF
            ]
        );

        /*A width that is not a multiple of 8 only converts the whole bytes*/
        let mut dst = vec![0u8; 64];
        i1_to_argb8888(&[0xFF, 0xFF], &mut dst, 12, 1, 2, 64, 1, 2);
        let px2 = |i: usize| u32::from_ne_bytes(dst[i * 4..i * 4 + 4].try_into().unwrap());
        assert!((0..8).all(|i| px2(i) == 2));
        assert!((8..16).all(|i| px2(i) == 0));
    }

    #[test]
    fn rgb565_swap_swaps_only_the_requested_pixels() {
        let mut buf: [u8; 8] = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        rgb565_swap(&mut buf, 3);
        assert_eq!(buf, [0x34, 0x12, 0x78, 0x56, 0xBC, 0x9A, 0xDE, 0xF0]);
        /*Even count and a count larger than the buffer*/
        rgb565_swap(&mut buf, 2);
        assert_eq!(buf, [0x12, 0x34, 0x56, 0x78, 0xBC, 0x9A, 0xDE, 0xF0]);
        rgb565_swap(&mut buf, 100);
        assert_eq!(buf, [0x34, 0x12, 0x78, 0x56, 0x9A, 0xBC, 0xF0, 0xDE]);
        rgb565_swap(&mut buf, 0);
        assert_eq!(buf, [0x34, 0x12, 0x78, 0x56, 0x9A, 0xBC, 0xF0, 0xDE]);
    }
}
