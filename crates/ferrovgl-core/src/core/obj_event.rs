//! Ported from lvgl v9.6.0: `src/core/lv_obj_event.c`
//! (also `include/lvgl/core/lv_obj_event.h`, `src/core/lv_obj_event_private.h`).
//!
//! Object event dispatch: preprocess callbacks, the class `event_cb` chain,
//! user callbacks, bubbling and trickling.
//!
//! The `lv_event_get_*()` parameter accessors of `lv_obj_event.c`
//! (`lv_event_get_indev()`, `lv_event_get_layer()`, `lv_event_set_ext_draw_size()`,
//! `lv_event_set_cover_res()`, ...) are methods of
//! [`Event`] in `misc/event.rs`.

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::{Obj, ObjClass, ObjFlags, SpecAttr};
use crate::misc::check_arg_failed;
use crate::misc::event::{
    Event, EventCallback, EventCode, EventDsc, EventDscId, EventFilter, EventList, EventParam, EventTarget,
    LvResult,
};

impl Ui {
    /// `lv_obj_send_event()`: send an event to `obj`, running its preprocess
    /// callbacks, its class event handlers, its normal callbacks and then
    /// bubbling / trickling the event if enabled.
    ///
    /// Returns [`LvResult::Invalid`] if `obj` was deleted by the event.
    pub fn obj_send_event(&mut self, obj: Obj, code: EventCode, param: EventParam<'_>) -> LvResult {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_send_event: obj is valid");
            return LvResult::Invalid;
        }

        let mut e = Event {
            current_target: EventTarget::Obj(obj),
            target: EventTarget::Obj(obj),
            code,
            user_data: None,
            ext_data: None,
            param,
            deleted: false,
            stop_bubbling: false,
            stop_processing: false,
            stop_trickling: false,
            frame: None,
        };

        self.event_push(&mut e);

        /*Send the event*/
        let res = self.obj_event_send_core(&mut e);

        /*Remove this element from the list*/
        self.event_pop(&mut e);

