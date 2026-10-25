//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_timer.h`, `src/misc/lv_timer_private.h`,
//! `src/misc/lv_timer.c`.
//!
//! `timer_ll` is an [`Arena`] plus an `order` list that mirrors the linked
//! list (head first). Callbacks are `Rc`s stored in the arena; they are
//! cloned out before being called so a callback may freely create, delete,
//! pause or reset timers (including itself), exactly like upstream.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;

use crate::core::Ui;
use crate::misc::arena::{Arena, Key};

/// `LV_NO_TIMER_READY`
pub const NO_TIMER_READY: u32 = 0xFFFF_FFFF;

/// `IDLE_MEAS_PERIOD` (ms): how often the idle percentage is re-evaluated.
const IDLE_MEAS_PERIOD: u32 = 500;
/// `DEF_PERIOD` (ms): period of `lv_timer_create_basic()`.
const DEF_PERIOD: u32 = 500;

/// Handle of a timer (`lv_timer_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TimerId(pub Key);

/// `lv_timer_cb_t`
pub type TimerCallback = Rc<dyn Fn(&mut Ui, TimerId)>;

/// `lv_timer_handler_resume_cb_t`
pub type TimerHandlerResumeCallback = Rc<dyn Fn(&mut Ui)>;

/// `free_cb` of a timer's `lv_ext_data_t` (`void (*)(void * data)`).
pub type TimerExtDataFreeCb = fn(Option<Rc<dyn Any>>);

/// `lv_ext_data_t` of a timer (`LV_USE_EXT_DATA`), set by
/// `lv_timer_set_external_data()`.
#[derive(Default)]
pub struct TimerExtData {
    /// `data`; independent of the timer's application `user_data`.
    pub data: Option<Rc<dyn Any>>,
    /// `free_cb`; called by `lv_timer_delete()` only (not by
    /// `lv_timer_core_deinit()`'s `lv_ll_clear`).
    pub free_cb: Option<TimerExtDataFreeCb>,
}

/// `lv_timer_t`
pub struct TimerData {
    /// `ext_data` (`LV_USE_EXT_DATA`).
    pub ext_data: TimerExtData,
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
    /// `periodic_last_tick`: written only by `lv_timer_periodic_handler()`.
    pub periodic_last_tick: u32,
    /// The function-local `static uint32_t last_tick` of
    /// `lv_timer_handler_run_in_period()` (independent of
    /// `periodic_last_tick`; per `Ui` instead of process-global).
    pub run_in_period_last_tick: u32,
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
            run_in_period_last_tick: 0,
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
        // Avoid concurrent running of the timer handler
        if self.timers.already_running {
            return 1;
        }
        self.timers.already_running = true;

        if !self.timers.run {
            self.timers.already_running = false; // Release mutex
            return 1;
        }

        let handler_start = self.tick_get();

        if handler_start == 0 {
            self.timers.run_cnt += 1;
            if self.timers.run_cnt > 100 {
                self.timers.run_cnt = 0;
                #[cfg(feature = "log")]
                log::warn!("It seems lv_tick_inc() is not called.");
            }
        }

        // Run all timer from the list
        loop {
            self.timers.deleted = false;
            self.timers.created = false;

            let mut timer_active = self.timers.order.first().copied();
            while let Some(timer) = timer_active {
                // The timer might be deleted if it runs only once ('repeat_count = 1')
                // So get next element until the current is surely valid
                let next = self.timer_get_next(Some(timer));

                if self.timer_exec(timer) {
                    // If a timer was created or deleted then this or the next item might be corrupted
                    if self.timers.created || self.timers.deleted {
                        break;
                    }
                }

                timer_active = next; // Load the next timer
            }
            if timer_active.is_none() {
                break;
            }
        }

        let time_until_next = self.timer_get_time_to_next();

        let elaps = self.tick_elaps(handler_start);
        self.timers.busy_time = self.timers.busy_time.wrapping_add(elaps);
        let idle_period_time = self.tick_elaps(self.timers.idle_period_start);
        if idle_period_time >= IDLE_MEAS_PERIOD {
            // Calculate the busy percentage (`idle_last` is a uint8_t: truncating store)
            self.timers.idle_last = (self.timers.busy_time.wrapping_mul(100) / idle_period_time) as u8;
            // But we need idle time: `idle_last > 100 ? 0 : 100 - idle_last`
            self.timers.idle_last = 100u8.saturating_sub(self.timers.idle_last);
            self.timers.busy_time = 0;
            self.timers.idle_period_start = self.tick_get();
        }

        self.timers.time_until_next = time_until_next;
        self.timers.already_running = false; // Release the mutex

