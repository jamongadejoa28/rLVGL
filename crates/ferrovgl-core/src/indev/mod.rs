//! Ported from lvgl v9.6.0: `include/lvgl/indev/lv_indev.h`, `src/indev/lv_indev_private.h`,
//! `src/indev/lv_indev_scroll.h`, `src/indev/lv_indev.c`.

//! Directional gestures are included. The optional upstream multitouch gesture
//! recognizers (`LV_USE_GESTURE_RECOGNITION`) are not represented by this API.
//! Device handles are revalidated after callbacks; recursive reads/deletes of
//! the same device are ignored rather than accessing a freed C-style pointer.

mod keypad;
mod pointer;
mod scroll;

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::{Obj, ObjFlags};
use crate::core::obj_pos::ObjPointTransformFlag;
use crate::display::DisplayId;
use crate::group::{GroupId, Key as LvKey};
use crate::misc::anim::AnimId;
use crate::misc::area::{Area, Dir, Point};
use crate::misc::arena::Key;
use crate::misc::event::{
    EventCallback, EventCode, EventDsc, EventDscId, EventFilter, EventList, EventParam, EventTarget, LvResult,
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

    /// Read with access to the UI, as in a C driver callback. Override when a
    /// driver needs UI APIs; ordinary drivers only implement [`Self::read`].
    /// The driver is temporarily detached while called, so recursive reads of
    /// this device do not alias its mutable state. Deleting the device is safe.
    fn read_with_ui(&mut self, _ui: &mut Ui, indev: IndevId, data: &mut IndevReadData) {
        self.read(indev, data);
    }
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
    deleting: bool,
    reading: bool,
    driver_revision: u64,
}

impl Ui {
    /// `lv_indev_create()`
    pub fn indev_create(&mut self) -> IndevId {
        let cfg = self.config();
        let data = IndevData {
            ty: IndevType::None,
            driver: None,
            state: IndevState::Released,
            prev_state: IndevState::Released,
            mode: IndevMode::Timer,
            long_pr_sent: false,
            reset_query: true,
            enabled: true,
            wait_until_release: false,
            stop_processing_query: false,
            timestamp: 0,
            pr_timestamp: 0,
            longpr_rep_timestamp: 0,
            driver_data: None,
            user_data: None,
            disp: self.display_get_default(),
            read_timer: None,
            scroll_limit: cfg.indev_scroll_limit,
            scroll_throw: cfg.indev_scroll_throw,
            gesture_min_velocity: cfg.indev_def_gesture_min_velocity,
            gesture_min_distance: cfg.indev_def_gesture_limit,
            long_press_time: cfg.indev_long_press_time,
            double_click_time: cfg.indev_double_click_time,
            long_press_repeat_time: cfg.indev_long_press_repeat_time,
            rotary_sensitivity: cfg.indev_def_rotary_sensitivity,
            pointer: IndevPointer::default(),
            keypad: IndevKeypad::default(),
            cursor: None,
            group: None,
            btn_points: Vec::new(),
            event_list: EventList::default(),
            scroll_throw_anim: None,
            key_remap_cb: None,
            deleting: false,
            reading: false,
            driver_revision: 0,
        };
        let id = IndevId(self.indevs.insert(data));
        self.indev_order.insert(0, id);
        let timer = self.timer_create(
            Rc::new(|ui, timer| ui.indev_read_timer_cb(timer)),
            self.config().refr_period_ms,
            Some(Rc::new(id)),
        );
        self.indev_mut(id).unwrap().read_timer = Some(timer);
        id
    }

