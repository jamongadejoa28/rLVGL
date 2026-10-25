//! Ported from LVGL v9.6.0: `src/core/lv_observer.c`.
//!
//! Also covers `include/lvgl/core/lv_observer.h`,
//! `src/core/lv_observer_private.h` and the observer-based style bindings of
//! `src/core/lv_obj_style.c` (`lv_obj_bind_style()`,
//! `lv_obj_bind_style_prop()`). The widget bindings (`lv_bar_bind_value()`,
//! `lv_label_bind_text()`, ...) live in the widget crate.
//!
//! Ownership: subjects and observers live in arenas of [`ObserverState`]
//! (`Ui::observer`, the `subject_ll` of `lv_global_t` plus each subject's
//! `subs_ll`) and are addressed by `Copy` handles. `lv_subject_create()`
//! inserts at the tail of `subject_ll`, `lv_subject_add_observer*()` at the
//! tail of `subs_ll`; the `*_order` / `subs` vectors keep those orders.
//! Callbacks are `Rc<dyn Fn(&mut Ui, ..)>`, cloned out of the arena before
//! they are called; anything that runs a callback re-checks the handles
//! afterwards.
//!
//! Pointer identity: upstream passes the `lv_subject_t *` / `lv_observer_t *`
//! as event `user_data` and finds the descriptors again by comparing those
//! pointers (`lv_obj_remove_event_cb_with_user_data(obj, NULL, subject)`).
//! Every subject and observer here owns one canonical `Rc<dyn Any>` playing
//! the role of that pointer ([`Ui::subject_as_user_data`],
//! [`SubjectId::from_user_data`]), compared with `Rc::ptr_eq`. The static
//! callbacks that upstream compares by function pointer
//! (`unsubscribe_on_delete_cb`, `group_notify_cb`) are created once per
//! [`ObserverState`] so their `Rc`s compare the same way.
//!
//! String subjects own their buffers (`lv_subject_set_string_buffer_static()`
//! takes a size instead of caller buffers); pointer subjects hold an
//! `Rc<dyn Any>` compared by identity like the C pointer. `lv_strlcpy()` /
//! `lv_vsnprintf()` truncation to `size - 1` bytes backs off to the previous
//! UTF-8 character boundary, because a Rust `String` cannot hold a split
//! character.
//!
//! A `static lv_subject_t s;` initialized with the deprecated
//! `lv_subject_init_*()` functions is storage the caller owns, outside of
//! `subject_ll`; [`Ui::subject_alloc_static`] provides that storage.

use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::cell::RefCell;

use crate::core::obj::{Obj, ObjFlags};
use crate::core::obj_style::StyleId;
use crate::core::ui::Ui;
use crate::misc::arena::{Arena, Key};
use crate::misc::check_arg_failed;
use crate::misc::color::Color;
use crate::misc::event::{Event, EventCallback, EventCode, event_free_user_data_cb};
use crate::misc::style::{State, StyleProp, StyleSelector, StyleValue};

/// Handle of a subject (`lv_subject_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubjectId(pub Key);

impl SubjectId {
    /// The subject whose `lv_subject_t *` is `data` (the reverse of
    /// [`Ui::subject_as_user_data`]): `(lv_subject_t *)lv_event_get_user_data(e)`
    /// of the event callbacks that are registered with the subject as user
    /// data. `None` if `data` is not a subject pointer.
    #[must_use]
    pub fn from_user_data(data: &Rc<dyn Any>) -> Option<SubjectId> {
        (**data).downcast_ref::<SubjectPtr>().map(|p| p.0)
    }
}

/// Handle of an observer (`lv_observer_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObserverId(pub Key);

/// The payload of a subject's canonical pointer `Rc`.
struct SubjectPtr(SubjectId);

/// The payload of an observer's canonical pointer `Rc`.
struct ObserverPtr(ObserverId);

/// `size : 24` of `lv_subject_t`: assignments keep the low 24 bits.
const SUBJECT_SIZE_MASK: u32 = 0x00FF_FFFF;

/// `lv_subject_type_t`: values of the subject's `type` field, with the
/// upstream discriminants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
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
#[non_exhaustive]
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

impl SubjectValue {
    /// `value.num` (0 for the zeroed union or another member).
    #[must_use]
    pub fn num(&self) -> i32 {
        match self {
            SubjectValue::Num(n) => *n,
            _ => 0,
        }
    }

    /// `value.float_v` (0 for the zeroed union or another member).
    #[cfg(feature = "float")]
    #[must_use]
    pub fn float_v(&self) -> f32 {
        match self {
            SubjectValue::Float(v) => *v,
            _ => 0.0,
        }
    }

    /// `(int32_t)value.float_v`: the cast of the `#if LV_USE_FLOAT` branches
    /// of the widget bindings (`bar_value_observer_cb()`, ...), usable from
    /// crates that do not see the `float` feature. 0 for the zeroed union,
    /// another member or without `LV_USE_FLOAT` (no float subject can exist
    /// then).
    #[must_use]
    pub fn float_v_as_i32(&self) -> i32 {
        match self {
            #[cfg(feature = "float")]
            SubjectValue::Float(v) => *v as i32,
            _ => 0,
        }
    }

    /// `(double)value.float_v`: the default argument promotion of
    /// `lv_label_set_text_fmt(obj, fmt, subject->value.float_v)` in
    /// `label_text_observer_cb()`, usable from crates that do not see the
    /// `float` feature. 0 for the zeroed union, another member or without
    /// `LV_USE_FLOAT`.
    #[must_use]
    pub fn float_v_as_f64(&self) -> f64 {
        match self {
            #[cfg(feature = "float")]
            SubjectValue::Float(v) => *v as f64,
            _ => 0.0,
        }
    }

    /// `value.color` (black for the zeroed union or another member).
    #[must_use]
    pub fn color(&self) -> Color {
        match self {
            SubjectValue::Color(c) => *c,
            _ => Color::black(),
        }
    }

    /// `value.pointer` of a pointer subject (`NULL` otherwise).
    #[must_use]
    pub fn pointer(&self) -> Option<Rc<dyn Any>> {
        match self {
            SubjectValue::Pointer(p) => p.clone(),
            _ => None,
        }
    }

    /// `value.pointer` of a string subject (`NULL` otherwise).
    #[must_use]
    pub fn str(&self) -> Option<&str> {
        match self {
            SubjectValue::Str(s) => s.as_deref(),
            _ => None,
        }
    }

    /// `value.pointer` of a group subject (`NULL` otherwise).
    #[must_use]
    pub fn group(&self) -> Option<Rc<[SubjectId]>> {
        match self {
            SubjectValue::Group(g) => g.clone(),
            _ => None,
        }
    }
}

/// The `free_cb` of a subject's `lv_ext_data_t` (`void (*)(void * data)`).
pub type SubjectExtDataFreeCb = fn(Option<Rc<dyn Any>>);

/// `lv_ext_data_t` of a subject (`LV_USE_EXT_DATA`), set by
/// `lv_subject_set_external_data()`.
#[derive(Debug, Default)]
pub struct SubjectExtData {
    /// `data`
    pub data: Option<Rc<dyn Any>>,
    /// `free_cb`
    pub free_cb: Option<SubjectExtDataFreeCb>,
}

/// `lv_subject_t`: an observable value.
#[derive(Debug, Default)]
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
    /// The canonical `lv_subject_t *` (not touched by `lv_memzero`).
    ptr: Option<Rc<dyn Any>>,
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
#[non_exhaustive]
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

impl core::fmt::Debug for ObserverUserCb {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let kind = match self {
            Self::Bool(_) => "Bool",
            Self::Int(_) => "Int",
            #[cfg(feature = "float")]
            Self::Float(_) => "Float",
            Self::String(_) => "String",
            Self::Color(_) => "Color",
            Self::Pointer(_) => "Pointer",
        };
        f.debug_tuple(kind).field(&crate::misc::OpaqueCallback).finish()
    }
}

/// `void * target` of an observer; `for_obj` is the [`ObserverTarget::Obj`]
/// variant.
#[derive(Debug, Clone, Default)]
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
    /// The canonical `lv_observer_t *`.
    ptr: Option<Rc<dyn Any>>,
}

impl core::fmt::Debug for Observer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Observer")
            .field("subject", &self.subject)
            .field("cb", &self.cb.as_ref().map(|_| crate::misc::OpaqueCallback))
            .field("target", &self.target)
            .field("user_data", &self.user_data)
            .field("user_cb", &self.user_cb)
            .field("auto_free_user_data", &self.auto_free_user_data)
            .field("notified", &self.notified)
            .field("for_obj", &self.for_obj)
            .field("ptr", &self.ptr)
            .finish()
    }
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
    /// The function pointer `unsubscribe_on_delete_cb`.
    unsubscribe_on_delete_cb: Option<EventCallback>,
    /// The function pointer `group_notify_cb`.
    group_notify_cb: Option<ObserverCb>,
}

impl core::fmt::Debug for ObserverState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ObserverState")
            .field("subjects", &self.subjects)
            .field("subject_order", &self.subject_order)
            .field("observers", &self.observers)
            .field(
                "unsubscribe_on_delete_cb",
                &self
                    .unsubscribe_on_delete_cb
                    .as_ref()
                    .map(|_| crate::misc::OpaqueCallback),
            )
            .field(
                "group_notify_cb",
                &self.group_notify_cb.as_ref().map(|_| crate::misc::OpaqueCallback),
            )
            .finish()
    }
}

impl ObserverState {
    /// `lv_subject_global_init()`: `lv_ll_init(&subject_list, ...)`, an empty
    /// list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            subjects: Arena::new(),
            subject_order: Vec::new(),
            observers: Arena::new(),
            unsubscribe_on_delete_cb: None,
            group_notify_cb: None,
        }
    }
}

/*********************
 *      TYPEDEFS
 *********************/

