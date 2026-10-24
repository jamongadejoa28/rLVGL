//! Ported from lvgl v9.6.0: `src/core/lv_global.h`, `src/core/lv_global.c`, `src/lv_init.c`
//! (and the numeric options of `lv_conf_template.h`).
//!
//! [`Ui`] is the single context that owns every object, display, input
//! device, group, timer, animation, style and event callback (PLAN.md 4.1).
//! It replaces `lv_global_t` and all static state; there is no global
//! mutable state anywhere in the crate.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;

use crate::core::obj::{Obj, ObjData, ObjId};
use crate::core::obj_style::{StyleTrans, StyleTransId};
use crate::display::{DisplayData, DisplayId};
use crate::draw::{DrawCtx, DrawUnit};
use crate::font::FontRef;
use crate::group::{GroupData, GroupId};
use crate::indev::{IndevData, IndevId};
use crate::layouts::LayoutDsc;
use crate::misc::anim::AnimState;
use crate::misc::arena::Arena;
use crate::misc::event::{EventCode, EventDsc, EventFrame};
use crate::misc::math::Rand;
use crate::misc::style::{Style, StyleProp};
use crate::misc::tick::TickState;
use crate::misc::timer::TimerState;
use crate::theme::Theme;

/// `LV_DEF_REFR_PERIOD`
pub const DEF_REFR_PERIOD: u32 = 33;
/// `LV_DPI_DEF`
pub const DPI_DEF: i32 = 130;
/// `LV_INDEV_DEF_LONG_PRESS_TIME`
pub const INDEV_DEF_LONG_PRESS_TIME: u16 = 400;
/// `LV_INDEV_DEF_LONG_PRESS_REP_TIME`
pub const INDEV_DEF_LONG_PRESS_REP_TIME: u16 = 100;
/// `LV_INDEV_DEF_DOUBLE_CLICK_TIME`
pub const INDEV_DEF_DOUBLE_CLICK_TIME: u16 = 400;
/// `LV_INDEV_DEF_SCROLL_LIMIT`
pub const INDEV_DEF_SCROLL_LIMIT: u8 = 10;
/// `LV_INDEV_DEF_SCROLL_THROW`
pub const INDEV_DEF_SCROLL_THROW: u8 = 10;
/// `LV_INDEV_DEF_GESTURE_LIMIT`
pub const INDEV_DEF_GESTURE_LIMIT: u8 = 50;
/// `LV_INDEV_DEF_GESTURE_MIN_VELOCITY`
pub const INDEV_DEF_GESTURE_MIN_VELOCITY: u8 = 3;
/// `LV_INDEV_DEF_ROTARY_SENSITIVITY`
pub const INDEV_DEF_ROTARY_SENSITIVITY: i32 = 256;
/// `LV_INV_BUF_SIZE`
pub const INV_BUF_SIZE: usize = 32;
/// `LV_DRAW_LAYER_SIMPLE_BUF_SIZE`
pub const DRAW_LAYER_SIMPLE_BUF_SIZE: u32 = 24576;
/// `LV_DRAW_LAYER_MAX_MEMORY` (0 = no limit)
pub const DRAW_LAYER_MAX_MEMORY: u32 = 0;
/// `LV_DRAW_SW_SHADOW_CACHE_SIZE` (the upstream test configuration uses 8).
pub const DRAW_SW_SHADOW_CACHE_SIZE: u32 = 8;