    /// `lv_indev_delete()`
    pub fn indev_delete(&mut self, indev: IndevId) {
        let Some(data) = self.indev_mut(indev) else {
            return;
        };
        if data.deleting {
            return;
        }
        data.deleting = true;
        self.indev_send_event(indev, EventCode::DELETE, EventParam::None);
        self.event_mark_deleted(EventTarget::Indev(indev));
        self.indev_with_event_list(indev, |ui, list| ui.event_remove_all(list));
        if let Some(timer) = self.indev(indev).and_then(|d| d.read_timer) {
            self.timer_delete(timer);
        }
        self.anim_delete(crate::misc::anim::AnimTarget::Indev(indev), None);
        self.indev_order.retain(|&id| id != indev);
        self.indevs.remove(indev.0);
        if self.indev_active == Some(indev) {
            self.indev_active = None;
            self.indev_obj_active = None;
        }
    }

    /// `lv_indev_get_next()`
    pub fn indev_get_next(&self, indev: Option<IndevId>) -> Option<IndevId> {
        match indev {
            None => self.indev_order.first().copied(),
            Some(indev) => {
                let index = self.indev_order.iter().position(|&id| id == indev)?;
                self.indev_order.get(index + 1).copied()
            }
        }
    }

    /// `lv_indev_read()`: read and process the device once.
    pub fn indev_read(&mut self, indev: IndevId) {
        if self.indev(indev).is_none_or(|d| d.reading || d.deleting) {
            return;
        }
        let old_active = self.indev_active;
        let old_obj = self.indev_obj_active;
        self.indev_active = Some(indev);
        self.indev_mut(indev).unwrap().reading = true;
        self.indev_read_inner(indev);
        if let Some(d) = self.indev_mut(indev) {
            d.reading = false;
        }
        // Nested reads restore the outer context; top-level reads clear it,
        // including the early-return paths that leave stale globals in C.
        self.indev_active = old_active.filter(|&i| self.indev(i).is_some());
        self.indev_obj_active = old_obj.filter(|&o| self.obj_is_valid(o));
    }

    /// `lv_indev_read_timer_cb()`
    pub fn indev_read_timer_cb(&mut self, timer: TimerId) {
        if let Some(id) = self
            .timer_get_user_data(timer)
            .and_then(|d| d.downcast_ref::<IndevId>().copied())
        {
            self.indev_read(id);
        }
    }

    /// `lv_indev_enable()`
    pub fn indev_enable(&mut self, indev: Option<IndevId>, enable: bool) {
        if let Some(indev) = indev {
            if let Some(d) = self.indev_mut(indev) {
                d.enabled = enable;
            }
        } else {
            for id in self.indev_order.clone() {
                if let Some(d) = self.indev_mut(id) {
                    d.enabled = enable;
                }
            }
        }
    }

    /// `lv_indev_active()`
    pub fn indev_active(&self) -> Option<IndevId> {
        self.indev_active
    }

    /// `lv_indev_set_type()`
    pub fn indev_set_type(&mut self, indev: IndevId, ty: IndevType) {
        if let Some(d) = self.indev_mut(indev) {
            d.ty = ty;
            d.reset_query = true;
        }
    }

    /// `lv_indev_set_read_cb()`
    pub fn indev_set_driver(&mut self, indev: IndevId, driver: Option<Box<dyn IndevDriver>>) {
        if let Some(d) = self.indev_mut(indev) {
            d.driver = driver;
            d.driver_revision = d.driver_revision.wrapping_add(1);
        }
    }

    /// `lv_indev_set_user_data()`
    pub fn indev_set_user_data(&mut self, indev: IndevId, user_data: Option<Rc<dyn Any>>) {
        if let Some(d) = self.indev_mut(indev) {
            d.user_data = user_data;
        }
    }

    /// `lv_indev_set_driver_data()`
    pub fn indev_set_driver_data(&mut self, indev: IndevId, driver_data: Option<Box<dyn Any>>) {
        if let Some(d) = self.indev_mut(indev) {
            d.driver_data = driver_data;
        }
    }

    /// `lv_indev_set_display()`
    pub fn indev_set_display(&mut self, indev: IndevId, disp: Option<DisplayId>) {
        if let Some(d) = self.indev_mut(indev) {
            d.disp = disp;
        }
    }

