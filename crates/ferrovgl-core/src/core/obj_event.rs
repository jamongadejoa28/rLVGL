//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_obj_event.h`, `src/core/lv_obj_event.c`.
//!
//! Object event dispatch: preprocess callbacks, the class `event_cb` chain,
//! user callbacks, bubbling and trickling.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::rc::Rc;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::{Obj, ObjClass};
use crate::misc::event::{
    Event, EventCallback, EventCode, EventDsc, EventDscId, EventFilter, EventParam, LvResult,
};

impl Ui {
    /// `lv_obj_send_event()`
    pub fn obj_send_event(&mut self, obj: Obj, code: EventCode, param: EventParam<'_>) -> LvResult {
        todo!()
    }

    /// `lv_obj_event_base()`: call the class event handlers from `class`
    /// upwards (the "base class" chain).
    pub fn obj_event_base(&mut self, class: &'static ObjClass, e: &mut Event<'_>) -> LvResult {
        todo!()
    }

    /// `lv_obj_add_event_cb()`
    pub fn obj_add_event_cb(
        &mut self,
        obj: Obj,
        cb: EventCallback,
        filter: impl Into<EventFilter>,
        user_data: Option<Rc<dyn Any>>,
    ) -> EventDscId {
        todo!()
    }

    /// `lv_obj_get_event_count()`
    pub fn obj_get_event_count(&self, obj: Obj) -> u32 {
        todo!()
    }

    /// `lv_obj_get_event_dsc()`
    pub fn obj_get_event_dsc(&self, obj: Obj, index: u32) -> Option<&EventDsc> {
        todo!()
    }

    /// `lv_obj_remove_event()`
    pub fn obj_remove_event(&mut self, obj: Obj, index: u32) -> bool {
        todo!()
    }

    /// `lv_obj_remove_event_dsc()`
    pub fn obj_remove_event_dsc(&mut self, obj: Obj, dsc: EventDscId) -> bool {
        todo!()
    }

    /// `lv_obj_remove_event_cb()`: remove every descriptor whose callback is
    /// `cb` (compared by `Rc::ptr_eq`). Returns the number removed.
    pub fn obj_remove_event_cb(&mut self, obj: Obj, cb: &EventCallback) -> u32 {
        todo!()
    }

    /// `lv_obj_remove_event_cb_with_user_data()`
    pub fn obj_remove_event_cb_with_user_data(
        &mut self,
        obj: Obj,
        cb: &EventCallback,
        user_data: &Rc<dyn Any>,
    ) -> u32 {
        todo!()
    }
}
