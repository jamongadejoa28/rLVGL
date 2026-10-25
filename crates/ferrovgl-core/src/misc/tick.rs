//! Ported from lvgl v9.6.0: `include/lvgl/tick/lv_tick.h`, `src/tick/lv_tick_private.h`,
//! `src/tick/lv_tick.c`.
//!
//! Provide access to the system tick with 1 millisecond resolution.

use core::cell::Cell;

use crate::core::Ui;

/// `lv_tick_get_cb_t`
pub type TickGetCallback = fn() -> u32;
/// `lv_delay_cb_t`
pub type DelayCallback = fn(u32);

/// `lv_tick_state_t`
#[derive(Clone, Debug, Default)]
pub(crate) struct TickState {
    /// `sys_time`
    pub sys_time: u32,
    /// `sys_irq_flag`: cleared by `lv_tick_inc()`, set by every
    /// `lv_tick_get()` that has no `tick_get_cb` (a `Cell` because the read
    /// path takes `&self`).
    pub irq_flag: Cell<bool>,
    /// `tick_get_cb`
    pub tick_get_cb: Option<TickGetCallback>,
    /// `delay_cb`
    pub delay_cb: Option<DelayCallback>,
}

impl Ui {
    /// `lv_tick_inc()`: advance the system tick by `tick_period` ms.
    ///
    /// Clears `sys_irq_flag` first, exactly like upstream, so a concurrent
    /// reader could detect the interruption (see [`Ui::tick_get`]).
    pub fn tick_inc(&mut self, tick_period: u32) {
        let state_p = &mut self.tick;

        state_p.irq_flag.set(false);
        state_p.sys_time = state_p.sys_time.wrapping_add(tick_period);
    }

    /// `lv_tick_get()`
    ///
    /// Upstream re-reads `sys_time` in a `do { irq_flag = 1; result = sys_time; }
    /// while(!irq_flag)` loop to survive `lv_tick_inc()` being called from an
    /// interrupt in the middle of the read. Through `&self` nothing can mutate
    /// `sys_time` concurrently, so the loop always terminates after its first
    /// iteration: it sets `sys_irq_flag` and reads `sys_time` once.
    #[must_use]
    pub fn tick_get(&self) -> u32 {
        let state_p = &self.tick;

        if let Some(cb) = state_p.tick_get_cb {
            return cb();
        }

        // do { sys_irq_flag = 1; result = sys_time; } while(!sys_irq_flag);
        let mut result;
        loop {
            state_p.irq_flag.set(true);
            result = state_p.sys_time;
            if state_p.irq_flag.get() {
                break;
            }
        }

        result
    }

    /// `lv_tick_elaps()`: ms elapsed since `prev_tick`.
    #[must_use]
    pub fn tick_elaps(&self, prev_tick: u32) -> u32 {
        Self::tick_diff(self.tick_get(), prev_tick)
    }

    /// `lv_tick_diff()`: `tick - prev_tick` with wrap-around.
    #[must_use]
    pub fn tick_diff(tick: u32, prev_tick: u32) -> u32 {
        // Unsigned overflow is well-defined and works for a single wrap around.
        tick.wrapping_sub(prev_tick)
    }

    /// `lv_delay_ms()`
    ///
    /// Without a delay callback this busy-waits on [`Ui::tick_elaps`], which
    /// only makes progress when a tick callback (`tick_set_cb`) drives the
    /// time, exactly like upstream without an interrupt calling `lv_tick_inc()`.
    pub fn delay_ms(&self, ms: u32) {
        if let Some(cb) = self.tick.delay_cb {
            cb(ms);
        } else {
            let t = self.tick_get();
            while self.tick_elaps(t) < ms {
                // Do something to not call `tick_elaps` too often as it might interfere
                // with interrupts (upstream: 100 iterations of a `volatile` multiply).
                let mut x = core::hint::black_box(ms);
                for _ in 0..100 {
                    x = core::hint::black_box(x.wrapping_mul(3));
                }
            }
        }
    }

    /// `lv_tick_set_cb()`
    pub fn tick_set_cb(&mut self, cb: Option<TickGetCallback>) {
        self.tick.tick_get_cb = cb;
    }

    /// `lv_tick_get_cb()`
    #[must_use]
    pub fn tick_get_cb(&self) -> Option<TickGetCallback> {
        self.tick.tick_get_cb
    }

    /// `lv_delay_set_cb()`
    pub fn delay_set_cb(&mut self, cb: Option<DelayCallback>) {
        self.tick.delay_cb = cb;
    }
}

/// Port of upstream `tests/src/test_cases/test_tick.c`.
///
/// `TickGetCallback` is a plain `fn() -> u32`, so the test tick sources are
/// per-test atomics (never shared between tests, which run in parallel).
#[cfg(test)]
mod tests {
    use core::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::core::UiConfig;

