//! Ported from lvgl v9.6.0: `include/lvgl/indev/lv_indev.h`, `src/indev/lv_indev_private.h`,
//! `src/indev/lv_indev_scroll.h`, `src/indev/lv_indev.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::display::DisplayId;
use crate::group::{GroupId, Key as LvKey};
use crate::misc::anim::AnimId;
use crate::misc::area::{Area, Dir, Point};
use crate::misc::arena::Key;
use crate::misc::event::{
    EventCallback, EventCode, EventDsc, EventDscId, EventFilter, EventList, EventParam, LvResult,
};
use crate::misc::timer::TimerId;

/// `LV_INDEV_VECT_HIST_SIZE`
pub const INDEV_VECT_HIST_SIZE: usize = 8;

/// Handle of an input device (`lv_indev_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct IndevId(pub Key);

/// `lv_indev_type_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IndevType {
    /// `LV_INDEV_TYPE_NONE`
    #[default]
    None = 0,
    /// `LV_INDEV_TYPE_POINTER`: touch pad, mouse, external button.
    Pointer = 1,
    /// `LV_INDEV_TYPE_KEYPAD`: keypad or keyboard.
    Keypad = 2,
    /// `LV_INDEV_TYPE_BUTTON`: external (hardware) button assigned to a point.
    Button = 3,
    /// `LV_INDEV_TYPE_ENCODER`: encoder with left/right turn and push.
    Encoder = 4,
}

/// `lv_indev_state_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IndevState {
    /// `LV_INDEV_STATE_RELEASED`
    #[default]
    Released = 0,
    /// `LV_INDEV_STATE_PRESSED`
    Pressed = 1,
}

/// `lv_indev_mode_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IndevMode {
    /// `LV_INDEV_MODE_NONE`
    #[default]
    None = 0,
    /// `LV_INDEV_MODE_TIMER`: read periodically from a timer.
    Timer = 1,
    /// `LV_INDEV_MODE_EVENT`: read when the application calls `indev_read()`.
    Event = 2,
}

/// `lv_indev_gesture_type_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IndevGestureType {
    /// `LV_INDEV_GESTURE_NONE`
    #[default]
    None = 0,
    /// `LV_INDEV_GESTURE_PINCH`
    Pinch = 1,
    /// `LV_INDEV_GESTURE_SWIPE`
    Swipe = 2,
    /// `LV_INDEV_GESTURE_ROTATE`
    Rotate = 3,
    /// `LV_INDEV_GESTURE_TWO_FINGERS_SWIPE`
    TwoFingersSwipe = 4,
    /// `LV_INDEV_GESTURE_SCROLL`
    Scroll = 5,
    /// `LV_INDEV_GESTURE_CNT`
    Cnt = 6,
}

/// `lv_indev_gesture_state_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IndevGestureState {
    /// `LV_INDEV_GESTURE_STATE_NONE`
    #[default]
    None = 0,
    /// `LV_INDEV_GESTURE_STATE_ONGOING`
    Ongoing = 1,
    /// `LV_INDEV_GESTURE_STATE_RECOGNIZED`
    Recognized = 2,
    /// `LV_INDEV_GESTURE_STATE_ENDED`
    Ended = 3,
    /// `LV_INDEV_GESTURE_STATE_CANCELED`
    Canceled = 4,
}

/// The gesture direction reported by `lv_indev_get_gesture_dir()` (a `lv_dir_t`).
pub type IndevGestureDir = Dir;

/// `lv_indev_data_t`: what the driver reports in `read_cb`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndevReadData {
    /// `state`
    pub state: IndevState,
    /// `point`: for pointer devices.
    pub point: Point,
    /// `key`: for keypads.
    pub key: u32,
    /// `btn_id`: for buttons.
    pub btn_id: u32,
    /// `enc_diff`: for encoders.
    pub enc_diff: i16,
    /// `timestamp`
    pub timestamp: u32,
    /// `continue_reading`: more data is buffered, read again.
    pub continue_reading: bool,
}

