//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_blur.h`, `src/draw/lv_draw_blur.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::draw::{DrawCtx, DrawDscBase, DrawTaskRef, LayerId};
use crate::misc::area::Area;
use crate::misc::style::BlurQuality;

/// `lv_draw_blur_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct BlurDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `blur_radius`
    pub blur_radius: i32,
    /// `corner_radius`
    pub corner_radius: i32,
    /// `quality`
    pub quality: BlurQuality,
}

impl BlurDsc {
    /// `lv_draw_blur_dsc_init()`
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for BlurDsc {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawCtx {
    /// `lv_draw_blur()`
    pub fn draw_blur(&mut self, layer: LayerId, dsc: &BlurDsc, coords: &Area) {
        todo!()
    }

    /// `lv_draw_task_get_blur_dsc()`
    pub fn task_get_blur_dsc(&mut self, task: DrawTaskRef) -> Option<&mut BlurDsc> {
        todo!()
    }
}
