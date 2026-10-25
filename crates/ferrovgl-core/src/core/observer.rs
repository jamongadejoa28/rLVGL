//! Ported from LVGL v9.6.0: `src/core/lv_observer.c`.
//!
//! Also covers `include/lvgl/core/lv_observer.h`,
//! `src/core/lv_observer_private.h` and the observer-based style bindings of
//! `src/core/lv_obj_style.c` (`lv_obj_bind_style()`,
//! `lv_obj_bind_style_prop()`). The widget bindings (`lv_bar_bind_value()`,
//! `lv_label_bind_text()`, ...) live in `ferrovgl-widgets/src/observer.rs`.
//!
//! Ownership: subjects and observers live in arenas of [`ObserverState`]
//! (`Ui::observer`, the `subject_ll` of `lv_global_t` plus each subject's
//! `subs_ll`) and are addressed by `Copy` handles. `lv_subject_create()`
//! inserts at the tail of `subject_ll`, `lv_subject_add_observer*()` at the
//! tail of `subs_ll`; the `*_order` / `subs` vectors keep those orders.
//! Callbacks are `Rc<dyn Fn(&mut Ui, ..)>`, cloned out of the arena before
//! they are called; anything that runs a callback must re-check the handles
//! afterwards.
//!
//! String subjects own their buffers (`lv_subject_set_string_buffer_static()`
//! takes a size instead of caller buffers); pointer subjects hold an
//! `Rc<dyn Any>` compared by identity like the C pointer.
//!
//! SKELETON: the types and public API are declared; the bodies are `todo!()`
//! until the module is ported. Only the hooks the rest of the core calls
//! (`Ui::observer_obj_deleted`, `Ui::subject_global_deinit`) are no-op
//! stubs so existing code paths never reach a `todo!()`.

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::cell::RefCell;

use crate::core::obj::{Obj, ObjFlags};
use crate::core::obj_style::StyleId;
use crate::core::ui::Ui;
use crate::misc::arena::{Arena, Key};
use crate::misc::color::Color;
use crate::misc::event::EventCode;
use crate::misc::style::{State, StyleProp, StyleSelector};

/// Handle of a subject (`lv_subject_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SubjectId(pub Key);

/// Handle of an observer (`lv_observer_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObserverId(pub Key);

/// `lv_subject_type_t`: values of the subject's `type` field, with the
/// upstream discriminants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SubjectType {
    /// `LV_SUBJECT_TYPE_INVALID`: the subject is not initialized yet.
    #[default]
    Invalid = 0,
    /// `LV_SUBJECT_TYPE_NONE`: a null value like None or NIL.
    None = 1,
    /// `LV_SUBJECT_TYPE_INT`: an `int32_t`.
    Int = 2,
    /// `LV_SUBJECT_TYPE_FLOAT`: a float, requires `LV_USE_FLOAT`.
    Float = 3,
    /// `LV_SUBJECT_TYPE_POINTER`: a void pointer.
    Pointer = 4,
    /// `LV_SUBJECT_TYPE_COLOR`: an `lv_color_t`.
    Color = 5,
    /// `LV_SUBJECT_TYPE_GROUP`: an array of subjects.
    Group = 6,
    /// `LV_SUBJECT_TYPE_STRING`: a char pointer.
    String = 7,
}

/// `lv_subject_value_t`: the C union as an enum; the subject's [`SubjectType`]
/// tells which member is meaningful, like in C.
#[derive(Clone, Default)]
pub enum SubjectValue {
    /// `num`: integer number (opacity, enums, booleans or "normal" numbers).
    /// Also the zeroed state of a fresh subject.
    Num(i32),
    /// `pointer` (`NULL` = `None`): a pointer subject's value, compared by
    /// `Rc` identity.
    Pointer(Option<Rc<dyn Any>>),
    /// `pointer` of a string subject: the owned buffer (`NULL` = `None`).
    Str(Option<String>),
    /// `pointer` of a group subject: the subject list (`NULL` = `None`).
    Group(Option<Rc<[SubjectId]>>),
    /// `color`
    Color(Color),
    /// `float_v` (`LV_USE_FLOAT`).
    #[cfg(feature = "float")]
    Float(f32),
    /// The all-zero union (`lv_memzero`).
    #[default]
    Zero,
}