/// `lv_indev_read_cb_t`
pub trait IndevDriver {
    /// `read_cb(indev, data)`: fill `data` with the current input state.
    /// `indev` is the device being read (for `lv_indev_get_display`,
    /// `lv_indev_get_driver_data`, ... through the `Ui` the caller holds).
    fn read(&mut self, indev: IndevId, data: &mut IndevReadData);
}

/// `lv_indev_key_remap_cb_t`: `(ui, indev, key) -> key`.
pub type IndevKeyRemapCallback = Rc<dyn Fn(&mut Ui, IndevId, u32) -> u32>;

/// `pointer` sub-struct of `lv_indev_t`.
#[derive(Clone, Debug, Default)]
pub struct IndevPointer {
    /// `act_point`
    pub act_point: Point,
    /// `last_point`
    pub last_point: Point,
    /// `last_raw_point`
    pub last_raw_point: Point,
    /// `vect`
    pub vect: Point,
    /// `vect_hist`
    pub vect_hist: [Point; INDEV_VECT_HIST_SIZE],
    /// `vect_hist_timestamp`
    pub vect_hist_timestamp: [u32; INDEV_VECT_HIST_SIZE],
    /// `vect_hist_index`
    pub vect_hist_index: u8,
    /// `scroll_sum`
    pub scroll_sum: Point,
    /// `scroll_throw_vect`
    pub scroll_throw_vect: Point,
    /// `scroll_throw_vect_ori`
    pub scroll_throw_vect_ori: Point,
    /// `act_obj`
    pub act_obj: Option<Obj>,
    /// `scroll_obj`
    pub scroll_obj: Option<Obj>,
    /// `last_pressed`
    pub last_pressed: Option<Obj>,
    /// `last_hovered`
    pub last_hovered: Option<Obj>,
    /// `scroll_area`
    pub scroll_area: Area,
    /// `gesture_sum`
    pub gesture_sum: Point,
    /// `diff`
    pub diff: i32,
    /// `short_click_streak`
    pub short_click_streak: u8,
    /// `last_short_click_point`
    pub last_short_click_point: Point,
    /// `last_short_click_timestamp`
    pub last_short_click_timestamp: u32,
    /// `scroll_dir`
    pub scroll_dir: Dir,
    /// `gesture_dir`
    pub gesture_dir: Dir,
    /// `gesture_sent`
    pub gesture_sent: bool,
    /// `press_moved`
    pub press_moved: bool,
    /// `pressed`
    pub pressed: bool,
    /// `ccw_rotation`
    pub ccw_rotation: bool,
}

/// `keypad` sub-struct of `lv_indev_t`.
#[derive(Clone, Debug, Default)]
pub struct IndevKeypad {
    /// `last_state`
    pub last_state: IndevState,
    /// `last_key`
    pub last_key: u32,
}

/// `lv_indev_t`
pub struct IndevData {
    /// `type`
    pub ty: IndevType,
    /// `read_cb`
    pub driver: Option<Box<dyn IndevDriver>>,
    /// `state`
    pub state: IndevState,
    /// `prev_state`
    pub prev_state: IndevState,
    /// `mode`
    pub mode: IndevMode,
    /// `long_pr_sent`
    pub long_pr_sent: bool,
    /// `reset_query`
    pub reset_query: bool,
    /// `enabled`
    pub enabled: bool,
    /// `wait_until_release`
    pub wait_until_release: bool,
    /// `stop_processing_query`
    pub stop_processing_query: bool,
    /// `timestamp`
    pub timestamp: u32,
    /// `pr_timestamp`
    pub pr_timestamp: u32,
    /// `longpr_rep_timestamp`
    pub longpr_rep_timestamp: u32,
    /// `driver_data`
    pub driver_data: Option<Box<dyn Any>>,
    /// `user_data`
    pub user_data: Option<Rc<dyn Any>>,
    /// `disp`
    pub disp: Option<DisplayId>,
    /// `read_timer`
    pub read_timer: Option<TimerId>,
    /// `scroll_limit`
    pub scroll_limit: u8,
    /// `scroll_throw`
    pub scroll_throw: u8,
    /// `gesture_min_velocity`
    pub gesture_min_velocity: u8,
    /// `gesture_min_distance`
    pub gesture_min_distance: u8,
    /// `long_press_time`
    pub long_press_time: u16,
    /// `double_click_time`
    pub double_click_time: u16,
    /// `long_press_repeat_time`
    pub long_press_repeat_time: u16,
    /// `rotary_sensitivity`
    pub rotary_sensitivity: i32,
    /// `pointer`
    pub pointer: IndevPointer,
    /// `keypad`
    pub keypad: IndevKeypad,
    /// `cursor`
    pub cursor: Option<Obj>,
    /// `group`
    pub group: Option<GroupId>,
    /// `btn_points`
    pub btn_points: Vec<Point>,
    /// `event_list`
    pub event_list: EventList,
    /// `scroll_throw_anim`
    pub scroll_throw_anim: Option<AnimId>,
    /// `key_remap_cb`
    pub key_remap_cb: Option<IndevKeyRemapCallback>,
}