        time_until_next
    }

    /// `lv_timer_get_time_to_next()`: time till the soonest timer (ms).
    pub fn timer_get_time_to_next(&mut self) -> u32 {
        let mut time_until_next = NO_TIMER_READY;
        let mut next = self.timers.order.first().copied();

        while let Some(timer) = next {
            if time_until_next == 0 {
                break;
            }
            if let Some(data) = self.timer(timer)
                && !data.paused
            {
                let timer_remaining = self.timer_time_remaining(timer);
                time_until_next = time_until_next.min(timer_remaining);
            }

            next = self.timer_get_next(Some(timer)); // Find the next timer
        }

        time_until_next
    }

    /// `lv_timer_periodic_handler()`: call it in the super-loop; it runs
    /// [`Ui::timer_handler`] at the right time.
    pub fn timer_periodic_handler(&mut self) {
        if self.tick_elaps(self.timers.periodic_last_tick) >= self.timers.time_until_next {
            self.timer_handler();
            self.timers.periodic_last_tick = self.tick_get();
        }
    }

    /// `lv_timer_create_basic()`: a 500 ms timer without callback.
    pub fn timer_create_basic(&mut self) -> TimerId {
        self.timer_create_inner(None, DEF_PERIOD, None)
    }

    /// `lv_timer_create()`
    pub fn timer_create(
        &mut self,
        cb: TimerCallback,
        period: u32,
        user_data: Option<Rc<dyn Any>>,
    ) -> TimerId {
        self.timer_create_inner(Some(cb), period, user_data)
    }

    /// Body of `lv_timer_create()` (the callback is nullable there).
    fn timer_create_inner(
        &mut self,
        cb: Option<TimerCallback>,
        period: u32,
        user_data: Option<Rc<dyn Any>>,
    ) -> TimerId {
        let new_timer = TimerData {
            period,
            cb,
            repeat_count: -1,
            paused: false,
            last_run: self.tick_get(),
            user_data,
            auto_delete: true,
            ext_data: TimerExtData::default(),
        };
        // lv_ll_ins_head()
        let id = TimerId(self.timers.timers.insert(new_timer));
        self.timers.order.insert(0, id);

        self.timers.created = true;

        self.timer_handler_resume();

        id
    }

    /// `lv_timer_set_cb()`
    pub fn timer_set_cb(&mut self, timer: TimerId, cb: Option<TimerCallback>) {
        let Some(data) = self.timer_mut(timer) else {
            crate::misc::check_arg_failed("timer_set_cb: invalid timer");
            return;
        };
        data.cb = cb;
    }

    /// `lv_timer_delete()`: a stale handle is `timer == NULL` (ignored).
    ///
    /// Calls the `ext_data.free_cb` (if any) after the timer is unlinked,
    /// then frees the timer.
    pub fn timer_delete(&mut self, timer: TimerId) {
        // lv_ll_remove()
        let Some(mut removed) = self.timers.timers.remove(timer.0) else {
            return;
        };
        self.timers.order.retain(|&t| t != timer);
        self.timers.deleted = true;

        if let Some(free_cb) = removed.ext_data.free_cb {
            free_cb(removed.ext_data.data.take());
        }

        // lv_free()
        drop(removed);
    }

    /// `lv_timer_pause()`
    pub fn timer_pause(&mut self, timer: TimerId) {
        let Some(data) = self.timer_mut(timer) else {
            crate::misc::check_arg_failed("timer_pause: invalid timer");
            return;
        };
        data.paused = true;
    }

    /// `lv_timer_resume()`
    pub fn timer_resume(&mut self, timer: TimerId) {
        let Some(data) = self.timer_mut(timer) else {
            crate::misc::check_arg_failed("timer_resume: invalid timer");
            return;
        };
        data.paused = false;
        self.timer_handler_resume();
    }

    /// `lv_timer_set_period()`
    pub fn timer_set_period(&mut self, timer: TimerId, period: u32) {
        let Some(data) = self.timer_mut(timer) else {
            crate::misc::check_arg_failed("timer_set_period: invalid timer");
            return;
        };
        data.period = period;
        self.timer_handler_resume();
    }

    /// `lv_timer_ready()`: make the timer run on the next `timer_handler()`.
    pub fn timer_ready(&mut self, timer: TimerId) {
        // LV_CHECK_ARG runs before lv_tick_get()
        if self.timer(timer).is_none() {
            crate::misc::check_arg_failed("timer_ready: invalid timer");
            return;
        }
        let tick = self.tick_get();
        let Some(data) = self.timer_mut(timer) else {
            return;
        };
        data.last_run = tick.wrapping_sub(data.period).wrapping_sub(1);
        self.timer_handler_resume();
    }

    /// `lv_timer_set_repeat_count()`: `1` = one time, `-1` = infinity, `n > 0` = residual times.
    pub fn timer_set_repeat_count(&mut self, timer: TimerId, repeat_count: i32) {
        let Some(data) = self.timer_mut(timer) else {
            crate::misc::check_arg_failed("timer_set_repeat_count: invalid timer");
            return;
        };
        data.repeat_count = repeat_count;
    }

    /// `lv_timer_set_auto_delete()`
    pub fn timer_set_auto_delete(&mut self, timer: TimerId, auto_delete: bool) {
        let Some(data) = self.timer_mut(timer) else {
            crate::misc::check_arg_failed("timer_set_auto_delete: invalid timer");
            return;
        };
        data.auto_delete = auto_delete;
    }

    /// `lv_timer_set_user_data()`
    pub fn timer_set_user_data(&mut self, timer: TimerId, user_data: Option<Rc<dyn Any>>) {
        let Some(data) = self.timer_mut(timer) else {
            crate::misc::check_arg_failed("timer_set_user_data: invalid timer");
            return;
        };
        data.user_data = user_data;
    }

    /// `lv_timer_reset()`: restart the period from now.
    pub fn timer_reset(&mut self, timer: TimerId) {
        // LV_CHECK_ARG runs before lv_tick_get()
        if self.timer(timer).is_none() {
            crate::misc::check_arg_failed("timer_reset: invalid timer");
            return;
        }
        let tick = self.tick_get();
        let Some(data) = self.timer_mut(timer) else {
            return;
        };
        data.last_run = tick;
        self.timer_handler_resume();
    }

    /// `lv_timer_enable()`
    pub fn timer_enable(&mut self, en: bool) {
        self.timers.run = en;
        if en {
            self.timer_handler_resume();
        }
    }

    /// `lv_timer_core_deinit()`: disable the handler and delete every timer.
    pub fn timer_core_deinit(&mut self) {
        self.timer_enable(false);

        // lv_ll_clear()
        self.timers.timers.clear();
        self.timers.order.clear();
    }

    /// `lv_timer_get_idle()`: idle percentage.
    pub fn timer_get_idle(&self) -> u32 {
        u32::from(self.timers.idle_last)
    }

    /// `lv_timer_get_time_until_next()`
    pub fn timer_get_time_until_next(&self) -> u32 {
        self.timers.time_until_next
    }

    /// `lv_timer_get_next()`: iterate the timers (`None` starts).
    pub fn timer_get_next(&self, timer: Option<TimerId>) -> Option<TimerId> {
        match timer {
            None => self.timers.order.first().copied(),
            Some(timer) => {
                let order = &self.timers.order;
                let pos = order.iter().position(|&t| t == timer)?;
                order.get(pos + 1).copied()
            }
        }
    }

    /// `lv_timer_handler_run_in_period()`: call it in the super-loop; it runs
    /// [`Ui::timer_handler`] every `period` ms. Returns the time after which
    /// it must be called again.
    pub fn timer_handler_run_in_period(&mut self, period: u32) -> u32 {
        // `static uint32_t last_tick` upstream.
        if self.tick_elaps(self.timers.run_in_period_last_tick) >= period {
            self.timers.run_in_period_last_tick = self.tick_get();
            return self.timer_handler();
        }
        1
    }

    /// `lv_timer_get_user_data()`
    pub fn timer_get_user_data(&self, timer: TimerId) -> Option<Rc<dyn Any>> {
        let Some(data) = self.timer(timer) else {
            crate::misc::check_arg_failed("timer_get_user_data: invalid timer");
            return None;
        };
        data.user_data.clone()
    }

    /// `lv_timer_get_paused()`
    pub fn timer_get_paused(&self, timer: TimerId) -> bool {
        let Some(data) = self.timer(timer) else {
            crate::misc::check_arg_failed("timer_get_paused: invalid timer");
            return false;
        };
        data.paused
    }

    /// `lv_timer_set_external_data()`: attach `data` and the `free_cb` that
    /// `lv_timer_delete()` calls with it.
    pub fn timer_set_external_data(
        &mut self,
        timer: TimerId,
        data: Option<Rc<dyn Any>>,
        free_cb: Option<TimerExtDataFreeCb>,
    ) {
        let Some(t) = self.timer_mut(timer) else {
            crate::misc::check_arg_failed("timer_set_external_data: invalid timer");
            return;
        };
        t.ext_data.data = data;
        t.ext_data.free_cb = free_cb;
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

    /// `lv_timer_exec()`: execute the timer if its remaining time is zero.
    /// Returns `true` if it was executed.
    fn timer_exec(&mut self, timer: TimerId) -> bool {
        let Some(data) = self.timer(timer) else {
            return false;
        };
        if data.paused {
            return false;
        }

        let mut exec = false;
        if self.timer_time_remaining(timer) == 0 {
            // Decrement the repeat count before executing the timer_cb.
            // If any timer is deleted `if(timer->repeat_count == 0)` is not executed below
            // but at least the repeat count is zero and the timer can be deleted in the next round
            let tick = self.tick_get();
            let Some(data) = self.timer_mut(timer) else {
                return false;
            };
            let original_repeat_count = data.repeat_count;
            if data.repeat_count > 0 {
                data.repeat_count -= 1;
            }
            data.last_run = tick;

            if let Some(cb) = data.cb.clone()
                && original_repeat_count != 0
            {
                cb(self, timer);
            }

            exec = true;
        }

        if !self.timers.deleted {
            // The timer might be deleted by itself as well
            if let Some(data) = self.timer(timer)
                && data.repeat_count == 0
            {
                // The repeat count is over, delete the timer
                if data.auto_delete {
                    self.timer_delete(timer);
                } else {
                    self.timer_pause(timer);
                }
            }
        }

        exec
    }

    /// `lv_timer_time_remaining()`: how much time remains before the timer
    /// must run (0 if it is due).
    fn timer_time_remaining(&self, timer: TimerId) -> u32 {
        let Some(data) = self.timer(timer) else {
            return 0;
        };
        // Check if at least 'period' time elapsed
        let elp = self.tick_elaps(data.last_run);
        if elp >= data.period {
            return 0;
        }
        data.period - elp
    }

    /// `lv_timer_handler_resume()`: a timer is ready to run, resume the timer loop.
    fn timer_handler_resume(&mut self) {
        // If there is a timer which is ready to run then resume the timer loop
        self.timers.time_until_next = 0;
        if let Some(cb) = self.timers.resume_cb.clone() {
            cb(self);
        }
    }

    /// `lv_timer_handler_set_resume_cb()`
    pub fn timer_handler_set_resume_cb(&mut self, cb: Option<TimerHandlerResumeCallback>) {
        self.timers.resume_cb = cb;
    }
}

