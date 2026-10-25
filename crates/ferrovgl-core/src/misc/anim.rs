//! Ported from lvgl v9.6.0: `src/misc/lv_anim.c` (types from `include/lvgl/core/lv_anim.h`,
//! `src/misc/lv_anim_private.h`).
//!
//! `var` + `exec_cb` become [`AnimTarget`] + [`AnimExec`]; `AnimExec`
//! implements `PartialEq` on its discriminant/tag so that
//! `lv_anim_delete(var, exec_cb)` keeps working. `custom_exec_cb` stays a
//! separate field like upstream (both are called by `anim_timer`).
//!
//! `anim_ll` is the arena [`AnimState::anims`] plus the head-first
//! [`AnimState::order`] list. An animation that is unlinked from the list
//! (`lv_ll_remove`) stays in the arena until its `completed_cb` /
//! `deleted_cb` returned, so the callbacks can still read it through
//! [`Ui::anim`] exactly like the C callbacks read the not yet freed node.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::core::obj_style::{ObjStyleSrc, StyleTransId};
use crate::display::DisplayId;
use crate::indev::IndevId;
use crate::misc::arena::{Arena, Key};
use crate::misc::color::Color;
use crate::misc::event::{Event, EventCallback};
use crate::misc::math::{BEZIER_VAL_MAX, BEZIER_VAL_SHIFT, bezier_val_float, bezier3, cubic_bezier, map};
use crate::misc::style::{
    Part, STYLE_PROP_FLAG_EXT_DRAW_UPDATE, STYLE_PROP_FLAG_TRANSFORM, StyleProp, StyleSelector, StyleValue,
    prop_has_flag_with_custom,
};
use crate::misc::timer::TimerId;

/// `LV_ANIM_RESOLUTION`: perform linear animations in max 1024 steps.
const ANIM_RESOLUTION: i32 = 1024;
/// `LV_ANIM_RES_SHIFT`: `log2(LV_ANIM_RESOLUTION)`.
const ANIM_RES_SHIFT: u32 = 10;
/// `LV_ANIM_SPEED_MASK`: in an anim. time this bit indicates that the value is speed, and not time.
const ANIM_SPEED_MASK: u32 = 0x8000_0000;

/// `DRAW_AREA_FLAGS` of `lv_obj_style.c` (used by the style transition exec callback).
const DRAW_AREA_FLAGS: u8 = STYLE_PROP_FLAG_TRANSFORM | STYLE_PROP_FLAG_EXT_DRAW_UPDATE;

