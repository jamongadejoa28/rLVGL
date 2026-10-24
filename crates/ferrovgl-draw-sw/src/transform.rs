//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_transform.c`.
//!
//! Rotation / scaling of images with nearest or bilinear sampling (the
//! software transform has no skew path upstream).

use ferrovgl_core::draw::image::{DrawImageDsc, DrawImageSup};
use ferrovgl_core::misc::area::{Area, Point};
use ferrovgl_core::misc::color::ColorFormat;

/// Per-pixel sampling data (`point_transform_dsc_t`).
#[derive(Clone, Debug, Default)]
pub struct PointTransfDsc {
    /// `sinma`
    pub sinma: i32,
    /// `cosma`
    pub cosma: i32,
    /// `scale_x`
    pub scale_x: i32,
    /// `scale_y`
    pub scale_y: i32,
    /// `angle`
    pub angle: i32,
    /// `pivot_x_256`
    pub pivot_x_256: i32,
    /// `pivot_y_256`
    pub pivot_y_256: i32,
    /// `pivot`
    pub pivot: Point,
}

/// `lv_draw_sw_transform()`: transform `src` (`src_w` x `src_h`, `src_cf`)
/// into `dest_buf` covering `dest_area`. The destination is always
/// ARGB8888 (or RGB565A8 / L8 / AL88 for matching sources, as upstream).
pub fn transform(
    dest_area: &Area,
    src_buf: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    draw_dsc: &DrawImageDsc,
    sup: Option<&DrawImageSup>,
    src_cf: ColorFormat,
    dest_buf: &mut [u8],
) {
    todo!()
}