impl core::fmt::Debug for SubjectValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SubjectValue::Num(n) => f.debug_tuple("Num").field(n).finish(),
            SubjectValue::Pointer(p) => f.debug_tuple("Pointer").field(&p.is_some()).finish(),
            SubjectValue::Str(s) => f.debug_tuple("Str").field(s).finish(),
            SubjectValue::Group(g) => f.debug_tuple("Group").field(g).finish(),
            SubjectValue::Color(c) => f.debug_tuple("Color").field(c).finish(),
            #[cfg(feature = "float")]
            SubjectValue::Float(v) => f.debug_tuple("Float").field(v).finish(),
            SubjectValue::Zero => f.write_str("Zero"),
        }
    }
}

/// The `free_cb` of a subject's `lv_ext_data_t` (`void (*)(void * data)`).
pub type SubjectExtDataFreeCb = fn(Option<Rc<dyn Any>>);

/// `lv_ext_data_t` of a subject (`LV_USE_EXT_DATA`), set by
/// `lv_subject_set_external_data()`.
#[derive(Default)]
pub struct SubjectExtData {
    /// `data`
    pub data: Option<Rc<dyn Any>>,
    /// `free_cb`
    pub free_cb: Option<SubjectExtDataFreeCb>,
}

/// `lv_subject_t`: an observable value.
#[derive(Default)]
pub struct Subject {
    /// `ext_data` (`LV_USE_EXT_DATA`).
    pub ext_data: SubjectExtData,
    /// `subs_ll`: the subscribers, in list order (`lv_ll_ins_tail`).
    pub subs: Vec<ObserverId>,
    /// `value`: current value.
    pub value: SubjectValue,
    /// `prev_value`: previous value.
    pub prev_value: SubjectValue,
    /// `min_value`: minimum value for min. int or float.
    pub min_value: SubjectValue,
    /// `max_value`: maximum value for max. int or float.
    pub max_value: SubjectValue,
    /// `user_data`: can be used freely by the user.
    pub user_data: Option<Rc<dyn Any>>,
    /// `type : 4`: one of the `LV_SUBJECT_TYPE_...` values.
    pub type_: SubjectType,
    /// `size : 24`: string buffer size or group length.
    pub size: u32,
    /// `notify_restart_query : 1`: an observer was deleted during
    /// notification, start notifying from the beginning.
    pub notify_restart_query: bool,
}

/// `lv_observer_cb_t`: called to notify an observer that the subject's value
/// has changed.
pub type ObserverCb = Rc<dyn Fn(&mut Ui, ObserverId, SubjectId)>;

/// `lv_obj_set_bool_t`: generic callback to set a boolean value on a widget.
pub type ObjSetBoolCb = Rc<dyn Fn(&mut Ui, Obj, bool)>;
/// `lv_obj_set_int_t`: generic callback to set an int value on a widget.
pub type ObjSetIntCb = Rc<dyn Fn(&mut Ui, Obj, i32)>;
/// `lv_obj_set_float_t`: generic callback to set a float value on a widget.
#[cfg(feature = "float")]
pub type ObjSetFloatCb = Rc<dyn Fn(&mut Ui, Obj, f32)>;
/// `lv_obj_set_string_t`: generic callback to set a string value on a widget.
pub type ObjSetStringCb = Rc<dyn Fn(&mut Ui, Obj, &str)>;
/// `lv_obj_set_color_t`: generic callback to set a color value on a widget.
pub type ObjSetColorCb = Rc<dyn Fn(&mut Ui, Obj, Color)>;
/// `lv_obj_set_pointer_t`: generic callback to set a pointer value on a widget.
pub type ObjSetPointerCb = Rc<dyn Fn(&mut Ui, Obj, Option<Rc<dyn Any>>)>;

