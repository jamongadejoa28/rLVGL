//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_mask.c`, `src/draw/sw/lv_draw_sw_mask.h`,
//! `src/draw/sw/lv_draw_sw_mask_private.h`.
//!
//! Line / angle / radius / fade / map masks and the circle cache used for
//! rounded corners.
//!
//! Upstream keeps the circle cache in `lv_global_t` behind
//! `circle_cache_mutex`; here it is a [`CircleCache`] owned by the caller (the
//! software draw unit) and passed explicitly, so there is no global state and
//! nothing to lock. The `void * masks[]` array terminated by `NULL` becomes a
//! slice of [`MaskParam`] and the per-type `dsc.cb` dispatch becomes a `match`.

use alloc::vec;
use alloc::vec::Vec;

use ferrovgl_core::misc::area::{Area, Point};
use ferrovgl_core::misc::color::{OPA_MAX, OPA_MIN, Opa};
use ferrovgl_core::misc::event::LvResult;
use ferrovgl_core::misc::math::{trigo_sin, udiv255};

/// `LV_MASK_ID_INV`
pub const MASK_ID_INV: i32 = -1;
/// `LV_MASK_MAX_NUM` (`LV_DRAW_SW_COMPLEX`)
pub const MASK_MAX_NUM: usize = 16;
/// `LV_DRAW_SW_CIRCLE_CACHE_SIZE`
pub const CIRCLE_CACHE_SIZE: usize = 4;

/// `CIRCLE_CACHE_LIFE_MAX`
const CIRCLE_CACHE_LIFE_MAX: i32 = 1000;

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

/*********************
 *   GLOBAL FUNCTIONS
 *********************/

/// `lv_draw_sw_mask_init()`
///
/// Upstream only creates `circle_cache_mutex`; the caller-owned
/// [`CircleCache`] needs no lock, so this is a no-op kept for API parity.
pub fn init(cache: &mut CircleCache) {
    let _ = cache;
}

/// `lv_draw_sw_mask_deinit()`
///
/// Upstream only deletes `circle_cache_mutex`; see [`init`].
pub fn deinit(cache: &mut CircleCache) {
    let _ = cache;
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
    let mut changed = false;

    for m in masks.iter_mut() {
        let res = match m {
            MaskParam::Line(p) => mask_line(mask_buf, abs_x, abs_y, len, p),
            MaskParam::Angle(p) => mask_angle(mask_buf, abs_x, abs_y, len, p),
            MaskParam::Radius(p) => mask_radius(cache, mask_buf, abs_x, abs_y, len, p),
            MaskParam::Fade(p) => mask_fade(mask_buf, abs_x, abs_y, len, p),
            MaskParam::Map(p) => mask_map(mask_buf, abs_x, abs_y, len, p),
        };
        if res == MaskRes::Transp {
            return MaskRes::Transp;
        } else if res == MaskRes::Changed {
            changed = true;
        }
    }

    if changed {
        MaskRes::Changed
    } else {
        MaskRes::FullCover
    }
}

/// `lv_draw_sw_mask_free_param()`: release a cached circle reference.
pub fn free_param(cache: &mut CircleCache, p: &mut MaskParam<'_>) {
    if let MaskParam::Radius(radius) = p {
        release_mask_radius(cache, radius);
    }
}

/// `lv_draw_sw_mask_cleanup()`: drop unused circles from the cache.
///
/// Like upstream this zeroes every entry (`lv_memzero`) regardless of
/// `used_cnt`; it is called at the end of a refresh when no mask is alive.
pub fn cleanup(cache: &mut CircleCache) {
    for entry in cache.entries.iter_mut() {
        *entry = RadiusCircleDsc::default();
    }
}

/// `lv_draw_sw_mask_line_points_init()`
pub fn line_points_init(param: &mut MaskLineParam, p1x: i32, p1y: i32, p2x: i32, p2y: i32, side: LineSide) {
    *param = MaskLineParam::default();

    let (mut p1x, mut p1y, mut p2x, mut p2y) = (p1x, p1y, p2x, p2y);

    if p1y == p2y && side == LineSide::Bottom {
        p1y -= 1;
        p2y -= 1;
    }

    if p1y > p2y {
        core::mem::swap(&mut p1x, &mut p2x);
        core::mem::swap(&mut p1y, &mut p2y);
    }

    param.p1 = Point::new(p1x, p1y);
    param.p2 = Point::new(p2x, p2y);
    param.side = side;

    param.origo = Point::new(p1x, p1y);
    param.flat = (p2x - p1x).abs() > (p2y - p1y).abs();
    param.yx_steep = 0;
    param.xy_steep = 0;

    let dx = p2x - p1x;
    let dy = p2y - p1y;

    if param.flat {
        /*Normalize the steep. Delta x should be relative to delta x = 1024*/
        let mut m: i32;

        if dx != 0 {
            m = (1i32 << 20) / dx; /*m is multiplier to normalize y (upscaled by 1024)*/
            param.yx_steep = m.wrapping_mul(dy) >> 10;
        }

        if dy != 0 {
            m = (1i32 << 20) / dy; /*m is multiplier to normalize x (upscaled by 1024)*/
            param.xy_steep = m.wrapping_mul(dx) >> 10;
        }
        param.steep = param.yx_steep;
    } else {
        /*Normalize the steep. Delta y should be relative to delta x = 1024*/
        let mut m: i32;

        if dy != 0 {
            m = (1i32 << 20) / dy; /*m is multiplier to normalize x (upscaled by 1024)*/
            param.xy_steep = m.wrapping_mul(dx) >> 10;
        }

        if dx != 0 {
            m = (1i32 << 20) / dx; /*m is multiplier to normalize x (upscaled by 1024)*/
            param.yx_steep = m.wrapping_mul(dy) >> 10;
        }
        param.steep = param.xy_steep;
    }

    match param.side {
        LineSide::Left => param.inv = false,
        LineSide::Right => param.inv = true,
        LineSide::Top => param.inv = param.steep > 0,
        LineSide::Bottom => param.inv = param.steep <= 0,
    }

    param.spx = param.steep >> 2;
    if param.steep < 0 {
        param.spx = -param.spx;
    }
}

/// `lv_draw_sw_mask_line_angle_init()`
pub fn line_angle_init(param: &mut MaskLineParam, px: i32, py: i32, angle: i16, side: LineSide) {
    /*Find an optimal degree.
     *lv_mask_line_points_init will swap the points to keep the smaller y in p1
     *Theoretically a line with `angle` or `angle+180` is the same only the points are swapped
     *Find the degree which keeps the origo in place*/
    let mut angle = angle;
    if angle > 180 {
        angle -= 180; /*> 180 will swap the origo*/
    }

    let p2x = (trigo_sin((angle as i32 + 90) as i16) >> 5) + px;
    let p2y = (trigo_sin(angle) >> 5) + py;

    line_points_init(param, px, py, p2x, p2y, side);
}

/// `lv_draw_sw_mask_angle_init()`
pub fn angle_init(
    param: &mut MaskAngleParam,
    vertex_x: i32,
    vertex_y: i32,
    start_angle: i32,
    end_angle: i32,
) {
    /*Constrain the input angles*/
    /*`if(a < 0) a = 0; else if(a > 359) a = 359;`*/
    let start_angle = lv_clamp(0, start_angle, 359);
    let end_angle = lv_clamp(0, end_angle, 359);

    if end_angle < start_angle {
        param.delta_deg = (360 - start_angle + end_angle) as u16;
    } else {
        param.delta_deg = (end_angle - start_angle).unsigned_abs() as u16;
    }

    param.start_angle = start_angle;
    param.end_angle = end_angle;
    param.vertex_p = Point::new(vertex_x, vertex_y);

    debug_assert!(start_angle >= 0 && start_angle <= 360, "Unexpected start angle");

    let start_side = if start_angle >= 0 && start_angle < 180 {
        LineSide::Left
    } else {
        LineSide::Right /*silence compiler*/
    };

    debug_assert!(end_angle >= 0 && start_angle <= 360, "Unexpected end angle");

    let end_side = if end_angle >= 0 && end_angle < 180 {
        LineSide::Right
    } else if end_angle >= 180 && end_angle < 360 {
        LineSide::Left
    } else {
        LineSide::Right /*silence compiler*/
    };

    line_angle_init(
        &mut param.start_line,
        vertex_x,
        vertex_y,
        start_angle as i16,
        start_side,
    );
    line_angle_init(
        &mut param.end_line,
        vertex_x,
        vertex_y,
        end_angle as i16,
        end_side,
    );
}

/// `lv_draw_sw_mask_radius_init()`
pub fn radius_init(
    cache: &mut CircleCache,
    param: &mut MaskRadiusParam,
    rect: &Area,
    radius: i32,
    inv: bool,
) -> LvResult {
    *param = MaskRadiusParam::default();
    let res = LvResult::Ok;
    let w = rect.width();
    let h = rect.height();
    let short_side = w.min(h);
    let mut radius = radius;
    if radius > short_side >> 1 {
        radius = short_side >> 1;
    }
    if radius < 0 {
        radius = 0;
    }

    param.rect = *rect;
    param.radius = radius;
    param.outer = inv;

    if radius == 0 {
        return res;
    }

    let circle = match acquire_mask_radius_from_cache(cache, radius) {
        Some(i) => CircleRef::Cached(i),
        None => match create_non_cached_mask_radius(radius) {
            Some(c) => CircleRef::Owned(c),
            None => return LvResult::Invalid,
        },
    };

    param.circle = circle;
    res
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
    param.coords = *coords;
    param.opa_top = opa_top;
    param.opa_bottom = opa_bottom;
    param.y_top = y_top;
    param.y_bottom = y_bottom;
}

/// `lv_draw_sw_mask_map_init()`
pub fn map_init<'a>(param: &mut MaskMapParam<'a>, coords: &Area, map: &'a [Opa]) {
    param.coords = *coords;
    param.map = map;
}

/*********************
 *   STATIC FUNCTIONS
 *********************/