    /// `lv_indev_set_long_press_time()`
    pub fn indev_set_long_press_time(&mut self, indev: IndevId, long_press_time: u16) {
        if let Some(d) = self.indev_mut(indev) {
            d.long_press_time = long_press_time;
        }
    }

    /// `lv_indev_set_long_press_repeat_time()`
    pub fn indev_set_long_press_repeat_time(&mut self, indev: IndevId, long_press_repeat_time: u16) {
        if let Some(d) = self.indev_mut(indev) {
            d.long_press_repeat_time = long_press_repeat_time;
        }
    }

    /// `lv_indev_set_double_click_time()`
    pub fn indev_set_double_click_time(&mut self, indev: IndevId, double_click_time: u16) {
        if let Some(d) = self.indev_mut(indev) {
            d.double_click_time = double_click_time;
        }
    }

    /// `lv_indev_set_scroll_limit()`
    pub fn indev_set_scroll_limit(&mut self, indev: IndevId, scroll_limit: u8) {
        if let Some(d) = self.indev_mut(indev) {
            d.scroll_limit = scroll_limit;
        }
    }

    /// `lv_indev_set_scroll_throw()`
    pub fn indev_set_scroll_throw(&mut self, indev: IndevId, scroll_throw: u8) {
        if let Some(d) = self.indev_mut(indev) {
            d.scroll_throw = scroll_throw;
        }
    }

    /// `lv_indev_set_gesture_min_velocity()`
    pub fn indev_set_gesture_min_velocity(&mut self, indev: IndevId, min_velocity: u8) {
        if let Some(d) = self.indev_mut(indev) {
            d.gesture_min_velocity = min_velocity;
        }
    }

    /// `lv_indev_set_gesture_min_distance()`
    pub fn indev_set_gesture_min_distance(&mut self, indev: IndevId, min_distance: u8) {
        if let Some(d) = self.indev_mut(indev) {
            d.gesture_min_distance = min_distance;
        }
    }

    /// `lv_indev_get_type()`
    pub fn indev_get_type(&self, indev: IndevId) -> IndevType {
        self.indev(indev).map_or(IndevType::None, |d| d.ty)
    }

    /// `lv_indev_get_state()`
    pub fn indev_get_state(&self, indev: IndevId) -> IndevState {
        self.indev(indev).map_or(IndevState::Released, |d| d.state)
    }

    /// `lv_indev_get_group()`
    pub fn indev_get_group(&self, indev: IndevId) -> Option<GroupId> {
        self.indev(indev).and_then(|d| d.group)
    }

    /// `lv_indev_get_display()`
    pub fn indev_get_display(&self, indev: IndevId) -> Option<DisplayId> {
        self.indev(indev).and_then(|d| d.disp)
    }

    /// `lv_indev_get_user_data()`
    pub fn indev_get_user_data(&self, indev: IndevId) -> Option<Rc<dyn Any>> {
        self.indev(indev).and_then(|d| d.user_data.clone())
    }

    /// `lv_indev_get_driver_data()`
    pub fn indev_get_driver_data(&mut self, indev: IndevId) -> Option<&mut dyn Any> {
        self.indev_mut(indev)?.driver_data.as_deref_mut()
    }

    /// `lv_indev_get_read_cb()`: the installed driver.
    pub fn indev_driver_mut(&mut self, indev: IndevId) -> Option<&mut dyn IndevDriver> {
        match self.indev_mut(indev)?.driver.as_mut() {
            Some(driver) => Some(driver.as_mut()),
            None => None,
        }
    }

    /// `lv_indev_get_press_moved()`
    pub fn indev_get_press_moved(&self, indev: IndevId) -> bool {
        self.indev(indev).is_some_and(|d| d.pointer.press_moved)
    }

    /// `lv_indev_reset()`: `None` indev resets all; `obj` limits to one object.
    pub fn indev_reset(&mut self, indev: Option<IndevId>, obj: Option<Obj>) {
        if let Some(i) = indev {
            self.indev_reset_core(i, obj);
        } else {
            for i in self.indev_order.clone() {
                self.indev_reset_core(i, obj);
            }
            self.indev_obj_active = None;
        }
    }

