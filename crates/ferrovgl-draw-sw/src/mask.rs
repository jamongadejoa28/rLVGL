//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_mask.c`, `src/draw/sw/lv_draw_sw_mask.h`,
//! `src/draw/sw/lv_draw_sw_mask_private.h`.
//!
//! Line / angle / radius / fade / map masks and the circle cache used for
//! rounded corners.

use alloc::vec::Vec;

use ferrovgl_core::misc::area::{Area, Point};
use ferrovgl_core::misc::color::Opa;
use ferrovgl_core::misc::event::LvResult;

/// `LV_MASK_ID_INV`
pub const MASK_ID_INV: i32 = -1;
/// `LV_MASK_MAX_NUM` (`LV_DRAW_SW_COMPLEX`)
pub const MASK_MAX_NUM: usize = 16;
/// `LV_DRAW_SW_CIRCLE_CACHE_SIZE`
pub const CIRCLE_CACHE_SIZE: usize = 4;

/// `lv_draw_sw_mask_res_t`
///
/// The default is `Transp` (0): upstream `lv_memzero`s blend descriptors, so
/// a zeroed `mask_res` is `TRANSP` and `lv_draw_sw_blend` early-returns for
/// `mask_buf && mask_res == TRANSP`. Callers set `Changed` explicitly.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MaskRes {
    /// `LV_DRAW_SW_MASK_RES_TRANSP`
    #[default]
    Transp = 0,
    /// `LV_DRAW_SW_MASK_RES_FULL_COVER`
    FullCover = 1,
    /// `LV_DRAW_SW_MASK_RES_CHANGED`
    Changed = 2,
    /// `LV_DRAW_SW_MASK_RES_UNKNOWN`
    Unknown = 3,
}

/// `lv_draw_sw_mask_type_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MaskType {
    /// `LV_DRAW_SW_MASK_TYPE_LINE`
    #[default]
    Line = 0,
    /// `LV_DRAW_SW_MASK_TYPE_ANGLE`
    Angle = 1,
    /// `LV_DRAW_SW_MASK_TYPE_RADIUS`
    Radius = 2,
    /// `LV_DRAW_SW_MASK_TYPE_FADE`
    Fade = 3,
    /// `LV_DRAW_SW_MASK_TYPE_MAP`
    Map = 4,
}

/// `lv_draw_sw_mask_line_side_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LineSide {
    /// `LV_DRAW_SW_MASK_LINE_SIDE_LEFT`
    #[default]
    Left = 0,
    /// `LV_DRAW_SW_MASK_LINE_SIDE_RIGHT`
    Right = 1,
    /// `LV_DRAW_SW_MASK_LINE_SIDE_TOP`
    Top = 2,
    /// `LV_DRAW_SW_MASK_LINE_SIDE_BOTTOM`
    Bottom = 3,
}

/// `lv_draw_sw_mask_line_param_t`
#[derive(Clone, Debug, Default)]
pub struct MaskLineParam {
    /// `cfg.p1`
    pub p1: Point,
    /// `cfg.p2`
    pub p2: Point,
    /// `cfg.side`
    pub side: LineSide,
    /// `origo`: a point of the line.
    pub origo: Point,
    /// `xy_steep`: X / Y steepness (`* 1024`).
    pub xy_steep: i32,
    /// `yx_steep`: Y / X steepness (`* 1024`).
    pub yx_steep: i32,
    /// `steep`: helper, `xy_steep` or `yx_steep` depending on `flat`.
    pub steep: i32,
    /// `spx`: steepness in 1 px in 0..255.
    pub spx: i32,
    /// `flat`: 1 if the line is more horizontal than vertical.
    pub flat: bool,
    /// `inv`: invert the mask.
    pub inv: bool,
}

/// `lv_draw_sw_mask_angle_param_t`
#[derive(Clone, Debug, Default)]
pub struct MaskAngleParam {
    /// `cfg.vertex_p`
    pub vertex_p: Point,
    /// `cfg.start_angle`
    pub start_angle: i32,
    /// `cfg.end_angle`
    pub end_angle: i32,
    /// `start_line`
    pub start_line: MaskLineParam,
    /// `end_line`
    pub end_line: MaskLineParam,
    /// `delta_deg`
    pub delta_deg: u16,
}

