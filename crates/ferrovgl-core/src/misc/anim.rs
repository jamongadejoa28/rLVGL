//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_anim.h`, `src/misc/lv_anim_private.h`,
//! `src/misc/lv_anim.c`.
//!
//! `var` + `exec_cb` become [`AnimTarget`] + [`AnimExec`]; `AnimExec`
//! implements `PartialEq` on its discriminant/tag so that
//! `lv_anim_delete(var, exec_cb)` keeps working. `custom_exec_cb` stays a
//! separate field like upstream (both are called by `anim_timer`).

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::core::obj_style::StyleTransId;
use crate::display::DisplayId;
use crate::indev::IndevId;
use crate::misc::arena::{Arena, Key};
use crate::misc::style::{StyleProp, StyleSelector};
use crate::misc::timer::TimerId;

/// `LV_ANIM_REPEAT_INFINITE`
pub const ANIM_REPEAT_INFINITE: u32 = 0xFFFF_FFFF;
/// `LV_ANIM_PLAYTIME_INFINITE`
pub const ANIM_PLAYTIME_INFINITE: u32 = 0xFFFF_FFFF;
/// `LV_ANIM_PAUSE_FOREVER`
pub const ANIM_PAUSE_FOREVER: u32 = 0xFFFF_FFFF;
/// `LV_ANIM_OFF`
pub const ANIM_OFF: bool = false;
/// `LV_ANIM_ON`
pub const ANIM_ON: bool = true;
/// `lv_anim_enable_t`
pub type AnimEnable = bool;

/// Handle of a running animation (`lv_anim_t *` inside the animation list).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AnimId(pub Key);

/// The animated variable (`void * var`).
///
/// Core itself animates objects, style transitions (`var = tr` in
/// `lv_obj_style.c`), input devices (scroll throw, `var = indev`) and
/// displays; widgets / applications use [`AnimTarget::Tag`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum AnimTarget {
    /// `NULL`
    #[default]
    None,
    /// An object.
    Obj(Obj),
    /// A running style transition (`trans_t *`), see
    /// [`crate::core::Ui::style_trans`].
    StyleTrans(StyleTransId),
    /// An input device (`indev_scroll_throw_anim_cb`).
    Indev(IndevId),
    /// A display (e.g. screen load animations).
    Display(DisplayId),
    /// An application-defined tag (e.g. a widget-internal animation).
    Tag(u32),
}

/// A custom exec callback: `(ui, target, value)`.
pub type AnimCustomExec = Rc<dyn Fn(&mut Ui, AnimTarget, i32)>;
/// `lv_anim_custom_exec_cb_t`: `(ui, anim, value)`, receives the animation itself.
pub type AnimCustomExecWithAnim = Rc<dyn Fn(&mut Ui, AnimId, i32)>;

/// `exec_cb`: what to do with the animated value.
///
/// Equality compares the variant and, for custom callbacks, the tag only, so
/// it has the same semantics as comparing C function pointers.
#[derive(Clone)]
pub enum AnimExec {
    /// No exec callback.
    None,
    /// `lv_obj_set_x`
    ObjX,
    /// `lv_obj_set_y`
    ObjY,
    /// `lv_obj_set_width`
    ObjWidth,
    /// `lv_obj_set_height`
    ObjHeight,
    /// `lv_obj_scroll_to_x` (raw scroll, used by scroll animations)
    ObjScrollX,
    /// `lv_obj_scroll_to_y`
    ObjScrollY,
    /// `lv_obj_set_style_opa` with a selector (fade in/out).
    ObjOpa(StyleSelector),
    /// A local style property set through the animation (`lv_obj_set_local_style_prop`).
    StyleProp {
        /// The property.
        prop: StyleProp,
        /// The selector.
        selector: StyleSelector,
    },
    /// A style transition (`trans_anim_cb` of `lv_obj_style.c`).
    StyleTrans {
        /// The property.
        prop: StyleProp,
        /// The selector.
        selector: StyleSelector,
    },
    /// An application / widget callback identified by `tag`.
    Custom {
        /// Identity of the callback (compared by `lv_anim_delete`).
        tag: u32,
        /// The callback.
        f: AnimCustomExec,
    },
}

