//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_grad.c`, `src/draw/sw/lv_draw_sw_grad.h`.
//!
//! Gradient colour maps (`lv_draw_sw_grad_calc_t`) and the per-draw state of
//! linear / radial / conical gradients (`complex-gradients` feature).

use alloc::vec::Vec;

use ferrovgl_core::draw::grad::{GradDir, GradDsc};
use ferrovgl_core::misc::area::Area;
use ferrovgl_core::misc::color::{Color, Opa};
use ferrovgl_core::misc::math::udiv255;

#[cfg(feature = "complex-gradients")]
use ferrovgl_core::draw::grad::{GradExtend, GradParams};
#[cfg(feature = "complex-gradients")]
use ferrovgl_core::misc::area::Point;
#[cfg(feature = "complex-gradients")]
use ferrovgl_core::misc::math::{atan2, sqrt32};
#[cfg(feature = "complex-gradients")]
use ferrovgl_core::misc::types::coord::pct_to_px;

/// `lv_draw_sw_grad_calc_t` (`lv_grad_t`): a computed gradient colour map.
#[derive(Clone, Debug, Default)]
pub struct GradCalc {
    /// `color_map`
    pub color_map: Vec<Color>,
    /// `opa_map`
    pub opa_map: Vec<Opa>,
    /// `size`
    pub size: u32,
}

/// `lv_grad_linear_state_t`
#[derive(Clone, Debug, Default)]
pub struct GradLinearState {
    /// `a` of `w = a * xp + b * yp + c`
    pub a: i32,
    /// `b`
    pub b: i32,
    /// `c`
    pub c: i32,
    /// `cgrad`: 256-entry colour map cache.
    pub cgrad: GradCalc,
}

/// `lv_grad_radial_state_t`
#[derive(Clone, Debug, Default)]
pub struct GradRadialState {
    /// `x0`: center of the start circle.
    pub x0: i32,
    /// `y0`
    pub y0: i32,
    /// `r0`: radius of the start circle.
    pub r0: i32,
    /// `inv_dr`: `1 / (r1 - r0)`
    pub inv_dr: i32,
    /// `a4`: `4 * a`
    pub a4: i32,
    /// `inv_a4`: `1 / (4 * a)`
    pub inv_a4: i32,
    /// `dx`
    pub dx: i32,
    /// `bpx` of `b(xp, yp) = xp * bpx + yp * bpy + bc`
    pub bpx: i32,
    /// `bpy`
    pub bpy: i32,
    /// `bc`
    pub bc: i32,
    /// `clip_area`
    pub clip_area: Area,
    /// `cgrad`
    pub cgrad: GradCalc,
}

/// `lv_grad_conical_state_t`
#[derive(Clone, Debug, Default)]
pub struct GradConicalState {
    /// `x0`
    pub x0: i32,
    /// `y0`
    pub y0: i32,
    /// `a`
    pub a: i32,
    /// `da`
    pub da: i32,
    /// `inv_da`
    pub inv_da: i32,
    /// `cgrad`
    pub cgrad: GradCalc,
}

/// Sentinel stored in `clip_area.x1` when the radial gradient is not clipped
/// (`-0x7fffffff` upstream).
#[cfg(feature = "complex-gradients")]
const RADIAL_NO_CLIP: i32 = -0x7fff_ffff;

/// `lv_sqr()`: `x * x` with C `int32_t` wrap-around.
#[cfg(feature = "complex-gradients")]
#[inline]
fn sqr(x: i32) -> i32 {
    x.wrapping_mul(x)
}

/**********************
 *   STATIC FUNCTIONS
 **********************/

/// `allocate_item()`: an uninitialised map sized for the gradient direction.
fn allocate_item(g: &GradDsc, w: i32, h: i32) -> GradCalc {
    let size: i32 = match g.dir {
        GradDir::Hor | GradDir::Linear | GradDir::Radial | GradDir::Conical => w,
        GradDir::Ver => h,
        GradDir::None => 64,
    };
    // `item->size` is `uint32_t`, assigned from the `int32_t` size.
    let size = size as u32;
    let n = size as usize;
    GradCalc {
        color_map: alloc::vec![Color::default(); n],
        opa_map: alloc::vec![0; n],
        size,
    }
}

/// `extend_w()`: map a raw position onto the 256-entry colour map according
/// to the extend mode.
#[cfg(feature = "complex-gradients")]
#[inline]
fn extend_w(w: i32, extend: GradExtend) -> i32 {
    if extend == GradExtend::Pad {
        /* Repeat the same color */
        return if w < 0 { 0 } else { w.min(255) };
    }
    if extend == GradExtend::Repeat {
        /* Repeat the pattern */
        return w & 255;
    }
    /* LV_GRAD_EXTEND_REFLECT */
    let mut w = w & 511;
    if w > 255 {
        w ^= 511; /* 511 - w */
    }
    w
}

/// `dsc->params.linear`. Upstream reads the union member unconditionally; a
/// descriptor holding other parameters reads as all zero here.
#[cfg(feature = "complex-gradients")]
fn linear_params(dsc: &GradDsc) -> (Point, Point) {
    match dsc.params {
        GradParams::Linear { start, end } => (start, end),
        _ => (Point::default(), Point::default()),
    }
}