    /// `lv_indev_stop_processing()`
    pub fn indev_stop_processing(&mut self, indev: IndevId) {
        if let Some(d) = self.indev_mut(indev) {
            d.stop_processing_query = true;
        }
    }

    /// `lv_indev_reset_long_press()`
    pub fn indev_reset_long_press(&mut self, indev: IndevId) {
        let now = self.tick_get();
        if let Some(d) = self.indev_mut(indev) {
            d.long_pr_sent = false;
            d.longpr_rep_timestamp = now;
            d.pr_timestamp = now;
        }
    }

    /// `lv_indev_set_cursor()`
    pub fn indev_set_cursor(&mut self, indev: IndevId, cur_obj: Option<Obj>) {
        let Some(d) = self.indev(indev) else {
            return;
        };
        if d.ty != IndevType::Pointer {
            return;
        }
        let Some(cur) = cur_obj.filter(|&o| self.obj_is_valid(o)) else {
            return;
        };
        let (disp, point) = (d.disp, d.pointer.act_point);
        let Some(parent) = disp.and_then(|d| self.display_get_layer_sys(Some(d))) else {
            return;
        };
        self.indev_mut(indev).unwrap().cursor = Some(cur);
        self.obj_set_parent(cur, parent);
        if self.obj_is_valid(cur) {
            self.obj_set_pos(cur, point.x, point.y);
            self.obj_remove_flag(cur, ObjFlags::CLICKABLE);
            self.obj_add_flag(cur, ObjFlags::IGNORE_LAYOUT | ObjFlags::FLOATING);
        }
    }

    /// `lv_indev_set_group()`
    pub fn indev_set_group(&mut self, indev: IndevId, group: Option<GroupId>) {
        if let Some(d) = self.indev_mut(indev)
            && matches!(d.ty, IndevType::Keypad | IndevType::Encoder)
        {
            d.group = group;
        }
    }

    /// `lv_indev_set_button_points()`
    pub fn indev_set_button_points(&mut self, indev: IndevId, points: Vec<Point>) {
        if let Some(d) = self.indev_mut(indev)
            && d.ty == IndevType::Button
        {
            d.btn_points = points;
        }
    }

    /// `lv_indev_get_point()`
    pub fn indev_get_point(&self, indev: IndevId) -> Point {
        self.indev(indev).map_or(Point::default(), |d| {
            if matches!(d.ty, IndevType::Pointer | IndevType::Button) {
                d.pointer.act_point
            } else {
                Point::new(-1, -1)
            }
        })
    }

    /// `lv_indev_get_gesture_dir()`
    pub fn indev_get_gesture_dir(&self, indev: IndevId) -> Dir {
        self.indev(indev).map_or(Dir::NONE, |d| d.pointer.gesture_dir)
    }

    /// `lv_indev_get_key()`
    pub fn indev_get_key(&self, indev: IndevId) -> u32 {
        self.indev(indev)
            .filter(|d| d.ty == IndevType::Keypad)
            .map_or(0, |d| d.keypad.last_key)
    }

    /// `lv_indev_get_short_click_streak()`
    pub fn indev_get_short_click_streak(&self, indev: IndevId) -> u8 {
        self.indev(indev).map_or(0, |d| d.pointer.short_click_streak)
    }

    /// `lv_indev_get_scroll_dir()`
    pub fn indev_get_scroll_dir(&self, indev: IndevId) -> Dir {
        self.indev(indev)
            .filter(|d| matches!(d.ty, IndevType::Pointer | IndevType::Button))
            .map_or(Dir::NONE, |d| d.pointer.scroll_dir)
    }

    /// `lv_indev_get_scroll_obj()`
    pub fn indev_get_scroll_obj(&self, indev: IndevId) -> Option<Obj> {
        self.indev(indev)
            .filter(|d| matches!(d.ty, IndevType::Pointer | IndevType::Button))
            .and_then(|d| d.pointer.scroll_obj)
    }