/// `lv_draw_sw_mask_radius_circle_dsc_t`: a cached quarter circle.
///
/// Upstream's `buf` (the single allocation backing the three arrays) is
/// omitted: each array owns its `Vec`.
#[derive(Clone, Debug, Default)]
pub struct RadiusCircleDsc {
    /// `cir_opa`: opacity of the anti-aliased pixels.
    pub cir_opa: Vec<Opa>,
    /// `x_start_on_y`: first x with non-zero opacity per row.
    pub x_start_on_y: Vec<u16>,
    /// `opa_start_on_y`: index into `cir_opa` per row.
    pub opa_start_on_y: Vec<u16>,
    /// `life`: cache eviction counter (-1 = free).
    pub life: i32,
    /// `used_cnt`
    pub used_cnt: u32,
    /// `radius`
    pub radius: i32,
}

/// `lv_draw_sw_mask_radius_circle_dsc_arr_t`: the circle cache.
#[derive(Clone, Debug, Default)]
pub struct CircleCache {
    /// The entries.
    pub entries: [RadiusCircleDsc; CIRCLE_CACHE_SIZE],
}

/// `lv_draw_sw_mask_radius_param_t`
#[derive(Clone, Debug, Default)]
pub struct MaskRadiusParam {
    /// `cfg.rect`
    pub rect: Area,
    /// `cfg.radius`
    pub radius: i32,
    /// `cfg.outer`: keep the outside of the rounded rectangle.
    pub outer: bool,
    /// `circle`: index into the [`CircleCache`], or an owned circle for
    /// radii not worth caching.
    pub circle: CircleRef,
}

/// Where the circle data of a radius mask lives.
#[derive(Clone, Debug, Default)]
pub enum CircleRef {
    /// No circle (radius 0).
    #[default]
    None,
    /// Index into [`CircleCache::entries`].
    Cached(usize),
    /// A circle owned by the parameter.
    Owned(RadiusCircleDsc),
}

/// `lv_draw_sw_mask_fade_param_t`
#[derive(Clone, Debug, Default)]
pub struct MaskFadeParam {
    /// `cfg.coords`
    pub coords: Area,
    /// `cfg.y_top`
    pub y_top: i32,
    /// `cfg.y_bottom`
    pub y_bottom: i32,
    /// `cfg.opa_top`
    pub opa_top: Opa,
    /// `cfg.opa_bottom`
    pub opa_bottom: Opa,
}

/// `lv_draw_sw_mask_map_param_t`
#[derive(Clone, Debug)]
pub struct MaskMapParam<'a> {
    /// `cfg.coords`
    pub coords: Area,
    /// `cfg.map`: one opacity byte per pixel of `coords`.
    pub map: &'a [Opa],
}

/// Any mask parameter (`void * p` with its `lv_draw_sw_mask_common_dsc_t`).
#[derive(Clone, Debug)]
pub enum MaskParam<'a> {
    /// A line mask.
    Line(MaskLineParam),
    /// An angle mask.
    Angle(MaskAngleParam),
    /// A radius mask.
    Radius(MaskRadiusParam),
    /// A fade mask.
    Fade(MaskFadeParam),
    /// A map mask.
    Map(MaskMapParam<'a>),
}

impl MaskParam<'_> {
    /// `dsc.type`
    pub const fn mask_type(&self) -> MaskType {
        match self {
            MaskParam::Line(_) => MaskType::Line,
            MaskParam::Angle(_) => MaskType::Angle,
            MaskParam::Radius(_) => MaskType::Radius,
            MaskParam::Fade(_) => MaskType::Fade,
            MaskParam::Map(_) => MaskType::Map,
        }
    }
}

/// `lv_draw_sw_mask_init()`
pub fn init(cache: &mut CircleCache) {
    todo!()
}

/// `lv_draw_sw_mask_deinit()`
pub fn deinit(cache: &mut CircleCache) {
    todo!()
}