/// `dsc->params.radial` as `(focal, end, focal_extent, end_extent)`; see
/// [`linear_params`] for the mismatched-variant case.
#[cfg(feature = "complex-gradients")]
fn radial_params(dsc: &GradDsc) -> (Point, Point, Point, Point) {
    match dsc.params {
        GradParams::Radial {
            focal,
            focal_extent,
            end,
            end_extent,
        } => (focal, end, focal_extent, end_extent),
        _ => (
            Point::default(),
            Point::default(),
            Point::default(),
            Point::default(),
        ),
    }
}

/// `dsc->params.conical` as `(center, start_angle, end_angle)`; see
/// [`linear_params`] for the mismatched-variant case.
#[cfg(feature = "complex-gradients")]
fn conical_params(dsc: &GradDsc) -> (Point, i16, i16) {
    match dsc.params {
        GradParams::Conical {
            center,
            start_angle,
            end_angle,
        } => (center, start_angle, end_angle),
        _ => (Point::default(), 0, 0),
    }
}

/**********************
 *     FUNCTIONS
 **********************/

/// `lv_draw_sw_grad_get()`: the colour map of `gradient` for a `w` x `h` area.
///
/// The map has `w` entries for horizontal / linear / radial / conical
/// gradients and `h` entries for vertical ones. Upstream returns `NULL` for
/// `LV_GRAD_DIR_NONE`; here that is an empty map (`size == 0`).
pub fn get(gradient: &GradDsc, w: i32, h: i32) -> GradCalc {
    /* No gradient, no cache */
    if gradient.dir == GradDir::None {
        return GradCalc::default();
    }

    /* Step 1: Search cache for the given key */
    let mut item = allocate_item(gradient, w, h);

    /* Step 3: Fill it with the gradient, as expected */
    for i in 0..item.size {
        let (color, opa) = color_calculate(gradient, item.size as i32, i as i32);
        item.color_map[i as usize] = color;
        item.opa_map[i as usize] = opa;
    }
    item
}

/// `lv_draw_sw_grad_color_calculate()`: colour and opacity at `frac` of `range`.
///
/// Gradients are specified in a virtual `0..=255` range, so this scales the
/// virtual range to the given `range` (`frac` is in `0..=range`).
pub fn color_calculate(dsc: &GradDsc, range: i32, frac: i32) -> (Color, Opa) {
    let stops = &dsc.stops;
    let last = dsc.stops_count as usize - 1;

    /* Clip out-of-bounds first */
    let min = (stops[0].frac as i32).wrapping_mul(range) >> 8;
    if frac <= min {
        return (stops[0].color, stops[0].opa);
    }

    let max = (stops[last].frac as i32).wrapping_mul(range) >> 8;
    if frac >= max {
        return (stops[last].color, stops[last].opa);
    }

    /* Find the 2 closest stop now */
    let mut found_i = 0usize;
    for i in 1..dsc.stops_count as usize {
        let cur = (stops[i].frac as i32).wrapping_mul(range) >> 8;
        if frac <= cur {
            found_i = i;
            break;
        }
    }

    debug_assert!(found_i != 0);

    let one = stops[found_i - 1].color;
    let two = stops[found_i].color;
    let min = (stops[found_i - 1].frac as i32).wrapping_mul(range) >> 8;
    let max = (stops[found_i].frac as i32).wrapping_mul(range) >> 8;
    let d = max - min;

    /* Then interpolate */
    let frac = frac - min;
    // `lv_opa_t mix = (frac * 255) / d;` -- int division truncated to uint8_t.
    let mix = (frac.wrapping_mul(255) / d) as Opa;
    let imix: Opa = 255 - mix;
    let (mix, imix) = (mix as u32, imix as u32);

    let color = Color::make(
        udiv255(two.red as u32 * mix + one.red as u32 * imix) as u8,
        udiv255(two.green as u32 * mix + one.green as u32 * imix) as u8,
        udiv255(two.blue as u32 * mix + one.blue as u32 * imix) as u8,
    );

    let opa = udiv255(stops[found_i].opa as u32 * mix + stops[found_i - 1].opa as u32 * imix) as Opa;
    (color, opa)
}

/// `lv_draw_sw_grad_cleanup()`
pub fn cleanup(grad: GradCalc) {
    drop(grad);
}

/*
    Calculate radial gradient based on the following equation:

    | P - (C1 - C0)w - C0 | = (r1 - r0)w + r0, where

        P: {xp, yp} is the point of interest
        C0: {x0, y0} is the center of the start circle
        C1: {x1, y1} is the center of the end circle
        r0 is the radius of the start circle
        r1 is the radius of the end circle
        w is the unknown variable
        || is the length of the vector

    The above equation can be rewritten as:

    ((r1-r0)^2 - (x1-x0)^2 - (y1-y0)^2) * w^2 + 2*((xp-x0)*(x1-x0) + (yp-y0)*(y1-y0)) * w + (-(xp-x0)^2 - (yp-y0)^) = 0

    The roots of the quadratical equation can be obtained using the well-known formula (-b +- sqrt(b^2 - 4ac)) / 2a
    We only need the more positive root.

    Let's denote
        dx = x1 - x0
        dy = y1 - y0
        dr = r1 - r0

    Thus:

    w = (-b(xp, yp) + sqrt(sqr(b(xp, yp)) - 4 * a * c(xp, yp))) / (2 * a), where

        b(xp, yp) = 2dx * xp + 2dy * yp + 2(r0 * dr - x0 * dx - y0 * dy)
        c(xp, yp) = r0^2 - (xp - x0)^2 - (yp - y0)^2

    Rewrite b(xp, yp) as:

    b(xp, yp) = xp * bpx + yp * bpy + bc, where

        bpx = 2dx
        bpy = 2dy
        bc = 2(r0 * dr - x0 * dx - y0 * dy)

    We can pre-calculate the constants, because they do not depend on the pixel coordinates.

*/