/// Runtime equivalents of the numeric `lv_conf.h` options.
#[derive(Clone, Debug)]
pub struct UiConfig {
    /// `LV_DEF_REFR_PERIOD` in ms.
    pub refr_period_ms: u32,
    /// `LV_DPI_DEF`
    pub dpi_def: i32,
    /// `LV_INDEV_DEF_LONG_PRESS_TIME`
    pub indev_long_press_time: u16,
    /// `LV_INDEV_DEF_LONG_PRESS_REP_TIME`
    pub indev_long_press_repeat_time: u16,
    /// `LV_INDEV_DEF_DOUBLE_CLICK_TIME`
    pub indev_double_click_time: u16,
    /// `LV_INDEV_DEF_SCROLL_LIMIT`
    pub indev_scroll_limit: u8,
    /// `LV_INDEV_DEF_SCROLL_THROW`
    pub indev_scroll_throw: u8,
    /// `LV_INDEV_DEF_GESTURE_LIMIT`
    pub indev_def_gesture_limit: u8,
    /// `LV_INDEV_DEF_GESTURE_MIN_VELOCITY`
    pub indev_def_gesture_min_velocity: u8,
    /// `LV_INDEV_DEF_ROTARY_SENSITIVITY`
    pub indev_def_rotary_sensitivity: i32,
    /// `LV_INV_BUF_SIZE`
    pub inv_buf_size: usize,
    /// `LV_DRAW_LAYER_SIMPLE_BUF_SIZE`
    pub draw_layer_simple_buf_size: u32,
    /// `LV_DRAW_LAYER_MAX_MEMORY`
    pub draw_layer_max_memory: u32,
    /// `LV_GRADIENT_MAX_STOPS` (informational; the array size is compile-time).
    pub gradient_max_stops: u8,
    /// `LV_DRAW_SW_SHADOW_CACHE_SIZE`
    pub draw_sw_shadow_cache_size: u32,
    /// `style_custom_table_size`: initial size of the custom property flag table.
    pub style_custom_table_size: u32,
    /// `LV_FONT_DEFAULT`: the font returned for an unset `text_font`.
    pub font_default: Option<FontRef>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            refr_period_ms: DEF_REFR_PERIOD,
            dpi_def: DPI_DEF,
            indev_long_press_time: INDEV_DEF_LONG_PRESS_TIME,
            indev_long_press_repeat_time: INDEV_DEF_LONG_PRESS_REP_TIME,
            indev_double_click_time: INDEV_DEF_DOUBLE_CLICK_TIME,
            indev_scroll_limit: INDEV_DEF_SCROLL_LIMIT,
            indev_scroll_throw: INDEV_DEF_SCROLL_THROW,
            indev_def_gesture_limit: INDEV_DEF_GESTURE_LIMIT,
            indev_def_gesture_min_velocity: INDEV_DEF_GESTURE_MIN_VELOCITY,
            indev_def_rotary_sensitivity: INDEV_DEF_ROTARY_SENSITIVITY,
            inv_buf_size: INV_BUF_SIZE,
            draw_layer_simple_buf_size: DRAW_LAYER_SIMPLE_BUF_SIZE,
            draw_layer_max_memory: DRAW_LAYER_MAX_MEMORY,
            gradient_max_stops: crate::draw::grad::GRADIENT_MAX_STOPS as u8,
            draw_sw_shadow_cache_size: DRAW_SW_SHADOW_CACHE_SIZE,
            style_custom_table_size: 0,
            font_default: None,
        }
    }
}

/// Work deferred to the next `timer_handler()` (`lv_async_call`,
/// `lv_obj_delete_async`).
pub enum AsyncJob {
    /// `lv_obj_delete_async()`
    DeleteObj(Obj),
    /// `lv_async_call()`
    Call(Rc<dyn Fn(&mut Ui)>),
}

