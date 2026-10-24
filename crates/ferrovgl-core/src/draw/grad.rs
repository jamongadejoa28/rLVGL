//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_grad.h`, `src/draw/lv_grad.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::misc::area::Point;
use crate::misc::color::{Color, Opa};

/// `LV_GRADIENT_MAX_STOPS` (lv_conf default; the upstream tests use 2 too).
pub const GRADIENT_MAX_STOPS: usize = 2;

/// `lv_grad_dir_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GradDir {
    /// `LV_GRAD_DIR_NONE`: no gradient (the `grad_color` property is ignored).
    #[default]
    None = 0,
    /// `LV_GRAD_DIR_VER`: simple vertical (top to bottom) gradient.
    Ver = 1,
    /// `LV_GRAD_DIR_HOR`: simple horizontal (left to right) gradient.
    Hor = 2,
    /// `LV_GRAD_DIR_LINEAR`: linear gradient defined by start and end points.
    Linear = 3,
    /// `LV_GRAD_DIR_RADIAL`: radial gradient defined by start and end circles.
    Radial = 4,
    /// `LV_GRAD_DIR_CONICAL`: conical gradient defined by center point, start and end angles.
    Conical = 5,
}

impl GradDir {
    /// Convert a raw style number back to a direction (`None` if unknown).
    pub const fn from_u8(v: u8) -> GradDir {
        match v {
            1 => GradDir::Ver,
            2 => GradDir::Hor,
            3 => GradDir::Linear,
            4 => GradDir::Radial,
            5 => GradDir::Conical,
            _ => GradDir::None,
        }
    }
}

/// `lv_grad_extend_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GradExtend {
    /// `LV_GRAD_EXTEND_PAD`: repeat the same color.
    #[default]
    Pad = 0,
    /// `LV_GRAD_EXTEND_REPEAT`: repeat the pattern.
    Repeat = 1,
    /// `LV_GRAD_EXTEND_REFLECT`: repeat the pattern mirrored.
    Reflect = 2,
}

/// `lv_grad_stop_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct GradStop {
    /// `color`
    pub color: Color,
    /// `opa`
    pub opa: Opa,
    /// `frac`: location in `0..=255`.
    pub frac: u8,
}

/// `params` union of `lv_grad_dsc_t`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GradParams {
    /// No parameters (simple `Ver` / `Hor`).
    None,
    /// `params.linear`
    Linear {
        /// `start`
        start: Point,
        /// `end`
        end: Point,
    },
    /// `params.radial`
    Radial {
        /// `focal`: center of the focal (starting) circle.
        focal: Point,
        /// `focal_extent`: a point on the focal circle.
        focal_extent: Point,
        /// `end`: center of the end circle.
        end: Point,
        /// `end_extent`: a point on the end circle.
        end_extent: Point,
    },
    /// `params.conical`
    Conical {
        /// `center`
        center: Point,
        /// `start_angle`
        start_angle: i16,
        /// `end_angle`
        end_angle: i16,
    },
}

/// `lv_grad_dsc_t`. The renderer-private `state` pointer is not stored here;
/// the software renderer computes it per draw call.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GradDsc {
    /// `stops`
    pub stops: [GradStop; GRADIENT_MAX_STOPS],
    /// `stops_count`
    pub stops_count: u8,
    /// `dir`
    pub dir: GradDir,
    /// `extend`
    pub extend: GradExtend,
    /// `params`
    pub params: GradParams,
}

impl GradDsc {
    /// A zeroed descriptor (`lv_memzero`).
    pub const fn new() -> Self {
        Self {
            stops: [GradStop {
                color: Color::BLACK,
                opa: 0,
                frac: 0,
            }; GRADIENT_MAX_STOPS],
            stops_count: 0,
            dir: GradDir::None,
            extend: GradExtend::Pad,
            params: GradParams::None,
        }
    }

    /// `lv_grad_init_stops()`: `opa` / `fracs` may be `None` (cover / evenly spaced).
    pub fn init_stops(
        &mut self,
        colors: &[Color],
        opa: Option<&[Opa]>,
        fracs: Option<&[u8]>,
        num_stops: usize,
    ) {
        todo!()
    }

    /// `lv_grad_horizontal_init()`
    pub fn horizontal_init(&mut self) {
        todo!()
    }

    /// `lv_grad_vertical_init()`
    pub fn vertical_init(&mut self) {
        todo!()
    }

    /// `lv_grad_linear_init()`
    pub fn linear_init(&mut self, from_x: i32, from_y: i32, to_x: i32, to_y: i32, extend: GradExtend) {
        todo!()
    }

    /// `lv_grad_radial_init()`
    pub fn radial_init(&mut self, center_x: i32, center_y: i32, to_x: i32, to_y: i32, extend: GradExtend) {
        todo!()
    }

    /// `lv_grad_radial_set_focal()`
    pub fn radial_set_focal(&mut self, center_x: i32, center_y: i32, radius: i32) {
        todo!()
    }

    /// `lv_grad_conical_init()`
    pub fn conical_init(
        &mut self,
        center_x: i32,
        center_y: i32,
        start_angle: i32,
        end_angle: i32,
        extend: GradExtend,
    ) {
        todo!()
    }
}

impl Default for GradDsc {
    fn default() -> Self {
        Self::new()
    }
}