/// `LV_BEZIER_VAL_FLOAT(0.42)`
const BEZIER_0_42: i32 = bezier_val_float(0.42);
/// `LV_BEZIER_VAL_FLOAT(0.58)`
const BEZIER_0_58: i32 = bezier_val_float(0.58);
/// `LV_BEZIER_VAL_FLOAT(0)`
const BEZIER_0: i32 = bezier_val_float(0.0);
/// `LV_BEZIER_VAL_FLOAT(1)`
const BEZIER_1: i32 = bezier_val_float(1.0);

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
    /// `lv_anim_init()`: zero everything, then 500 ms, `0 -> 100`, one
    /// repeat, linear path, `early_apply` on.
    pub fn new() -> Self {
        Self {
            var: AnimTarget::None,
            exec: AnimExec::None,
            custom_exec_cb: None,
            start_cb: None,
            completed_cb: None,
            deleted_cb: None,
            get_value_cb: None,
            user_data: None,
            path: AnimPath::Linear,
            start_value: 0,
            current_value: 0,
            end_value: 100,
            duration: 500,
            act_time: 0,
            reverse_delay: 0,
            reverse_duration: 0,
            repeat_delay: 0,
            repeat_cnt: 1,
            bezier3: AnimBezier3Para::default(),
            last_timer_run: 0,
            pause_time: 0,
            pause_duration: 0,
            is_paused: false,
            reverse_play_in_progress: false,
            run_round: false,
            start_cb_called: false,
            early_apply: true,
        }
    }

    /// `lv_anim_get_playtime()`: total play time including delays and
    /// repeats ([`ANIM_PLAYTIME_INFINITE`] for infinite repeats).
    pub fn playtime(&self) -> u32 {
        if self.repeat_cnt == ANIM_REPEAT_INFINITE {
            return ANIM_PLAYTIME_INFINITE;
        }

        let mut repeat_cnt = self.repeat_cnt;
        if repeat_cnt < 1 {
            repeat_cnt = 1;
        }

        let mut playtime = self
            .repeat_delay
            .wrapping_add(self.duration as u32)
            .wrapping_add(self.reverse_delay)
            .wrapping_add(self.reverse_duration);
        playtime = playtime.wrapping_mul(repeat_cnt);
        playtime
    }

    /// `lv_anim_path_linear()`: calculate the current value of an animation
    /// applying linear characteristic.
    pub fn path_linear(&self) -> i32 {
        // Calculate the current step
        let step = map(self.act_time, 0, self.duration, 0, ANIM_RESOLUTION);

        // Get the new value which will be proportional to `step`
        // and the `start` and `end` values
        let mut new_value = step.wrapping_mul(self.end_value.wrapping_sub(self.start_value));
        new_value >>= ANIM_RES_SHIFT;
        new_value = new_value.wrapping_add(self.start_value);

        new_value
    }

    /// `lv_anim_path_ease_in()`: slow at the beginning.
    pub fn path_ease_in(&self) -> i32 {
        self.path_cubic_bezier(BEZIER_0_42, BEZIER_0, BEZIER_1, BEZIER_1)
    }

    /// `lv_anim_path_ease_out()`: slow at the end.
    pub fn path_ease_out(&self) -> i32 {
        self.path_cubic_bezier(BEZIER_0, BEZIER_0, BEZIER_0_58, BEZIER_1)
    }

    /// `lv_anim_path_ease_in_out()`: slow at the beginning and at the end.
    pub fn path_ease_in_out(&self) -> i32 {
        self.path_cubic_bezier(BEZIER_0_42, BEZIER_0, BEZIER_0_58, BEZIER_1)
    }

    /// `lv_anim_path_overshoot()`: overshoot the end value.
    pub fn path_overshoot(&self) -> i32 {
        self.path_cubic_bezier(341, 0, 683, 1300)
    }

    /// `lv_anim_path_bounce()`: bounce back from the end value.
    pub fn path_bounce(&self) -> i32 {
        // Calculate the current step
        let mut t = map(self.act_time, 0, self.duration, 0, BEZIER_VAL_MAX);
        let mut diff = self.end_value.wrapping_sub(self.start_value);

        // 3 bounces has 5 parts: 3 down and 2 up. One part is t / 5 long

        if t < 408 {
            // Go down
            t = (t * 2500) >> BEZIER_VAL_SHIFT; // [0..1024] range
            t = BEZIER_VAL_MAX - t;
        } else if t >= 408 && t < 614 {
            // First bounce back
            t -= 408;
            t *= 5; // to [0..1024] range
            diff /= 20;
        } else if t >= 614 && t < 819 {
            // Fall back
            t -= 614;
            t *= 5; // to [0..1024] range
            t = BEZIER_VAL_MAX - t;
            diff /= 20;
        } else if t >= 819 && t < 921 {
            // Second bounce back
            t -= 819;
            t *= 10; // to [0..1024] range
            diff /= 40;
        } else if t >= 921 && t <= BEZIER_VAL_MAX {
            // Fall back
            t -= 921;
            t *= 10; // to [0..1024] range
            t = BEZIER_VAL_MAX - t;
            diff /= 40;
        }

        // `if(t > LV_BEZIER_VAL_MAX) t = LV_BEZIER_VAL_MAX; if(t < 0) t = 0;`
        t = t.clamp(0, BEZIER_VAL_MAX);
        let step = bezier3(t, 0, 500, 800, BEZIER_VAL_MAX);

        let mut new_value = step.wrapping_mul(diff);
        new_value >>= BEZIER_VAL_SHIFT;
        new_value = self.end_value.wrapping_sub(new_value);

        new_value
    }

    /// `lv_anim_path_step()`: `start_value` until the end, then `end_value`.
    pub fn path_step(&self) -> i32 {
        if self.act_time >= self.duration {
            self.end_value
        } else {
            self.start_value
        }
    }

    /// `lv_anim_path_custom_bezier3()`: cubic Bézier with the control points
    /// of [`Anim::bezier3`].
    pub fn path_custom_bezier3(&self) -> i32 {
        let para = &self.bezier3;
        self.path_cubic_bezier(para.x1 as i32, para.y1 as i32, para.x2 as i32, para.y2 as i32)
    }

    /// Evaluate `self.path` (dispatch of `a->path_cb(a)`).
    pub fn path_value(&self) -> i32 {
        match self.path {
            AnimPath::Linear => self.path_linear(),
            AnimPath::EaseIn => self.path_ease_in(),
            AnimPath::EaseOut => self.path_ease_out(),
            AnimPath::EaseInOut => self.path_ease_in_out(),
            AnimPath::Overshoot => self.path_overshoot(),
            AnimPath::Bounce => self.path_bounce(),
            AnimPath::Step => self.path_step(),
            AnimPath::Bezier3 => self.path_custom_bezier3(),
            AnimPath::Custom(f) => f(self),
        }
    }

    /// `lv_anim_set_var()`
    pub fn set_var(&mut self, var: AnimTarget) -> &mut Self {
        self.var = var;
        self
    }

    /// `lv_anim_set_exec_cb()`
    pub fn set_exec_cb(&mut self, exec: AnimExec) -> &mut Self {
        self.exec = exec;
        self
    }

    /// `lv_anim_set_duration()`
    pub fn set_duration(&mut self, duration: u32) -> &mut Self {
        self.duration = duration as i32;
        self
    }

    /// `lv_anim_set_delay()`: stored as a negative `act_time`.
    pub fn set_delay(&mut self, delay: u32) -> &mut Self {
        self.act_time = (delay as i32).wrapping_neg();
        self
    }

    /// `lv_anim_set_values()`
    pub fn set_values(&mut self, start: i32, end: i32) -> &mut Self {
        self.start_value = start;
        self.current_value = i32::MIN;
        self.end_value = end;
        self
    }

    /// `lv_anim_set_custom_exec_cb()`: sets `custom_exec_cb` only;
    /// `exec_cb` is left untouched.
    pub fn set_custom_exec_cb(&mut self, cb: Option<AnimCustomExecCb>) -> &mut Self {
        self.custom_exec_cb = cb;
        self
    }

    /// `lv_anim_set_path_cb()`
    pub fn set_path_cb(&mut self, path: AnimPath) -> &mut Self {
        self.path = path;
        self
    }

    /// `lv_anim_set_start_cb()`
    pub fn set_start_cb(&mut self, cb: Option<AnimCallback>) -> &mut Self {
        self.start_cb = cb;
        self
    }

    /// `lv_anim_set_get_value_cb()`
    pub fn set_get_value_cb(&mut self, cb: Option<AnimGetValueCallback>) -> &mut Self {
        self.get_value_cb = cb;
        self
    }

    /// `lv_anim_set_completed_cb()`
    pub fn set_completed_cb(&mut self, cb: Option<AnimCallback>) -> &mut Self {
        self.completed_cb = cb;
        self
    }

    /// `lv_anim_set_deleted_cb()`
    pub fn set_deleted_cb(&mut self, cb: Option<AnimCallback>) -> &mut Self {
        self.deleted_cb = cb;
        self
    }

    /// `lv_anim_set_reverse_duration()` (also `lv_anim_set_reverse_time()`).
    pub fn set_reverse_duration(&mut self, duration: u32) -> &mut Self {
        self.reverse_duration = duration;
        self
    }

    /// `lv_anim_set_reverse_time()`: alias of [`Anim::set_reverse_duration`].
    pub fn set_reverse_time(&mut self, duration: u32) -> &mut Self {
        self.set_reverse_duration(duration)
    }

    /// `lv_anim_set_reverse_delay()`
    pub fn set_reverse_delay(&mut self, delay: u32) -> &mut Self {
        self.reverse_delay = delay;
        self
    }

    /// `lv_anim_set_repeat_count()`
    pub fn set_repeat_count(&mut self, cnt: u32) -> &mut Self {
        self.repeat_cnt = cnt;
        self
    }

    /// `lv_anim_set_repeat_delay()`
    pub fn set_repeat_delay(&mut self, delay: u32) -> &mut Self {
        self.repeat_delay = delay;
        self
    }

    /// `lv_anim_set_early_apply()`
    pub fn set_early_apply(&mut self, en: bool) -> &mut Self {
        self.early_apply = en;
        self
    }

    /// `lv_anim_set_user_data()`
    pub fn set_user_data(&mut self, user_data: Option<Rc<dyn Any>>) -> &mut Self {
        self.user_data = user_data;
        self
    }

    /// `lv_anim_set_bezier3_param()`
    pub fn set_bezier3_param(&mut self, x1: i16, y1: i16, x2: i16, y2: i16) -> &mut Self {
        let para = &mut self.bezier3;

        para.x1 = x1;
        para.x2 = x2;
        para.y1 = y1;
        para.y2 = y2;
        self
    }

    /// `lv_anim_get_delay()`
    pub fn delay(&self) -> u32 {
        self.act_time.wrapping_neg() as u32
    }

    /// `lv_anim_get_time()`
    pub fn time(&self) -> u32 {
        self.duration as u32
    }

    /// `lv_anim_get_repeat_count()`
    pub fn repeat_count(&self) -> u32 {
        self.repeat_cnt
    }

    /// `lv_anim_get_user_data()`
    pub fn user_data(&self) -> Option<&Rc<dyn Any>> {
        self.user_data.as_ref()
    }

    /// `lv_anim_is_paused()`
    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    /// `lv_anim_path_cubic_bezier()` (static in `lv_anim.c`).
    fn path_cubic_bezier(&self, x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
        // Calculate the current step
        let t = map(self.act_time, 0, self.duration, 0, BEZIER_VAL_MAX) as u32;
        let step = cubic_bezier(t as i32, x1, y1, x2, y2);

        let mut new_value = step.wrapping_mul(self.end_value.wrapping_sub(self.start_value));
        new_value >>= BEZIER_VAL_SHIFT;
        new_value = new_value.wrapping_add(self.start_value);

        new_value
    }

    /// `resolve_time()` (static in `lv_anim.c`): turn speed-encoded times
    /// into real times.
    fn resolve_time(&mut self) {
        self.duration = Ui::anim_resolve_speed(self.duration as u32, self.start_value, self.end_value) as i32;
        self.reverse_duration =
            Ui::anim_resolve_speed(self.reverse_duration, self.start_value, self.end_value);
        self.reverse_delay = Ui::anim_resolve_speed(self.reverse_delay, self.start_value, self.end_value);
        self.repeat_delay = Ui::anim_resolve_speed(self.repeat_delay, self.start_value, self.end_value);
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
    /// Whether `lv_anim_core_init()` ran (it is run lazily by the first
    /// animation call when `Ui::new` did not run it).
    core_inited: bool,
    /// The `anim_vsync_event` callback, kept so that the very same `Rc`
    /// can be unregistered (C compares the function pointer).
    vsync_cb: Option<EventCallback>,
}

impl AnimState {
    /// The empty animation list; the timer is created by
    /// [`Ui::anim_core_init`].
    pub fn new() -> Self {
        Self {
            anims: Arena::new(),
            order: Vec::new(),
            timer: None,
            list_changed: false,
            run_round: false,
            vsync_registered: false,
            core_inited: false,
            vsync_cb: None,
        }
    }

