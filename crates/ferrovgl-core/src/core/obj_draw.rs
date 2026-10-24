//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_obj_draw.h`, `src/core/lv_obj_draw_private.h`,
//! `src/core/lv_obj_draw.c`.
//!
//! Initialise draw descriptors from an object's styles and manage the
//! extended draw size.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::core::Ui;
use crate::core::obj::{LayerType, Obj};
use crate::draw::arc::ArcDsc;
use crate::draw::blur::BlurDsc;
use crate::draw::image::DrawImageDsc;
use crate::draw::label::LabelDsc;
use crate::draw::line::LineDsc;
use crate::draw::rect::RectDsc;
use crate::misc::style::Part;

impl Ui {
    /// `lv_obj_init_draw_rect_dsc()`
    pub fn obj_init_draw_rect_dsc(&self, obj: Obj, part: Part, draw_dsc: &mut RectDsc) {
        todo!()
    }

    /// `lv_obj_init_draw_label_dsc()`
    pub fn obj_init_draw_label_dsc(&self, obj: Obj, part: Part, draw_dsc: &mut LabelDsc) {
        todo!()
    }

    /// `lv_obj_init_draw_image_dsc()`
    pub fn obj_init_draw_image_dsc(&self, obj: Obj, part: Part, draw_dsc: &mut DrawImageDsc) {
        todo!()
    }

    /// `lv_obj_init_draw_line_dsc()`
    pub fn obj_init_draw_line_dsc(&self, obj: Obj, part: Part, draw_dsc: &mut LineDsc) {
        todo!()
    }

    /// `lv_obj_init_draw_arc_dsc()`
    pub fn obj_init_draw_arc_dsc(&self, obj: Obj, part: Part, draw_dsc: &mut ArcDsc) {
        todo!()
    }

    /// `lv_obj_init_draw_blur_dsc()`
    pub fn obj_init_draw_blur_dsc(&self, obj: Obj, part: Part, draw_dsc: &mut BlurDsc) {
        todo!()
    }

    /// `lv_obj_set_ext_draw_size()`
    pub fn obj_set_ext_draw_size(&mut self, obj: Obj, size: i32) {
        todo!()
    }

    /// `lv_obj_calculate_ext_draw_size()`
    pub fn obj_calculate_ext_draw_size(&self, obj: Obj, part: Part) -> i32 {
        todo!()
    }

    /// `lv_obj_refresh_ext_draw_size()`
    pub fn obj_refresh_ext_draw_size(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_get_ext_draw_size()`
    pub fn obj_get_ext_draw_size(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_layer_type()`
    pub fn obj_get_layer_type(&self, obj: Obj) -> LayerType {
        todo!()
    }
}
