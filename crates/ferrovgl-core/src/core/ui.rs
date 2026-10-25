//! Ported from lvgl v9.6.0: `src/lv_init.c`, `src/misc/lv_async.c`, `src/core/lv_global.h`
//! (and the numeric options of `lv_conf_template.h`).
//!
//! [`Ui`] is the single context that owns every object, display, input
//! device, group, timer, animation, style and event callback (PLAN.md 4.1).
//! It replaces `lv_global_t` and all static state; there is no global
//! mutable state anywhere in the crate.
//!
//! `lv_init()` is [`Ui::new`] and `lv_deinit()` is [`Ui::deinit`]; both run
//! the subsystems in upstream order. Subsystems that do not exist in the
//! Rust port (OS layer, file systems, image decoders, profiler, sysmon,
//! observer, span stack, platform drivers) are skipped at their upstream
//! position. The software renderer lives in `ferrovgl-draw-sw`, so
//! `lv_draw_sw_init()` is called by the application right after `Ui::new`
//! (`ferrovgl_draw_sw::init(&mut ui)`).
//!
//! `lv_async_call()` is ported 1:1: every call creates its own one-shot
//! timer (period 0, `repeat_count` 1, inserted at the timer list head)
//! whose user data carries the job, so async jobs interleave with the other
//! timers exactly like upstream.

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
use crate::misc::event::{EventCode, EventDsc, EventFrame, LvResult};
use crate::misc::math::Rand;
use crate::misc::style::{Style, StyleProp};
use crate::misc::tick::TickState;
use crate::misc::timer::{TimerId, TimerState};
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
/// `lv_obj_delete_async`): the `cb` + `user_data` pair of `lv_async_info_t`.
///
/// Scheduled with [`Ui::async_call_job`] / [`Ui::async_call`] and cancelled
/// with [`Ui::async_call_job_cancel`] / [`Ui::async_call_cancel`].
#[derive(Clone)]
pub enum AsyncJob {
    /// `lv_obj_delete_async()`: `lv_async_call(lv_obj_delete_async_cb, obj)`.
    DeleteObj(Obj),
    /// `lv_async_call()`
    Call(Rc<dyn Fn(&mut Ui)>),
}