    /// The animation after `id` in `anim_ll` (`lv_ll_get_next`).
    fn next_of(&self, id: AnimId) -> Option<AnimId> {
        let pos = self.order.iter().position(|&x| x == id)?;
        self.order.get(pos + 1).copied()
    }

    /// `lv_ll_remove()`
    fn unlink(&mut self, id: AnimId) {
        self.order.retain(|&x| x != id);
    }
}

impl Default for AnimState {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    /// `lv_anim_core_init()`: create the animation timer (paused while the
    /// list is empty).
    pub fn anim_core_init(&mut self) {
        self.anims.order.clear();
        self.anims.core_inited = true;
        let period = self.config().refr_period_ms;
        let timer = self.timer_create(Rc::new(|ui: &mut Ui, _t: TimerId| ui.anim_timer()), period, None);
        self.anims.timer = Some(timer);
        self.anim_mark_list_change(); // Turn off the animation timer
        self.anims.list_changed = false;
        self.anims.run_round = false;
    }

    /// Run `lv_anim_core_init()` if it has not run yet.
    fn anim_core_ensure_init(&mut self) {
        if !self.anims.core_inited {
            self.anim_core_init();
        }
    }

    /// `lv_anim_core_deinit()`
    pub fn anim_core_deinit(&mut self) {
        self.anim_delete_all();
    }

    /// `lv_anim_enable_vsync_mode()`: drive the animations from the default
    /// display's vsync events instead of a timer.
    pub fn anim_enable_vsync_mode(&mut self, enable: bool) {
        self.anim_core_ensure_init();
        if enable {
            // Remove animation timer, use vsync instead
            if let Some(timer) = self.anims.timer {
                self.timer_delete(timer);
                self.anims.timer = None;
            }
        } else if self.anims.timer.is_none() {
            let period = self.config().refr_period_ms;
            let timer = self.timer_create(Rc::new(|ui: &mut Ui, _t: TimerId| ui.anim_timer()), period, None);
            self.anims.timer = Some(timer);

            if self.anims.vsync_registered {
                self.anim_vsync_unregister();
                self.anims.vsync_registered = false;
            }
        }

        self.anim_mark_list_change();
    }

    /// `lv_anim_start()`: copy `a` into the animation list and start it.
    pub fn anim_start(&mut self, a: &Anim) -> AnimId {
        self.anim_core_ensure_init();

        // Do not let two animations for the same 'var' with the same 'exec_cb'
        if a.early_apply && (!matches!(a.exec, AnimExec::None) || a.custom_exec_cb.is_some()) {
            self.anim_remove_concurrent(None, a.var, &a.exec, a.custom_exec_cb.is_some());
        }

        // Add the new animation to the animation linked list
        let mut new_anim = a.clone();
        new_anim.run_round = self.anims.run_round;
        new_anim.last_timer_run = self.tick_get();
        new_anim.is_paused = false;
        let early_apply = new_anim.early_apply;
        let get_value_cb = new_anim.get_value_cb.clone();
        let id = AnimId(self.anims.anims.insert(new_anim));
        self.anims.order.insert(0, id);

        // Set the start value
        if early_apply {
            if let Some(cb) = get_value_cb {
                let v_ofs = cb(self, id);
                if let Some(n) = self.anim_mut(id) {
                    n.start_value = n.start_value.wrapping_add(v_ofs);
                    n.end_value = n.end_value.wrapping_add(v_ofs);
                }
            }

            let applied = self.anim_mut(id).map(|n| {
                n.resolve_time();
                n.current_value = n.path_value();
                (n.var, n.exec.clone(), n.current_value)
            });
            if let Some((var, exec, value)) = applied {
                if !matches!(exec, AnimExec::None) {
                    self.anim_call_exec(var, &exec, value);
                }
                let custom = self.anim(id).and_then(|n| n.custom_exec_cb.clone());
                if let Some(custom) = custom {
                    let value = self.anim(id).map_or(value, |n| n.current_value);
                    (custom.f)(self, id, value);
                }
            }
        }

        // Creating an animation changed the linked list.
        // It's important if it happens in a ready callback. (see `anim_timer`)
        self.anim_mark_list_change();

        id
    }

    /// `lv_anim_delete()`: delete all animations of `var` with `exec`
    /// (`AnimTarget::None` = any variable, `None` / `AnimExec::None` = any
    /// exec). Returns `true` if at least one was deleted.
    pub fn anim_delete(&mut self, var: AnimTarget, exec: Option<&AnimExec>) -> bool {
        let exec = exec.filter(|e| !matches!(e, AnimExec::None));
        let mut del_any = false;
        let mut cur = self.anims.order.first().copied();
        while let Some(id) = cur {
            let mut del = false;
            let matches = self.anim(id).is_some_and(|a| {
                (a.var == var || var == AnimTarget::None) && exec.is_none_or(|e| a.exec == *e)
            });
            if matches {
                self.anim_remove(id);
                // Read by `anim_timer`. It need to know if a delete occurred in the linked list
                self.anim_mark_list_change();
                del_any = true;
                del = true;
            }

            // Always start from the head on delete, because we don't know
            // how `anim_ll_p` was changes in `a->deleted_cb`
            cur = if del {
                self.anims.order.first().copied()
            } else {
                self.anims.next_of(id)
            };
        }

        del_any
    }

    /// `lv_anim_delete_all()`
    pub fn anim_delete_all(&mut self) {
        // lv_ll_clear_custom(anim_ll_p, remove_anim)
        let mut cur = self.anims.order.first().copied();
        while let Some(id) = cur {
            let next = self.anims.next_of(id);
            self.anim_remove(id);
            // C would read a freed node if `deleted_cb` removed `next`;
            // restart from the head instead.
            cur = match next {
                Some(n) if self.anims.order.contains(&n) => Some(n),
                _ => self.anims.order.first().copied(),
            };
        }
        self.anim_mark_list_change();
    }

    /// `lv_anim_get()`: find an animation of `var` with `exec`.
    pub fn anim_get(&self, var: AnimTarget, exec: Option<&AnimExec>) -> Option<AnimId> {
        if var == AnimTarget::None {
            crate::misc::check_arg_failed("anim_get: var is NULL");
            return None;
        }
        let exec = exec.filter(|e| !matches!(e, AnimExec::None));
        self.anims.order.iter().copied().find(|&id| {
            self.anim(id)
                .is_some_and(|a| a.var == var && exec.is_none_or(|e| a.exec == *e))
        })
    }

    /// `lv_anim_get_timer()`
    pub fn anim_get_timer(&self) -> Option<TimerId> {
        self.anims.timer
    }

    /// `lv_anim_count_running()`
    pub fn anim_count_running(&self) -> u16 {
        // `uint16_t cnt; cnt++` per node: truncating
        self.anims.order.len() as u16
    }

    /// `lv_anim_speed_clamped()`: encode a speed (units/sec) with a time
    /// range (ms) into a value usable as duration.
    pub fn anim_speed_clamped(speed: u32, min_time: u32, max_time: u32) -> u32 {
        if speed == 0 {
            crate::misc::check_arg_failed("anim_speed_clamped: speed is 0");
            return 0;
        }
        let mut speed = speed;
        let mut min_time = min_time;
        let mut max_time = max_time;

        if speed > 10000 {
            speed = 10230;
        }
        if min_time > 10000 {
            min_time = 10230;
        }
        if max_time > 10000 {
            max_time = 10230;
        }

        // Lower the resolution to fit the 0.1023 range
        speed = (speed + 5) / 10;
        min_time = (min_time + 5) / 10;
        max_time = (max_time + 5) / 10;

        ANIM_SPEED_MASK
            .wrapping_add(max_time << 20)
            .wrapping_add(min_time << 10)
            .wrapping_add(speed)
    }

