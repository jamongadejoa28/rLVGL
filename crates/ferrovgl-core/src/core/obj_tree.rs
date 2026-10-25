//! Ported from lvgl v9.6.0: `src/core/lv_obj_tree.c`
//!
//! Also covers `include/lvgl/core/lv_obj_tree.h`, `lv_obj_add_child()` /
//! `lv_obj_remove_child()` of `src/core/lv_obj.c` and the
//! `lv_obj_move_foreground()` / `lv_obj_move_background()` inlines of
//! `include/lvgl/api_map/lv_api_map_v8.h`.
//!
//! Parent/child relations, deletion and tree walking.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::core::Ui;
use crate::core::obj::{Obj, ObjClass, ObjId, ObjTreeWalkRes, SpecAttr};
use crate::core::ui::AsyncJob;
use crate::display::DisplayId;
use crate::indev::{IndevId, IndevState, IndevType};
use crate::misc::anim::{ANIM_OFF, Anim, AnimCallback, AnimExec, AnimId, AnimTarget};
use crate::misc::check_arg_failed;
use crate::misc::event::{EventCode, EventParam, LvResult};

/// `lv_obj_tree_walk_cb_t`
pub type ObjTreeWalkCallback<'a> = &'a mut dyn FnMut(&mut Ui, Obj) -> ObjTreeWalkRes;

/// `LV_OBJ_NAME_MAX_LEN`: size of the buffers names are resolved into.
pub const OBJ_NAME_MAX_LEN: usize = 128;

impl Ui {
    /// `lv_obj_get_child(obj, i)` for `i >= 0`, read straight from `spec_attr`.
    #[inline]
    pub(super) fn child_at(&self, obj: Obj, i: u32) -> Option<Obj> {
        let spec = self.obj(obj)?.spec.as_deref()?;
        spec.children.get(i as usize).copied().map(Obj::from)
    }

    /// `lv_obj_delete()`: send `LV_EVENT_DELETE`, delete the children
    /// (deepest first), run the destructors and free the slot.
    pub fn obj_delete(&mut self, obj: Obj) {
        // `obj == NULL`
        let Some(o) = self.obj(obj) else { return };
        if o.is_deleting {
            return;
        }

        #[cfg(feature = "log")]
        log::trace!("begin (delete {:?})", obj);
        self.obj_invalidate(obj);

        let par = self.obj_get_parent(obj);

        let mut disp: Option<DisplayId> = None;
        let mut act_screen_del = false;
        if par.is_none() {
            disp = self.obj_get_display(obj);
            let Some(d) = disp else { return }; // Shouldn't happen
            if self.displays.get(d.0).and_then(|d| d.act_scr) == Some(obj.id()) {
                act_screen_del = true;
            }
        }

        self.obj_tree_delete_core(obj);

        // Call the ancestor's event handler to the parent to notify it about the child delete
        if let Some(par) = par
            && self.obj(par).is_some_and(|p| !p.is_deleting)
        {
            self.obj_scrollbar_invalidate(par);
            self.obj_send_event(par, EventCode::CHILD_CHANGED, EventParam::None);
            self.obj_send_event(par, EventCode::CHILD_DELETED, EventParam::None);
        }

        // Handle if the active screen was deleted
        if act_screen_del {
            #[cfg(feature = "log")]
            log::warn!("the active screen was deleted");
            if let Some(d) = disp.and_then(|d| self.displays.get_mut(d.0)) {
                d.act_scr = None;
            }
        }

        #[cfg(feature = "log")]
        log::trace!("finished (delete {:?})", obj);
    }

    /// `lv_obj_clean()`: delete all children.
    pub fn obj_clean(&mut self, obj: Obj) {
        #[cfg(feature = "log")]
        log::trace!("begin (clean {:?})", obj);
        if !self.obj_tree_check_obj(obj) {
            return;
        }

        self.obj_invalidate(obj);

        let cnt = self.obj_get_child_count(obj);
        let mut child = self.obj_tree_first_not_deleting_child(obj);
        while let Some(c) = child {
            self.obj_tree_delete_core(c);
            child = self.obj_tree_first_not_deleting_child(obj);
        }
        // Just to remove scroll animations if any
        self.obj_scroll_to(obj, 0, 0, ANIM_OFF);
        if let Some(spec) = self.obj_mut(obj).and_then(|o| o.spec.as_deref_mut()) {
            spec.scroll.x = 0;
            spec.scroll.y = 0;
        }

        if self.obj_get_child_count(obj) < cnt {
            self.obj_send_event(obj, EventCode::CHILD_CHANGED, EventParam::None);
            self.obj_send_event(obj, EventCode::CHILD_DELETED, EventParam::None);
        }

        #[cfg(feature = "log")]
        log::trace!("finished (clean {:?})", obj);
    }

    /// `lv_obj_delete_delayed()`: delete `obj` after `delay_ms` using an
    /// animation whose completed callback is
    /// [`obj_delete_anim_completed_cb`](Self::obj_delete_anim_completed_cb).
    pub fn obj_delete_delayed(&mut self, obj: Obj, delay_ms: u32) {
        if !self.obj_is_valid(obj) {
            return;
        }

        let completed: AnimCallback = Rc::new(|ui: &mut Ui, a: AnimId| ui.obj_delete_anim_completed_cb(a));
        let mut a = Anim::new();
        a.set_var(AnimTarget::Obj(obj));
        a.set_exec_cb(AnimExec::None);
        a.set_duration(1);
        a.set_delay(delay_ms);
        a.set_completed_cb(Some(completed));
        self.anim_start(&a);
    }

    /// `lv_obj_delete_anim_completed_cb()`: delete the object an animation
    /// was running on (usable as an animation's completed callback).
    pub fn obj_delete_anim_completed_cb(&mut self, a: AnimId) {
        let Some(var) = self.anim(a).map(|a| a.var) else {
            check_arg_failed("lv_obj_delete_anim_completed_cb: a != NULL");
            return;
        };

        if let AnimTarget::Obj(obj) = var {
            self.obj_delete(obj);
        }
    }

    /// `lv_obj_delete_async()`: delete from the next `timer_handler()`.
    pub fn obj_delete_async(&mut self, obj: Obj) {
        if !self.obj_is_valid(obj) {
            return;
        }

        // lv_async_call(lv_obj_delete_async_cb, obj)
        self.async_call_job(AsyncJob::DeleteObj(obj));
    }

