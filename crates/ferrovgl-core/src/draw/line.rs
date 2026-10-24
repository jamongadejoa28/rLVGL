//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_line.h`, `src/draw/lv_draw_line.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::rc::Rc;

use crate::draw::{DrawCtx, DrawDscBase, DrawTaskRef, LayerId};
use crate::misc::area::PointPrecise;
use crate::misc::color::{Color, Opa};
use crate::misc::types::ValuePrecise;

/// `LV_DRAW_LINE_POINT_NONE`
#[cfg(feature = "float")]
pub const DRAW_LINE_POINT_NONE: ValuePrecise = f32::MAX;
/// `LV_DRAW_LINE_POINT_NONE`
#[cfg(not(feature = "float"))]
pub const DRAW_LINE_POINT_NONE: ValuePrecise = i32::MAX;

/// `lv_draw_line_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct LineDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `p1`
    pub p1: PointPrecise,
    /// `p2`
    pub p2: PointPrecise,
    /// `points` / `point_cnt`: a polyline (used by `lv_draw_line_iterate`).
    pub points: Option<Rc<[PointPrecise]>>,
    /// `color`
    pub color: Color,
    /// `width`
    pub width: i32,
    /// `dash_width`
    pub dash_width: i32,
    /// `dash_gap`
    pub dash_gap: i32,
    /// `opa`
    pub opa: Opa,
    /// `round_start`
    pub round_start: bool,
    /// `round_end`
    pub round_end: bool,
    /// `raw_end`: do not bother with perpendicular line ending if it's not visible.
    pub raw_end: bool,
}

impl LineDsc {
    /// `lv_draw_line_dsc_init()`
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for LineDsc {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawCtx {
    /// `lv_draw_line()`
    pub fn draw_line(&mut self, layer: LayerId, dsc: &LineDsc) {
        todo!()
    }

    /// `lv_draw_task_get_line_dsc()`
    pub fn task_get_line_dsc(&mut self, task: DrawTaskRef) -> Option<&mut LineDsc> {
        todo!()
    }

    /// `lv_draw_line_iterate()`: call `draw_line_cb` for every segment of `dsc.points`.
    pub fn draw_line_iterate(
        &mut self,
        t: DrawTaskRef,
        dsc: &mut LineDsc,
        draw_line_cb: &mut dyn FnMut(&mut DrawCtx, DrawTaskRef, &LineDsc),
    ) {
        todo!()
    }
}