    /// `lv_anim_speed()`: encode a speed (units/sec) without time limits.
    pub fn anim_speed(speed: u32) -> u32 {
        if speed == 0 {
            crate::misc::check_arg_failed("anim_speed: speed is 0");
            return 0;
        }
        Self::anim_speed_clamped(speed, 0, 10000)
    }

    /// `lv_anim_speed_to_time()`: the time needed to go from `start` to
    /// `end` with `speed` units/sec (at least 1 ms).
    pub fn anim_speed_to_time(speed: u32, start: i32, end: i32) -> u32 {
        if speed == 0 {
            crate::misc::check_arg_failed("anim_speed_to_time: speed is 0");
            return 0;
        }
        let d = abs_diff_i32(start, end);
        let mut time = d.wrapping_mul(1000) / speed;

        time = if time == 0 { 1 } else { time };

        time
    }

    /// `lv_anim_refr_now()`: run the animation timer immediately.
    pub fn anim_refr_now(&mut self) {
        self.anim_timer();
    }

    /// `lv_anim_custom_delete()`: `lv_anim_delete(a ? a->var : NULL, exec_cb)`.
    pub fn anim_custom_delete(&mut self, a: AnimId, exec: Option<&AnimExec>) -> bool {
        let var = self.anim(a).map_or(AnimTarget::None, |x| x.var);
        self.anim_delete(var, exec)
    }

    /// `lv_anim_custom_get()`: `lv_anim_get(a ? a->var : NULL, exec_cb)`.
    pub fn anim_custom_get(&self, a: AnimId, exec: Option<&AnimExec>) -> Option<AnimId> {
        let var = self.anim(a).map_or(AnimTarget::None, |x| x.var);
        self.anim_get(var, exec)
    }

    /// `lv_anim_resolve_speed()`: decode a value from
    /// [`Ui::anim_speed_clamped`] into a time; plain times are returned as is.
    pub fn anim_resolve_speed(speed_or_time: u32, start: i32, end: i32) -> u32 {
        // It was a simple time
        if (speed_or_time & ANIM_SPEED_MASK) == 0 {
            return speed_or_time;
        }

        let d = abs_diff_i32(start, end);
        let speed = speed_or_time & 0x3FF;
        let time = d.wrapping_mul(100) / speed; // Speed is in 10 units per sec
        let max_time = (speed_or_time >> 20) & 0x3FF;
        let min_time = (speed_or_time >> 10) & 0x3FF;

        // LV_CLAMP(min, val, max) = LV_MIN(LV_MAX(val, min), max)
        time.max(min_time * 10).min(max_time * 10)
    }

    /// `lv_anim_is_paused()`
    pub fn anim_is_paused(&self, a: AnimId) -> bool {
        match self.anim(a) {
            Some(a) => a.is_paused,
            None => {
                crate::misc::check_arg_failed("anim_is_paused: invalid anim");
                false
            }
        }
    }

    /// `lv_anim_pause()`: pause until [`Ui::anim_resume`].
    pub fn anim_pause(&mut self, a: AnimId) {
        self.anim_pause_for_internal(a, ANIM_PAUSE_FOREVER);
    }

    /// `lv_anim_pause_for()`: pause for `ms` milliseconds.
    pub fn anim_pause_for(&mut self, a: AnimId, ms: u32) {
        self.anim_pause_for_internal(a, ms);
    }