/// `custom_exec_cb` (`lv_anim_custom_exec_cb_t`): a second exec callback
/// that receives the animation itself. Independent of `exec_cb`; both run in
/// `anim_timer` (`exec_cb` first).
#[derive(Clone)]
pub struct AnimCustomExecCb {
    /// Identity of the callback (compared like a C function pointer).
    pub tag: u32,
    /// The callback.
    pub f: AnimCustomExecWithAnim,
}

impl PartialEq for AnimCustomExecCb {
    fn eq(&self, other: &Self) -> bool {
        self.tag == other.tag
    }
}

impl core::fmt::Debug for AnimCustomExecCb {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AnimCustomExecCb")
            .field("tag", &self.tag)
            .finish_non_exhaustive()
    }
}

impl AnimExec {
    /// Discriminant used for comparisons.
    const fn kind(&self) -> u32 {
        match self {
            AnimExec::None => 0,
            AnimExec::ObjX => 1,
            AnimExec::ObjY => 2,
            AnimExec::ObjWidth => 3,
            AnimExec::ObjHeight => 4,
            AnimExec::ObjScrollX => 5,
            AnimExec::ObjScrollY => 6,
            AnimExec::ObjOpa(_) => 7,
            AnimExec::StyleProp { .. } => 8,
            AnimExec::StyleTrans { .. } => 9,
            AnimExec::Custom { .. } => 10,
        }
    }
}

impl PartialEq for AnimExec {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AnimExec::Custom { tag: a, .. }, AnimExec::Custom { tag: b, .. }) => a == b,
            (AnimExec::ObjOpa(a), AnimExec::ObjOpa(b)) => a == b,
            (
                AnimExec::StyleProp {
                    prop: pa,
                    selector: sa,
                },
                AnimExec::StyleProp {
                    prop: pb,
                    selector: sb,
                },
            )
            | (
                AnimExec::StyleTrans {
                    prop: pa,
                    selector: sa,
                },
                AnimExec::StyleTrans {
                    prop: pb,
                    selector: sb,
                },
            ) => pa == pb && sa == sb,
            _ => self.kind() == other.kind(),
        }
    }
}

impl core::fmt::Debug for AnimExec {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AnimExec::Custom { tag, .. } => {
                f.debug_struct("Custom").field("tag", tag).finish_non_exhaustive()
            }
            AnimExec::StyleProp { prop, selector } => f
                .debug_struct("StyleProp")
                .field("prop", prop)
                .field("selector", selector)
                .finish(),
            AnimExec::StyleTrans { prop, selector } => f
                .debug_struct("StyleTrans")
                .field("prop", prop)
                .field("selector", selector)
                .finish(),
            AnimExec::ObjOpa(s) => f.debug_tuple("ObjOpa").field(s).finish(),
            other => write!(
                f,
                "{}",
                match other {
                    AnimExec::None => "None",
                    AnimExec::ObjX => "ObjX",
                    AnimExec::ObjY => "ObjY",
                    AnimExec::ObjWidth => "ObjWidth",
                    AnimExec::ObjHeight => "ObjHeight",
                    AnimExec::ObjScrollX => "ObjScrollX",
                    AnimExec::ObjScrollY => "ObjScrollY",
                    _ => "?",
                }
            ),
        }
    }
}

/// `lv_anim_path_cb_t` for user paths.
pub type AnimPathCallback = fn(&Anim) -> i32;

/// `path_cb`: the easing of an animation.
#[derive(Copy, Clone, Debug, Default)]
pub enum AnimPath {
    /// `lv_anim_path_linear`
    #[default]
    Linear,
    /// `lv_anim_path_ease_in`
    EaseIn,
    /// `lv_anim_path_ease_out`
    EaseOut,
    /// `lv_anim_path_ease_in_out`
    EaseInOut,
    /// `lv_anim_path_overshoot`
    Overshoot,
    /// `lv_anim_path_bounce`
    Bounce,
    /// `lv_anim_path_step`
    Step,
    /// `lv_anim_path_custom_bezier3` (uses `Anim::bezier3`).
    Bezier3,
    /// A user function.
    Custom(AnimPathCallback),
}