/// `LV_CLAMP(min, val, max)` = `LV_MAX(min, LV_MIN(val, max))`.
///
/// Unlike `i32::clamp` it never panics when `min > max`.
#[inline]
const fn lv_clamp(min: i32, val: i32, max: i32) -> i32 {
    let v = if val < max { val } else { max };
    if min > v { min } else { v }
}

/// `lv_memzero(&mask_buf[from], count)`
#[inline]
fn memzero(buf: &mut [Opa], from: i32, count: i32) {
    let from = from as usize;
    buf[from..from + count as usize].fill(0);
}

/// `lv_draw_sw_mask_line()` (`lv_draw_mask_line`)
pub fn mask_line(mask_buf: &mut [Opa], abs_x: i32, abs_y: i32, len: i32, p: &mut MaskLineParam) -> MaskRes {
    /*Make to points relative to the vertex*/
    let abs_y = abs_y - p.origo.y;
    let abs_x = abs_x - p.origo.x;

    /*Handle special cases*/
    if p.steep == 0 {
        /*Horizontal*/
        if p.flat {
            /*Non sense: Can't be on the right/left of a horizontal line*/
            if p.side == LineSide::Left
                || p.side == LineSide::Right
                || (p.side == LineSide::Top && abs_y < 0)
                || (p.side == LineSide::Bottom && abs_y > 0)
            {
                return MaskRes::FullCover;
            } else {
                return MaskRes::Transp;
            }
        }
        /*Vertical*/
        else {
            /*Non sense: Can't be on the top/bottom of a vertical line*/
            if p.side == LineSide::Top
                || p.side == LineSide::Bottom
                || (p.side == LineSide::Right && abs_x > 0)
            {
                return MaskRes::FullCover;
            } else if p.side == LineSide::Left {
                if abs_x + len < 0 {
                    return MaskRes::FullCover;
                } else {
                    let k = -abs_x;
                    if k < 0 {
                        return MaskRes::Transp;
                    }
                    if k >= 0 && k < len {
                        memzero(mask_buf, k, len - k);
                    }
                    return MaskRes::Changed;
                }
            } else {
                if abs_x + len < 0 {
                    return MaskRes::Transp;
                } else {
                    let mut k = -abs_x;
                    if k < 0 {
                        k = 0;
                    }
                    if k >= len {
                        return MaskRes::Transp;
                    } else if k >= 0 && k < len {
                        memzero(mask_buf, 0, k);
                    }
                    return MaskRes::Changed;
                }
            }
        }
    }

    if p.flat {
        line_mask_flat(mask_buf, abs_x, abs_y, len, p)
    } else {
        line_mask_steep(mask_buf, abs_x, abs_y, len, p)
    }
}

/// `line_mask_flat()`
fn line_mask_flat(mask_buf: &mut [Opa], abs_x: i32, abs_y: i32, len: i32, p: &MaskLineParam) -> MaskRes {
    let mut y_at_x: i32;
    y_at_x = p.yx_steep.wrapping_mul(abs_x) >> 10;

    if p.yx_steep > 0 {
        if y_at_x > abs_y {
            if p.inv {
                return MaskRes::FullCover;
            } else {
                return MaskRes::Transp;
            }
        }
    } else if y_at_x < abs_y {
        if p.inv {
            return MaskRes::FullCover;
        } else {
            return MaskRes::Transp;
        }
    }

    /*At the end of the mask if the limit line is smaller than the mask's y.
     *Then the mask is in the "good" area*/
    y_at_x = p.yx_steep.wrapping_mul(abs_x + len) >> 10;
    if p.yx_steep > 0 {
        if y_at_x < abs_y {
            if p.inv {
                return MaskRes::Transp;
            } else {
                return MaskRes::FullCover;
            }
        }
    } else if y_at_x > abs_y {
        if p.inv {
            return MaskRes::Transp;
        } else {
            return MaskRes::FullCover;
        }
    }

    let xe: i32 = if p.yx_steep > 0 {
        (abs_y * 256).wrapping_mul(p.xy_steep) >> 10
    } else {
        ((abs_y + 1) * 256).wrapping_mul(p.xy_steep) >> 10
    };

    let xei = xe >> 8;
    let xef = xe & 0xFF;

    let mut px_h: i32 = if xef == 0 {
        255
    } else {
        255 - (((255 - xef) * p.spx) >> 8)
    };
    let mut k = xei - abs_x;
    let mut m: Opa;

    if xef != 0 {
        if k >= 0 && k < len {
            m = (255 - (((255 - xef) * (255 - px_h)) >> 9)) as Opa;
            if p.inv {
                m = 255 - m;
            }
            mask_buf[k as usize] = mask_mix(mask_buf[k as usize], m);
        }
        k += 1;
    }

    while px_h > p.spx {
        if k >= 0 && k < len {
            m = (px_h - (p.spx >> 1)) as Opa;
            if p.inv {
                m = 255 - m;
            }
            mask_buf[k as usize] = mask_mix(mask_buf[k as usize], m);
        }
        px_h -= p.spx;
        k += 1;
        if k >= len {
            break;
        }
    }

    if k < len && k >= 0 {
        let x_inters = px_h.wrapping_mul(p.xy_steep) >> 10;
        m = (x_inters.wrapping_mul(px_h) >> 9) as Opa;
        if p.yx_steep < 0 {
            m = 255 - m;
        }
        if p.inv {
            m = 255 - m;
        }
        mask_buf[k as usize] = mask_mix(mask_buf[k as usize], m);
    }

    if p.inv {
        k = xei - abs_x;
        if k > len {
            return MaskRes::Transp;
        }
        if k >= 0 {
            memzero(mask_buf, 0, k);
        }
    } else {
        k += 1;
        if k < 0 {
            return MaskRes::Transp;
        }
        if k < len {
            memzero(mask_buf, k, len - k);
        }
    }

    MaskRes::Changed
}

/// `line_mask_steep()`
fn line_mask_steep(mask_buf: &mut [Opa], abs_x: i32, abs_y: i32, len: i32, p: &MaskLineParam) -> MaskRes {
    let mut k: i32;
    let mut x_at_y: i32;
    /*At the beginning of the mask if the limit line is greater than the mask's y.
     *Then the mask is in the "wrong" area*/
    x_at_y = p.xy_steep.wrapping_mul(abs_y) >> 10;
    if p.xy_steep > 0 {
        x_at_y += 1;
    }
    if x_at_y < abs_x {
        if p.inv {
            return MaskRes::FullCover;
        } else {
            return MaskRes::Transp;
        }
    }

    /*At the end of the mask if the limit line is smaller than the mask's y.
     *Then the mask is in the "good" area*/
    x_at_y = p.xy_steep.wrapping_mul(abs_y) >> 10;
    if x_at_y > abs_x + len {
        if p.inv {
            return MaskRes::Transp;
        } else {
            return MaskRes::FullCover;
        }
    }

    /*X start*/
    let xs = (abs_y * 256).wrapping_mul(p.xy_steep) >> 10;
    let mut xsi = xs >> 8;
    let mut xsf = xs & 0xFF;

    /*X end*/
    let xe = ((abs_y + 1) * 256).wrapping_mul(p.xy_steep) >> 10;
    let xei = xe >> 8;
    let xef = xe & 0xFF;

    let mut m: Opa;

    k = xsi - abs_x;
    if xsi != xei && (p.xy_steep < 0 && xsf == 0) {
        xsf = 0xFF;
        xsi = xei;
        k -= 1;
    }

    if xsi == xei {
        if k >= 0 && k < len {
            m = ((xsf + xef) >> 1) as Opa;
            if p.inv {
                m = 255 - m;
            }
            mask_buf[k as usize] = mask_mix(mask_buf[k as usize], m);
        }
        k += 1;

        if p.inv {
            k = xsi - abs_x;
            if k >= len {
                return MaskRes::Transp;
            }
            if k >= 0 {
                memzero(mask_buf, 0, k);
            }
        } else {
            if k > len {
                k = len;
            }
            if k == 0 {
                return MaskRes::Transp;
            } else if k > 0 {
                memzero(mask_buf, k, len - k);
            }
        }
    } else {
        let y_inters: i32;
        if p.xy_steep < 0 {
            y_inters = xsf.wrapping_mul(p.yx_steep.wrapping_neg()) >> 10;
            if k >= 0 && k < len {
                m = (y_inters.wrapping_mul(xsf) >> 9) as Opa;
                if p.inv {
                    m = 255 - m;
                }
                mask_buf[k as usize] = mask_mix(mask_buf[k as usize], m);
            }
            k -= 1;

            let x_inters = (255 - y_inters).wrapping_mul(p.xy_steep.wrapping_neg()) >> 10;

            if k >= 0 && k < len {
                m = (255 - ((255 - y_inters).wrapping_mul(x_inters) >> 9)) as Opa;
                if p.inv {
                    m = 255 - m;
                }
                mask_buf[k as usize] = mask_mix(mask_buf[k as usize], m);
            }

            k += 2;

            if p.inv {
                k = xsi - abs_x - 1;

                /*upstream: `if(k > len) k = len; else if(k > 0) memzero` (the
                 *`k = len` store is dead, `k` is not read afterwards)*/
                if k <= len && k > 0 {
                    memzero(mask_buf, 0, k);
                }
            } else {
                if k > len {
                    return MaskRes::FullCover;
                }
                if k >= 0 {
                    memzero(mask_buf, k, len - k);
                }
            }
        } else {
            y_inters = (255 - xsf).wrapping_mul(p.yx_steep) >> 10;
            if k >= 0 && k < len {
                m = (255 - (y_inters.wrapping_mul(255 - xsf) >> 9)) as Opa;
                if p.inv {
                    m = 255 - m;
                }
                mask_buf[k as usize] = mask_mix(mask_buf[k as usize], m);
            }

            k += 1;

            let x_inters = (255 - y_inters).wrapping_mul(p.xy_steep) >> 10;
            if k >= 0 && k < len {
                m = ((255 - y_inters).wrapping_mul(x_inters) >> 9) as Opa;
                if p.inv {
                    m = 255 - m;
                }
                mask_buf[k as usize] = mask_mix(mask_buf[k as usize], m);
            }
            k += 1;

            if p.inv {
                k = xsi - abs_x;
                if k > len {
                    return MaskRes::Transp;
                }
                if k >= 0 {
                    memzero(mask_buf, 0, k);
                }
            } else {
                if k > len {
                    k = len;
                }
                if k == 0 {
                    return MaskRes::Transp;
                } else if k > 0 {
                    memzero(mask_buf, k, len - k);
                }
            }
        }
    }

    MaskRes::Changed
}