/// `flag_cond_t`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum FlagCond {
    /// `FLAG_COND_EQ`
    Eq = 0,
    /// `FLAG_COND_GT`
    Gt = 1,
    /// `FLAG_COND_GE`
    Ge = 2,
}

/// `flag_and_cond_t`
struct FlagAndCond {
    /// `flag`
    flag: u32,
    /// `value.num`
    value: i32,
    /// `inv : 1`
    inv: bool,
    /// `cond : 3`
    cond: FlagCond,
}

/// `subject_set_int_user_data_t`
struct SubjectSetIntUserData {
    subject: SubjectId,
    value: i32,
}

/// `subject_set_float_user_data_t`
#[cfg(feature = "float")]
struct SubjectSetFloatUserData {
    subject: SubjectId,
    value: f32,
}

/// `subject_set_string_user_data_t`
struct SubjectSetStringUserData {
    subject: SubjectId,
    value: String,
}

/// `bind_style_t` (`lv_obj_style.c`)
struct BindStyle {
    style: StyleId,
    selector: StyleSelector,
    value: i32,
}

/// `bind_style_prop_t` (`lv_obj_style.c`)
struct BindStyleProp {
    prop: StyleProp,
    selector: StyleSelector,
}

/// `LV_MIN(a, b)`
#[inline]
fn lv_min<T: PartialOrd>(a: T, b: T) -> T {
    if a < b { a } else { b }
}

/// `LV_MAX(a, b)`
#[inline]
fn lv_max<T: PartialOrd>(a: T, b: T) -> T {
    if a > b { a } else { b }
}

/// `LV_CLAMP(min, val, max)`: `LV_MAX(min, LV_MIN(val, max))`.
#[inline]
fn lv_clamp<T: PartialOrd>(min: T, val: T, max: T) -> T {
    lv_max(min, lv_min(val, max))
}

/// `lv_strlcpy(dst, src, size)` into a fresh buffer: at most `size - 1`
/// bytes of `src` (backing off to a character boundary).
fn strlcpy(src: &str, size: usize) -> String {
    let max = size.saturating_sub(1);
    if src.len() <= max {
        return String::from(src);
    }
    let mut end = max;
    while !src.is_char_boundary(end) {
        end -= 1;
    }
    String::from(&src[..end])
}

