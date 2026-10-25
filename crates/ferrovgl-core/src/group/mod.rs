//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_group.h`, `src/core/lv_group_private.h`,
//! `src/core/lv_group.c`.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::indev::{IndevId, IndevType};
use crate::misc::arena::Key as ArenaKey;
use crate::misc::event::{EventCode, EventParam, LvResult};
use crate::misc::style::State;

/// Handle of a group (`lv_group_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    // Reentrant callbacks can delete the group or remove their own object.
    deleting: bool,
    clearing: bool,
    removing: Vec<Obj>,
}

impl core::fmt::Debug for GroupData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GroupData")
            .field("objs", &self.objs)
            .field("focus", &self.focus)
            .field(
                "focus_cb",
                &self.focus_cb.as_ref().map(|_| crate::misc::OpaqueCallback),
            )
            .field(
                "edge_cb",
                &self.edge_cb.as_ref().map(|_| crate::misc::OpaqueCallback),
            )
            .field("user_data", &self.user_data)
            .field("frozen", &self.frozen)
            .field("editing", &self.editing)
            .field("refocus_policy", &self.refocus_policy)
            .field("wrap", &self.wrap)
            .field("deleting", &self.deleting)
            .field("clearing", &self.clearing)
            .field("removing", &self.removing)
            .finish()
    }
}

impl Ui {
    /// `lv_group_create()`
    pub fn group_create(&mut self) -> GroupId {
        let group = GroupId(self.groups.insert(GroupData {
            objs: Vec::new(),
            focus: None,
            focus_cb: None,
            edge_cb: None,
            user_data: None,
            frozen: false,
            editing: false,
            refocus_policy: GroupRefocusPolicy::Prev,
            wrap: true,
            deleting: false,
            clearing: false,
            removing: Vec::new(),
        }));
        self.group_order.insert(0, group);
        group
    }

    /// `lv_group_delete()`
    pub fn group_delete(&mut self, group: GroupId) {
        let Some(data) = self.group_mut(group) else { return };
        if data.deleting {
            return;
        }
        data.deleting = true;
        // A defocus callback can request group deletion while a removal is
        // already dispatching that same event. Keep focus observable, but do
        // not recursively send DEFOCUSED for it again.
        let defocusing = data.clearing
            || data
                .focus
                .and_then(|index| data.objs.get(index))
                .is_some_and(|obj| data.removing.contains(obj));
        if !defocusing && let Some(obj) = self.group_get_focused(group) {
            let param = self.group_indev_param(group);
            self.obj_send_event(obj, EventCode::DEFOCUSED, param);
            if self.obj_is_valid(obj) {
                self.obj_invalidate(obj);
            }
        }
        self.group_clear_members(group);
        for id in self.indev_order.clone() {
            if self.indev_get_group(id) == Some(group) {
                self.indev_set_group(id, None);
            }
        }
        if self.group_default == Some(group) {
            self.group_default = None;
        }
        self.group_order.retain(|&id| id != group);
        self.groups.remove(group.0);
    }

    /// `lv_group_set_default()`
    pub fn group_set_default(&mut self, group: Option<GroupId>) {
        self.group_default = group;
    }

    /// `lv_group_get_default()`
    #[must_use]
    pub fn group_get_default(&self) -> Option<GroupId> {
        self.group_default
    }

    /// `lv_group_add_obj()`
    pub fn group_add_obj(&mut self, group: GroupId, obj: Obj) {
        if self.group(group).is_none_or(|g| g.deleting) || !self.obj_is_valid(obj) {
            return;
        }
        self.obj_allocate_spec_attr(obj);
        self.group_remove_obj(obj);
        if self.group(group).is_none_or(|g| g.deleting) || !self.obj_is_valid(obj) {
            return;
        }
        // A removal callback may have moved the object into a different group.
        if self.obj_get_group(obj).is_some() {
            return;
        }
        self.obj_allocate_spec_attr(obj).unwrap().group = Some(group);
        let g = self.group_mut(group).unwrap();
        g.objs.push(obj);
        if g.objs.len() == 1 {
            self.group_refocus(group);
        }
    }

    /// `lv_group_swap_obj()`
    pub fn group_swap_obj(&mut self, obj1: Obj, obj2: Obj) {
        let Some(group) = self.obj_get_group(obj1) else {
            return;
        };
        if self.obj_get_group(obj2) != Some(group) {
            return;
        }
        let Some(g) = self.group_mut(group) else { return };
        for obj in &mut g.objs {
            if *obj == obj1 {
                *obj = obj2;
            } else if *obj == obj2 {
                *obj = obj1;
            }
        }
        // C swaps the list entries first and then restores focus to the object.
        match self.group_get_focused(group) {
            Some(obj) if obj == obj1 => self.group_focus_obj(obj2),
            Some(obj) if obj == obj2 => self.group_focus_obj(obj1),
            _ => {}
        }
    }