/// `lv_draw_sw_grad_radial_setup()`: precalculate the constants used while
/// rendering a radial gradient in `coords`.
#[cfg(feature = "complex-gradients")]
pub fn radial_setup(dsc: &GradDsc, coords: &Area) -> GradRadialState {
    let (mut start, mut end, mut start_extent, mut end_extent) = radial_params(dsc);
    let mut state = GradRadialState::default();

    /* Convert from percentage coordinates */
    let wdt = coords.width();
    let hgt = coords.height();

    start.x = pct_to_px(start.x, wdt);
    end.x = pct_to_px(end.x, wdt);
    start_extent.x = pct_to_px(start_extent.x, wdt);
    end_extent.x = pct_to_px(end_extent.x, wdt);
    start.y = pct_to_px(start.y, hgt);
    end.y = pct_to_px(end.y, hgt);
    start_extent.y = pct_to_px(start_extent.y, hgt);
    end_extent.y = pct_to_px(end_extent.y, hgt);

    /* Calculate radii */
    // `int16_t r_start = lv_sqrt32(...)`: the root is truncated to 16 bits.
    let r_start = sqrt32(
        sqr(start_extent.x.wrapping_sub(start.x)).wrapping_add(sqr(start_extent.y.wrapping_sub(start.y)))
            as u32,
    ) as i16 as i32;
    let r_end = sqrt32(
        sqr(end_extent.x.wrapping_sub(end.x)).wrapping_add(sqr(end_extent.y.wrapping_sub(end.y))) as u32,
    ) as i16 as i32;
    debug_assert!(r_end != 0);

    /* Create gradient color map */
    state.cgrad = get(dsc, 256, 0);

    state.x0 = start.x;
    state.y0 = start.y;
    state.r0 = r_start;
    let dr = r_end - r_start;
    if end.x == start.x && end.y == start.y {
        debug_assert!(dr != 0);
        state.a4 = sqr(dr) << 2;
        state.bpx = 0;
        state.bpy = 0;
        state.bc = state.r0.wrapping_mul(dr) << 1;
        state.dx = 0;
        state.inv_dr = (1 << (8 + 16)) / dr;
    } else {
        let dx = end.x.wrapping_sub(start.x);
        let dy = end.y.wrapping_sub(start.y);
        state.dx = dx; /* needed for incremental calculation */
        state.a4 = sqr(dr).wrapping_sub(sqr(dx)).wrapping_sub(sqr(dy)) << 2;
        /* b(xp, yp) = xp * bpx + yp * bpy + bc */
        state.bpx = dx << 1;
        state.bpy = dy << 1;
        state.bc = state
            .r0
            .wrapping_mul(dr)
            .wrapping_sub(state.x0.wrapping_mul(dx))
            .wrapping_sub(state.y0.wrapping_mul(dy))
            << 1;
    }
    state.inv_a4 = if state.a4 != 0 {
        (1 << (13 + 16)) / state.a4
    } else {
        0
    };
    /* check for possible clipping */
    if dsc.extend == GradExtend::Pad
        /* if extend mode is 'pad', then we can clip to the end circle's bounding box, if the start circle is entirely within the end circle */
        && (sqr(start.x.wrapping_sub(end.x)).wrapping_add(sqr(start.y.wrapping_sub(end.y))) < sqr(r_end - r_start))
    {
        if r_end > r_start {
            state.clip_area.set(
                end.x.wrapping_sub(r_end),
                end.y.wrapping_sub(r_end),
                end.x.wrapping_add(r_end),
                end.y.wrapping_add(r_end),
            );
        } else {
            state.clip_area.set(
                start.x.wrapping_sub(r_start),
                start.y.wrapping_sub(r_start),
                start.x.wrapping_add(r_start),
                start.y.wrapping_add(r_start),
            );
        }
    } else {
        state.clip_area.x1 = RADIAL_NO_CLIP;
    }
    state
}

/// `lv_draw_sw_grad_radial_cleanup()`: the state owns its colour map, so
/// this only drops it.
#[cfg(feature = "complex-gradients")]
pub fn radial_cleanup(state: GradRadialState) {
    drop(state);
}