    /// `lv_obj_set_parent()`: move `obj` to the end of `parent`'s children.
    pub fn obj_set_parent(&mut self, obj: Obj, parent: Obj) {
        if !self.obj_tree_check_obj(obj) || !self.obj_tree_check_obj(parent) {
            return;
        }

        let Some(old_parent) = self.obj_tree_parent_field(obj) else {
            #[cfg(feature = "log")]
            log::warn!("Can't set the parent of a screen");
            return;
        };

        if parent == old_parent {
            return;
        }

        if !self.obj_tree_allocate_spec_attr(parent) {
            return;
        }

        // Add the object to the new parents list first
        if self.obj_add_child(parent, obj) != LvResult::Ok {
            #[cfg(feature = "log")]
            log::warn!("Failed to attach child to parent");
            return;
        }

        // Remove the object from the old parent's child list
        // This should never fail, thus we do only do it after adding it to the new parent's list
        self.obj_remove_child(old_parent, obj);

        // Notify the original parent because one of its children is lost
        let Some(p) = self.obj_tree_parent_field(obj) else {
            return;
        };
        self.obj_scrollbar_invalidate(p);
        let Some(p) = self.obj_tree_parent_field(obj) else {
            return;
        };
        self.obj_send_event(p, EventCode::CHILD_CHANGED, EventParam::Obj(obj));
        let Some(p) = self.obj_tree_parent_field(obj) else {
            return;
        };
        self.obj_send_event(p, EventCode::CHILD_DELETED, EventParam::None);
        let Some(o) = self.obj_mut(obj) else { return };
        o.parent = Some(parent.id());

        // Notify the new parent about the child
        self.obj_send_event(parent, EventCode::CHILD_CHANGED, EventParam::Obj(obj));
        self.obj_send_event(parent, EventCode::CHILD_CREATED, EventParam::None);

        self.obj_mark_layout_as_dirty(obj);

        self.obj_invalidate(obj);
    }

    /// `lv_obj_swap()`: swap the positions of two objects in their parents'
    /// child lists (the parents are swapped too).
    pub fn obj_swap(&mut self, obj1: Obj, obj2: Obj) {
        if !self.obj_tree_check_obj(obj1) || !self.obj_tree_check_obj(obj2) {
            return;
        }

        let parent = self.obj_get_parent(obj1);
        let parent2 = self.obj_get_parent(obj2);
        // C dereferences `parent->spec_attr`: swapping a screen is undefined there.
        let (Some(parent), Some(parent2)) = (parent, parent2) else {
            check_arg_failed("lv_obj_swap: both objects need a parent");
            return;
        };

        let index1 = self.obj_get_index(obj1);
        let index2 = self.obj_get_index(obj2);

        self.obj_send_event(parent2, EventCode::CHILD_DELETED, EventParam::Obj(obj2));
        self.obj_send_event(parent, EventCode::CHILD_DELETED, EventParam::Obj(obj1));

        if let Some(slot) = self.obj_tree_child_slot(parent, index1) {
            *slot = obj2.id();
        }
        if let Some(o) = self.obj_mut(obj2) {
            o.parent = Some(parent.id());
        }

        if let Some(slot) = self.obj_tree_child_slot(parent2, index2) {
            *slot = obj1.id();
        }
        if let Some(o) = self.obj_mut(obj1) {
            o.parent = Some(parent2.id());
        }

        self.obj_send_event(parent, EventCode::CHILD_CHANGED, EventParam::Obj(obj2));
        self.obj_send_event(parent, EventCode::CHILD_CREATED, EventParam::Obj(obj2));
        self.obj_send_event(parent2, EventCode::CHILD_CHANGED, EventParam::Obj(obj1));
        self.obj_send_event(parent2, EventCode::CHILD_CREATED, EventParam::Obj(obj1));

        self.obj_invalidate(parent);

        if parent != parent2 {
            self.obj_invalidate(parent2);
        }
        self.group_swap_obj(obj1, obj2);
    }

    /// `lv_obj_move_to_index()`: negative indexes count from the end.
    pub fn obj_move_to_index(&mut self, obj: Obj, index: i32) {
        if !self.obj_tree_check_obj(obj) {
            return;
        }

        // Check parent validity
        let Some(parent) = self.obj_get_parent(obj) else {
            #[cfg(feature = "log")]
            log::warn!("parent is NULL");
            return;
        };

        let parent_child_count = self.obj_get_child_count(parent);
        // old_index only can be 0 or greater, this point cannot be reached if the parent is not null
        let old_index = self.obj_get_index(obj);
        debug_assert!(0 <= old_index);

        let mut index = index;
        if index < 0 {
            index = (index as u32).wrapping_add(parent_child_count) as i32;
        }

        // Index was negative and the absolute value is greater than parent child count
        if (index < 0)
            // Index is same or bigger than parent child count
            || (index >= parent_child_count as i32)
            // If both previous and new index are the same
            || (index == old_index)
        {
            return;
        }

        let Some(children) = self
            .obj_mut(parent)
            .and_then(|p| p.spec.as_deref_mut())
            .map(|s| &mut s.children)
        else {
            return;
        };
        let mut i = old_index;
        if index < old_index {
            while i > index {
                children[i as usize] = children[(i - 1) as usize];
                i -= 1;
            }
        } else {
            while i < index {
                children[i as usize] = children[(i + 1) as usize];
                i += 1;
            }
        }

        children[index as usize] = obj.id();
        self.obj_send_event(parent, EventCode::CHILD_CHANGED, EventParam::None);
        self.obj_invalidate(parent);
    }

    /// `lv_obj_move_foreground()` (`lv_api_map_v8.h`): move `obj` to the
    /// end of its parent's children.
    pub fn obj_move_foreground(&mut self, obj: Obj) {
        let Some(parent) = self.obj_get_parent(obj) else {
            #[cfg(feature = "log")]
            log::warn!("parent is NULL");
            return;
        };

        let cnt = self.obj_get_child_count(parent) as i32;
        self.obj_move_to_index(obj, cnt.wrapping_sub(1));
    }

    /// `lv_obj_move_background()` (`lv_api_map_v8.h`): move `obj` to the
    /// start of its parent's children.
    pub fn obj_move_background(&mut self, obj: Obj) {
        self.obj_move_to_index(obj, 0);
    }

    /// `lv_obj_get_screen()`: the root of `obj`'s tree.
    pub fn obj_get_screen(&self, obj: Obj) -> Obj {
        if !self.obj_tree_check_obj(obj) {
            return obj;
        }

        let mut act_par = obj;
        while let Some(par) = self.obj_get_parent(act_par) {
            act_par = par;
        }

        act_par
    }

    /// `lv_obj_get_display()`: the display whose screen list contains
    /// `obj`'s screen.
    pub fn obj_get_display(&self, obj: Obj) -> Option<DisplayId> {
        let o = self.obj_tree_obj_checked(obj)?;

        let scr = if o.parent.is_none() {
            obj // `obj` is a screen
        } else {
            self.obj_get_screen(obj) // get the screen of `obj`
        };

        let mut d = self.display_get_next(None);
        while let Some(disp) = d {
            if let Some(data) = self.displays.get(disp.0)
                && data.screens.contains(&scr.id())
            {
                return Some(disp);
            }
            d = self.display_get_next(Some(disp));
        }

        #[cfg(feature = "log")]
        log::warn!("No screen found");
        None
    }

    /// `lv_obj_get_parent()`
    pub fn obj_get_parent(&self, obj: Obj) -> Option<Obj> {
        let o = self.obj_tree_obj_checked(obj)?;

        o.parent.map(Obj)
    }