/// Upstream has no `test_timer.c`; these are invariants derived from
/// `lv_timer.c` itself.
#[cfg(test)]
mod tests {
    use alloc::vec;
    use core::cell::Cell;

    use super::*;
    use crate::core::UiConfig;

    fn ui() -> Ui {
        Ui::new(UiConfig::default())
    }

    /// A callback that counts its invocations.
    fn counter() -> (Rc<Cell<u32>>, TimerCallback) {
        let count = Rc::new(Cell::new(0));
        let c = count.clone();
        let cb: TimerCallback = Rc::new(move |_ui: &mut Ui, _t: TimerId| c.set(c.get() + 1));
        (count, cb)
    }

    #[test]
    fn core_init_state_matches_upstream() {
        // lv_timer_core_init() itself starts empty. Ui::new() subsequently
        // runs lv_anim_core_init(), which installs one paused timer.
        let state = TimerState::new();
        assert!(state.run);
        assert!(state.timers.is_empty());
        assert!(state.order.is_empty());
        let ui = ui();
        let animation = ui.anim_get_timer().unwrap();
        assert!(ui.timers.run);
        assert_eq!(ui.timers.order, vec![animation]);
        assert_eq!(ui.timer_get_next(None), Some(animation));
        assert_eq!(ui.timer_get_next(Some(animation)), None);
        assert!(ui.timer_get_paused(animation));
        assert_eq!(NO_TIMER_READY, 0xFFFF_FFFF);
    }