/// `void (*user_cb)(void)` of an observer: the setter stored by the
/// `lv_obj_bind_<type>()` functions.
#[derive(Clone)]
pub enum ObserverUserCb {
    /// `lv_obj_bind_bool()`
    Bool(ObjSetBoolCb),
    /// `lv_obj_bind_int()`
    Int(ObjSetIntCb),
    /// `lv_obj_bind_float()`
    #[cfg(feature = "float")]
    Float(ObjSetFloatCb),
    /// `lv_obj_bind_string()`
    String(ObjSetStringCb),
    /// `lv_obj_bind_color()`
    Color(ObjSetColorCb),
    /// `lv_obj_bind_pointer()`
    Pointer(ObjSetPointerCb),
}

/// `void * target` of an observer; `for_obj` is the [`ObserverTarget::Obj`]
/// variant.
#[derive(Clone, Default)]
pub enum ObserverTarget {
    /// `NULL`
    #[default]
    None,
    /// A widget (`for_obj == 1`, `lv_subject_add_observer_obj()`).
    Obj(Obj),
    /// Any other pointer (`lv_subject_add_observer_with_target()`).
    Other(Rc<dyn Any>),
}

/// `lv_observer_t`: the descriptor returned when subscribing to a subject.
pub struct Observer {
    /// `subject`: the observed subject.
    pub subject: SubjectId,
    /// `cb`: called when the value changes.
    pub cb: Option<ObserverCb>,
    /// `target`: e.g. a widget or any pointer. `for_obj : 1` is
    /// `matches!(target, ObserverTarget::Obj(_))`.
    pub target: ObserverTarget,
    /// `user_data`: can be used freely by the user.
    pub user_data: Option<Rc<dyn Any>>,
    /// `user_cb`: additional function pointer, can be used freely.
    pub user_cb: Option<ObserverUserCb>,
    /// `auto_free_user_data : 1`: free the user data when the observer is
    /// removed (here: drop the `Rc`).
    pub auto_free_user_data: bool,
    /// `notified : 1`: the observer was already notified.
    pub notified: bool,
    /// `for_obj : 1`: `target` is a widget.
    pub for_obj: bool,
}

/// `lv_subject_increment_dsc_t`: created by
/// `lv_obj_add_subject_increment_event()`, shared with its event callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubjectIncrementDsc {
    /// `subject`: the subject to adjust.
    pub subject: SubjectId,
    /// `step`: the step added to the subject.
    pub step: i32,
    /// `rollover`: start over from the other end when one end is exceeded.
    pub rollover: bool,
    /// `min_value`: don't set a value smaller than this.
    pub min_value: i32,
    /// `max_value`: don't set a value larger than this.
    pub max_value: i32,
}

/// Shared handle of a [`SubjectIncrementDsc`] (`lv_subject_increment_dsc_t *`).
pub type SubjectIncrementDscRef = Rc<RefCell<SubjectIncrementDsc>>;

/// The observer part of `lv_global_t`: `subject_ll` and the storage of every
/// subject's `subs_ll`.
#[derive(Default)]
pub struct ObserverState {
    /// All subjects.
    pub subjects: Arena<Subject>,
    /// Order of `subject_ll` (`lv_ll_ins_tail`).
    pub subject_order: Vec<SubjectId>,
    /// All observers; each subject's `subs` keeps its list order.
    pub observers: Arena<Observer>,
}

impl ObserverState {
    /// `lv_subject_global_init()`: `lv_ll_init(&subject_list, ...)`, an empty
    /// list.
    pub const fn new() -> Self {
        Self {
            subjects: Arena::new(),
            subject_order: Vec::new(),
            observers: Arena::new(),
        }
    }
}