    /// `lv_obj_get_child()`: negative indexes count from the end.
    pub fn obj_get_child(&self, obj: Obj, idx: i32) -> Option<Obj> {
        let o = self.obj_tree_obj_checked(obj)?;

        let spec = o.spec.as_deref()?;
        let child_cnt = spec.children.len() as u32;

        let mut idx = idx;
        if idx < 0 {
            idx = (child_cnt as i32).wrapping_add(idx);
            if idx < 0 {
                return None;
            }
        }

        let idu = idx as u32;

        if idu < child_cnt {
            Some(Obj(spec.children[idu as usize]))
        } else {
            None
        }
    }

    /// `lv_obj_get_child_by_type()`: the `idx`-th child whose class is
    /// exactly `class` (`-1` is the last such child).
    pub fn obj_get_child_by_type(&self, obj: Obj, idx: i32, class: &'static ObjClass) -> Option<Obj> {
        let o = self.obj_tree_obj_checked(obj)?;

        let spec = o.spec.as_deref()?;

        let cnt = spec.children.len() as i32;
        let mut idx = idx;
        if idx >= 0 {
            for i in 0..cnt {
                let child = spec.children[i as usize];
                if self.obj_tree_class_is(child, class) {
                    if idx == 0 {
                        return Some(Obj(child));
                    } else {
                        idx -= 1;
                    }
                }
            }
        } else {
            idx += 1; // -1 means the first child
            let mut i = cnt - 1;
            while i >= 0 {
                let child = spec.children[i as usize];
                if self.obj_tree_class_is(child, class) {
                    if idx == 0 {
                        return Some(Obj(child));
                    } else {
                        idx += 1;
                    }
                }
                i -= 1;
            }
        }
        None
    }

    /// `lv_obj_get_sibling()`: the child of `obj`'s parent at
    /// `index(obj) + idx`.
    pub fn obj_get_sibling(&self, obj: Obj, idx: i32) -> Option<Obj> {
        let o = self.obj_tree_obj_checked(obj)?;
        if o.parent.is_none() {
            check_arg_failed("lv_obj_get_sibling: obj->parent != NULL");
            return None;
        }

        let parent = self.obj_get_parent(obj)?;
        let sibling_idx = self.obj_get_index(obj).wrapping_add(idx);
        if sibling_idx < 0 {
            return None;
        }

        self.obj_get_child(parent, sibling_idx)
    }

    /// `lv_obj_get_sibling_by_type()`: like
    /// [`obj_get_sibling`](Self::obj_get_sibling) counting only children of
    /// exactly `class`.
    pub fn obj_get_sibling_by_type(&self, obj: Obj, idx: i32, class: &'static ObjClass) -> Option<Obj> {
        let o = self.obj_tree_obj_checked(obj)?;
        if o.parent.is_none() {
            check_arg_failed("lv_obj_get_sibling_by_type: obj->parent != NULL");
            return None;
        }

        let parent = self.obj_get_parent(obj)?;
        let sibling_idx = self.obj_get_index_by_type(obj, class).wrapping_add(idx);
        if sibling_idx < 0 {
            return None;
        }

        self.obj_get_child_by_type(parent, sibling_idx, class)
    }

    /// `lv_obj_get_child_count()`
    pub fn obj_get_child_count(&self, obj: Obj) -> u32 {
        let Some(o) = self.obj_tree_obj_checked(obj) else {
            return 0;
        };
        match o.spec.as_deref() {
            None => 0,
            Some(spec) => spec.children.len() as u32,
        }
    }

    /// `lv_obj_get_child_count_by_type()`: number of children whose class
    /// is exactly `class`.
    pub fn obj_get_child_count_by_type(&self, obj: Obj, class: &'static ObjClass) -> u32 {
        let Some(o) = self.obj_tree_obj_checked(obj) else {
            return 0;
        };

        let Some(spec) = o.spec.as_deref() else { return 0 };

        let mut cnt: u32 = 0;
        for &child in spec.children.iter() {
            if self.obj_tree_class_is(child, class) {
                cnt += 1;
            }
        }
        cnt
    }

    /// `lv_obj_set_name()`: set a (copied) name; `None` clears it.
    pub fn obj_set_name(&mut self, obj: Obj, name: Option<Rc<str>>) {
        if !self.obj_tree_check_obj(obj) {
            return;
        }

        if !self.obj_tree_allocate_spec_attr(obj) {
            return;
        }
        let Some(spec) = self.obj_mut(obj).and_then(|o| o.spec.as_deref_mut()) else {
            return;
        };

        // `lv_free()` of a non-static name is the drop of the old `Rc`.
        match name {
            None => {
                spec.name = None;
                spec.name_static = true;
            }
            Some(name) => {
                spec.name = Some(name);
                spec.name_static = false;
            }
        }
    }

    /// `lv_obj_set_name_static()`: set a name that is not copied.
    pub fn obj_set_name_static(&mut self, obj: Obj, name: &'static str) {
        if !self.obj_tree_check_obj(obj) {
            return;
        }

        if !self.obj_tree_allocate_spec_attr(obj) {
            return;
        }
        let Some(spec) = self.obj_mut(obj).and_then(|o| o.spec.as_deref_mut()) else {
            return;
        };

        spec.name = Some(Rc::from(name));
        spec.name_static = true;
    }

    /// `lv_obj_get_name()`
    pub fn obj_get_name(&self, obj: Obj) -> Option<Rc<str>> {
        let o = self.obj_tree_obj_checked(obj)?;

        o.spec.as_deref()?.name.clone()
    }

    /// `lv_obj_get_name_resolved()`: the name, or `<class>_<index>`; a name
    /// ending in `#` is auto-indexed among the older siblings with the same
    /// name. Resolved into a [`OBJ_NAME_MAX_LEN`] byte buffer like the
    /// upstream callers do.
    pub fn obj_get_name_resolved(&self, obj: Obj) -> String {
        let buf = self.obj_tree_name_resolved(obj, OBJ_NAME_MAX_LEN);
        String::from_utf8_lossy(&buf).into_owned()
    }

    /// `lv_obj_find_by_name(NULL, name)`: the `parent == NULL` branch of
    /// `lv_obj_find_by_name()`, which searches below
    /// `lv_display_get_screen_active(lv_display_get_default())`.
    pub fn obj_find_by_name_on_active_screen(&self, name: &str) -> Option<Obj> {
        let parent = self.display_get_screen_active(self.display_get_default())?;

        self.obj_find_by_name(parent, name)
    }

    /// `lv_obj_find_by_name()`: recursive search below `parent` (direct
    /// children first, then depth first).
    pub fn obj_find_by_name(&self, parent: Obj, name: &str) -> Option<Obj> {
        // `parent == NULL` is `obj_find_by_name_on_active_screen()`. C does not
        // validate a non-NULL `parent`; a stale handle simply finds nothing.
        if !self.obj_is_valid(parent) {
            return None;
        }

        let child = self.obj_tree_find_by_name_direct(parent, name.as_bytes(), None);
        if child.is_some() {
            return child;
        }

        // Search children recursively
        let child_cnt = self.obj_get_child_count(parent) as usize;
        for i in 0..child_cnt {
            let child = Obj(self.obj_children(parent)[i]);
            let found = self.obj_find_by_name(child, name);
            if found.is_some() {
                return found;
            }
        }

        None
    }

