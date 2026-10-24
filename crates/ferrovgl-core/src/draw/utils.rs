//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_utils.h`, `src/draw/lv_draw_utils.c`.
//!
//! Pixel buffer helpers: I1 conversion, RGB565 byte swapping and buffer
//! rotation used by the display flush path.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::display::Rotation;
use crate::misc::color::ColorFormat;

/// `lv_draw_i1_to_argb8888()`
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
    todo!()
}

/// `lv_draw_rgb565_swap()`: swap the bytes of `buf_size_px` RGB565 pixels.
pub fn rgb565_swap(buf: &mut [u8], buf_size_px: u32) {
    todo!()
}

/// `lv_draw_i1_invert()`
pub fn i1_invert(buf: &mut [u8], buf_size: u32) {
    todo!()
}

/// `lv_draw_i1_convert_to_vtiled()`
pub fn i1_convert_to_vtiled(
    buf: &[u8],
    buf_size: u32,
    width: u32,
    height: u32,
    out_buf: &mut [u8],
    out_buf_size: u32,
    bit_order_lsb: bool,
) {
    todo!()
}

/// `lv_draw_rotate()`: rotate `src` into `dest` (ARGB8888 / RGB888 / RGB565 / L8).
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
    todo!()
}

/// `rotate90_argb8888()`
pub fn rotate90_argb8888(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    todo!()
}

/// `rotate180_argb8888()`
pub fn rotate180_argb8888(
    src: &[u8],
    dst: &mut [u8],
    width: i32,
    height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    todo!()
}

/// `rotate270_argb8888()`
pub fn rotate270_argb8888(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    todo!()
}

/// `rotate90_rgb888()`
pub fn rotate90_rgb888(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    todo!()
}

/// `rotate180_rgb888()`
pub fn rotate180_rgb888(
    src: &[u8],
    dst: &mut [u8],
    width: i32,
    height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    todo!()
}

/// `rotate270_rgb888()`
pub fn rotate270_rgb888(
    src: &[u8],
    dst: &mut [u8],
    width: i32,
    height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    todo!()
}

/// `rotate90_rgb565()`
pub fn rotate90_rgb565(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    todo!()
}

/// `rotate180_rgb565()`
pub fn rotate180_rgb565(
    src: &[u8],
    dst: &mut [u8],
    width: i32,
    height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    todo!()
}

/// `rotate270_rgb565()`
pub fn rotate270_rgb565(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    todo!()
}

/// `rotate90_l8()`
pub fn rotate90_l8(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    todo!()
}

/// `rotate180_l8()`
pub fn rotate180_l8(src: &[u8], dst: &mut [u8], width: i32, height: i32, src_stride: i32, dest_stride: i32) {
    todo!()
}

/// `rotate270_l8()`
pub fn rotate270_l8(
    src: &[u8],
    dst: &mut [u8],
    src_width: i32,
    src_height: i32,
    src_stride: i32,
    dest_stride: i32,
) {
    todo!()
}