/// `a == b` for two nullable pointers.
fn opt_ptr_eq(a: Option<&Rc<dyn Any>>, b: Option<&Rc<dyn Any>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

/// `LV_USE_FLOAT`: whether float subjects (and the `#if LV_USE_FLOAT`
/// branches of the widget bindings) are compiled in, for crates that do not
/// see the `float` feature.
pub const USE_FLOAT: bool = cfg!(feature = "float");

/// `LV_LOG_WARN`, shared with the widget bindings.
#[inline]
pub fn log_warn(msg: &str) {
    #[cfg(feature = "log")]
    log::warn!("{msg}");
    #[cfg(not(feature = "log"))]
    let _ = msg;
}

/// The observer whose `lv_observer_t *` is `data`.
fn observer_from_user_data(data: Option<&Rc<dyn Any>>) -> Option<ObserverId> {
    data.and_then(|d| (**d).downcast_ref::<ObserverPtr>())
        .map(|p| p.0)
}

impl Ui {
    /// `lv_subject_global_deinit()`: deinit the group subjects first, then
    /// every other subject, and clear `subject_ll`. Called by `Ui::deinit()`
    /// at the `lv_deinit()` position.
    pub(crate) fn subject_global_deinit(&mut self) {
        /* always deinit group subjects first as they will
         * dereference other subjects during deinitialization */
        let list = self.observer.subject_order.clone();
        for &curr in &list {
            if self.subject_type(curr) == Some(SubjectType::Group) {
                self.deinit_subject(curr);
            }
        }

        for &curr in &list {
            if self.subject_type(curr).is_some_and(|t| t != SubjectType::Group) {
                self.deinit_subject(curr);
            }
        }
        /*lv_ll_clear(&subject_list)*/
        let list = core::mem::take(&mut self.observer.subject_order);
        for curr in list {
            self.observer.subjects.remove(curr.0);
        }
    }

    /// Object deletion hook, called by `lv_obj_delete()`'s core
    /// (`obj_tree_delete_core`) just before the object's slot is freed.
    ///
    /// Nothing to do: like upstream, widget observers are removed by the
    /// `LV_EVENT_DELETE` callback `unsubscribe_on_delete_cb` registered by
    /// `lv_subject_add_observer_obj()`.
    pub(crate) fn observer_obj_deleted(&mut self, obj: Obj) {
        let _ = obj;
    }

    /// `subject->type`, `None` for a stale handle.
    fn subject_type(&self, subject: SubjectId) -> Option<SubjectType> {
        self.observer.subjects.get(subject.0).map(|s| s.type_)
    }

    /// The canonical `lv_subject_t *` of `subject`, used as the `user_data`
    /// of the event callbacks registered with the subject as user data, so
    /// `lv_obj_remove_event_cb_with_user_data(obj, NULL, subject)` finds
    /// them. `None` for a stale handle.
    #[must_use]
    pub fn subject_as_user_data(&self, subject: SubjectId) -> Option<Rc<dyn Any>> {
        self.observer.subjects.get(subject.0).and_then(|s| s.ptr.clone())
    }

    /// The canonical `lv_observer_t *` of `observer`.
    fn observer_ptr(&self, observer: ObserverId) -> Option<Rc<dyn Any>> {
        self.observer
            .observers
            .get(observer.0)
            .and_then(|o| o.ptr.clone())
    }

    /// The `Rc` of the function `unsubscribe_on_delete_cb`.
    fn unsubscribe_on_delete_cb_rc(&mut self) -> EventCallback {
        self.observer
            .unsubscribe_on_delete_cb
            .get_or_insert_with(|| Rc::new(unsubscribe_on_delete_cb))
            .clone()
    }

    /// The `Rc` of the function `group_notify_cb`.
    fn group_notify_cb_rc(&mut self) -> ObserverCb {
        self.observer
            .group_notify_cb
            .get_or_insert_with(|| Rc::new(group_notify_cb))
            .clone()
    }

    /// Storage of a caller-owned `lv_subject_t` (`static lv_subject_t s;`):
    /// a zeroed `LV_SUBJECT_TYPE_INVALID` subject that is not in
    /// `subject_ll`, to be initialized with the deprecated
    /// `lv_subject_init_*()` functions and released with
    /// `lv_subject_deinit()`. Its slot lives as long as the [`Ui`], like a
    /// static.
    #[must_use]
    pub fn subject_alloc_static(&mut self) -> SubjectId {
        let key = self.observer.subjects.insert(Subject::default());
        let id = SubjectId(key);
        self.observer.subjects[key].ptr = Some(Rc::new(SubjectPtr(id)));
        id
    }

    /// `lv_subject_create()`: create a subject of `type_`. `None` (`NULL`)
    /// for `Invalid`/`None`, or `Float` without `LV_USE_FLOAT`.
    #[must_use]
    pub fn subject_create(&mut self, type_: SubjectType) -> Option<SubjectId> {
        if !(type_ != SubjectType::Invalid && type_ != SubjectType::None) {
            check_arg_failed("lv_subject_create: type != INVALID && type != NONE");
            return None;
        }
        if !(cfg!(feature = "float") || type_ != SubjectType::Float) {
            check_arg_failed("lv_subject_create: LV_USE_FLOAT || type != FLOAT");
            return None;
        }
        /*lv_ll_ins_tail(&subject_list)*/
        let subject = self.subject_alloc_static();
        self.observer.subject_order.push(subject);

        self.init_common(subject);

        match type_ {
            SubjectType::Int => self.init_int(subject, 0),
            SubjectType::Float => {
                #[cfg(feature = "float")]
                self.init_float(subject, 0.0);
            }
            SubjectType::Pointer => self.init_pointer(subject, None),
            SubjectType::Color => self.init_color(subject, Color::black()),
            SubjectType::Group => self.init_group(subject, None),
            SubjectType::String => self.init_string(subject, 0, false, ""),
            SubjectType::Invalid | SubjectType::None => unreachable!(),
        }
        Some(subject)
    }

    /// `lv_subject_delete()`: deinit the subject and remove it from
    /// `subject_ll`. `None` is ignored.
    pub fn subject_delete(&mut self, subject: Option<SubjectId>) {
        let Some(subject) = subject else {
            return;
        };
        if !self.observer.subjects.contains(subject.0) {
            return;
        }
        /* TODO: we can check here if this subject is
         * referenced by a group subject and remove it from the list*/

        self.deinit_subject(subject);
        if let Some(pos) = self.observer.subject_order.iter().position(|&s| s == subject) {
            self.observer.subject_order.remove(pos);
        }
        self.observer.subjects.remove(subject.0);
    }

    /// `lv_subject_set_external_data()` (`LV_USE_EXT_DATA`): attach external
    /// data with a destructor called when the subject is deinitialized.
    pub fn subject_set_external_data(
        &mut self,
        subject: SubjectId,
        data: Option<Rc<dyn Any>>,
        free_cb: Option<SubjectExtDataFreeCb>,
    ) {
        let Some(s) = self.observer.subjects.get_mut(subject.0) else {
            check_arg_failed("lv_subject_set_external_data: subject != NULL");
            return;
        };

        s.ext_data.data = data;
        s.ext_data.free_cb = free_cb;
    }

    /// `lv_subject_init_int()`: initialize an integer subject in place.
    #[deprecated(note = "the subject init API is deprecated, use `subject_create` instead")]
    pub fn subject_init_int(&mut self, subject: SubjectId, value: i32) {
        if !self.observer.subjects.contains(subject.0) {
            check_arg_failed("lv_subject_init_int: subject != NULL");
            return;
        }
        self.init_common(subject);
        self.init_int(subject, value);
    }

    /// `LV_CHECK_ARG(subject != NULL)` + `LV_CHECK_ARG(subject->type == type_)`.
    fn subject_check_type(&self, subject: SubjectId, type_: SubjectType, func: &str) -> bool {
        let Some(s) = self.observer.subjects.get(subject.0) else {
            check_arg_failed(func);
            return false;
        };
        if s.type_ != type_ {
            check_arg_failed(func);
            return false;
        }
        true
    }

    /// `lv_subject_set_int()`: set the value of an integer subject and
    /// notify the observers.
    pub fn subject_set_int(&mut self, subject: SubjectId, value: i32) {
        if !self.subject_check_type(subject, SubjectType::Int, "lv_subject_set_int") {
            return;
        }
        let s = &mut self.observer.subjects[subject.0];

        let value = lv_clamp(s.min_value.num(), value, s.max_value.num());

        s.prev_value = SubjectValue::Num(s.value.num());
        s.value = SubjectValue::Num(value);
        self.subject_notify_if_changed(subject);
    }

    /// `lv_subject_get_int()`
    #[must_use]
    pub fn subject_get_int(&self, subject: SubjectId) -> i32 {
        if !self.subject_check_type(subject, SubjectType::Int, "lv_subject_get_int") {
            return 0;
        }

        self.observer.subjects[subject.0].value.num()
    }

    /// `lv_subject_get_previous_int()`
    #[must_use]
    pub fn subject_get_previous_int(&self, subject: SubjectId) -> i32 {
        if !self.subject_check_type(subject, SubjectType::Int, "lv_subject_get_previous_int") {
            return 0;
        }

        self.observer.subjects[subject.0].prev_value.num()
    }

    /// `lv_subject_set_min_value_int()`
    pub fn subject_set_min_value_int(&mut self, subject: SubjectId, min_value: i32) {
        if !self.subject_check_type(subject, SubjectType::Int, "lv_subject_set_min_value_int") {
            return;
        }

        self.observer.subjects[subject.0].min_value = SubjectValue::Num(min_value);
    }

    /// `lv_subject_set_max_value_int()`
    pub fn subject_set_max_value_int(&mut self, subject: SubjectId, max_value: i32) {
        if !self.subject_check_type(subject, SubjectType::Int, "lv_subject_set_max_value_int") {
            return;
        }

        self.observer.subjects[subject.0].max_value = SubjectValue::Num(max_value);
    }

    /// `lv_subject_init_float()`: initialize a float subject in place.
    #[cfg(feature = "float")]
    #[deprecated(note = "the subject init API is deprecated, use `subject_create` instead")]
    pub fn subject_init_float(&mut self, subject: SubjectId, value: f32) {
        if !self.observer.subjects.contains(subject.0) {
            check_arg_failed("lv_subject_init_float: subject != NULL");
            return;
        }

        self.init_common(subject);
        self.init_float(subject, value);
    }

    /// `lv_subject_set_float()`: set the value of a float subject and notify
    /// the observers.
    #[cfg(feature = "float")]
    pub fn subject_set_float(&mut self, subject: SubjectId, value: f32) {
        if !self.subject_check_type(subject, SubjectType::Float, "lv_subject_set_float") {
            return;
        }
        let s = &mut self.observer.subjects[subject.0];

        let value = lv_clamp(s.min_value.float_v(), value, s.max_value.float_v());

        s.prev_value = SubjectValue::Float(s.value.float_v());
        s.value = SubjectValue::Float(value);
        self.subject_notify_if_changed(subject);
    }

    /// `lv_subject_set_float(subject, (float)value)`: the `#if LV_USE_FLOAT`
    /// branch of the widget bindings' value-changed callbacks
    /// (`slider_value_changed_event_cb()`, ...), usable from crates that do
    /// not see the `float` feature. Without `LV_USE_FLOAT` no float subject
    /// can exist and this does nothing.
    pub fn subject_set_float_from_i32(&mut self, subject: SubjectId, value: i32) {
        #[cfg(feature = "float")]
        self.subject_set_float(subject, value as f32);
        #[cfg(not(feature = "float"))]
        let _ = (subject, value);
    }

    /// `lv_subject_get_float()`
    #[cfg(feature = "float")]
    #[must_use]
    pub fn subject_get_float(&self, subject: SubjectId) -> f32 {
        if !self.subject_check_type(subject, SubjectType::Float, "lv_subject_get_float") {
            return 0.0;
        }

        self.observer.subjects[subject.0].value.float_v()
    }

    /// `lv_subject_get_previous_float()`
    #[cfg(feature = "float")]
    #[must_use]
    pub fn subject_get_previous_float(&self, subject: SubjectId) -> f32 {
        if !self.subject_check_type(subject, SubjectType::Float, "lv_subject_get_previous_float") {
            return 0.0;
        }

        self.observer.subjects[subject.0].prev_value.float_v()
    }

    /// `lv_subject_set_min_value_float()`
    #[cfg(feature = "float")]
    pub fn subject_set_min_value_float(&mut self, subject: SubjectId, min_value: f32) {
        if !self.subject_check_type(subject, SubjectType::Float, "lv_subject_set_min_value_float") {
            return;
        }

        self.observer.subjects[subject.0].min_value = SubjectValue::Float(min_value);
    }

    /// `lv_subject_set_max_value_float()`
    #[cfg(feature = "float")]
    pub fn subject_set_max_value_float(&mut self, subject: SubjectId, max_value: f32) {
        if !self.subject_check_type(subject, SubjectType::Float, "lv_subject_set_max_value_float") {
            return;
        }

        self.observer.subjects[subject.0].max_value = SubjectValue::Float(max_value);
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
        if !self.observer.subjects.contains(subject.0) {
            check_arg_failed("lv_subject_init_string: subject != NULL");
            return;
        }
        /*`buf != NULL || size == 0` and `value != NULL` always hold*/

        self.init_common(subject);
        self.init_string(subject, size, track_previous, value);
    }

    /// `lv_subject_set_string_buffer_static()`: give a string subject owned
    /// buffers of `size` bytes (`track_previous` = non-`NULL` `prev_buf`).
    pub fn subject_set_string_buffer_static(
        &mut self,
        subject: SubjectId,
        size: usize,
        track_previous: bool,
    ) {
        if !self.subject_check_type(
            subject,
            SubjectType::String,
            "lv_subject_set_string_buffer_static",
        ) {
            return;
        }
        if size == 0 {
            check_arg_failed("lv_subject_set_string_buffer_static: size > 0");
            return;
        }

        /*buf[0] = '\0'; if(prev_buf) prev_buf[0] = '\0';*/
        let s = &mut self.observer.subjects[subject.0];
        s.size = (size as u32) & SUBJECT_SIZE_MASK;
        s.value = SubjectValue::Str(Some(String::new()));
        s.prev_value = SubjectValue::Str(track_previous.then(String::new));
    }

    /// `lv_subject_set_string()`: copy `buf` (`lv_strlcpy` into `size`
    /// bytes) and notify the observers if it changed.
    pub fn subject_set_string(&mut self, subject: SubjectId, buf: &str) {
        if !self.subject_check_type(subject, SubjectType::String, "lv_subject_set_string") {
            return;
        }
        let s = &mut self.observer.subjects[subject.0];

        if s.size == 0 {
            return;
        }
        let size = s.size as usize;
        if matches!(s.prev_value, SubjectValue::Str(Some(_))) {
            let cur = strlcpy(s.value.str().unwrap_or(""), size);
            s.prev_value = SubjectValue::Str(Some(cur));
        }

        s.value = SubjectValue::Str(Some(strlcpy(buf, size)));

        self.subject_notify_if_changed(subject);
    }

    /// `lv_subject_snprintf()`: format a new string into the subject and
    /// notify the observers if it changed.
    pub fn subject_snprintf(&mut self, subject: SubjectId, args: core::fmt::Arguments<'_>) {
        if !self.subject_check_type(subject, SubjectType::String, "lv_subject_snprintf") {
            return;
        }
        let s = &mut self.observer.subjects[subject.0];

        if s.size < 1 {
            return;
        }
        let size = s.size as usize;

        if matches!(s.prev_value, SubjectValue::Str(Some(_))) {
            let cur = strlcpy(s.value.str().unwrap_or(""), size);
            s.prev_value = SubjectValue::Str(Some(cur));
        }

        let formatted = alloc::fmt::format(args);
        s.value = SubjectValue::Str(Some(strlcpy(&formatted, size)));

        self.subject_notify_if_changed(subject);
    }

    /// `lv_subject_get_string()`: `None` is `NULL`.
    #[must_use]
    pub fn subject_get_string(&self, subject: SubjectId) -> Option<&str> {
        if !self.subject_check_type(subject, SubjectType::String, "lv_subject_get_string") {
            return None;
        }

        self.observer.subjects[subject.0].value.str()
    }

    /// `lv_subject_get_previous_string()`: `None` is `NULL`.
    #[must_use]
    pub fn subject_get_previous_string(&self, subject: SubjectId) -> Option<&str> {
        if !self.subject_check_type(subject, SubjectType::String, "lv_subject_get_previous_string") {
            return None;
        }

        self.observer.subjects[subject.0].prev_value.str()
    }

    /// `lv_subject_init_pointer()`: initialize a pointer subject in place.
    #[deprecated(note = "the subject init API is deprecated, use `subject_create` instead")]
    pub fn subject_init_pointer(&mut self, subject: SubjectId, value: Option<Rc<dyn Any>>) {
        if !self.observer.subjects.contains(subject.0) {
            check_arg_failed("lv_subject_init_pointer: subject != NULL");
            return;
        }

        /*lv_memzero + lv_ll_init(&subject->subs_ll)*/
        self.init_common(subject);
        self.init_pointer(subject, value);
    }

    /// `lv_subject_set_pointer()`: set the value of a pointer subject and
    /// notify the observers (regardless of whether it changed).
    pub fn subject_set_pointer(&mut self, subject: SubjectId, ptr: Option<Rc<dyn Any>>) {
        if !self.subject_check_type(subject, SubjectType::Pointer, "lv_subject_set_pointer") {
            return;
        }
        let s = &mut self.observer.subjects[subject.0];

        s.prev_value = SubjectValue::Pointer(s.value.pointer());
        s.value = SubjectValue::Pointer(ptr);
        self.subject_notify_if_changed(subject);
    }

    /// `lv_subject_get_pointer()`
    #[must_use]
    pub fn subject_get_pointer(&self, subject: SubjectId) -> Option<Rc<dyn Any>> {
        if !self.subject_check_type(subject, SubjectType::Pointer, "lv_subject_get_pointer") {
            return None;
        }

        self.observer.subjects[subject.0].value.pointer()
    }

    /// `lv_subject_get_previous_pointer()`
    #[must_use]
    pub fn subject_get_previous_pointer(&self, subject: SubjectId) -> Option<Rc<dyn Any>> {
        if !self.subject_check_type(subject, SubjectType::Pointer, "lv_subject_get_previous_pointer") {
            return None;
        }

        self.observer.subjects[subject.0].prev_value.pointer()
    }

    /// `lv_subject_init_color()`: initialize a color subject in place.
    #[deprecated(note = "the subject init API is deprecated, use `subject_create` instead")]
    pub fn subject_init_color(&mut self, subject: SubjectId, color: Color) {
        if !self.observer.subjects.contains(subject.0) {
            check_arg_failed("lv_subject_init_color: subject != NULL");
            return;
        }

        /*lv_memzero + lv_ll_init(&subject->subs_ll)*/
        self.init_common(subject);
        self.init_color(subject, color);
    }

    /// `lv_subject_set_color()`: set the value of a color subject and notify
    /// the observers if it changed.
    pub fn subject_set_color(&mut self, subject: SubjectId, color: Color) {
        if !self.subject_check_type(subject, SubjectType::Color, "lv_subject_set_color") {
            return;
        }
        let s = &mut self.observer.subjects[subject.0];

        s.prev_value = SubjectValue::Color(s.value.color());
        s.value = SubjectValue::Color(color);
        self.subject_notify_if_changed(subject);
    }

    /// `lv_subject_get_color()`
    #[must_use]
    pub fn subject_get_color(&self, subject: SubjectId) -> Color {
        if !self.subject_check_type(subject, SubjectType::Color, "lv_subject_get_color") {
            return Color::black();
        }

        self.observer.subjects[subject.0].value.color()
    }

    /// `lv_subject_get_previous_color()`
    #[must_use]
    pub fn subject_get_previous_color(&self, subject: SubjectId) -> Color {
        if !self.subject_check_type(subject, SubjectType::Color, "lv_subject_get_previous_color") {
            return Color::black();
        }

        self.observer.subjects[subject.0].prev_value.color()
    }

    /// `lv_subject_init_group()`: initialize a group subject in place;
    /// observers of `group_subject` are notified when any of `list` changes.
    #[deprecated(note = "the subject init API is deprecated, use `subject_create` instead")]
    pub fn subject_init_group(&mut self, group_subject: SubjectId, list: Rc<[SubjectId]>) {
        if !self.observer.subjects.contains(group_subject.0) {
            check_arg_failed("lv_subject_init_group: subject != NULL");
            return;
        }
        /*`list != NULL` and `list[i] != NULL` always hold*/

        self.init_common(group_subject);
        self.init_group(group_subject, Some(list));
    }

    /// `lv_subject_set_group_list_static()`: set the subject list of a group
    /// subject (`list_len` is `list.len()`).
    pub fn subject_set_group_list_static(&mut self, subject: SubjectId, list: Rc<[SubjectId]>) {
        if !self.subject_check_type(subject, SubjectType::Group, "lv_subject_set_group_list_static") {
            return;
        }
        /*`list != NULL` and `list[i] != NULL` always hold*/

        self.remove_observers_from_group_subject(subject);
        self.add_observers_to_group_subject(subject, Some(list));
    }

    /// `lv_subject_deinit()`: remove all observers and the bound widget
    /// events. `None` is ignored.
    #[deprecated(note = "the subject init API is deprecated, use `subject_create`/`subject_delete` instead")]
    pub fn subject_deinit(&mut self, subject: Option<SubjectId>) {
        let Some(subject) = subject else {
            return;
        };
        if !self.observer.subjects.contains(subject.0) {
            return;
        }
        self.deinit_subject(subject);
    }

    /// `lv_subject_get_group_element()`: `None` if `index` is out of bounds.
    #[must_use]
    pub fn subject_get_group_element(&self, subject: SubjectId, index: i32) -> Option<SubjectId> {
        if !self.subject_check_type(subject, SubjectType::Group, "lv_subject_get_group_element") {
            return None;
        }
        let s = &self.observer.subjects[subject.0];
        if !(index >= 0 && (index as u32) < s.size) {
            check_arg_failed("lv_subject_get_group_element: index >= 0 && index < subject->size");
            return None;
        }

        s.value.group().and_then(|list| list.get(index as usize).copied())
    }

    /// `lv_subject_add_observer()`: subscribe `observer_cb`; it is called
    /// once immediately.
    pub fn subject_add_observer(
        &mut self,
        subject: SubjectId,
        observer_cb: ObserverCb,
        user_data: Option<Rc<dyn Any>>,
    ) -> Option<ObserverId> {
        if !self.observer.subjects.contains(subject.0) {
            check_arg_failed("lv_subject_add_observer: subject != NULL");
            return None;
        }

        let observer = self.subject_add_observer_obj(subject, observer_cb, None, user_data)?;

        if let Some(o) = self.observer.observers.get_mut(observer.0) {
            o.for_obj = false;
        }
        Some(observer)
    }

    /// `lv_ll_ins_tail(&subject->subs_ll)` + `lv_memzero(observer)`.
    fn observer_ins_tail(&mut self, subject: SubjectId) -> ObserverId {
        let key = self.observer.observers.insert(Observer {
            subject,
            cb: None,
            target: ObserverTarget::None,
            user_data: None,
            user_cb: None,
            auto_free_user_data: false,
            notified: false,
            for_obj: false,
            ptr: None,
        });
        let id = ObserverId(key);
        self.observer.observers[key].ptr = Some(Rc::new(ObserverPtr(id)));
        self.observer.subjects[subject.0].subs.push(id);
        id
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
        let Some(s) = self.observer.subjects.get(subject.0) else {
            check_arg_failed("lv_subject_add_observer_obj: subject != NULL");
            return None;
        };
        if s.type_ == SubjectType::Invalid {
            check_arg_failed("lv_subject_add_observer_obj: subject->type != LV_SUBJECT_TYPE_INVALID");
            return None;
        }

        let observer = self.observer_ins_tail(subject);

        {
            let o = &mut self.observer.observers[observer.0];
            o.subject = subject;
            o.cb = Some(observer_cb.clone());
            o.user_data = user_data;
            o.target = obj.map_or(ObserverTarget::None, ObserverTarget::Obj);
            o.for_obj = true;
        }
        /* subscribe to delete event of the object */
        if let Some(obj) = obj {
            let cb = self.unsubscribe_on_delete_cb_rc();
            let ptr = self.observer_ptr(observer);
            self.obj_add_event_cb(obj, cb, EventCode::DELETE, ptr);
        }

        /* Update Observer immediately. */
        observer_cb(self, observer, subject);

        Some(observer)
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
        let Some(s) = self.observer.subjects.get(subject.0) else {
            check_arg_failed("lv_subject_add_observer_with_target: subject != NULL");
            return None;
        };
        if s.type_ == SubjectType::Invalid {
            check_arg_failed("lv_subject_add_observer_with_target: subject->type != LV_SUBJECT_TYPE_INVALID");
            return None;
        }

        let observer = self.observer_ins_tail(subject);

        {
            let o = &mut self.observer.observers[observer.0];
            o.subject = subject;
            o.cb = Some(observer_cb.clone());
            o.user_data = user_data;
            o.target = target.map_or(ObserverTarget::None, ObserverTarget::Other);
        }

        /* Update Observer immediately. */
        observer_cb(self, observer, subject);

        Some(observer)
    }

    /// `lv_observer_delete()`: remove the observer from its subject. `None`
    /// is ignored.
    pub fn observer_delete(&mut self, observer: Option<ObserverId>) {
        let Some(observer) = observer else {
            return;
        };
        let Some(o) = self.observer.observers.get(observer.0) else {
            return;
        };
        let subject = o.subject;

        if o.for_obj
            && let ObserverTarget::Obj(target) = o.target
        {
            let ptr = o.ptr.clone();
            let cb = self.unsubscribe_on_delete_cb_rc();
            self.obj_remove_event_cb_with_user_data_opt(target, Some(&cb), ptr.as_ref());
            if let Some(subject_ptr) = self.subject_as_user_data(subject) {
                self.obj_remove_event_cb_with_user_data_opt(target, None, Some(&subject_ptr));
            }
        }

        if let Some(s) = self.observer.subjects.get_mut(subject.0) {
            s.notify_restart_query = true;

            /*lv_ll_remove(&(observer->subject->subs_ll), observer)*/
            if let Some(pos) = s.subs.iter().position(|&id| id == observer) {
                s.subs.remove(pos);
            }
        }

        /*`auto_free_user_data`: the `user_data` reference is dropped with the observer*/
        self.observer.observers.remove(observer.0);
    }

    /// `lv_obj_remove_from_subject()`: remove the observers of `obj` from
    /// `subject`, or from all subjects when `None`.
    pub fn obj_remove_from_subject(&mut self, obj: Obj, subject: Option<SubjectId>) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_remove_from_subject: obj != NULL");
            return;
        }
        /* subject == NULL is documented as valid: remove from ALL subjects */

        /*
         * Look for the `observer` that connects `obj` and `subject`
         * Since the obj is associated with the subject,
         *  the `obj` will have an LV_EVENT_REMOVE event with the `unsubscribe_on_delete_cb` callback
         *  associated.
         * From the event we can then find the observer in the event's `user_data` field
         */
        let unsubscribe_cb = self.unsubscribe_on_delete_cb_rc();
        let event_cnt = self.obj_get_event_count(obj) as i32;
        let mut i = event_cnt - 1;
        while i >= 0 {
            let observer = self
                .obj_get_event_dsc(obj, i as u32)
                .filter(|dsc| Rc::ptr_eq(&dsc.cb, &unsubscribe_cb))
                .and_then(|dsc| observer_from_user_data(dsc.user_data.as_ref()));
            if let Some(observer) = observer
                && let Some(o) = self.observer.observers.get(observer.0)
                && (subject.is_none() || subject == Some(o.subject))
            {
                /* lv_observer_delete handles the deletion of all possible event callbacks */
                self.observer_delete(Some(observer));
            }
            i -= 1;
        }
        /* Gracefully de-couple `subject` from Widget by deleting any existing
         * `LV_EVENT_VALUE_CHANGED` event associated with `subject` in case
         * one of the `..._bind_value()` functions was used. */
        let subject_ptr = subject.and_then(|s| self.subject_as_user_data(s));
        self.obj_remove_event_cb_with_user_data_opt(obj, None, subject_ptr.as_ref());
    }

    /// `lv_observer_get_target()`
    #[must_use]
    pub fn observer_get_target(&self, observer: ObserverId) -> ObserverTarget {
        let Some(o) = self.observer.observers.get(observer.0) else {
            check_arg_failed("lv_observer_get_target: observer != NULL");
            return ObserverTarget::None;
        };

        o.target.clone()
    }

    /// `lv_observer_get_target_obj()`
    #[must_use]
    pub fn observer_get_target_obj(&self, observer: ObserverId) -> Option<Obj> {
        if !self.observer.observers.contains(observer.0) {
            check_arg_failed("lv_observer_get_target_obj: observer != NULL");
            return None;
        }

        match self.observer_get_target(observer) {
            ObserverTarget::Obj(obj) => Some(obj),
            _ => None,
        }
    }

    /// `lv_observer_get_user_data()`
    #[must_use]
    pub fn observer_get_user_data(&self, observer: ObserverId) -> Option<Rc<dyn Any>> {
        let Some(o) = self.observer.observers.get(observer.0) else {
            check_arg_failed("lv_observer_get_user_data: observer != NULL");
            return None;
        };

        o.user_data.clone()
    }

    /// `lv_observer_set_user_data()`
    pub fn observer_set_user_data(&mut self, observer: ObserverId, user_data: Option<Rc<dyn Any>>) {
        let Some(o) = self.observer.observers.get_mut(observer.0) else {
            check_arg_failed("lv_observer_set_user_data: observer != NULL");
            return;
        };

        /* Release internally-owned data only when replacing it.
         * (Replacing the `Rc` below drops the observer's reference.) */

        /* Data assigned through the public setter is always caller-owned. */
        o.user_data = user_data;
        o.auto_free_user_data = false;
    }

    /// `lv_subject_notify()`: notify all observers of `subject`.
    pub fn subject_notify(&mut self, subject: SubjectId) {
        let ObserverState {
            subjects, observers, ..
        } = &mut self.observer;
        let Some(s) = subjects.get_mut(subject.0) else {
            check_arg_failed("lv_subject_notify: subject != NULL");
            return;
        };

        for &id in &s.subs {
            if let Some(o) = observers.get_mut(id.0) {
                o.notified = false;
            }
        }

        loop {
            let Some(s) = self.observer.subjects.get_mut(subject.0) else {
                return;
            };
            s.notify_restart_query = false;
            /*LV_LL_READ(&(subject->subs_ll), observer)*/
            let mut i = 0;
            loop {
                let Some(s) = self.observer.subjects.get(subject.0) else {
                    return;
                };
                let Some(&observer) = s.subs.get(i) else {
                    break;
                };
                i += 1;
                let Some(o) = self.observer.observers.get(observer.0) else {
                    continue;
                };
                if let Some(cb) = o.cb.clone()
                    && !o.notified
                {
                    cb(self, observer, subject);
                    let Some(s) = self.observer.subjects.get(subject.0) else {
                        return;
                    };
                    if s.notify_restart_query {
                        break;
                    }
                    if let Some(o) = self.observer.observers.get_mut(observer.0) {
                        o.notified = true;
                    }
                }
            }
            if !self
                .observer
                .subjects
                .get(subject.0)
                .is_some_and(|s| s.notify_restart_query)
            {
                break;
            }
        }
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
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_add_subject_increment_event: obj != NULL");
            return None;
        }
        let Some(type_) = self.subject_type(subject) else {
            check_arg_failed("lv_obj_add_subject_increment_event: subject != NULL");
            return None;
        };
        if !(type_ == SubjectType::Int || type_ == SubjectType::Float) {
            check_arg_failed("lv_obj_add_subject_increment_event: subject->type == INT || FLOAT");
            return None;
        }

        let user_data: SubjectIncrementDscRef = Rc::new(RefCell::new(SubjectIncrementDsc {
            step,
            subject,
            rollover: false,
            min_value: i32::MIN,
            max_value: i32::MAX,
        }));
        let ud: Rc<dyn Any> = user_data.clone();
        self.obj_add_event_cb(obj, Rc::new(subject_increment_cb), trigger, Some(ud.clone()));
        self.obj_add_event_cb(obj, Rc::new(event_free_user_data_cb), EventCode::DELETE, Some(ud));

        Some(user_data)
    }

    /// `lv_obj_set_subject_increment_event_min_value()`
    #[allow(clippy::collapsible_if)] // keep the C `if` nesting
    pub fn obj_set_subject_increment_event_min_value(
        &mut self,
        obj: Obj,
        dsc: &SubjectIncrementDscRef,
        min_value: i32,
    ) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_set_subject_increment_event_min_value: obj != NULL");
            return;
        }

        let subject = {
            let mut d = dsc.borrow_mut();
            d.min_value = min_value;
            d.subject
        };
        let type_ = self.subject_type(subject);
        if type_ == Some(SubjectType::Int) {
            if self.observer.subjects[subject.0].value.num() < min_value {
                self.subject_set_int(subject, min_value);
            }
        }
        #[cfg(feature = "float")]
        if type_ == Some(SubjectType::Float)
            && self.observer.subjects[subject.0].value.float_v() < min_value as f32
        {
            /*else if: the types exclude each other*/
            self.subject_set_float(subject, min_value as f32);
        }
    }

    /// `lv_obj_set_subject_increment_event_max_value()`
    #[allow(clippy::collapsible_if)] // keep the C `if` nesting
    pub fn obj_set_subject_increment_event_max_value(
        &mut self,
        obj: Obj,
        dsc: &SubjectIncrementDscRef,
        max_value: i32,
    ) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_set_subject_increment_event_max_value: obj != NULL");
            return;
        }

        let subject = {
            let mut d = dsc.borrow_mut();
            d.max_value = max_value;
            d.subject
        };
        let type_ = self.subject_type(subject);
        if type_ == Some(SubjectType::Int) {
            if self.observer.subjects[subject.0].value.num() > max_value {
                self.subject_set_int(subject, max_value);
            }
        }
        #[cfg(feature = "float")]
        if type_ == Some(SubjectType::Float)
            && self.observer.subjects[subject.0].value.float_v() > max_value as f32
        {
            /*else if: the types exclude each other*/
            self.subject_set_float(subject, max_value as f32);
        }
    }

    /// `lv_obj_set_subject_increment_event_rollover()`: what to do when the
    /// min/max value is crossed.
    pub fn obj_set_subject_increment_event_rollover(
        &mut self,
        obj: Obj,
        dsc: &SubjectIncrementDscRef,
        rollover: bool,
    ) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_set_subject_increment_event_rollover: obj != NULL");
            return;
        }

        dsc.borrow_mut().rollover = rollover;
    }

    /// `LV_CHECK_ARG(obj != NULL)`, `LV_CHECK_ARG(subject != NULL)` and
    /// `LV_CHECK_ARG(subject->type == type_)` of the event helpers.
    fn check_obj_subject(&self, obj: Obj, subject: SubjectId, type_: SubjectType, func: &str) -> bool {
        if !self.obj_is_valid(obj) {
            check_arg_failed(func);
            return false;
        }
        self.subject_check_type(subject, type_, func)
    }

    /// `lv_obj_add_subject_toggle_event()`: toggle an integer subject
    /// between 0 and 1 on `trigger`.
    pub fn obj_add_subject_toggle_event(&mut self, obj: Obj, subject: SubjectId, trigger: EventCode) {
        if !self.check_obj_subject(obj, subject, SubjectType::Int, "lv_obj_add_subject_toggle_event") {
            return;
        }

        let ptr = self.subject_as_user_data(subject);
        self.obj_add_event_cb(obj, Rc::new(subject_toggle_cb), trigger, ptr);
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
        if !self.check_obj_subject(obj, subject, SubjectType::Int, "lv_obj_add_subject_set_int_event") {
            return;
        }

        let user_data: Rc<dyn Any> = Rc::new(SubjectSetIntUserData { subject, value });

        self.obj_add_event_cb(obj, Rc::new(subject_set_int_cb), trigger, Some(user_data.clone()));
        self.obj_add_event_cb(
            obj,
            Rc::new(event_free_user_data_cb),
            EventCode::DELETE,
            Some(user_data),
        );
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
        if !self.check_obj_subject(
            obj,
            subject,
            SubjectType::Float,
            "lv_obj_add_subject_set_float_event",
        ) {
            return;
        }

        let user_data: Rc<dyn Any> = Rc::new(SubjectSetFloatUserData { subject, value });

        self.obj_add_event_cb(
            obj,
            Rc::new(subject_set_float_cb),
            trigger,
            Some(user_data.clone()),
        );
        self.obj_add_event_cb(
            obj,
            Rc::new(event_free_user_data_cb),
            EventCode::DELETE,
            Some(user_data),
        );
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
        if !self.check_obj_subject(
            obj,
            subject,
            SubjectType::String,
            "lv_obj_add_subject_set_string_event",
        ) {
            return;
        }

        /*lv_strdup(value)*/
        let user_data: Rc<dyn Any> = Rc::new(SubjectSetStringUserData {
            subject,
            value: String::from(value),
        });

        self.obj_add_event_cb(
            obj,
            Rc::new(subject_set_string_cb),
            trigger,
            Some(user_data.clone()),
        );
        self.obj_add_event_cb(
            obj,
            Rc::new(subject_set_string_free_user_data_event_cb),
            EventCode::DELETE,
            Some(user_data),
        );
    }

    /// The body shared by the `lv_obj_bind_<type>()` functions: subscribe
    /// `observer_cb` for `obj`, then store the setter and call the observer
    /// manually.
    fn obj_bind_setter(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        type_: SubjectType,
        observer_cb: fn(&mut Ui, ObserverId, SubjectId),
        user_cb: ObserverUserCb,
        func: &str,
    ) -> Option<ObserverId> {
        if !self.check_obj_subject(obj, subject, type_, func) {
            return None;
        }

        let Some(observable) = self.subject_add_observer_obj(subject, Rc::new(observer_cb), Some(obj), None)
        else {
            log_warn("Couldn't add observer to subject");
            return None;
        };

        /* Passing a function pointer as void * user_data generates warning so set it here, and call the callback manually */
        if let Some(o) = self.observer.observers.get_mut(observable.0) {
            o.user_cb = Some(user_cb);
        }
        observer_cb(self, observable, subject);
        Some(observable)
    }

    /// `lv_obj_bind_bool()`: call `set_bool_cb` with the subject's value on
    /// subscribing and whenever it changes.
    pub fn obj_bind_bool(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        set_bool_cb: ObjSetBoolCb,
    ) -> Option<ObserverId> {
        self.obj_bind_setter(
            obj,
            subject,
            SubjectType::Int,
            set_bool_observer,
            ObserverUserCb::Bool(set_bool_cb),
            "lv_obj_bind_bool",
        )
    }

    /// `lv_obj_bind_int()`: call `set_int_cb` with the subject's value on
    /// subscribing and whenever it changes.
    pub fn obj_bind_int(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        set_int_cb: ObjSetIntCb,
    ) -> Option<ObserverId> {
        self.obj_bind_setter(
            obj,
            subject,
            SubjectType::Int,
            set_int_observer,
            ObserverUserCb::Int(set_int_cb),
            "lv_obj_bind_int",
        )
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
        self.obj_bind_setter(
            obj,
            subject,
            SubjectType::Float,
            set_float_observer,
            ObserverUserCb::Float(set_float_cb),
            "lv_obj_bind_float",
        )
    }

    /// `lv_obj_bind_string()`: call `set_string_cb` with the subject's value
    /// on subscribing and whenever it changes.
    pub fn obj_bind_string(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        set_string_cb: ObjSetStringCb,
    ) -> Option<ObserverId> {
        self.obj_bind_setter(
            obj,
            subject,
            SubjectType::String,
            set_string_observer,
            ObserverUserCb::String(set_string_cb),
            "lv_obj_bind_string",
        )
    }

    /// `lv_obj_bind_color()`: call `set_color_cb` with the subject's value
    /// on subscribing and whenever it changes.
    pub fn obj_bind_color(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        set_color_cb: ObjSetColorCb,
    ) -> Option<ObserverId> {
        self.obj_bind_setter(
            obj,
            subject,
            SubjectType::Color,
            set_color_observer,
            ObserverUserCb::Color(set_color_cb),
            "lv_obj_bind_color",
        )
    }

    /// `lv_obj_bind_pointer()`: call `set_pointer_cb` with the subject's
    /// value on subscribing and whenever it changes.
    pub fn obj_bind_pointer(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        set_pointer_cb: ObjSetPointerCb,
    ) -> Option<ObserverId> {
        self.obj_bind_setter(
            obj,
            subject,
            SubjectType::Pointer,
            set_pointer_observer,
            ObserverUserCb::Pointer(set_pointer_cb),
            "lv_obj_bind_pointer",
        )
    }

    /// `LV_CHECK_ARG(obj != NULL)`, `LV_CHECK_ARG(subject != NULL)` and
    /// `LV_CHECK_ARG_FORMAT_MSG(subject->type == LV_SUBJECT_TYPE_INT, ..., "Incompatible subject type: %d")`
    /// followed by `bind_to_bitfield()`.
    fn obj_bind_bitfield(
        &mut self,
        obj: Obj,
        subject: SubjectId,
        cb: fn(&mut Ui, ObserverId, SubjectId),
        flag: u32,
        ref_value: i32,
        inv: bool,
        cond: FlagCond,
    ) -> Option<ObserverId> {
        if !self.check_obj_subject(obj, subject, SubjectType::Int, "Incompatible subject type") {
            return None;
        }

        self.bind_to_bitfield(subject, obj, cb, flag, ref_value, inv, cond)
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
        self.obj_bind_bitfield(
            obj,
            subject,
            obj_flag_observer_cb,
            flag.bits(),
            ref_value,
            false,
            FlagCond::Eq,
        )
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
        self.obj_bind_bitfield(
            obj,
            subject,
            obj_flag_observer_cb,
            flag.bits(),
            ref_value,
            true,
            FlagCond::Eq,
        )
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
        self.obj_bind_bitfield(
            obj,
            subject,
            obj_flag_observer_cb,
            flag.bits(),
            ref_value,
            false,
            FlagCond::Gt,
        )
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
        self.obj_bind_bitfield(
            obj,
            subject,
            obj_flag_observer_cb,
            flag.bits(),
            ref_value,
            false,
            FlagCond::Ge,
        )
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
        /* a < b == !(a >= b) */
        self.obj_bind_bitfield(
            obj,
            subject,
            obj_flag_observer_cb,
            flag.bits(),
            ref_value,
            true,
            FlagCond::Ge,
        )
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
        /* a <= b == !(a > b) */
        self.obj_bind_bitfield(
            obj,
            subject,
            obj_flag_observer_cb,
            flag.bits(),
            ref_value,
            true,
            FlagCond::Gt,
        )
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
        self.obj_bind_bitfield(
            obj,
            subject,
            obj_state_observer_cb,
            state.bits() as u32,
            ref_value,
            false,
            FlagCond::Eq,
        )
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
        self.obj_bind_bitfield(
            obj,
            subject,
            obj_state_observer_cb,
            state.bits() as u32,
            ref_value,
            true,
            FlagCond::Eq,
        )
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
        self.obj_bind_bitfield(
            obj,
            subject,
            obj_state_observer_cb,
            state.bits() as u32,
            ref_value,
            false,
            FlagCond::Gt,
        )
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
        self.obj_bind_bitfield(
            obj,
            subject,
            obj_state_observer_cb,
            state.bits() as u32,
            ref_value,
            false,
            FlagCond::Ge,
        )
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
        /* a < b == !(a >= b) */
        self.obj_bind_bitfield(
            obj,
            subject,
            obj_state_observer_cb,
            state.bits() as u32,
            ref_value,
            true,
            FlagCond::Ge,
        )
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
        /* a <= b == !(a > b) */
        self.obj_bind_bitfield(
            obj,
            subject,
            obj_state_observer_cb,
            state.bits() as u32,
            ref_value,
            true,
            FlagCond::Gt,
        )
    }

    /// `lv_obj_bind_checked()`: keep an integer subject and the widget's
    /// `LV_STATE_CHECKED` in sync in both directions.
    pub fn obj_bind_checked(&mut self, obj: Obj, subject: SubjectId) -> Option<ObserverId> {
        if !self.check_obj_subject(obj, subject, SubjectType::Int, "Incompatible subject type") {
            return None;
        }

        let observable = self.bind_to_bitfield(
            subject,
            obj,
            obj_state_observer_cb,
            State::CHECKED.bits() as u32,
            0,
            true,
            FlagCond::Eq,
        );

        let ptr = self.subject_as_user_data(subject);
        self.obj_add_event_cb(
            obj,
            Rc::new(obj_value_changed_event_cb),
            EventCode::VALUE_CHANGED,
            ptr,
        );

        observable
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
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_bind_style: obj != NULL");
            return None;
        }
        if !self.styles.contains(style.0) {
            check_arg_failed("lv_obj_bind_style: style != NULL");
            return None;
        }
        let Some(type_) = self.subject_type(subject) else {
            check_arg_failed("lv_obj_bind_style: subject != NULL");
            return None;
        };

        if type_ != SubjectType::Int {
            log_warn("Subject type must be `int`");
            return None;
        }

        self.obj_add_style(obj, style, selector);

        let p: Rc<dyn Any> = Rc::new(BindStyle {
            style,
            selector,
            value: ref_value,
        });

        let observable =
            self.subject_add_observer_obj(subject, Rc::new(bind_style_observer_cb), Some(obj), Some(p));
        if let Some(o) = observable.and_then(|id| self.observer.observers.get_mut(id.0)) {
            o.auto_free_user_data = true;
        }
        observable
    }

    /// `lv_obj_bind_style_prop()` (`lv_obj_style.c`): set the local style
    /// property `prop` with `selector` from the subject's value.
    ///
    /// A pointer subject's value is the `lv_style_value_t.ptr`: it should
    /// hold a [`StyleValue`] (or one of its pointer payloads, e.g. a
    /// `FontRef` or an `ImageSrc`); `NULL` sets [`StyleValue::Null`].
    pub fn obj_bind_style_prop(
        &mut self,
        obj: Obj,
        prop: StyleProp,
        selector: StyleSelector,
        subject: SubjectId,
    ) -> Option<ObserverId> {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_bind_style_prop: obj != NULL");
            return None;
        }
        let Some(type_) = self.subject_type(subject) else {
            check_arg_failed("lv_obj_bind_style_prop: subject != NULL");
            return None;
        };

        if type_ != SubjectType::Int && type_ != SubjectType::Color && type_ != SubjectType::Pointer {
            log_warn("Subject type must be `int`, `color`, or `pointer`");
            return None;
        }

        let p: Rc<dyn Any> = Rc::new(BindStyleProp { prop, selector });

        let observable =
            self.subject_add_observer_obj(subject, Rc::new(bind_style_prop_observer_cb), Some(obj), Some(p));
        if let Some(o) = observable.and_then(|id| self.observer.observers.get_mut(id.0)) {
            o.auto_free_user_data = true;
        }
        observable
    }

    /*********************
     *   STATIC FUNCTIONS
     *********************/

    /// `init_common()`: `lv_memzero(subject)` + `lv_ll_init(&subject->subs_ll)`.
    /// The observers that were in the list are orphaned, like upstream.
    fn init_common(&mut self, subject: SubjectId) {
        let s = &mut self.observer.subjects[subject.0];
        let ptr = s.ptr.take();
        *s = Subject {
            ptr,
            ..Subject::default()
        };
    }

    /// `init_int()`
    fn init_int(&mut self, subject: SubjectId, value: i32) {
        let s = &mut self.observer.subjects[subject.0];
        s.type_ = SubjectType::Int;
        s.value = SubjectValue::Num(value);
        s.prev_value = SubjectValue::Num(value);
        s.min_value = SubjectValue::Num(i32::MIN);
        s.max_value = SubjectValue::Num(i32::MAX);
    }

    /// `init_float()`
    #[cfg(feature = "float")]
    fn init_float(&mut self, subject: SubjectId, value: f32) {
        let s = &mut self.observer.subjects[subject.0];
        s.type_ = SubjectType::Float;
        s.value = SubjectValue::Float(value);
        s.prev_value = SubjectValue::Float(value);
        s.min_value = SubjectValue::Float(-f32::MAX);
        s.max_value = SubjectValue::Float(f32::MAX);
    }

    /// `init_pointer()`
    fn init_pointer(&mut self, subject: SubjectId, value: Option<Rc<dyn Any>>) {
        let s = &mut self.observer.subjects[subject.0];
        s.type_ = SubjectType::Pointer;
        s.value = SubjectValue::Pointer(value.clone());
        s.prev_value = SubjectValue::Pointer(value);
    }

    /// `init_color()`
    fn init_color(&mut self, subject: SubjectId, color: Color) {
        let s = &mut self.observer.subjects[subject.0];
        s.type_ = SubjectType::Color;
        s.value = SubjectValue::Color(color);
        s.prev_value = SubjectValue::Color(color);
    }

    /// `init_group()`
    fn init_group(&mut self, subject: SubjectId, list: Option<Rc<[SubjectId]>>) {
        self.observer.subjects[subject.0].type_ = SubjectType::Group;
        self.add_observers_to_group_subject(subject, list);
    }

    /// `add_observers_to_group_subject()`
    fn add_observers_to_group_subject(&mut self, subject: SubjectId, list: Option<Rc<[SubjectId]>>) {
        let size = list.as_ref().map_or(0, |l| l.len());
        {
            let s = &mut self.observer.subjects[subject.0];
            s.value = SubjectValue::Group(list.clone());
            s.size = (size as u32) & SUBJECT_SIZE_MASK;
        }

        let Some(list) = list else {
            return;
        };
        let group_ptr = self.subject_as_user_data(subject);
        for i in 0..size {
            let cb = self.group_notify_cb_rc();
            self.subject_add_observer(list[i], cb, group_ptr.clone());
        }
    }

    /// `init_string()`: `size == 0` stands for `buf == NULL` (and no
    /// `prev_buf`).
    fn init_string(&mut self, subject: SubjectId, size: usize, track_previous: bool, value: &str) {
        let (buf, prev_buf) = if size > 0 {
            (
                Some(strlcpy(value, size)),
                track_previous.then(|| strlcpy(value, size)),
            )
        } else {
            (None, None)
        };

        let s = &mut self.observer.subjects[subject.0];
        s.type_ = SubjectType::String;
        s.size = (size as u32) & SUBJECT_SIZE_MASK;
        s.value = SubjectValue::Str(buf);
        s.prev_value = SubjectValue::Str(prev_buf);
    }

    /// `deinit()`
    fn deinit_subject(&mut self, subject: SubjectId) {
        if self.subject_type(subject) == Some(SubjectType::Group) {
            self.remove_observers_from_group_subject(subject);
        }

        let subs = self.observer.subjects[subject.0].subs.clone();
        for observer in subs {
            self.observer_delete(Some(observer));
        }

        let s = &mut self.observer.subjects[subject.0];
        if let Some(free_cb) = s.ext_data.free_cb {
            let data = s.ext_data.data.take();
            free_cb(data);
        }

        self.observer.subjects[subject.0].subs.clear();
    }

    /// `bind_to_bitfield()`
    #[allow(clippy::too_many_arguments)] // the C signature
    fn bind_to_bitfield(
        &mut self,
        subject: SubjectId,
        obj: Obj,
        cb: fn(&mut Ui, ObserverId, SubjectId),
        flag: u32,
        ref_value: i32,
        inv: bool,
        cond: FlagCond,
    ) -> Option<ObserverId> {
        let p: Rc<dyn Any> = Rc::new(FlagAndCond {
            flag,
            value: ref_value,
            inv,
            cond,
        });

        let observable = self.subject_add_observer_obj(subject, Rc::new(cb), Some(obj), Some(p));
        if let Some(o) = observable.and_then(|id| self.observer.observers.get_mut(id.0)) {
            o.auto_free_user_data = true;
        }

        observable
    }

    /// `lv_subject_notify_if_changed()`
    fn subject_notify_if_changed(&mut self, subject: SubjectId) {
        let s = &self.observer.subjects[subject.0];

        let changed = match s.type_ {
            SubjectType::Int => s.value.num() != s.prev_value.num(),
            #[cfg(feature = "float")]
            SubjectType::Float => s.value.float_v() != s.prev_value.float_v(),
            #[cfg(not(feature = "float"))]
            SubjectType::Float => false,
            /* Always notify as we don't know how to compare this */
            SubjectType::Group | SubjectType::Pointer => true,
            SubjectType::Color => s.value.color() != s.prev_value.color(),
            SubjectType::String => match (s.value.str(), s.prev_value.str()) {
                (_, None) => true,
                (cur, Some(prev)) => cur != Some(prev),
            },
            SubjectType::Invalid | SubjectType::None => unreachable!(),
        };
        if changed {
            self.subject_notify(subject);
        }
    }

    /// `remove_observers_from_group_subject()`
    fn remove_observers_from_group_subject(&mut self, subject: SubjectId) {
        let s = &self.observer.subjects[subject.0];
        let size = s.size as usize;
        let list = s.value.group();
        let group_ptr = self.subject_as_user_data(subject);
        let cb = self.group_notify_cb_rc();

        for i in 0..size {
            let Some(sub) = list.as_ref().and_then(|l| l.get(i).copied()) else {
                continue;
            };
            self.subject_remove_observer(sub, &cb, group_ptr.as_ref());
        }
    }

    /// `subject_remove_observer()`
    fn subject_remove_observer(
        &mut self,
        subject: SubjectId,
        cb: &ObserverCb,
        user_data: Option<&Rc<dyn Any>>,
    ) {
        let Some(s) = self.observer.subjects.get(subject.0) else {
            return;
        };

        let subs = s.subs.clone();
        for obs in subs {
            let hit = self.observer.observers.get(obs.0).is_some_and(|o| {
                o.cb.as_ref().is_some_and(|c| Rc::ptr_eq(c, cb))
                    && opt_ptr_eq(o.user_data.as_ref(), user_data)
            });
            if hit {
                self.observer_delete(Some(obs));
            }
        }
    }
}