/// `lv_global_t`: the LVGL context.
///
/// Every upstream `lv_ll_t` becomes an [`Arena`] (stable handles) plus an
/// `*_order` list that mirrors the list order: `lv_ll_ins_head` is
/// `order.insert(0, id)`, iteration / `*_get_next()` / restart-from-head
/// walk `order`, so execution order stays identical to C even after
/// deletions reuse arena slots.
pub struct Ui {
    config: UiConfig,
    /// All objects.
    pub objs: Arena<ObjData>,
    /// `disp_ll`
    pub displays: Arena<DisplayData>,
    /// Order of `disp_ll` (head first; `lv_display_create` is `ins_head`).
    pub display_order: Vec<DisplayId>,
    /// `disp_default`
    pub disp_default: Option<DisplayId>,
    /// `disp_refresh`
    pub disp_refresh: Option<DisplayId>,
    /// `indev_ll`
    pub indevs: Arena<IndevData>,
    /// Order of `indev_ll` (head first; `lv_indev_create` is `ins_head`).
    pub indev_order: Vec<IndevId>,
    /// `indev_active`
    pub indev_active: Option<IndevId>,
    /// `indev_obj_active`
    pub indev_obj_active: Option<Obj>,
    /// `group_ll`
    pub groups: Arena<GroupData>,
    /// Order of `group_ll` (head first; `lv_group_create` is `ins_head`).
    pub group_order: Vec<GroupId>,
    /// `group_default`
    pub group_default: Option<GroupId>,
    /// Themes (`lv_theme_create()`).
    pub themes: Arena<Theme>,
    /// `timer_state`
    pub timers: TimerState,
    /// `anim_state`
    pub anims: AnimState,
    /// `tick_state`
    pub tick: TickState,
    /// Shared styles (`lv_style_t` the application keeps and shares).
    pub styles: Arena<Style>,
    /// Event descriptors of every event list.
    pub event_dscs: Arena<EventDsc>,
    /// `event_header`: the stack of in-flight events (`lv_event_t.prev` chain).
    pub event_stack: Vec<EventFrame>,
    /// `event_last_register_id` (starts at `LV_EVENT_LAST`).
    pub event_last_register_id: u32,
    /// `style_refresh`
    pub style_refresh: bool,
    /// `style_trans_ll`: running style transitions (`trans_t` nodes), addressed
    /// by [`StyleTransId`] from `AnimTarget::StyleTrans`.
    pub style_trans: Arena<StyleTrans>,
    /// Order of `style_trans_ll` (head first; `ins_head`).
    pub style_trans_order: Vec<StyleTransId>,
    /// `style_custom_prop_flag_lookup_table`
    pub style_custom_prop_flags: Vec<u8>,
    /// `style_last_custom_prop_id`
    pub style_last_custom_prop_id: u32,
    /// `blur_obj_cnt`
    pub blur_obj_cnt: u32,
    /// `layout_list`
    pub layouts: Vec<LayoutDsc>,
    /// `layout_update_mutex`
    pub layout_update_mutex: bool,
    /// `draw_info` (layers, tasks, dispatch state).
    pub draw: DrawCtx,
    /// `draw_info.unit_head`: registered draw units in order. Kept beside
    /// [`Ui::draw`] (not inside it) so `unit.dispatch(&mut ui.draw, layer)`
    /// borrows disjoint fields.
    pub draw_units: Vec<Box<dyn DrawUnit>>,
    /// `math_rand_seed`
    pub rand: Rand,
    /// Pending async jobs.
    pub async_queue: Vec<AsyncJob>,
    /// Application state reachable from callbacks (`lv_global_t::user_data`).
    pub app: Option<Box<dyn Any>>,
    /// `inited`
    pub inited: bool,
    /// `deinit_in_progress`
    pub deinit_in_progress: bool,
}

impl Ui {
    /// `lv_init()`: build an empty context. Displays, input devices and the
    /// software draw unit are added by the caller / the renderer crate.
    pub fn new(config: UiConfig) -> Self {
        let mut ui = Self {
            objs: Arena::new(),
            displays: Arena::new(),
            display_order: Vec::new(),
            disp_default: None,
            disp_refresh: None,
            indevs: Arena::new(),
            indev_order: Vec::new(),
            indev_active: None,
            indev_obj_active: None,
            groups: Arena::new(),
            group_order: Vec::new(),
            group_default: None,
            themes: Arena::new(),
            timers: TimerState::new(),
            anims: AnimState::new(),
            tick: TickState::default(),
            styles: Arena::new(),
            event_dscs: Arena::new(),
            event_stack: Vec::new(),
            event_last_register_id: EventCode::LAST.0 as u32,
            style_refresh: true,
            style_trans: Arena::new(),
            style_trans_order: Vec::new(),
            style_custom_prop_flags: Vec::new(),
            style_last_custom_prop_id: StyleProp::LAST_BUILT_IN.0 as u32,
            blur_obj_cnt: 0,
            layouts: crate::layouts::builtin_layouts(),
            layout_update_mutex: false,
            draw: DrawCtx::new(),
            draw_units: Vec::new(),
            rand: Rand::default(),
            async_queue: Vec::new(),
            app: None,
            inited: true,
            deinit_in_progress: false,
            config,
        };
        ui.style_custom_prop_flags
            .resize(ui.config.style_custom_table_size as usize, 0);
        ui
    }

    /// The configuration this context was created with.
    #[inline]
    pub fn config(&self) -> &UiConfig {
        &self.config
    }