/// The two "start angle > end angle in the same half" branches of
/// `lv_draw_mask_angle()`: they are identical except for which line is
/// applied to the left part (`first`) and which to the right (`second`).
fn angle_split(
    mask_buf: &mut [Opa],
    abs_x: i32,
    abs_y: i32,
    len: i32,
    rel_x: i32,
    rel_y: i32,
    start_angle: i32,
    end_angle: i32,
    start_xy_steep: i32,
    end_xy_steep: i32,
    first: &mut MaskLineParam,
    second: &mut MaskLineParam,
) -> MaskRes {
    /*Start angle mask can work only from the end of end angle mask*/
    let end_angle_first = rel_y.wrapping_mul(end_xy_steep) >> 10;
    let mut start_angle_last = (rel_y + 1).wrapping_mul(start_xy_steep) >> 10;

    /*Do not let the line end cross the vertex else it will affect the opposite part*/
    if (start_angle > 270 && start_angle <= 359 && start_angle_last < 0)
        || (start_angle > 0 && start_angle <= 90 && start_angle_last < 0)
        || (start_angle > 90 && start_angle < 270 && start_angle_last > 0)
    {
        start_angle_last = 0;
    }

    if (end_angle > 270 && end_angle <= 359 && start_angle_last < 0)
        || (end_angle > 0 && end_angle <= 90 && start_angle_last < 0)
        || (end_angle > 90 && end_angle < 270 && start_angle_last > 0)
    {
        start_angle_last = 0;
    }

    let dist = (end_angle_first - start_angle_last) >> 1;

    let mut res1 = MaskRes::FullCover;

    let mut tmp = start_angle_last + dist - rel_x;
    if tmp > len {
        tmp = len;
    }
    if tmp > 0 {
        res1 = mask_line(&mut mask_buf[..tmp as usize], abs_x, abs_y, tmp, first);
        if res1 == MaskRes::Transp {
            memzero(mask_buf, 0, tmp);
        }
    }

    if tmp > len {
        tmp = len;
    }
    if tmp < 0 {
        tmp = 0;
    }
    let res2 = mask_line(
        &mut mask_buf[tmp as usize..],
        abs_x + tmp,
        abs_y,
        len - tmp,
        second,
    );
    if res2 == MaskRes::Transp {
        memzero(mask_buf, tmp, len - tmp);
    }
    if res1 == res2 { res1 } else { MaskRes::Changed }
}

/// `lv_draw_sw_mask_angle()` (`lv_draw_mask_angle`)
pub fn mask_angle(mask_buf: &mut [Opa], abs_x: i32, abs_y: i32, len: i32, p: &mut MaskAngleParam) -> MaskRes {
    let rel_y = abs_y - p.vertex_p.y;
    let rel_x = abs_x - p.vertex_p.x;

    let MaskAngleParam {
        vertex_p,
        start_angle,
        end_angle,
        start_line,
        end_line,
        ..
    } = p;
    let (vertex_p, start_angle, end_angle) = (*vertex_p, *start_angle, *end_angle);

    if start_angle < 180 && end_angle < 180 && start_angle != 0 && end_angle != 0 && start_angle > end_angle {
        if abs_y < vertex_p.y {
            return MaskRes::FullCover;
        }

        angle_split(
            mask_buf,
            abs_x,
            abs_y,
            len,
            rel_x,
            rel_y,
            start_angle,
            end_angle,
            start_line.xy_steep,
            end_line.xy_steep,
            start_line,
            end_line,
        )
    } else if start_angle > 180 && end_angle > 180 && start_angle > end_angle {
        if abs_y > vertex_p.y {
            return MaskRes::FullCover;
        }

        angle_split(
            mask_buf,
            abs_x,
            abs_y,
            len,
            rel_x,
            rel_y,
            start_angle,
            end_angle,
            start_line.xy_steep,
            end_line.xy_steep,
            end_line,
            start_line,
        )
    } else {
        let res1: MaskRes;
        let res2: MaskRes;

        if start_angle == 180 {
            if abs_y < vertex_p.y {
                res1 = MaskRes::FullCover;
            } else {
                res1 = MaskRes::Unknown;
            }
        } else if start_angle == 0 {
            if abs_y < vertex_p.y {
                res1 = MaskRes::Unknown;
            } else {
                res1 = MaskRes::FullCover;
            }
        } else if (start_angle < 180 && abs_y < vertex_p.y) || (start_angle > 180 && abs_y >= vertex_p.y) {
            res1 = MaskRes::Unknown;
        } else {
            res1 = mask_line(mask_buf, abs_x, abs_y, len, start_line);
        }

        if end_angle == 180 {
            if abs_y < vertex_p.y {
                res2 = MaskRes::Unknown;
            } else {
                res2 = MaskRes::FullCover;
            }
        } else if end_angle == 0 {
            if abs_y < vertex_p.y {
                res2 = MaskRes::FullCover;
            } else {
                res2 = MaskRes::Unknown;
            }
        } else if (end_angle < 180 && abs_y < vertex_p.y) || (end_angle > 180 && abs_y >= vertex_p.y) {
            res2 = MaskRes::Unknown;
        } else {
            res2 = mask_line(mask_buf, abs_x, abs_y, len, end_line);
        }

        if res1 == MaskRes::Transp
            || res2 == MaskRes::Transp
            || (res1 == MaskRes::Unknown && res2 == MaskRes::Unknown)
        {
            MaskRes::Transp
        } else if res1 == MaskRes::FullCover && res2 == MaskRes::FullCover {
            MaskRes::FullCover
        } else {
            MaskRes::Changed
        }
    }
}

/// `lv_draw_sw_mask_radius()` (`lv_draw_mask_radius`)
pub fn mask_radius(
    cache: &CircleCache,
    mask_buf: &mut [Opa],
    abs_x: i32,
    abs_y: i32,
    len: i32,
    p: &mut MaskRadiusParam,
) -> MaskRes {
    let outer = p.outer;
    let radius = p.radius;
    let rect = p.rect;

    if !outer {
        if abs_y < rect.y1 || abs_y > rect.y2 {
            return MaskRes::Transp;
        }
    } else if abs_y < rect.y1 || abs_y > rect.y2 {
        return MaskRes::FullCover;
    }

    if (abs_x >= rect.x1 + radius && abs_x + len <= rect.x2 - radius)
        || (abs_y >= rect.y1 + radius && abs_y <= rect.y2 - radius)
    {
        if !outer {
            /*Remove the edges*/
            let last = rect.x1 - abs_x;
            if last > len {
                return MaskRes::Transp;
            }
            if last >= 0 {
                memzero(mask_buf, 0, last);
            }

            let first = rect.x2 - abs_x + 1;
            if first <= 0 {
                return MaskRes::Transp;
            } else if first < len {
                memzero(mask_buf, first, len - first);
            }
            if last == 0 && first == len {
                return MaskRes::FullCover;
            } else {
                return MaskRes::Changed;
            }
        } else {
            let mut first = rect.x1 - abs_x;
            if first < 0 {
                first = 0;
            }
            if first <= len {
                let mut last = rect.x2 - abs_x - first + 1;
                if first + last > len {
                    last = len - first;
                }
                if last >= 0 {
                    memzero(mask_buf, first, last);
                }
            }
        }
        return MaskRes::Changed;
    }

    let k = rect.x1 - abs_x; /*First relevant coordinate on the of the mask*/
    let w = rect.width();
    let h = rect.height();
    let abs_y = abs_y - rect.y1;

    let cir_y = if abs_y < radius {
        radius - abs_y - 1
    } else {
        abs_y - (h - radius)
    };

    /*`p->circle` is never NULL here: `radius > 0` means `radius_init()`
     *assigned one, and `radius == 0` always took the early return above.*/
    let circle: &RadiusCircleDsc = match &p.circle {
        CircleRef::Cached(i) => &cache.entries[*i],
        CircleRef::Owned(c) => c,
        CircleRef::None => panic!("radius mask without a circle (radius {radius})"),
    };
    let (aa_opa, aa_len, x_start) = get_next_line(circle, cir_y);
    let mut cir_x_right = k + w - radius + x_start;
    let mut cir_x_left = k + radius - x_start - 1;
    let mut i: i32 = 0;

    if !outer {
        while i < aa_len {
            let opa = aa_opa[(aa_len - i - 1) as usize];
            if cir_x_right + i >= 0 && cir_x_right + i < len {
                let idx = (cir_x_right + i) as usize;
                mask_buf[idx] = mask_mix(opa, mask_buf[idx]);
            }
            if cir_x_left - i >= 0 && cir_x_left - i < len {
                let idx = (cir_x_left - i) as usize;
                mask_buf[idx] = mask_mix(opa, mask_buf[idx]);
            }
            i += 1;
        }

        /*Clean the right side*/
        cir_x_right = lv_clamp(0, cir_x_right + i, len);
        memzero(mask_buf, cir_x_right, len - cir_x_right);

        /*Clean the left side*/
        cir_x_left = lv_clamp(0, cir_x_left - aa_len + 1, len);
        memzero(mask_buf, 0, cir_x_left);
    } else {
        while i < aa_len {
            let opa = 255 - aa_opa[(aa_len - 1 - i) as usize];
            if cir_x_right + i >= 0 && cir_x_right + i < len {
                let idx = (cir_x_right + i) as usize;
                mask_buf[idx] = mask_mix(opa, mask_buf[idx]);
            }
            if cir_x_left - i >= 0 && cir_x_left - i < len {
                let idx = (cir_x_left - i) as usize;
                mask_buf[idx] = mask_mix(opa, mask_buf[idx]);
            }
            i += 1;
        }

        let clr_start = lv_clamp(0, cir_x_left + 1, len);
        let clr_len = lv_clamp(0, cir_x_right - clr_start, len - clr_start);
        memzero(mask_buf, clr_start, clr_len);
    }

    MaskRes::Changed
}