    /// `lv_indev_get_vect()`
    pub fn indev_get_vect(&self, indev: IndevId) -> Point {
        self.indev(indev)
            .filter(|d| matches!(d.ty, IndevType::Pointer | IndevType::Button))
            .map_or(Point::default(), |d| d.pointer.vect)
    }

    /// `lv_indev_get_cursor()`
    pub fn indev_get_cursor(&self, indev: IndevId) -> Option<Obj> {
        self.indev(indev).and_then(|d| d.cursor)
    }

    /// `lv_indev_wait_release()`
    pub fn indev_wait_release(&mut self, indev: IndevId) {
        if let Some(d) = self.indev_mut(indev) {
            d.wait_until_release = true;
        }
    }

    /// `lv_indev_get_active_obj()`
    pub fn indev_get_active_obj(&self) -> Option<Obj> {
        self.indev_obj_active
    }

    /// `lv_indev_get_read_timer()`
    pub fn indev_get_read_timer(&self, indev: IndevId) -> Option<TimerId> {
        self.indev(indev).and_then(|d| d.read_timer)
    }

    /// `lv_indev_set_mode()`
    pub fn indev_set_mode(&mut self, indev: IndevId, mode: IndevMode) {
        let Some(d) = self.indev_mut(indev) else {
            return;
        };
        if d.mode == mode {
            return;
        }
        d.mode = mode;
        if let Some(timer) = d.read_timer {
            match mode {
                IndevMode::Event => self.timer_pause(timer),
                IndevMode::Timer => {
                    self.timer_set_cb(timer, Some(Rc::new(|ui, timer| ui.indev_read_timer_cb(timer))));
                    self.timer_resume(timer);
                }
                IndevMode::None => {}
            }
        }
    }

    /// `lv_indev_get_mode()`
    pub fn indev_get_mode(&self, indev: IndevId) -> IndevMode {
        self.indev(indev).map_or(IndevMode::None, |d| d.mode)
    }

    /// `lv_indev_search_obj()`: the topmost clickable object at `point`.
    pub fn indev_search_obj(&mut self, obj: Obj, point: Point) -> Option<Obj> {
        if !self.obj_is_valid(obj) || self.obj_has_flag(obj, ObjFlags::HIDDEN) {
            return None;
        }
        let mut transformed = point;
        self.obj_transform_point(obj, &mut transformed, ObjPointTransformFlag::Inverse);
        let hit = self.obj_hit_test(obj, transformed);
        // ADV_HITTEST callbacks can delete the tested object or its parent.
        let mut coords = self.obj(obj)?.coords;
        if self.obj_has_flag(obj, ObjFlags::OVERFLOW_VISIBLE) {
            let size = self.obj_get_ext_draw_size(obj);
            coords.increase(size, size);
        }
        if coords.is_point_on(transformed, 0) {
            let children = self
                .obj(obj)?
                .spec
                .as_ref()
                .map(|s| s.children.clone())
                .unwrap_or_default();
            for child in children.into_iter().rev() {
                if let Some(found) = self.indev_search_obj(Obj(child), transformed) {
                    return Some(found);
                }
                if !self.obj_is_valid(obj) {
                    return None;
                }
            }
        }
        if hit && self.obj_is_valid(obj) {
            Some(obj)
        } else {
            None
        }
    }

    /// `lv_indev_add_event_cb()`
    pub fn indev_add_event_cb(
        &mut self,
        indev: IndevId,
        cb: EventCallback,
        filter: impl Into<EventFilter>,
        user_data: Option<Rc<dyn Any>>,
    ) -> EventDscId {
        let filter = filter.into();
        if self.indev(indev).is_none() {
            let key = self.event_dscs.insert(EventDsc {
                ext_data: Default::default(),
                cb,
                filter: filter.to_u32(),
                user_data,
            });
            self.event_dscs.remove(key);
            return EventDscId(key);
        }
        self.indev_with_event_list(indev, |ui, list| ui.event_add(list, cb, filter, user_data))
            .unwrap()
    }