    fn ui() -> Ui {
        // setUp(): tick module reset (fresh context, no callbacks).
        let mut ui = Ui::new(UiConfig::default());
        ui.tick_set_cb(None);
        ui.delay_set_cb(None);
        ui
    }

    #[test]
    fn tick_inc_and_get() {
        let mut ui = ui();
        // Initial tick should be 0
        assert_eq!(ui.tick_get(), 0);

        // Increment tick by 10
        ui.tick_inc(10);
        assert_eq!(ui.tick_get(), 10);

        // Increment tick by 100
        ui.tick_inc(100);
        assert_eq!(ui.tick_get(), 110);
    }

    #[test]
    fn tick_get_with_callback() {
        static TICK: AtomicU32 = AtomicU32::new(0);
        fn test_tick_cb() -> u32 {
            TICK.load(Ordering::Relaxed)
        }

        let mut ui = ui();
        ui.tick_set_cb(Some(test_tick_cb));
        // TEST_ASSERT_EQUAL(test_tick_cb, lv_tick_get_cb()): checked by behaviour,
        // function pointers are not reliably comparable.
        let registered = ui.tick_get_cb().expect("callback registered");

        TICK.store(1234, Ordering::Relaxed);
        assert_eq!(registered(), 1234);
        assert_eq!(ui.tick_get(), 1234);

        TICK.store(5678, Ordering::Relaxed);
        let tmp_tick = ui.tick_get();
        assert_eq!(tmp_tick, 5678);
    }

    #[test]
    fn tick_elaps_normal() {
        let mut ui = ui();
        let start = ui.tick_get();
        ui.tick_inc(50);
        assert_eq!(ui.tick_elaps(start), 50);

        ui.tick_inc(100);
        assert_eq!(ui.tick_elaps(start), 150);
    }

    #[test]
    fn tick_elaps_overflow() {
        static TICK: AtomicU32 = AtomicU32::new(0);
        fn test_tick_cb() -> u32 {
            TICK.load(Ordering::Relaxed)
        }

        let mut ui = ui();
        // Simulate near overflow
        TICK.store(u32::MAX - 10, Ordering::Relaxed);
        ui.tick_set_cb(Some(test_tick_cb));
        let start = ui.tick_get();

        // Simulate overflow
        TICK.store(20, Ordering::Relaxed);
        assert_eq!(ui.tick_elaps(start), 31);
    }

    #[test]
    fn delay_ms_with_callback() {
        static DELAY_CALLED: AtomicU32 = AtomicU32::new(0);
        fn test_delay_cb(ms: u32) {
            DELAY_CALLED.store(ms, Ordering::Relaxed);
        }

        let mut ui = ui();
        ui.delay_set_cb(Some(test_delay_cb));
        ui.delay_ms(100);
        assert_eq!(DELAY_CALLED.load(Ordering::Relaxed), 100);
    }

    #[test]
    fn delay_ms_without_callback() {
        static TICK: AtomicU32 = AtomicU32::new(0);
        fn test_tick_inc_cb() -> u32 {
            TICK.fetch_add(1, Ordering::Relaxed)
        }

        let mut ui = ui();
        ui.tick_set_cb(Some(test_tick_inc_cb));
        let start = ui.tick_get();
        ui.delay_ms(10);
        assert!(ui.tick_elaps(start) >= 10);
    }

    #[test]
    fn sys_time_and_diff_wrap_around() {
        let mut ui = ui();
        ui.tick_inc(u32::MAX);
        assert_eq!(ui.tick_get(), u32::MAX);
        ui.tick_inc(2);
        assert_eq!(ui.tick_get(), 1);
        assert_eq!(Ui::tick_diff(1, u32::MAX), 2);
        assert_eq!(Ui::tick_diff(5, 5), 0);
    }

    #[test]
    fn tick_inc_clears_irq_flag() {
        let mut ui = ui();
        ui.tick.irq_flag.set(true);
        ui.tick_inc(1);
        assert!(!ui.tick.irq_flag.get());
    }

    #[test]
    fn tick_get_sets_irq_flag_without_callback() {
        static TICK: AtomicU32 = AtomicU32::new(7);
        fn test_tick_cb() -> u32 {
            TICK.load(Ordering::Relaxed)
        }

        let mut ui = ui();
        ui.tick_inc(1);
        assert!(!ui.tick.irq_flag.get());
        assert_eq!(ui.tick_get(), 1);
        assert!(ui.tick.irq_flag.get(), "lv_tick_get sets sys_irq_flag = 1");

        // With tick_get_cb the flag is not touched
        ui.tick_inc(1);
        ui.tick_set_cb(Some(test_tick_cb));
        assert_eq!(ui.tick_get(), 7);
        assert!(!ui.tick.irq_flag.get());
    }
}