    /// `lv_obj_get_child_by_name()`: `"a/b/c"` path lookup, each segment
    /// matched against the resolved names of the direct children.
    pub fn obj_get_child_by_name(&self, parent: Obj, name_path: &str) -> Option<Obj> {
        let o = self.obj_tree_obj_checked(parent)?;
        if o.spec.is_none() {
            check_arg_failed("lv_obj_get_child_by_name: parent->spec_attr != NULL");
            return None;
        }

        let mut parent = parent;
        let mut path = name_path.as_bytes();
        while !path.is_empty() {
            let segment = path;
            let mut len = 0;

            // Calculate the length of the current segment
            while len < path.len() && path[len] != b'/' {
                len += 1;
            }

            // Look for a child whose resolved name exactly matches the segment
            let child = self.obj_tree_find_by_name_direct(parent, segment, Some(len))?;

            // Advance to the next segment
            path = &path[len..];
            if path.first() == Some(&b'/') {
                path = &path[1..]; // Skip the '/'
            }

            // If there is no further segment, we've found the target child
            if path.is_empty() {
                return Some(child);
            }

            parent = child;
        }

        None
    }

    /// `lv_obj_get_index()`: -1 if `obj` has no parent.
    pub fn obj_get_index(&self, obj: Obj) -> i32 {
        if !self.obj_tree_check_obj(obj) {
            return -1;
        }

        let Some(parent) = self.obj_get_parent(obj) else {
            return -1;
        };

        for (i, &c) in self.obj_children(parent).iter().enumerate() {
            if c == obj.id() {
                return i as i32;
            }
        }

        // Shouldn't reach this point
        -1
    }

    /// `lv_obj_get_index_by_type()`: the index of `obj` among the siblings
    /// whose class is exactly `class` (-1 if not found or no parent).
    pub fn obj_get_index_by_type(&self, obj: Obj, class: &'static ObjClass) -> i32 {
        if !self.obj_tree_check_obj(obj) {
            return -1;
        }

        let Some(parent) = self.obj_get_parent(obj) else {
            return 0xFFFF_FFFFu32 as i32;
        };

        let mut idx: u32 = 0;
        for &child in self.obj_children(parent) {
            if self.obj_tree_class_is(child, class) {
                if child == obj.id() {
                    return idx as i32;
                }
                idx += 1;
            }
        }

        // Can happen if there was no children with the given type
        -1
    }

    /// `lv_obj_tree_walk()`: `None` walks every screen of every display.
    pub fn obj_tree_walk(&mut self, start_obj: Option<Obj>, cb: ObjTreeWalkCallback<'_>) {
        self.obj_tree_walk_core(start_obj, cb);
    }

    /// `lv_obj_dump_tree()` (logs the tree; no-op without the `log` feature).
    pub fn obj_dump_tree(&mut self, start_obj: Option<Obj>) {
        match start_obj {
            None => {
                let mut disp = self.display_get_next(None);
                while let Some(d) = disp {
                    let screens: Vec<ObjId> = self
                        .displays
                        .get(d.0)
                        .map(|d| d.screens.clone())
                        .unwrap_or_default();
                    for scr in screens {
                        self.obj_tree_dump_core(Obj(scr), 0);
                    }
                    disp = self.display_get_next(Some(d));
                }
            }
            Some(start_obj) => self.obj_tree_dump_core(start_obj, 0),
        }
    }

    /// `lv_obj_add_child()` (`lv_obj.c`): append `child` to `parent`'s
    /// children. `parent` gets a `spec_attr` if it has none (C requires the
    /// caller to allocate it first).
    pub fn obj_add_child(&mut self, parent: Obj, child: Obj) -> LvResult {
        if !self.obj_tree_check_obj(parent) {
            return LvResult::Invalid;
        }
        if !self.obj_is_valid(child) {
            check_arg_failed("lv_obj_add_child: child != NULL");
            return LvResult::Invalid;
        }

        if !self.obj_tree_allocate_spec_attr(parent) {
            return LvResult::Invalid;
        }
        let Some(spec) = self.obj_mut(parent).and_then(|p| p.spec.as_deref_mut()) else {
            return LvResult::Invalid;
        };
        spec.children.push(child.id());
        LvResult::Ok
    }

    /// `lv_obj_remove_child()` (`lv_obj.c`): remove `child` from
    /// `parent`'s children (the child's `parent` field is not touched).
    pub fn obj_remove_child(&mut self, parent: Obj, child: Obj) {
        if !self.obj_tree_check_obj(parent) || !self.obj_tree_check_obj(child) {
            return;
        }

        let index = self.obj_get_index(child);
        let Some(spec) = self.obj_mut(parent).and_then(|p| p.spec.as_deref_mut()) else {
            return;
        };
        // C would index `children[-1]` here.
        if index < 0 || index as usize >= spec.children.len() {
            return;
        }

        let child_cnt = spec.children.len();
        let mut i = index as usize;
        while i + 1 < child_cnt {
            spec.children[i] = spec.children[i + 1];
            i += 1;
        }

        // No more children
        if child_cnt == 1 {
            spec.children = Vec::new();
            return;
        }
        spec.children.truncate(child_cnt - 1);
    }

    /// Children of `obj` (empty if it has no `spec_attr`).
    pub fn obj_children(&self, obj: Obj) -> &[ObjId] {
        match self.obj(obj).and_then(|o| o.spec.as_deref()) {
            Some(spec) => &spec.children,
            None => &[],
        }
    }

    // ------------------------------------------------------------------
    // Private helpers (unique `obj_tree_` prefix: `impl Ui` is shared).
    // ------------------------------------------------------------------

    /// `LV_CHECK_OBJ(obj, MY_CLASS, ...)`: the handle must be live.
    fn obj_tree_check_obj(&self, obj: Obj) -> bool {
        if self.obj_is_valid(obj) {
            true
        } else {
            check_arg_failed("LV_CHECK_OBJ: invalid object");
            false
        }
    }

    /// [`obj_tree_check_obj`](Self::obj_tree_check_obj) returning the data.
    fn obj_tree_obj_checked(&self, obj: Obj) -> Option<&crate::core::obj::ObjData> {
        let o = self.obj(obj);
        if o.is_none() {
            check_arg_failed("LV_CHECK_OBJ: invalid object");
        }
        o
    }

    /// `obj->parent` (no check; `None` also for a stale handle).
    fn obj_tree_parent_field(&self, obj: Obj) -> Option<Obj> {
        self.obj(obj).and_then(|o| o.parent).map(Obj)
    }

    /// `child->class_p == class_p`
    fn obj_tree_class_is(&self, child: ObjId, class: &'static ObjClass) -> bool {
        self.objs
            .get(child.0)
            .is_some_and(|c| core::ptr::eq(c.class, class))
    }