impl Ui {
    /// `lv_subject_global_deinit()`: deinit the group subjects first, then
    /// every other subject, and clear `subject_ll`. Called by `Ui::deinit()`
    /// at the `lv_deinit()` position.
    ///
    /// SKELETON: a no-op stub (no subjects can exist before the module is
    /// ported); must be ported 1:1 together with the rest of this file.
    pub(crate) fn subject_global_deinit(&mut self) {}

    /// Object deletion hook, called by `lv_obj_delete()`'s core
    /// (`obj_tree_delete_core`) just before the object's slot is freed.
    ///
    /// Upstream removes widget observers through the `LV_EVENT_DELETE`
    /// callback `unsubscribe_on_delete_cb` registered by
    /// `lv_subject_add_observer_obj()`; this hook only exists so stale
    /// [`ObserverTarget::Obj`] handles can be dropped without touching
    /// `obj_tree.rs` again.
    ///
    /// SKELETON: a no-op stub.
    pub(crate) fn observer_obj_deleted(&mut self, obj: Obj) {}

    /// `lv_subject_create()`: create a subject of `type_`. `None` (`NULL`)
    /// for `Invalid`/`None`, or `Float` without `LV_USE_FLOAT`.
    pub fn subject_create(&mut self, type_: SubjectType) -> Option<SubjectId> {
        todo!()
    }

    /// `lv_subject_delete()`: deinit the subject and remove it from
    /// `subject_ll`. `None` is ignored.
    pub fn subject_delete(&mut self, subject: Option<SubjectId>) {
        todo!()
    }

    /// `lv_subject_set_external_data()` (`LV_USE_EXT_DATA`): attach external
    /// data with a destructor called when the subject is deinitialized.
    pub fn subject_set_external_data(
        &mut self,
        subject: SubjectId,
        data: Option<Rc<dyn Any>>,
        free_cb: Option<SubjectExtDataFreeCb>,
    ) {
        todo!()
    }

    /// `lv_subject_init_int()`: initialize an integer subject in place.
    #[deprecated(note = "the subject init API is deprecated, use `subject_create` instead")]
    pub fn subject_init_int(&mut self, subject: SubjectId, value: i32) {
        todo!()
    }

    /// `lv_subject_set_int()`: set the value of an integer subject and
    /// notify the observers.
    pub fn subject_set_int(&mut self, subject: SubjectId, value: i32) {
        todo!()
    }

    /// `lv_subject_get_int()`
    pub fn subject_get_int(&self, subject: SubjectId) -> i32 {
        todo!()
    }

    /// `lv_subject_get_previous_int()`
    pub fn subject_get_previous_int(&self, subject: SubjectId) -> i32 {
        todo!()
    }

    /// `lv_subject_set_min_value_int()`
    pub fn subject_set_min_value_int(&mut self, subject: SubjectId, min_value: i32) {
        todo!()
    }

    /// `lv_subject_set_max_value_int()`
    pub fn subject_set_max_value_int(&mut self, subject: SubjectId, max_value: i32) {
        todo!()
    }

    /// `lv_subject_init_float()`: initialize a float subject in place.
    #[cfg(feature = "float")]
    #[deprecated(note = "the subject init API is deprecated, use `subject_create` instead")]
    pub fn subject_init_float(&mut self, subject: SubjectId, value: f32) {
        todo!()
    }

    /// `lv_subject_set_float()`: set the value of a float subject and notify
    /// the observers.
    #[cfg(feature = "float")]
    pub fn subject_set_float(&mut self, subject: SubjectId, value: f32) {
        todo!()
    }

    /// `lv_subject_get_float()`
    #[cfg(feature = "float")]
    pub fn subject_get_float(&self, subject: SubjectId) -> f32 {
        todo!()
    }

