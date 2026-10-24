//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_grad.c`, `src/draw/sw/lv_draw_sw_grad.h`.
//!
//! Gradient colour maps (`lv_draw_sw_grad_calc_t`) and the per-draw state of
//! linear / radial / conical gradients (`complex-gradients` feature).

use alloc::vec::Vec;

use ferrovgl_core::draw::grad::GradDsc;
use ferrovgl_core::misc::area::Area;
use ferrovgl_core::misc::color::{Color, Opa};

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

/// `lv_draw_sw_grad_color_calculate()`: colour and opacity at `frac` of `range`.
pub fn color_calculate(dsc: &GradDsc, range: i32, frac: i32) -> (Color, Opa) {
    todo!()
}

/// `lv_draw_sw_grad_get()`: the colour map of `gradient` for a `w` x `h` area.
pub fn get(gradient: &GradDsc, w: i32, h: i32) -> GradCalc {
    todo!()
}

/// `lv_draw_sw_grad_cleanup()`
pub fn cleanup(grad: GradCalc) {
    drop(grad);
}

/// `lv_draw_sw_grad_linear_setup()`
#[cfg(feature = "complex-gradients")]
pub fn linear_setup(dsc: &GradDsc, coords: &Area) -> GradLinearState {
    todo!()
}

/// `lv_draw_sw_grad_linear_cleanup()`: the state owns its colour map, so
/// this only drops it.
#[cfg(feature = "complex-gradients")]
pub fn linear_cleanup(state: GradLinearState) {
    drop(state);
}

/// `lv_draw_sw_grad_linear_get_line()`
#[cfg(feature = "complex-gradients")]
pub fn linear_get_line(
    dsc: &GradDsc,
    state: &GradLinearState,
    xp: i32,
    yp: i32,
    width: i32,
    result: &mut GradCalc,
) {
    todo!()
}

/// `lv_draw_sw_grad_radial_setup()`
#[cfg(feature = "complex-gradients")]
pub fn radial_setup(dsc: &GradDsc, coords: &Area) -> GradRadialState {
    todo!()
}

/// `lv_draw_sw_grad_radial_cleanup()`: the state owns its colour map, so
/// this only drops it.
#[cfg(feature = "complex-gradients")]
pub fn radial_cleanup(state: GradRadialState) {
    drop(state);
}

/// `lv_draw_sw_grad_radial_get_line()`
#[cfg(feature = "complex-gradients")]
pub fn radial_get_line(
    dsc: &GradDsc,
    state: &GradRadialState,
    xp: i32,
    yp: i32,
    width: i32,
    result: &mut GradCalc,
) {
    todo!()
}

/// `lv_draw_sw_grad_conical_setup()`
#[cfg(feature = "complex-gradients")]
pub fn conical_setup(dsc: &GradDsc, coords: &Area) -> GradConicalState {
    todo!()
}

/// `lv_draw_sw_grad_conical_cleanup()`: the state owns its colour map, so
/// this only drops it.
#[cfg(feature = "complex-gradients")]
pub fn conical_cleanup(state: GradConicalState) {
    drop(state);
}

/// `lv_draw_sw_grad_conical_get_line()`
#[cfg(feature = "complex-gradients")]
pub fn conical_get_line(
    dsc: &GradDsc,
    state: &GradConicalState,
    xp: i32,
    yp: i32,
    width: i32,
    result: &mut GradCalc,
) {
    todo!()
}