impl PartialEq for AnimPath {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AnimPath::Custom(a), AnimPath::Custom(b)) => core::ptr::fn_addr_eq(*a, *b),
            (AnimPath::Custom(_), _) | (_, AnimPath::Custom(_)) => false,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

/// `lv_anim_bezier3_para_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct AnimBezier3Para {
    /// `x1`
    pub x1: i16,
    /// `y1`
    pub y1: i16,
    /// `x2`
    pub x2: i16,
    /// `y2`
    pub y2: i16,
}

/// `lv_anim_start_cb_t` / `lv_anim_completed_cb_t` / `lv_anim_deleted_cb_t`
pub type AnimCallback = Rc<dyn Fn(&mut Ui, AnimId)>;
/// `lv_anim_get_value_cb_t`
pub type AnimGetValueCallback = Rc<dyn Fn(&mut Ui, AnimId) -> i32>;

/// `lv_anim_t`
#[derive(Clone)]
pub struct Anim {
    /// `var`
    pub var: AnimTarget,
    /// `exec_cb`
    pub exec: AnimExec,
    /// `custom_exec_cb`
    pub custom_exec_cb: Option<AnimCustomExecCb>,
    /// `start_cb`
    pub start_cb: Option<AnimCallback>,
    /// `completed_cb`
    pub completed_cb: Option<AnimCallback>,
    /// `deleted_cb`
    pub deleted_cb: Option<AnimCallback>,
    /// `get_value_cb`
    pub get_value_cb: Option<AnimGetValueCallback>,
    /// `user_data`
    pub user_data: Option<Rc<dyn Any>>,
    /// `path_cb`
    pub path: AnimPath,
    /// `start_value`
    pub start_value: i32,
    /// `current_value`
    pub current_value: i32,
    /// `end_value`
    pub end_value: i32,
    /// `duration` in ms.
    pub duration: i32,
    /// `act_time`: current time of the animation; negative while delayed.
    pub act_time: i32,
    /// `reverse_delay`
    pub reverse_delay: u32,
    /// `reverse_duration`
    pub reverse_duration: u32,
    /// `repeat_delay`
    pub repeat_delay: u32,
    /// `repeat_cnt`
    pub repeat_cnt: u32,
    /// `parameter.bezier3`
    pub bezier3: AnimBezier3Para,
    /// `last_timer_run`
    pub last_timer_run: u32,
    /// `pause_time`
    pub pause_time: u32,
    /// `pause_duration`
    pub pause_duration: u32,
    /// `is_paused`
    pub is_paused: bool,
    /// `reverse_play_in_progress`
    pub reverse_play_in_progress: bool,
    /// `run_round`
    pub run_round: bool,
    /// `start_cb_called`
    pub start_cb_called: bool,
    /// `early_apply`
    pub early_apply: bool,
}

impl Anim {
    /// `lv_anim_init()`
    pub fn new() -> Self {
        todo!()
    }

    /// `lv_anim_set_var()`
    pub fn set_var(&mut self, var: AnimTarget) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_exec_cb()`
    pub fn set_exec_cb(&mut self, exec: AnimExec) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_duration()`
    pub fn set_duration(&mut self, duration: u32) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_delay()`
    pub fn set_delay(&mut self, delay: u32) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_values()`
    pub fn set_values(&mut self, start: i32, end: i32) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_custom_exec_cb()`: sets `custom_exec_cb` only;
    /// `exec_cb` is left untouched.
    pub fn set_custom_exec_cb(&mut self, cb: Option<AnimCustomExecCb>) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_path_cb()`
    pub fn set_path_cb(&mut self, path: AnimPath) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_start_cb()`
    pub fn set_start_cb(&mut self, cb: Option<AnimCallback>) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_get_value_cb()`
    pub fn set_get_value_cb(&mut self, cb: Option<AnimGetValueCallback>) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_completed_cb()`
    pub fn set_completed_cb(&mut self, cb: Option<AnimCallback>) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_deleted_cb()`
    pub fn set_deleted_cb(&mut self, cb: Option<AnimCallback>) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_reverse_duration()` (also `lv_anim_set_reverse_time()`).
    pub fn set_reverse_duration(&mut self, duration: u32) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_reverse_delay()`
    pub fn set_reverse_delay(&mut self, delay: u32) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_repeat_count()`
    pub fn set_repeat_count(&mut self, cnt: u32) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_repeat_delay()`
    pub fn set_repeat_delay(&mut self, delay: u32) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_early_apply()`
    pub fn set_early_apply(&mut self, en: bool) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_user_data()`
    pub fn set_user_data(&mut self, user_data: Option<Rc<dyn Any>>) -> &mut Self {
        todo!()
    }

