//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_group.h`, `src/core/lv_group_private.h`,
//! `src/core/lv_group.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::misc::arena::Key as ArenaKey;
use crate::misc::event::LvResult;

/// Handle of a group (`lv_group_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GroupId(pub ArenaKey);

/// `lv_key_t`: key codes understood by widgets. Any other `u32` is a
/// character.
pub type Key = u32;

/// `LV_KEY_UP`
pub const KEY_UP: Key = 17;
/// `LV_KEY_DOWN`
pub const KEY_DOWN: Key = 18;
/// `LV_KEY_RIGHT`
pub const KEY_RIGHT: Key = 19;
/// `LV_KEY_LEFT`
pub const KEY_LEFT: Key = 20;
/// `LV_KEY_ESC`
pub const KEY_ESC: Key = 27;
/// `LV_KEY_DEL`
pub const KEY_DEL: Key = 127;
/// `LV_KEY_BACKSPACE`
pub const KEY_BACKSPACE: Key = 8;
/// `LV_KEY_ENTER`
pub const KEY_ENTER: Key = 10;
/// `LV_KEY_NEXT`
pub const KEY_NEXT: Key = 9;
/// `LV_KEY_PREV`
pub const KEY_PREV: Key = 11;
/// `LV_KEY_HOME`
pub const KEY_HOME: Key = 2;
/// `LV_KEY_END`
pub const KEY_END: Key = 3;

/// `lv_group_refocus_policy_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GroupRefocusPolicy {
    /// `LV_GROUP_REFOCUS_POLICY_NEXT`
    #[default]
    Next = 0,
    /// `LV_GROUP_REFOCUS_POLICY_PREV`
    Prev = 1,
}

/// `lv_group_focus_cb_t`
pub type GroupFocusCallback = Rc<dyn Fn(&mut Ui, GroupId)>;
/// `lv_group_edge_cb_t`
pub type GroupEdgeCallback = Rc<dyn Fn(&mut Ui, GroupId, bool)>;

/// `lv_group_t`
pub struct GroupData {
    /// `obj_ll`
    pub objs: Vec<Obj>,
    /// `obj_focus`: index into `objs`.
    pub focus: Option<usize>,
    /// `focus_cb`
    pub focus_cb: Option<GroupFocusCallback>,
    /// `edge_cb`
    pub edge_cb: Option<GroupEdgeCallback>,
    /// `user_data`
    pub user_data: Option<Rc<dyn Any>>,
    /// `frozen`
    pub frozen: bool,
    /// `editing`
    pub editing: bool,
    /// `refocus_policy`
    pub refocus_policy: GroupRefocusPolicy,
    /// `wrap`
    pub wrap: bool,
}

impl Ui {
    /// `lv_group_create()`
    pub fn group_create(&mut self) -> GroupId {
        todo!()
    }

    /// `lv_group_delete()`
    pub fn group_delete(&mut self, group: GroupId) {
        todo!()
    }

    /// `lv_group_set_default()`
    pub fn group_set_default(&mut self, group: Option<GroupId>) {
        self.group_default = group;
    }

    /// `lv_group_get_default()`
    pub fn group_get_default(&self) -> Option<GroupId> {
        self.group_default
    }

    /// `lv_group_add_obj()`
    pub fn group_add_obj(&mut self, group: GroupId, obj: Obj) {
        todo!()
    }

    /// `lv_group_swap_obj()`
    pub fn group_swap_obj(&mut self, obj1: Obj, obj2: Obj) {
        todo!()
    }

    /// `lv_group_remove_obj()`
    pub fn group_remove_obj(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_group_remove_all_objs()`
    pub fn group_remove_all_objs(&mut self, group: GroupId) {
        todo!()
    }

    /// `lv_group_focus_obj()`
    pub fn group_focus_obj(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_group_focus_next()`
    pub fn group_focus_next(&mut self, group: GroupId) {
        todo!()
    }

    /// `lv_group_focus_prev()`
    pub fn group_focus_prev(&mut self, group: GroupId) {
        todo!()
    }

    /// `lv_group_focus_freeze()`
    pub fn group_focus_freeze(&mut self, group: GroupId, en: bool) {
        todo!()
    }

    /// `lv_group_send_data()`
    pub fn group_send_data(&mut self, group: GroupId, c: u32) -> LvResult {
        todo!()
    }

    /// `lv_group_set_focus_cb()`
    pub fn group_set_focus_cb(&mut self, group: GroupId, focus_cb: Option<GroupFocusCallback>) {
        todo!()
    }

    /// `lv_group_set_edge_cb()`
    pub fn group_set_edge_cb(&mut self, group: GroupId, edge_cb: Option<GroupEdgeCallback>) {
        todo!()
    }

    /// `lv_group_set_refocus_policy()`
    pub fn group_set_refocus_policy(&mut self, group: GroupId, policy: GroupRefocusPolicy) {
        todo!()
    }

    /// `lv_group_set_editing()`
    pub fn group_set_editing(&mut self, group: GroupId, edit: bool) {
        todo!()
    }

    /// `lv_group_set_wrap()`
    pub fn group_set_wrap(&mut self, group: GroupId, en: bool) {
        todo!()
    }

    /// `lv_group_get_focused()`
    pub fn group_get_focused(&self, group: GroupId) -> Option<Obj> {
        todo!()
    }

    /// `lv_group_get_focus_cb()`
    pub fn group_get_focus_cb(&self, group: GroupId) -> Option<GroupFocusCallback> {
        todo!()
    }

    /// `lv_group_get_edge_cb()`
    pub fn group_get_edge_cb(&self, group: GroupId) -> Option<GroupEdgeCallback> {
        todo!()
    }

    /// `lv_group_get_editing()`
    pub fn group_get_editing(&self, group: GroupId) -> bool {
        todo!()
    }

    /// `lv_group_get_wrap()`
    pub fn group_get_wrap(&self, group: GroupId) -> bool {
        todo!()
    }

    /// `lv_group_get_obj_count()`
    pub fn group_get_obj_count(&self, group: GroupId) -> u32 {
        todo!()
    }

    /// `lv_group_get_obj_by_index()`
    pub fn group_get_obj_by_index(&self, group: GroupId, index: u32) -> Option<Obj> {
        todo!()
    }

    /// `lv_group_get_count()`
    pub fn group_get_count(&self) -> u32 {
        self.groups.len() as u32
    }

    /// `lv_group_by_index()`
    pub fn group_by_index(&self, index: u32) -> Option<GroupId> {
        todo!()
    }

    /// `lv_group_set_user_data()`
    pub fn group_set_user_data(&mut self, group: GroupId, user_data: Option<Rc<dyn Any>>) {
        todo!()
    }

    /// `lv_group_get_user_data()`
    pub fn group_get_user_data(&self, group: GroupId) -> Option<Rc<dyn Any>> {
        todo!()
    }

    /// Borrow a group.
    #[inline]
    pub fn group(&self, group: GroupId) -> Option<&GroupData> {
        self.groups.get(group.0)
    }

    /// Mutably borrow a group.
    #[inline]
    pub fn group_mut(&mut self, group: GroupId) -> Option<&mut GroupData> {
        self.groups.get_mut(group.0)
    }
}