/// `lv_draw_sw_mask_fade()` (`lv_draw_mask_fade`)
pub fn mask_fade(mask_buf: &mut [Opa], abs_x: i32, abs_y: i32, len: i32, p: &mut MaskFadeParam) -> MaskRes {
    if abs_y < p.coords.y1 {
        return MaskRes::FullCover;
    }
    if abs_y > p.coords.y2 {
        return MaskRes::FullCover;
    }
    if abs_x + len < p.coords.x1 {
        return MaskRes::FullCover;
    }
    if abs_x > p.coords.x2 {
        return MaskRes::FullCover;
    }

    let mut len = len;
    if abs_x + len > p.coords.x2 {
        len -= abs_x + len - p.coords.x2 - 1;
    }

    let x_ofs = if abs_x < p.coords.x1 {
        p.coords.x1 - abs_x
    } else {
        0
    };
    len -= x_ofs;

    /*C keeps `len` signed: with a degenerate area (x2 < x1) it can end up <= 0, every
     *`for(i = 0; i < len; i++)` loop below is skipped and CHANGED is returned untouched.*/
    if len <= 0 {
        return MaskRes::Changed;
    }
    let mask_buf = &mut mask_buf[x_ofs as usize..];
    let len = len as usize;

    if abs_y <= p.y_top {
        for i in 0..len {
            mask_buf[i] = mask_mix(mask_buf[i], p.opa_top);
        }
        MaskRes::Changed
    } else if abs_y >= p.y_bottom {
        for i in 0..len {
            mask_buf[i] = mask_mix(mask_buf[i], p.opa_bottom);
        }
        MaskRes::Changed
    } else {
        /*Calculate the opa proportionally*/
        let opa_diff: i16 = (p.opa_bottom as i32 - p.opa_top as i32) as i16;
        let y_diff: i32 = p.y_bottom - p.y_top + 1;
        /*LV_OPA_MIX2(a1, a2) = ((lv_opa_t)(((int32_t)(a1) * (a2)) >> 8)): the macro truncates to
         *lv_opa_t BEFORE the `/ y_diff`, and the quotient is truncated to lv_opa_t again.*/
        let mix2: Opa = (((abs_y - p.y_top) * opa_diff as i32) >> 8) as Opa;
        let mut opa_act: Opa = (mix2 as i32 / y_diff) as Opa;
        opa_act = opa_act.wrapping_add(p.opa_top);

        for i in 0..len {
            mask_buf[i] = mask_mix(mask_buf[i], opa_act);
        }
        MaskRes::Changed
    }
}

/// `lv_draw_sw_mask_map()` (`lv_draw_mask_map`)
pub fn mask_map(mask_buf: &mut [Opa], abs_x: i32, abs_y: i32, len: i32, p: &mut MaskMapParam<'_>) -> MaskRes {
    /*Handle out of the mask cases*/
    if abs_y < p.coords.y1 {
        return MaskRes::FullCover;
    }
    if abs_y > p.coords.y2 {
        return MaskRes::FullCover;
    }
    if abs_x + len < p.coords.x1 {
        return MaskRes::FullCover;
    }
    if abs_x > p.coords.x2 {
        return MaskRes::FullCover;
    }

    /*Got to the current row in the map*/
    let mut map_ofs: i32 = (abs_y - p.coords.y1) * p.coords.width();

    let mut len = len;
    if abs_x + len > p.coords.x2 {
        len -= abs_x + len - p.coords.x2 - 1;
    }

    let mut x_ofs = 0;
    if abs_x < p.coords.x1 {
        x_ofs = p.coords.x1 - abs_x;
        len -= x_ofs;
    } else {
        map_ofs += abs_x - p.coords.x1;
    }

    /*C keeps `len` signed: with a degenerate area (x2 < x1) it can end up <= 0, the loop is
     *skipped and neither the mask buffer nor the map is read.*/
    if len <= 0 {
        return MaskRes::Changed;
    }

    let mask_buf = &mut mask_buf[x_ofs as usize..];
    let map_tmp = &p.map[map_ofs as usize..];
    for i in 0..len as usize {
        mask_buf[i] = mask_mix(mask_buf[i], map_tmp[i]);
    }

    MaskRes::Changed
}

/// `circ_init()`: initialize the circle drawing.
///
/// Returns the first point and the decision variable `tmp`.
fn circ_init(radius: i32) -> (Point, i32) {
    (Point::new(radius, 0), 1 - radius)
}

/// `circ_cont()`: `true` if the circle is not ready yet.
fn circ_cont(c: &Point) -> bool {
    c.y <= c.x
}

/// `circ_next()`: get the next point from the circle.
fn circ_next(c: &mut Point, tmp: &mut i32) {
    if *tmp <= 0 {
        *tmp += 2 * c.y + 3; /*Change in decision criterion for y -> y+1*/
    } else {
        *tmp += 2 * (c.y - c.x) + 5; /*Change for y -> y+1, x -> x-1*/
        c.x -= 1;
    }
    c.y += 1;
}

/// Store one anti-aliased circle point (the repeated
/// `cir_x[cir_size] = ..; cir_y[cir_size] = ..; c->cir_opa[cir_size] = ..; *= 16;`
/// block of `circ_calc_aa4()`).
#[inline]
fn circ_put(
    c: &mut RadiusCircleDsc,
    cir_x: &mut [i32],
    cir_y: &mut [i32],
    idx: usize,
    x: i32,
    y: i32,
    opa: u32,
) {
    cir_x[idx] = x;
    cir_y[idx] = y;
    c.cir_opa[idx] = (opa as Opa).wrapping_mul(16);
}

/// `circ_calc_aa4()`: compute the anti-aliased quarter circle of `radius`
/// into `c` (4x oversampled midpoint algorithm).
fn circ_calc_aa4(c: &mut RadiusCircleDsc, radius: i32) -> LvResult {
    debug_assert!(radius > 0, "caller should handle radius == 0 fast path");

    let r = radius as usize;
    /*`buf` is `radius * 6 + 6` bytes: 2r+2 opa bytes and two (r+1) u16 arrays*/
    c.cir_opa = vec![0; 2 * r + 2];
    c.radius = radius;
    c.opa_start_on_y = vec![0; r + 1];
    c.x_start_on_y = vec![0; r + 1];

    /*Special case, handle manually*/
    if radius == 1 {
        c.cir_opa[0] = 180;
        c.opa_start_on_y[0] = 0;
        c.opa_start_on_y[1] = 1;
        c.x_start_on_y[0] = 0;
        return LvResult::Ok;
    }

    let mut cir_x: Vec<i32> = vec![0; (r + 1) * 2];
    let mut cir_y: Vec<i32> = vec![0; (r + 1) * 2];

    let mut y_8th_cnt: u32 = 0;
    let (mut cp, mut tmp) = circ_init(radius * 4); /*Upscale by 4*/

    let mut x_int = [0u32; 4];
    let mut x_fract = [0u32; 4];
    let mut cir_size: usize = 0;
    x_int[0] = (cp.x >> 2) as u32;
    x_fract[0] = 0;

    /*Calculate an 1/8 circle*/
    while circ_cont(&cp) {
        /*Calculate 4 point of the circle */
        let mut i = 0;
        while i < 4 {
            circ_next(&mut cp, &mut tmp);
            if !circ_cont(&cp) {
                break;
            }
            x_int[i] = (cp.x >> 2) as u32;
            x_fract[i] = (cp.x & 0x3) as u32;
            i += 1;
        }
        if i != 4 {
            break;
        }

        let y = y_8th_cnt as i32;
        /*All lines on the same x when downscaled*/
        if x_int[0] == x_int[3] {
            let opa = x_fract[0] + x_fract[1] + x_fract[2] + x_fract[3];
            circ_put(c, &mut cir_x, &mut cir_y, cir_size, x_int[0] as i32, y, opa);
            cir_size += 1;
        }
        /*Second line on new x when downscaled*/
        else if x_int[0] != x_int[1] {
            circ_put(
                c,
                &mut cir_x,
                &mut cir_y,
                cir_size,
                x_int[0] as i32,
                y,
                x_fract[0],
            );
            cir_size += 1;

            let opa = 1 * 4 + x_fract[1] + x_fract[2] + x_fract[3];
            circ_put(
                c,
                &mut cir_x,
                &mut cir_y,
                cir_size,
                x_int[0].wrapping_sub(1) as i32,
                y,
                opa,
            );
            cir_size += 1;
        }
        /*Third line on new x when downscaled*/
        else if x_int[0] != x_int[2] {
            let opa = x_fract[0] + x_fract[1];
            circ_put(c, &mut cir_x, &mut cir_y, cir_size, x_int[0] as i32, y, opa);
            cir_size += 1;

            let opa = 2 * 4 + x_fract[2] + x_fract[3];
            circ_put(
                c,
                &mut cir_x,
                &mut cir_y,
                cir_size,
                x_int[0].wrapping_sub(1) as i32,
                y,
                opa,
            );
            cir_size += 1;
        }
        /*Forth line on new x when downscaled*/
        else {
            let opa = x_fract[0] + x_fract[1] + x_fract[2];
            circ_put(c, &mut cir_x, &mut cir_y, cir_size, x_int[0] as i32, y, opa);
            cir_size += 1;

            let opa = 3 * 4 + x_fract[3];
            circ_put(
                c,
                &mut cir_x,
                &mut cir_y,
                cir_size,
                x_int[0].wrapping_sub(1) as i32,
                y,
                opa,
            );
            cir_size += 1;
        }

        y_8th_cnt += 1;
    }

    /*The point on the 1/8 circle is special, calculate it manually*/
    let mid = radius * 723;
    let mid_int = mid >> 10;
    if cir_x[cir_size - 1] != mid_int || cir_y[cir_size - 1] != mid_int {
        let mut tmp_val = mid - (mid_int << 10);
        if tmp_val <= 512 {
            tmp_val = tmp_val * tmp_val * 2;
            tmp_val >>= 10 + 6;
        } else {
            tmp_val = 1024 - tmp_val;
            tmp_val = tmp_val * tmp_val * 2;
            tmp_val >>= 10 + 6;
            tmp_val = 15 - tmp_val;
        }

        circ_put(
            c,
            &mut cir_x,
            &mut cir_y,
            cir_size,
            mid_int,
            mid_int,
            tmp_val as u32,
        );
        cir_size += 1;
    }

    /*Build the second octet by mirroring the first*/
    let mut i = cir_size as i32 - 2;
    while i >= 0 {
        let iu = i as usize;
        cir_x[cir_size] = cir_y[iu];
        cir_y[cir_size] = cir_x[iu];
        c.cir_opa[cir_size] = c.cir_opa[iu];
        i -= 1;
        cir_size += 1;
    }

    let mut y: usize = 0;
    let mut i: usize = 0;
    c.opa_start_on_y[0] = 0;
    while i < cir_size {
        c.opa_start_on_y[y] = i as u16;
        c.x_start_on_y[y] = cir_x[i] as u16;
        while i < cir_size && cir_y[i] <= y as i32 {
            c.x_start_on_y[y] = (c.x_start_on_y[y] as i32).min(cir_x[i]) as u16;
            i += 1;
        }
        y += 1;
    }

    LvResult::Ok
}