    /// `lv_indev_get_event_count()`
    pub fn indev_get_event_count(&self, indev: IndevId) -> u32 {
        self.indev(indev).map_or(0, |d| d.event_list.len() as u32)
    }

    /// `lv_indev_get_event_dsc()`
    pub fn indev_get_event_dsc(&self, indev: IndevId, index: u32) -> Option<&EventDsc> {
        self.event_get_dsc(&self.indev(indev)?.event_list, index as usize)
    }

    /// `lv_indev_remove_event()`
    pub fn indev_remove_event(&mut self, indev: IndevId, index: u32) -> bool {
        self.indev_with_event_list(indev, |ui, list| ui.event_remove(list, index as usize))
            .unwrap_or(false)
    }

    /// `lv_indev_remove_event_cb_with_user_data()`
    pub fn indev_remove_event_cb_with_user_data(
        &mut self,
        indev: IndevId,
        cb: &EventCallback,
        user_data: &Rc<dyn Any>,
    ) -> u32 {
        let mut removed = 0;
        for index in (0..self.indev_get_event_count(indev)).rev() {
            if self.indev_get_event_dsc(indev, index).is_some_and(|d| {
                Rc::ptr_eq(&d.cb, cb) && d.user_data.as_ref().is_some_and(|u| Rc::ptr_eq(u, user_data))
            }) {
                self.indev_remove_event(indev, index);
                removed += 1;
            }
        }
        removed
    }

    /// `lv_indev_send_event()`
    pub fn indev_send_event(&mut self, indev: IndevId, code: EventCode, param: EventParam<'_>) -> LvResult {
        let Some(d) = self.indev(indev) else {
            return LvResult::Invalid;
        };
        self.event_push_and_send(d.event_list.clone(), code, EventTarget::Indev(indev), param)
    }

    /// `lv_indev_set_key_remap_cb()`
    pub fn indev_set_key_remap_cb(&mut self, indev: IndevId, remap_cb: Option<IndevKeyRemapCallback>) {
        if let Some(d) = self.indev_mut(indev) {
            d.key_remap_cb = remap_cb;
        }
    }

    /// `lv_indev_set_ccw()`
    pub fn indev_set_ccw(&mut self, indev: IndevId) {
        if let Some(d) = self.indev_mut(indev)
            && d.ty == IndevType::Pointer
        {
            d.pointer.ccw_rotation = true;
        }
    }

    /// `lv_indev_clear_ccw()`
    pub fn indev_clear_ccw(&mut self, indev: IndevId) {
        if let Some(d) = self.indev_mut(indev)
            && d.ty == IndevType::Pointer
        {
            d.pointer.ccw_rotation = false;
        }
    }

    /// `lv_indev_get_ccw()`
    pub fn indev_get_ccw(&self, indev: IndevId) -> bool {
        self.indev(indev)
            .is_some_and(|d| d.ty == IndevType::Pointer && d.pointer.ccw_rotation)
    }

