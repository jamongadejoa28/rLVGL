//! Ported from lvgl v9.6.0: `include/lvgl/tick/lv_tick.h`, `src/tick/lv_tick_private.h`,
//! `src/tick/lv_tick.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::core::Ui;

/// `lv_tick_get_cb_t`
pub type TickGetCallback = fn() -> u32;
/// `lv_delay_cb_t`
pub type DelayCallback = fn(u32);

/// `lv_tick_state_t`
#[derive(Clone, Debug, Default)]
pub struct TickState {
    /// `sys_time`
    pub sys_time: u32,
    /// `sys_irq_flag`
    pub irq_flag: bool,
    /// `tick_get_cb`
    pub tick_get_cb: Option<TickGetCallback>,
    /// `delay_cb`
    pub delay_cb: Option<DelayCallback>,
}

impl Ui {
    /// `lv_tick_inc()`: advance the system tick by `tick_period` ms.
    pub fn tick_inc(&mut self, tick_period: u32) {
        todo!()
    }

    /// `lv_tick_get()`
    pub fn tick_get(&self) -> u32 {
        todo!()
    }

    /// `lv_tick_elaps()`: ms elapsed since `prev_tick`.
    pub fn tick_elaps(&self, prev_tick: u32) -> u32 {
        todo!()
    }

    /// `lv_tick_diff()`: `tick - prev_tick` with wrap-around.
    pub fn tick_diff(tick: u32, prev_tick: u32) -> u32 {
        todo!()
    }

    /// `lv_delay_ms()`
    pub fn delay_ms(&self, ms: u32) {
        todo!()
    }

    /// `lv_tick_set_cb()`
    pub fn set_tick_cb(&mut self, cb: Option<TickGetCallback>) {
        todo!()
    }

    /// `lv_tick_get_cb()`
    pub fn tick_cb(&self) -> Option<TickGetCallback> {
        todo!()
    }

    /// `lv_delay_set_cb()`
    pub fn set_delay_cb(&mut self, cb: Option<DelayCallback>) {
        todo!()
    }
}