/// `subject_toggle_cb()`
fn subject_toggle_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(subject) = e.user_data().and_then(SubjectId::from_user_data) else {
        return;
    };
    let mut v = ui.subject_get_int(subject);
    v = (v == 0) as i32;

    ui.subject_set_int(subject, v);
}

/// `subject_set_int_cb()`
fn subject_set_int_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(user_data) = e
        .user_data()
        .and_then(|d| (**d).downcast_ref::<SubjectSetIntUserData>())
    else {
        return;
    };
    let (subject, value) = (user_data.subject, user_data.value);
    ui.subject_set_int(subject, value);
}

/// `subject_set_float_cb()`
#[cfg(feature = "float")]
fn subject_set_float_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(user_data) = e
        .user_data()
        .and_then(|d| (**d).downcast_ref::<SubjectSetFloatUserData>())
    else {
        return;
    };
    let (subject, value) = (user_data.subject, user_data.value);
    ui.subject_set_float(subject, value);
}

/// `subject_set_string_cb()`
fn subject_set_string_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(user_data) = e.user_data().cloned() else {
        return;
    };
    let Some(user_data) = (*user_data).downcast_ref::<SubjectSetStringUserData>() else {
        return;
    };
    ui.subject_set_string(user_data.subject, &user_data.value);
}