impl AsyncJob {
    /// The `info->cb == async_xcb && info->user_data == user_data` match of
    /// `lv_async_call_cancel()`: a closure is its own `cb` + `user_data`,
    /// so callbacks match by `Rc` identity; object deletions match by object.
    fn matches(&self, other: &AsyncJob) -> bool {
        match (self, other) {
            (AsyncJob::DeleteObj(a), AsyncJob::DeleteObj(b)) => a == b,
            (AsyncJob::Call(a), AsyncJob::Call(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// `lv_async_info_t`: stored as the async timer's user data. Its type is
/// private, so a timer whose user data downcasts to it is an async timer
/// (upstream: `timer->timer_cb == lv_async_timer_cb`).
struct AsyncInfo {
    job: AsyncJob,
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
    /// `subject_ll` and the observers of every subject (`LV_USE_OBSERVER`).
    #[cfg(feature = "observer")]
    pub observer: crate::core::observer::ObserverState,
    /// `objid_array` (+ `objid_count`, its length): the per-class object
    /// counters of the builtin object id generator (`LV_USE_OBJ_ID_BUILTIN`).
    #[cfg(feature = "obj-id")]
    pub objid_array: Vec<crate::core::obj_id::ClassInfo>,
    /// File-scope `static` variables of widget modules (for example
    /// `ta_insert_replace` of `lv_textarea.c`), one value per type. See
    /// [`Ui::widget_static_mut`].
    pub widget_statics: Vec<(core::any::TypeId, Box<dyn Any>)>,
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
    ///
    /// The struct literal is `lv_global_init()` (zeroed state, `style_refresh`,
    /// the register id counters, `lv_rand_set_seed(0x1234ABCD)`); the
    /// subsystem initialisers then run in `lv_init()` order.
    pub fn new(config: UiConfig) -> Self {
        /*Initialize members of static variable lv_global */
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
            /*lv_subject_global_init(): an empty subject list*/
            #[cfg(feature = "observer")]
            observer: crate::core::observer::ObserverState::new(),
            #[cfg(feature = "obj-id")]
            objid_array: Vec::new(),
            widget_statics: Vec::new(),
            app: None,
            inited: false,
            deinit_in_progress: false,
            config,
        };
        ui.style_custom_prop_flags
            .resize(ui.config.style_custom_table_size as usize, 0);

        /*lv_mem_init(), lv_draw_buf_init_handlers(), lv_os_init(): nothing to do*/

        // lv_timer_core_init(): the list is empty (TimerState::new);
        // initially enable the lv_timer handling.
        ui.timer_enable(true);

        /*lv_fs_init(): no file systems. lv_layout_init(): `layouts` holds the
         *built-in flex and grid layouts (lv_flex_init / lv_grid_init)*/

        ui.anim_core_init();

        // lv_group_init(): `groups` / `group_order` start empty.
        // lv_draw_init(): `draw` (DrawCtx::new) and `draw_units` start empty;
        // lv_draw_sw_init() is run by the renderer crate.
        // lv_obj_style_init(): `style_trans` / `style_trans_order` start empty.
        // lv_refr_init(): empty upstream.
        // lv_image_decoder_init() / lv_bin_decoder_init(): no decoder registry.

        ui.inited = true;
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
    ///
    /// Runs the upstream teardown order: default display reset, input
    /// devices then displays (`lv_cleanup_devices`), themes, style
    /// transitions and the custom property table (`lv_obj_style_deinit`),
    /// draw units (`lv_draw_deinit`), groups, animations, layouts and
    /// finally the timers.
    pub fn deinit(&mut self) {
        /*Do nothing if already deinit*/
        if !self.inited {
            return; /*"lv_deinit: already deinit!"*/
        }

        if self.deinit_in_progress {
            return;
        }

        self.deinit_in_progress = true;

        // lv_display_set_default(NULL)
        self.disp_default = None;

        self.cleanup_devices();

        // lv_theme_default_deinit() / lv_theme_simple_deinit() /
        // lv_theme_mono_deinit(): free the themes.
        self.themes.clear();

        /*lv_image_decoder_deinit(): no decoder registry. lv_refr_deinit(): empty upstream.*/

        self.obj_style_deinit();
        // Shared styles, including theme-owned styles, belong to this context.
        self.styles.clear();

        /*lv_draw_sw_deinit() (mask cache) is owned by the renderer crate;
         *its units are deleted by lv_draw_deinit() below*/
        self.draw_deinit();

        // lv_group_deinit(): lv_ll_clear(group_ll_p)
        self.groups.clear();
        self.group_order.clear();

        self.anim_core_deinit();

        // lv_layout_deinit(): lv_free(layout_list_def)
        self.layouts = Vec::new();

        self.timer_core_deinit();

        #[cfg(feature = "obj-id")]
        self.objid_builtin_destroy();

        /*lv_fs_deinit(): nothing to do*/

        #[cfg(feature = "observer")]
        self.subject_global_deinit();

        /*lv_mem_deinit(): nothing to do*/

        self.inited = false;
    }

    /// `lv_cleanup_devices()`: `lv_ll_clear_custom()` of `indev_ll` with
    /// `lv_indev_delete` and of `disp_ll` with `lv_display_delete`, walking
    /// from the head and reading the next node before the cleanup call.
    fn cleanup_devices(&mut self) {
        /* cleanup indev and display */
        let mut i = self.indev_order.first().copied();
        while let Some(indev) = i {
            let i_next = next_in_order(&self.indev_order, indev);
            self.indev_delete(indev);
            i = i_next;
        }

        let mut i = self.display_order.first().copied();
        while let Some(disp) = i {
            let i_next = next_in_order(&self.display_order, disp);
            self.display_delete(disp);
            i = i_next;
        }
    }

    /// `lv_draw_deinit()`: call every unit's `delete_cb` from `unit_head`
    /// on, then forget the units. Like upstream `unit_cnt` is left as is.
    fn draw_deinit(&mut self) {
        for cur_unit in self.draw_units.iter_mut() {
            cur_unit.delete();
        }
        self.draw_units = Vec::new();
    }

    /// The file-scope `static` variable of type `T` of a widget module,
    /// created with `T::default()` (C's zero-initialized static) on first
    /// use. Like a C `static`, it lives as long as this `Ui` and is not reset
    /// by [`Ui::deinit`].
    pub fn widget_static_mut<T: Any + Default>(&mut self) -> &mut T {
        let id = core::any::TypeId::of::<T>();
        let index = match self.widget_statics.iter().position(|(t, _)| *t == id) {
            Some(index) => index,
            None => {
                self.widget_statics.push((id, Box::new(T::default())));
                self.widget_statics.len() - 1
            }
        };
        match self.widget_statics[index].1.downcast_mut::<T>() {
            Some(value) => value,
            None => unreachable!("widget static stored under a foreign TypeId"),
        }
    }

    /// The file-scope `static` variable of type `T` of a widget module, or
    /// `None` if it was never written (C: still zero-initialized).
    pub fn widget_static<T: Any>(&self) -> Option<&T> {
        let id = core::any::TypeId::of::<T>();
        self.widget_statics
            .iter()
            .find(|(t, _)| *t == id)
            .and_then(|(_, value)| value.downcast_ref::<T>())
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
    ///
    /// The upstream allocation failures (`LV_RESULT_INVALID`) cannot happen,
    /// so nothing is returned.
    pub fn async_call(&mut self, f: Rc<dyn Fn(&mut Ui)>) {
        self.async_call_job(AsyncJob::Call(f));
    }

    /// `lv_async_call()` for any [`AsyncJob`]: create a one-shot timer with
    /// period 0 that runs `job` (`lv_obj_delete_async()` passes
    /// [`AsyncJob::DeleteObj`]). Returns the timer.
    pub fn async_call_job(&mut self, job: AsyncJob) -> TimerId {
        /*Allocate an info structure*/
        let info: Rc<dyn Any> = Rc::new(AsyncInfo { job });

        /*Create a new timer*/
        let timer = self.timer_create(Rc::new(Ui::async_timer_cb), 0, Some(info));

        self.timer_set_repeat_count(timer, 1);
        timer
    }

    /// `lv_async_call_cancel()`: delete every pending async call of `f`.
    /// `LvResult::Ok` if at least one was cancelled.
    pub fn async_call_cancel(&mut self, f: &Rc<dyn Fn(&mut Ui)>) -> LvResult {
        self.async_call_job_cancel(&AsyncJob::Call(f.clone()))
    }

    /// `lv_async_call_cancel()` for any [`AsyncJob`] (callbacks match by
    /// `Rc` identity, deletions by object).
    pub fn async_call_job_cancel(&mut self, job: &AsyncJob) -> LvResult {
        let mut timer = self.timer_get_next(None);
        let mut res = LvResult::Invalid;

        while let Some(t) = timer {
            /*Find the next timer node*/
            let timer_next = self.timer_get_next(Some(t));

            /*Find async timer callback*/
            if let Some(info) = self.async_info(t) {
                /*Match user function callback and user data*/
                if info.job.matches(job) {
                    self.timer_delete(t);
                    res = LvResult::Ok;
                }
            }

            timer = timer_next;
        }

        res
    }

    /// The `lv_async_info_t` of `timer`, if it is an async timer.
    fn async_info(&self, timer: TimerId) -> Option<Rc<AsyncInfo>> {
        self.timer_get_user_data(timer)?.downcast::<AsyncInfo>().ok()
    }

    /// `lv_async_timer_cb()`
    fn async_timer_cb(&mut self, timer: TimerId) {
        /*Save the info because an lv_async_call_cancel might delete it in the callback*/
        let Some(info) = self.async_info(timer) else {
            return;
        };
        let info_save = info.job.clone();
        drop(info);
        self.timer_delete(timer);

        match info_save {
            AsyncJob::Call(cb) => cb(self),
            // lv_obj_delete_async_cb(): lv_obj_delete(obj). A handle that
            // went stale in the meantime is ignored (LV_ASSERT_OBJ upstream).
            AsyncJob::DeleteObj(obj) => {
                if self.resolve(obj.id()).is_some() {
                    self.obj_delete(obj);
                }
            }
        }
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
        crate::misc::style::register_prop(
            &mut self.style_custom_prop_flags,
            &mut self.style_last_custom_prop_id,
            flag,
        )
    }

    /// `lv_style_get_num_custom_props()`
    pub fn style_get_num_custom_props(&self) -> u32 {
        crate::misc::style::get_num_custom_props(self.style_last_custom_prop_id)
    }

    /// `lv_style_prop_lookup_flags()` including custom properties: the same
    /// `ANY` / `INV` early returns as [`crate::misc::style::lookup_flags`],
    /// then the built-in table, then `style_custom_prop_flags`.
    pub fn style_prop_lookup_flags(&self, prop: StyleProp) -> u8 {
        crate::misc::style::lookup_flags_with_custom(prop, &self.style_custom_prop_flags)
    }

    /// `lv_draw_create_unit()`: register a draw unit, returning its `idx`
    /// (the `draw_unit_id` tasks refer to).
    ///
    /// Like upstream the unit becomes the new `unit_head` (it is inserted at
    /// the front of [`Ui::draw_units`]) and `idx` is `unit_cnt` *after* the
    /// increment, so the first unit is 1.
    pub fn draw_create_unit(&mut self, unit: Box<dyn DrawUnit>) -> u8 {
        self.draw_units.insert(0, unit);
        self.draw.unit_cnt = self.draw.unit_cnt.wrapping_add(1);

        self.draw.unit_cnt as u8
    }

    /// `lv_draw_get_unit_count()`: `unit_cnt`.
    #[inline]
    pub fn draw_unit_count(&self) -> u32 {
        self.draw.unit_cnt
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

/// `lv_ll_get_next()` over an `*_order` list.
fn next_in_order<T: Copy + PartialEq>(order: &[T], cur: T) -> Option<T> {
    let pos = order.iter().position(|&x| x == cur)?;
    order.get(pos + 1).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use core::cell::RefCell;

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

    /// Records the async callbacks in the order they ran.
    type Log = Rc<RefCell<Vec<u32>>>;

    fn logger(log: &Log, v: u32) -> Rc<dyn Fn(&mut Ui)> {
        let log = log.clone();
        Rc::new(move |_ui: &mut Ui| log.borrow_mut().push(v))
    }

    /// A `Ui` whose tick is past the "lv_tick_inc() is not called" value.
    fn ui() -> Ui {
        let mut ui = Ui::new(UiConfig::default());
        ui.tick_inc(1);
        ui
    }

    #[test]
    fn lv_init_creates_the_paused_animation_timer() {
        // lv_timer_core_init() enables the handler, then lv_anim_core_init()
        // creates the (paused, the list is empty) animation timer.
        let ui = Ui::new(UiConfig::default());
        assert!(ui.timers.run);
        let anim_timer = ui.anims.timer.expect("lv_anim_core_init creates the timer");
        assert_eq!(ui.timers.order, vec![anim_timer]);
        let t = ui.timer(anim_timer).unwrap();
        assert_eq!(t.period, DEF_REFR_PERIOD);
        assert!(t.paused);
        assert!(!ui.anims.list_changed);
        assert!(!ui.anims.run_round);
        assert!(ui.style_refresh);
        assert!(!ui.deinit_in_progress);
        assert!(ui.disp_default.is_none());
    }

    /// Port of `test_async.c: test_async_call_basic_functionality`.
    #[test]
    fn async_call_basic_functionality() {
        let mut ui = ui();
        let log: Log = Rc::default();
        ui.async_call(logger(&log, 1));
        assert!(log.borrow().is_empty(), "runs from the timer handler only");
        ui.timer_handler();
        assert_eq!(*log.borrow(), vec![1]);
        /*The one-shot timer deleted itself*/
        assert_eq!(ui.timers.order, vec![ui.anims.timer.unwrap()]);
        ui.timer_handler();
        assert_eq!(*log.borrow(), vec![1]);
    }

    #[test]
    fn async_call_is_a_period_0_one_shot_timer_at_the_head() {
        let mut ui = ui();
        let log: Log = Rc::default();
        let t = ui.async_call_job(AsyncJob::Call(logger(&log, 1)));
        assert_eq!(ui.timer_get_next(None), Some(t));
        let d = ui.timer(t).unwrap();
        assert_eq!(d.period, 0);
        assert_eq!(d.repeat_count, 1);
        assert!(d.auto_delete);
    }

    #[test]
    fn async_calls_run_head_first_like_the_timer_list() {
        // lv_timer_create() inserts at the head, so the newest call runs first.
        let mut ui = ui();
        let log: Log = Rc::default();
        ui.async_call(logger(&log, 1));
        ui.async_call(logger(&log, 2));
        ui.async_call(logger(&log, 3));
        ui.timer_handler();
        assert_eq!(*log.borrow(), vec![3, 2, 1]);
    }

    /// Port of `test_async.c: test_async_call_cancel_match`.
    #[test]
    fn async_call_cancel_match() {
        let mut ui = ui();
        let log: Log = Rc::default();
        let cb = logger(&log, 1);
        let other = logger(&log, 2);
        ui.async_call(cb.clone());
        ui.async_call(other.clone());
        ui.async_call(cb.clone());
        assert_eq!(ui.async_call_cancel(&cb), LvResult::Ok);
        assert_eq!(ui.async_call_cancel(&cb), LvResult::Invalid);
        ui.timer_handler();
        assert_eq!(*log.borrow(), vec![2]);
        assert_eq!(ui.async_call_cancel(&other), LvResult::Invalid);
    }

    #[test]
    fn async_call_cancel_does_not_touch_other_timers() {
        let mut ui = ui();
        let plain = ui.timer_create_basic();
        let n = ui.timers.order.len();
        let log: Log = Rc::default();
        assert_eq!(ui.async_call_cancel(&logger(&log, 1)), LvResult::Invalid);
        assert_eq!(
            ui.async_call_job_cancel(&AsyncJob::DeleteObj(Obj(ObjId(Arena::<()>::new().insert(()))))),
            LvResult::Invalid
        );
        assert_eq!(ui.timers.order.len(), n);
        assert!(ui.timer(plain).is_some());
    }

    #[test]
    fn async_callback_can_cancel_and_schedule_others() {
        let mut ui = ui();
        let log: Log = Rc::default();
        let victim = logger(&log, 2);
        let v = victim.clone();
        let l = log.clone();
        let l2 = log.clone();
        // Runs first (newest); cancels the older call and schedules a new one.
        ui.async_call(victim);
        ui.async_call(Rc::new(move |ui: &mut Ui| {
            l.borrow_mut().push(1);
            assert_eq!(ui.async_call_cancel(&v), LvResult::Ok);
            let l3 = l2.clone();
            ui.async_call(Rc::new(move |_ui: &mut Ui| l3.borrow_mut().push(3)));
        }));
        ui.timer_handler();
        /*The call created inside the handler is due at once (period 0)*/
        assert_eq!(*log.borrow(), vec![1, 3]);
    }

    #[test]
    fn delete_obj_job_ignores_a_stale_object() {
        // lv_obj_delete_async() on an object deleted in the meantime.
        let mut ui = ui();
        let stale = Obj(ObjId(Arena::<()>::new().insert(())));
        let t = ui.async_call_job(AsyncJob::DeleteObj(stale));
        ui.timer_handler();
        assert!(ui.timer(t).is_none());
        let t = ui.async_call_job(AsyncJob::DeleteObj(stale));
        assert_eq!(
            ui.async_call_job_cancel(&AsyncJob::DeleteObj(stale)),
            LvResult::Ok
        );
        assert!(ui.timer(t).is_none());
    }

    /// Port of `test_style.c: test_custom_prop_ids`.
    #[test]
    fn custom_prop_ids() {
        let mut ui = Ui::new(UiConfig::default());
        let mut fake_flag: u8 = 0;
        let initial_custom_props = ui.style_get_num_custom_props();
        let max_props_to_register = 64;
        for i in 0..max_props_to_register {
            let prop = ui.style_register_prop(fake_flag);
            /* Should have a higher index than the last built-in prop */
            assert!(prop.0 > StyleProp::LAST_BUILT_IN.0);
            if i == 0 {
                /* Should be equal to the first expected index of a custom prop */
                assert_eq!(
                    prop.0 as u32,
                    StyleProp::NUM_BUILT_IN as u32 + initial_custom_props
                );
            }
            /*We should find our flags*/
            assert_eq!(fake_flag, ui.style_prop_lookup_flags(prop));
            if fake_flag == 0xff {
                fake_flag = 0;
            } else {
                fake_flag += 1;
            }
        }
        assert_eq!(
            initial_custom_props + max_props_to_register,
            ui.style_get_num_custom_props()
        );
        assert_eq!(ui.style_custom_prop_flags.len(), 64);
    }

    #[test]
    fn style_prop_lookup_flags_early_returns_and_built_ins() {
        let ui = Ui::new(UiConfig::default());
        assert_eq!(
            ui.style_prop_lookup_flags(StyleProp::ANY),
            crate::misc::style::STYLE_PROP_FLAG_ALL
        );
        assert_eq!(ui.style_prop_lookup_flags(StyleProp::INV), 0);
        for p in 0..StyleProp::NUM_BUILT_IN {
            assert_eq!(
                ui.style_prop_lookup_flags(StyleProp(p)),
                crate::misc::style::lookup_flags(StyleProp(p))
            );
        }
        /*Not registered yet*/
        assert_eq!(ui.style_prop_lookup_flags(StyleProp(StyleProp::NUM_BUILT_IN)), 0);
    }

    /// A draw unit that records its `delete_cb` calls.
    struct DeleteLog {
        name: &'static str,
        log: Rc<RefCell<Vec<&'static str>>>,
    }

    impl DrawUnit for DeleteLog {
        fn name(&self) -> &'static str {
            self.name
        }
        fn evaluate(&mut self, _task: &mut crate::draw::DrawTask) -> i32 {
            0
        }
        fn dispatch(&mut self, _ctx: &mut DrawCtx, _layer: crate::draw::LayerId) -> i32 {
            crate::draw::DRAW_UNIT_IDLE
        }
        fn delete(&mut self) {
            self.log.borrow_mut().push(self.name);
        }
    }

    #[test]
    fn draw_create_unit_prepends_and_counts_from_one() {
        let mut ui = Ui::new(UiConfig::default());
        let log = Rc::new(RefCell::new(Vec::new()));
        let a = ui.draw_create_unit(Box::new(DeleteLog {
            name: "A",
            log: log.clone(),
        }));
        let b = ui.draw_create_unit(Box::new(DeleteLog {
            name: "B",
            log: log.clone(),
        }));
        // new_unit->idx = ++unit_cnt
        assert_eq!((a, b), (1, 2));
        assert_eq!(ui.draw_unit_count(), 2);
        assert_eq!(ui.draw.unit_cnt, 2);
        /*new_unit->next = unit_head; unit_head = new_unit*/
        let names: Vec<_> = ui.draw_units.iter().map(|u| u.name()).collect();
        assert_eq!(names, vec!["B", "A"]);

        // lv_draw_deinit() calls delete_cb from the head on.
        ui.deinit();
        assert_eq!(*log.borrow(), vec!["B", "A"]);
        assert!(ui.draw_units.is_empty());
    }

    #[test]
    fn deinit_tears_everything_down_once() {
        let mut ui = ui();
        let log: Log = Rc::default();
        ui.async_call(logger(&log, 1));
        ui.style_register_prop(0x5);
        ui.theme_create_for_test();
        ui.deinit();
        assert!(!ui.is_initialized());
        assert!(ui.deinit_in_progress);
        assert!(ui.timers.timers.is_empty());
        assert!(ui.timers.order.is_empty());
        assert!(!ui.timers.run, "lv_timer_core_deinit disables the handler");
        assert!(ui.anims.anims.is_empty());
        assert!(ui.groups.is_empty());
        assert!(ui.themes.is_empty());
        assert!(ui.layouts.is_empty());
        assert!(ui.style_custom_prop_flags.is_empty());
        assert!(ui.style_trans.is_empty());
        /*Pending async calls are dropped with the timers*/
        assert_eq!(ui.timer_handler(), 1);
        assert!(log.borrow().is_empty());

        /*Do nothing if already deinit*/
        ui.deinit();
        assert!(!ui.is_initialized());
    }

    impl Ui {
        /// Insert a theme without `lv_theme_create()` (ported elsewhere).
        fn theme_create_for_test(&mut self) {
            self.themes.insert(Theme::new());
        }
    }

    #[test]
    fn layer_go_to_xy_is_range_checked() {
        use crate::misc::area::Area;
        use crate::misc::color::ColorFormat;
        let mut ui = Ui::new(UiConfig::default());
        let layer = ui
            .draw
            .layer_create(None, ColorFormat::Argb8888, &Area::new(0, 0, 9, 4));
        assert_eq!(ui.draw.layer_go_to_xy(layer, 0, 0), None, "no buffer yet");
        assert!(ui.draw.layer_alloc_buf(layer));
        let stride = ui
            .draw
            .layer(layer)
            .unwrap()
            .draw_buf
            .as_ref()
            .unwrap()
            .header
            .stride as usize;
        assert_eq!(ui.draw.layer_go_to_xy(layer, 0, 0), Some(0));
        assert_eq!(ui.draw.layer_go_to_xy(layer, 3, 2), Some(2 * stride + 12));
        assert_eq!(ui.draw.layer_go_to_xy(layer, 9, 4), Some(4 * stride + 36));
        /*lv_draw_buf_goto_xy(): "coordinates out of range"*/
        assert_eq!(ui.draw.layer_go_to_xy(layer, 10, 0), None);
        assert_eq!(ui.draw.layer_go_to_xy(layer, 0, 5), None);
        assert_eq!(ui.draw.layer_go_to_xy(layer, -1, 0), None);
    }
}
