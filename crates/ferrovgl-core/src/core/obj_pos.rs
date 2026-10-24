//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_obj_pos.h`, `src/core/lv_obj_pos.c`.
//!
//! Position, size, layout updates, alignment, transforms and invalidation.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::misc::area::{Align, Area, Point};
use crate::misc::event::LvResult;
use crate::misc::style::StyleProp;

/// `lv_obj_point_transform_flag_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ObjPointTransformFlag {
    /// `LV_OBJ_POINT_TRANSFORM_FLAG_NONE`
    #[default]
    None = 0x00,
    /// `LV_OBJ_POINT_TRANSFORM_FLAG_RECURSIVE`
    Recursive = 0x01,
    /// `LV_OBJ_POINT_TRANSFORM_FLAG_INVERSE`
    Inverse = 0x02,
    /// `LV_OBJ_POINT_TRANSFORM_FLAG_INVERSE_RECURSIVE`
    InverseRecursive = 0x03,
}

impl ObjPointTransformFlag {
    /// `flags & RECURSIVE`
    #[inline]
    pub const fn recursive(self) -> bool {
        (self as u8) & 0x01 != 0
    }
    /// `flags & INVERSE`
    #[inline]
    pub const fn inverse(self) -> bool {
        (self as u8) & 0x02 != 0
    }
}

/// `lv_clamp_width()`
pub fn clamp_width(width: i32, min_width: i32, max_width: i32, ref_width: i32) -> i32 {
    todo!()
}

/// `lv_clamp_height()`
pub fn clamp_height(height: i32, min_height: i32, max_height: i32, ref_height: i32) -> i32 {
    todo!()
}

impl Ui {
    /// `lv_obj_set_pos()`
    pub fn obj_set_pos(&mut self, obj: Obj, x: i32, y: i32) {
        todo!()
    }

    /// `lv_obj_set_x()`
    pub fn obj_set_x(&mut self, obj: Obj, x: i32) {
        todo!()
    }

    /// `lv_obj_set_y()`
    pub fn obj_set_y(&mut self, obj: Obj, y: i32) {
        todo!()
    }

    /// `lv_obj_set_size()`
    pub fn obj_set_size(&mut self, obj: Obj, w: i32, h: i32) {
        todo!()
    }

    /// `lv_obj_refr_size()`: returns `true` if the size changed.
    pub fn obj_refr_size(&mut self, obj: Obj) -> bool {
        todo!()
    }

    /// `lv_obj_set_width()`
    pub fn obj_set_width(&mut self, obj: Obj, w: i32) {
        todo!()
    }

    /// `lv_obj_set_height()`
    pub fn obj_set_height(&mut self, obj: Obj, h: i32) {
        todo!()
    }

    /// `lv_obj_set_content_width()`
    pub fn obj_set_content_width(&mut self, obj: Obj, w: i32) {
        todo!()
    }

    /// `lv_obj_set_content_height()`
    pub fn obj_set_content_height(&mut self, obj: Obj, h: i32) {
        todo!()
    }

    /// `lv_obj_set_layout()`
    pub fn obj_set_layout(&mut self, obj: Obj, layout: u32) {
        todo!()
    }

    /// `lv_obj_is_layout_positioned()`
    pub fn obj_is_layout_positioned(&self, obj: Obj) -> bool {
        todo!()
    }