    /// `lv_subject_get_previous_float()`
    #[cfg(feature = "float")]
    pub fn subject_get_previous_float(&self, subject: SubjectId) -> f32 {
        todo!()
    }

    /// `lv_subject_set_min_value_float()`
    #[cfg(feature = "float")]
    pub fn subject_set_min_value_float(&mut self, subject: SubjectId, min_value: f32) {
        todo!()
    }

    /// `lv_subject_set_max_value_float()`
    #[cfg(feature = "float")]
    pub fn subject_set_max_value_float(&mut self, subject: SubjectId, max_value: f32) {
        todo!()
    }

    /// `lv_subject_init_string()`: initialize a string subject in place with
    /// owned buffers of `size` bytes (`track_previous` = non-`NULL`
    /// `prev_buf`).
    #[deprecated(note = "the subject init API is deprecated, use `subject_create` instead")]
    pub fn subject_init_string(
        &mut self,
        subject: SubjectId,
        size: usize,
        track_previous: bool,
        value: &str,
    ) {
        todo!()
    }

    /// `lv_subject_set_string_buffer_static()`: give a string subject owned
    /// buffers of `size` bytes (`track_previous` = non-`NULL` `prev_buf`).
    pub fn subject_set_string_buffer_static(
        &mut self,
        subject: SubjectId,
        size: usize,
        track_previous: bool,
    ) {
        todo!()
    }

    /// `lv_subject_set_string()`: copy `buf` (`lv_strlcpy` into `size`
    /// bytes) and notify the observers if it changed.
    pub fn subject_set_string(&mut self, subject: SubjectId, buf: &str) {
        todo!()
    }

    /// `lv_subject_snprintf()`: format a new string into the subject and
    /// notify the observers if it changed.
    pub fn subject_snprintf(&mut self, subject: SubjectId, args: core::fmt::Arguments<'_>) {
        todo!()
    }

    /// `lv_subject_get_string()`: `None` is `NULL`.
    pub fn subject_get_string(&self, subject: SubjectId) -> Option<&str> {
        todo!()
    }

    /// `lv_subject_get_previous_string()`: `None` is `NULL`.
    pub fn subject_get_previous_string(&self, subject: SubjectId) -> Option<&str> {
        todo!()
    }

    /// `lv_subject_init_pointer()`: initialize a pointer subject in place.
    #[deprecated(note = "the subject init API is deprecated, use `subject_create` instead")]
    pub fn subject_init_pointer(&mut self, subject: SubjectId, value: Option<Rc<dyn Any>>) {
        todo!()
    }

    /// `lv_subject_set_pointer()`: set the value of a pointer subject and
    /// notify the observers (regardless of whether it changed).
    pub fn subject_set_pointer(&mut self, subject: SubjectId, ptr: Option<Rc<dyn Any>>) {
        todo!()
    }

    /// `lv_subject_get_pointer()`
    pub fn subject_get_pointer(&self, subject: SubjectId) -> Option<Rc<dyn Any>> {
        todo!()
    }

    /// `lv_subject_get_previous_pointer()`
    pub fn subject_get_previous_pointer(&self, subject: SubjectId) -> Option<Rc<dyn Any>> {
        todo!()
    }

    /// `lv_subject_init_color()`: initialize a color subject in place.
    #[deprecated(note = "the subject init API is deprecated, use `subject_create` instead")]
    pub fn subject_init_color(&mut self, subject: SubjectId, color: Color) {
        todo!()
    }

    /// `lv_subject_set_color()`: set the value of a color subject and notify
    /// the observers if it changed.
    pub fn subject_set_color(&mut self, subject: SubjectId, color: Color) {
        todo!()
    }

    /// `lv_subject_get_color()`
    pub fn subject_get_color(&self, subject: SubjectId) -> Color {
        todo!()
    }