    /// `&parent->spec_attr->children[index]`
    fn obj_tree_child_slot(&mut self, parent: Obj, index: i32) -> Option<&mut ObjId> {
        if index < 0 {
            return None;
        }
        self.obj_mut(parent)?
            .spec
            .as_deref_mut()?
            .children
            .get_mut(index as usize)
    }

    /// `lv_obj_allocate_spec_attr()` (`lv_obj.c`): give `obj` a
    /// `spec_attr` with the upstream defaults if it has none.
    pub(crate) fn obj_tree_allocate_spec_attr(&mut self, obj: Obj) -> bool {
        let Some(o) = self.obj_mut(obj) else {
            check_arg_failed("LV_CHECK_OBJ: invalid object");
            return false;
        };
        if o.spec.is_none() {
            o.spec = Some(Box::new(SpecAttr::new()));
        }
        true
    }

    /// `obj_indev_reset()` (static)
    fn obj_tree_indev_reset(&mut self, indev: IndevId, obj: Obj) {
        // If the input device is already in the release state,
        // there is no need to wait for the input device to be released
        if self.indev_get_state(indev) != IndevState::Released {
            // Wait for release to avoid accidentally triggering other obj to be clicked
            self.indev_wait_release(indev);
        }

        // Reset the input device
        self.indev_reset(Some(indev), Some(obj));
    }

    /// `obj_delete_core()` (static)
    fn obj_tree_delete_core(&mut self, obj: Obj) {
        let Some(o) = self.obj_mut(obj) else { return };
        if o.is_deleting {
            return;
        }

        o.is_deleting = true;

        // Let the user free the resources used in `LV_EVENT_DELETE`
        let res = self.obj_send_event(obj, EventCode::DELETE, EventParam::None);
        if res == LvResult::Invalid {
            if let Some(o) = self.obj_mut(obj) {
                o.is_deleting = false;
            }
            return;
        }

        // Release the blur bookkeeping now: the style teardown below doesn't go
        // through lv_obj_update_blur_status()
        if let Some(o) = self.obj_mut(obj)
            && o.has_blur
        {
            o.has_blur = false;
            debug_assert!(self.blur_obj_cnt > 0);
            self.blur_obj_cnt = self.blur_obj_cnt.wrapping_sub(1);
        }

        // Clean registered event_cb
        if let Some(spec) = self.obj_mut(obj).and_then(|o| o.spec.as_deref_mut()) {
            let mut list = core::mem::take(&mut spec.event_list);
            self.event_remove_all(&mut list);
            if let Some(spec) = self.obj_mut(obj).and_then(|o| o.spec.as_deref_mut()) {
                spec.event_list = list;
            }
        }

        // Recursively delete the children
        let mut child = self.obj_get_child(obj, 0);
        while let Some(c) = child {
            self.obj_tree_delete_core(c);
            child = self.obj_get_child(obj, 0);
        }

        let group = self.obj_get_group(obj);

        // Reset all input devices if the object to delete is used
        let mut indev = self.indev_get_next(None);
        while let Some(i) = indev {
            let indev_type = self.indev_get_type(i);
            if indev_type == IndevType::Pointer || indev_type == IndevType::Button {
                let (act_obj, scroll_obj) = self
                    .indevs
                    .get(i.0)
                    .map(|d| (d.pointer.act_obj, d.pointer.scroll_obj))
                    .unwrap_or((None, None));
                if act_obj == Some(obj) || scroll_obj == Some(obj) {
                    self.obj_tree_indev_reset(i, obj);
                }
                if let Some(d) = self.indevs.get_mut(i.0) {
                    if d.pointer.last_pressed == Some(obj) {
                        d.pointer.last_pressed = None;
                    }
                    if d.pointer.last_hovered == Some(obj) {
                        d.pointer.last_hovered = None;
                    }
                }
            }

            let indev_group = self.indevs.get(i.0).and_then(|d| d.group);
            if indev_group == group && Some(obj) == self.indev_get_active_obj() {
                self.obj_tree_indev_reset(i, obj);
            }
            indev = self.indev_get_next(Some(i));
        }

        // Delete all pending async del-s
        // (`lv_async_call_cancel(lv_obj_delete_async_cb, obj)` until it fails)
        let mut async_cancel_res = LvResult::Ok;
        while async_cancel_res == LvResult::Ok {
            async_cancel_res = self.obj_tree_async_delete_cancel(obj);
        }

        // All children deleted. Now clean up the object specific data
        self.obj_destruct(obj);

        // Remove the screen for the screen list
        match self.obj_tree_parent_field(obj) {
            None => {
                if let Some(disp) = self.obj_get_display(obj)
                    && let Some(d) = self.displays.get_mut(disp.0)
                    && let Some(id) = d.screens.iter().position(|&s| s == obj.id())
                {
                    // Find the screen in the list and shift the rest down
                    d.screens.remove(id);
                }
            }
            // Remove the object from the child list of its parent
            Some(parent) => self.obj_remove_child(parent, obj),
        }

        if let Some(o) = self.obj_mut(obj) {
            o.parent = None;
        }

        // Free the object itself
        self.objs.remove(obj.id().0);
    }

    /// `lv_async_call_cancel(lv_obj_delete_async_cb, obj)`: remove all
    /// pending deletion timers for `obj`, preserving other async jobs.
    fn obj_tree_async_delete_cancel(&mut self, obj: Obj) -> LvResult {
        self.async_call_job_cancel(&AsyncJob::DeleteObj(obj))
    }

    /// `walk_core()` (static)
    fn obj_tree_walk_core(
        &mut self,
        obj: Option<Obj>,
        cb: &mut dyn FnMut(&mut Ui, Obj) -> ObjTreeWalkRes,
    ) -> ObjTreeWalkRes {
        let Some(obj) = obj else {
            let mut disp = self.display_get_next(None);
            while let Some(d) = disp {
                let mut i = 0;
                while let Some(scr) = self.displays.get(d.0).and_then(|d| d.screens.get(i).copied()) {
                    self.obj_tree_walk_core(Some(Obj(scr)), cb);
                    i += 1;
                }
                disp = self.display_get_next(Some(d));
            }
            return ObjTreeWalkRes::End; // The value doesn't matter as it wasn't called recursively
        };

        let res = cb(self, obj);

        if res == ObjTreeWalkRes::End {
            return ObjTreeWalkRes::End;
        }

        if res != ObjTreeWalkRes::SkipChildren {
            let mut i: u32 = 0;
            // A callback deleting `obj` is a use-after-free in C; stop here instead.
            while self.obj_is_valid(obj) && i < self.obj_get_child_count(obj) {
                if let Some(child) = self.obj_get_child(obj, i as i32) {
                    let res = self.obj_tree_walk_core(Some(child), cb);
                    if res == ObjTreeWalkRes::End {
                        return ObjTreeWalkRes::End;
                    }
                }
                i += 1;
            }
        }
        ObjTreeWalkRes::Next
    }

