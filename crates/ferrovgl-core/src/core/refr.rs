//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_refr.h`, `src/core/lv_refr_private.h`,
//! `src/core/lv_refr.c`.
//!
//! Invalidation and the refresh pipeline: join areas, render each invalid
//! area (chunked for partial buffers), draw objects into layers, dispatch
//! and flush.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::display::DisplayId;
use crate::draw::LayerId;
use crate::misc::area::Area;
use crate::misc::event::LvResult;
use crate::misc::timer::TimerId;

impl Ui {
    /// `lv_refr_now()`: refresh `disp` (or every display) immediately.
    pub fn refr_now(&mut self, disp: Option<DisplayId>) {
        todo!()
    }

    /// `lv_obj_redraw()`: draw one object (without children) into `layer`.
    pub fn obj_redraw(&mut self, layer: LayerId, obj: Obj) {
        todo!()
    }

    /// `lv_display_refr_timer()`: the periodic refresh.
    pub fn display_refr_timer(&mut self, timer: Option<TimerId>) {
        todo!()
    }

    /// `lv_inv_area()`: invalidate `area` (or everything) on `disp`.
    pub fn inv_area(&mut self, disp: Option<DisplayId>, area: Option<&Area>) -> LvResult {
        todo!()
    }

    /// `lv_refr_get_disp_refreshing()`
    pub fn refr_get_disp_refreshing(&self) -> Option<DisplayId> {
        self.disp_refresh
    }

    /// `lv_refr_set_disp_refreshing()`
    pub fn refr_set_disp_refreshing(&mut self, disp: Option<DisplayId>) {
        self.disp_refresh = disp;
    }

    /// `lv_refr_get_top_obj()`: the topmost object fully covering `area`.
    pub fn refr_get_top_obj(&mut self, area: &Area, obj: Obj) -> Option<Obj> {
        todo!()
    }

    /// `lv_obj_refr()`: draw `obj` and its children into `layer`, creating
    /// sub-layers for opacity / transforms.
    pub fn obj_refr(&mut self, layer: LayerId, obj: Obj) {
        todo!()
    }

    /// `lv_refr_join_areas()`
    pub fn refr_join_areas(&mut self, disp: DisplayId) {
        todo!()
    }

    /// `refr_invalid_areas()`
    pub fn refr_invalid_areas(&mut self, disp: DisplayId) {
        todo!()
    }

    /// `refr_area()`
    pub fn refr_area(&mut self, disp: DisplayId, area: &Area, y_offset: i32) {
        todo!()
    }

    /// `refr_area_part()`
    pub fn refr_area_part(&mut self, disp: DisplayId, layer: LayerId) {
        todo!()
    }

    /// `refr_obj_and_children()`
    pub fn refr_obj_and_children(&mut self, layer: LayerId, top_obj: Option<Obj>) {
        todo!()
    }

    /// `lv_obj_invalidate_expand_blur()`
    pub fn obj_invalidate_expand_blur(&mut self, disp: DisplayId) {
        todo!()
    }
}