/// `subject_increment_cb()`
fn subject_increment_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(user_data) = e
        .user_data()
        .and_then(|d| (**d).downcast_ref::<RefCell<SubjectIncrementDsc>>())
        .map(|d| *d.borrow())
    else {
        return;
    };
    let subject = user_data.subject;

    match ui.subject_type(subject) {
        Some(SubjectType::Int) => {
            let s = &ui.observer.subjects[subject.0];
            /*Use the smaller range*/
            let max_value = lv_min(user_data.max_value, s.max_value.num());
            let min_value = lv_max(user_data.min_value, s.min_value.num());

            let mut value = ui.subject_get_int(subject);
            value = value.wrapping_add(user_data.step);

            if user_data.rollover {
                if value > max_value {
                    value = min_value;
                } else if value < min_value {
                    value = max_value;
                }
            } else {
                value = lv_clamp(min_value, value, max_value);
            }

            ui.subject_set_int(subject, value);
        }
        #[cfg(feature = "float")]
        Some(SubjectType::Float) => {
            let s = &ui.observer.subjects[subject.0];
            /*Use the smaller range*/
            let max_value = lv_min(user_data.max_value as f32, s.max_value.float_v());
            let min_value = lv_max(user_data.min_value as f32, s.min_value.float_v());

            let mut value = ui.subject_get_float(subject);
            value += user_data.step as f32;

            if user_data.rollover {
                if value > max_value {
                    value = min_value;
                } else if value < min_value {
                    value = max_value;
                }
            } else {
                value = lv_clamp(min_value, value, max_value);
            }

            ui.subject_set_float(subject, value);
        }
        _ => {}
    }
}

