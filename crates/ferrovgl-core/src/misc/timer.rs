//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_timer.h`, `src/misc/lv_timer_private.h`,
//! `src/misc/lv_timer.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;

use crate::core::Ui;
use crate::misc::arena::{Arena, Key};

/// `LV_NO_TIMER_READY`
pub const NO_TIMER_READY: u32 = 0xFFFF_FFFF;

/// Handle of a timer (`lv_timer_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TimerId(pub Key);

/// `lv_timer_cb_t`
pub type TimerCallback = Rc<dyn Fn(&mut Ui, TimerId)>;

/// `lv_timer_handler_resume_cb_t`
pub type TimerHandlerResumeCallback = Rc<dyn Fn(&mut Ui)>;

/// `lv_timer_t`
pub struct TimerData {
    /// `period`: how often the timer should run, in ms.
    pub period: u32,
    /// `last_run`: tick of the last run.
    pub last_run: u32,
    /// `timer_cb`
    pub cb: Option<TimerCallback>,
    /// `user_data`
    pub user_data: Option<Rc<dyn Any>>,
    /// `repeat_count`: -1 = infinite.
    pub repeat_count: i32,
    /// `paused`
    pub paused: bool,
    /// `auto_delete`: delete when `repeat_count` reaches 0.
    pub auto_delete: bool,
}

/// `lv_timer_state_t`
pub struct TimerState {
    /// `timer_ll`
    pub timers: Arena<TimerData>,
    /// Order of `timer_ll` (head first): `lv_timer_create_basic` is
    /// `lv_ll_ins_head` (`order.insert(0, id)`), so the most recently
    /// created timer runs first; `lv_timer_handler` iterates `order` and
    /// restarts from its head after a create / delete; `lv_timer_get_next`
    /// follows it too.
    pub order: Vec<TimerId>,
    /// `lv_timer_run`
    pub run: bool,
    /// `idle_last`
    pub idle_last: u8,
    /// `timer_deleted`
    pub deleted: bool,
    /// `timer_created`
    pub created: bool,
    /// `timer_time_until_next`
    pub time_until_next: u32,
    /// `already_running`
    pub already_running: bool,
    /// `periodic_last_tick`
    pub periodic_last_tick: u32,
    /// `busy_time`
    pub busy_time: u32,
    /// `idle_period_start`
    pub idle_period_start: u32,
    /// `run_cnt`
    pub run_cnt: u32,
    /// `resume_cb` (+ `resume_data`)
    pub resume_cb: Option<TimerHandlerResumeCallback>,
}

impl TimerState {
    /// `lv_timer_core_init()`
    pub fn new() -> Self {
        Self {
            timers: Arena::new(),
            order: Vec::new(),
            run: true,
            idle_last: 0,
            deleted: false,
            created: false,
            time_until_next: 0,
            already_running: false,
            periodic_last_tick: 0,
            busy_time: 0,
            idle_period_start: 0,
            run_cnt: 0,
            resume_cb: None,
        }
    }
}

impl Default for TimerState {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    /// `lv_timer_handler()`: run all ready timers. Returns the time until the
    /// next timer in ms ([`NO_TIMER_READY`] if none).
    pub fn timer_handler(&mut self) -> u32 {
        todo!()
    }

    /// `lv_timer_handler_run_in_period()`
    pub fn timer_handler_run_in_period(&mut self, period: u32) -> u32 {
        todo!()
    }

    /// `lv_timer_get_time_to_next()`
    pub fn timer_get_time_to_next(&mut self) -> u32 {
        todo!()
    }

    /// `lv_timer_periodic_handler()`
    pub fn timer_periodic_handler(&mut self) {
        todo!()
    }

    /// `lv_timer_handler_set_resume_cb()`
    pub fn timer_handler_set_resume_cb(&mut self, cb: Option<TimerHandlerResumeCallback>) {
        todo!()
    }

    /// `lv_timer_create_basic()`
    pub fn timer_create_basic(&mut self) -> TimerId {
        todo!()
    }

    /// `lv_timer_create()`
    pub fn timer_create(
        &mut self,
        cb: TimerCallback,
        period: u32,
        user_data: Option<Rc<dyn Any>>,
    ) -> TimerId {
        todo!()
    }

    /// `lv_timer_delete()`
    pub fn timer_delete(&mut self, timer: TimerId) {
        todo!()
    }

    /// `lv_timer_pause()`
    pub fn timer_pause(&mut self, timer: TimerId) {
        todo!()
    }

    /// `lv_timer_resume()`
    pub fn timer_resume(&mut self, timer: TimerId) {
        todo!()
    }

    /// `lv_timer_set_cb()`
    pub fn timer_set_cb(&mut self, timer: TimerId, cb: Option<TimerCallback>) {
        todo!()
    }

    /// `lv_timer_set_period()`
    pub fn timer_set_period(&mut self, timer: TimerId, period: u32) {
        todo!()
    }

    /// `lv_timer_ready()`: make the timer run on the next `timer_handler()`.
    pub fn timer_ready(&mut self, timer: TimerId) {
        todo!()
    }

    /// `lv_timer_set_repeat_count()`
    pub fn timer_set_repeat_count(&mut self, timer: TimerId, repeat_count: i32) {
        todo!()
    }

    /// `lv_timer_set_auto_delete()`
    pub fn timer_set_auto_delete(&mut self, timer: TimerId, auto_delete: bool) {
        todo!()
    }

    /// `lv_timer_set_user_data()`
    pub fn timer_set_user_data(&mut self, timer: TimerId, user_data: Option<Rc<dyn Any>>) {
        todo!()
    }

    /// `lv_timer_reset()`
    pub fn timer_reset(&mut self, timer: TimerId) {
        todo!()
    }

    /// `lv_timer_enable()`
    pub fn timer_enable(&mut self, en: bool) {
        todo!()
    }

    /// `lv_timer_get_idle()`: idle percentage.
    pub fn timer_get_idle(&self) -> u32 {
        todo!()
    }

    /// `lv_timer_get_time_until_next()`
    pub fn timer_get_time_until_next(&self) -> u32 {
        todo!()
    }

    /// `lv_timer_get_next()`: iterate the timers (`None` starts).
    pub fn timer_get_next(&self, timer: Option<TimerId>) -> Option<TimerId> {
        todo!()
    }

    /// `lv_timer_get_user_data()`
    pub fn timer_get_user_data(&self, timer: TimerId) -> Option<Rc<dyn Any>> {
        todo!()
    }

    /// `lv_timer_get_paused()`
    pub fn timer_get_paused(&self, timer: TimerId) -> bool {
        todo!()
    }

    /// Borrow a timer's data.
    #[inline]
    pub fn timer(&self, timer: TimerId) -> Option<&TimerData> {
        self.timers.timers.get(timer.0)
    }

    /// Mutably borrow a timer's data.
    #[inline]
    pub fn timer_mut(&mut self, timer: TimerId) -> Option<&mut TimerData> {
        self.timers.timers.get_mut(timer.0)
    }
}
