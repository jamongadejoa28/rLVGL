//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_obj_tree.h`, `src/core/lv_obj_tree.c`.
//!
//! Parent/child relations, deletion and tree walking.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::rc::Rc;

use crate::core::Ui;
use crate::core::obj::{Obj, ObjClass, ObjTreeWalkRes};
use crate::display::DisplayId;
use crate::misc::anim::AnimId;
use crate::misc::event::LvResult;

/// `lv_obj_tree_walk_cb_t`
pub type ObjTreeWalkCallback<'a> = &'a mut dyn FnMut(&mut Ui, Obj) -> ObjTreeWalkRes;

impl Ui {
    /// `lv_obj_delete()`: send `LV_EVENT_DELETE`, delete the children
    /// (deepest first), run the destructors and free the slot.
    pub fn obj_delete(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_clean()`: delete all children.
    pub fn obj_clean(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_delete_delayed()`
    pub fn obj_delete_delayed(&mut self, obj: Obj, delay_ms: u32) {
        todo!()
    }

    /// `lv_obj_delete_anim_completed_cb()`
    pub fn obj_delete_anim_completed_cb(&mut self, a: AnimId) {
        todo!()
    }

    /// `lv_obj_delete_async()`: delete from the next `timer_handler()`.
    pub fn obj_delete_async(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_set_parent()`
    pub fn obj_set_parent(&mut self, obj: Obj, parent: Obj) {
        todo!()
    }

    /// `lv_obj_swap()`
    pub fn obj_swap(&mut self, obj1: Obj, obj2: Obj) {
        todo!()
    }

    /// `lv_obj_move_to_index()`: negative indexes count from the end.
    pub fn obj_move_to_index(&mut self, obj: Obj, index: i32) {
        todo!()
    }

    /// `lv_obj_get_screen()`
    pub fn obj_get_screen(&self, obj: Obj) -> Obj {
        todo!()
    }

    /// `lv_obj_get_display()`
    pub fn obj_get_display(&self, obj: Obj) -> Option<DisplayId> {
        todo!()
    }

    /// `lv_obj_get_parent()`
    pub fn obj_get_parent(&self, obj: Obj) -> Option<Obj> {
        todo!()
    }

    /// `lv_obj_get_child()`: negative indexes count from the end.
    pub fn obj_get_child(&self, obj: Obj, idx: i32) -> Option<Obj> {
        todo!()
    }

    /// `lv_obj_get_child_by_type()`
    pub fn obj_get_child_by_type(&self, obj: Obj, idx: i32, class: &'static ObjClass) -> Option<Obj> {
        todo!()
    }

    /// `lv_obj_get_sibling()`
    pub fn obj_get_sibling(&self, obj: Obj, idx: i32) -> Option<Obj> {
        todo!()
    }

    /// `lv_obj_get_sibling_by_type()`
    pub fn obj_get_sibling_by_type(&self, obj: Obj, idx: i32, class: &'static ObjClass) -> Option<Obj> {
        todo!()
    }

    /// `lv_obj_get_child_count()`
    pub fn obj_get_child_count(&self, obj: Obj) -> u32 {
        todo!()
    }

    /// `lv_obj_get_child_count_by_type()`
    pub fn obj_get_child_count_by_type(&self, obj: Obj, class: &'static ObjClass) -> u32 {
        todo!()
    }

    /// `lv_obj_set_name()`
    pub fn obj_set_name(&mut self, obj: Obj, name: Option<Rc<str>>) {
        todo!()
    }

    /// `lv_obj_set_name_static()`
    pub fn obj_set_name_static(&mut self, obj: Obj, name: &'static str) {
        todo!()
    }

    /// `lv_obj_get_name()`
    pub fn obj_get_name(&self, obj: Obj) -> Option<Rc<str>> {
        todo!()
    }

    /// `lv_obj_get_name_resolved()`: the name, or `<class>_<index>`.
    pub fn obj_get_name_resolved(&self, obj: Obj) -> alloc::string::String {
        todo!()
    }

    /// `lv_obj_find_by_name()`: recursive search below `parent`.
    pub fn obj_find_by_name(&self, parent: Obj, name: &str) -> Option<Obj> {
        todo!()
    }

    /// `lv_obj_get_child_by_name()`: `"a/b/c"` path lookup.
    pub fn obj_get_child_by_name(&self, parent: Obj, name_path: &str) -> Option<Obj> {
        todo!()
    }

    /// `lv_obj_get_index()`: -1 if `obj` has no parent.
    pub fn obj_get_index(&self, obj: Obj) -> i32 {
        todo!()
    }

    /// `lv_obj_get_index_by_type()`
    pub fn obj_get_index_by_type(&self, obj: Obj, class: &'static ObjClass) -> i32 {
        todo!()
    }

    /// `lv_obj_tree_walk()`: `None` walks every screen of every display.
    pub fn obj_tree_walk(&mut self, start_obj: Option<Obj>, cb: ObjTreeWalkCallback<'_>) {
        todo!()
    }

    /// `lv_obj_dump_tree()` (logs the tree; no-op without the `log` feature).
    pub fn obj_dump_tree(&mut self, start_obj: Option<Obj>) {
        todo!()
    }

    /// `lv_obj_add_child()`
    pub fn obj_add_child(&mut self, parent: Obj, child: Obj) -> LvResult {
        todo!()
    }

    /// `lv_obj_remove_child()`
    pub fn obj_remove_child(&mut self, parent: Obj, child: Obj) {
        todo!()
    }

    /// Children of `obj` (empty if it has no `spec_attr`).
    pub fn obj_children(&self, obj: Obj) -> &[crate::core::obj::ObjId] {
        todo!()
    }
}