/// `group_notify_cb()`
fn group_notify_cb(ui: &mut Ui, observer: ObserverId, _subject: SubjectId) {
    let subject_group = ui
        .observer
        .observers
        .get(observer.0)
        .and_then(|o| o.user_data.as_ref())
        .and_then(SubjectId::from_user_data);
    if let Some(subject_group) = subject_group {
        ui.subject_notify(subject_group);
    }
}

/// `unsubscribe_on_delete_cb()`
fn unsubscribe_on_delete_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let observer = observer_from_user_data(e.user_data());
    ui.observer_delete(observer);
}

/// `p->cond` / `p->inv` evaluation shared by `obj_flag_observer_cb()` and
/// `obj_state_observer_cb()`: `None` if the observer has no
/// `flag_and_cond_t`.
fn flag_and_cond_eval(ui: &Ui, observer: ObserverId, subject: SubjectId) -> Option<(Obj, u32, bool)> {
    let o = ui.observer.observers.get(observer.0)?;
    let p = o
        .user_data
        .as_ref()
        .and_then(|d| (**d).downcast_ref::<FlagAndCond>())?;
    let ObserverTarget::Obj(target) = o.target else {
        return None;
    };
    let num = ui.observer.subjects.get(subject.0)?.value.num();

    /* Initializing this keeps some compilers happy */
    let mut res = match p.cond {
        FlagCond::Eq => num == p.value,
        FlagCond::Gt => num > p.value,
        FlagCond::Ge => num >= p.value,
    };
    if p.inv {
        res = !res;
    }
    Some((target, p.flag, res))
}