impl Ui {
    /// `lv_indev_create()`
    pub fn indev_create(&mut self) -> IndevId {
        todo!()
    }

    /// `lv_indev_delete()`
    pub fn indev_delete(&mut self, indev: IndevId) {
        todo!()
    }

    /// `lv_indev_get_next()`
    pub fn indev_get_next(&self, indev: Option<IndevId>) -> Option<IndevId> {
        todo!()
    }

    /// `lv_indev_read()`: read and process the device once.
    pub fn indev_read(&mut self, indev: IndevId) {
        todo!()
    }

    /// `lv_indev_read_timer_cb()`
    pub fn indev_read_timer_cb(&mut self, timer: TimerId) {
        todo!()
    }

    /// `lv_indev_enable()`
    pub fn indev_enable(&mut self, indev: Option<IndevId>, enable: bool) {
        todo!()
    }

    /// `lv_indev_active()`
    pub fn indev_active(&self) -> Option<IndevId> {
        self.indev_active
    }

    /// `lv_indev_set_type()`
    pub fn indev_set_type(&mut self, indev: IndevId, ty: IndevType) {
        todo!()
    }

    /// `lv_indev_set_read_cb()`
    pub fn indev_set_driver(&mut self, indev: IndevId, driver: Option<Box<dyn IndevDriver>>) {
        todo!()
    }

    /// `lv_indev_set_user_data()`
    pub fn indev_set_user_data(&mut self, indev: IndevId, user_data: Option<Rc<dyn Any>>) {
        todo!()
    }

    /// `lv_indev_set_driver_data()`
    pub fn indev_set_driver_data(&mut self, indev: IndevId, driver_data: Option<Box<dyn Any>>) {
        todo!()
    }

    /// `lv_indev_set_display()`
    pub fn indev_set_display(&mut self, indev: IndevId, disp: Option<DisplayId>) {
        todo!()
    }

    /// `lv_indev_set_long_press_time()`
    pub fn indev_set_long_press_time(&mut self, indev: IndevId, long_press_time: u16) {
        todo!()
    }

    /// `lv_indev_set_long_press_repeat_time()`
    pub fn indev_set_long_press_repeat_time(&mut self, indev: IndevId, long_press_repeat_time: u16) {
        todo!()
    }

    /// `lv_indev_set_double_click_time()`
    pub fn indev_set_double_click_time(&mut self, indev: IndevId, double_click_time: u16) {
        todo!()
    }

    /// `lv_indev_set_scroll_limit()`
    pub fn indev_set_scroll_limit(&mut self, indev: IndevId, scroll_limit: u8) {
        todo!()
    }

    /// `lv_indev_set_scroll_throw()`
    pub fn indev_set_scroll_throw(&mut self, indev: IndevId, scroll_throw: u8) {
        todo!()
    }

    /// `lv_indev_set_gesture_min_velocity()`
    pub fn indev_set_gesture_min_velocity(&mut self, indev: IndevId, min_velocity: u8) {
        todo!()
    }

    /// `lv_indev_set_gesture_min_distance()`
    pub fn indev_set_gesture_min_distance(&mut self, indev: IndevId, min_distance: u8) {
        todo!()
    }