    /// Remap a key through the device's `key_remap_cb`.
    pub fn indev_remap_key(&mut self, indev: IndevId, key: LvKey) -> LvKey {
        let cb = self.indev(indev).and_then(|d| d.key_remap_cb.clone());
        cb.map_or(key, |cb| cb(self, indev, key))
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
mod input_tests;

impl Ui {
    fn indev_with_event_list<R>(
        &mut self,
        indev: IndevId,
        f: impl FnOnce(&mut Ui, &mut EventList) -> R,
    ) -> Option<R> {
        let mut list = core::mem::take(&mut self.indev_mut(indev)?.event_list);
        let result = f(self, &mut list);
        if let Some(d) = self.indev_mut(indev) {
            d.event_list = list;
        }
        Some(result)
    }

    fn indev_read_inner(&mut self, indev: IndevId) {
        let Some(disp) = self.indev(indev).and_then(|d| d.disp) else {
            return;
        };
        self.indev_proc_reset_query_handler(indev);
        if self.indev(indev).is_none_or(|d| !d.enabled)
            || self.display(disp).is_none_or(|d| d.prev_scr.is_some())
        {
            return;
        }
        loop {
            let Some(d) = self.indev_mut(indev) else {
                return;
            };
            let mut data = IndevReadData::default();
            match d.ty {
                IndevType::Pointer => data.point = d.pointer.last_raw_point,
                IndevType::Keypad | IndevType::Encoder => data.key = d.keypad.last_key,
                _ => {}
            }
            let driver_revision = d.driver_revision;
            if let Some(mut driver) = d.driver.take() {
                driver.read_with_ui(self, indev, &mut data);
                let Some(d) = self.indev_mut(indev) else {
                    return;
                };
                if d.driver_revision == driver_revision && d.driver.is_none() {
                    d.driver = Some(driver);
                }
                if data.timestamp == 0 {
                    data.timestamp = self.tick_get();
                }
            }
            let Some(d) = self.indev(indev) else {
                return;
            };
            let continue_reading = d.mode != IndevMode::Event && data.continue_reading;
            self.indev_proc_reset_query_handler(indev);
            self.indev_obj_active = None;
            let Some(d) = self.indev_mut(indev) else {
                return;
            };
            d.state = data.state;
            d.timestamp = data.timestamp;
            let ty = d.ty;
            if (data.state == IndevState::Pressed || (ty == IndevType::Encoder && data.enc_diff != 0))
                && let Some(disp) = d.disp
                && let Some(display) = self.display_mut(disp)
            {
                display.last_activity_time = data.timestamp;
            }
            match ty {
                IndevType::Pointer => self.indev_pointer_proc(indev, &mut data),
                IndevType::Button => self.indev_button_proc(indev, &data),
                IndevType::Keypad => self.indev_keypad_proc(indev, &mut data),
                IndevType::Encoder => self.indev_encoder_proc(indev, &mut data),
                IndevType::None => {}
            }
            self.indev_proc_reset_query_handler(indev);
            if !continue_reading || self.indev(indev).is_none_or(|d| d.disp.is_none()) {
                break;
            }
        }
    }

    fn indev_proc_reset_query_handler(&mut self, indev: IndevId) {
        let now = self.tick_get();
        let Some(d) = self.indev_mut(indev) else {
            return;
        };
        if !d.reset_query {
            return;
        }
        d.pointer.act_obj = None;
        d.pointer.scroll_obj = None;
        d.pointer.last_hovered = None;
        d.timestamp = now;
        d.long_pr_sent = false;
        d.pr_timestamp = 0;
        d.longpr_rep_timestamp = 0;
        d.pointer.scroll_sum = Point::default();
        d.pointer.scroll_dir = Dir::NONE;
        d.pointer.scroll_throw_vect = Point::default();
        d.pointer.gesture_sum = Point::default();
        d.reset_query = false;
        if d.ty == IndevType::Encoder {
            d.keypad.last_key = crate::group::KEY_ENTER;
        }
        d.stop_processing_query = false;
        self.indev_obj_active = None;
    }

    fn indev_reset_check(&mut self, indev: IndevId) -> bool {
        let reset = self.indev(indev).is_none_or(|d| d.reset_query || d.deleting);
        if reset {
            self.indev_obj_active = None;
        }
        reset
    }

    fn indev_reset_core(&mut self, indev: IndevId, obj: Option<Obj>) {
        let Some(d) = self.indev_mut(indev) else {
            return;
        };
        d.reset_query = true;
        let ty = d.ty;
        if self.indev_active == Some(indev) {
            self.indev_obj_active = None;
        }
        // This type condition intentionally follows v9.6 (POINTER or KEYPAD).
        if !matches!(ty, IndevType::Pointer | IndevType::Keypad) {
            return;
        }
        let d = self.indev_mut(indev).unwrap();
        if obj.is_none() || d.pointer.last_pressed == obj {
            d.pointer.last_pressed = None;
        }
        if let Some(active) = d.pointer.act_obj.take() {
            self.obj_send_event(active, EventCode::INDEV_RESET, EventParam::Indev(indev));
            self.indev_send_event(indev, EventCode::INDEV_RESET, EventParam::Obj(active));
        }
        let Some(d) = self.indev_mut(indev) else {
            return;
        };
        if let Some(scroll) = d.pointer.scroll_obj.take() {
            self.obj_send_event(scroll, EventCode::INDEV_RESET, EventParam::Indev(indev));
            self.indev_send_event(indev, EventCode::INDEV_RESET, EventParam::Obj(scroll));
        }
        if let Some(d) = self.indev_mut(indev)
            && (obj.is_none() || d.pointer.last_hovered == obj)
        {
            d.pointer.last_hovered = None;
        }
    }

    /// Upstream's private `send_event`: selected device callbacks run before
    /// object callbacks, allowing `indev_stop_processing` to consume an event.
    fn indev_dispatch(&mut self, indev: IndevId, code: EventCode, param: EventParam<'_>) -> LvResult {
        if matches!(
            code,
            EventCode::PRESSED
                | EventCode::SHORT_CLICKED
                | EventCode::CLICKED
                | EventCode::RELEASED
                | EventCode::LONG_PRESSED
                | EventCode::LONG_PRESSED_REPEAT
                | EventCode::ROTARY
                | EventCode::KEY
        ) {
            let obj = self.indev_obj_active;
            self.indev_send_event(indev, code, obj.map_or(EventParam::None, EventParam::Obj));
            if self.indev_reset_check(indev) {
                return LvResult::Invalid;
            }
            if let Some(d) = self.indev_mut(indev)
                && d.stop_processing_query
            {
                d.stop_processing_query = false;
                return LvResult::Ok;
            }
        }
        if let Some(obj) = self.indev_obj_active {
            if !self.obj_is_valid(obj) {
                return LvResult::Invalid;
            }
            let result = self.obj_send_event(obj, code, param);
            if self.indev_reset_check(indev) || result == LvResult::Invalid {
                return LvResult::Invalid;
            }
        }
        LvResult::Ok
    }

    fn indev_input_event(&mut self, indev: IndevId, code: EventCode) -> bool {
        self.indev_dispatch(indev, code, EventParam::Indev(indev)) == LvResult::Ok
    }

    fn indev_proc_short_click(&mut self, indev: IndevId) -> bool {
        let point = self.indev_get_point(indev);
        let Some(d) = self.indev_mut(indev) else {
            return false;
        };
        d.pointer.short_click_streak = d.pointer.short_click_streak.wrapping_add(1);
        if d.timestamp.wrapping_sub(d.pointer.last_short_click_timestamp) > u32::from(d.double_click_time) {
            d.pointer.short_click_streak = 1;
        } else if matches!(d.ty, IndevType::Pointer | IndevType::Button) {
            let dx = i64::from(d.pointer.last_short_click_point.x) - i64::from(d.pointer.act_point.x);
            let dy = i64::from(d.pointer.last_short_click_point.y) - i64::from(d.pointer.act_point.y);
            if dx * dx + dy * dy > i64::from(d.scroll_limit).pow(2) {
                d.pointer.short_click_streak = 1;
            }
        }
        d.pointer.last_short_click_timestamp = d.timestamp;
        d.pointer.last_short_click_point = point;
        if !self.indev_input_event(indev, EventCode::SHORT_CLICKED) {
            return false;
        }
        let Some(d) = self.indev(indev) else {
            return false;
        };
        let code = match (i32::from(d.pointer.short_click_streak) - 1) % 3 {
            0 => EventCode::SINGLE_CLICKED,
            1 => EventCode::DOUBLE_CLICKED,
            2 => EventCode::TRIPLE_CLICKED,
            _ => return true,
        };
        self.indev_input_event(indev, code)
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