/// `lv_draw_sw_grad_radial_get_line()`: fill the first `width` entries of
/// `result` with the gradient line starting at `(xp, yp)` in gradient space.
#[cfg(feature = "complex-gradients")]
pub fn radial_get_line(
    dsc: &GradDsc,
    state: &GradRadialState,
    xp: i32,
    yp: i32,
    width: i32,
    result: &mut GradCalc,
) {
    let mut xp = xp;
    let mut width = width;
    let grad = &state.cgrad;
    /* offset of `buf` / `opa` from the start of the result maps */
    let mut ofs = 0usize;

    /* check for possible clipping */
    if state.clip_area.x1 != RADIAL_NO_CLIP {
        /* fill line with end color for pixels outside the clipped region */
        let c = grad.color_map[255];
        let o = grad.opa_map[255];
        let mut w = width;
        let mut i = 0usize;
        while w > 0 {
            result.color_map[i] = c;
            result.opa_map[i] = o;
            i += 1;
            w -= 1;
        }
        /* is this line fully outside the clip area? */
        if yp < state.clip_area.y1
            || yp >= state.clip_area.y2
            || xp >= state.clip_area.x2
            || xp.wrapping_add(width) < state.clip_area.x1
        {
            return;
        } else {
            /* not fully outside: clip line to the bounding box */
            let x1 = xp.max(state.clip_area.x1);
            let x2 = xp.wrapping_add(width).min(state.clip_area.x2);
            ofs = (x1 - xp) as usize;
            xp = x1;
            width = x2 - x1;
        }
    }

    let buf = &mut result.color_map[ofs..];
    let opa = &mut result.opa_map[ofs..];

    let mut b = xp
        .wrapping_mul(state.bpx)
        .wrapping_add(yp.wrapping_mul(state.bpy))
        .wrapping_add(state.bc);
    let mut c = sqr(state.r0)
        .wrapping_sub(sqr(xp.wrapping_sub(state.x0)))
        .wrapping_sub(sqr(yp.wrapping_sub(state.y0)));
    /* We can save some calculations by using the previous values of b and c */
    let db = state.dx << 1;
    let mut dc = (xp.wrapping_sub(state.x0) << 1).wrapping_add(1);

    let mut i = 0usize;
    if state.a4 == 0 {
        /* not a quadratic equation: solve linear equation: w = -c/b */
        while width > 0 {
            let w = extend_w(
                if b == 0 {
                    0
                } else {
                    (c << 8).wrapping_neg().wrapping_div(b)
                },
                dsc.extend,
            );
            buf[i] = grad.color_map[w as usize];
            opa[i] = grad.opa_map[w as usize];
            b = b.wrapping_add(db);
            c = c.wrapping_sub(dc);
            dc = dc.wrapping_add(2);
            i += 1;
            width -= 1;
        }
    } else {
        /* solve quadratical equation */
        if state.bpx != 0 || state.bpy != 0 {
            /* general case (circles are not concentric): w = (-b + sqrt(b^2 - 4ac))/2a (we only need the more positive root)*/
            let a4 = state.a4 >> 4;
            while width > 0 {
                /* b^2 shifted down by 2*4=8, 4ac shifted down by 8 */
                let det = sqr(b >> 4).wrapping_sub(a4.wrapping_mul(c >> 4));
                /* check determinant: if negative, then there is no solution: use starting color */
                let w = if det < 0 {
                    0
                } else {
                    /* square root shifted down by 4 (includes *256 to set output range) */
                    extend_w(
                        (sqrt32(det as u32).wrapping_sub(b >> 4)).wrapping_mul(state.inv_a4) >> 16,
                        dsc.extend,
                    )
                };
                buf[i] = grad.color_map[w as usize];
                opa[i] = grad.opa_map[w as usize];
                b = b.wrapping_add(db);
                c = c.wrapping_sub(dc);
                dc = dc.wrapping_add(2);
                i += 1;
                width -= 1;
            }
        } else {
            /* special case: concentric circles: w = (sqrt((xp-x0)^2 + (yx-y0)^2)-r0)/(r1-r0) */
            c = sqr(xp.wrapping_sub(state.x0)).wrapping_add(sqr(yp.wrapping_sub(state.y0)));
            while width > 0 {
                let w = extend_w(
                    (sqrt32(c as u32).wrapping_sub(state.r0)).wrapping_mul(state.inv_dr) >> 16,
                    dsc.extend,
                );
                buf[i] = grad.color_map[w as usize];
                opa[i] = grad.opa_map[w as usize];
                c = c.wrapping_add(dc);
                dc = dc.wrapping_add(2);
                i += 1;
                width -= 1;
            }
        }
    }
}

/*
    Calculate linear gradient based on the following equation:

    w = ((P - C0) x (C1 - C0)) / | C1 - C0 |^2, where

        P: {xp, yp} is the point of interest
        C0: {x0, y0} is the start point of the gradient vector
        C1: {x1, y1} is the end point of the gradient vector
        w is the unknown variable

        || is the length of the vector
        x is a dot product

    The above equation can be rewritten as:

    w = xp * (dx / (dx^2 + dy^2)) + yp * (dy / (dx^2 + dy^2)) - (x0 * dx + y0 * dy) / (dx^2 + dy^2), where

        dx = x1 - x0
        dy = y1 - y0

    We can pre-calculate the constants, because they do not depend on the pixel coordinates.

*/