/// `get_next_line()`: the anti-aliased pixels of row `y` of a quarter
/// circle, their count (`len`) and the first x (`x_start`).
fn get_next_line(c: &RadiusCircleDsc, y: i32) -> (&[Opa], i32, i32) {
    let y = y as usize;
    let len = c.opa_start_on_y[y + 1] as i32 - c.opa_start_on_y[y] as i32;
    let x_start = c.x_start_on_y[y] as i32;
    let start = c.opa_start_on_y[y] as usize;
    (&c.cir_opa[start..start + len as usize], len, x_start)
}

/// `mask_mix()`
#[inline]
fn mask_mix(mask_act: Opa, mask_new: Opa) -> Opa {
    if mask_new >= OPA_MAX {
        return mask_act;
    }
    if mask_new <= OPA_MIN {
        return 0;
    }

    udiv255(mask_act as u32 * mask_new as u32) as Opa
}

/// `create_non_cached_mask_radius()`: a circle owned by the parameter
/// (`life == -1`), used when every cache entry is busy.
fn create_non_cached_mask_radius(radius: i32) -> Option<RadiusCircleDsc> {
    let mut circle = RadiusCircleDsc::default();
    init_mask_radius_cache(&mut circle, false);

    let res = circ_calc_aa4(&mut circle, radius);
    if res != LvResult::Ok {
        return None;
    }
    Some(circle)
}

/// `acquire_mask_radius_from_cache()`: reuse the entry of `radius`, or
/// compute it into the unused entry with the lowest `life`. `None` when
/// every entry is in use.
fn acquire_mask_radius_from_cache(cache: &mut CircleCache, radius: i32) -> Option<usize> {
    /*Try to reuse a circle cache entry*/
    for i in 0..CIRCLE_CACHE_SIZE {
        if cache.entries[i].radius != radius {
            continue;
        }

        cache.entries[i].used_cnt = cache.entries[i].used_cnt.wrapping_add(1);
        circle_cache_aging(&mut cache.entries[i].life, radius);
        return Some(i);
    }

    /*If not cached use the free entry with lowest life*/
    let mut entry: Option<usize> = None;
    for i in 0..CIRCLE_CACHE_SIZE {
        if cache.entries[i].used_cnt > 0 {
            continue;
        }
        let Some(e) = entry else {
            entry = Some(i);
            continue;
        };
        if cache.entries[i].life >= cache.entries[e].life {
            continue;
        }
        entry = Some(i);
    }

    let e = entry?;

    cache.entries[e].radius = radius;

    let res = circ_calc_aa4(&mut cache.entries[e], radius);
    if res != LvResult::Ok {
        return None;
    }

    /* only init cache after initializing the entry*/
    init_mask_radius_cache(&mut cache.entries[e], true);

    Some(e)
}

/// `release_mask_radius()`: drop an owned circle or decrement the
/// `used_cnt` of a cached one. The reference is cleared afterwards.
fn release_mask_radius(cache: &mut CircleCache, radius_p: &mut MaskRadiusParam) {
    match core::mem::take(&mut radius_p.circle) {
        CircleRef::None => {}
        /* not cached (`life < 0`): freed with the parameter */
        CircleRef::Owned(circle) => drop(circle),
        CircleRef::Cached(i) => {
            cache.entries[i].used_cnt = cache.entries[i].used_cnt.wrapping_sub(1);
        }
    }
}

/// `init_mask_radius_cache()`
#[inline]
fn init_mask_radius_cache(circle: &mut RadiusCircleDsc, cached: bool) {
    if cached {
        circle.life = 0;
        circle.used_cnt = 1;
        circle_cache_aging(&mut circle.life, circle.radius);
    } else {
        circle.life = -1;
    }
}