    /// `lv_group_remove_obj()`
    pub fn group_remove_obj(&mut self, obj: Obj) {
        let Some(group) = self.obj_get_group(obj) else {
            return;
        };
        let Some(g) = self.group_mut(group) else { return };
        if g.removing.contains(&obj) {
            return;
        }
        g.removing.push(obj);
        if self.group_get_focused(group) == Some(obj) {
            let g = self.group_mut(group).unwrap();
            g.frozen = false;
            if g.objs.len() > 1 && !g.deleting && !g.clearing {
                self.group_refocus(group);
            }
        }
        if self.group_get_focused(group) == Some(obj) {
            // C leaves the old focus observable throughout DEFOCUSED.
            // `removing` guards recursive removal; delete/remove-all already
            // dispatch their own DEFOCUSED event before entering here.
            if self.group(group).is_some_and(|g| !g.deleting && !g.clearing) {
                let param = self.group_indev_param(group);
                self.obj_send_event(obj, EventCode::DEFOCUSED, param);
            }
            if self.group_get_focused(group) == Some(obj) {
                self.group_mut(group).unwrap().focus = None;
            }
        }
        if let Some(g) = self.group_mut(group) {
            if let Some(index) = g.objs.iter().position(|&o| o == obj) {
                g.objs.remove(index);
                g.focus = g.focus.and_then(|focused| {
                    if focused == index {
                        None
                    } else {
                        Some(focused - usize::from(focused > index))
                    }
                });
            }
            g.removing.retain(|&o| o != obj);
        }
        if let Some(spec) = self.obj_mut(obj).and_then(|o| o.spec.as_deref_mut())
            && spec.group == Some(group)
        {
            spec.group = None;
        }
    }

    /// `lv_group_remove_all_objs()`
    pub fn group_remove_all_objs(&mut self, group: GroupId) {
        let Some(data) = self.group_mut(group) else { return };
        if data.clearing || data.deleting {
            return;
        }
        data.clearing = true;
        let defocusing = data
            .focus
            .and_then(|index| data.objs.get(index))
            .is_some_and(|obj| data.removing.contains(obj));
        if !defocusing && let Some(obj) = self.group_get_focused(group) {
            let param = self.group_indev_param(group);
            self.obj_send_event(obj, EventCode::DEFOCUSED, param);
            if self.obj_is_valid(obj) {
                self.obj_invalidate(obj);
            }
        }
        self.group_clear_members(group);
        if let Some(data) = self.group_mut(group) {
            data.clearing = false;
        }
    }

    /// `lv_group_focus_obj()`
    pub fn group_focus_obj(&mut self, obj: Obj) {
        let Some(group) = self.obj_get_group(obj) else {
            return;
        };
        if self.group(group).is_none_or(|g| g.frozen || g.deleting) {
            return;
        }
        self.group_set_editing(group, false);
        if self.group_get_focused(group) == Some(obj) {
            return;
        }
        if let Some(old) = self.group_get_focused(group) {
            let param = self.group_indev_param(group);
            if self.obj_send_event(old, EventCode::DEFOCUSED, param) != LvResult::Ok {
                return;
            }
            if self.obj_is_valid(old) {
                self.obj_invalidate(old);
            }
        }
        if !self.group_assign_focus(group, obj) {
            return;
        }
        // Explicit focus calls the callback BEFORE FOCUSED; traversal does after.
        if let Some(callback) = self.group_get_focus_cb(group) {
            callback(self, group);
        }
        let Some(focused) = self.group_get_focused(group) else {
            return;
        };
        let param = self.group_indev_param(group);
        if self.obj_send_event(focused, EventCode::FOCUSED, param) == LvResult::Ok
            && self.obj_is_valid(focused)
        {
            self.obj_invalidate(focused);
        }
    }

    /// `lv_group_focus_next()`
    pub fn group_focus_next(&mut self, group: GroupId) {
        if !self.group_focus_step(group, true)
            && let Some(cb) = self.group_get_edge_cb(group)
        {
            cb(self, group, true);
        }
    }

    /// `lv_group_focus_prev()`
    pub fn group_focus_prev(&mut self, group: GroupId) {
        if !self.group_focus_step(group, false)
            && let Some(cb) = self.group_get_edge_cb(group)
        {
            cb(self, group, false);
        }
    }