/// `lv_draw_sw_grad_linear_setup()`: precalculate the constants used while
/// rendering a linear gradient in `coords`.
#[cfg(feature = "complex-gradients")]
pub fn linear_setup(dsc: &GradDsc, coords: &Area) -> GradLinearState {
    let (mut start, mut end) = linear_params(dsc);

    /* Create gradient color map */
    let cgrad = get(dsc, 256, 0);

    /* Convert from percentage coordinates */
    let wdt = coords.width();
    let hgt = coords.height();

    start.x = pct_to_px(start.x, wdt);
    end.x = pct_to_px(end.x, wdt);
    start.y = pct_to_px(start.y, hgt);
    end.y = pct_to_px(end.y, hgt);

    /* Precalculate constants */
    let dx = end.x.wrapping_sub(start.x);
    let dy = end.y.wrapping_sub(start.y);

    let mut l2 = sqr(dx).wrapping_add(sqr(dy));
    if l2 == 0 {
        l2 = 1;
    }
    GradLinearState {
        a: (dx << 16).wrapping_div(l2),
        b: (dy << 16).wrapping_div(l2),
        c: (start.x.wrapping_mul(dx).wrapping_add(start.y.wrapping_mul(dy)) << 16).wrapping_div(l2),
        cgrad,
    }
}

/// `lv_draw_sw_grad_linear_cleanup()`: the state owns its colour map, so
/// this only drops it.
#[cfg(feature = "complex-gradients")]
pub fn linear_cleanup(state: GradLinearState) {
    drop(state);
}

/// `lv_draw_sw_grad_linear_get_line()`: fill the first `width` entries of
/// `result` with the gradient line starting at `(xp, yp)` in gradient space.
#[cfg(feature = "complex-gradients")]
pub fn linear_get_line(
    dsc: &GradDsc,
    state: &GradLinearState,
    xp: i32,
    yp: i32,
    width: i32,
    result: &mut GradCalc,
) {
    let grad = &state.cgrad;

    let mut x = xp
        .wrapping_mul(state.a)
        .wrapping_add(yp.wrapping_mul(state.b))
        .wrapping_sub(state.c);
    let d = state.a;

    let mut i = 0usize;
    let mut width = width;
    while width > 0 {
        let w = extend_w(x >> 8, dsc.extend);
        result.color_map[i] = grad.color_map[w as usize];
        result.opa_map[i] = grad.opa_map[w as usize];
        x = x.wrapping_add(d);
        i += 1;
        width -= 1;
    }
}

/*
    Calculate conical gradient based on the following equation:

    w = (atan((yp - y0)/(xp - x0)) - alpha) / (beta - alpha), where

        P: {xp, yp} is the point of interest
        C0: {x0, y0} is the center of the gradient
        alpha is the start angle
        beta is the end angle
        w is the unknown variable
*/

/// `lv_draw_sw_grad_conical_setup()`: precalculate the constants used while
/// rendering a conical gradient in `coords`.
#[cfg(feature = "complex-gradients")]
pub fn conical_setup(dsc: &GradDsc, coords: &Area) -> GradConicalState {
    let (mut c0, start_angle, end_angle) = conical_params(dsc);
    let alpha = start_angle as i32 % 360;
    let mut beta = end_angle as i32 % 360;

    /* Create gradient color map */
    let cgrad = get(dsc, 256, 0);

    /* Convert from percentage coordinates */
    let wdt = coords.width();
    let hgt = coords.height();

    c0.x = pct_to_px(c0.x, wdt);
    c0.y = pct_to_px(c0.y, hgt);

    /* Precalculate constants */
    if beta <= alpha {
        beta += 360;
    }
    GradConicalState {
        x0: c0.x,
        y0: c0.y,
        a: alpha,
        da: beta - alpha,
        inv_da: (1 << 16) / (beta - alpha),
        cgrad,
    }
}

/// `lv_draw_sw_grad_conical_cleanup()`: the state owns its colour map, so
/// this only drops it.
#[cfg(feature = "complex-gradients")]
pub fn conical_cleanup(state: GradConicalState) {
    drop(state);
}