    /// `dump_tree_core()` (static)
    fn obj_tree_dump_core(&mut self, obj: Obj, depth: i32) {
        if !self.obj_is_valid(obj) {
            return;
        }
        #[cfg(feature = "log")]
        {
            // Known deviation: LV_USE_OBJ_ID=y in the parity config, but
            // `ObjData` has no `id` field and `lv_obj_stringify_id()` is not
            // ported, so this is the `lv_obj_stringify_id() == NULL` fallback.
            let id = "obj0";
            // id of `obj0` is an invalid id for builtin id
            // LV_LOG_USER: the highest LVGL level, printed whenever logging is
            // on, so it maps to the least filtered `log` level.
            log::error!(
                "{:width$}obj:{:?}, id:{};",
                "",
                obj.id(),
                id,
                width = (2 * depth) as usize
            );
        }
        #[cfg(not(feature = "log"))]
        let _ = (depth, "LV_USE_LOG == 0: nothing is printed");

        let child_cnt = self.obj_get_child_count(obj);
        for i in 0..child_cnt {
            if let Some(child) = self.obj_get_child(obj, i as i32) {
                self.obj_tree_dump_core(child, depth + 1);
            }
        }
    }

    /// `lv_obj_get_first_not_deleting_child()` (static)
    fn obj_tree_first_not_deleting_child(&self, obj: Obj) -> Option<Obj> {
        let spec = self.obj(obj)?.spec.as_deref()?;

        for &c in spec.children.iter() {
            if self.objs.get(c.0).is_some_and(|c| !c.is_deleting) {
                return Some(Obj(c));
            }
        }

        None
    }

    /// `find_by_name_direct()` (static): `len == None` is `UINT16_MAX`
    /// (compare the whole `name`), otherwise only `name[..len]` is matched.
    fn obj_tree_find_by_name_direct(&self, parent: Obj, name: &[u8], len: Option<usize>) -> Option<Obj> {
        let child_cnt = self.obj_get_child_count(parent) as usize;
        for i in 0..child_cnt {
            let child = Obj(self.obj_children(parent)[i]);

            let child_name_resolved = self.obj_tree_name_resolved(child, OBJ_NAME_MAX_LEN);

            match len {
                None => {
                    if c_str(&child_name_resolved) == c_str(name) {
                        return Some(child);
                    }
                }
                Some(len) => {
                    // strncmp(resolved, name, len) == 0 && resolved[len] == '\0'
                    let seg = c_str(&name[..len.min(name.len())]);
                    if c_str(&child_name_resolved) == seg && seg.len() == len {
                        return Some(child);
                    }
                }
            }
        }

        None
    }

    /// Body of `lv_obj_get_name_resolved()` writing into a `buf_size` byte
    /// buffer (the result excludes the terminating `NUL`).
    fn obj_tree_name_resolved(&self, obj: Obj, buf_size: usize) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::new();
        let Some(o) = self.obj_tree_obj_checked(obj) else {
            return buf;
        };
        if buf_size == 0 {
            check_arg_failed("lv_obj_get_name_resolved: buf_size > 0");
            return buf;
        }

        let spec_name: Vec<u8> = o
            .spec
            .as_deref()
            .and_then(|s| s.name.as_deref())
            .map(|n| c_str(n.as_bytes()).to_vec())
            .unwrap_or_default();

        let name_opt = self.obj_get_name(obj);
        // Use a default name which auto-indexing
        let name: Vec<u8> = match name_opt {
            None => {
                let mut n = alloc::format!("{}_#", o.class.name).into_bytes();
                n.truncate(OBJ_NAME_MAX_LEN - 1); // snprintf into name_buf[LV_OBJ_NAME_MAX_LEN]
                n
            }
            Some(n) => c_str(n.as_bytes()).to_vec(),
        };

        let name_len = name.len();
        let parent = self.obj_get_parent(obj);

        // If the last character is # automatically index the children with the same name start
        if let Some(parent) = parent
            && name_len > 0
            && name[name_len - 1] == b'#'
        {
            let child_cnt = self.obj_get_child_count(parent);
            let mut cnt: u32 = 0;
            for i in 0..child_cnt {
                let Some(child) = self.obj_get_child(parent, i as i32) else {
                    break;
                };
                // All siblings older siblings are checked, craft the name of this widget
                if child == obj {
                    let num = alloc::format!("{}", cnt as i32).into_bytes();
                    let num_len = num.len(); // snprintf returns the untruncated length
                    // Is there enough space for the name and the index?
                    if buf_size > name_len + num_len {
                        // E.g. buf = "some_name_", so trim the # from the end
                        buf.extend_from_slice(&name[..name_len - 1]);
                        buf.extend_from_slice(&num[..num.len().min(7)]); // char num_buf[8]
                    } else {
                        // Use the name as it is as a fallback
                        strlcpy(&mut buf, &spec_name, buf_size);
                    }
                    break;
                }
                // Check the older siblings. IF they start with the same name count them
                else {
                    let child_name = self.obj_get_name(child);
                    match child_name {
                        None => {
                            // If the name we are looking for start with the child's class name
                            // increment the index. E.g. <class_name>_#
                            let class_name = self.obj(child).map_or("", |c| c.class.name).as_bytes();
                            let class_name_len = class_name.len();
                            if name_len > 3 && class_name_len == name_len - 2 && name.starts_with(class_name)
                            {
                                cnt += 1;
                            }
                        }
                        // The name is set, check if it's e.g. <some_name>#
                        Some(child_name) => {
                            if c_str(child_name.as_bytes()) == name.as_slice() {
                                cnt += 1;
                            }
                        }
                    }
                }
            }
        } else {
            // Just use the set name
            strlcpy(&mut buf, &spec_name, buf_size);
        }

        buf
    }
}

/// The bytes of a C string: everything before the first `NUL`.
fn c_str(s: &[u8]) -> &[u8] {
    match s.iter().position(|&b| b == 0) {
        Some(p) => &s[..p],
        None => s,
    }
}