    /// `lv_group_focus_freeze()`
    pub fn group_focus_freeze(&mut self, group: GroupId, en: bool) {
        if let Some(g) = self.group_mut(group) {
            g.frozen = en;
        }
    }

    /// `lv_group_send_data()`
    pub fn group_send_data(&mut self, group: GroupId, c: u32) -> LvResult {
        if self.group(group).is_none() {
            return LvResult::Invalid;
        }
        let Some(obj) = self.group_get_focused(group) else {
            return LvResult::Ok;
        };
        if self.obj_has_state(obj, State::DISABLED) {
            return LvResult::Ok;
        }
        self.obj_send_event(obj, EventCode::KEY, EventParam::Key(c))
    }

    /// `lv_group_set_focus_cb()`
    pub fn group_set_focus_cb(&mut self, group: GroupId, focus_cb: Option<GroupFocusCallback>) {
        if let Some(g) = self.group_mut(group) {
            g.focus_cb = focus_cb;
        }
    }

    /// `lv_group_set_edge_cb()`
    pub fn group_set_edge_cb(&mut self, group: GroupId, edge_cb: Option<GroupEdgeCallback>) {
        if let Some(g) = self.group_mut(group) {
            g.edge_cb = edge_cb;
        }
    }

    /// `lv_group_set_refocus_policy()`
    pub fn group_set_refocus_policy(&mut self, group: GroupId, policy: GroupRefocusPolicy) {
        if let Some(g) = self.group_mut(group) {
            g.refocus_policy = policy;
        }
    }

    /// `lv_group_set_editing()`
    pub fn group_set_editing(&mut self, group: GroupId, edit: bool) {
        let Some(g) = self.group_mut(group) else { return };
        if g.editing == edit {
            return;
        }
        g.editing = edit;
        if let Some(obj) = self.group_get_focused(group) {
            let param = self.group_indev_param(group);
            if self.obj_send_event(obj, EventCode::FOCUSED, param) == LvResult::Ok && self.obj_is_valid(obj) {
                self.obj_invalidate(obj);
            }
        }
    }

    /// `lv_group_set_wrap()`
    pub fn group_set_wrap(&mut self, group: GroupId, en: bool) {
        if let Some(g) = self.group_mut(group) {
            g.wrap = en;
        }
    }

    /// `lv_group_get_focused()`
    #[must_use]
    pub fn group_get_focused(&self, group: GroupId) -> Option<Obj> {
        let g = self.group(group)?;
        g.objs
            .get(g.focus?)
            .copied()
            .filter(|&obj| self.obj_is_valid(obj))
    }

    /// `lv_group_get_focus_cb()`
    #[must_use]
    pub fn group_get_focus_cb(&self, group: GroupId) -> Option<GroupFocusCallback> {
        self.group(group)?.focus_cb.clone()
    }

    /// `lv_group_get_edge_cb()`
    #[must_use]
    pub fn group_get_edge_cb(&self, group: GroupId) -> Option<GroupEdgeCallback> {
        self.group(group)?.edge_cb.clone()
    }

    /// `lv_group_get_editing()`
    #[must_use]
    pub fn group_get_editing(&self, group: GroupId) -> bool {
        self.group(group).is_some_and(|g| g.editing)
    }

    /// `lv_group_get_wrap()`
    #[must_use]
    pub fn group_get_wrap(&self, group: GroupId) -> bool {
        self.group(group).is_some_and(|g| g.wrap)
    }

    /// `lv_group_get_obj_count()`
    #[must_use]
    pub fn group_get_obj_count(&self, group: GroupId) -> u32 {
        self.group(group).map_or(0, |g| g.objs.len() as u32)
    }

    /// `lv_group_get_obj_by_index()`
    #[must_use]
    pub fn group_get_obj_by_index(&self, group: GroupId, index: u32) -> Option<Obj> {
        self.group(group)?.objs.get(index as usize).copied()
    }

    /// `lv_group_get_count()`
    #[must_use]
    pub fn group_get_count(&self) -> u32 {
        self.groups.len() as u32
    }

    /// `lv_group_by_index()`
    #[must_use]
    pub fn group_by_index(&self, index: u32) -> Option<GroupId> {
        self.group_order.iter().rev().nth(index as usize).copied()
    }

    /// `lv_group_set_user_data()`
    pub fn group_set_user_data(&mut self, group: GroupId, user_data: Option<Rc<dyn Any>>) {
        if let Some(g) = self.group_mut(group) {
            g.user_data = user_data;
        }
    }