/// `CIRCLE_CACHE_AGING(life, r)`:
/// `life = LV_MIN(life + (r < 16 ? 1 : (r >> 4)), 1000)`
#[inline]
fn circle_cache_aging(life: &mut i32, r: i32) {
    *life = (*life + if r < 16 { 1 } else { r >> 4 }).min(CIRCLE_CACHE_LIFE_MAX);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::misc::color::OPA_COVER;

    fn full(len: usize) -> Vec<Opa> {
        vec![OPA_COVER; len]
    }

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

    #[test]
    fn mask_mix_semantics() {
        assert_eq!(mask_mix(100, 255), 100);
        assert_eq!(mask_mix(100, OPA_MAX), 100);
        assert_eq!(mask_mix(100, OPA_MIN), 0);
        assert_eq!(mask_mix(100, 0), 0);
        // LV_UDIV255(255 * 128) == 128
        assert_eq!(mask_mix(255, 128), 128);
        assert_eq!(mask_mix(128, 128), ((128u32 * 128 * 0x8081) >> 0x17) as u8);
    }

    #[test]
    fn lv_clamp_never_panics() {
        assert_eq!(lv_clamp(0, 5, 10), 5);
        assert_eq!(lv_clamp(0, -5, 10), 0);
        assert_eq!(lv_clamp(0, 50, 10), 10);
        // min > max: LV_MAX(min, LV_MIN(val, max)) == min
        assert_eq!(lv_clamp(5, 3, 2), 5);
    }

    #[test]
    fn line_points_init_vertical_and_horizontal() {
        let mut p = MaskLineParam::default();
        line_points_init(&mut p, 10, 0, 10, 100, LineSide::Left);
        assert!(!p.flat);
        assert_eq!(p.steep, 0);
        assert_eq!(p.xy_steep, 0);
        assert_eq!(p.yx_steep, 0);
        assert!(!p.inv);
        assert_eq!(p.origo, Point::new(10, 0));

        // p1y > p2y: the points are swapped so that p1 has the smaller y.
        line_points_init(&mut p, 10, 100, 10, 0, LineSide::Right);
        assert_eq!(p.p1, Point::new(10, 0));
        assert_eq!(p.p2, Point::new(10, 100));
        assert_eq!(p.origo, Point::new(10, 0));
        assert!(p.inv);

        line_points_init(&mut p, 0, 10, 100, 10, LineSide::Top);
        assert!(p.flat);
        assert_eq!(p.steep, 0);
        assert_eq!(p.origo, Point::new(0, 10));

        // Horizontal + BOTTOM moves the line one pixel up.
        line_points_init(&mut p, 0, 10, 100, 10, LineSide::Bottom);
        assert_eq!(p.origo, Point::new(0, 9));
        assert_eq!(p.p1.y, 9);
        assert_eq!(p.p2.y, 9);
    }

    #[test]
    fn line_points_init_diagonal_steep_values() {
        // 45 degrees: dx == dy, so it is not flat; m = 2^20 / 100 = 10485,
        // (10485 * 100) >> 10 = 1023.
        let mut p = MaskLineParam::default();
        line_points_init(&mut p, 0, 0, 100, 100, LineSide::Left);
        assert!(!p.flat);
        assert_eq!(p.xy_steep, 1023);
        assert_eq!(p.yx_steep, 1023);
        assert_eq!(p.steep, 1023);
        assert_eq!(p.spx, 255);
        assert!(!p.inv);

        // Flat line going down-right: yx_steep > 0, TOP keeps inv = 1.
        line_points_init(&mut p, 0, 0, 100, 50, LineSide::Top);
        assert!(p.flat);
        assert_eq!(p.yx_steep, (10485 * 50) >> 10);
        assert_eq!(p.xy_steep, ((1 << 20) / 50 * 100) >> 10);
        assert_eq!(p.steep, p.yx_steep);
        assert!(p.inv);
        line_points_init(&mut p, 0, 0, 100, 50, LineSide::Bottom);
        assert!(!p.inv);

        // Negative steep: spx is the absolute value.
        line_points_init(&mut p, 100, 0, 0, 50, LineSide::Top);
        assert!(p.flat);
        assert!(p.steep < 0);
        assert_eq!(p.spx, -(p.steep >> 2));
        assert!(!p.inv);
    }

    #[test]
    fn vertical_line_mask_special_cases() {
        let mut p = MaskLineParam::default();
        line_points_init(&mut p, 10, 0, 10, 100, LineSide::Left);
        let mut buf = full(20);
        assert_eq!(mask_line(&mut buf, 0, 5, 20, &mut p), MaskRes::Changed);
        assert!(buf[..10].iter().all(|&o| o == 255));
        assert!(buf[10..].iter().all(|&o| o == 0));

        // Completely on the left of the line.
        let mut buf = full(5);
        assert_eq!(mask_line(&mut buf, 0, 5, 5, &mut p), MaskRes::FullCover);
        // Completely on the right of the line.
        assert_eq!(mask_line(&mut buf, 11, 5, 5, &mut p), MaskRes::Transp);

        line_points_init(&mut p, 10, 0, 10, 100, LineSide::Right);
        let mut buf = full(20);
        assert_eq!(mask_line(&mut buf, 0, 5, 20, &mut p), MaskRes::Changed);
        assert!(buf[..10].iter().all(|&o| o == 0));
        assert!(buf[10..].iter().all(|&o| o == 255));
        assert_eq!(mask_line(&mut buf, 11, 5, 5, &mut p), MaskRes::FullCover);
        assert_eq!(mask_line(&mut buf, 0, 5, 5, &mut p), MaskRes::Transp);

        // Top/bottom of a vertical line is nonsense: full cover.
        line_points_init(&mut p, 10, 0, 10, 100, LineSide::Top);
        assert_eq!(mask_line(&mut buf, 0, 5, 5, &mut p), MaskRes::FullCover);
    }

    #[test]
    fn horizontal_line_mask_special_cases() {
        let mut p = MaskLineParam::default();
        let mut buf = full(10);
        line_points_init(&mut p, 0, 10, 100, 10, LineSide::Top);
        assert_eq!(mask_line(&mut buf, 0, 5, 10, &mut p), MaskRes::FullCover);
        assert_eq!(mask_line(&mut buf, 0, 10, 10, &mut p), MaskRes::Transp);
        assert_eq!(mask_line(&mut buf, 0, 15, 10, &mut p), MaskRes::Transp);

        line_points_init(&mut p, 0, 10, 100, 10, LineSide::Bottom);
        assert_eq!(mask_line(&mut buf, 0, 10, 10, &mut p), MaskRes::FullCover);
        assert_eq!(mask_line(&mut buf, 0, 9, 10, &mut p), MaskRes::Transp);
        assert_eq!(mask_line(&mut buf, 0, 5, 10, &mut p), MaskRes::Transp);

        line_points_init(&mut p, 0, 10, 100, 10, LineSide::Left);
        assert_eq!(mask_line(&mut buf, 0, 50, 10, &mut p), MaskRes::FullCover);
        assert!(buf.iter().all(|&o| o == 255));
    }

    /// Values traced by hand through `line_mask_steep()` for the 45 degree
    /// line (0,0)-(100,100), side LEFT, row 50 starting at x = 0.
    #[test]
    fn steep_line_row_values() {
        let mut p = MaskLineParam::default();
        line_points_init(&mut p, 0, 0, 100, 100, LineSide::Left);
        let mut buf = full(100);
        assert_eq!(mask_line(&mut buf, 0, 50, 100, &mut p), MaskRes::Changed);
        assert!(buf[..=49].iter().all(|&o| o == 255));
        assert_eq!(buf[50], 115);
        assert!(buf[51..].iter().all(|&o| o == 0));

        // The RIGHT side is the complement of the LEFT side.
        line_points_init(&mut p, 0, 0, 100, 100, LineSide::Right);
        let mut buf = full(100);
        assert_eq!(mask_line(&mut buf, 0, 50, 100, &mut p), MaskRes::Changed);
        assert!(buf[..=48].iter().all(|&o| o == 0));
        assert_eq!(buf[49], 0);
        assert_eq!(buf[50], 255 - 115);
        assert!(buf[51..].iter().all(|&o| o == 255));

        // Far away from the line on either side.
        let mut buf = full(10);
        line_points_init(&mut p, 0, 0, 100, 100, LineSide::Left);
        assert_eq!(mask_line(&mut buf, 0, 50, 10, &mut p), MaskRes::FullCover);
        assert_eq!(mask_line(&mut buf, 80, 50, 10, &mut p), MaskRes::Transp);
    }

    #[test]
    fn flat_line_row_is_monotonic_and_complementary() {
        // Flat line (0,0)-(100,50), keep the TOP side.
        let mut top = MaskLineParam::default();
        line_points_init(&mut top, 0, 0, 100, 50, LineSide::Top);
        let mut bottom = MaskLineParam::default();
        line_points_init(&mut bottom, 0, 0, 100, 50, LineSide::Bottom);

        let mut buf_t = full(100);
        let mut buf_b = full(100);
        assert_eq!(mask_line(&mut buf_t, 0, 25, 100, &mut top), MaskRes::Changed);
        assert_eq!(mask_line(&mut buf_b, 0, 25, 100, &mut bottom), MaskRes::Changed);
        // The line crosses y = 25 around x = 50: left of it is below the line
        // (bottom side), right of it is above (top side).
        assert_eq!(buf_t[0], 0);
        assert_eq!(buf_t[99], 255);
        assert_eq!(buf_b[0], 255);
        assert_eq!(buf_b[99], 0);
        // Anti-aliasing ramps are monotonic.
        assert!(buf_t.windows(2).all(|w| w[0] <= w[1]));
        assert!(buf_b.windows(2).all(|w| w[0] >= w[1]));
        // Top/bottom are complementary wherever the mask was touched.
        for x in 0..100 {
            assert_eq!(buf_t[x] as u32 + buf_b[x] as u32, 255, "x = {x}");
        }

        // Rows far above / below the line.
        let mut buf = full(100);
        assert_eq!(mask_line(&mut buf, 0, -10, 100, &mut top), MaskRes::FullCover);
        assert_eq!(mask_line(&mut buf, 0, 60, 100, &mut top), MaskRes::Transp);
        assert_eq!(mask_line(&mut buf, 0, -10, 100, &mut bottom), MaskRes::Transp);
        assert_eq!(mask_line(&mut buf, 0, 60, 100, &mut bottom), MaskRes::FullCover);
    }

    #[test]
    fn line_angle_init_matches_points() {
        // 90 degrees points straight down (LVGL y grows downwards).
        let mut p = MaskLineParam::default();
        line_angle_init(&mut p, 50, 50, 90, LineSide::Right);
        assert_eq!(p.p1, Point::new(50, 50));
        assert_eq!(p.p2, Point::new(50, 50 + (32768 >> 5)));
        assert!(!p.flat);
        assert_eq!(p.steep, 0);

        // > 180 is folded back by 180 so that the origo stays in place.
        let mut q = MaskLineParam::default();
        line_angle_init(&mut q, 50, 50, 270, LineSide::Right);
        assert_eq!(q.p1, p.p1);
        assert_eq!(q.p2, p.p2);
    }

    #[test]
    fn angle_init_fields() {
        let mut a = MaskAngleParam::default();
        angle_init(&mut a, 50, 50, 30, 120);
        assert_eq!(a.vertex_p, Point::new(50, 50));
        assert_eq!(a.start_angle, 30);
        assert_eq!(a.end_angle, 120);
        assert_eq!(a.delta_deg, 90);
        assert_eq!(a.start_line.side, LineSide::Left);
        assert_eq!(a.end_line.side, LineSide::Right);

        angle_init(&mut a, 0, 0, 300, 45);
        assert_eq!(a.delta_deg, 360 - 300 + 45);
        assert_eq!(a.start_line.side, LineSide::Right);
        assert_eq!(a.end_line.side, LineSide::Right);

        // Constrained to 0..=359.
        angle_init(&mut a, 0, 0, -20, 400);
        assert_eq!(a.start_angle, 0);
        assert_eq!(a.end_angle, 359);
        assert_eq!(a.delta_deg, 359);
        assert_eq!(a.end_line.side, LineSide::Left);
    }

    #[test]
    fn angle_mask_quadrant() {
        // 0..90 keeps the bottom-right quadrant of the vertex.
        let mut a = MaskAngleParam::default();
        angle_init(&mut a, 50, 50, 0, 90);

        let mut buf = full(100);
        assert_eq!(mask_angle(&mut buf, 0, 20, 100, &mut a), MaskRes::Transp);

        let mut buf = full(100);
        assert_eq!(mask_angle(&mut buf, 0, 60, 100, &mut a), MaskRes::Changed);
        assert!(buf[..50].iter().all(|&o| o == 0));
        assert!(buf[50..].iter().all(|&o| o == 255));

        // Entirely right of the vertex and below it: full cover.
        let mut buf = full(10);
        assert_eq!(mask_angle(&mut buf, 60, 60, 10, &mut a), MaskRes::FullCover);
    }

    #[test]
    fn angle_mask_start_greater_than_end_same_half() {
        // start > end, both < 180: the two line masks split the row.
        let mut a = MaskAngleParam::default();
        angle_init(&mut a, 50, 50, 135, 45);
        let mut buf = full(100);
        assert_eq!(mask_angle(&mut buf, 0, 20, 100, &mut a), MaskRes::FullCover);
        let mut buf = full(100);
        let res = mask_angle(&mut buf, 0, 80, 100, &mut a);
        assert_eq!(res, MaskRes::Changed);
        // The wedge 45..135 (below the vertex) is cut out around x = 50.
        assert_eq!(buf[50], 0);
        assert_eq!(buf[0], 255);
        assert_eq!(buf[99], 255);

        // start > end, both > 180: mirrored above the vertex.
        angle_init(&mut a, 50, 50, 315, 225);
        let mut buf = full(100);
        assert_eq!(mask_angle(&mut buf, 0, 80, 100, &mut a), MaskRes::FullCover);
        let mut buf = full(100);
        assert_eq!(mask_angle(&mut buf, 0, 20, 100, &mut a), MaskRes::Changed);
        assert_eq!(buf[50], 0);
        assert_eq!(buf[0], 255);
        assert_eq!(buf[99], 255);
    }

    #[test]
    fn circ_calc_radius_1_and_2() {
        let mut c = RadiusCircleDsc::default();
        assert_eq!(circ_calc_aa4(&mut c, 1), LvResult::Ok);
        assert_eq!(c.radius, 1);
        assert_eq!(c.cir_opa[0], 180);
        assert_eq!(&c.opa_start_on_y[..], &[0, 1]);
        assert_eq!(c.x_start_on_y[0], 0);
        let (opa, len, x_start) = get_next_line(&c, 0);
        assert_eq!((opa, len, x_start), (&[180u8][..], 1, 0));

        // Traced by hand through the C code for radius 2.
        let mut c = RadiusCircleDsc::default();
        assert_eq!(circ_calc_aa4(&mut c, 2), LvResult::Ok);
        assert_eq!(&c.cir_opa[..5], &[0, 224, 80, 224, 0]);
        assert_eq!(&c.opa_start_on_y[..], &[0, 2, 4]);
        assert_eq!(&c.x_start_on_y[..], &[1, 0, 0]);
    }

    #[test]
    fn circ_calc_invariants() {
        for radius in 1..=200 {
            let mut c = RadiusCircleDsc::default();
            assert_eq!(circ_calc_aa4(&mut c, radius), LvResult::Ok);
            let r = radius as usize;
            assert_eq!(c.cir_opa.len(), 2 * r + 2);
            assert_eq!(c.opa_start_on_y.len(), r + 1);
            assert_eq!(c.x_start_on_y.len(), r + 1);
            // Rows are consecutive, non-overlapping slices of cir_opa.
            assert!(c.opa_start_on_y.windows(2).all(|w| w[0] <= w[1]));
            assert!((c.opa_start_on_y[r] as usize) <= c.cir_opa.len());
            // Every row that the radius mask may ask for exists.
            for y in 0..radius {
                let (opa, len, x_start) = get_next_line(&c, y);
                assert_eq!(opa.len() as i32, len);
                assert!(x_start <= radius, "radius {radius} y {y}");
            }
        }
    }

    #[test]
    fn circle_cache_policy() {
        let mut cache = CircleCache::default();
        let rect = Area::new(0, 0, 99, 99);

        let mut p1 = MaskRadiusParam::default();
        assert_eq!(radius_init(&mut cache, &mut p1, &rect, 10, false), LvResult::Ok);
        let CircleRef::Cached(i1) = p1.circle else {
            panic!("expected a cached circle");
        };
        assert_eq!(cache.entries[i1].radius, 10);
        assert_eq!(cache.entries[i1].used_cnt, 1);
        assert_eq!(cache.entries[i1].life, 1);

        // Same radius: same entry, used_cnt and life grow.
        let mut p2 = MaskRadiusParam::default();
        assert_eq!(radius_init(&mut cache, &mut p2, &rect, 10, true), LvResult::Ok);
        assert!(matches!(p2.circle, CircleRef::Cached(i) if i == i1));
        assert_eq!(cache.entries[i1].used_cnt, 2);
        assert_eq!(cache.entries[i1].life, 2);
        assert!(p2.outer);

        // Fill the remaining entries with other radii.
        let mut others = Vec::new();
        for r in [20, 32, 48] {
            let mut p = MaskRadiusParam::default();
            assert_eq!(radius_init(&mut cache, &mut p, &rect, r, false), LvResult::Ok);
            assert!(matches!(p.circle, CircleRef::Cached(_)));
            others.push(p);
        }
        // r >= 16 ages by r >> 4.
        let CircleRef::Cached(i48) = others[2].circle else {
            panic!("expected a cached circle");
        };
        assert_eq!(cache.entries[i48].life, 3);

        // Every entry is in use: a fifth radius is not cached.
        let mut p5 = MaskRadiusParam::default();
        assert_eq!(radius_init(&mut cache, &mut p5, &rect, 7, false), LvResult::Ok);
        let CircleRef::Owned(ref c) = p5.circle else {
            panic!("expected an owned circle");
        };
        assert_eq!(c.life, -1);
        assert_eq!(c.radius, 7);

        // Releasing clears the reference and decrements used_cnt.
        let mut m1 = MaskParam::Radius(p1);
        free_param(&mut cache, &mut m1);
        assert!(matches!(
            m1,
            MaskParam::Radius(MaskRadiusParam {
                circle: CircleRef::None,
                ..
            })
        ));
        assert_eq!(cache.entries[i1].used_cnt, 1);
        let mut m2 = MaskParam::Radius(p2);
        free_param(&mut cache, &mut m2);
        assert_eq!(cache.entries[i1].used_cnt, 0);
        let mut m5 = MaskParam::Radius(p5);
        free_param(&mut cache, &mut m5);

        // Release radius 48 too: now two entries are free (life 2 and 3);
        // a new radius evicts the one with the lowest life (radius 10).
        let mut m48 = MaskParam::Radius(others.pop().unwrap());
        free_param(&mut cache, &mut m48);
        assert_eq!(cache.entries[i48].used_cnt, 0);
        let mut p6 = MaskRadiusParam::default();
        assert_eq!(radius_init(&mut cache, &mut p6, &rect, 9, false), LvResult::Ok);
        assert!(matches!(p6.circle, CircleRef::Cached(i) if i == i1));
        assert_eq!(cache.entries[i1].radius, 9);
        assert_eq!(cache.entries[i1].used_cnt, 1);
        assert_eq!(cache.entries[i1].life, 1);
        assert_eq!(cache.entries[i48].radius, 48);

        // A released entry is still a cache hit.
        let mut p7 = MaskRadiusParam::default();
        assert_eq!(radius_init(&mut cache, &mut p7, &rect, 48, false), LvResult::Ok);
        assert!(matches!(p7.circle, CircleRef::Cached(i) if i == i48));
        assert_eq!(cache.entries[i48].used_cnt, 1);

        cleanup(&mut cache);
        for e in &cache.entries {
            assert_eq!(e.radius, 0);
            assert_eq!(e.used_cnt, 0);
            assert_eq!(e.life, 0);
            assert!(e.cir_opa.is_empty());
        }
    }

    #[test]
    fn radius_init_clamps_radius() {
        let mut cache = CircleCache::default();
        let rect = Area::new(0, 0, 19, 49);
        let mut p = MaskRadiusParam::default();
        assert_eq!(radius_init(&mut cache, &mut p, &rect, 100, false), LvResult::Ok);
        assert_eq!(p.radius, 10);
        assert_eq!(p.rect, rect);
        assert!(matches!(p.circle, CircleRef::Cached(_)));

        assert_eq!(radius_init(&mut cache, &mut p, &rect, -5, false), LvResult::Ok);
        assert_eq!(p.radius, 0);
        assert!(matches!(p.circle, CircleRef::None));
    }

    #[test]
    fn radius_mask_rectangle_part() {
        let mut cache = CircleCache::default();
        let rect = Area::new(10, 10, 19, 19);
        let mut p = MaskRadiusParam::default();
        assert_eq!(radius_init(&mut cache, &mut p, &rect, 0, false), LvResult::Ok);

        let mut buf = full(30);
        assert_eq!(mask_radius(&cache, &mut buf, 0, 15, 30, &mut p), MaskRes::Changed);
        assert!(buf[..10].iter().all(|&o| o == 0));
        assert!(buf[10..20].iter().all(|&o| o == 255));
        assert!(buf[20..].iter().all(|&o| o == 0));

        // Rows outside the rectangle.
        assert_eq!(mask_radius(&cache, &mut buf, 0, 5, 30, &mut p), MaskRes::Transp);
        assert_eq!(mask_radius(&cache, &mut buf, 0, 25, 30, &mut p), MaskRes::Transp);
        // Exactly the rectangle width: full cover.
        let mut buf = full(10);
        assert_eq!(
            mask_radius(&cache, &mut buf, 10, 15, 10, &mut p),
            MaskRes::FullCover
        );
        // Entirely outside horizontally.
        assert_eq!(mask_radius(&cache, &mut buf, 20, 15, 10, &mut p), MaskRes::Transp);
        assert_eq!(mask_radius(&cache, &mut buf, 30, 15, 10, &mut p), MaskRes::Transp);

        // Inverse: keep the outside.
        let mut p = MaskRadiusParam::default();
        assert_eq!(radius_init(&mut cache, &mut p, &rect, 0, true), LvResult::Ok);
        let mut buf = full(30);
        assert_eq!(mask_radius(&cache, &mut buf, 0, 15, 30, &mut p), MaskRes::Changed);
        assert!(buf[..10].iter().all(|&o| o == 255));
        assert!(buf[10..20].iter().all(|&o| o == 0));
        assert!(buf[20..].iter().all(|&o| o == 255));
        assert_eq!(
            mask_radius(&cache, &mut buf, 0, 5, 30, &mut p),
            MaskRes::FullCover
        );
    }

    #[test]
    fn radius_mask_corners_are_symmetric_and_complementary() {
        let mut cache = CircleCache::default();
        let rect = Area::new(0, 0, 39, 39);
        let mut inner = MaskRadiusParam::default();
        let mut outer = MaskRadiusParam::default();
        assert_eq!(
            radius_init(&mut cache, &mut inner, &rect, 10, false),
            LvResult::Ok
        );
        assert_eq!(radius_init(&mut cache, &mut outer, &rect, 10, true), LvResult::Ok);

        for y in 0..40 {
            let mut bi = full(40);
            let mut bo = full(40);
            // The straight middle rows span exactly the rectangle width:
            // `last == 0 && first == len` reports FULL_COVER for the inner
            // mask, while the outer mask always reports CHANGED.
            let expected = if (10..30).contains(&y) {
                MaskRes::FullCover
            } else {
                MaskRes::Changed
            };
            assert_eq!(
                mask_radius(&cache, &mut bi, 0, y, 40, &mut inner),
                expected,
                "y {y}"
            );
            assert_eq!(
                mask_radius(&cache, &mut bo, 0, y, 40, &mut outer),
                MaskRes::Changed
            );
            // Left/right symmetry.
            for x in 0..20 {
                assert_eq!(bi[x], bi[39 - x], "y {y} x {x}");
            }
            // Top/bottom symmetry.
            let mut bm = full(40);
            mask_radius(&cache, &mut bm, 0, 39 - y, 40, &mut inner);
            assert_eq!(bi, bm, "row {y}");
            // Inner + outer == cover for every pixel.
            for x in 0..40 {
                assert_eq!(bi[x] as u32 + bo[x] as u32, 255, "y {y} x {x}");
            }
            if y >= 10 && y < 30 {
                assert!(bi.iter().all(|&o| o == 255));
            } else {
                // The middle is kept; the corner pixel is cut except on the
                // innermost corner row, whose only anti-aliased pixel is the
                // zero-opacity "edge" entry at x_start == radius.
                assert_eq!(bi[20], 255);
                if y == 9 || y == 30 {
                    assert_eq!(bi[0], 255, "y {y}");
                } else {
                    assert!(bi[0] < 255, "y {y}");
                }
            }
        }
        // Row 0 of a radius-10 corner: nearly everything is transparent.
        let mut b = full(40);
        mask_radius(&cache, &mut b, 0, 0, 40, &mut inner);
        assert!(b[..5].iter().all(|&o| o == 0));
    }

    /// `test_radius_mask_overflow()` from `test_draw_sw_mask.c`, pure part:
    /// a 2560 x 2560 rounded rectangle with radius 1280 must not overflow
    /// the `uint16_t` circle tables nor index out of bounds.
    #[test]
    fn radius_mask_overflow() {
        let (width, height) = (1280, 1280);
        let mut cache = CircleCache::default();
        let rect = Area::new(0, 0, width * 2 - 1, height * 2 - 1);
        let mut p = MaskRadiusParam::default();
        assert_eq!(
            radius_init(&mut cache, &mut p, &rect, height, false),
            LvResult::Ok
        );
        assert_eq!(p.radius, 1280);

        let len = (width * 2) as usize;
        for y in [0, 1, 640, 1279, 1280, 2558, 2559] {
            let mut buf = full(len);
            assert_eq!(
                mask_radius(&cache, &mut buf, 0, y, len as i32, &mut p),
                MaskRes::Changed
            );
            for x in 0..len / 2 {
                assert_eq!(buf[x], buf[len - 1 - x]);
            }
            assert_eq!(buf[len / 2], 255);
        }
        let mut m = MaskParam::Radius(p);
        free_param(&mut cache, &mut m);
    }

    #[test]
    fn fade_mask() {
        let coords = Area::new(10, 0, 29, 99);
        let mut p = MaskFadeParam::default();
        fade_init(&mut p, &coords, 255, 10, 0, 90);
        assert_eq!(p.coords, coords);
        assert_eq!((p.opa_top, p.y_top, p.opa_bottom, p.y_bottom), (255, 10, 0, 90));

        // Outside vertically / horizontally.
        let mut buf = full(40);
        assert_eq!(mask_fade(&mut buf, 0, -1, 40, &mut p), MaskRes::FullCover);
        assert_eq!(mask_fade(&mut buf, 0, 100, 40, &mut p), MaskRes::FullCover);
        assert_eq!(mask_fade(&mut buf, 0, 50, 9, &mut p), MaskRes::FullCover);
        assert_eq!(mask_fade(&mut buf, 30, 50, 10, &mut p), MaskRes::FullCover);

        // Above y_top: opa_top (255 -> unchanged), only inside coords.
        let mut buf = full(40);
        assert_eq!(mask_fade(&mut buf, 0, 5, 40, &mut p), MaskRes::Changed);
        assert!(buf.iter().all(|&o| o == 255));

        // Below y_bottom: opa_bottom (0) inside coords, untouched outside.
        let mut buf = full(40);
        assert_eq!(mask_fade(&mut buf, 0, 95, 40, &mut p), MaskRes::Changed);
        assert!(buf[..10].iter().all(|&o| o == 255));
        assert!(buf[10..30].iter().all(|&o| o == 0));
        assert!(buf[30..].iter().all(|&o| o == 255));

        // In between: LV_OPA_MIX2(y - y_top, opa_diff) / y_diff + opa_top.
        let y = 50;
        let opa_diff: i32 = -255;
        let y_diff = 90 - 10 + 1;
        // LV_OPA_MIX2 casts to lv_opa_t before the division: (uint8)-40 = 216, 216 / 81 = 2,
        // 2 + 255 wraps to 1.
        let expected = (((((y - 10) * opa_diff) >> 8) as u8 as i32 / y_diff) as u8).wrapping_add(255);
        assert_eq!(expected, 1);
        let mut buf = full(40);
        assert_eq!(mask_fade(&mut buf, 0, y, 40, &mut p), MaskRes::Changed);
        assert_eq!(buf[15], mask_mix(255, expected));
        assert_eq!(buf[15], 0);
        assert!(buf[10..30].iter().all(|&o| o == 0));
        assert_eq!(buf[0], 255);
        assert_eq!(buf[35], 255);

        // A fade with a non-trivial middle value.
        let mut p = MaskFadeParam::default();
        fade_init(&mut p, &Area::new(0, 0, 9, 9), 0, 0, 255, 1);
        let mut buf = full(10);
        assert_eq!(mask_fade(&mut buf, 0, 0, 10, &mut p), MaskRes::Changed);
        assert!(buf.iter().all(|&o| o == 0));
        let mut buf = full(10);
        assert_eq!(mask_fade(&mut buf, 0, 1, 10, &mut p), MaskRes::Changed);
        assert!(buf.iter().all(|&o| o == 255));
    }

    #[test]
    fn fade_mask_degenerate_area_is_noop() {
        // x2 < x1: clipping leaves len = 20 - 14 - 10 = -4; C skips the loop and returns CHANGED.
        let mut p = MaskFadeParam::default();
        fade_init(&mut p, &Area::new(10, 0, 5, 99), 255, 10, 0, 90);
        for y in [5, 50, 95] {
            let mut buf = full(20);
            assert_eq!(mask_fade(&mut buf, 0, y, 20, &mut p), MaskRes::Changed);
            assert!(buf.iter().all(|&o| o == 255));
        }
    }

    #[test]
    fn map_mask_degenerate_area_is_noop() {
        let map: [Opa; 4] = [0; 4];
        let mut p = MaskMapParam {
            coords: Area::default(),
            map: &[],
        };
        map_init(&mut p, &Area::new(10, 0, 5, 3), &map);
        let mut buf = full(20);
        assert_eq!(mask_map(&mut buf, 0, 2, 20, &mut p), MaskRes::Changed);
        assert!(buf.iter().all(|&o| o == 255));
    }

    #[test]
    fn map_mask() {
        let coords = Area::new(2, 2, 5, 5);
        let map: [Opa; 16] = [
            0, 1, 2, 3, //
            10, 20, 30, 255, //
            100, 128, 200, 254, //
            255, 255, 255, 255,
        ];
        let mut p = MaskMapParam {
            coords: Area::default(),
            map: &[],
        };
        map_init(&mut p, &coords, &map);
        assert_eq!(p.coords, coords);
        assert_eq!(p.map.len(), 16);

        let mut buf = full(10);
        assert_eq!(mask_map(&mut buf, 0, 1, 10, &mut p), MaskRes::FullCover);
        assert_eq!(mask_map(&mut buf, 0, 6, 10, &mut p), MaskRes::FullCover);
        assert_eq!(mask_map(&mut buf, 6, 3, 10, &mut p), MaskRes::FullCover);
        assert_eq!(mask_map(&mut buf, 0, 3, 1, &mut p), MaskRes::FullCover);
        assert!(buf.iter().all(|&o| o == 255));

        // Row 3 is the second map row; the buffer starts left of coords.
        let mut buf = full(10);
        assert_eq!(mask_map(&mut buf, 0, 3, 10, &mut p), MaskRes::Changed);
        assert_eq!(&buf[..2], &[255, 255]);
        assert_eq!(buf[2], mask_mix(255, 10));
        assert_eq!(buf[3], mask_mix(255, 20));
        assert_eq!(buf[4], mask_mix(255, 30));
        assert_eq!(buf[5], 255);
        assert!(buf[6..].iter().all(|&o| o == 255));

        // Buffer starting inside coords and ending past them.
        let mut buf = full(10);
        assert_eq!(mask_map(&mut buf, 4, 4, 10, &mut p), MaskRes::Changed);
        assert_eq!(buf[0], mask_mix(255, 200));
        // 254 >= LV_OPA_MAX (253): mask_mix keeps the current value.
        assert_eq!(buf[1], 255);
        assert!(buf[2..].iter().all(|&o| o == 255));

        // The first row has values <= LV_OPA_MIN: fully transparent.
        let mut buf = full(4);
        assert_eq!(mask_map(&mut buf, 2, 2, 4, &mut p), MaskRes::Changed);
        assert_eq!(&buf[..], &[0, 0, 0, mask_mix(255, 3)]);
    }

    #[test]
    fn apply_combines_results() {
        let mut cache = CircleCache::default();
        let rect = Area::new(10, 10, 19, 19);
        let mut r = MaskRadiusParam::default();
        assert_eq!(radius_init(&mut cache, &mut r, &rect, 0, false), LvResult::Ok);
        let mut l = MaskLineParam::default();
        line_points_init(&mut l, 15, 0, 15, 100, LineSide::Left);

        // Nothing changed: full cover.
        let mut f = MaskFadeParam::default();
        fade_init(&mut f, &Area::new(50, 50, 60, 60), 0, 50, 0, 60);
        let mut masks = [MaskParam::Fade(f)];
        let mut buf = full(30);
        assert_eq!(
            apply(&mut cache, &mut masks, &mut buf, 0, 15, 30),
            MaskRes::FullCover
        );
        assert!(buf.iter().all(|&o| o == 255));

        // Both masks apply: only x in 10..15 survives.
        let mut masks = [MaskParam::Radius(r.clone()), MaskParam::Line(l.clone())];
        let mut buf = full(30);
        assert_eq!(
            apply(&mut cache, &mut masks, &mut buf, 0, 15, 30),
            MaskRes::Changed
        );
        assert!(buf[..10].iter().all(|&o| o == 0));
        assert!(buf[10..15].iter().all(|&o| o == 255));
        assert!(buf[15..].iter().all(|&o| o == 0));

        // A transparent mask short-circuits.
        let mut masks = [MaskParam::Radius(r), MaskParam::Line(l)];
        let mut buf = full(30);
        assert_eq!(apply(&mut cache, &mut masks, &mut buf, 0, 5, 30), MaskRes::Transp);

        // No masks: full cover.
        assert_eq!(apply(&mut cache, &mut [], &mut buf, 0, 5, 30), MaskRes::FullCover);

        for m in masks.iter_mut() {
            free_param(&mut cache, m);
        }
        init(&mut cache);
        deinit(&mut cache);
    }
}