        res
    }

    /// `lv_obj_event_base()`: call the event handler of the closest ancestor
    /// of `class` (`class->base_class` upwards) that has an `event_cb`.
    ///
    /// Widgets call it as `ui.obj_event_base(&MY_CLASS, e)` at the start of
    /// their own `event_cb`, exactly like `lv_obj_event_base(MY_CLASS, e)`.
    /// (The C `class_p == NULL` form, used only by the dispatcher, is the
    /// crate-private `obj_event_base_core(None, e)`.)
    pub fn obj_event_base(&mut self, class: &'static ObjClass, e: &mut Event<'_>) -> LvResult {
        self.obj_event_base_core(Some(class), e)
    }

    /// `lv_obj_event_base()` with a nullable `class_p`: `None` starts from the
    /// class of `e.current_target` itself.
    pub(crate) fn obj_event_base_core(
        &mut self,
        class: Option<&'static ObjClass>,
        e: &mut Event<'_>,
    ) -> LvResult {
        let Some(current) = e.current_target.obj() else {
            check_arg_failed("lv_obj_event_base: e->current_target != NULL");
            return LvResult::Invalid;
        };

        let mut base: Option<&'static ObjClass> = match class {
            None => self.obj(current).map(|o| o.class),
            Some(c) => c.base,
        };

        /*Find a base in which call the ancestor's event handler_cb if set*/
        while let Some(b) = base
            && b.event_cb.is_none()
        {
            base = b.base;
        }

        let Some(base) = base else {
            return LvResult::Ok;
        };
        let Some(event_cb) = base.event_cb else {
            return LvResult::Ok;
        };

        /*Call the actual event callback*/
        e.user_data = None;
        self.obj_event_frame_sync(e);
        event_cb(base, self, e);
        self.obj_event_frame_sync(e);

        let mut res = LvResult::Ok;
        /*Stop if the object is deleted*/
        if e.deleted {
            res = LvResult::Invalid;
        }

        res
    }

    /// `lv_obj_add_event_cb()`: register `cb` for the events matching
    /// `filter` (`EventCode::ALL` for every event, `preprocess` to run before
    /// the class handlers).
    ///
    /// Upstream returns `NULL` for an invalid object; here a stale `obj`
    /// reports the failed check and returns a dangling id that matches no
    /// list.
    pub fn obj_add_event_cb(
        &mut self,
        obj: Obj,
        cb: EventCallback,
        filter: impl Into<EventFilter>,
        user_data: Option<Rc<dyn Any>>,
    ) -> EventDscId {
        let filter = filter.into();
        let Some(mut list) = self.obj_take_event_list(obj) else {
            check_arg_failed("lv_obj_add_event_cb: obj is valid");
            // `return NULL`: an id whose descriptor is already freed.
            let mut tmp = EventList::new();
            let id = self.event_add(&mut tmp, cb, filter, user_data);
            self.event_remove_all(&mut tmp);
            return id;
        };
        let id = self.event_add(&mut list, cb, filter, user_data);
        self.obj_put_event_list(obj, list);
        id
    }

    /// `lv_obj_get_event_count()`
    pub fn obj_get_event_count(&self, obj: Obj) -> u32 {
        let Some(spec) = self.obj(obj).and_then(|o| o.spec.as_deref()) else {
            return 0;
        };
        spec.event_list.len() as u32
    }

    /// `lv_obj_get_event_dsc()`
    pub fn obj_get_event_dsc(&self, obj: Obj, index: u32) -> Option<&EventDsc> {
        let spec = self.obj(obj).and_then(|o| o.spec.as_deref())?;
        self.event_get_dsc(&spec.event_list, index as usize)
    }

    /// `lv_obj_remove_event()`
    pub fn obj_remove_event(&mut self, obj: Obj, index: u32) -> bool {
        if self.obj(obj).and_then(|o| o.spec.as_deref()).is_none() {
            return false;
        }
        let Some(mut list) = self.obj_take_event_list(obj) else {
            return false;
        };
        let res = self.event_remove(&mut list, index as usize);
        self.obj_put_event_list(obj, list);
        res
    }

    /// `lv_obj_remove_event_dsc()`
    pub fn obj_remove_event_dsc(&mut self, obj: Obj, dsc: EventDscId) -> bool {
        if self.obj(obj).and_then(|o| o.spec.as_deref()).is_none() {
            return false;
        }
        let Some(mut list) = self.obj_take_event_list(obj) else {
            return false;
        };
        let res = self.event_remove_dsc(&mut list, dsc);
        self.obj_put_event_list(obj, list);
        res
    }

    /// `lv_obj_remove_event_cb()`: remove every descriptor whose callback is
    /// `cb` (compared by `Rc::ptr_eq`). Returns the number removed.
    ///
    /// The C `event_cb == NULL` wildcard is [`Ui::obj_remove_event_cb_opt`].
    pub fn obj_remove_event_cb(&mut self, obj: Obj, cb: &EventCallback) -> u32 {
        self.obj_remove_event_cb_opt(obj, Some(cb))
    }

    /// `lv_obj_remove_event_cb()` with a nullable `event_cb`: `None` (C
    /// `NULL`) matches every descriptor, so all of them are removed.
    /// Returns the number removed.
    pub fn obj_remove_event_cb_opt(&mut self, obj: Obj, cb: Option<&EventCallback>) -> u32 {
        self.obj_remove_event_matching(obj, |dsc| cb.is_none_or(|cb| Rc::ptr_eq(&dsc.cb, cb)))
    }

    /// `lv_obj_remove_event_cb_with_user_data()`: remove every descriptor
    /// whose callback is `cb` and whose `user_data` is `user_data` (both
    /// compared by `Rc::ptr_eq`). Returns the number removed.
    ///
    /// The C forms with a `NULL` `event_cb` or a `NULL` `user_data` are
    /// [`Ui::obj_remove_event_cb_with_user_data_opt`].
    pub fn obj_remove_event_cb_with_user_data(
        &mut self,
        obj: Obj,
        cb: &EventCallback,
        user_data: &Rc<dyn Any>,
    ) -> u32 {
        self.obj_remove_event_cb_with_user_data_opt(obj, Some(cb), Some(user_data))
    }

    /// `lv_obj_remove_event_cb_with_user_data()` with nullable arguments:
    /// `cb == None` matches any callback (C `event_cb == NULL`), and
    /// `user_data` must equal the descriptor's `user_data` exactly, so
    /// `None` matches only descriptors registered with `None` (C
    /// `dsc->user_data == user_data` with both `NULL`). Returns the number
    /// removed.
    pub fn obj_remove_event_cb_with_user_data_opt(
        &mut self,
        obj: Obj,
        cb: Option<&EventCallback>,
        user_data: Option<&Rc<dyn Any>>,
    ) -> u32 {
        self.obj_remove_event_matching(obj, |dsc| {
            cb.is_none_or(|cb| Rc::ptr_eq(&dsc.cb, cb))
                && match (dsc.user_data.as_ref(), user_data) {
                    (None, None) => true,
                    (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                    _ => false,
                }
        })
    }

    /// The loop shared by `lv_obj_remove_event_cb()` and
    /// `lv_obj_remove_event_cb_with_user_data()`: walk the descriptors from
    /// the last to the first and remove the matching ones.
    pub(crate) fn obj_remove_event_matching(&mut self, obj: Obj, matches: impl Fn(&EventDsc) -> bool) -> u32 {
        let event_cnt = self.obj_get_event_count(obj);
        let mut removed_count: u32 = 0;

        if event_cnt == 0 {
            return 0;
        }

        let mut i = event_cnt as i32 - 1;
        while i >= 0 {
            let hit = self.obj_get_event_dsc(obj, i as u32).is_some_and(&matches);
            if hit {
                self.obj_remove_event(obj, i as u32);
                removed_count += 1;
            }
            i -= 1;
        }

        removed_count
    }

    /// `event_send_core()`: one step of the dispatch on `e.current_target`,
    /// recursing into the parent (bubbling) and the children (trickling).
    fn obj_event_send_core(&mut self, e: &mut Event<'_>) -> LvResult {
        #[cfg(feature = "log")]
        log::trace!(
            "Sending event {} to {:?} with {:?} param",
            e.code.0,
            e.target,
            e.param
        );

        self.obj_event_frame_sync(e);
        let indev_act = self.indev_active();
        if indev_act.is_some() {
            if e.stop_processing {
                return LvResult::Ok;
            }
            if e.deleted {
                return LvResult::Invalid;
            }
        }

        let Some(target) = e.current_target.obj() else {
            return LvResult::Invalid;
        };
        let mut res: LvResult;
        // `list` is `NULL` when the target has no special attributes; like in
        // C it is decided once, before any callback runs.
        let has_list = self.obj(target).is_some_and(|o| o.spec.is_some());

        res = self.obj_event_send_list(has_list, target, e, true);
        if res != LvResult::Ok || e.stop_processing {
            return res;
        }

        res = self.obj_event_base_core(None, e);
        if res != LvResult::Ok || e.stop_processing {
            return res;
        }

        res = self.obj_event_send_list(has_list, target, e, false);
        if res != LvResult::Ok || e.stop_processing {
            return res;
        }

        let parent = e.current_target.obj().and_then(|o| self.obj_event_parent(o));
        if let Some(parent) = parent
            && self.obj_event_is_bubbled(e)
        {
            e.current_target = EventTarget::Obj(parent);
            res = self.obj_event_send_core(e);
        }
        if res != LvResult::Ok {
            return res;
        }

        /*Trickle down to children if enabled*/
        if self.obj_event_is_trickled(e) {
            let child_count = self.obj_event_child_count(target);

            /* we don't want the event to bubble up again when trickling down */
            e.stop_bubbling = true;

            let mut i: u32 = 0;
            while i < child_count && res == LvResult::Ok && !e.stop_processing {
                if let Some(child) = self.obj_event_child(target, i) {
                    e.current_target = EventTarget::Obj(child);
                    res = self.obj_event_send_core(e);
                    if res != LvResult::Ok {
                        #[cfg(feature = "log")]
                        log::warn!("Trickle down event {} to child {:?} failed", e.code.0, child);
                        break;
                    }
                }
                i += 1;
            }
        }

        res
    }

    /// `lv_event_send(list, e, preprocess)` on the live event list of
    /// `target` (`list == NULL` -> `LV_RESULT_OK` when `has_list` is false).
    ///
    /// [`Ui::event_send`] takes a snapshot and recognises the live list by
    /// its ids, so the snapshot is taken right before the call (callbacks
    /// run earlier in the dispatch may have changed the list).
    fn obj_event_send_list(
        &mut self,
        has_list: bool,
        target: Obj,
        e: &mut Event<'_>,
        preprocess: bool,
    ) -> LvResult {
        if !has_list {
            return LvResult::Ok;
        }
        let list = self
            .obj(target)
            .and_then(|o| o.spec.as_deref())
            .map(|s| s.event_list.clone())
            .unwrap_or_default();
        self.event_send(list, e, preprocess)
    }

    /// `event_is_bubbled()`
    fn obj_event_is_bubbled(&self, e: &Event<'_>) -> bool {
        if e.stop_bubbling {
            return false;
        }

        /*Event codes that always bubble*/
        match e.code {
            EventCode::CHILD_CREATED | EventCode::CHILD_DELETED => return true,
            _ => {}
        }

        /*Check other codes only if bubbling is enabled*/
        if !self.obj_event_has_flag(e.current_target, ObjFlags::EVENT_BUBBLE) {
            return false;
        }

        !is_non_propagated_code(e.code)
    }

    /// `event_is_trickled()`
    fn obj_event_is_trickled(&self, e: &Event<'_>) -> bool {
        if e.stop_trickling {
            return false;
        }

        /*Check other codes only if trickle is enabled*/
        if !self.obj_event_has_flag(e.current_target, ObjFlags::EVENT_TRICKLE) {
            return false;
        }

        !is_non_propagated_code(e.code)
    }

    /// `lv_obj_is_event_bubble()` / `lv_obj_is_event_trickle()` of an event
    /// target (`false` for a stale handle).
    fn obj_event_has_flag(&self, target: EventTarget, flag: ObjFlags) -> bool {
        target
            .obj()
            .and_then(|o| self.obj(o))
            .is_some_and(|d| d.flags.contains(flag))
    }

    /// `lv_obj_get_parent()`
    fn obj_event_parent(&self, obj: Obj) -> Option<Obj> {
        let parent = self.obj(obj)?.parent?;
        self.resolve(parent)
    }

    /// `lv_obj_get_child_count()`
    fn obj_event_child_count(&self, obj: Obj) -> u32 {
        self.obj(obj)
            .and_then(|o| o.spec.as_deref())
            .map_or(0, |s| s.children.len() as u32)
    }

    /// `lv_obj_get_child(obj, idx)` for a non-negative `idx`.
    fn obj_event_child(&self, obj: Obj, idx: u32) -> Option<Obj> {
        let spec = self.obj(obj)?.spec.as_deref()?;
        let id = *spec.children.get(idx as usize)?;
        self.resolve(id)
    }

    /// Keep the event-stack frame of `e` in step with `e` (in C they are the
    /// same memory): the frame follows `e`'s targets so that
    /// [`Ui::event_mark_deleted`] finds it, and `deleted` set on either side
    /// is visible on both.
    fn obj_event_frame_sync(&mut self, e: &mut Event<'_>) {
        let Some(frame) = e.frame.and_then(|i| self.event_stack.get_mut(i)) else {
            return;
        };
        frame.original_target = e.target;
        frame.current_target = e.current_target;
        if frame.deleted {
            e.deleted = true;
        } else if e.deleted {
            frame.deleted = true;
        }
    }

    /// `lv_obj_allocate_spec_attr()` + move `obj->spec_attr->event_list` out
    /// so it can be passed to the `lv_event_*` list functions together with
    /// `&mut self`. Put it back with [`Ui::obj_put_event_list`].
    fn obj_take_event_list(&mut self, obj: Obj) -> Option<EventList> {
        let o = self.obj_mut(obj)?;
        let spec = o.spec.get_or_insert_with(|| Box::new(SpecAttr::new()));
        Some(core::mem::take(&mut spec.event_list))
    }

    /// Store a list taken by [`Ui::obj_take_event_list`] back.
    fn obj_put_event_list(&mut self, obj: Obj, list: EventList) {
        if let Some(spec) = self.obj_mut(obj).and_then(|o| o.spec.as_deref_mut()) {
            spec.event_list = list;
        }
    }
}