/// `lv_draw_sw_grad_conical_get_line()`: fill the first `width` entries of
/// `result` with the gradient line starting at `(xp, yp)` in gradient space.
#[cfg(feature = "complex-gradients")]
pub fn conical_get_line(
    dsc: &GradDsc,
    state: &GradConicalState,
    xp: i32,
    yp: i32,
    width: i32,
    result: &mut GradCalc,
) {
    let grad = &state.cgrad;

    let mut dx = xp.wrapping_sub(state.x0);
    let dy = yp.wrapping_sub(state.y0);

    let mut i = 0usize;
    let mut width = width;
    if dy == 0 {
        /* we will eventually go through the center of the conical: need an extra test in the loop to avoid both dx and dy being zero in atan2 */
        while width > 0 {
            let w = if dx == 0 {
                0
            } else {
                let mut d = atan2(dy, dx) as i32 - state.a;
                if d < 0 {
                    d += 360;
                }
                extend_w(d.wrapping_mul(state.inv_da) >> 8, dsc.extend)
            };
            result.color_map[i] = grad.color_map[w as usize];
            result.opa_map[i] = grad.opa_map[w as usize];
            dx = dx.wrapping_add(1);
            i += 1;
            width -= 1;
        }
    } else {
        while width > 0 {
            let mut d = atan2(dy, dx) as i32 - state.a;
            if d < 0 {
                d += 360;
            }
            let w = extend_w(d.wrapping_mul(state.inv_da) >> 8, dsc.extend);
            result.color_map[i] = grad.color_map[w as usize];
            result.opa_map[i] = grad.opa_map[w as usize];
            dx = dx.wrapping_add(1);
            i += 1;
            width -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::misc::color::OPA_COVER;

    const RED: Color = Color::make(255, 0, 0);
    const BLUE: Color = Color::make(0, 0, 255);

    fn red_blue() -> GradDsc {
        let mut g = GradDsc::new();
        g.init_stops(&[RED, BLUE], Some(&[OPA_COVER, 0]), None, 2);
        g
    }

    #[test]
    fn color_calculate_endpoints_and_midpoint() {
        let g = red_blue();
        assert_eq!(color_calculate(&g, 256, 0), (RED, OPA_COVER));
        assert_eq!(color_calculate(&g, 256, -5), (RED, OPA_COVER));
        // max = (255 * 256) >> 8 = 255, so frac >= 255 is the last stop.
        assert_eq!(color_calculate(&g, 256, 255), (BLUE, 0));
        assert_eq!(color_calculate(&g, 256, 256), (BLUE, 0));
        assert_eq!(color_calculate(&g, 256, 1000), (BLUE, 0));
        // frac = 128: d = 255, mix = 128, imix = 127.
        let (c, o) = color_calculate(&g, 256, 128);
        assert_eq!(c, Color::make(127, 0, 128));
        assert_eq!(o, 127);
    }

    #[test]
    fn color_calculate_scales_to_range() {
        let g = red_blue();
        // range 10: min = 0, max = (255 * 10) >> 8 = 9.
        assert_eq!(color_calculate(&g, 10, 0).0, RED);
        assert_eq!(color_calculate(&g, 10, 9).0, BLUE);
        // frac = 3: mix = 3 * 255 / 9 = 85, imix = 170
        let (c, o) = color_calculate(&g, 10, 3);
        assert_eq!(c, Color::make(170, 0, 85));
        assert_eq!(o, 170);
    }

    #[test]
    fn color_calculate_respects_stop_fracs() {
        let mut g = GradDsc::new();
        g.init_stops(&[RED, BLUE], None, Some(&[64, 192]), 2);
        // min = (64 * 256) >> 8 = 64, max = 192.
        assert_eq!(color_calculate(&g, 256, 63).0, RED);
        assert_eq!(color_calculate(&g, 256, 64).0, RED);
        assert_eq!(color_calculate(&g, 256, 192).0, BLUE);
        // frac = 128 -> (128 - 64) * 255 / 128 = 127
        let (c, o) = color_calculate(&g, 256, 128);
        assert_eq!(c, Color::make(128, 0, 127));
        assert_eq!(o, OPA_COVER);
    }

    #[test]
    fn get_horizontal_uses_width() {
        let mut g = red_blue();
        g.horizontal_init();
        let m = get(&g, 10, 20);
        assert_eq!(m.size, 10);
        assert_eq!(m.color_map.len(), 10);
        assert_eq!(m.opa_map.len(), 10);
        assert_eq!(m.color_map[0], RED);
        assert_eq!(m.opa_map[0], OPA_COVER);
        assert_eq!(m.color_map[9], BLUE);
        assert_eq!(m.opa_map[9], 0);
        for i in 0..10 {
            assert_eq!((m.color_map[i], m.opa_map[i]), color_calculate(&g, 10, i as i32));
        }
    }

    #[test]
    fn get_vertical_uses_height() {
        let mut g = red_blue();
        g.vertical_init();
        let m = get(&g, 10, 20);
        assert_eq!(m.size, 20);
        assert_eq!(m.color_map[0], RED);
        assert_eq!(m.color_map[19], BLUE);
        // Monotonic ramp from red to blue.
        for i in 1..20 {
            assert!(m.color_map[i].red <= m.color_map[i - 1].red);
            assert!(m.color_map[i].blue >= m.color_map[i - 1].blue);
            assert!(m.opa_map[i] <= m.opa_map[i - 1]);
        }
        cleanup(m);
    }

    #[test]
    fn get_none_is_empty() {
        let g = red_blue();
        let m = get(&g, 10, 20);
        assert_eq!(m.size, 0);
        assert!(m.color_map.is_empty());
        assert!(m.opa_map.is_empty());
    }

    #[test]
    fn get_256_hits_every_stop_exactly() {
        let mut g = red_blue();
        g.horizontal_init();
        let m = get(&g, 256, 0);
        assert_eq!(m.size, 256);
        assert_eq!(m.color_map[0], RED);
        assert_eq!(m.color_map[255], BLUE);
        assert_eq!(m.color_map[128], Color::make(127, 0, 128));
    }

    #[cfg(feature = "complex-gradients")]
    mod complex {
        use super::*;
        use ferrovgl_core::misc::types::coord::pct;

        fn result(width: usize) -> GradCalc {
            GradCalc {
                color_map: alloc::vec![Color::default(); width],
                opa_map: alloc::vec![0; width],
                size: width as u32,
            }
        }

        #[test]
        fn extend_modes() {
            assert_eq!(extend_w(-10, GradExtend::Pad), 0);
            assert_eq!(extend_w(100, GradExtend::Pad), 100);
            assert_eq!(extend_w(255, GradExtend::Pad), 255);
            assert_eq!(extend_w(300, GradExtend::Pad), 255);

            assert_eq!(extend_w(100, GradExtend::Repeat), 100);
            assert_eq!(extend_w(256, GradExtend::Repeat), 0);
            assert_eq!(extend_w(300, GradExtend::Repeat), 44);
            assert_eq!(extend_w(-1, GradExtend::Repeat), 255);

            assert_eq!(extend_w(100, GradExtend::Reflect), 100);
            assert_eq!(extend_w(255, GradExtend::Reflect), 255);
            assert_eq!(extend_w(256, GradExtend::Reflect), 255);
            assert_eq!(extend_w(300, GradExtend::Reflect), 211);
            assert_eq!(extend_w(511, GradExtend::Reflect), 0);
            assert_eq!(extend_w(512, GradExtend::Reflect), 0);
            assert_eq!(extend_w(-1, GradExtend::Reflect), 0);
        }

        #[test]
        fn linear_horizontal_pixels() {
            let mut g = red_blue();
            g.linear_init(0, 0, 255, 0, GradExtend::Pad);
            let coords = Area::from_size(0, 0, 256, 1);
            let st = linear_setup(&g, &coords);
            assert_eq!(st.a, 257);
            assert_eq!(st.b, 0);
            assert_eq!(st.c, 0);
            assert_eq!(st.cgrad.size, 256);

            let mut r = result(256);
            linear_get_line(&g, &st, 0, 0, 256, &mut r);
            assert_eq!(r.color_map[0], RED);
            assert_eq!(r.opa_map[0], OPA_COVER);
            assert_eq!(r.color_map[255], BLUE);
            assert_eq!(r.opa_map[255], 0);
            for x in 0..256usize {
                let w = (x as i32 * 257) >> 8;
                assert_eq!(r.color_map[x], st.cgrad.color_map[w as usize], "x = {x}");
                assert_eq!(r.opa_map[x], st.cgrad.opa_map[w as usize], "x = {x}");
            }

            // Past the end: pad clamps, repeat wraps.
            let mut r = result(4);
            linear_get_line(&g, &st, 300, 0, 4, &mut r);
            assert!(r.color_map.iter().all(|c| *c == BLUE));
            g.extend = GradExtend::Repeat;
            linear_get_line(&g, &st, 300, 0, 4, &mut r);
            for i in 0..4 {
                let w = (((300 + i as i32) * 257) >> 8) & 255;
                assert_eq!(r.color_map[i], st.cgrad.color_map[w as usize]);
            }
            linear_cleanup(st);
        }

        #[test]
        fn linear_percent_coordinates() {
            let mut g = red_blue();
            g.linear_init(pct(0), pct(0), pct(100), pct(0), GradExtend::Pad);
            let coords = Area::from_size(10, 10, 256, 100);
            let st = linear_setup(&g, &coords);
            // dx = 256 -> a = (256 << 16) / 65536 = 256: w == xp.
            assert_eq!(st.a, 256);
            assert_eq!(st.b, 0);
            let mut r = result(256);
            linear_get_line(&g, &st, 0, 50, 256, &mut r);
            assert_eq!(r.color_map, st.cgrad.color_map);
            assert_eq!(r.opa_map, st.cgrad.opa_map);
        }

        #[test]
        fn linear_vertical_pixels() {
            let mut g = red_blue();
            g.linear_init(0, 0, 0, 255, GradExtend::Pad);
            let coords = Area::from_size(0, 0, 1, 256);
            let st = linear_setup(&g, &coords);
            assert_eq!(st.a, 0);
            assert_eq!(st.b, 257);
            let mut r = result(1);
            for y in [0, 128, 255] {
                linear_get_line(&g, &st, 0, y, 1, &mut r);
                let w = ((y * 257) >> 8) as usize;
                assert_eq!(r.color_map[0], st.cgrad.color_map[w]);
            }
            linear_get_line(&g, &st, 0, 0, 1, &mut r);
            assert_eq!(r.color_map[0], RED);
            linear_get_line(&g, &st, 0, 255, 1, &mut r);
            assert_eq!(r.color_map[0], BLUE);
        }

        #[test]
        fn radial_concentric() {
            let mut g = red_blue();
            g.radial_init(50, 50, 100, 50, GradExtend::Pad);
            let coords = Area::from_size(0, 0, 100, 100);
            let st = radial_setup(&g, &coords);
            assert_eq!((st.x0, st.y0, st.r0), (50, 50, 0));
            assert_eq!(st.a4, 10000);
            assert_eq!((st.bpx, st.bpy, st.bc, st.dx), (0, 0, 0, 0));
            assert_eq!(st.inv_dr, (1 << 24) / 50);
            assert_eq!(st.inv_a4, (1 << 29) / 10000);
            // Pad + start circle inside end circle => clipped to the end circle's box.
            assert_eq!(st.clip_area, Area::new(0, 0, 100, 100));

            let mut r = result(100);
            radial_get_line(&g, &st, 0, 50, 100, &mut r);
            assert_eq!(r.color_map[50], RED);
            assert_eq!(r.opa_map[50], OPA_COVER);
            assert_eq!(r.color_map[0], BLUE);
            assert_eq!(r.opa_map[0], 0);
            // x = 25: distance 25 -> (25 * inv_dr) >> 16 = 127
            let w = ((25 * st.inv_dr) >> 16) as usize;
            assert_eq!(w, 127);
            assert_eq!(r.color_map[25], st.cgrad.color_map[w]);
            assert_eq!(r.color_map[75], st.cgrad.color_map[w]);

            // A line fully outside the clip box is filled with the end colour.
            let mut r = result(100);
            radial_get_line(&g, &st, 0, 200, 100, &mut r);
            assert!(r.color_map.iter().all(|c| *c == BLUE));
            assert!(r.opa_map.iter().all(|o| *o == 0));

            // A line crossing the clip box edge: outside part is end colour.
            let mut r = result(120);
            radial_get_line(&g, &st, -10, 50, 120, &mut r);
            assert!(r.color_map[..10].iter().all(|c| *c == BLUE));
            assert_eq!(r.color_map[60], RED);
            assert!(r.color_map[110..].iter().all(|c| *c == BLUE));
            radial_cleanup(st);
        }

        #[test]
        fn radial_repeat_has_no_clip() {
            let mut g = red_blue();
            g.radial_init(50, 50, 100, 50, GradExtend::Repeat);
            let coords = Area::from_size(0, 0, 100, 100);
            let st = radial_setup(&g, &coords);
            assert_eq!(st.clip_area.x1, RADIAL_NO_CLIP);
            let mut r = result(200);
            radial_get_line(&g, &st, 0, 50, 200, &mut r);
            // distance 100 wraps back to the start colour (w = 510 & 255 = 254).
            assert_eq!(r.color_map[50], RED);
            let w = (((100 - 0) * st.inv_dr) >> 16) & 255;
            assert_eq!(r.color_map[150], st.cgrad.color_map[w as usize]);
        }

        #[test]
        fn radial_focal_offset_general_case() {
            let mut g = red_blue();
            g.radial_init(50, 50, 100, 50, GradExtend::Pad);
            g.radial_set_focal(40, 50, 0);
            let coords = Area::from_size(0, 0, 100, 100);
            let st = radial_setup(&g, &coords);
            assert_eq!((st.x0, st.y0, st.r0), (40, 50, 0));
            assert_eq!(st.dx, 10);
            assert_eq!(st.a4, (2500 - 100) << 2);
            assert_eq!((st.bpx, st.bpy), (20, 0));
            assert_eq!(st.bc, (0 - 40 * 10) << 1);
            let mut r = result(100);
            radial_get_line(&g, &st, 0, 50, 100, &mut r);
            // The focal point is the start colour, the circle edge (x = 0 is
            // exactly 50 px from the end centre) the end colour; x = 99 is one
            // pixel inside the edge: w = 251.
            assert_eq!(r.color_map[40], RED);
            assert_eq!(r.color_map[0], BLUE);
            assert_eq!(r.color_map[99], st.cgrad.color_map[251]);
            // Hand-evaluated upstream fixed-point values (b >> 4, c >> 4, lv_sqrt32,
            // * inv_a4 >> 16). The >> 4 quantisation makes the ramp locally
            // non-monotonic near the focal point (19 then 18), as in C.
            for (x, w) in [(41usize, 19usize), (42, 18), (50, 45), (90, 214), (99, 251)] {
                assert_eq!(r.color_map[x], st.cgrad.color_map[w], "x = {x}");
                assert_eq!(r.opa_map[x], st.cgrad.opa_map[w], "x = {x}");
            }
        }

        #[test]
        fn conical_full_turn() {
            let mut g = red_blue();
            g.conical_init(50, 50, 0, 360, GradExtend::Pad);
            let coords = Area::from_size(0, 0, 100, 100);
            let st = conical_setup(&g, &coords);
            assert_eq!((st.x0, st.y0, st.a, st.da), (50, 50, 0, 360));
            assert_eq!(st.inv_da, 65536 / 360);

            // dy == 0 path: the centre itself and the +x axis are angle 0.
            let mut r = result(100);
            conical_get_line(&g, &st, 0, 50, 100, &mut r);
            assert_eq!(r.color_map[50], RED);
            assert_eq!(r.color_map[99], RED);
            // -x axis: 180 degrees.
            let w = ((180 * st.inv_da) >> 8) as usize;
            assert_eq!(r.color_map[0], st.cgrad.color_map[w]);

            // dy != 0 path: straight below the centre is 90 degrees.
            let mut r = result(1);
            conical_get_line(&g, &st, 50, 100, 1, &mut r);
            let w = ((90 * st.inv_da) >> 8) as usize;
            assert_eq!(r.color_map[0], st.cgrad.color_map[w]);
            conical_cleanup(st);
        }

        #[test]
        fn conical_start_angle_wraps() {
            let mut g = red_blue();
            g.conical_init(50, 50, 90, 450, GradExtend::Pad);
            let coords = Area::from_size(0, 0, 100, 100);
            let st = conical_setup(&g, &coords);
            // 450 % 360 = 90 <= 90 -> beta = 450, da = 360.
            assert_eq!((st.a, st.da), (90, 360));
            let mut r = result(1);
            // Straight below: 90 - 90 = 0 -> start colour.
            conical_get_line(&g, &st, 50, 100, 1, &mut r);
            assert_eq!(r.color_map[0], RED);
            // +x axis: 0 - 90 < 0 -> 270.
            conical_get_line(&g, &st, 100, 50, 1, &mut r);
            let w = ((270 * st.inv_da) >> 8) as usize;
            assert_eq!(r.color_map[0], st.cgrad.color_map[w]);
        }
    }
}