    /// `lv_anim_set_bezier3_param()`
    pub fn set_bezier3_param(&mut self, x1: i16, y1: i16, x2: i16, y2: i16) -> &mut Self {
        todo!()
    }

    /// `lv_anim_get_delay()`
    pub fn delay(&self) -> u32 {
        todo!()
    }

    /// `lv_anim_get_playtime()`
    pub fn playtime(&self) -> u32 {
        todo!()
    }

    /// `lv_anim_get_time()`
    pub fn time(&self) -> u32 {
        todo!()
    }

    /// `lv_anim_get_repeat_count()`
    pub fn repeat_count(&self) -> u32 {
        todo!()
    }

    /// `lv_anim_get_user_data()`
    pub fn user_data(&self) -> Option<&Rc<dyn Any>> {
        self.user_data.as_ref()
    }

    /// `lv_anim_is_paused()`
    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    /// `lv_anim_path_linear()`
    pub fn path_linear(&self) -> i32 {
        todo!()
    }

    /// `lv_anim_path_ease_in()`
    pub fn path_ease_in(&self) -> i32 {
        todo!()
    }

    /// `lv_anim_path_ease_out()`
    pub fn path_ease_out(&self) -> i32 {
        todo!()
    }

    /// `lv_anim_path_ease_in_out()`
    pub fn path_ease_in_out(&self) -> i32 {
        todo!()
    }

    /// `lv_anim_path_overshoot()`
    pub fn path_overshoot(&self) -> i32 {
        todo!()
    }

    /// `lv_anim_path_bounce()`
    pub fn path_bounce(&self) -> i32 {
        todo!()
    }

    /// `lv_anim_path_step()`
    pub fn path_step(&self) -> i32 {
        todo!()
    }

    /// `lv_anim_path_custom_bezier3()`
    pub fn path_custom_bezier3(&self) -> i32 {
        todo!()
    }

    /// Evaluate `self.path` (dispatch of `a->path_cb(a)`).
    pub fn path_value(&self) -> i32 {
        todo!()
    }
}