    /// `lv_is_initialized()`
    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.inited
    }

    /// `lv_deinit()`: delete every display, object, timer and animation.
    pub fn deinit(&mut self) {
        todo!()
    }

    /// Store application state (`lv_global_t::user_data`).
    pub fn set_app<T: Any>(&mut self, app: T) {
        self.app = Some(Box::new(app));
    }

    /// Borrow the application state as `T`.
    pub fn app<T: Any>(&self) -> Option<&T> {
        self.app.as_deref()?.downcast_ref::<T>()
    }

    /// Mutably borrow the application state as `T`.
    pub fn app_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.app.as_deref_mut()?.downcast_mut::<T>()
    }

    /// `lv_async_call()`: run `f` from the next `timer_handler()`.
    pub fn async_call(&mut self, f: Rc<dyn Fn(&mut Ui)>) {
        todo!()
    }

    /// `lv_rand()`
    #[inline]
    pub fn rand(&mut self, min: u32, max: u32) -> u32 {
        self.rand.next(min, max)
    }

    /// `lv_rand_set_seed()`
    #[inline]
    pub fn rand_set_seed(&mut self, seed: u32) {
        self.rand.set_seed(seed);
    }

    /// `lv_style_register_prop()`: register a custom property and return its
    /// id (`>= StyleProp::NUM_BUILT_IN`), or `StyleProp::INV` on failure.
    pub fn style_register_prop(&mut self, flag: u8) -> StyleProp {
        todo!()
    }

    /// `lv_style_get_num_custom_props()`
    pub fn style_get_num_custom_props(&self) -> u32 {
        todo!()
    }

    /// `lv_style_prop_lookup_flags()` including custom properties: the same
    /// `ANY` / `INV` early returns as [`crate::misc::style::lookup_flags`],
    /// then the built-in table, then `style_custom_prop_flags`.
    pub fn style_prop_lookup_flags(&self, prop: StyleProp) -> u8 {
        todo!()
    }

    /// `lv_draw_create_unit()`: register a draw unit, returning its index
    /// (the `draw_unit_id` tasks refer to).
    pub fn draw_create_unit(&mut self, unit: Box<dyn DrawUnit>) -> u8 {
        todo!()
    }

    /// `lv_draw_get_unit_count()`
    #[inline]
    pub fn draw_unit_count(&self) -> u32 {
        self.draw_units.len() as u32
    }

    /// Resolve an object id to a live handle (`None` if deleted).
    #[inline]
    pub fn resolve(&self, id: ObjId) -> Option<Obj> {
        if self.objs.contains(id.0) {
            Some(Obj(id))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_match_lv_conf() {
        let c = UiConfig::default();
        assert_eq!(c.refr_period_ms, 33);
        assert_eq!(c.dpi_def, 130);
        assert_eq!(c.indev_long_press_time, 400);
        assert_eq!(c.indev_long_press_repeat_time, 100);
        assert_eq!(c.indev_scroll_limit, 10);
        assert_eq!(c.indev_scroll_throw, 10);
        assert_eq!(c.indev_def_gesture_limit, 50);
        assert_eq!(c.indev_def_gesture_min_velocity, 3);
        assert_eq!(c.inv_buf_size, 32);
        assert_eq!(c.draw_layer_simple_buf_size, 24576);
        assert_eq!(c.draw_layer_max_memory, 0);
        assert_eq!(c.gradient_max_stops, 2);
    }

    #[test]
    fn new_ui_is_empty_and_initialised() {
        let mut ui = Ui::new(UiConfig::default());
        assert!(ui.is_initialized());
        assert!(ui.objs.is_empty());
        assert!(ui.displays.is_empty());
        assert_eq!(ui.layouts.len(), crate::layouts::builtin_layouts().len());
        ui.set_app(42u32);
        assert_eq!(ui.app::<u32>(), Some(&42));
        *ui.app_mut::<u32>().unwrap() = 7;
        assert_eq!(ui.app::<u32>(), Some(&7));
        assert!(ui.app::<i64>().is_none());
    }

    #[test]
    fn register_ids_start_after_the_built_ins() {
        // src/lv_init.c: event_last_register_id = LV_EVENT_LAST,
        // style_last_custom_prop_id = LV_STYLE_LAST_BUILT_IN_PROP.
        let ui = Ui::new(UiConfig::default());
        assert_eq!(ui.event_last_register_id, EventCode::LAST.0 as u32);
        assert_eq!(ui.event_last_register_id, 80);
        assert_eq!(ui.style_last_custom_prop_id, StyleProp::LAST_BUILT_IN.0 as u32);
        assert_eq!(ui.style_last_custom_prop_id, 177);
        assert!(ui.event_stack.is_empty());
        assert!(ui.style_trans.is_empty());
        assert_eq!(ui.draw_unit_count(), 0);
    }
}