    /// `lv_anim_resume()`
    pub fn anim_resume(&mut self, a: AnimId) {
        let run_round = self.anims.run_round;
        let Some(a) = self.anim_mut(a) else {
            crate::misc::check_arg_failed("anim_resume: invalid anim");
            return;
        };
        a.is_paused = false;
        a.pause_duration = 0;
        a.run_round = run_round;
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

    /// `anim_timer()` (static in `lv_anim.c`): periodically handle the
    /// animations.
    fn anim_timer(&mut self) {
        // Flip the run round
        self.anims.run_round = !self.anims.run_round;

        let mut cur = self.anims.order.first().copied();
        while let Some(id) = cur {
            self.anim_timer_step(id);

            // If the linked list changed due to anim. delete then it's not safe to continue
            // the reading of the list from here -> start from the head
            cur = if self.anims.list_changed {
                self.anims.order.first().copied()
            } else {
                match self.anims.next_of(id) {
                    Some(n) => Some(n),
                    // `id` left the list without a list change mark: only
                    // possible if a callback freed it behind our back.
                    None if !self.anims.order.contains(&id) => self.anims.order.first().copied(),
                    None => None,
                }
            };
        }
    }

    /// The body of the `anim_timer()` loop for one animation.
    fn anim_timer_step(&mut self, id: AnimId) {
        let Some(a) = self.anim(id) else {
            self.anims.list_changed = true;
            return;
        };
        let elaps = self.tick_elaps(a.last_timer_run);

        if a.is_paused {
            let time_paused = self.tick_elaps(a.pause_time);
            let pause_duration = a.pause_duration;
            let is_pause_over = pause_duration != ANIM_PAUSE_FOREVER && time_paused >= pause_duration;

            if is_pause_over {
                let pause_overrun = time_paused.wrapping_sub(pause_duration);
                let run_round = self.anims.run_round;
                if let Some(a) = self.anim_mut(id) {
                    a.is_paused = false;
                    a.act_time = a.act_time.wrapping_add(pause_overrun as i32);
                    a.run_round = !run_round;
                }
            }
        } else if let Some(a) = self.anim_mut(id) {
            a.act_time = a.act_time.wrapping_add(elaps as i32);
        }
        let tick = self.tick_get();
        if let Some(a) = self.anim_mut(id) {
            a.last_timer_run = tick;
        }

        // It can be set by `lv_anim_delete()` typically in `end_cb`. If set then an animation delete
        // happened in `anim_completed_handler` which could make this linked list reading corrupt
        // because the list is changed meanwhile
        self.anims.list_changed = false;

        let run_round = self.anims.run_round;
        let Some(a) = self.anim_mut(id) else { return };
        if a.is_paused || a.run_round == run_round {
            return;
        }
        a.run_round = run_round; // The list readying might be reset so need to know which anim has run already

        // The animation will run now for the first time. Call `start_cb`
        if !a.start_cb_called && a.act_time >= 0 {
            if !a.early_apply
                && let Some(cb) = a.get_value_cb.clone()
            {
                let v_ofs = cb(self, id);
                // C would write into a freed node if `get_value_cb` deleted it.
                let Some(a) = self.anim_mut(id) else { return };
                a.start_value = a.start_value.wrapping_add(v_ofs);
                a.end_value = a.end_value.wrapping_add(v_ofs);
            }

            let Some(a) = self.anim_mut(id) else { return };
            a.resolve_time();

            if let Some(cb) = a.start_cb.clone() {
                cb(self, id);
            }
            // C would write into a freed node if `start_cb` deleted it.
            let Some(a) = self.anim_mut(id) else { return };
            a.start_cb_called = true;

            // Do not let two animations for the same 'var' with the same 'exec_cb'
            let var = a.var;
            let exec = a.exec.clone();
            let has_custom = a.custom_exec_cb.is_some();
            self.anim_remove_concurrent(Some(id), var, &exec, has_custom);
        }

        let Some(a) = self.anim_mut(id) else { return };
        if a.act_time < 0 {
            return;
        }
        let act_time_original = a.act_time; // The unclipped version is used later to correctly repeat the animation
        if a.act_time > a.duration {
            a.act_time = a.duration;
        }

        let act_time_before_exec = a.act_time;
        let new_value = a.path_value();

        if new_value != a.current_value {
            a.current_value = new_value;
            let var = a.var;
            let exec = a.exec.clone();
            let custom = a.custom_exec_cb.clone();
            // Apply the calculated value
            if !matches!(exec, AnimExec::None) {
                self.anim_call_exec(var, &exec, new_value);
            }
            if !self.anims.list_changed
                && let Some(custom) = custom
            {
                (custom.f)(self, id, new_value);
            }
        }

        if !self.anims.list_changed {
            let Some(a) = self.anim_mut(id) else { return };
            // Restore the original time to see if there is over time, ignoring silly values.
            // Restore only if it wasn't changed in the `exec_cb` for some special reasons.
            if a.act_time == act_time_before_exec && act_time_original < a.duration.wrapping_mul(2) {
                a.act_time = act_time_original;
            }

            // If the time is elapsed the animation is ready
            if a.act_time >= a.duration {
                self.anim_completed_handler(id);
            }
        }
    }

    /// `anim_completed_handler()` (static in `lv_anim.c`): called when an
    /// animation is completed to do the necessary things, e.g. repeat, play
    /// in reverse, delete etc.
    fn anim_completed_handler(&mut self, id: AnimId) {
        let Some(a) = self.anim_mut(id) else { return };
        // In the end of a forward anim decrement repeat cnt.
        if !a.reverse_play_in_progress && a.repeat_cnt > 0 && a.repeat_cnt != ANIM_REPEAT_INFINITE {
            a.repeat_cnt -= 1;
        }

        // Delete animation if
        // - no repeat left and no reverse play scheduled (simple one shot animation); or
        // - no repeat, reverse play enabled (reverse_duration != 0) and reverse play is completed.
        if a.repeat_cnt == 0 && (a.reverse_duration == 0 || a.reverse_play_in_progress) {
            // Delete the animation from the list.
            // This way the `completed_cb` will see the animations like it's animation is already deleted
            self.anims.unlink(id);
            // Flag that the list has changed
            self.anim_mark_list_change();

            // Call the callback function at the end
            if let Some(cb) = self.anim(id).and_then(|a| a.completed_cb.clone()) {
                cb(self, id);
            }
            if let Some(cb) = self.anim(id).and_then(|a| a.deleted_cb.clone()) {
                cb(self, id);
            }
            // lv_free(a)
            self.anims.anims.remove(id.0);
        }
        // If the animation is not deleted then restart it
        else {
            // Restart the animation. If the time is over a little compensate it.
            let mut over_time: i32 = 0;
            a.start_cb_called = false;
            if a.act_time > a.duration {
                over_time = a.act_time.wrapping_sub(a.duration);
            }
            a.act_time = over_time.wrapping_sub(a.repeat_delay as i32);
            // Swap start and end values in reverse-play mode
            if a.reverse_duration != 0 {
                // If now now playing in reverse, use the 'reverse_delay'.
                if !a.reverse_play_in_progress {
                    a.act_time = (a.reverse_delay as i32).wrapping_neg();
                }

                // Toggle reverse-play state
                a.reverse_play_in_progress = !a.reverse_play_in_progress;
                // Swap the start and end values
                core::mem::swap(&mut a.start_value, &mut a.end_value);
                // Swap the time and reverse_duration
                let tmp = a.duration;
                a.duration = a.reverse_duration as i32;
                a.reverse_duration = tmp as u32;
            }
        }
    }

    /// `anim_vsync_event()` (static in `lv_anim.c`).
    fn anim_vsync_event(&mut self) {
        self.anim_timer();
    }

    /// The `anim_vsync_event` callback as a shareable [`EventCallback`].
    fn anim_vsync_cb(&mut self) -> EventCallback {
        if let Some(cb) = &self.anims.vsync_cb {
            return cb.clone();
        }
        let cb: EventCallback = Rc::new(|ui: &mut Ui, _e: &mut Event<'_>| ui.anim_vsync_event());
        self.anims.vsync_cb = Some(cb.clone());
        cb
    }

    /// `lv_display_register_vsync_event(lv_display_get_default(), anim_vsync_event, NULL)`
    fn anim_vsync_register(&mut self) {
        let cb = self.anim_vsync_cb();
        match self.display_get_default() {
            Some(disp) => {
                self.display_register_vsync_event(disp, cb, None);
            }
            None => crate::misc::check_arg_failed("anim: no default display for vsync"),
        }
    }

    /// `lv_display_unregister_vsync_event(lv_display_get_default(), anim_vsync_event, NULL)`
    fn anim_vsync_unregister(&mut self) {
        let cb = self.anim_vsync_cb();
        match self.display_get_default() {
            Some(disp) => {
                self.display_unregister_vsync_event(disp, &cb, None);
            }
            None => crate::misc::check_arg_failed("anim: no default display for vsync"),
        }
    }

    /// `anim_mark_list_change()` (static in `lv_anim.c`): flag the change
    /// and pause / resume the animation timer (or vsync registration).
    fn anim_mark_list_change(&mut self) {
        self.anims.list_changed = true;
        if self.anims.order.is_empty() {
            if let Some(timer) = self.anims.timer {
                self.timer_pause(timer);
                return;
            }

            if self.anims.vsync_registered {
                self.anim_vsync_unregister();
                self.anims.vsync_registered = false;
            }

            return;
        }

        if let Some(timer) = self.anims.timer {
            self.timer_resume(timer);
            return;
        }

        if !self.anims.vsync_registered {
            self.anim_vsync_register();
            self.anims.vsync_registered = true;
        }
    }

    /// `lv_anim_pause_for_internal()` (static in `lv_anim.c`).
    fn anim_pause_for_internal(&mut self, a: AnimId, ms: u32) {
        let tick = self.tick_get();
        let Some(a) = self.anim_mut(a) else {
            crate::misc::check_arg_failed("anim_pause: invalid anim");
            return;
        };
        a.is_paused = true;
        a.pause_time = tick;
        a.pause_duration = ms;
    }

    /// `remove_concurrent_anims()` (static in `lv_anim.c`): remove the
    /// animations which are animating the same `var` with the same
    /// `exec_cb` and are already running or have `early_apply`.
    ///
    /// `current` is the animation itself when it is in the list
    /// (`a != a_current`); `None` for the template given to `lv_anim_start`.
    /// Returns `true` if at least one animation was deleted.
    fn anim_remove_concurrent(
        &mut self,
        current: Option<AnimId>,
        var: AnimTarget,
        exec: &AnimExec,
        has_custom_exec: bool,
    ) -> bool {
        if matches!(exec, AnimExec::None) && !has_custom_exec {
            return false;
        }

        let mut del_any = false;
        let mut cur = self.anims.order.first().copied();
        while let Some(id) = cur {
            let mut del = false;
            // We can't test for custom_exec_cb equality because in the MicroPython binding
            // a wrapper callback is used here an the real callback data is stored in the `user_data`.
            // Therefore equality check would remove all animations.
            let matches = Some(id) != current
                && self.anim(id).is_some_and(|a| {
                    (a.act_time >= 0 || a.early_apply)
                        && a.var == var
                        && (!matches!(a.exec, AnimExec::None) && a.exec == *exec)
                });
            if matches {
                self.anims.unlink(id);
                if let Some(cb) = self.anim(id).and_then(|a| a.deleted_cb.clone()) {
                    cb(self, id);
                }
                self.anims.anims.remove(id.0);
                // Read by `anim_timer`. It need to know if a delete occurred in the linked list
                self.anim_mark_list_change();

                del_any = true;
                del = true;
            }

            // Always start from the head on delete, because we don't know
            // how `anim_ll_p` was changes in `a->deleted_cb`
            cur = if del {
                self.anims.order.first().copied()
            } else {
                self.anims.next_of(id)
            };
        }

        del_any
    }

    /// `remove_anim()` (static in `lv_anim.c`): unlink, call `deleted_cb`, free.
    fn anim_remove(&mut self, id: AnimId) {
        self.anims.unlink(id);
        if let Some(cb) = self.anim(id).and_then(|a| a.deleted_cb.clone()) {
            cb(self, id);
        }
        self.anims.anims.remove(id.0);
    }

    /// `a->exec_cb(a->var, v)`: resolve an [`AnimExec`] to the Ui method it
    /// stands for. A target of the wrong kind is ignored.
    fn anim_call_exec(&mut self, var: AnimTarget, exec: &AnimExec, v: i32) {
        match exec {
            AnimExec::None => {}
            AnimExec::ObjX => {
                if let AnimTarget::Obj(obj) = var {
                    self.obj_set_x(obj, v);
                }
            }
            AnimExec::ObjY => {
                if let AnimTarget::Obj(obj) = var {
                    self.obj_set_y(obj, v);
                }
            }
            AnimExec::ObjWidth => {
                if let AnimTarget::Obj(obj) = var {
                    self.obj_set_width(obj, v);
                }
            }
            AnimExec::ObjHeight => {
                if let AnimTarget::Obj(obj) = var {
                    self.obj_set_height(obj, v);
                }
            }
            AnimExec::ObjScrollX => {
                // scroll_x_anim() of lv_obj_scroll.c
                if let AnimTarget::Obj(obj) = var {
                    let sx = self.obj_get_scroll_x(obj);
                    let _ = self.obj_scroll_by_raw(obj, v.wrapping_add(sx), 0);
                }
            }
            AnimExec::ObjScrollY => {
                // scroll_y_anim() of lv_obj_scroll.c
                if let AnimTarget::Obj(obj) = var {
                    let sy = self.obj_get_scroll_y(obj);
                    let _ = self.obj_scroll_by_raw(obj, 0, v.wrapping_add(sy));
                }
            }
            AnimExec::ObjOpa(selector) => {
                // lv_obj_set_style_opa(obj, v, selector): `v` is passed as `lv_opa_t`
                if let AnimTarget::Obj(obj) = var {
                    self.obj_set_local_style_prop(
                        obj,
                        StyleProp::OPA,
                        StyleValue::Num(v as u8 as i32),
                        *selector,
                    );
                }
            }
            AnimExec::StyleProp { prop, selector } => {
                if let AnimTarget::Obj(obj) = var {
                    self.obj_set_local_style_prop(obj, *prop, StyleValue::Num(v), *selector);
                }
            }
            AnimExec::StyleTrans { .. } => {
                if let AnimTarget::StyleTrans(tr) = var {
                    self.anim_exec_style_trans(tr, v);
                }
            }
            AnimExec::Custom { f, .. } => f(self, var, v),
        }
    }

    /// `trans_anim_cb()` of `lv_obj_style.c` (the exec callback of
    /// [`AnimExec::StyleTrans`]): write the interpolated value into the
    /// object's transition style.
    fn anim_exec_style_trans(&mut self, tr: StyleTransId, v: i32) {
        let Some(t) = self.style_trans.get(tr.0) else {
            return;
        };
        let obj = t.obj;
        let prop = t.prop;
        let selector = t.selector;
        let start_value = t.start_value.clone();
        let end_value = t.end_value.clone();
        let draw_area = prop_has_flag_with_custom(prop, DRAW_AREA_FLAGS, &self.style_custom_prop_flags);

        let Some(od) = self.obj(obj) else { return };
        let style_cnt = od.styles.len();

        // Find the transition style of of the descriptor
        for i in 0..style_cnt {
            let Some(os) = self.obj(obj).and_then(|od| od.styles.get(i)) else {
                return;
            };
            if !os.is_trans() || os.selector != selector {
                continue;
            }

            let value_final = match prop {
                StyleProp::BORDER_SIDE | StyleProp::BORDER_POST | StyleProp::BLEND_MODE => {
                    if v < 255 {
                        StyleValue::Num(start_value.num())
                    } else {
                        StyleValue::Num(end_value.num())
                    }
                }
                StyleProp::TRANSITION | StyleProp::TEXT_FONT => {
                    if v < 255 {
                        start_value.clone()
                    } else {
                        end_value.clone()
                    }
                }
                StyleProp::COLOR_FILTER_DSC => {
                    if matches!(start_value, StyleValue::Null) {
                        end_value.clone()
                    } else if matches!(end_value, StyleValue::Null) || v < 128 {
                        start_value.clone()
                    } else {
                        end_value.clone()
                    }
                }
                StyleProp::RECOLOR
                | StyleProp::BG_COLOR
                | StyleProp::BG_GRAD_COLOR
                | StyleProp::BORDER_COLOR
                | StyleProp::TEXT_COLOR
                | StyleProp::SHADOW_COLOR
                | StyleProp::OUTLINE_COLOR
                | StyleProp::IMAGE_RECOLOR => {
                    if v <= 0 {
                        StyleValue::Color(start_value.color())
                    } else if v >= 255 {
                        StyleValue::Color(end_value.color())
                    } else {
                        StyleValue::Color(Color::mix(end_value.color(), start_value.color(), v as u8))
                    }
                }
                _ => {
                    if v == 0 {
                        start_value.clone()
                    } else if v == 255 {
                        end_value.clone()
                    } else {
                        let s = start_value.num();
                        let e = end_value.num();
                        StyleValue::Num(s.wrapping_add(e.wrapping_sub(s).wrapping_mul(v) >> 8))
                    }
                }
            };

            // Don't do anything is the prop didn't changed
            if let ObjStyleSrc::Trans(style) = &os.src
                && let Some(old_value) = style.get_prop(prop)
                && *old_value == value_final
            {
                break;
            }

            // If the draw size changes invalidate the old area first
            let part = selector.part();
            if part == Part::MAIN && draw_area {
                let _ = self.obj_invalidate(obj);
            }
            let Some(od) = self.obj_mut(obj) else { return };
            if let Some(os) = od.styles.get_mut(i)
                && let ObjStyleSrc::Trans(style) = &mut os.src
            {
                style.set_prop(prop, value_final);
            }

            // LV_OBJ_STYLE_CACHE
            let prop_shifted = 1u32 << (prop.0 >> 3);
            if part == Part::MAIN {
                od.style_main_prop_is_set |= prop_shifted;
            } else {
                od.style_other_prop_is_set |= prop_shifted;
            }

            self.obj_refresh_style(obj, Part(selector.0), prop);
            break;
        }
    }
}

/// `LV_ABS(start - end)` as `uint32_t` (C wraps the subtraction).
#[inline]
fn abs_diff_i32(start: i32, end: i32) -> u32 {
    let x = start.wrapping_sub(end);
    (if x > 0 { x } else { x.wrapping_neg() }) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::UiConfig;
    use core::cell::Cell;

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

    // ---- helpers mirroring the upstream test harness ----

    fn ui() -> Ui {
        Ui::new(UiConfig::default())
    }

    /// `lv_test_wait()`: `lv_tick_inc(1); lv_timer_handler();` per ms, then
    /// `lv_refr_now()` (which starts with `lv_anim_refr_now()`).
    fn wait(ui: &mut Ui, ms: u32) {
        for _ in 0..ms {
            ui.tick_inc(1);
            ui.timer_handler();
        }
        ui.anim_refr_now();
    }

    /// `exec_cb` of test_anim.c: `*var = v`.
    fn exec_into(cell: &Rc<Cell<i32>>) -> AnimExec {
        let c = cell.clone();
        AnimExec::Custom {
            tag: 1,
            f: Rc::new(move |_, _, v| c.set(v)),
        }
    }

    /// The usual `0 -> 100` in 100 ms animation of test_anim.c.
    fn base_anim(var: AnimTarget, exec: AnimExec) -> Anim {
        let mut a = Anim::new();
        a.set_var(var)
            .set_values(0, 100)
            .set_exec_cb(exec)
            .set_duration(100);
        a
    }

    // ---- tests/src/test_cases/test_anim.c ----

    #[test]
    fn test_anim_delete() {
        let mut ui = ui();
        let var = Rc::new(Cell::new(0));
        let exec = exec_into(&var);
        let target = AnimTarget::Tag(1);
        ui.anim_start(&base_anim(target, exec.clone()));

        wait(&mut ui, 20);
        assert_eq!(var.get(), 19);

        // Wrong variable, nothing should happen
        let deleted = ui.anim_delete(AnimTarget::Tag(2), Some(&exec));
        assert!(!deleted);

        wait(&mut ui, 20);
        assert_eq!(var.get(), 39);

        // The value shouldn't change after delete
        let deleted = ui.anim_delete(target, Some(&exec));
        assert!(deleted);

        wait(&mut ui, 20);
        assert_eq!(var.get(), 39);
    }

    #[test]
    fn test_anim_delete_custom() {
        let mut ui = ui();
        let var = Rc::new(Cell::new(0));
        let target = AnimTarget::Tag(1);
        let mut a = base_anim(target, AnimExec::None);
        let c = var.clone();
        a.set_custom_exec_cb(Some(AnimCustomExecCb {
            tag: 2,
            f: Rc::new(move |_, _, v| c.set(v)),
        }));
        ui.anim_start(&a);

        wait(&mut ui, 20);
        assert_eq!(var.get(), 19);

        // Wrong callback, nothing should happen
        let deleted = ui.anim_delete(target, Some(&exec_into(&var)));
        assert!(!deleted);

        wait(&mut ui, 20);
        assert_eq!(var.get(), 39);

        // The value shouldn't change after delete
        let deleted = ui.anim_delete(target, None);
        assert!(deleted);

        wait(&mut ui, 20);
        assert_eq!(var.get(), 39);
    }

    #[test]
    fn test_anim_pause() {
        let mut ui = ui();
        let var = Rc::new(Cell::new(0));
        let animation = ui.anim_start(&base_anim(AnimTarget::Tag(1), exec_into(&var)));

        wait(&mut ui, 40);
        assert_eq!(var.get(), 39);

        ui.anim_pause(animation);
        assert!(ui.anim_is_paused(animation));

        wait(&mut ui, 40);
        assert_eq!(var.get(), 39);

        ui.anim_resume(animation);

        wait(&mut ui, 20);
        assert_eq!(var.get(), 59);

        wait(&mut ui, 41);
        assert_eq!(var.get(), 100);
        assert_eq!(ui.anim_count_running(), 0);
    }

    #[test]
    fn test_anim_pause_for() {
        let mut ui = ui();
        let var = Rc::new(Cell::new(0));
        let exec = exec_into(&var);
        let animation = ui.anim_start(&base_anim(AnimTarget::Tag(1), exec.clone()));

        ui.anim_pause_for(animation, 20);
        wait(&mut ui, 40);

        assert_eq!(var.get(), 19);

        ui.anim_pause_for(animation, 20);

        wait(&mut ui, 40);
        wait(&mut ui, 40);

        assert_eq!(var.get(), 79);

        ui.anim_delete(AnimTarget::Tag(1), Some(&exec));
    }

    #[test]
    fn test_anim_pause_for_resume() {
        let mut ui = ui();
        let var = Rc::new(Cell::new(0));
        let animation = ui.anim_start(&base_anim(AnimTarget::Tag(1), exec_into(&var)));

        ui.anim_pause_for(animation, 40);

        wait(&mut ui, 20);
        ui.anim_resume(animation);

        wait(&mut ui, 20);
        assert_eq!(var.get(), 19);
    }

    #[test]
    fn test_anim_start_cb_is_called() {
        let mut ui = ui();
        let var = Rc::new(Cell::new(0));
        let exec = exec_into(&var);
        let count = Rc::new(Cell::new(0));
        let mut a = base_anim(AnimTarget::Tag(1), exec.clone());
        a.set_user_data(Some(count.clone() as Rc<dyn Any>));
        a.set_start_cb(Some(Rc::new(|ui: &mut Ui, id: AnimId| {
            let ud = ui.anim(id).and_then(|a| a.user_data().cloned()).unwrap();
            let c = ud.downcast_ref::<Cell<i32>>().unwrap();
            c.set(c.get() + 1);
        })));
        a.set_repeat_count(2);
        ui.anim_start(&a);
        wait(&mut ui, 50);
        assert_eq!(count.get(), 1);
        wait(&mut ui, 100);
        assert_eq!(count.get(), 2);
        wait(&mut ui, 50);
        ui.anim_delete(AnimTarget::Tag(1), Some(&exec));
    }

    #[test]
    fn test_anim_vsync_mode_timer() {
        // The timer part of test_anim_vsync_mode (sending vsync events needs a display).
        let mut ui = ui();
        ui.anim_enable_vsync_mode(true);

        // anim timer should be NULL when vsync mode is enabled
        assert!(ui.anim_get_timer().is_none());

        ui.anim_enable_vsync_mode(false);
        let timer = ui.anim_get_timer().expect("timer re-created");
        // No animation: the timer is paused
        assert!(ui.timer_get_paused(timer));

        // Test normal timer mode
        let var = Rc::new(Cell::new(0));
        let mut a = base_anim(AnimTarget::Tag(1), exec_into(&var));
        a.set_values(0, 1000).set_duration(1000);
        ui.anim_start(&a);
        assert!(!ui.timer_get_paused(timer));
        wait(&mut ui, 20);
        assert_eq!(var.get(), 19);
        ui.anim_delete_all();
        assert!(ui.timer_get_paused(timer));
    }

    // ---- invariants derived from lv_anim.c ----

    #[test]
    fn init_defaults() {
        let a = Anim::new();
        assert_eq!(a.duration, 500);
        assert_eq!(a.start_value, 0);
        assert_eq!(a.end_value, 100);
        assert_eq!(a.repeat_cnt, 1);
        assert_eq!(a.path, AnimPath::Linear);
        assert!(a.early_apply);
        assert_eq!(a.current_value, 0);
        let mut a = a;
        a.set_values(3, 4);
        assert_eq!(a.current_value, i32::MIN);
        a.set_delay(30);
        assert_eq!(a.act_time, -30);
        assert_eq!(a.delay(), 30);
        assert_eq!(a.time(), 500);
    }

    #[test]
    fn playtime() {
        let mut a = Anim::new();
        a.set_duration(100)
            .set_repeat_delay(10)
            .set_reverse_delay(20)
            .set_reverse_duration(30);
        assert_eq!(a.playtime(), 160);
        a.set_repeat_count(3);
        assert_eq!(a.playtime(), 480);
        a.set_repeat_count(0);
        assert_eq!(a.playtime(), 160);
        a.set_repeat_count(ANIM_REPEAT_INFINITE);
        assert_eq!(a.playtime(), ANIM_PLAYTIME_INFINITE);
    }

    #[test]
    fn bezier_constants() {
        assert_eq!(BEZIER_0_42, 430);
        assert_eq!(BEZIER_0_58, 593);
        assert_eq!(BEZIER_0, 0);
        assert_eq!(BEZIER_1, 1024);
    }

    fn at(path: AnimPath, act_time: i32) -> i32 {
        let mut a = Anim::new();
        a.set_values(0, 1000).set_duration(1000).set_path_cb(path);
        a.act_time = act_time;
        a.path_value()
    }

    #[test]
    fn path_end_points() {
        for p in [
            AnimPath::Linear,
            AnimPath::EaseIn,
            AnimPath::EaseOut,
            AnimPath::EaseInOut,
            AnimPath::Overshoot,
            AnimPath::Step,
        ] {
            assert_eq!(at(p, 0), 0, "{p:?}");
            assert_eq!(at(p, 1000), 1000, "{p:?}");
        }
        // bounce ends at `end_value` and starts there too (it falls from it)
        assert_eq!(at(AnimPath::Bounce, 1000), 1000);
        assert_eq!(at(AnimPath::Step, 999), 0);
        // linear: lv_map(500, 0, 1000, 0, 1024) = 512; 512 * 1000 >> 10 = 500
        assert_eq!(at(AnimPath::Linear, 500), 500);
        // ease-in is below linear in the middle, ease-out above
        assert!(at(AnimPath::EaseIn, 500) < 500);
        assert!(at(AnimPath::EaseOut, 500) > 500);
        // overshoot goes above the end value
        assert!((0..=1000).map(|t| at(AnimPath::Overshoot, t)).any(|v| v > 1000));
        // custom bezier3 with the linear control points is linear-ish
        let mut a = Anim::new();
        a.set_values(0, 1000)
            .set_duration(1000)
            .set_path_cb(AnimPath::Bezier3)
            .set_bezier3_param(341, 341, 683, 683);
        a.act_time = 1000;
        assert_eq!(a.path_value(), 1000);
        // a user path
        fn half(a: &Anim) -> i32 {
            a.end_value / 2
        }
        assert_eq!(at(AnimPath::Custom(half), 10), 500);
    }

    #[test]
    fn speed_encoding() {
        assert_eq!(Ui::anim_speed(100), 0x8000_0000 + (1000 << 20) + 10);
        assert_eq!(
            Ui::anim_speed_clamped(100, 200, 3000),
            0x8000_0000 + (300 << 20) + (20 << 10) + 10
        );
        // speed 100 units/s over 50 units = 500 ms
        assert_eq!(Ui::anim_resolve_speed(Ui::anim_speed(100), 0, 50), 500);
        // clamped to min 200 ms / max 3000 ms
        assert_eq!(
            Ui::anim_resolve_speed(Ui::anim_speed_clamped(100, 200, 3000), 0, 5),
            200
        );
        assert_eq!(
            Ui::anim_resolve_speed(Ui::anim_speed_clamped(100, 200, 3000), 0, 500),
            3000
        );
        // plain times pass through
        assert_eq!(Ui::anim_resolve_speed(123, 0, 500), 123);
        assert_eq!(Ui::anim_speed_to_time(100, 0, 200), 2000);
        assert_eq!(Ui::anim_speed_to_time(100, 200, 0), 2000);
        assert_eq!(Ui::anim_speed_to_time(100_000, 0, 1), 1);
        // > 10000 is truncated to 10230
        assert_eq!(Ui::anim_speed_clamped(20000, 0, 0) & 0x3FF, 1023);
    }

    #[test]
    fn speed_is_resolved_on_start() {
        let mut ui = ui();
        let mut a = base_anim(AnimTarget::Tag(1), exec_into(&Rc::new(Cell::new(0))));
        a.set_values(0, 50).duration = Ui::anim_speed(100) as i32;
        let id = ui.anim_start(&a);
        assert_eq!(ui.anim(id).unwrap().duration, 500);
    }

    #[test]
    fn starting_same_var_and_exec_deletes_the_old_one() {
        let mut ui = ui();
        let var = Rc::new(Cell::new(0));
        let exec = exec_into(&var);
        let deleted = Rc::new(Cell::new(0));
        let d = deleted.clone();
        let mut a = base_anim(AnimTarget::Tag(1), exec.clone());
        a.set_deleted_cb(Some(Rc::new(move |ui: &mut Ui, id: AnimId| {
            // the anim is still readable from its deleted_cb
            assert!(ui.anim(id).is_some());
            d.set(d.get() + 1);
        })));
        let first = ui.anim_start(&a);
        assert_eq!(ui.anim_count_running(), 1);
        let second = ui.anim_start(&a);
        assert_eq!(ui.anim_count_running(), 1);
        assert_eq!(deleted.get(), 1);
        assert!(ui.anim(first).is_none());
        assert_eq!(ui.anim_get(AnimTarget::Tag(1), Some(&exec)), Some(second));
        assert_eq!(ui.anim_custom_get(second, None), Some(second));

        // another var or another exec is kept
        ui.anim_start(&base_anim(AnimTarget::Tag(2), exec.clone()));
        ui.anim_start(
            &base_anim(AnimTarget::Tag(1), AnimExec::ObjX)
                .set_early_apply(false)
                .clone(),
        );
        assert_eq!(ui.anim_count_running(), 3);
        // the head is the most recent one
        assert_eq!(ui.anims.order.len(), 3);
        assert!(ui.anim_custom_delete(second, None));
        assert_eq!(ui.anim_count_running(), 1);
        ui.anim_delete_all();
        assert_eq!(ui.anim_count_running(), 0);
        assert_eq!(deleted.get(), 2);
    }

    #[test]
    fn early_apply_sets_start_value_immediately() {
        let mut ui = ui();
        let var = Rc::new(Cell::new(-1));
        let mut a = base_anim(AnimTarget::Tag(1), exec_into(&var));
        a.set_values(10, 20).set_delay(50);
        ui.anim_start(&a);
        assert_eq!(var.get(), 10);
        ui.anim_delete_all();

        let var = Rc::new(Cell::new(-1));
        let started = Rc::new(Cell::new(false));
        let s = started.clone();
        let mut a = base_anim(AnimTarget::Tag(1), exec_into(&var));
        a.set_values(10, 20)
            .set_delay(50)
            .set_early_apply(false)
            .set_start_cb(Some(Rc::new(move |_: &mut Ui, _: AnimId| s.set(true))))
            .set_get_value_cb(Some(Rc::new(|_: &mut Ui, _: AnimId| 100)));
        ui.anim_start(&a);
        assert_eq!(var.get(), -1);
        wait(&mut ui, 49);
        assert_eq!(var.get(), -1);
        assert!(!started.get());
        wait(&mut ui, 1);
        assert!(started.get());
        // get_value_cb offsets start and end when the animation starts
        assert_eq!(var.get(), 110);
        wait(&mut ui, 100);
        assert_eq!(var.get(), 120);
        assert_eq!(ui.anim_count_running(), 0);
    }

    #[test]
    fn reverse_and_repeat() {
        let mut ui = ui();
        let var = Rc::new(Cell::new(0));
        let completed = Rc::new(Cell::new(0));
        let c = completed.clone();
        let mut a = base_anim(AnimTarget::Tag(1), exec_into(&var));
        a.set_reverse_duration(50)
            .set_repeat_count(2)
            .set_completed_cb(Some(Rc::new(move |ui: &mut Ui, id: AnimId| {
                // unlinked from the list but still readable
                assert_eq!(ui.anim_count_running(), 0);
                assert!(ui.anim(id).is_some());
                c.set(c.get() + 1);
            })));
        let id = ui.anim_start(&a);
        wait(&mut ui, 100);
        assert_eq!(var.get(), 100);
        assert!(ui.anim(id).unwrap().reverse_play_in_progress);
        wait(&mut ui, 25);
        assert_eq!(var.get(), 50);
        wait(&mut ui, 25);
        assert_eq!(var.get(), 0);
        assert_eq!(ui.anim(id).unwrap().repeat_cnt, 1);
        wait(&mut ui, 100);
        assert_eq!(var.get(), 100);
        assert_eq!(completed.get(), 0);
        wait(&mut ui, 50);
        assert_eq!(var.get(), 0);
        assert_eq!(completed.get(), 1);
        assert!(ui.anim(id).is_none());
    }

    #[test]
    fn delete_from_completed_cb_restarts_from_head() {
        let mut ui = ui();
        let v1 = Rc::new(Cell::new(0));
        let v2 = Rc::new(Cell::new(0));
        let mut a = base_anim(AnimTarget::Tag(1), exec_into(&v1));
        a.set_duration(10)
            .set_completed_cb(Some(Rc::new(|ui: &mut Ui, _: AnimId| {
                ui.anim_delete(AnimTarget::Tag(2), None);
            })));
        ui.anim_start(&a);
        ui.anim_start(&base_anim(AnimTarget::Tag(2), exec_into(&v2)));
        wait(&mut ui, 10);
        assert_eq!(v1.get(), 100);
        assert_eq!(ui.anim_count_running(), 0);
    }

    #[test]
    fn abs_diff_wraps_like_c() {
        assert_eq!(abs_diff_i32(0, 5), 5);
        assert_eq!(abs_diff_i32(5, 0), 5);
        assert_eq!(abs_diff_i32(i32::MIN, 0), 0x8000_0000);
    }
}