/// The codes `event_is_bubbled()` / `event_is_trickled()` never propagate.
fn is_non_propagated_code(code: EventCode) -> bool {
    matches!(
        code,
        EventCode::HIT_TEST
            | EventCode::COVER_CHECK
            | EventCode::REFR_EXT_DRAW_SIZE
            | EventCode::DRAW_MAIN_BEGIN
            | EventCode::DRAW_MAIN
            | EventCode::DRAW_MAIN_END
            | EventCode::DRAW_POST_BEGIN
            | EventCode::DRAW_POST
            | EventCode::DRAW_POST_END
            | EventCode::DRAW_TASK_ADDED
            | EventCode::REFRESH
            | EventCode::DELETE
            | EventCode::CHILD_CREATED
            | EventCode::CHILD_DELETED
            | EventCode::CHILD_CHANGED
            | EventCode::SIZE_CHANGED
            | EventCode::STYLE_CHANGED
            | EventCode::GET_SELF_SIZE
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::core::UiConfig;
    use crate::core::obj::{ObjClassEditable, ObjClassGroupDef, ObjData, ObjId};
    use crate::misc::area::Area;
    use alloc::vec;
    use alloc::vec::Vec;
    use core::cell::{Cell, RefCell};

    /// A class without an `event_cb` anywhere in its chain.
    pub(crate) static PLAIN_CLASS: ObjClass = PLAIN;

    const PLAIN: ObjClass = ObjClass {
        base: None,
        name: "test_plain",
        new_state: || Box::new(()),
        constructor: None,
        destructor: None,
        event_cb: None,
        width_def: 0,
        height_def: 0,
        editable: ObjClassEditable::Inherit,
        group_def: ObjClassGroupDef::Inherit,
        theme_inheritable: false,
    };

    fn log_push(ui: &mut Ui, s: &'static str) {
        if let Some(log) = ui.app_mut::<Vec<&'static str>>() {
            log.push(s);
        }
    }

    fn root_event(_c: &'static ObjClass, ui: &mut Ui, _e: &mut Event<'_>) {
        log_push(ui, "root");
    }

    fn derived_event(_c: &'static ObjClass, ui: &mut Ui, e: &mut Event<'_>) {
        let res = ui.obj_event_base(&DERIVED_CLASS, e);
        if res != LvResult::Ok {
            return;
        }
        log_push(ui, "derived");
    }

    static ROOT_CLASS: ObjClass = ObjClass {
        base: None,
        name: "test_root",
        event_cb: Some(root_event),
        ..PLAIN
    };

    /// No `event_cb`: `lv_obj_event_base()` must skip it.
    static MIDDLE_CLASS: ObjClass = ObjClass {
        base: Some(&ROOT_CLASS),
        name: "test_middle",
        event_cb: None,
        ..PLAIN
    };

    static DERIVED_CLASS: ObjClass = ObjClass {
        base: Some(&MIDDLE_CLASS),
        name: "test_derived",
        event_cb: Some(derived_event),
        ..PLAIN
    };

    /// Insert an object directly into the arena (`lv_obj_create()` is
    /// ported elsewhere) and link it to `parent`.
    pub(crate) fn add_obj(ui: &mut Ui, class: &'static ObjClass, parent: Option<Obj>) -> Obj {
        let data = ObjData {
            ext_data: Default::default(),
            class,
            parent: parent.map(|p| p.0),
            spec: None,
            styles: Vec::new(),
            style_main_prop_is_set: 0,
            style_other_prop_is_set: 0,
            user_data: None,
            coords: Area::new(0, 0, 99, 49),
            flags: ObjFlags::empty(),
            radio_button: false,
            state: Default::default(),
            layout_inv: false,
            readjust_scroll_after_layout: false,
            scr_layout_inv: false,
            skip_trans: false,
            h_layout: false,
            w_layout: false,
            h_ignore_size: false,
            w_ignore_size: false,
            is_deleting: false,
            rendered: false,
            has_blur: false,
            #[cfg(feature = "obj-id")]
            id: 0,
            widget: Box::new(()),
        };
        let obj = Obj(ObjId(ui.objs.insert(data)));
        if let Some(p) = parent {
            let pd = ui.obj_mut(p).unwrap();
            pd.spec
                .get_or_insert_with(|| Box::new(SpecAttr::new()))
                .children
                .push(obj.0);
        }
        obj
    }

    /// The parts of `lv_obj_delete()` the dispatcher depends on: mark the
    /// running events as deleted and free the object.
    fn fake_delete(ui: &mut Ui, obj: Obj) {
        ui.event_mark_deleted(EventTarget::Obj(obj));
        if let Some(p) = ui.obj(obj).and_then(|o| o.parent)
            && let Some(spec) = ui.objs.get_mut(p.0).and_then(|o| o.spec.as_deref_mut())
        {
            spec.children.retain(|c| *c != obj.0);
        }
        ui.objs.remove(obj.0.0);
    }

    fn set_flag(ui: &mut Ui, obj: Obj, f: ObjFlags) {
        ui.obj_mut(obj).unwrap().flags.insert(f);
    }

    fn counter_cb(counter: &Rc<Cell<u32>>) -> EventCallback {
        let c = counter.clone();
        Rc::new(move |_ui: &mut Ui, _e: &mut Event<'_>| c.set(c.get() + 1))
    }

    fn new_ui() -> Ui {
        let mut ui = Ui::new(UiConfig::default());
        ui.set_app(Vec::<&'static str>::new());
        ui
    }

    #[test]
    fn class_chain_skips_classes_without_event_cb() {
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &DERIVED_CLASS, None);
        let res = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
        assert_eq!(res, LvResult::Ok);
        assert_eq!(ui.app::<Vec<&str>>().unwrap(), &vec!["root", "derived"]);
        assert!(ui.event_stack.is_empty());
    }

    #[test]
    fn event_base_without_handler_is_ok() {
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        assert_eq!(
            ui.obj_send_event(obj, EventCode::CLICKED, EventParam::None),
            LvResult::Ok
        );
        let mut e = Event {
            code: EventCode::CLICKED,
            target: EventTarget::Obj(obj),
            current_target: EventTarget::Obj(obj),
            param: EventParam::None,
            user_data: None,
            ext_data: None,
            stop_processing: false,
            stop_bubbling: false,
            stop_trickling: false,
            deleted: false,
            frame: None,
        };
        assert_eq!(ui.obj_event_base(&ROOT_CLASS, &mut e), LvResult::Ok);
    }

    #[test]
    fn preprocess_class_and_post_order() {
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &ROOT_CLASS, None);
        let post: EventCallback = Rc::new(|ui: &mut Ui, _e: &mut Event<'_>| log_push(ui, "post"));
        let pre: EventCallback = Rc::new(|ui: &mut Ui, _e: &mut Event<'_>| log_push(ui, "pre"));
        ui.obj_add_event_cb(obj, post, EventCode::CLICKED, None);
        ui.obj_add_event_cb(obj, pre, EventFilter::new(EventCode::CLICKED, true), None);
        ui.obj_send_event(obj, EventCode::CLICKED, EventParam::None);
        assert_eq!(ui.app::<Vec<&str>>().unwrap(), &vec!["pre", "root", "post"]);
    }

    /// test_event.c: test_event_stop_processing (sent directly instead of
    /// through a simulated click).
    #[test]
    fn stop_processing() {
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        let cnt: [Rc<Cell<u32>>; 4] = Default::default();
        let pre_stop = Rc::new(Cell::new(false));
        let post_stop = Rc::new(Cell::new(false));
        let mk = |c: &Rc<Cell<u32>>, stop: Option<&Rc<Cell<bool>>>| -> EventCallback {
            let c = c.clone();
            let stop = stop.cloned();
            Rc::new(move |_ui: &mut Ui, e: &mut Event<'_>| {
                c.set(c.get() + 1);
                if stop.as_ref().is_some_and(|s| s.get()) {
                    e.stop_processing();
                }
            })
        };
        let pre = EventFilter::new(EventCode::CLICKED, true);
        ui.obj_add_event_cb(obj, mk(&cnt[0], Some(&pre_stop)), pre, None);
        ui.obj_add_event_cb(obj, mk(&cnt[1], None), pre, None);
        ui.obj_add_event_cb(obj, mk(&cnt[2], Some(&post_stop)), EventCode::CLICKED, None);
        ui.obj_add_event_cb(obj, mk(&cnt[3], None), EventCode::CLICKED, None);

        let run = |ui: &mut Ui, ps: bool, qs: bool| -> [u32; 4] {
            for c in &cnt {
                c.set(0);
            }
            pre_stop.set(ps);
            post_stop.set(qs);
            ui.obj_send_event(obj, EventCode::CLICKED, EventParam::None);
            [cnt[0].get(), cnt[1].get(), cnt[2].get(), cnt[3].get()]
        };
        assert_eq!(run(&mut ui, false, false), [1, 1, 1, 1]);
        assert_eq!(run(&mut ui, true, false), [1, 0, 0, 0]);
        assert_eq!(run(&mut ui, false, true), [1, 1, 1, 0]);
    }

    /// test_event.c: test_event_remove_event_cb
    #[test]
    fn remove_event_cb_removes_all_matching() {
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        let cb: EventCallback = Rc::new(|_ui: &mut Ui, _e: &mut Event<'_>| {});
        let other: EventCallback = Rc::new(|_ui: &mut Ui, _e: &mut Event<'_>| {});
        assert_eq!(ui.obj_get_event_count(obj), 0);
        assert_eq!(ui.obj_remove_event_cb(obj, &cb), 0);
        ui.obj_add_event_cb(obj, cb.clone(), EventCode::CLICKED, None);
        ui.obj_add_event_cb(obj, other.clone(), EventCode::CLICKED, None);
        ui.obj_add_event_cb(obj, cb.clone(), EventCode::PRESSED, None);
        assert_eq!(ui.obj_get_event_count(obj), 3);
        assert_eq!(ui.obj_remove_event_cb(obj, &cb), 2);
        assert_eq!(ui.obj_get_event_count(obj), 1);
        assert!(Rc::ptr_eq(&ui.obj_get_event_dsc(obj, 0).unwrap().cb, &other));
        assert!(ui.obj_get_event_dsc(obj, 1).is_none());
    }

    #[test]
    fn remove_event_cb_with_user_data() {
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        let cb: EventCallback = Rc::new(|_ui: &mut Ui, _e: &mut Event<'_>| {});
        let ud1: Rc<dyn Any> = Rc::new(1u32);
        let ud2: Rc<dyn Any> = Rc::new(2u32);
        ui.obj_add_event_cb(obj, cb.clone(), EventCode::ALL, Some(ud1.clone()));
        ui.obj_add_event_cb(obj, cb.clone(), EventCode::ALL, Some(ud2.clone()));
        ui.obj_add_event_cb(obj, cb.clone(), EventCode::ALL, None);
        assert_eq!(ui.obj_remove_event_cb_with_user_data(obj, &cb, &ud1), 1);
        assert_eq!(ui.obj_get_event_count(obj), 2);
        let ud = ui.obj_get_event_dsc(obj, 0).unwrap().user_data.clone().unwrap();
        assert!(Rc::ptr_eq(&ud, &ud2));
    }

    /// `lv_obj_remove_event_cb(obj, NULL)` removes every descriptor.
    #[test]
    fn remove_event_cb_null_removes_all() {
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        let cb: EventCallback = Rc::new(|_ui: &mut Ui, _e: &mut Event<'_>| {});
        let other: EventCallback = Rc::new(|_ui: &mut Ui, _e: &mut Event<'_>| {});
        assert_eq!(ui.obj_remove_event_cb_opt(obj, None), 0);
        ui.obj_add_event_cb(obj, cb.clone(), EventCode::CLICKED, None);
        ui.obj_add_event_cb(obj, other.clone(), EventCode::CLICKED, None);
        ui.obj_add_event_cb(obj, cb.clone(), EventCode::PRESSED, Some(Rc::new(3u32)));
        assert_eq!(ui.obj_remove_event_cb_opt(obj, None), 3);
        assert_eq!(ui.obj_get_event_count(obj), 0);
    }

    /// `lv_obj_remove_event_cb_with_user_data()` with `NULL` user_data
    /// and/or `NULL` callback.
    #[test]
    fn remove_event_cb_with_null_user_data() {
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        let cb: EventCallback = Rc::new(|_ui: &mut Ui, _e: &mut Event<'_>| {});
        let other: EventCallback = Rc::new(|_ui: &mut Ui, _e: &mut Event<'_>| {});
        let ud1: Rc<dyn Any> = Rc::new(1u32);
        ui.obj_add_event_cb(obj, cb.clone(), EventCode::ALL, Some(ud1.clone()));
        ui.obj_add_event_cb(obj, cb.clone(), EventCode::ALL, None);
        ui.obj_add_event_cb(obj, other.clone(), EventCode::ALL, None);
        ui.obj_add_event_cb(obj, other.clone(), EventCode::ALL, Some(ud1.clone()));
        // (cb, NULL): only the cb descriptor registered with NULL user_data.
        assert_eq!(ui.obj_remove_event_cb_with_user_data_opt(obj, Some(&cb), None), 1);
        assert_eq!(ui.obj_get_event_count(obj), 3);
        // (NULL, ud1): any callback registered with ud1.
        assert_eq!(
            ui.obj_remove_event_cb_with_user_data_opt(obj, None, Some(&ud1)),
            2
        );
        assert_eq!(ui.obj_get_event_count(obj), 1);
        let dsc = ui.obj_get_event_dsc(obj, 0).unwrap();
        assert!(Rc::ptr_eq(&dsc.cb, &other));
        assert!(dsc.user_data.is_none());
        // (NULL, NULL): any callback registered with NULL user_data.
        assert_eq!(ui.obj_remove_event_cb_with_user_data_opt(obj, None, None), 1);
        assert_eq!(ui.obj_get_event_count(obj), 0);
    }

    /// test_event.c: test_event_remove_event_dsc
    #[test]
    fn remove_event_dsc() {
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        let cb: EventCallback = Rc::new(|_ui: &mut Ui, _e: &mut Event<'_>| {});
        let dsc_1 = ui.obj_add_event_cb(obj, cb.clone(), EventCode::CLICKED, None);
        let dsc_2 = ui.obj_add_event_cb(obj, cb.clone(), EventCode::PRESSED, None);
        assert_eq!(ui.obj_get_event_count(obj), 2);

        assert!(ui.obj_remove_event_dsc(obj, dsc_1));
        assert_eq!(ui.obj_get_event_count(obj), 1);
        assert_eq!(
            ui.obj_get_event_dsc(obj, 0).map(|d| d.code()),
            Some(EventCode::PRESSED)
        );
        assert_eq!(
            ui.obj(obj).unwrap().spec.as_ref().unwrap().event_list.dscs[0],
            dsc_2
        );

        /* Removing the same descriptor twice must fail */
        assert!(!ui.obj_remove_event_dsc(obj, dsc_1));
        assert_eq!(ui.obj_get_event_count(obj), 1);

        assert!(ui.obj_remove_event_dsc(obj, dsc_2));
        assert_eq!(ui.obj_get_event_count(obj), 0);

        /* A descriptor of another widget doesn't belong to this list */
        let other = add_obj(&mut ui, &PLAIN_CLASS, None);
        let dsc_other = ui.obj_add_event_cb(other, cb, EventCode::CLICKED, None);
        assert!(!ui.obj_remove_event_dsc(obj, dsc_other));
        assert_eq!(ui.obj_get_event_count(other), 1);
    }

    #[test]
    fn no_spec_attr_queries() {
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        assert_eq!(ui.obj_get_event_count(obj), 0);
        assert!(ui.obj_get_event_dsc(obj, 0).is_none());
        assert!(!ui.obj_remove_event(obj, 0));
        assert!(ui.obj(obj).unwrap().spec.is_none());
        // Sending to an object without special attributes runs the class only.
        assert_eq!(
            ui.obj_send_event(obj, EventCode::CLICKED, EventParam::None),
            LvResult::Ok
        );
        assert!(ui.obj(obj).unwrap().spec.is_none());
    }

    /// test_event.c: test_event_stop_bubbling
    #[test]
    fn bubbling_and_stop_bubbling() {
        let mut ui = new_ui();
        let parent = add_obj(&mut ui, &PLAIN_CLASS, None);
        let child = add_obj(&mut ui, &PLAIN_CLASS, Some(parent));
        set_flag(&mut ui, child, ObjFlags::EVENT_BUBBLE);

        let child_cnt = Rc::new(Cell::new(0u32));
        let parent_cnt = Rc::new(Cell::new(0u32));
        let stop = Rc::new(Cell::new(false));
        {
            let c = child_cnt.clone();
            let s = stop.clone();
            let cb: EventCallback = Rc::new(move |_ui: &mut Ui, e: &mut Event<'_>| {
                c.set(c.get() + 1);
                if s.get() {
                    e.stop_bubbling();
                }
            });
            ui.obj_add_event_cb(child, cb, EventCode::CLICKED, None);
        }
        ui.obj_add_event_cb(parent, counter_cb(&parent_cnt), EventCode::CLICKED, None);

        ui.obj_send_event(child, EventCode::CLICKED, EventParam::None);
        assert_eq!((child_cnt.get(), parent_cnt.get()), (1, 1));

        child_cnt.set(0);
        parent_cnt.set(0);
        stop.set(true);
        ui.obj_send_event(child, EventCode::CLICKED, EventParam::None);
        assert_eq!((child_cnt.get(), parent_cnt.get()), (1, 0));

        // Codes that never bubble even with EVENT_BUBBLE.
        stop.set(false);
        parent_cnt.set(0);
        ui.obj_add_event_cb(parent, counter_cb(&parent_cnt), EventCode::SIZE_CHANGED, None);
        ui.obj_send_event(child, EventCode::SIZE_CHANGED, EventParam::None);
        assert_eq!(parent_cnt.get(), 0);
    }

    #[test]
    fn child_created_always_bubbles() {
        let mut ui = new_ui();
        let grand = add_obj(&mut ui, &PLAIN_CLASS, None);
        let parent = add_obj(&mut ui, &PLAIN_CLASS, Some(grand));
        let child = add_obj(&mut ui, &PLAIN_CLASS, Some(parent));
        let parent_cnt = Rc::new(Cell::new(0u32));
        let grand_cnt = Rc::new(Cell::new(0u32));
        ui.obj_add_event_cb(parent, counter_cb(&parent_cnt), EventCode::CHILD_CREATED, None);
        ui.obj_add_event_cb(grand, counter_cb(&grand_cnt), EventCode::CHILD_CREATED, None);
        ui.obj_send_event(child, EventCode::CHILD_CREATED, EventParam::Obj(child));
        // Always bubbles (no EVENT_BUBBLE flag needed), all the way up.
        assert_eq!((parent_cnt.get(), grand_cnt.get()), (1, 1));
        // Without the flag other codes do not bubble.
        ui.obj_add_event_cb(parent, counter_cb(&parent_cnt), EventCode::CLICKED, None);
        ui.obj_send_event(child, EventCode::CLICKED, EventParam::None);
        assert_eq!(parent_cnt.get(), 1);
    }

    /// test_event_trickle.c: basic / stop / disabled
    #[test]
    fn trickle() {
        let mut ui = new_ui();
        let count = Rc::new(Cell::new(0u32));
        let targets: Rc<RefCell<Vec<Obj>>> = Rc::default();
        let cb: EventCallback = {
            let c = count.clone();
            let t = targets.clone();
            Rc::new(move |_ui: &mut Ui, e: &mut Event<'_>| {
                c.set(c.get() + 1);
                t.borrow_mut().push(e.current_target_obj().unwrap());
            })
        };

        // basic
        let parent = add_obj(&mut ui, &PLAIN_CLASS, None);
        set_flag(&mut ui, parent, ObjFlags::EVENT_TRICKLE);
        ui.obj_add_event_cb(parent, cb.clone(), EventCode::CLICKED, None);
        let child1 = add_obj(&mut ui, &PLAIN_CLASS, Some(parent));
        ui.obj_add_event_cb(child1, cb.clone(), EventCode::CLICKED, None);
        let child2 = add_obj(&mut ui, &PLAIN_CLASS, Some(parent));
        ui.obj_add_event_cb(child2, cb.clone(), EventCode::CLICKED, None);
        ui.obj_send_event(parent, EventCode::CLICKED, EventParam::None);
        assert_eq!(count.get(), 3);
        assert_eq!(&*targets.borrow(), &[parent, child1, child2]);

        // stop
        let parent2 = add_obj(&mut ui, &PLAIN_CLASS, None);
        set_flag(&mut ui, parent2, ObjFlags::EVENT_TRICKLE);
        ui.obj_add_event_cb(parent2, cb.clone(), EventCode::CLICKED, None);
        let stop: EventCallback = Rc::new(|_ui: &mut Ui, e: &mut Event<'_>| e.stop_trickling());
        ui.obj_add_event_cb(parent2, stop, EventCode::CLICKED, None);
        let c21 = add_obj(&mut ui, &PLAIN_CLASS, Some(parent2));
        ui.obj_add_event_cb(c21, cb.clone(), EventCode::CLICKED, None);
        count.set(0);
        ui.obj_send_event(parent2, EventCode::CLICKED, EventParam::None);
        assert_eq!(count.get(), 1);

        // disabled
        let parent3 = add_obj(&mut ui, &PLAIN_CLASS, None);
        ui.obj_add_event_cb(parent3, cb.clone(), EventCode::CLICKED, None);
        let c31 = add_obj(&mut ui, &PLAIN_CLASS, Some(parent3));
        ui.obj_add_event_cb(c31, cb.clone(), EventCode::CLICKED, None);
        count.set(0);
        ui.obj_send_event(parent3, EventCode::CLICKED, EventParam::None);
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn trickled_children_do_not_bubble_back() {
        let mut ui = new_ui();
        let count = Rc::new(Cell::new(0u32));
        let parent = add_obj(&mut ui, &PLAIN_CLASS, None);
        set_flag(&mut ui, parent, ObjFlags::EVENT_TRICKLE);
        let child = add_obj(&mut ui, &PLAIN_CLASS, Some(parent));
        set_flag(&mut ui, child, ObjFlags::EVENT_BUBBLE);
        ui.obj_add_event_cb(parent, counter_cb(&count), EventCode::CLICKED, None);
        ui.obj_send_event(parent, EventCode::CLICKED, EventParam::None);
        assert_eq!(count.get(), 1);
    }

    /// test_event.c: test_event_object_deletion
    #[test]
    fn delete_in_class_event() {
        fn deleting_event(_c: &'static ObjClass, ui: &mut Ui, e: &mut Event<'_>) {
            if e.code() == EventCode::VALUE_CHANGED {
                let t = e.target_obj().unwrap();
                fake_delete(ui, t);
            }
        }
        static DELETING_CLASS: ObjClass = ObjClass {
            name: "test_deleting",
            event_cb: Some(deleting_event),
            ..PLAIN
        };
        let mut ui = new_ui();
        let parent = add_obj(&mut ui, &PLAIN_CLASS, None);
        let obj = add_obj(&mut ui, &DELETING_CLASS, Some(parent));
        set_flag(&mut ui, obj, ObjFlags::EVENT_BUBBLE);
        let post = Rc::new(Cell::new(0u32));
        let parent_cnt = Rc::new(Cell::new(0u32));
        ui.obj_add_event_cb(obj, counter_cb(&post), EventCode::ALL, None);
        ui.obj_add_event_cb(parent, counter_cb(&parent_cnt), EventCode::ALL, None);
        let res = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
        assert_eq!(res, LvResult::Invalid);
        assert!(!ui.obj_is_valid(obj));
        // Neither the post callbacks nor the parent ran.
        assert_eq!((post.get(), parent_cnt.get()), (0, 0));
        assert!(ui.event_stack.is_empty());
    }

    #[test]
    #[cfg(not(feature = "check-arg-panic"))]
    fn stale_handle() {
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        fake_delete(&mut ui, obj);
        assert_eq!(
            ui.obj_send_event(obj, EventCode::CLICKED, EventParam::None),
            LvResult::Invalid
        );
        let cb: EventCallback = Rc::new(|_ui: &mut Ui, _e: &mut Event<'_>| {});
        let id = ui.obj_add_event_cb(obj, cb, EventCode::ALL, None);
        assert!(ui.event_dscs.get(id.0).is_none());
        assert!(ui.event_dscs.is_empty());
    }

    /// test_event.c: test_event_delete_obj_in_recursive_event_call
    #[test]
    fn delete_obj_in_recursive_event_call() {
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        let click_count = Rc::new(Cell::new(0u32));
        let cb: EventCallback = {
            let cc = click_count.clone();
            Rc::new(move |ui: &mut Ui, e: &mut Event<'_>| {
                let obj = e.target_obj().unwrap();
                cc.set(cc.get() + 1);
                if cc.get() == 10 {
                    ui.obj_remove_event(obj, 0);
                } else if cc.get() == 15 {
                    fake_delete(ui, obj);
                } else {
                    ui.obj_send_event(obj, EventCode::CLICKED, EventParam::None);
                }
            })
        };
        ui.obj_add_event_cb(obj, cb.clone(), EventCode::CLICKED, None);
        ui.obj_add_event_cb(obj, cb, EventCode::CLICKED, None);
        let res = ui.obj_send_event(obj, EventCode::CLICKED, EventParam::None);
        assert_eq!(res, LvResult::Invalid);
        assert_eq!(click_count.get(), 15);
        assert!(!ui.obj_is_valid(obj));
        assert!(ui.event_stack.is_empty());
    }

    #[test]
    fn delete_parent_while_bubbling() {
        let mut ui = new_ui();
        let parent = add_obj(&mut ui, &PLAIN_CLASS, None);
        let child = add_obj(&mut ui, &PLAIN_CLASS, Some(parent));
        set_flag(&mut ui, child, ObjFlags::EVENT_BUBBLE);
        set_flag(&mut ui, parent, ObjFlags::EVENT_TRICKLE);
        let del: EventCallback = Rc::new(|ui: &mut Ui, e: &mut Event<'_>| {
            let cur = e.current_target_obj().unwrap();
            fake_delete(ui, cur);
        });
        ui.obj_add_event_cb(parent, del, EventCode::CLICKED, None);
        let res = ui.obj_send_event(child, EventCode::CLICKED, EventParam::None);
        assert_eq!(res, LvResult::Invalid);
        assert!(!ui.obj_is_valid(parent));
    }

    #[test]
    fn ext_draw_size_param_reaches_callbacks() {
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &PLAIN_CLASS, None);
        let cb: EventCallback = Rc::new(|_ui: &mut Ui, e: &mut Event<'_>| e.set_ext_draw_size(7));
        ui.obj_add_event_cb(obj, cb, EventCode::REFR_EXT_DRAW_SIZE, None);
        let mut s = 3;
        ui.obj_send_event(
            obj,
            EventCode::REFR_EXT_DRAW_SIZE,
            EventParam::ExtDrawSize(&mut s),
        );
        assert_eq!(s, 7);
    }

    #[test]
    fn user_data_is_passed_and_cleared_for_class() {
        fn class_cb(_c: &'static ObjClass, ui: &mut Ui, e: &mut Event<'_>) {
            if e.user_data().is_none() {
                log_push(ui, "class-no-ud");
            }
        }
        static UD_CLASS: ObjClass = ObjClass {
            name: "test_ud",
            event_cb: Some(class_cb),
            ..PLAIN
        };
        let mut ui = new_ui();
        let obj = add_obj(&mut ui, &UD_CLASS, None);
        let cb: EventCallback = Rc::new(|ui: &mut Ui, e: &mut Event<'_>| {
            if e.user_data().and_then(|u| u.downcast_ref::<u32>()) == Some(&5) {
                log_push(ui, "ud5");
            }
        });
        ui.obj_add_event_cb(
            obj,
            cb.clone(),
            EventFilter::new(EventCode::ALL, true),
            Some(Rc::new(5u32)),
        );
        ui.obj_send_event(obj, EventCode::CLICKED, EventParam::None);
        assert_eq!(ui.app::<Vec<&str>>().unwrap(), &vec!["ud5", "class-no-ud"]);
    }

    #[test]
    fn non_propagated_codes_match_upstream() {
        assert!(is_non_propagated_code(EventCode::GET_SELF_SIZE));
        assert!(is_non_propagated_code(EventCode::DELETE));
        assert!(!is_non_propagated_code(EventCode::CLICKED));
        assert!(!is_non_propagated_code(EventCode::VALUE_CHANGED));
        assert!(!is_non_propagated_code(EventCode::LAYOUT_CHANGED));
    }
}