    #[test]
    fn create_sets_defaults_and_resumes_handler() {
        let mut ui = ui();
        ui.tick_inc(1000);
        ui.timers.time_until_next = 123;
        let (_, cb) = counter();
        let t = ui.timer_create(cb, 50, Some(Rc::new(7u8)));
        let d = ui.timer(t).unwrap();
        assert_eq!(d.period, 50);
        assert_eq!(d.last_run, 1000);
        assert_eq!(d.repeat_count, -1);
        assert!(!d.paused);
        assert!(d.auto_delete);
        assert!(d.cb.is_some());
        assert!(ui.timers.created);
        // lv_timer_handler_resume(): time_until_next = 0
        assert_eq!(ui.timer_get_time_until_next(), 0);
        let ud = ui.timer_get_user_data(t).unwrap();
        assert_eq!(ud.downcast_ref::<u8>(), Some(&7));
        assert!(!ui.timer_get_paused(t));

        let b = ui.timer_create_basic();
        let d = ui.timer(b).unwrap();
        assert_eq!(d.period, 500);
        assert!(d.cb.is_none());
        assert!(d.user_data.is_none());
        assert!(ui.timer_get_user_data(b).is_none());
    }

    #[test]
    fn ins_head_order_and_get_next() {
        let mut ui = ui();
        let animation = ui.anim_get_timer().unwrap();
        let a = ui.timer_create_basic();
        let b = ui.timer_create_basic();
        let c = ui.timer_create_basic();
        // lv_ll_ins_head(): the newest timer is the head.
        assert_eq!(ui.timers.order, vec![c, b, a, animation]);
        assert_eq!(ui.timer_get_next(None), Some(c));
        assert_eq!(ui.timer_get_next(Some(c)), Some(b));
        assert_eq!(ui.timer_get_next(Some(b)), Some(a));
        assert_eq!(ui.timer_get_next(Some(a)), Some(animation));
        assert_eq!(ui.timer_get_next(Some(animation)), None);
        ui.timer_delete(b);
        assert!(ui.timers.deleted);
        assert_eq!(ui.timer_get_next(Some(c)), Some(a));
        assert_eq!(ui.timer_get_next(Some(b)), None, "stale handle");
        assert_eq!(ui.timers.timers.len(), 3);
        // Deleting a stale handle is `timer == NULL`: a no-op.
        ui.timers.deleted = false;
        ui.timer_delete(b);
        assert!(!ui.timers.deleted);
    }

    #[test]
    fn handler_runs_due_timers_and_reports_time_to_next() {
        let mut ui = ui();
        ui.tick_inc(1); // tick 0 is the "lv_tick_inc not called" heuristic
        let (count, cb) = counter();
        let t = ui.timer_create(cb, 100, None);

        // Not due yet: nothing runs, 100 ms remain.
        assert_eq!(ui.timer_handler(), 100);
        assert_eq!(count.get(), 0);
        assert_eq!(ui.timer_get_time_until_next(), 100);

        ui.tick_inc(60);
        assert_eq!(ui.timer_handler(), 40);
        assert_eq!(count.get(), 0);

        ui.tick_inc(40);
        assert_eq!(ui.timer_handler(), 100);
        assert_eq!(count.get(), 1);
        assert_eq!(ui.timer(t).unwrap().last_run, 101);

        // Late by 30 ms: runs, and the period restarts from now (not from the due time).
        ui.tick_inc(130);
        assert_eq!(ui.timer_handler(), 100);
        assert_eq!(count.get(), 2);
        assert_eq!(ui.timer(t).unwrap().last_run, 231);

        // Only the paused animation timer remains: LV_NO_TIMER_READY.
        ui.timer_delete(t);
        assert_eq!(ui.timer_handler(), NO_TIMER_READY);
    }