/// `obj_flag_observer_cb()`
fn obj_flag_observer_cb(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let Some((target, flag, res)) = flag_and_cond_eval(ui, observer, subject) else {
        return;
    };

    if res {
        ui.obj_add_flag(target, ObjFlags::from_bits_retain(flag));
    } else {
        ui.obj_remove_flag(target, ObjFlags::from_bits_retain(flag));
    }
}

/// `obj_state_observer_cb()`
fn obj_state_observer_cb(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let Some((target, flag, res)) = flag_and_cond_eval(ui, observer, subject) else {
        return;
    };

    /*`lv_state_t` is `uint16_t`*/
    let state = State::from_bits_retain(flag as u16);
    if res {
        ui.obj_add_state(target, state);
    } else {
        ui.obj_remove_state(target, state);
    }
}

/// `obj_value_changed_event_cb()`
fn obj_value_changed_event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(obj) = e.current_target_obj() else {
        return;
    };
    let Some(subject) = e.user_data().and_then(SubjectId::from_user_data) else {
        return;
    };

    let checked = ui.obj_has_state(obj, State::CHECKED);
    ui.subject_set_int(subject, checked as i32);
}

/// `subject_set_string_free_user_data_event_cb()`: drops the event's
/// reference to the `subject_set_string_user_data_t`.
fn subject_set_string_free_user_data_event_cb(_ui: &mut Ui, e: &mut Event<'_>) {
    let user_data = e.user_data.take();
    drop(user_data);
}

