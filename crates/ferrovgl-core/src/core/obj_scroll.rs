//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_obj_scroll.h`, `src/core/lv_obj_scroll_private.h`,
//! `src/core/lv_obj_scroll.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::core::Ui;
use crate::core::obj::{Obj, ScrollSnap, ScrollbarMode};
use crate::misc::anim::AnimEnable;
use crate::misc::area::{Area, Dir, Point};
use crate::misc::event::LvResult;

impl Ui {
    /// `lv_obj_set_scrollbar_mode()`
    pub fn obj_set_scrollbar_mode(&mut self, obj: Obj, mode: ScrollbarMode) {
        todo!()
    }

    /// `lv_obj_set_scroll_dir()`
    pub fn obj_set_scroll_dir(&mut self, obj: Obj, dir: Dir) {
        todo!()
    }

    /// `lv_obj_set_scroll_snap_x()`
    pub fn obj_set_scroll_snap_x(&mut self, obj: Obj, align: ScrollSnap) {
        todo!()
    }

    /// `lv_obj_set_scroll_snap_y()`
    pub fn obj_set_scroll_snap_y(&mut self, obj: Obj, align: ScrollSnap) {
        todo!()
    }

    /// `lv_obj_get_scrollbar_mode()`
    pub fn obj_get_scrollbar_mode(&self, obj: Obj) -> ScrollbarMode {
        todo!()
    }

    /// `lv_obj_get_scroll_dir()`
    pub fn obj_get_scroll_dir(&self, obj: Obj) -> Dir {
        todo!()
    }

    /// `lv_obj_get_scroll_snap_x()`
    pub fn obj_get_scroll_snap_x(&self, obj: Obj) -> ScrollSnap {
        todo!()
    }

    /// `lv_obj_get_scroll_snap_y()`
    pub fn obj_get_scroll_snap_y(&self, obj: Obj) -> ScrollSnap {
        todo!()
    }

    /// `lv_obj_get_scroll_x()`
    pub fn obj_get_scroll_x(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_scroll_y()`
    pub fn obj_get_scroll_y(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_scroll_top()`
    pub fn obj_get_scroll_top(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_scroll_bottom()`
    pub fn obj_get_scroll_bottom(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_scroll_left()`
    pub fn obj_get_scroll_left(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_scroll_right()`
    pub fn obj_get_scroll_right(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_scroll_end()`
    pub fn obj_get_scroll_end(&self, obj: Obj) -> Point {
        todo!()
    }

    /// `lv_obj_scroll_by()`
    pub fn obj_scroll_by(&mut self, obj: Obj, dx: i32, dy: i32, anim_en: AnimEnable) {
        todo!()
    }

    /// `lv_obj_scroll_by_bounded()`
    pub fn obj_scroll_by_bounded(&mut self, obj: Obj, dx: i32, dy: i32, anim_en: AnimEnable) {
        todo!()
    }

    /// `lv_obj_scroll_to()`
    pub fn obj_scroll_to(&mut self, obj: Obj, x: i32, y: i32, anim_en: AnimEnable) {
        todo!()
    }

    /// `lv_obj_scroll_to_x()`
    pub fn obj_scroll_to_x(&mut self, obj: Obj, x: i32, anim_en: AnimEnable) {
        todo!()
    }

    /// `lv_obj_scroll_to_y()`
    pub fn obj_scroll_to_y(&mut self, obj: Obj, y: i32, anim_en: AnimEnable) {
        todo!()
    }

    /// `lv_obj_scroll_to_view()`
    pub fn obj_scroll_to_view(&mut self, obj: Obj, anim_en: AnimEnable) {
        todo!()
    }

    /// `lv_obj_scroll_to_view_recursive()`
    pub fn obj_scroll_to_view_recursive(&mut self, obj: Obj, anim_en: AnimEnable) {
        todo!()
    }

    /// `lv_obj_scroll_by_raw()`
    pub fn obj_scroll_by_raw(&mut self, obj: Obj, x: i32, y: i32) -> LvResult {
        todo!()
    }

    /// `lv_obj_is_scrolling()`
    pub fn obj_is_scrolling(&self, obj: Obj) -> bool {
        todo!()
    }

    /// `lv_obj_stop_scroll_anim()`
    pub fn obj_stop_scroll_anim(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_update_snap()`
    pub fn obj_update_snap(&mut self, obj: Obj, anim_en: AnimEnable) {
        todo!()
    }

    /// `lv_obj_get_scrollbar_area()`: returns `(hor_area, ver_area)`.
    pub fn obj_get_scrollbar_area(&self, obj: Obj) -> (Area, Area) {
        todo!()
    }

    /// `lv_obj_scrollbar_invalidate()`
    pub fn obj_scrollbar_invalidate(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_readjust_scroll()`
    pub fn obj_readjust_scroll(&mut self, obj: Obj, anim_en: AnimEnable) {
        todo!()
    }
}