    /// `lv_subject_get_previous_color()`
    pub fn subject_get_previous_color(&self, subject: SubjectId) -> Color {
        todo!()
    }

    /// `lv_subject_init_group()`: initialize a group subject in place;
    /// observers of `group_subject` are notified when any of `list` changes.
    #[deprecated(note = "the subject init API is deprecated, use `subject_create` instead")]
    pub fn subject_init_group(&mut self, group_subject: SubjectId, list: Rc<[SubjectId]>) {
        todo!()
    }

    /// `lv_subject_set_group_list_static()`: set the subject list of a group
    /// subject (`list_len` is `list.len()`).
    pub fn subject_set_group_list_static(&mut self, subject: SubjectId, list: Rc<[SubjectId]>) {
        todo!()
    }

    /// `lv_subject_deinit()`: remove all observers and the bound widget
    /// events. `None` is ignored.
    #[deprecated(note = "the subject init API is deprecated, use `subject_create`/`subject_delete` instead")]
    pub fn subject_deinit(&mut self, subject: Option<SubjectId>) {
        todo!()
    }

    /// `lv_subject_get_group_element()`: `None` if `index` is out of bounds.
    pub fn subject_get_group_element(&self, subject: SubjectId, index: i32) -> Option<SubjectId> {
        todo!()
    }