/// `(lv_obj_t *)observer->target` and the setter of a binding observer.
fn binding_target(ui: &Ui, observer: ObserverId) -> Option<(Obj, ObserverUserCb)> {
    let o = ui.observer.observers.get(observer.0)?;
    let ObserverTarget::Obj(obj) = o.target else {
        return None;
    };
    Some((obj, o.user_cb.clone()?))
}

/// `set_bool_observer()`
fn set_bool_observer(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let Some((obj, ObserverUserCb::Bool(set_bool_cb))) = binding_target(ui, observer) else {
        return;
    };
    let v = ui.observer.subjects.get(subject.0).map_or(0, |s| s.value.num());
    set_bool_cb(ui, obj, v != 0);
}

/// `set_int_observer()`
fn set_int_observer(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let Some((obj, ObserverUserCb::Int(set_int_cb))) = binding_target(ui, observer) else {
        return;
    };
    let v = ui.observer.subjects.get(subject.0).map_or(0, |s| s.value.num());
    set_int_cb(ui, obj, v);
}

/// `set_float_observer()`
#[cfg(feature = "float")]
fn set_float_observer(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let Some((obj, ObserverUserCb::Float(set_float_cb))) = binding_target(ui, observer) else {
        return;
    };
    let v = ui
        .observer
        .subjects
        .get(subject.0)
        .map_or(0.0, |s| s.value.float_v());
    set_float_cb(ui, obj, v);
}

/// `set_string_observer()`: a `NULL` buffer is passed as `""`.
fn set_string_observer(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let Some((obj, ObserverUserCb::String(set_string_cb))) = binding_target(ui, observer) else {
        return;
    };
    let v = ui
        .observer
        .subjects
        .get(subject.0)
        .and_then(|s| s.value.str())
        .map(String::from)
        .unwrap_or_default();
    set_string_cb(ui, obj, &v);
}

/// `set_color_observer()`
fn set_color_observer(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let Some((obj, ObserverUserCb::Color(set_color_cb))) = binding_target(ui, observer) else {
        return;
    };
    let v = ui
        .observer
        .subjects
        .get(subject.0)
        .map_or(Color::black(), |s| s.value.color());
    set_color_cb(ui, obj, v);
}

/// `set_pointer_observer()`
fn set_pointer_observer(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let Some((obj, ObserverUserCb::Pointer(set_pointer_cb))) = binding_target(ui, observer) else {
        return;
    };
    let v = ui
        .observer
        .subjects
        .get(subject.0)
        .and_then(|s| s.value.pointer());
    set_pointer_cb(ui, obj, v);
}

/// `bind_style_observer_cb()` (`lv_obj_style.c`)
fn bind_style_observer_cb(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let Some(user_data) = ui.observer_get_user_data(observer) else {
        return;
    };
    let Some(p) = (*user_data).downcast_ref::<BindStyle>() else {
        return;
    };
    let ObserverTarget::Obj(target) = ui.observer_get_target(observer) else {
        return;
    };

    let v = ui.subject_get_int(subject);
    let en = v == p.value;
    ui.obj_set_style_enabled(target, p.style, p.selector, en);
}

/// The `lv_style_value_t` whose `ptr` is a pointer subject's value.
fn style_value_from_pointer(ptr: Option<Rc<dyn Any>>) -> StyleValue {
    let Some(ptr) = ptr else {
        return StyleValue::Null;
    };
    let any = &*ptr;
    if let Some(v) = any.downcast_ref::<StyleValue>() {
        return v.clone();
    }
    if let Some(v) = any.downcast_ref::<crate::font::FontRef>() {
        return StyleValue::Font(v.clone());
    }
    if let Some(v) = any.downcast_ref::<crate::misc::style::GradRef>() {
        return StyleValue::Grad(v.clone());
    }
    if let Some(v) = any.downcast_ref::<crate::misc::style::ImageSrc>() {
        return StyleValue::Image(v.clone());
    }
    if let Some(v) = any.downcast_ref::<crate::misc::style::TransitionRef>() {
        return StyleValue::Transition(v.clone());
    }
    if let Some(v) = any.downcast_ref::<crate::misc::style::TracksRef>() {
        return StyleValue::Tracks(v.clone());
    }
    if let Some(v) = any.downcast_ref::<crate::misc::style::AnimRef>() {
        return StyleValue::Anim(v.clone());
    }
    if let Some(v) = any.downcast_ref::<crate::misc::color::ColorFilter>() {
        return StyleValue::ColorFilter(*v);
    }
    if let Some(v) = any.downcast_ref::<crate::misc::style::ImageColorkey>() {
        return StyleValue::Colorkey(*v);
    }
    StyleValue::Null
}

/// `bind_style_prop_observer_cb()` (`lv_obj_style.c`)
fn bind_style_prop_observer_cb(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let Some(user_data) = ui.observer_get_user_data(observer) else {
        return;
    };
    let Some(p) = (*user_data).downcast_ref::<BindStyleProp>() else {
        return;
    };

    let style_v = match ui.subject_type(subject) {
        Some(SubjectType::Int) => StyleValue::Num(ui.subject_get_int(subject)),
        Some(SubjectType::Color) => StyleValue::Color(ui.subject_get_color(subject)),
        Some(SubjectType::Pointer) => style_value_from_pointer(ui.subject_get_pointer(subject)),
        _ => {
            log_warn("Not supported subject type");
            return;
        }
    };

    let ObserverTarget::Obj(target) = ui.observer_get_target(observer) else {
        return;
    };
    ui.obj_set_local_style_prop(target, p.prop, style_v, p.selector);
}