    #[test]
    fn repeat_count_one_auto_deletes() {
        let mut ui = ui();
        let animation = ui.anim_get_timer().unwrap();
        ui.tick_inc(1);
        let (count, cb) = counter();
        let t = ui.timer_create(cb, 10, None);
        ui.timer_set_repeat_count(t, 1);
        ui.tick_inc(10);
        ui.timer_handler();
        assert_eq!(count.get(), 1);
        assert!(ui.timer(t).is_none(), "deleted after its single run");
        assert_eq!(ui.timers.order, vec![animation]);
        assert!(ui.timer_get_paused(animation));
        ui.tick_inc(10);
        assert_eq!(ui.timer_handler(), NO_TIMER_READY);
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn repeat_count_n_runs_n_times() {
        let mut ui = ui();
        ui.tick_inc(1);
        let (count, cb) = counter();
        let t = ui.timer_create(cb, 10, None);
        ui.timer_set_repeat_count(t, 3);
        for i in 1..=3 {
            ui.tick_inc(10);
            ui.timer_handler();
            assert_eq!(count.get(), i);
        }
        assert!(ui.timer(t).is_none());
    }

    #[test]
    fn repeat_count_zero_without_auto_delete_pauses() {
        let mut ui = ui();
        ui.tick_inc(1);
        let (count, cb) = counter();
        let t = ui.timer_create(cb, 10, None);
        ui.timer_set_repeat_count(t, 1);
        ui.timer_set_auto_delete(t, false);
        ui.tick_inc(10);
        assert_eq!(ui.timer_handler(), NO_TIMER_READY, "paused timers do not count");
        assert_eq!(count.get(), 1);
        let d = ui.timer(t).expect("kept");
        assert!(d.paused);
        assert_eq!(d.repeat_count, 0);
        assert!(ui.timer_get_paused(t));

        // Resuming with repeat_count 0: the callback is NOT called
        // (original_repeat_count == 0) and the timer is paused again.
        ui.timer_resume(t);
        ui.tick_inc(10);
        ui.timer_handler();
        assert_eq!(count.get(), 1);
        assert!(ui.timer_get_paused(t));

        // Give it one more go.
        ui.timer_set_repeat_count(t, 1);
        ui.timer_resume(t);
        ui.tick_inc(10);
        ui.timer_handler();
        assert_eq!(count.get(), 2);
        assert!(ui.timer_get_paused(t));
    }

    #[test]
    fn repeat_count_zero_set_externally_is_collected_without_running() {
        let mut ui = ui();
        ui.tick_inc(1);
        let (count, cb) = counter();
        let t = ui.timer_create(cb, 100, None);
        ui.timer_set_repeat_count(t, 0);
        // Not due, but repeat_count == 0 && auto_delete: deleted in lv_timer_exec.
        ui.timer_handler();
        assert_eq!(count.get(), 0);
        assert!(ui.timer(t).is_none());
    }

    #[test]
    fn pause_resume_ready_reset() {
        let mut ui = ui();
        ui.tick_inc(1);
        let (count, cb) = counter();
        let t = ui.timer_create(cb, 100, None);

        ui.timer_pause(t);
        assert!(ui.timer_get_paused(t));
        ui.tick_inc(100);
        assert_eq!(ui.timer_handler(), NO_TIMER_READY);
        assert_eq!(count.get(), 0);

        ui.timers.time_until_next = 55;
        ui.timer_resume(t);
        assert_eq!(ui.timers.time_until_next, 0, "resume() resumes the handler");
        assert!(!ui.timer_get_paused(t));
        ui.timer_handler();
        assert_eq!(count.get(), 1);

        // lv_timer_ready(): last_run = tick - period - 1 -> due immediately.
        ui.timer_ready(t);
        assert_eq!(
            ui.timer(t).unwrap().last_run,
            101u32.wrapping_sub(100).wrapping_sub(1)
        );
        assert_eq!(ui.timer_get_time_to_next(), 0);
        ui.timer_handler();
        assert_eq!(count.get(), 2);

        // lv_timer_reset(): the full period restarts from now.
        ui.tick_inc(90);
        ui.timer_reset(t);
        assert_eq!(ui.timer(t).unwrap().last_run, 191);
        assert_eq!(ui.timer_get_time_to_next(), 100);
        ui.tick_inc(10);
        ui.timer_handler();
        assert_eq!(count.get(), 2);

        // lv_timer_set_period() takes effect at once.
        ui.timer_set_period(t, 5);
        assert_eq!(ui.timer_get_time_to_next(), 0);
        ui.timer_handler();
        assert_eq!(count.get(), 3);
    }

    #[test]
    fn ready_wraps_around_near_tick_zero() {
        let mut ui = ui();
        let t = ui.timer_create_basic();
        ui.timer_ready(t);
        // tick 0 - 500 - 1 in uint32_t arithmetic
        assert_eq!(ui.timer(t).unwrap().last_run, u32::MAX - 500);
        assert_eq!(ui.timer_get_time_to_next(), 0);
    }

    #[test]
    fn set_cb_and_user_data() {
        let mut ui = ui();
        ui.tick_inc(1);
        let t = ui.timer_create_basic();
        ui.tick_inc(500);
        ui.timer_handler(); // NULL callback: nothing to call, still "executed"
        assert_eq!(ui.timer(t).unwrap().last_run, 501);

        let (count, cb) = counter();
        ui.timer_set_cb(t, Some(cb));
        ui.timer_set_user_data(t, Some(Rc::new(41i32)));
        ui.tick_inc(500);
        ui.timer_handler();
        assert_eq!(count.get(), 1);
        assert_eq!(
            ui.timer_get_user_data(t).unwrap().downcast_ref::<i32>(),
            Some(&41)
        );
        ui.timer_set_cb(t, None);
        ui.timer_set_user_data(t, None);
        ui.tick_inc(500);
        ui.timer_handler();
        assert_eq!(count.get(), 1);
        assert!(ui.timer_get_user_data(t).is_none());
    }

    #[test]
    fn enable_false_makes_handler_return_one() {
        let mut ui = ui();
        ui.tick_inc(1);
        let (count, cb) = counter();
        let _t = ui.timer_create(cb, 10, None);
        ui.timer_enable(false);
        ui.tick_inc(10);
        assert_eq!(ui.timer_handler(), 1);
        assert_eq!(count.get(), 0);
        assert!(!ui.timers.already_running);
        ui.timers.time_until_next = 99;
        ui.timer_enable(true);
        assert_eq!(ui.timers.time_until_next, 0);
        ui.timer_handler();
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn nested_handler_call_is_rejected() {
        let mut ui = ui();
        ui.tick_inc(1);
        let seen = Rc::new(Cell::new(u32::MAX));
        let s = seen.clone();
        let cb: TimerCallback = Rc::new(move |ui: &mut Ui, _t| s.set(ui.timer_handler()));
        ui.timer_create(cb, 10, None);
        ui.tick_inc(10);
        ui.timer_handler();
        assert_eq!(seen.get(), 1, "already_running => 1");
        assert!(!ui.timers.already_running);
    }

    #[test]
    fn callback_deleting_itself_and_others() {
        let mut ui = ui();
        let animation = ui.anim_get_timer().unwrap();
        ui.tick_inc(1);
        let (count_a, cb_a) = counter();
        let a = ui.timer_create(cb_a, 10, None);
        let (count_c, cb_c) = counter();
        // b deletes itself and a; c is created last and runs first.
        let b_count = Rc::new(Cell::new(0));
        let bc = b_count.clone();
        let cb_b: TimerCallback = Rc::new(move |ui: &mut Ui, me: TimerId| {
            bc.set(bc.get() + 1);
            ui.timer_delete(a);
            ui.timer_delete(me);
        });
        let b = ui.timer_create(cb_b, 10, None);
        let c = ui.timer_create(cb_c, 10, None);
        assert_eq!(ui.timers.order, vec![c, b, a, animation]);

        ui.tick_inc(10);
        let next = ui.timer_handler();
        assert_eq!(count_c.get(), 1);
        assert_eq!(b_count.get(), 1);
        assert_eq!(count_a.get(), 0, "a was deleted before its turn");
        assert!(ui.timer(a).is_none());
        assert!(ui.timer(b).is_none());
        assert_eq!(ui.timers.order, vec![c, animation]);
        assert_eq!(next, 10);

        // Restarting from the head after the deletion must not run c twice.
        ui.tick_inc(10);
        ui.timer_handler();
        assert_eq!(count_c.get(), 2);
    }

    #[test]
    fn callback_creating_a_timer_restarts_from_head_once() {
        let mut ui = ui();
        let animation = ui.anim_get_timer().unwrap();
        ui.tick_inc(1);
        let (count_a, cb_a) = counter();
        let a = ui.timer_create(cb_a, 10, None);
        let (count_new, cb_new) = counter();
        let created = Rc::new(Cell::new(None));
        let cr = created.clone();
        let cb_b: TimerCallback = Rc::new(move |ui: &mut Ui, _me| {
            if cr.get().is_none() {
                // period 0: due immediately, so the restart from the head picks it up
                let n = ui.timer_create(cb_new.clone(), 0, None);
                ui.timer_set_repeat_count(n, 1);
                cr.set(Some(n));
            }
        });
        let b = ui.timer_create(cb_b, 10, None);

        ui.tick_inc(10);
        ui.timer_handler();
        let n = created.get().unwrap();
        assert_eq!(count_new.get(), 1, "new head timer ran in the same handler pass");
        assert!(ui.timer(n).is_none(), "repeat_count 1 => auto deleted");
        assert_eq!(count_a.get(), 1, "a ran exactly once despite the restart");
        assert_eq!(ui.timers.order, vec![b, a, animation]);
    }

    #[test]
    fn time_to_next_skips_paused_and_stops_at_zero() {
        let mut ui = ui();
        ui.tick_inc(1);
        let a = ui.timer_create_basic(); // 500
        let b = ui.timer_create_basic();
        ui.timer_set_period(b, 50);
        let c = ui.timer_create_basic();
        ui.timer_set_period(c, 20);
        ui.timer_pause(c);
        assert_eq!(ui.timer_get_time_to_next(), 50);
        ui.timer_resume(c);
        assert_eq!(ui.timer_get_time_to_next(), 20);
        ui.tick_inc(20);
        assert_eq!(ui.timer_get_time_to_next(), 0);
        ui.timer_delete(a);
        ui.timer_delete(b);
        ui.timer_delete(c);
        assert_eq!(ui.timer_get_time_to_next(), NO_TIMER_READY);
    }

    #[test]
    fn idle_accounting() {
        let mut ui = ui();
        // sys_time 0: the "tick_inc not called" counter increments.
        ui.timer_handler();
        assert_eq!(ui.timers.run_cnt, 1);
        assert_eq!(ui.timer_get_idle(), 0, "not measured yet");

        // Below IDLE_MEAS_PERIOD nothing is evaluated.
        ui.tick_inc(499);
        ui.timer_handler();
        assert_eq!(ui.timer_get_idle(), 0);
        assert_eq!(ui.timers.idle_period_start, 0);

        // Handler never consumes time with a static tick: 0 % busy => 100 % idle.
        ui.tick_inc(1);
        ui.timer_handler();
        assert_eq!(ui.timer_get_idle(), 100);
        assert_eq!(ui.timers.busy_time, 0);
        assert_eq!(ui.timers.idle_period_start, 500);

        // Pretend 250 ms were busy out of the next 500 ms window.
        ui.timers.busy_time = 250;
        ui.tick_inc(500);
        ui.timer_handler();
        assert_eq!(ui.timer_get_idle(), 50);

        // > 100 % busy (clock went backwards / overlong handler) => idle 0.
        ui.timers.busy_time = 1000;
        ui.tick_inc(500);
        ui.timer_handler();
        assert_eq!(ui.timer_get_idle(), 0);

        // uint8_t truncating store: 256 % busy reads as 0 % => idle 100.
        ui.timers.busy_time = 1280;
        ui.tick_inc(500);
        ui.timer_handler();
        assert_eq!(ui.timer_get_idle(), 100);
    }

    #[test]
    fn run_cnt_wraps_after_100_tickless_calls() {
        let mut ui = ui();
        for _ in 0..100 {
            ui.timer_handler();
        }
        assert_eq!(ui.timers.run_cnt, 100);
        ui.timer_handler();
        assert_eq!(ui.timers.run_cnt, 0);
    }

    #[test]
    fn periodic_handler_and_run_in_period() {
        let mut ui = ui();
        ui.tick_inc(1);
        let (count, cb) = counter();
        let t = ui.timer_create(cb, 30, None);
        // time_until_next is 0 after create => runs at once
        ui.timer_periodic_handler();
        assert_eq!(ui.timers.periodic_last_tick, 1);
        assert_eq!(ui.timer_get_time_until_next(), 30);
        ui.tick_inc(10);
        ui.timer_periodic_handler(); // 10 < 30: skipped
        assert_eq!(ui.timers.periodic_last_tick, 1);
        ui.tick_inc(20);
        ui.timer_periodic_handler();
        assert_eq!(count.get(), 1);
        assert_eq!(ui.timers.periodic_last_tick, 31);

        // lv_timer_handler_run_in_period()
        let mut ui = Ui::new(UiConfig::default());
        ui.tick_inc(1);
        let (count, cb) = counter();
        let _ = ui.timer_create(cb, 5, None);
        // `last_tick` starts at 0: only 1 ms elapsed < 10 => not yet
        assert_eq!(ui.timer_handler_run_in_period(10), 1);
        assert_eq!(count.get(), 0);
        ui.tick_inc(9);
        assert_eq!(
            ui.timer_handler_run_in_period(10),
            5,
            "period elapsed: runs the handler"
        );
        assert_eq!(count.get(), 1);
        assert_eq!(ui.timers.run_in_period_last_tick, 10);
        assert_eq!(
            ui.timers.periodic_last_tick, 0,
            "run_in_period has its own last_tick"
        );
        ui.tick_inc(5);
        assert_eq!(ui.timer_handler_run_in_period(10), 1, "within the period: 1");
        assert_eq!(count.get(), 1);
        ui.tick_inc(5);
        assert_eq!(ui.timer_handler_run_in_period(10), 5);
        assert_eq!(count.get(), 2);
        let _ = t;
    }

    #[test]
    fn resume_cb_is_called_from_every_resume_point() {
        let mut ui = ui();
        let n = Rc::new(Cell::new(0));
        let c = n.clone();
        ui.timer_handler_set_resume_cb(Some(Rc::new(move |_ui: &mut Ui| c.set(c.get() + 1))));
        let t = ui.timer_create_basic(); // 1
        ui.timer_resume(t); // 2
        ui.timer_set_period(t, 1); // 3
        ui.timer_ready(t); // 4
        ui.timer_reset(t); // 5
        ui.timer_enable(false); // no resume
        ui.timer_enable(true); // 6
        ui.timer_pause(t); // no resume
        ui.timer_set_repeat_count(t, 2); // no resume
        assert_eq!(n.get(), 6);
        ui.timer_handler_set_resume_cb(None);
        ui.timer_reset(t);
        assert_eq!(n.get(), 6);
    }

    #[test]
    fn core_deinit_disables_and_clears() {
        let mut ui = ui();
        ui.timer_create_basic();
        ui.timer_create_basic();
        ui.timer_core_deinit();
        assert!(!ui.timers.run);
        assert!(ui.timers.timers.is_empty());
        assert!(ui.timers.order.is_empty());
        assert_eq!(ui.timer_handler(), 1);
    }

    #[cfg(not(feature = "check-arg-panic"))]
    #[test]
    fn stale_handles_are_argument_check_failures() {
        let mut ui = ui();
        let t = ui.timer_create_basic();
        ui.timer_delete(t);
        ui.timer_pause(t);
        ui.timer_resume(t);
        ui.timer_set_period(t, 1);
        ui.timer_ready(t);
        ui.timer_reset(t);
        ui.timer_set_repeat_count(t, 1);
        ui.timer_set_auto_delete(t, false);
        ui.timer_set_cb(t, None);
        ui.timer_set_user_data(t, None);
        assert!(ui.timer_get_user_data(t).is_none());
        assert!(!ui.timer_get_paused(t));
        // Only lv_anim_core_init()'s timer remains.
        let animation = ui.anim_get_timer().unwrap();
        assert_eq!(ui.timers.timers.len(), 1);
        assert_eq!(ui.timers.order, vec![animation]);
    }
    #[test]
    fn run_in_period_last_tick_is_independent_of_periodic_handler() {
        // tick 100: lv_timer_periodic_handler() runs (time_until_next == 0)
        let mut ui = ui();
        ui.tick_inc(100);
        ui.timer_periodic_handler();
        assert_eq!(ui.timers.periodic_last_tick, 100);
        // tick 105: run_in_period's own last_tick is still 0 => 105 >= 10 runs
        ui.tick_inc(5);
        let (count, cb) = counter();
        let t = ui.timer_create(cb, 0, None);
        assert_ne!(ui.timer_handler_run_in_period(10), 1);
        assert_eq!(count.get(), 1);
        assert_eq!(ui.timers.run_in_period_last_tick, 105);
        assert_eq!(ui.timers.periodic_last_tick, 100);
        ui.timer_delete(t);
    }

    #[test]
    fn delete_calls_ext_data_free_cb_once_but_deinit_does_not() {
        use core::sync::atomic::{AtomicU32, Ordering};
        static FREED: AtomicU32 = AtomicU32::new(0);
        static FREED_WITH_DATA: AtomicU32 = AtomicU32::new(0);
        fn free_fn(data: Option<Rc<dyn Any>>) {
            FREED.fetch_add(1, Ordering::Relaxed);
            if let Some(v) = data.and_then(|d| d.downcast::<u32>().ok()) {
                FREED_WITH_DATA.store(*v, Ordering::Relaxed);
            }
        }

        let mut ui = ui();
        ui.tick_inc(1);
        let (count, cb) = counter();
        let t = ui.timer_create(cb, 0, None);
        ui.timer_set_repeat_count(t, 1);
        ui.timer_set_external_data(t, Some(Rc::new(42u32)), Some(free_fn));
        assert!(ui.timer(t).unwrap().ext_data.free_cb.is_some());
        // Auto-delete inside lv_timer_exec calls free_cb(data) exactly once
        ui.timer_handler();
        assert_eq!(count.get(), 1);
        assert!(ui.timer(t).is_none());
        assert_eq!(FREED.load(Ordering::Relaxed), 1);
        assert_eq!(FREED_WITH_DATA.load(Ordering::Relaxed), 42);
        // Deleting the stale handle again is `timer == NULL`: no second call
        ui.timer_delete(t);
        assert_eq!(FREED.load(Ordering::Relaxed), 1);

        // lv_timer_core_deinit() (lv_ll_clear) does not call free_cb
        let t2 = ui.timer_create_basic();
        ui.timer_set_external_data(t2, None, Some(free_fn));
        ui.timer_core_deinit();
        assert_eq!(FREED.load(Ordering::Relaxed), 1);
    }

    #[cfg(not(feature = "check-arg-panic"))]
    #[test]
    fn ready_and_reset_check_arg_before_reading_tick() {
        use core::sync::atomic::{AtomicU32, Ordering};
        static TICK: AtomicU32 = AtomicU32::new(0);
        fn counting_tick() -> u32 {
            TICK.fetch_add(1, Ordering::Relaxed)
        }

        let mut ui = ui();
        let t = ui.timer_create_basic();
        ui.timer_delete(t);
        ui.set_tick_cb(Some(counting_tick));
        let before = TICK.load(Ordering::Relaxed);
        ui.timer_ready(t);
        ui.timer_reset(t);
        ui.timer_set_external_data(t, None, None);
        assert_eq!(
            TICK.load(Ordering::Relaxed),
            before,
            "stale handle must not read the tick"
        );
        ui.set_tick_cb(None);
    }
}