    /// `lv_obj_mark_layout_as_dirty()`
    pub fn obj_mark_layout_as_dirty(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_update_layout()`
    pub fn obj_update_layout(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_set_align()`
    pub fn obj_set_align(&mut self, obj: Obj, align: Align) {
        todo!()
    }

    /// `lv_obj_align()`
    pub fn obj_align(&mut self, obj: Obj, align: Align, x_ofs: i32, y_ofs: i32) {
        todo!()
    }

    /// `lv_obj_align_to()`
    pub fn obj_align_to(&mut self, obj: Obj, base: Obj, align: Align, x_ofs: i32, y_ofs: i32) {
        todo!()
    }

    /// `lv_obj_center()`
    pub fn obj_center(&mut self, obj: Obj) {
        self.obj_align(obj, Align::Center, 0, 0);
    }

    /// `lv_obj_get_coords()`
    pub fn obj_get_coords(&self, obj: Obj) -> Area {
        todo!()
    }

    /// `lv_obj_get_x()`
    pub fn obj_get_x(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_x2()`
    pub fn obj_get_x2(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_y()`
    pub fn obj_get_y(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_y2()`
    pub fn obj_get_y2(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_x_aligned()`
    pub fn obj_get_x_aligned(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_y_aligned()`
    pub fn obj_get_y_aligned(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_width()`
    pub fn obj_get_width(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_height()`
    pub fn obj_get_height(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_content_width()`
    pub fn obj_get_content_width(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_content_height()`
    pub fn obj_get_content_height(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_content_coords()`
    pub fn obj_get_content_coords(&self, obj: Obj) -> Area {
        todo!()
    }

    /// `lv_obj_get_self_width()`
    pub fn obj_get_self_width(&mut self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_self_height()`
    pub fn obj_get_self_height(&mut self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_style_clamped_width()`
    pub fn obj_get_style_clamped_width(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_style_clamped_height()`
    pub fn obj_get_style_clamped_height(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_is_style_any_width_content()`
    pub fn obj_is_style_any_width_content(&self, obj: Obj) -> bool {
        todo!()
    }

    /// `lv_obj_is_style_any_height_content()`
    pub fn obj_is_style_any_height_content(&self, obj: Obj) -> bool {
        todo!()
    }

    /// `lv_obj_is_width_min()`
    pub fn obj_is_width_min(&self, obj: Obj) -> bool {
        todo!()
    }

    /// `lv_obj_is_height_min()`
    pub fn obj_is_height_min(&self, obj: Obj) -> bool {
        todo!()
    }

    /// `lv_obj_is_width_max()`
    pub fn obj_is_width_max(&self, obj: Obj) -> bool {
        todo!()
    }

    /// `lv_obj_is_height_max()`
    pub fn obj_is_height_max(&self, obj: Obj) -> bool {
        todo!()
    }

    /// `lv_obj_refresh_self_size()`
    pub fn obj_refresh_self_size(&mut self, obj: Obj) -> bool {
        todo!()
    }

    /// `lv_obj_refr_pos()`
    pub fn obj_refr_pos(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_move_to()`
    pub fn obj_move_to(&mut self, obj: Obj, x: i32, y: i32) {
        todo!()
    }

    /// `lv_obj_move_children_by()`
    pub fn obj_move_children_by(&mut self, obj: Obj, x_diff: i32, y_diff: i32, ignore_floating: bool) {
        todo!()
    }

    /// `lv_obj_transform_point()`
    pub fn obj_transform_point(&self, obj: Obj, p: &mut Point, flags: ObjPointTransformFlag) {
        todo!()
    }

    /// `lv_obj_transform_point_array()`
    pub fn obj_transform_point_array(&self, obj: Obj, points: &mut [Point], flags: ObjPointTransformFlag) {
        todo!()
    }

    /// `lv_obj_get_transformed_area()`
    pub fn obj_get_transformed_area(&self, obj: Obj, area: &mut Area, flags: ObjPointTransformFlag) {
        todo!()
    }

    /// `lv_obj_invalidate_area()`
    pub fn obj_invalidate_area(&mut self, obj: Obj, area: &Area) -> LvResult {
        todo!()
    }

    /// `lv_obj_invalidate()`
    pub fn obj_invalidate(&mut self, obj: Obj) -> LvResult {
        todo!()
    }

    /// `lv_obj_area_is_visible()`: clips `area` to the visible part.
    pub fn obj_area_is_visible(&self, obj: Obj, area: &mut Area) -> bool {
        todo!()
    }

    /// `lv_obj_is_visible()`
    pub fn obj_is_visible(&self, obj: Obj) -> bool {
        todo!()
    }

    /// `lv_obj_set_ext_click_area()`
    pub fn obj_set_ext_click_area(&mut self, obj: Obj, size: i32) {
        todo!()
    }

    /// `lv_obj_get_click_area()`
    pub fn obj_get_click_area(&self, obj: Obj) -> Area {
        todo!()
    }

    /// `lv_obj_hit_test()`
    pub fn obj_hit_test(&mut self, obj: Obj, point: Point) -> bool {
        todo!()
    }

    /// `lv_obj_calc_dynamic_width()`
    pub fn obj_calc_dynamic_width(&self, obj: Obj, prop: StyleProp) -> i32 {
        todo!()
    }

    /// `lv_obj_calc_dynamic_height()`
    pub fn obj_calc_dynamic_height(&self, obj: Obj, prop: StyleProp) -> i32 {
        todo!()
    }
}
