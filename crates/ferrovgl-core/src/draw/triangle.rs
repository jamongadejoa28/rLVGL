//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_triangle.h`, `src/draw/lv_draw_triangle.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::draw::grad::GradDsc;
use crate::draw::{DrawCtx, DrawDscBase, DrawTaskRef, LayerId};
use crate::misc::area::PointPrecise;
use crate::misc::color::{Color, Opa};

/// `lv_draw_triangle_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct TriangleDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `p`
    pub p: [PointPrecise; 3],
    /// `color`
    pub color: Color,
    /// `opa`
    pub opa: Opa,
    /// `grad`
    pub grad: GradDsc,
}

impl TriangleDsc {
    /// `lv_draw_triangle_dsc_init()`
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for TriangleDsc {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawCtx {
    /// `lv_draw_triangle()`
    pub fn draw_triangle(&mut self, layer: LayerId, dsc: &TriangleDsc) {
        todo!()
    }

    /// `lv_draw_task_get_triangle_dsc()`
    pub fn task_get_triangle_dsc(&mut self, task: DrawTaskRef) -> Option<&mut TriangleDsc> {
        todo!()
    }
}