    /// `lv_indev_get_type()`
    pub fn indev_get_type(&self, indev: IndevId) -> IndevType {
        todo!()
    }

    /// `lv_indev_get_state()`
    pub fn indev_get_state(&self, indev: IndevId) -> IndevState {
        todo!()
    }

    /// `lv_indev_get_group()`
    pub fn indev_get_group(&self, indev: IndevId) -> Option<GroupId> {
        todo!()
    }

    /// `lv_indev_get_display()`
    pub fn indev_get_display(&self, indev: IndevId) -> Option<DisplayId> {
        todo!()
    }

    /// `lv_indev_get_user_data()`
    pub fn indev_get_user_data(&self, indev: IndevId) -> Option<Rc<dyn Any>> {
        todo!()
    }

    /// `lv_indev_get_driver_data()`
    pub fn indev_get_driver_data(&mut self, indev: IndevId) -> Option<&mut dyn Any> {
        todo!()
    }

    /// `lv_indev_get_read_cb()`: the installed driver.
    pub fn indev_driver_mut(&mut self, indev: IndevId) -> Option<&mut dyn IndevDriver> {
        todo!()
    }

    /// `lv_indev_get_press_moved()`
    pub fn indev_get_press_moved(&self, indev: IndevId) -> bool {
        todo!()
    }

    /// `lv_indev_reset()`: `None` indev resets all; `obj` limits to one object.
    pub fn indev_reset(&mut self, indev: Option<IndevId>, obj: Option<Obj>) {
        todo!()
    }

    /// `lv_indev_stop_processing()`
    pub fn indev_stop_processing(&mut self, indev: IndevId) {
        todo!()
    }

    /// `lv_indev_reset_long_press()`
    pub fn indev_reset_long_press(&mut self, indev: IndevId) {
        todo!()
    }

    /// `lv_indev_set_cursor()`
    pub fn indev_set_cursor(&mut self, indev: IndevId, cur_obj: Option<Obj>) {
        todo!()
    }

    /// `lv_indev_set_group()`
    pub fn indev_set_group(&mut self, indev: IndevId, group: Option<GroupId>) {
        todo!()
    }

    /// `lv_indev_set_button_points()`
    pub fn indev_set_button_points(&mut self, indev: IndevId, points: Vec<Point>) {
        todo!()
    }

    /// `lv_indev_get_point()`
    pub fn indev_get_point(&self, indev: IndevId) -> Point {
        todo!()
    }

    /// `lv_indev_get_gesture_dir()`
    pub fn indev_get_gesture_dir(&self, indev: IndevId) -> Dir {
        todo!()
    }

    /// `lv_indev_get_key()`
    pub fn indev_get_key(&self, indev: IndevId) -> u32 {
        todo!()
    }

    /// `lv_indev_get_short_click_streak()`
    pub fn indev_get_short_click_streak(&self, indev: IndevId) -> u8 {
        todo!()
    }

    /// `lv_indev_get_scroll_dir()`
    pub fn indev_get_scroll_dir(&self, indev: IndevId) -> Dir {
        todo!()
    }

    /// `lv_indev_get_scroll_obj()`
    pub fn indev_get_scroll_obj(&self, indev: IndevId) -> Option<Obj> {
        todo!()
    }

    /// `lv_indev_get_vect()`
    pub fn indev_get_vect(&self, indev: IndevId) -> Point {
        todo!()
    }

    /// `lv_indev_get_cursor()`
    pub fn indev_get_cursor(&self, indev: IndevId) -> Option<Obj> {
        todo!()
    }

    /// `lv_indev_wait_release()`
    pub fn indev_wait_release(&mut self, indev: IndevId) {
        todo!()
    }

    /// `lv_indev_get_active_obj()`
    pub fn indev_get_active_obj(&self) -> Option<Obj> {
        self.indev_obj_active
    }

    /// `lv_indev_get_read_timer()`
    pub fn indev_get_read_timer(&self, indev: IndevId) -> Option<TimerId> {
        todo!()
    }

    /// `lv_indev_set_mode()`
    pub fn indev_set_mode(&mut self, indev: IndevId, mode: IndevMode) {
        todo!()
    }