    /// `lv_group_get_user_data()`
    #[must_use]
    pub fn group_get_user_data(&self, group: GroupId) -> Option<Rc<dyn Any>> {
        self.group(group)?.user_data.clone()
    }

    /// Borrow a group.
    #[inline]
    #[must_use]
    pub fn group(&self, group: GroupId) -> Option<&GroupData> {
        self.groups.get(group.0)
    }

    /// Mutably borrow a group.
    #[inline]
    #[must_use]
    pub fn group_mut(&mut self, group: GroupId) -> Option<&mut GroupData> {
        self.groups.get_mut(group.0)
    }
}

impl Ui {
    fn group_clear_members(&mut self, group: GroupId) {
        let Some(g) = self.group_mut(group) else { return };
        g.focus = None;
        let objects = core::mem::take(&mut g.objs);
        for obj in objects {
            if let Some(spec) = self.obj_mut(obj).and_then(|o| o.spec.as_deref_mut())
                && spec.group == Some(group)
            {
                spec.group = None;
            }
        }
    }

    fn group_refocus(&mut self, group: GroupId) {
        let Some(g) = self.group_mut(group) else { return };
        let wrap = g.wrap;
        let next = g.refocus_policy == GroupRefocusPolicy::Next;
        g.wrap = true;
        if next {
            self.group_focus_next(group);
        } else {
            self.group_focus_prev(group);
        }
        if let Some(g) = self.group_mut(group) {
            g.wrap = wrap;
        }
    }

    fn group_assign_focus(&mut self, group: GroupId, obj: Obj) -> bool {
        if self.obj_get_group(obj) != Some(group) {
            return false;
        }
        let Some(g) = self.group_mut(group) else {
            return false;
        };
        if g.deleting {
            return false;
        }
        let Some(index) = g.objs.iter().position(|&o| o == obj) else {
            return false;
        };
        g.focus = Some(index);
        true
    }

    fn group_focus_step(&mut self, group: GroupId, next: bool) -> bool {
        let Some(g) = self.group(group) else { return false };
        if g.frozen || g.deleting || g.objs.is_empty() {
            return false;
        }
        let count = g.objs.len();
        let mut candidate = None;
        let mut index = g
            .focus
            .map(|i| i as isize)
            .unwrap_or(if next { -1 } else { count as isize });
        for _ in 0..count {
            index += if next { 1 } else { -1 };
            if index < 0 || index >= count as isize {
                if !g.wrap && g.focus.is_some() {
                    break;
                }
                index = if next { 0 } else { count as isize - 1 };
            }
            let obj = g.objs[index as usize];
            if !self.obj_is_valid(obj) || self.obj_has_state(obj, State::DISABLED) {
                continue;
            }
            let mut parent = Some(obj);
            let mut hidden = false;
            while let Some(o) = parent {
                if self.obj_is_hidden(o) {
                    hidden = true;
                    break;
                }
                parent = self.obj_get_parent(o);
            }
            if !hidden {
                candidate = Some(obj);
                break;
            }
        }
        let Some(candidate) = candidate else { return false };
        if self.group_get_focused(group) == Some(candidate) {
            return false;
        }
        if let Some(old) = self.group_get_focused(group) {
            let param = self.group_indev_param(group);
            if self.obj_send_event(old, EventCode::DEFOCUSED, param) != LvResult::Ok {
                return false;
            }
            if self.obj_is_valid(old) {
                self.obj_invalidate(old);
            }
        }
        if !self.group_assign_focus(group, candidate) {
            return false;
        }
        let param = self.group_indev_param(group);
        if self.obj_send_event(candidate, EventCode::FOCUSED, param) != LvResult::Ok {
            return false;
        }
        if !self.obj_is_valid(candidate) || self.group(group).is_none() {
            return false;
        }
        self.obj_invalidate(candidate);
        if let Some(cb) = self.group_get_focus_cb(group) {
            cb(self, group);
        }
        true
    }

    fn group_indev_param(&self, group: GroupId) -> EventParam<'static> {
        self.group_guess_indev(group)
            .map_or(EventParam::None, EventParam::Indev)
    }

    fn group_guess_indev(&self, group: GroupId) -> Option<IndevId> {
        let mut guess = None;
        for &id in &self.indev_order {
            // C prefers the first pointer even if it has no group assigned.
            if self.indev_get_type(id) == IndevType::Pointer {
                return Some(id);
            }
            if self.indev_get_group(id) == Some(group) {
                guess = Some(id);
            }
        }
        guess
    }
}

#[cfg(test)]
mod tests;