    /// `lv_subject_add_observer()`: subscribe `observer_cb`; it is called
    /// once immediately.
    pub fn subject_add_observer(
        &mut self,
        subject: SubjectId,
        observer_cb: ObserverCb,
        user_data: Option<Rc<dyn Any>>,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_subject_add_observer_obj()`: subscribe for a widget; the observer
    /// is removed when the widget is deleted.
    pub fn subject_add_observer_obj(
        &mut self,
        subject: SubjectId,
        observer_cb: ObserverCb,
        obj: Option<Obj>,
        user_data: Option<Rc<dyn Any>>,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_subject_add_observer_with_target()`: subscribe and save a target
    /// pointer.
    pub fn subject_add_observer_with_target(
        &mut self,
        subject: SubjectId,
        observer_cb: ObserverCb,
        target: Option<Rc<dyn Any>>,
        user_data: Option<Rc<dyn Any>>,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_observer_delete()`: remove the observer from its subject. `None`
    /// is ignored.
    pub fn observer_delete(&mut self, observer: Option<ObserverId>) {
        todo!()
    }

    /// `lv_obj_remove_from_subject()`: remove the observers of `obj` from
    /// `subject`, or from all subjects when `None`.
    pub fn obj_remove_from_subject(&mut self, obj: Obj, subject: Option<SubjectId>) {
        todo!()
    }

    /// `lv_observer_get_target()`
    pub fn observer_get_target(&self, observer: ObserverId) -> ObserverTarget {
        todo!()
    }

    /// `lv_observer_get_target_obj()`
    pub fn observer_get_target_obj(&self, observer: ObserverId) -> Option<Obj> {
        todo!()
    }

    /// `lv_observer_get_user_data()`
    pub fn observer_get_user_data(&self, observer: ObserverId) -> Option<Rc<dyn Any>> {
        todo!()
    }

    /// `lv_observer_set_user_data()`
    pub fn observer_set_user_data(&mut self, observer: ObserverId, user_data: Option<Rc<dyn Any>>) {
        todo!()
    }

    /// `lv_subject_notify()`: notify all observers of `subject`.
    pub fn subject_notify(&mut self, subject: SubjectId) {
        todo!()
    }

    /// `lv_obj_add_subject_increment_event()`: increment (or decrement) an
    /// integer or float subject by `step` on `trigger`.
    pub fn obj_add_subject_increment_event(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        trigger: EventCode,
        step: i32,
    ) -> Option<SubjectIncrementDscRef> {
        todo!()
    }

    /// `lv_obj_set_subject_increment_event_min_value()`
    pub fn obj_set_subject_increment_event_min_value(
        &mut self,
        obj: Obj,
        dsc: &SubjectIncrementDscRef,
        min_value: i32,
    ) {
        todo!()
    }

    /// `lv_obj_set_subject_increment_event_max_value()`
    pub fn obj_set_subject_increment_event_max_value(
        &mut self,
        obj: Obj,
        dsc: &SubjectIncrementDscRef,
        max_value: i32,
    ) {
        todo!()
    }

    /// `lv_obj_set_subject_increment_event_rollover()`: what to do when the
    /// min/max value is crossed.
    pub fn obj_set_subject_increment_event_rollover(
        &mut self,
        obj: Obj,
        dsc: &SubjectIncrementDscRef,
        rollover: bool,
    ) {
        todo!()
    }

    /// `lv_obj_add_subject_toggle_event()`: toggle an integer subject
    /// between 0 and 1 on `trigger`.
    pub fn obj_add_subject_toggle_event(&mut self, obj: Obj, subject: SubjectId, trigger: EventCode) {
        todo!()
    }

    /// `lv_obj_add_subject_set_int_event()`: set an integer subject to
    /// `value` on `trigger`.
    pub fn obj_add_subject_set_int_event(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        trigger: EventCode,
        value: i32,
    ) {
        todo!()
    }

    /// `lv_obj_add_subject_set_float_event()`: set a float subject to
    /// `value` on `trigger`.
    #[cfg(feature = "float")]
    pub fn obj_add_subject_set_float_event(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        trigger: EventCode,
        value: f32,
    ) {
        todo!()
    }

    /// `lv_obj_add_subject_set_string_event()`: set a string subject to
    /// `value` on `trigger`.
    pub fn obj_add_subject_set_string_event(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        trigger: EventCode,
        value: &str,
    ) {
        todo!()
    }

    /// `lv_obj_bind_bool()`: call `set_bool_cb` with the subject's value on
    /// subscribing and whenever it changes.
    pub fn obj_bind_bool(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        set_bool_cb: ObjSetBoolCb,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_int()`: call `set_int_cb` with the subject's value on
    /// subscribing and whenever it changes.
    pub fn obj_bind_int(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        set_int_cb: ObjSetIntCb,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_float()`: call `set_float_cb` with the subject's value
    /// on subscribing and whenever it changes.
    #[cfg(feature = "float")]
    pub fn obj_bind_float(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        set_float_cb: ObjSetFloatCb,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_string()`: call `set_string_cb` with the subject's value
    /// on subscribing and whenever it changes.
    pub fn obj_bind_string(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        set_string_cb: ObjSetStringCb,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_color()`: call `set_color_cb` with the subject's value
    /// on subscribing and whenever it changes.
    pub fn obj_bind_color(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        set_color_cb: ObjSetColorCb,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_pointer()`: call `set_pointer_cb` with the subject's
    /// value on subscribing and whenever it changes.
    pub fn obj_bind_pointer(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        set_pointer_cb: ObjSetPointerCb,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_flag_if_eq()`: set `flag` if the integer subject equals
    /// `ref_value`, clear it otherwise.
    #[deprecated(note = "use `obj_bind_bool()` or `subject_add_observer_obj()` instead")]
    pub fn obj_bind_flag_if_eq(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        flag: ObjFlags,
        ref_value: i32,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_flag_if_not_eq()`: set `flag` if the integer subject is
    /// not equal to `ref_value`, clear it otherwise.
    #[deprecated(note = "use `obj_bind_bool()` or `subject_add_observer_obj()` instead")]
    pub fn obj_bind_flag_if_not_eq(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        flag: ObjFlags,
        ref_value: i32,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_flag_if_gt()`: set `flag` if the integer subject is
    /// greater than `ref_value`, clear it otherwise.
    #[deprecated(note = "use `obj_bind_bool()` or `subject_add_observer_obj()` instead")]
    pub fn obj_bind_flag_if_gt(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        flag: ObjFlags,
        ref_value: i32,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_flag_if_ge()`: set `flag` if the integer subject is
    /// greater than or equal to `ref_value`, clear it otherwise.
    #[deprecated(note = "use `obj_bind_bool()` or `subject_add_observer_obj()` instead")]
    pub fn obj_bind_flag_if_ge(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        flag: ObjFlags,
        ref_value: i32,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_flag_if_lt()`: set `flag` if the integer subject is less
    /// than `ref_value`, clear it otherwise.
    #[deprecated(note = "use `obj_bind_bool()` or `subject_add_observer_obj()` instead")]
    pub fn obj_bind_flag_if_lt(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        flag: ObjFlags,
        ref_value: i32,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_flag_if_le()`: set `flag` if the integer subject is less
    /// than or equal to `ref_value`, clear it otherwise.
    #[deprecated(note = "use `obj_bind_bool()` or `subject_add_observer_obj()` instead")]
    pub fn obj_bind_flag_if_le(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        flag: ObjFlags,
        ref_value: i32,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_state_if_eq()`: set `state` if the integer subject
    /// equals `ref_value`, clear it otherwise.
    #[deprecated(note = "use `obj_bind_bool()` or `subject_add_observer_obj()` instead")]
    pub fn obj_bind_state_if_eq(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        state: State,
        ref_value: i32,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_state_if_not_eq()`: set `state` if the integer subject
    /// is not equal to `ref_value`, clear it otherwise.
    #[deprecated(note = "use `obj_bind_bool()` or `subject_add_observer_obj()` instead")]
    pub fn obj_bind_state_if_not_eq(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        state: State,
        ref_value: i32,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_state_if_gt()`: set `state` if the integer subject is
    /// greater than `ref_value`, clear it otherwise.
    #[deprecated(note = "use `obj_bind_bool()` or `subject_add_observer_obj()` instead")]
    pub fn obj_bind_state_if_gt(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        state: State,
        ref_value: i32,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_state_if_ge()`: set `state` if the integer subject is
    /// greater than or equal to `ref_value`, clear it otherwise.
    #[deprecated(note = "use `obj_bind_bool()` or `subject_add_observer_obj()` instead")]
    pub fn obj_bind_state_if_ge(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        state: State,
        ref_value: i32,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_state_if_lt()`: set `state` if the integer subject is
    /// less than `ref_value`, clear it otherwise.
    #[deprecated(note = "use `obj_bind_bool()` or `subject_add_observer_obj()` instead")]
    pub fn obj_bind_state_if_lt(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        state: State,
        ref_value: i32,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_state_if_le()`: set `state` if the integer subject is
    /// less than or equal to `ref_value`, clear it otherwise.
    #[deprecated(note = "use `obj_bind_bool()` or `subject_add_observer_obj()` instead")]
    pub fn obj_bind_state_if_le(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        state: State,
        ref_value: i32,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_checked()`: keep an integer subject and the widget's
    /// `LV_STATE_CHECKED` in sync in both directions.
    pub fn obj_bind_checked(&mut self, obj: Obj, subject: SubjectId) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_style()` (`lv_obj_style.c`): add `style` with `selector`
    /// while the integer subject equals `ref_value`, remove it otherwise.
    pub fn obj_bind_style(
        &mut self,
        obj: Obj,
        style: StyleId,
        selector: StyleSelector,
        subject: SubjectId,
        ref_value: i32,
    ) -> Option<ObserverId> {
        todo!()
    }

    /// `lv_obj_bind_style_prop()` (`lv_obj_style.c`): set the local style
    /// property `prop` with `selector` from the subject's value.
    pub fn obj_bind_style_prop(
        &mut self,
        obj: Obj,
        prop: StyleProp,
        selector: StyleSelector,
        subject: SubjectId,
    ) -> Option<ObserverId> {
        todo!()
    }
}