    /// `lv_indev_get_mode()`
    pub fn indev_get_mode(&self, indev: IndevId) -> IndevMode {
        todo!()
    }

    /// `lv_indev_search_obj()`: the topmost clickable object at `point`.
    pub fn indev_search_obj(&mut self, obj: Obj, point: Point) -> Option<Obj> {
        todo!()
    }

    /// `lv_indev_add_event_cb()`
    pub fn indev_add_event_cb(
        &mut self,
        indev: IndevId,
        cb: EventCallback,
        filter: impl Into<EventFilter>,
        user_data: Option<Rc<dyn Any>>,
    ) -> EventDscId {
        todo!()
    }

    /// `lv_indev_get_event_count()`
    pub fn indev_get_event_count(&self, indev: IndevId) -> u32 {
        todo!()
    }

    /// `lv_indev_get_event_dsc()`
    pub fn indev_get_event_dsc(&self, indev: IndevId, index: u32) -> Option<&EventDsc> {
        todo!()
    }

    /// `lv_indev_remove_event()`
    pub fn indev_remove_event(&mut self, indev: IndevId, index: u32) -> bool {
        todo!()
    }

    /// `lv_indev_remove_event_cb_with_user_data()`
    pub fn indev_remove_event_cb_with_user_data(
        &mut self,
        indev: IndevId,
        cb: &EventCallback,
        user_data: &Rc<dyn Any>,
    ) -> u32 {
        todo!()
    }

    /// `lv_indev_send_event()`
    pub fn indev_send_event(&mut self, indev: IndevId, code: EventCode, param: EventParam<'_>) -> LvResult {
        todo!()
    }

    /// `lv_indev_set_key_remap_cb()`
    pub fn indev_set_key_remap_cb(&mut self, indev: IndevId, remap_cb: Option<IndevKeyRemapCallback>) {
        todo!()
    }

    /// `lv_indev_set_ccw()`
    pub fn indev_set_ccw(&mut self, indev: IndevId) {
        todo!()
    }

    /// `lv_indev_clear_ccw()`
    pub fn indev_clear_ccw(&mut self, indev: IndevId) {
        todo!()
    }

    /// `lv_indev_get_ccw()`
    pub fn indev_get_ccw(&self, indev: IndevId) -> bool {
        todo!()
    }

    /// `lv_indev_find_scroll_obj()`
    pub fn indev_find_scroll_obj(&mut self, indev: IndevId) -> Option<Obj> {
        todo!()
    }

    /// `lv_indev_scroll_handler()`
    pub fn indev_scroll_handler(&mut self, indev: IndevId) {
        todo!()
    }

    /// `lv_indev_scroll_throw_handler()`
    pub fn indev_scroll_throw_handler(&mut self, indev: IndevId) {
        todo!()
    }

    /// `lv_indev_scroll_throw_predict()`
    pub fn indev_scroll_throw_predict(&mut self, indev: IndevId, dir: Dir) -> i32 {
        todo!()
    }

    /// `lv_indev_scroll_get_snap_dist()`
    pub fn indev_scroll_get_snap_dist(&mut self, obj: Obj) -> Point {
        todo!()
    }

    /// Remap a key through the device's `key_remap_cb`.
    pub fn indev_remap_key(&mut self, indev: IndevId, key: LvKey) -> LvKey {
        todo!()
    }

    /// Borrow an input device.
    #[inline]
    pub fn indev(&self, indev: IndevId) -> Option<&IndevData> {
        self.indevs.get(indev.0)
    }

    /// Mutably borrow an input device.
    #[inline]
    pub fn indev_mut(&mut self, indev: IndevId) -> Option<&mut IndevData> {
        self.indevs.get_mut(indev.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_values() {
        assert_eq!(IndevType::Button as u8, 3);
        assert_eq!(IndevType::Encoder as u8, 4);
        assert_eq!(IndevMode::Event as u8, 2);
        assert_eq!(IndevGestureType::Cnt as u8, 6);
        assert_eq!(INDEV_VECT_HIST_SIZE, 8);
    }
}