impl Default for Anim {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_anim_state_t`
pub struct AnimState {
    /// `anim_ll`
    pub anims: Arena<Anim>,
    /// Order of `anim_ll` (head first): `lv_anim_start` is `lv_ll_ins_head`
    /// (`order.insert(0, id)`), `anim_timer` / `lv_anim_delete` /
    /// `lv_anim_get` iterate `order` and restart from its head.
    pub order: Vec<AnimId>,
    /// `timer`
    pub timer: Option<TimerId>,
    /// `anim_list_changed`
    pub list_changed: bool,
    /// `anim_run_round`
    pub run_round: bool,
    /// `anim_vsync_registered`
    pub vsync_registered: bool,
}

impl AnimState {
    /// `lv_anim_core_init()` (the timer is created by `Ui::new`).
    pub fn new() -> Self {
        Self {
            anims: Arena::new(),
            order: Vec::new(),
            timer: None,
            list_changed: false,
            run_round: false,
            vsync_registered: false,
        }
    }
}

impl Default for AnimState {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    /// `lv_anim_start()`: copy `a` into the animation list and start it.
    pub fn anim_start(&mut self, a: &Anim) -> AnimId {
        todo!()
    }

    /// `lv_anim_delete()`: delete all animations of `var` with `exec`
    /// (`None` = any exec). Returns `true` if at least one was deleted.
    pub fn anim_delete(&mut self, var: AnimTarget, exec: Option<&AnimExec>) -> bool {
        todo!()
    }

    /// `lv_anim_delete_all()`
    pub fn anim_delete_all(&mut self) {
        todo!()
    }

    /// `lv_anim_get()`: find an animation of `var` with `exec`.
    pub fn anim_get(&self, var: AnimTarget, exec: Option<&AnimExec>) -> Option<AnimId> {
        todo!()
    }

    /// `lv_anim_get_timer()`
    pub fn anim_get_timer(&self) -> Option<TimerId> {
        self.anims.timer
    }

    /// `lv_anim_custom_delete()`
    pub fn anim_custom_delete(&mut self, a: AnimId, exec: Option<&AnimExec>) -> bool {
        todo!()
    }

    /// `lv_anim_custom_get()`
    pub fn anim_custom_get(&self, a: AnimId, exec: Option<&AnimExec>) -> Option<AnimId> {
        todo!()
    }

    /// `lv_anim_count_running()`
    pub fn anim_count_running(&self) -> u16 {
        todo!()
    }

    /// `lv_anim_speed()`
    pub fn anim_speed(speed: u32) -> u32 {
        todo!()
    }

    /// `lv_anim_speed_clamped()`
    pub fn anim_speed_clamped(speed: u32, min_time: u32, max_time: u32) -> u32 {
        todo!()
    }

    /// `lv_anim_resolve_speed()`
    pub fn anim_resolve_speed(speed_or_time: u32, start: i32, end: i32) -> u32 {
        todo!()
    }

    /// `lv_anim_speed_to_time()`
    pub fn anim_speed_to_time(speed: u32, start: i32, end: i32) -> u32 {
        todo!()
    }

    /// `lv_anim_refr_now()`: run the animation timer immediately.
    pub fn anim_refr_now(&mut self) {
        todo!()
    }

    /// `lv_anim_pause()`
    pub fn anim_pause(&mut self, a: AnimId) {
        todo!()
    }

    /// `lv_anim_pause_for()`
    pub fn anim_pause_for(&mut self, a: AnimId, ms: u32) {
        todo!()
    }

    /// `lv_anim_resume()`
    pub fn anim_resume(&mut self, a: AnimId) {
        todo!()
    }

    /// `lv_anim_enable_vsync_mode()`
    pub fn anim_enable_vsync_mode(&mut self, enable: bool) {
        todo!()
    }

    /// Borrow a running animation.
    #[inline]
    pub fn anim(&self, a: AnimId) -> Option<&Anim> {
        self.anims.anims.get(a.0)
    }

    /// Mutably borrow a running animation.
    #[inline]
    pub fn anim_mut(&mut self, a: AnimId) -> Option<&mut Anim> {
        self.anims.anims.get_mut(a.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_equality_ignores_closures() {
        let a = AnimExec::Custom {
            tag: 7,
            f: Rc::new(|_, _, _| {}),
        };
        let b = AnimExec::Custom {
            tag: 7,
            f: Rc::new(|_, _, _| {}),
        };
        let c = AnimExec::Custom {
            tag: 8,
            f: Rc::new(|_, _, _| {}),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(AnimExec::ObjX, AnimExec::ObjX);
        assert_ne!(AnimExec::ObjX, AnimExec::ObjY);
        assert_eq!(AnimPath::Linear, AnimPath::Linear);
        assert_ne!(AnimPath::Linear, AnimPath::Bounce);
        assert_eq!(ANIM_REPEAT_INFINITE, 0xFFFF_FFFF);
        let x = AnimCustomExecCb {
            tag: 1,
            f: Rc::new(|_, _, _| {}),
        };
        let y = AnimCustomExecCb {
            tag: 1,
            f: Rc::new(|_, _, _| {}),
        };
        assert_eq!(x, y);
        assert_ne!(AnimTarget::Tag(0), AnimTarget::None);
    }
}