/// `lv_strlcpy(dst, src, dst_size)`: copy at most `dst_size - 1` bytes.
fn strlcpy(dst: &mut Vec<u8>, src: &[u8], dst_size: usize) {
    dst.clear();
    if dst_size == 0 {
        return;
    }
    let n = src.len().min(dst_size - 1);
    dst.extend_from_slice(&src[..n]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::UiConfig;
    use crate::core::obj::OBJ_CLASS;
    use crate::core::obj_class::tests::{TEST_A, TEST_B, raw_obj};
    use alloc::vec;

    fn ids(v: &[Obj]) -> Vec<ObjId> {
        v.iter().map(|o| o.id()).collect()
    }

    /// scr -> [a, b, c]; a -> [a0, a1]
    fn tree(ui: &mut Ui) -> (Obj, Obj, Obj, Obj, Obj, Obj) {
        let scr = raw_obj(ui, &OBJ_CLASS, None);
        let a = raw_obj(ui, &OBJ_CLASS, Some(scr));
        let b = raw_obj(ui, &TEST_A, Some(scr));
        let c = raw_obj(ui, &OBJ_CLASS, Some(scr));
        let a0 = raw_obj(ui, &TEST_B, Some(a));
        let a1 = raw_obj(ui, &OBJ_CLASS, Some(a));
        (scr, a, b, c, a0, a1)
    }

    #[test]
    fn get_child_positive_and_negative() {
        let mut ui = Ui::new(UiConfig::default());
        let (scr, a, b, c, _, _) = tree(&mut ui);
        assert_eq!(ui.obj_get_child(scr, 0), Some(a));
        assert_eq!(ui.obj_get_child(scr, 2), Some(c));
        assert_eq!(ui.obj_get_child(scr, 3), None);
        assert_eq!(ui.obj_get_child(scr, -1), Some(c));
        assert_eq!(ui.obj_get_child(scr, -3), Some(a));
        assert_eq!(ui.obj_get_child(scr, -4), None);
        assert_eq!(ui.obj_get_child(b, 0), None); // no spec_attr
        assert_eq!(ui.obj_get_child_count(scr), 3);
        assert_eq!(ui.obj_get_child_count(b), 0);
        assert_eq!(ui.obj_children(scr), ids(&[a, b, c]).as_slice());
    }

    #[test]
    fn parent_screen_index() {
        let mut ui = Ui::new(UiConfig::default());
        let (scr, a, b, c, a0, a1) = tree(&mut ui);
        assert_eq!(ui.obj_get_parent(scr), None);
        assert_eq!(ui.obj_get_parent(a1), Some(a));
        assert_eq!(ui.obj_get_screen(a1), scr);
        assert_eq!(ui.obj_get_screen(scr), scr);
        assert_eq!(ui.obj_get_index(scr), -1);
        assert_eq!(ui.obj_get_index(a), 0);
        assert_eq!(ui.obj_get_index(b), 1);
        assert_eq!(ui.obj_get_index(c), 2);
        assert_eq!(ui.obj_get_index(a0), 0);
        assert_eq!(ui.obj_get_index(a1), 1);
    }

    #[test]
    fn siblings() {
        let mut ui = Ui::new(UiConfig::default());
        let (_, a, b, c, _, _) = tree(&mut ui);
        assert_eq!(ui.obj_get_sibling(b, -1), Some(a));
        assert_eq!(ui.obj_get_sibling(b, 1), Some(c));
        assert_eq!(ui.obj_get_sibling(b, 0), Some(b));
        assert_eq!(ui.obj_get_sibling(a, -1), None);
        assert_eq!(ui.obj_get_sibling(c, 1), None);
    }

    #[test]
    fn by_type_queries() {
        // Ported from test_obj_tree.c `test_obj_tree_get_child_by_type` /
        // `test_obj_tree_get_sibling_by_type` (with test classes).
        let mut ui = Ui::new(UiConfig::default());
        let scr = raw_obj(&mut ui, &OBJ_CLASS, None);
        let o0 = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        let x0 = raw_obj(&mut ui, &TEST_A, Some(scr));
        let o1 = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        let x1 = raw_obj(&mut ui, &TEST_A, Some(scr));
        let y0 = raw_obj(&mut ui, &TEST_B, Some(scr));

        assert_eq!(ui.obj_get_child_by_type(scr, 0, &TEST_A), Some(x0));
        assert_eq!(ui.obj_get_child_by_type(scr, 1, &TEST_A), Some(x1));
        assert_eq!(ui.obj_get_child_by_type(scr, 2, &TEST_A), None);
        assert_eq!(ui.obj_get_child_by_type(scr, -1, &TEST_A), Some(x1));
        assert_eq!(ui.obj_get_child_by_type(scr, -2, &TEST_A), Some(x0));
        assert_eq!(ui.obj_get_child_by_type(scr, -3, &TEST_A), None);
        // Exact class match only: TEST_B derives from TEST_A but isn't counted.
        assert_eq!(ui.obj_get_child_count_by_type(scr, &TEST_A), 2);
        assert_eq!(ui.obj_get_child_count_by_type(scr, &TEST_B), 1);
        assert_eq!(ui.obj_get_child_count_by_type(scr, &OBJ_CLASS), 2);

        assert_eq!(ui.obj_get_index_by_type(x1, &TEST_A), 1);
        assert_eq!(ui.obj_get_index_by_type(o1, &OBJ_CLASS), 1);
        assert_eq!(ui.obj_get_index_by_type(y0, &TEST_A), -1);
        assert_eq!(ui.obj_get_index_by_type(scr, &TEST_A), -1);

        assert_eq!(ui.obj_get_sibling_by_type(x0, 1, &TEST_A), Some(x1));
        assert_eq!(ui.obj_get_sibling_by_type(x1, -1, &TEST_A), Some(x0));
        assert_eq!(ui.obj_get_sibling_by_type(x0, -1, &TEST_A), None);
        assert_eq!(ui.obj_get_sibling_by_type(o0, 1, &OBJ_CLASS), Some(o1));
    }

    #[test]
    fn add_and_remove_child() {
        let mut ui = Ui::new(UiConfig::default());
        let (scr, a, b, c, _, _) = tree(&mut ui);
        ui.obj_remove_child(scr, b);
        assert_eq!(ui.obj_children(scr), ids(&[a, c]).as_slice());
        // remove_child doesn't touch child->parent
        assert_eq!(ui.obj_get_parent(b), Some(scr));
        ui.obj_remove_child(scr, a);
        assert_eq!(ui.obj_children(scr), ids(&[c]).as_slice());
        ui.obj_remove_child(scr, c);
        assert!(ui.obj_children(scr).is_empty());
        assert_eq!(ui.obj_get_child_count(scr), 0);
        assert_eq!(ui.obj_add_child(scr, b), LvResult::Ok);
        assert_eq!(ui.obj_children(scr), ids(&[b]).as_slice());
    }

    #[test]
    fn tree_walk_order_and_control() {
        let mut ui = Ui::new(UiConfig::default());
        let (scr, a, b, c, a0, a1) = tree(&mut ui);

        let mut seen = Vec::new();
        ui.obj_tree_walk(Some(scr), &mut |_, o| {
            seen.push(o);
            ObjTreeWalkRes::Next
        });
        assert_eq!(seen, vec![scr, a, a0, a1, b, c]);

        let mut seen = Vec::new();
        ui.obj_tree_walk(Some(scr), &mut |_, o| {
            seen.push(o);
            if o == a {
                ObjTreeWalkRes::SkipChildren
            } else {
                ObjTreeWalkRes::Next
            }
        });
        assert_eq!(seen, vec![scr, a, b, c]);

        let mut seen = Vec::new();
        ui.obj_tree_walk(Some(scr), &mut |_, o| {
            seen.push(o);
            if o == a0 {
                ObjTreeWalkRes::End
            } else {
                ObjTreeWalkRes::Next
            }
        });
        assert_eq!(seen, vec![scr, a, a0]);
    }

    #[test]
    fn names_resolve_with_auto_index() {
        // Ported from test_obj_name.c (`test_obj_name_auto_index` style cases).
        let mut ui = Ui::new(UiConfig::default());
        let scr = raw_obj(&mut ui, &OBJ_CLASS, None);
        let o0 = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        let x0 = raw_obj(&mut ui, &TEST_A, Some(scr));
        let o1 = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        let n0 = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        let n1 = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        let fixed = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        ui.obj_set_name_static(n0, "btn_#");
        ui.obj_set_name(n1, Some(Rc::from("btn_#")));
        ui.obj_set_name(fixed, Some(Rc::from("fixed")));

        assert_eq!(ui.obj_get_name_resolved(o0), "lv_obj_0");
        assert_eq!(ui.obj_get_name_resolved(x0), "a_0");
        assert_eq!(ui.obj_get_name_resolved(o1), "lv_obj_1");
        assert_eq!(ui.obj_get_name_resolved(n0), "btn_0");
        assert_eq!(ui.obj_get_name_resolved(n1), "btn_1");
        assert_eq!(ui.obj_get_name_resolved(fixed), "fixed");
        assert_eq!(ui.obj_get_name(fixed).as_deref(), Some("fixed"));
        assert_eq!(ui.obj_get_name(o0), None);
        assert!(ui.obj(n0).unwrap().spec.as_ref().unwrap().name_static);
        assert!(!ui.obj(n1).unwrap().spec.as_ref().unwrap().name_static);

        // Clearing marks the name static like upstream.
        ui.obj_set_name(fixed, None);
        assert_eq!(ui.obj_get_name(fixed), None);
        assert!(ui.obj(fixed).unwrap().spec.as_ref().unwrap().name_static);
        assert_eq!(ui.obj_get_name_resolved(fixed), "lv_obj_2");
    }

    #[test]
    fn find_and_path_lookup() {
        let mut ui = Ui::new(UiConfig::default());
        let scr = raw_obj(&mut ui, &OBJ_CLASS, None);
        let cont = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        let other = raw_obj(&mut ui, &OBJ_CLASS, Some(scr));
        let btn = raw_obj(&mut ui, &OBJ_CLASS, Some(cont));
        let label = raw_obj(&mut ui, &TEST_A, Some(btn));
        ui.obj_set_name_static(cont, "cont");
        ui.obj_set_name_static(other, "other");
        ui.obj_set_name_static(btn, "btn");
        ui.obj_set_name_static(label, "label");

        assert_eq!(ui.obj_find_by_name(scr, "label"), Some(label));
        assert_eq!(ui.obj_find_by_name(scr, "other"), Some(other));
        assert_eq!(ui.obj_find_by_name(scr, "nope"), None);
        assert_eq!(ui.obj_get_child_by_name(scr, "cont/btn/label"), Some(label));
        assert_eq!(ui.obj_get_child_by_name(scr, "cont/btn"), Some(btn));
        assert_eq!(ui.obj_get_child_by_name(scr, "cont/btn/"), Some(btn));
        assert_eq!(ui.obj_get_child_by_name(scr, "cont/bt"), None);
        assert_eq!(ui.obj_get_child_by_name(scr, "label"), None);
        assert_eq!(ui.obj_get_child_by_name(scr, ""), None);
        // Default (auto-indexed) names can be used in paths too.
        assert_eq!(ui.obj_get_child_by_name(btn, "label"), Some(label));
        ui.obj_set_name(label, None);
        assert_eq!(ui.obj_get_child_by_name(scr, "cont/btn/a_0"), Some(label));
    }

    #[test]
    fn first_not_deleting_child_skips_deleting() {
        let mut ui = Ui::new(UiConfig::default());
        let (scr, a, b, _, _, _) = tree(&mut ui);
        assert_eq!(ui.obj_tree_first_not_deleting_child(scr), Some(a));
        ui.obj_mut(a).unwrap().is_deleting = true;
        assert_eq!(ui.obj_tree_first_not_deleting_child(scr), Some(b));
        assert_eq!(ui.obj_tree_first_not_deleting_child(b), None);
    }

    #[test]
    fn delete_async_uses_timers_and_cancel_removes_all_matches() {
        let mut ui = Ui::new(UiConfig::default());
        let (_, a, b, _, _, _) = tree(&mut ui);
        let original_timers = ui.timers.order.clone();
        ui.obj_delete_async(a);
        let first_a = ui.timer_get_next(None).unwrap();
        ui.obj_delete_async(b);
        let only_b = ui.timer_get_next(None).unwrap();
        ui.obj_delete_async(a);
        let second_a = ui.timer_get_next(None).unwrap();
        assert_eq!(ui.timers.order.len(), original_timers.len() + 3);
        assert!(ui.obj_is_valid(a));
        assert!(ui.obj_is_valid(b));
        assert_eq!(ui.timer(first_a).unwrap().period, 0);
        assert_eq!(ui.timer(first_a).unwrap().repeat_count, 1);

        assert_eq!(ui.obj_tree_async_delete_cancel(a), LvResult::Ok);
        assert_eq!(ui.obj_tree_async_delete_cancel(a), LvResult::Invalid);
        assert!(ui.timer(first_a).is_none());
        assert!(ui.timer(second_a).is_none());
        assert_eq!(ui.timer_get_next(None), Some(only_b));
        assert_eq!(&ui.timers.order[1..], original_timers.as_slice());
        assert_eq!(ui.obj_tree_async_delete_cancel(b), LvResult::Ok);
        assert_eq!(ui.timers.order, original_timers);
    }

    #[test]
    fn delete_delayed_starts_a_1ms_anim_on_the_object() {
        let mut ui = Ui::new(UiConfig::default());
        let (_, a, _, _, _, _) = tree(&mut ui);
        ui.obj_delete_delayed(a, 300);
        let id = ui.anims.order[0];
        let anim = ui.anim(id).unwrap();
        assert_eq!(anim.var, AnimTarget::Obj(a));
        assert!(matches!(anim.exec, AnimExec::None));
        assert_eq!(anim.duration, 1);
        assert_eq!(anim.act_time, -300);
        assert!(anim.completed_cb.is_some());
    }

    #[test]
    fn allocate_spec_attr_defaults() {
        let mut ui = Ui::new(UiConfig::default());
        let o = raw_obj(&mut ui, &OBJ_CLASS, None);
        assert!(ui.obj(o).unwrap().spec.is_none());
        assert!(ui.obj_tree_allocate_spec_attr(o));
        let spec = ui.obj(o).unwrap().spec.as_deref().unwrap();
        assert_eq!(spec.scroll_dir, crate::misc::area::Dir::ALL);
        assert_eq!(spec.scrollbar_mode, crate::core::obj::ScrollbarMode::Auto);
        assert!(spec.children.is_empty());
    }

    #[test]
    fn c_string_helpers() {
        let mut v = Vec::new();
        strlcpy(&mut v, b"hello", 4);
        assert_eq!(v, b"hel");
        strlcpy(&mut v, b"hi", 128);
        assert_eq!(v, b"hi");
        assert_eq!(c_str(b"ab\0cd"), b"ab");
    }
}