/// `lv_draw_sw_mask_apply()`: apply every mask in `masks` to `mask_buf`
/// (`len` pixels starting at absolute `(abs_x, abs_y)`).
pub fn apply(
    cache: &mut CircleCache,
    masks: &mut [MaskParam<'_>],
    mask_buf: &mut [Opa],
    abs_x: i32,
    abs_y: i32,
    len: i32,
) -> MaskRes {
    todo!()
}

/// `lv_draw_sw_mask_free_param()`: release a cached circle reference.
pub fn free_param(cache: &mut CircleCache, p: &mut MaskParam<'_>) {
    todo!()
}

/// `lv_draw_sw_mask_line_points_init()`
pub fn line_points_init(param: &mut MaskLineParam, p1x: i32, p1y: i32, p2x: i32, p2y: i32, side: LineSide) {
    todo!()
}

/// `lv_draw_sw_mask_line_angle_init()`
pub fn line_angle_init(param: &mut MaskLineParam, px: i32, py: i32, angle: i16, side: LineSide) {
    todo!()
}

/// `lv_draw_sw_mask_angle_init()`
pub fn angle_init(
    param: &mut MaskAngleParam,
    vertex_x: i32,
    vertex_y: i32,
    start_angle: i32,
    end_angle: i32,
) {
    todo!()
}

/// `lv_draw_sw_mask_radius_init()`
pub fn radius_init(
    cache: &mut CircleCache,
    param: &mut MaskRadiusParam,
    rect: &Area,
    radius: i32,
    inv: bool,
) -> LvResult {
    todo!()
}

/// `lv_draw_sw_mask_fade_init()`
pub fn fade_init(
    param: &mut MaskFadeParam,
    coords: &Area,
    opa_top: Opa,
    y_top: i32,
    opa_bottom: Opa,
    y_bottom: i32,
) {
    todo!()
}

/// `lv_draw_sw_mask_map_init()`
pub fn map_init<'a>(param: &mut MaskMapParam<'a>, coords: &Area, map: &'a [Opa]) {
    todo!()
}

/// `lv_draw_sw_mask_cleanup()`: drop unused circles from the cache.
pub fn cleanup(cache: &mut CircleCache) {
    todo!()
}

/// `lv_draw_sw_mask_line()`
pub fn mask_line(mask_buf: &mut [Opa], abs_x: i32, abs_y: i32, len: i32, p: &mut MaskLineParam) -> MaskRes {
    todo!()
}

/// `lv_draw_sw_mask_angle()`
pub fn mask_angle(mask_buf: &mut [Opa], abs_x: i32, abs_y: i32, len: i32, p: &mut MaskAngleParam) -> MaskRes {
    todo!()
}

/// `lv_draw_sw_mask_radius()`
pub fn mask_radius(
    cache: &CircleCache,
    mask_buf: &mut [Opa],
    abs_x: i32,
    abs_y: i32,
    len: i32,
    p: &mut MaskRadiusParam,
) -> MaskRes {
    todo!()
}

/// `lv_draw_sw_mask_fade()`
pub fn mask_fade(mask_buf: &mut [Opa], abs_x: i32, abs_y: i32, len: i32, p: &mut MaskFadeParam) -> MaskRes {
    todo!()
}

/// `lv_draw_sw_mask_map()`
pub fn mask_map(mask_buf: &mut [Opa], abs_x: i32, abs_y: i32, len: i32, p: &mut MaskMapParam<'_>) -> MaskRes {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_values() {
        assert_eq!(MaskRes::Changed as u8, 2);
        assert_eq!(MaskRes::default(), MaskRes::Transp);
        assert_eq!(MaskRes::default() as u8, 0);
        assert_eq!(MaskType::Map as u8, 4);
        assert_eq!(LineSide::Bottom as u8, 3);
        assert_eq!(CIRCLE_CACHE_SIZE, 4);
        assert_eq!(MASK_MAX_NUM, 16);
        let p = MaskParam::Fade(MaskFadeParam::default());
        assert_eq!(p.mask_type(), MaskType::Fade);
    }
}
