//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_box_shadow.c`.
//!
//! Integer box blur based shadow with the `LV_DRAW_SW_SHADOW_CACHE_SIZE` cache.

use alloc::vec::Vec;

use ferrovgl_core::draw::rect::BoxShadowDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;

use crate::SwDrawUnit;

/// `lv_draw_sw_shadow_cache_t`
#[derive(Clone, Debug, Default)]
pub struct ShadowCache {
    /// `cache`: the cached corner (`cache_size * cache_size` bytes).
    pub cache: Vec<u8>,
    /// `cache_size`
    pub size: i32,
    /// `cache_r`
    pub r: i32,
}

/// `lv_draw_sw_box_shadow()`
pub fn box_shadow(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &BoxShadowDsc, coords: &Area) {
    todo!()
}

/// `shadow_draw_corner_buf()`: render the up-sampled 16-bit corner into
/// the caller-provided `sh_buf`, blur it in place with
/// [`shadow_blur_corner`] and down-convert to 8 bit inside (the cache
/// lookup stays in [`box_shadow`], as upstream).
pub fn shadow_draw_corner_buf(coords: &Area, sh_buf: &mut [u16], sw: i32, r: i32) {
    todo!()
}

/// `shadow_blur_corner()`
pub fn shadow_blur_corner(size: i32, sw: i32, sh_ups_buf: &mut [u16]) {
    todo!()
}
