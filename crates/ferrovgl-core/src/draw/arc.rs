//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_arc.h`, `src/draw/lv_draw_arc.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::draw::{DrawCtx, DrawDscBase, DrawTaskRef, LayerId};
use crate::misc::area::{Area, Point};
use crate::misc::color::{Color, Opa};
use crate::misc::style::ImageSrc;
use crate::misc::types::ValuePrecise;

/// `lv_draw_arc_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct ArcDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `color`
    pub color: Color,
    /// `width`
    pub width: i32,
    /// `start_angle`
    pub start_angle: ValuePrecise,
    /// `end_angle`
    pub end_angle: ValuePrecise,
    /// `center`
    pub center: Point,
    /// `img_src`
    pub img_src: ImageSrc,
    /// `radius`
    pub radius: u16,
    /// `opa`
    pub opa: Opa,
    /// `rounded`
    pub rounded: bool,
}

impl ArcDsc {
    /// `lv_draw_arc_dsc_init()`
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for ArcDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_arc_get_area()`: bounding area of an arc.
pub fn arc_get_area(
    x: i32,
    y: i32,
    radius: u16,
    start_angle: ValuePrecise,
    end_angle: ValuePrecise,
    w: i32,
    rounded: bool,
    area: &mut Area,
) {
    todo!()
}

impl DrawCtx {
    /// `lv_draw_arc()`
    pub fn draw_arc(&mut self, layer: LayerId, dsc: &ArcDsc) {
        todo!()
    }

    /// `lv_draw_task_get_arc_dsc()`
    pub fn task_get_arc_dsc(&mut self, task: DrawTaskRef) -> Option<&mut ArcDsc> {
        todo!()
    }
}
