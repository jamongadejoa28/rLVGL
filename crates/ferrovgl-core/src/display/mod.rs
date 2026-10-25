//! Ported from lvgl v9.6.0: `src/display/lv_display.c` (with
//! `include/lvgl/display/lv_display.h` and `src/display/lv_display_private.h`).
//!
//! The configuration follows the parity build (`tests/configs/full.defconfig`
//! with `depth_32`): `LV_COLOR_DEPTH 32` (`LV_COLOR_FORMAT_DEFAULT` = XRGB8888),
//! `LV_DRAW_SW_DRAW_UNIT_CNT 1`, `LV_DRAW_TRANSFORM_USE_MATRIX 0`,
//! `LV_USE_OBJ_NAME 1`, `LV_USE_THEME_DEFAULT 1`, `LV_USE_EXT_DATA 1`. The
//! sysmon overlays (`LV_USE_PERF_MONITOR` / `LV_USE_MEM_MONITOR`, hidden by the
//! upstream test runner anyway) are not part of the core crate.
//!
//! `lv_theme_default_init()` / `lv_theme_default_get()` live in the widgets
//! crate, so `lv_display_create()`'s `LV_USE_THEME_DEFAULT` block is a hook
//! ([`DisplayThemeInit`]) passed to [`Ui::display_create_ex`]; plain
//! [`Ui::display_create`] behaves like a build without a default theme.
//!
//! `lv_display_set_draw_buf_handlers()` is intentionally not ported:
//! [`DrawBuf`] has no `handlers` (`lv_draw_buf_handlers_t` is omitted, see
//! `draw/draw_buf.rs`), so there is nothing to assign.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;
use core::cell::Cell;

use crate::core::Ui;
use crate::core::obj::{Obj, ObjId, ObjTreeWalkRes, ScrollbarMode};
use crate::draw::LayerId;
use crate::draw::draw_buf::{BufStorage, DrawBuf, STRIDE_AUTO};
use crate::misc::anim::{Anim, AnimCallback, AnimExec, AnimId, AnimTarget};
use crate::misc::area::{Area, Point};
use crate::misc::arena::Key;
use crate::misc::check_arg_failed;
use crate::misc::color::{ColorFormat, OPA_COVER, OPA_TRANSP};
use crate::misc::event::{
    Event, EventCallback, EventCode, EventDsc, EventDscId, EventFilter, EventList, EventParam, EventTarget,
    LvResult,
};
use crate::misc::style::{StyleProp, StyleSelector, StyleValue};
use crate::misc::timer::{TimerCallback, TimerId};
use crate::theme::ThemeId;

/// `LV_INV_BUF_SIZE`
pub const INV_BUF_SIZE: usize = 32;

/// `LV_COLOR_FORMAT_DEFAULT` of the parity configuration (`LV_COLOR_DEPTH 32`).
pub const COLOR_FORMAT_DEFAULT: ColorFormat = ColorFormat::Xrgb8888;

/// `LV_COLOR_DEPTH`
const COLOR_DEPTH: u32 = 32;

/// `DEFAULT_TILE_CNT`: `LV_DRAW_SW_DRAW_UNIT_CNT` (1) since tiled rendering
/// is not disabled.
const DEFAULT_TILE_CNT: u8 = 1;

/// `LV_DRAW_TRANSFORM_USE_MATRIX`
const DRAW_TRANSFORM_USE_MATRIX: bool = false;

/// Identity of `set_x_anim()` (compared like a C function pointer by
/// `lv_anim_delete`).
const ANIM_TAG_SET_X: u32 = 0x4C56_4458;
/// Identity of `set_y_anim()`.
const ANIM_TAG_SET_Y: u32 = 0x4C56_4459;
/// Identity of `opa_scale_anim()`.
const ANIM_TAG_OPA_SCALE: u32 = 0x4C56_444F;

/// Handle of a display (`lv_display_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DisplayId(pub Key);

/// The `LV_USE_THEME_DEFAULT` block of `lv_display_create()`:
///
/// ```c
/// if(lv_theme_default_is_inited() == false)
///     disp->theme = lv_theme_default_init(disp, lv_palette_main(LV_PALETTE_BLUE),
///                       lv_palette_main(LV_PALETTE_RED), LV_THEME_DEFAULT_DARK, LV_FONT_DEFAULT);
/// else
///     disp->theme = lv_theme_default_get();
/// ```
///
/// Called by [`Ui::display_create_ex`] with the new display (already the
/// temporary default display) before its screens are created; the returned
/// theme becomes `disp->theme`.
pub type DisplayThemeInit = fn(&mut Ui, DisplayId) -> Option<ThemeId>;

/// `void (* free_cb)(void * data)` of `lv_ext_data_t`.
pub type ExtDataFreeCb = fn(Option<Box<dyn Any>>);

/// `lv_ext_data_t` (`LV_USE_EXT_DATA`)
pub struct ExtData {
    /// `data`
    pub data: Option<Box<dyn Any>>,
    /// `free_cb`: called with `data` by `lv_display_delete()`.
    pub free_cb: Option<ExtDataFreeCb>,
}

/// `lv_display_rotation_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Rotation {
    /// `LV_DISPLAY_ROTATION_0`
    #[default]
    R0 = 0,
    /// `LV_DISPLAY_ROTATION_90`
    R90 = 1,
    /// `LV_DISPLAY_ROTATION_180`
    R180 = 2,
    /// `LV_DISPLAY_ROTATION_270`
    R270 = 3,
}

/// `lv_display_render_mode_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RenderMode {
    /// `LV_DISPLAY_RENDER_MODE_PARTIAL`: render only the invalidated areas in
    /// a small buffer.
    #[default]
    Partial = 0,
    /// `LV_DISPLAY_RENDER_MODE_DIRECT`: render directly into a screen sized buffer.
    Direct = 1,
    /// `LV_DISPLAY_RENDER_MODE_FULL`: always redraw the whole screen.
    Full = 2,
}

/// `lv_screen_load_anim_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ScreenLoadAnim {
    /// `LV_SCREEN_LOAD_ANIM_NONE`
    #[default]
    None = 0,
    /// `LV_SCREEN_LOAD_ANIM_OVER_LEFT`
    OverLeft,
    /// `LV_SCREEN_LOAD_ANIM_OVER_RIGHT`
    OverRight,
    /// `LV_SCREEN_LOAD_ANIM_OVER_TOP`
    OverTop,
    /// `LV_SCREEN_LOAD_ANIM_OVER_BOTTOM`
    OverBottom,
    /// `LV_SCREEN_LOAD_ANIM_MOVE_LEFT`
    MoveLeft,
    /// `LV_SCREEN_LOAD_ANIM_MOVE_RIGHT`
    MoveRight,
    /// `LV_SCREEN_LOAD_ANIM_MOVE_TOP`
    MoveTop,
    /// `LV_SCREEN_LOAD_ANIM_MOVE_BOTTOM`
    MoveBottom,
    /// `LV_SCREEN_LOAD_ANIM_FADE_IN` (also `LV_SCREEN_LOAD_ANIM_FADE_ON`)
    FadeIn,
    /// `LV_SCREEN_LOAD_ANIM_FADE_OUT`
    FadeOut,
    /// `LV_SCREEN_LOAD_ANIM_OUT_LEFT`
    OutLeft,
    /// `LV_SCREEN_LOAD_ANIM_OUT_RIGHT`
    OutRight,
    /// `LV_SCREEN_LOAD_ANIM_OUT_TOP`
    OutTop,
    /// `LV_SCREEN_LOAD_ANIM_OUT_BOTTOM`
    OutBottom,
}

impl ScreenLoadAnim {
    /// `LV_SCREEN_LOAD_ANIM_FADE_ON`
    pub const FADE_ON: ScreenLoadAnim = ScreenLoadAnim::FadeIn;
}

/// Result of [`DisplayDriver::flush`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FlushResult {
    /// The buffer was consumed (`lv_display_flush_ready()` already called).
    Done,
    /// The transfer is in progress; the driver calls
    /// [`Ui::display_flush_ready`] later.
    Pending,
}

/// The display backend (`flush_cb`, `flush_wait_cb`, `sync_cb`, ...).
pub trait DisplayDriver {
    /// `flush_cb`: copy `px_map` (in the display's colour format) to `area`.
    /// `px_map` is mutable like the C `uint8_t * px_map` so drivers can
    /// swap / invert it in place (`lv_draw_sw_rgb565_swap`, I1 inversion)
    /// before pushing it.
    fn flush(&mut self, disp: DisplayId, area: &Area, px_map: &mut [u8]) -> FlushResult;
    /// `flush_wait_cb`: block until a pending flush is done.
    fn flush_wait(&mut self, _disp: DisplayId) {}
    /// Whether the backend provides `sync_cb`. Override this together with
    /// [`Self::sync`] to enable driver-managed synchronization; otherwise
    /// direct double buffering uses the internal copy path.
    fn has_sync(&self) -> bool {
        false
    }
    /// `sync_cb`: copy `area` from the active to the inactive buffer.
    fn sync(&mut self, _disp: DisplayId, _area: &Area) {}
    /// `sync_wait_cb`
    fn sync_wait(&mut self, _disp: DisplayId) {}
    /// Display events such as `LV_EVENT_RESOLUTION_CHANGED`.
    ///
    /// Called by [`Ui::display_send_event`] after the display's event
    /// callbacks ran (and only if the display still exists), in place of
    /// the driver registering a C event callback on the display.
    fn on_event(&mut self, _code: EventCode) {}
}

/// `lv_display_t`
pub struct DisplayData {
    /// `hor_res`
    pub hor_res: i32,
    /// `ver_res`
    pub ver_res: i32,
    /// `physical_hor_res`
    pub physical_hor_res: i32,
    /// `physical_ver_res`
    pub physical_ver_res: i32,
    /// `offset_x`
    pub offset_x: i32,
    /// `offset_y`
    pub offset_y: i32,
    /// `dpi`
    pub dpi: u32,
    /// `buf_1`
    pub buf_1: Option<DrawBuf>,
    /// `buf_2`
    pub buf_2: Option<DrawBuf>,
    /// `buf_3`
    pub buf_3: Option<DrawBuf>,
    /// `buf_act`: 0 = none, 1..=3 = `buf_n`.
    pub buf_act: u8,
    /// `flush_cb`, `flush_wait_cb`, `sync_cb`, `sync_wait_cb`
    pub driver: Option<Box<dyn DisplayDriver>>,
    /// `flushing`
    pub flushing: bool,
    /// `flushing_last`
    pub flushing_last: bool,
    /// `last_area`
    pub last_area: bool,
    /// `last_part`
    pub last_part: bool,
    /// `syncing`
    pub syncing: bool,
    /// `syncing_last`
    pub syncing_last: bool,
    /// `sync_areas`
    pub sync_areas: Vec<Area>,
    /// `render_mode`
    pub render_mode: RenderMode,
    /// `antialiasing`
    pub antialiasing: bool,
    /// `tile_cnt`
    pub tile_cnt: u8,
    /// `stride_is_auto`
    pub stride_is_auto: bool,
    /// `rendering_in_progress`
    pub rendering_in_progress: bool,
    /// `color_format`
    pub color_format: ColorFormat,
    /// `inv_areas[LV_INV_BUF_SIZE]`: like the C array it always holds
    /// `inv_buf_size` entries (zeroed by `display_create` and on a resolution
    /// change); only the first `inv_p` are valid.
    pub inv_areas: Vec<Area>,
    /// `inv_area_joined[LV_INV_BUF_SIZE]`: always `inv_buf_size` entries,
    /// like `inv_areas`.
    pub inv_area_joined: Vec<bool>,
    /// `inv_p`: number of valid entries of `inv_areas`.
    pub inv_p: u32,
    /// `inv_en_cnt`
    pub inv_en_cnt: i32,
    /// `layer_head`
    pub layer_head: Option<LayerId>,
    /// `screens` (`screen_cnt` is its length), in creation order.
    pub screens: Vec<ObjId>,
    /// `sys_layer`
    pub sys_layer: Option<ObjId>,
    /// `top_layer`
    pub top_layer: Option<ObjId>,
    /// `act_scr`
    pub act_scr: Option<ObjId>,
    /// `bottom_layer`
    pub bottom_layer: Option<ObjId>,
    /// `prev_scr`
    pub prev_scr: Option<ObjId>,
    /// `scr_to_load`
    pub scr_to_load: Option<ObjId>,
    /// `draw_prev_over_act`
    pub draw_prev_over_act: bool,
    /// `del_prev`
    pub del_prev: bool,
    /// `driver_data`
    pub driver_data: Option<Box<dyn Any>>,
    /// `user_data`
    pub user_data: Option<Rc<dyn Any>>,
    /// `event_list`
    pub event_list: EventList,
    /// `rotation`
    pub rotation: Rotation,
    /// `matrix_rotation`
    pub matrix_rotation: bool,
    /// `theme`
    pub theme: Option<ThemeId>,
    /// `refr_timer`
    pub refr_timer: Option<TimerId>,
    /// `last_activity_time`
    pub last_activity_time: u32,
    /// `refreshed_area`
    pub refreshed_area: Area,
    /// `vsync_count`
    pub vsync_count: u32,
    /// `ext_data` (`LV_USE_EXT_DATA`)
    pub ext_data: ExtData,
}

impl DisplayData {
    /// The `lv_memzero()`-ed display of `lv_display_create()` with the fields
    /// it sets before creating the draw layer.
    pub(crate) fn new(hor_res: i32, ver_res: i32, dpi: i32, inv_buf_size: usize) -> Self {
        Self {
            hor_res,
            ver_res,
            physical_hor_res: -1,
            physical_ver_res: -1,
            offset_x: 0,
            offset_y: 0,
            dpi: dpi as u32,
            buf_1: None,
            buf_2: None,
            buf_3: None,
            buf_act: 0,
            driver: None,
            flushing: false,
            flushing_last: false,
            last_area: false,
            last_part: false,
            syncing: false,
            syncing_last: false,
            sync_areas: Vec::new(),
            render_mode: RenderMode::Partial,
            antialiasing: COLOR_DEPTH > 8,
            tile_cnt: DEFAULT_TILE_CNT,
            stride_is_auto: false,
            rendering_in_progress: false,
            color_format: COLOR_FORMAT_DEFAULT,
            inv_areas: vec![Area::default(); inv_buf_size],
            inv_area_joined: vec![false; inv_buf_size],
            inv_p: 0,
            inv_en_cnt: 0,
            layer_head: None,
            screens: Vec::new(),
            sys_layer: None,
            top_layer: None,
            act_scr: None,
            bottom_layer: None,
            prev_scr: None,
            scr_to_load: None,
            draw_prev_over_act: false,
            del_prev: false,
            driver_data: None,
            user_data: None,
            event_list: EventList::new(),
            rotation: Rotation::R0,
            matrix_rotation: false,
            theme: None,
            refr_timer: None,
            last_activity_time: 0,
            refreshed_area: Area::default(),
            vsync_count: 0,
            ext_data: ExtData {
                free_cb: None,
                data: None,
            },
        }
    }

    /// `disp->buf_act` as a reference.
    fn buf_act_ref(&self) -> Option<&DrawBuf> {
        match self.buf_act {
            1 => self.buf_1.as_ref(),
            2 => self.buf_2.as_ref(),
            3 => self.buf_3.as_ref(),
            _ => None,
        }
    }
}

/// `lv_load_screen_result_t`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum LoadScreenResult {
    /// `LV_LOAD_SCREEN_RESULT_OK`
    Ok,
    /// `LV_LOAD_SCREEN_RESULT_OLD_SCREEN_DELETED`
    OldScreenDeleted,
    /// `LV_LOAD_SCREEN_RESULT_NEW_SCREEN_DELETED`
    NewScreenDeleted,
    /// `LV_LOAD_SCREEN_RESULT_BOTH_SCREENS_DELETED`
    BothScreensDeleted,
    /// `LV_LOAD_SCREEN_RESULT_DISPLAY_DELETED`
    DisplayDeleted,
}

/// Byte address of the start of a buffer (`void * buf`), for the
/// `LV_DRAW_BUF_ALIGN` check of `lv_display_set_buffers_with_stride()`.
fn storage_addr(storage: &BufStorage) -> usize {
    match storage {
        BufStorage::Owned(b) => b.as_ptr() as usize,
        BufStorage::Static(b) => b.as_ptr() as usize,
        BufStorage::StaticMut(b) => b.as_ptr() as usize,
        BufStorage::Shared(b) => b.as_ptr() as usize,
    }
}

/// `dsc->user_data == user_data` (pointer identity, `NULL == NULL`).
fn user_data_eq(a: Option<&Rc<dyn Any>>, b: Option<&Rc<dyn Any>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

impl Ui {
    /// `lv_display_create()` without a default theme (as if
    /// `LV_USE_THEME_DEFAULT` were 0: `disp->theme` stays `NULL`).
    ///
    /// Signature deviation: upstream returns `NULL` for a non-positive
    /// resolution, but this handle cannot be `None`, so after the failed
    /// argument check the resolution is clamped to 1 and a display is
    /// created. [`Ui::display_create_ex`] is the 1:1 port that returns `None`.
    pub fn display_create(&mut self, hor_res: i32, ver_res: i32) -> DisplayId {
        let mut hor_res = hor_res;
        let mut ver_res = ver_res;
        if hor_res <= 0 {
            check_arg_failed("display_create: hor_res > 0");
            hor_res = 1;
        }
        if ver_res <= 0 {
            check_arg_failed("display_create: ver_res > 0");
            ver_res = 1;
        }
        self.display_create_checked(hor_res, ver_res, None)
    }

    /// `lv_display_create()`: returns `None` (and creates nothing) for a
    /// non-positive resolution. `theme_init` is the `LV_USE_THEME_DEFAULT`
    /// block (the widgets crate's default-theme init-or-get); it runs right
    /// after the refresh timer is created and before the four screens are,
    /// so the active screen gets the theme's styles at creation. `None`
    /// leaves `disp->theme` `NULL`.
    pub fn display_create_ex(
        &mut self,
        hor_res: i32,
        ver_res: i32,
        theme_init: Option<DisplayThemeInit>,
    ) -> Option<DisplayId> {
        if hor_res <= 0 {
            check_arg_failed("display_create: hor_res > 0");
            return None;
        }
        if ver_res <= 0 {
            check_arg_failed("display_create: ver_res > 0");
            return None;
        }
        Some(self.display_create_checked(hor_res, ver_res, theme_init))
    }

    /// The body of `lv_display_create()` after `LV_CHECK_ARG`.
    fn display_create_checked(
        &mut self,
        hor_res: i32,
        ver_res: i32,
        theme_init: Option<DisplayThemeInit>,
    ) -> DisplayId {
        let dpi_def = self.config().dpi_def;
        let inv_buf_size = self.config().inv_buf_size;
        let data = DisplayData::new(hor_res, ver_res, dpi_def, inv_buf_size);
        // lv_ll_ins_head(disp_ll_p)
        let disp = DisplayId(self.displays.insert(data));
        self.display_order.insert(0, disp);

        let disp_area = Area::new(0, 0, hor_res - 1, ver_res - 1);

        /* TODO: (v10) make `lv_draw_layer_init` take in a display pointer
         * instead of assuming it needs to be bound to the refreshing display*/
        let original_refr_disp = self.refr_get_disp_refreshing();
        self.refr_set_disp_refreshing(Some(disp));
        let layer = self.draw_layer_create(None, COLOR_FORMAT_DEFAULT, &disp_area);
        if let Some(d) = self.display_mut(disp) {
            d.layer_head = Some(layer);
        }
        self.refr_set_disp_refreshing(original_refr_disp);

        let tick = self.tick_get();
        if let Some(d) = self.display_mut(disp) {
            d.inv_en_cnt = 1;
            d.last_activity_time = tick;
        }

        let disp_def_tmp = self.disp_default;
        /*Temporarily change the default screen to create the default screens on the new display*/
        self.disp_default = Some(disp);

        /*Create a refresh timer*/
        let refr_cb: TimerCallback = Rc::new(|ui: &mut Ui, t: TimerId| ui.display_refr_timer(Some(t)));
        let period = self.config().refr_period_ms;
        let user_data: Rc<dyn Any> = Rc::new(disp);
        let timer = self.timer_create(refr_cb, period, Some(user_data));
        if let Some(d) = self.display_mut(disp) {
            d.refr_timer = Some(timer);
        }

        /* LV_USE_THEME_DEFAULT: `lv_theme_default_is_inited()` ?
         * `lv_theme_default_get()` : `lv_theme_default_init(disp, ...)`,
         * provided by the widgets crate through `theme_init`. */
        if let Some(init) = theme_init {
            let theme = init(self, disp);
            if let Some(d) = self.display_mut(disp) {
                d.theme = theme;
            }
        }

        /*Create bottom layer on the display*/
        let bottom_layer = self.obj_create(None);
        if let Some(d) = self.display_mut(disp) {
            d.bottom_layer = Some(bottom_layer.0);
        }
        /*Create a default screen on the display*/
        let act_scr = self.obj_create(None);
        if let Some(d) = self.display_mut(disp) {
            d.act_scr = Some(act_scr.0);
        }
        /*Create top layer on the display*/
        let top_layer = self.obj_create(None);
        if let Some(d) = self.display_mut(disp) {
            d.top_layer = Some(top_layer.0);
        }
        /*Create sys layer on the display*/
        let sys_layer = self.obj_create(None);
        if let Some(d) = self.display_mut(disp) {
            d.sys_layer = Some(sys_layer.0);
        }
        self.obj_remove_style_all(bottom_layer);
        self.obj_remove_style_all(top_layer);
        self.obj_remove_style_all(sys_layer);
        self.obj_set_clickable(top_layer, false);
        self.obj_set_clickable(sys_layer, false);

        let cf = self
            .display(disp)
            .map_or(COLOR_FORMAT_DEFAULT, |d| d.color_format);
        if cf.has_alpha() {
            self.obj_remove_local_style_prop(bottom_layer, StyleProp::BG_OPA, StyleSelector::DEFAULT);
        } else {
            self.obj_set_local_style_prop(
                bottom_layer,
                StyleProp::BG_OPA,
                StyleValue::Num(255),
                StyleSelector::DEFAULT,
            );
        }

        self.obj_set_scrollbar_mode(bottom_layer, ScrollbarMode::Off);
        self.obj_set_scrollbar_mode(top_layer, ScrollbarMode::Off);
        self.obj_set_scrollbar_mode(sys_layer, ScrollbarMode::Off);

        let _ = self.obj_invalidate(act_scr);

        /*Revert the default display*/
        self.disp_default = disp_def_tmp;
        /*Initialize the default display*/
        if self.disp_default.is_none() {
            self.disp_default = Some(disp);
        }

        self.display_add_event_cb(disp, Rc::new(disp_event_cb), EventCode::REFR_REQUEST, None);

        /*Be sure the screen will be refreshed immediately on start up*/
        if let Some(t) = self.display(disp).and_then(|d| d.refr_timer) {
            self.timer_ready(t);
        }

        disp
    }

    /// `lv_display_delete()`
    pub fn display_delete(&mut self, disp: DisplayId) {
        if !self.displays.contains(disp.0) {
            return;
        }
        let was_default = Some(disp) == self.display_get_default();
        let was_refr = Some(disp) == self.refr_get_disp_refreshing();

        self.display_send_event(disp, EventCode::DELETE, EventParam::None);
        self.event_mark_deleted(EventTarget::Display(disp));
        self.display_with_event_list(disp, |ui, list| ui.event_remove_all(list));

        /*Detach the input devices*/
        let mut indev = self.indev_get_next(None);
        while let Some(i) = indev {
            if self.indev_get_display(i) == Some(disp) {
                self.indev_set_display(i, None);
            }
            indev = self.indev_get_next(Some(i));
        }

        /* Delete screen and other obj */
        if let Some(sys) = self.display(disp).and_then(|d| d.sys_layer) {
            if let Some(o) = self.resolve(sys) {
                self.obj_delete(o);
            }
            if let Some(d) = self.display_mut(disp) {
                d.sys_layer = None;
            }
        }
        if let Some(top) = self.display(disp).and_then(|d| d.top_layer) {
            if let Some(o) = self.resolve(top) {
                self.obj_delete(o);
            }
            if let Some(d) = self.display_mut(disp) {
                d.top_layer = None;
            }
        }

        if let Some(bottom) = self.display(disp).and_then(|d| d.bottom_layer) {
            if let Some(o) = self.resolve(bottom) {
                self.obj_delete(o);
            }
            if let Some(d) = self.display_mut(disp) {
                d.bottom_layer = None;
            }
        }

        if let Some(d) = self.display_mut(disp) {
            d.act_scr = None;
        }

        /*Delete the screens*/
        while let Some(scr) = self.display(disp).and_then(|d| d.screens.first().copied()) {
            if let Some(o) = self.resolve(scr) {
                self.obj_delete(o);
            }
            // `lv_obj_delete()` removes the screen from `screens`; drop a
            // stale entry so the loop always terminates.
            if let Some(d) = self.display_mut(disp)
                && d.screens.first() == Some(&scr)
            {
                d.screens.remove(0);
            }
        }

        let (refr_timer, layer_head) = match self.display_mut(disp) {
            Some(d) => {
                d.sync_areas.clear();
                (d.refr_timer, d.layer_head)
            }
            None => (None, None),
        };
        self.display_order.retain(|&d| d != disp);
        if let Some(t) = refr_timer {
            self.timer_delete(t);
        }

        if let Some(l) = layer_head {
            self.draw_layer_delete(l);
        }

        /* LV_USE_EXT_DATA */
        self.display_ext_data_free(disp);

        self.displays.remove(disp.0);

        if was_default {
            self.disp_default = self.display_order.first().copied();
        }

        if was_refr {
            self.refr_set_disp_refreshing(None);
        }
    }

    /// `lv_display_set_default()`
    pub fn display_set_default(&mut self, disp: DisplayId) {
        self.disp_default = Some(disp);
    }

    /// `lv_display_get_default()`
    pub fn display_get_default(&self) -> Option<DisplayId> {
        self.disp_default
    }

    /// `lv_display_get_next()`: `None` returns the head of the display list.
    pub fn display_get_next(&self, disp: Option<DisplayId>) -> Option<DisplayId> {
        match disp {
            None => self.display_order.first().copied(),
            Some(d) => {
                let pos = self.display_order.iter().position(|&x| x == d)?;
                self.display_order.get(pos + 1).copied()
            }
        }
    }

    /*---------------------
     * RESOLUTION
     *--------------------*/

    /// `lv_display_set_resolution()`
    pub fn display_set_resolution(&mut self, disp: DisplayId, hor_res: i32, ver_res: i32) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_resolution: disp != NULL") else {
            return;
        };
        if hor_res <= 0 {
            check_arg_failed("display_set_resolution: hor_res > 0");
            return;
        }
        if ver_res <= 0 {
            check_arg_failed("display_set_resolution: ver_res > 0");
            return;
        }

        if d.hor_res == hor_res && d.ver_res == ver_res {
            return;
        }

        d.hor_res = hor_res;
        d.ver_res = ver_res;

        self.update_resolution(disp);
    }

    /// `lv_display_set_physical_resolution()`
    pub fn display_set_physical_resolution(&mut self, disp: DisplayId, hor_res: i32, ver_res: i32) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_physical_resolution: disp != NULL")
        else {
            return;
        };
        if hor_res <= 0 {
            check_arg_failed("display_set_physical_resolution: hor_res > 0");
            return;
        }
        if ver_res <= 0 {
            check_arg_failed("display_set_physical_resolution: ver_res > 0");
            return;
        }

        d.physical_hor_res = hor_res;
        d.physical_ver_res = ver_res;

        let sys = d.sys_layer;
        self.invalidate_opt(sys);
    }

    /// `lv_display_set_offset()`
    pub fn display_set_offset(&mut self, disp: DisplayId, x: i32, y: i32) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_offset: disp != NULL") else {
            return;
        };

        d.offset_x = x;
        d.offset_y = y;

        let sys = d.sys_layer;
        self.invalidate_opt(sys);
    }

    /// `lv_display_set_rotation()`
    pub fn display_set_rotation(&mut self, disp: DisplayId, rotation: Rotation) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_rotation: disp != NULL") else {
            return;
        };

        d.rotation = rotation;
        self.update_resolution(disp);
    }

    /// `lv_display_set_matrix_rotation()`: needs `LV_DRAW_TRANSFORM_USE_MATRIX`,
    /// which the parity configuration disables, so the argument check always
    /// fails.
    pub fn display_set_matrix_rotation(&mut self, disp: DisplayId, enable: bool) {
        if !DRAW_TRANSFORM_USE_MATRIX {
            check_arg_failed("LV_DRAW_TRANSFORM_USE_MATRIX is not enabled");
            return;
        }

        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_matrix_rotation: disp != NULL") else {
            return;
        };
        if !(d.render_mode == RenderMode::Direct || d.render_mode == RenderMode::Full) {
            check_arg_failed("display_set_matrix_rotation: Unsupported rendering mode");
            return;
        }

        d.matrix_rotation = enable;
    }

    /// `lv_display_set_dpi()`
    pub fn display_set_dpi(&mut self, disp: DisplayId, dpi: i32) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_dpi: disp != NULL") else {
            return;
        };

        d.dpi = dpi as u32;
    }

    /// `lv_display_get_horizontal_resolution()`
    pub fn display_get_horizontal_resolution(&self, disp: Option<DisplayId>) -> i32 {
        let Some(d) = self.disp_checked(disp, "display_get_horizontal_resolution: disp != NULL") else {
            return 0;
        };

        match d.rotation {
            Rotation::R90 | Rotation::R270 => d.ver_res,
            _ => d.hor_res,
        }
    }

    /// `lv_display_get_vertical_resolution()`
    pub fn display_get_vertical_resolution(&self, disp: Option<DisplayId>) -> i32 {
        let Some(d) = self.disp_checked(disp, "display_get_vertical_resolution: disp != NULL") else {
            return 0;
        };
        match d.rotation {
            Rotation::R90 | Rotation::R270 => d.hor_res,
            _ => d.ver_res,
        }
    }

    /// `lv_display_get_original_horizontal_resolution()`
    pub fn display_get_original_horizontal_resolution(&self, disp: Option<DisplayId>) -> i32 {
        let Some(d) = self.disp_checked(disp, "display_get_original_horizontal_resolution: disp != NULL")
        else {
            return 0;
        };

        d.hor_res
    }

    /// `lv_display_get_original_vertical_resolution()`
    pub fn display_get_original_vertical_resolution(&self, disp: Option<DisplayId>) -> i32 {
        let Some(d) = self.disp_checked(disp, "display_get_original_vertical_resolution: disp != NULL")
        else {
            return 0;
        };

        d.ver_res
    }

    /// `lv_display_get_physical_horizontal_resolution()`
    pub fn display_get_physical_horizontal_resolution(&self, disp: Option<DisplayId>) -> i32 {
        let Some(d) = self.disp_checked(disp, "display_get_physical_horizontal_resolution: disp != NULL")
        else {
            return 0;
        };

        match d.rotation {
            Rotation::R90 | Rotation::R270 => {
                if d.physical_ver_res > 0 {
                    d.physical_ver_res
                } else {
                    d.ver_res
                }
            }
            _ => {
                if d.physical_hor_res > 0 {
                    d.physical_hor_res
                } else {
                    d.hor_res
                }
            }
        }
    }

    /// `lv_display_get_physical_vertical_resolution()`
    pub fn display_get_physical_vertical_resolution(&self, disp: Option<DisplayId>) -> i32 {
        let Some(d) = self.disp_checked(disp, "display_get_physical_vertical_resolution: disp != NULL")
        else {
            return 0;
        };
        match d.rotation {
            Rotation::R90 | Rotation::R270 => {
                if d.physical_hor_res > 0 {
                    d.physical_hor_res
                } else {
                    d.hor_res
                }
            }
            _ => {
                if d.physical_ver_res > 0 {
                    d.physical_ver_res
                } else {
                    d.ver_res
                }
            }
        }
    }

    /// `lv_display_get_offset_x()`
    pub fn display_get_offset_x(&self, disp: Option<DisplayId>) -> i32 {
        let Some(d) = self.disp_checked(disp, "display_get_offset_x: disp != NULL") else {
            return 0;
        };
        match d.rotation {
            Rotation::R90 => d.offset_y,
            Rotation::R180 => self
                .display_get_physical_horizontal_resolution(disp)
                .wrapping_sub(d.offset_x),
            Rotation::R270 => self
                .display_get_physical_horizontal_resolution(disp)
                .wrapping_sub(d.offset_y),
            _ => d.offset_x,
        }
    }

    /// `lv_display_get_offset_y()`
    pub fn display_get_offset_y(&self, disp: Option<DisplayId>) -> i32 {
        let Some(d) = self.disp_checked(disp, "display_get_offset_y: disp != NULL") else {
            return 0;
        };
        match d.rotation {
            Rotation::R90 => d.offset_x,
            Rotation::R180 => self
                .display_get_physical_vertical_resolution(disp)
                .wrapping_sub(d.offset_y),
            Rotation::R270 => self
                .display_get_physical_vertical_resolution(disp)
                .wrapping_sub(d.offset_x),
            _ => d.offset_y,
        }
    }

    /// `lv_display_get_rotation()`
    pub fn display_get_rotation(&self, disp: Option<DisplayId>) -> Rotation {
        let Some(d) = self.disp_checked(disp, "display_get_rotation: disp != NULL") else {
            return Rotation::R0;
        };
        d.rotation
    }

    /// `lv_display_get_matrix_rotation()`
    pub fn display_get_matrix_rotation(&self, disp: Option<DisplayId>) -> bool {
        let Some(d) = self.disp_checked(disp, "display_get_matrix_rotation: disp != NULL") else {
            return false;
        };
        if !DRAW_TRANSFORM_USE_MATRIX {
            check_arg_failed("LV_DRAW_TRANSFORM_USE_MATRIX is not enabled");
            return false;
        }
        d.matrix_rotation
    }

    /// `lv_display_get_dpi()`
    pub fn display_get_dpi(&self, disp: Option<DisplayId>) -> i32 {
        let Some(d) = self.disp_checked(disp, "display_get_dpi: disp != NULL") else {
            return self.config().dpi_def;
        };

        d.dpi as i32
    }

    /*---------------------
     * BUFFERING
     *--------------------*/

    /// `lv_display_set_draw_buffers()`
    pub fn display_set_draw_buffers(
        &mut self,
        disp: DisplayId,
        buf1: Option<DrawBuf>,
        buf2: Option<DrawBuf>,
    ) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_draw_buffers: disp != NULL") else {
            return;
        };
        if buf1.is_none() {
            check_arg_failed("display_set_draw_buffers: buf1 != NULL");
            return;
        }

        d.buf_1 = buf1;
        d.buf_2 = buf2;
        d.buf_act = 1;

        d.stride_is_auto = false;
    }

    /// `lv_display_set_3rd_draw_buffer()`
    pub fn display_set_3rd_draw_buffer(&mut self, disp: DisplayId, buf3: Option<DrawBuf>) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_3rd_draw_buffer: disp != NULL") else {
            return;
        };
        if d.buf_1.is_none() {
            check_arg_failed("buf1 should already exist in order to provide a third buffer");
            return;
        }
        if d.buf_2.is_none() {
            check_arg_failed("buf2 should already exist in order to provide a third buffer");
            return;
        }

        d.buf_3 = buf3;
    }

    /// `lv_display_set_buffers()`: `buf_size` is in bytes.
    pub fn display_set_buffers(
        &mut self,
        disp: DisplayId,
        buf1: BufStorage,
        buf2: Option<BufStorage>,
        buf_size: u32,
        render_mode: RenderMode,
    ) {
        if !self.displays.contains(disp.0) {
            check_arg_failed("display_set_buffers: disp != NULL");
            return;
        }
        self.display_set_buffers_with_stride(disp, buf1, buf2, buf_size, STRIDE_AUTO, render_mode);
    }

    /// `lv_display_set_buffers_with_stride()`
    pub fn display_set_buffers_with_stride(
        &mut self,
        disp: DisplayId,
        buf1: BufStorage,
        buf2: Option<BufStorage>,
        buf_size: u32,
        stride: u32,
        render_mode: RenderMode,
    ) {
        if !self.displays.contains(disp.0) {
            check_arg_failed("display_set_buffers_with_stride: disp != NULL");
            return;
        }

        let cf = self.display_get_color_format(Some(disp));
        let w = self.display_get_original_horizontal_resolution(Some(disp)) as u32;
        let mut h = self.display_get_original_vertical_resolution(Some(disp)) as u32;

        if !(w != 0 && h != 0) {
            check_arg_failed("display resolution is 0");
            return;
        }

        /* buf1 or buf2 is not aligned according to LV_DRAW_BUF_ALIGN */
        let addr1 = storage_addr(&buf1);
        if addr1 != DrawBuf::align(addr1, cf) {
            check_arg_failed("buf1 is not properly aligned");
            return;
        }
        if let Some(b2) = &buf2 {
            let addr2 = storage_addr(b2);
            if addr2 != DrawBuf::align(addr2, cf) {
                check_arg_failed("buf2 is not properly aligned");
                return;
            }
        }

        let mut stride = stride;
        let is_auto_stride = stride == STRIDE_AUTO;
        if is_auto_stride {
            stride = DrawBuf::width_to_stride(w, cf);
        }

        if render_mode == RenderMode::Partial {
            if stride == 0 {
                check_arg_failed("stride is 0, check your color format and width");
                return;
            }
            /* for partial mode, we calculate the height based on the buf_size and stride */
            h = buf_size / stride;
            if h == 0 {
                check_arg_failed("the buffer is too small");
                return;
            }
        } else if stride.wrapping_mul(h) > buf_size {
            check_arg_failed(if render_mode == RenderMode::Full {
                "FULL mode requires screen sized buffer(s)"
            } else {
                "DIRECT mode requires screen sized buffer(s)"
            });
            return;
        }

        /* `lv_draw_buf_init()` cannot fail here: `stride * h <= buf_size` was
         * ensured above. */
        let Ok(static_buf1) = DrawBuf::init(w, h, cf, stride, buf1, buf_size) else {
            return;
        };
        let static_buf2 = match buf2 {
            Some(b2) => match DrawBuf::init(w, h, cf, stride, b2, buf_size) {
                Ok(b) => Some(b),
                Err(_) => return,
            },
            None => None,
        };
        self.display_set_draw_buffers(disp, Some(static_buf1), static_buf2);
        self.display_set_render_mode(disp, render_mode);
        if let Some(d) = self.display_mut(disp) {
            d.stride_is_auto = is_auto_stride;
        }
    }

    /// `lv_display_set_render_mode()`
    pub fn display_set_render_mode(&mut self, disp: DisplayId, render_mode: RenderMode) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_render_mode: disp != NULL") else {
            return;
        };
        d.render_mode = render_mode;
    }

    /// `lv_display_set_flush_cb()`, `lv_display_set_flush_wait_cb()`,
    /// `lv_display_set_sync_cb()`, `lv_display_set_sync_wait_cb()`: install
    /// the driver implementing them.
    pub fn display_set_driver(&mut self, disp: DisplayId, driver: Option<Box<dyn DisplayDriver>>) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_driver: disp != NULL") else {
            return;
        };

        d.driver = driver;
    }

    /// `lv_display_get_flush_cb()`: borrow the driver.
    pub fn display_driver_mut(&mut self, disp: DisplayId) -> Option<&mut dyn DisplayDriver> {
        let d = self.displays.get_mut(disp.0)?;
        match d.driver.as_deref_mut() {
            Some(drv) => Some(drv),
            None => None,
        }
    }

    /// `lv_display_set_color_format()`
    pub fn display_set_color_format(&mut self, disp: DisplayId, color_format: ColorFormat) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_color_format: disp != NULL") else {
            return;
        };

        d.color_format = color_format;
        let layer_head = d.layer_head;

        if let Some(b) = d.buf_1.as_mut() {
            b.header.cf = color_format;
        }
        if let Some(b) = d.buf_2.as_mut() {
            b.header.cf = color_format;
        }
        if let Some(b) = d.buf_3.as_mut() {
            b.header.cf = color_format;
        }
        let bottom_layer = d.bottom_layer.and_then(|id| self.resolve(id));

        if let Some(l) = layer_head.and_then(|l| self.draw.layer_mut(l)) {
            l.color_format = color_format;
        }

        if let Some(bottom) = bottom_layer {
            if color_format.has_alpha() {
                self.obj_remove_local_style_prop(bottom, StyleProp::BG_OPA, StyleSelector::DEFAULT);
            } else {
                self.obj_set_local_style_prop(
                    bottom,
                    StyleProp::BG_OPA,
                    StyleValue::Num(255),
                    StyleSelector::DEFAULT,
                );
            }
        }

        self.display_send_event(disp, EventCode::COLOR_FORMAT_CHANGED, EventParam::None);
    }

    /// `lv_display_get_color_format()`
    pub fn display_get_color_format(&self, disp: Option<DisplayId>) -> ColorFormat {
        let Some(d) = self.disp_checked(disp, "display_get_color_format: disp != NULL") else {
            return ColorFormat::Unknown;
        };

        d.color_format
    }

    /// `lv_display_set_tile_cnt()`
    pub fn display_set_tile_cnt(&mut self, disp: DisplayId, tile_cnt: u32) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_tile_cnt: disp != NULL") else {
            return;
        };
        if tile_cnt >= 256 {
            check_arg_failed("tile_cnt must be smaller than 256");
            return;
        }

        d.tile_cnt = tile_cnt as u8;
    }

    /// `lv_display_get_tile_cnt()`
    pub fn display_get_tile_cnt(&self, disp: DisplayId) -> u32 {
        let Some(d) = self.disp_checked(Some(disp), "display_get_tile_cnt: disp != NULL") else {
            return 0;
        };

        d.tile_cnt as u32
    }

    /// `lv_display_set_antialiasing()` ("Disabling anti-aliasing is not
    /// supported since v9").
    pub fn display_set_antialiasing(&mut self, disp: DisplayId, en: bool) {
        #[cfg(feature = "log")]
        log::warn!("Disabling anti-aliasing is not supported since v9. This function will be removed.");

        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_antialiasing: disp != NULL") else {
            return;
        };

        d.antialiasing = en;
    }

    /// `lv_display_get_antialiasing()`
    pub fn display_get_antialiasing(&self, disp: Option<DisplayId>) -> bool {
        let Some(d) = self.disp_checked(disp, "display_get_antialiasing: disp != NULL") else {
            return false;
        };

        d.antialiasing
    }

    /// `lv_display_get_render_mode()`
    pub fn display_get_render_mode(&self, disp: DisplayId) -> RenderMode {
        let Some(d) = self.disp_checked(Some(disp), "display_get_render_mode: disp != NULL") else {
            return RenderMode::Partial;
        };

        d.render_mode
    }

    /// `lv_display_flush_ready()`
    pub fn display_flush_ready(&mut self, disp: DisplayId) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_flush_ready: disp != NULL") else {
            return;
        };
        d.flushing = false;
    }

    /// `lv_display_flush_is_last()`
    pub fn display_flush_is_last(&self, disp: DisplayId) -> bool {
        let Some(d) = self.disp_checked(Some(disp), "display_flush_is_last: disp != NULL") else {
            return false;
        };
        d.flushing_last
    }

    /// `lv_display_sync_ready()`
    pub fn display_sync_ready(&mut self, disp: DisplayId) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_sync_ready: disp != NULL") else {
            return;
        };
        d.syncing = false;
    }

    /// `lv_display_sync_is_last()`
    pub fn display_sync_is_last(&self, disp: DisplayId) -> bool {
        let Some(d) = self.disp_checked(Some(disp), "display_sync_is_last: disp != NULL") else {
            return false;
        };
        d.syncing_last
    }

    /// `lv_display_is_double_buffered()`
    pub fn display_is_double_buffered(&self, disp: DisplayId) -> bool {
        let Some(d) = self.disp_checked(Some(disp), "display_is_double_buffered: disp != NULL") else {
            return false;
        };
        d.buf_2.is_some()
    }

    /*---------------------
     * SCREENS
     *--------------------*/

    /// `lv_display_get_screen_active()`
    pub fn display_get_screen_active(&self, disp: Option<DisplayId>) -> Option<Obj> {
        let d = self.disp_checked(disp, "display_get_screen_active: disp != NULL")?;

        d.act_scr.and_then(|id| self.resolve(id))
    }

    /// `lv_display_get_screen_loading()`
    pub fn display_get_screen_loading(&self, disp: Option<DisplayId>) -> Option<Obj> {
        let d = self.disp_checked(disp, "display_get_screen_loading: disp != NULL")?;

        d.scr_to_load.and_then(|id| self.resolve(id))
    }

    /// `lv_display_get_screen_prev()`
    pub fn display_get_screen_prev(&self, disp: Option<DisplayId>) -> Option<Obj> {
        let d = self.disp_checked(disp, "display_get_screen_prev: disp != NULL")?;

        d.prev_scr.and_then(|id| self.resolve(id))
    }

    /// `lv_display_get_layer_top()`
    pub fn display_get_layer_top(&self, disp: Option<DisplayId>) -> Option<Obj> {
        let d = self.disp_checked(disp, "display_get_layer_top: disp != NULL")?;

        d.top_layer.and_then(|id| self.resolve(id))
    }

    /// `lv_display_get_layer_sys()`
    pub fn display_get_layer_sys(&self, disp: Option<DisplayId>) -> Option<Obj> {
        let d = self.disp_checked(disp, "display_get_layer_sys: disp != NULL")?;

        d.sys_layer.and_then(|id| self.resolve(id))
    }

    /// `lv_display_get_layer_bottom()`
    pub fn display_get_layer_bottom(&self, disp: Option<DisplayId>) -> Option<Obj> {
        let d = self.disp_checked(disp, "display_get_layer_bottom: disp != NULL")?;

        d.bottom_layer.and_then(|id| self.resolve(id))
    }

    /// `lv_display_get_screen_by_name()` (`LV_USE_OBJ_NAME`)
    pub fn display_get_screen_by_name(&self, disp: Option<DisplayId>, screen_name: &str) -> Option<Obj> {
        let d = self.disp_checked(disp, "display_get_screen_by_name: disp != NULL")?;

        for &id in &d.screens {
            let Some(scr) = self.resolve(id) else {
                continue;
            };
            let n = self.obj_get_name(scr);
            if let Some(n) = n
                && screen_name == &*n
            {
                return Some(scr);
            }
        }

        None
    }

    /// `lv_screen_load()`
    pub fn screen_load(&mut self, scr: Obj) {
        self.screen_load_anim(scr, ScreenLoadAnim::None, 0, 0, false);
    }

    /// `lv_screen_load_anim()`
    pub fn screen_load_anim(
        &mut self,
        scr: Obj,
        anim_type: ScreenLoadAnim,
        time: u32,
        delay: u32,
        auto_del: bool,
    ) {
        let Some(d) = self.obj_get_display(scr).filter(|d| self.displays.contains(d.0)) else {
            check_arg_failed("screen_load_anim: scr != NULL");
            return;
        };
        let mut act_scr = self
            .display(d)
            .and_then(|dd| dd.act_scr)
            .and_then(|id| self.resolve(id));
        let scr_to_load = self.display(d).and_then(|dd| dd.scr_to_load);

        if act_scr == Some(scr) || scr_to_load == Some(scr.0) {
            return;
        }

        /*If another screen load animation is in progress
         *make target screen loaded immediately. */
        if let Some(stl) = scr_to_load.and_then(|id| self.resolve(id))
            && act_scr != Some(stl)
        {
            self.anim_delete(AnimTarget::Obj(stl), None);
            self.obj_set_pos(stl, 0, 0);
            self.obj_remove_local_style_prop(stl, StyleProp::OPA, StyleSelector::DEFAULT);

            if let Some(dd) = self.display_mut(d) {
                dd.prev_scr = dd.act_scr;
            }
            act_scr = Some(stl); /*Active screen changed.*/
            let res = self.load_new_screen(stl);
            if res == LoadScreenResult::DisplayDeleted {
                return;
            }
            if old_screen_deleted(res)
                && let Some(dd) = self.display_mut(d)
            {
                dd.prev_scr = None;
            }
            if new_screen_deleted(res) {
                return;
            }
        }

        let Some(dd) = self.display_mut(d) else {
            return;
        };
        dd.scr_to_load = Some(scr.0);

        let prev_scr = dd.prev_scr;
        let del_prev = dd.del_prev;
        if let Some(prev) = prev_scr.and_then(|id| self.resolve(id))
            && del_prev
        {
            self.obj_delete(prev);
        }
        let Some(dd) = self.display_mut(d) else {
            return;
        };
        dd.prev_scr = None;

        dd.draw_prev_over_act = is_out_anim(anim_type);
        dd.del_prev = auto_del;

        // A deleted handle would be a dangling pointer in C.
        let act_scr = act_scr.filter(|a| self.resolve(a.0).is_some());

        /*Be sure there is no other animation on the screens*/
        self.anim_delete(AnimTarget::Obj(scr), None);
        if let Some(act) = act_scr {
            self.anim_delete(AnimTarget::Obj(act), None);
        }

        /*Be sure both screens are in a normal position*/
        self.obj_set_pos(scr, 0, 0);
        if let Some(act) = act_scr {
            self.obj_set_pos(act, 0, 0);
        }
        self.obj_remove_local_style_prop(scr, StyleProp::OPA, StyleSelector::DEFAULT);
        if let Some(act) = act_scr {
            self.obj_remove_local_style_prop(act, StyleProp::OPA, StyleSelector::DEFAULT);
        }

        /*Shortcut for immediate load*/
        if time == 0 && delay == 0 {
            let res = self.load_new_screen(scr);
            if res == LoadScreenResult::DisplayDeleted {
                return;
            }
            if new_screen_deleted(res)
                && let Some(dd) = self.display_mut(d)
            {
                dd.act_scr = None;
            }
            if !old_screen_deleted(res)
                && auto_del
                && let Some(act) = act_scr
            {
                self.obj_delete(act);
            }
            return;
        }

        let mut a_new = Anim::new();
        a_new.set_var(AnimTarget::Obj(scr));
        let start_cb: AnimCallback = Rc::new(|ui: &mut Ui, a: AnimId| ui.scr_load_anim_start(a));
        a_new.set_start_cb(Some(start_cb));
        let completed_cb: AnimCallback = Rc::new(|ui: &mut Ui, a: AnimId| ui.scr_anim_completed(a));
        a_new.set_completed_cb(Some(completed_cb));
        a_new.set_duration(time);
        a_new.set_delay(delay);

        let mut a_old = Anim::new();
        a_old.set_var(act_scr.map_or(AnimTarget::None, AnimTarget::Obj));
        a_old.set_duration(time);
        a_old.set_delay(delay);

        let hor_res = self.display_get_horizontal_resolution(Some(d));
        let ver_res = self.display_get_vertical_resolution(Some(d));

        match anim_type {
            ScreenLoadAnim::None => {
                /*Create a dummy animation to apply the delay*/
                a_new.set_exec_cb(set_x_anim());
                a_new.set_values(0, 0);
            }
            ScreenLoadAnim::OverLeft => {
                a_new.set_exec_cb(set_x_anim());
                a_new.set_values(hor_res, 0);
            }
            ScreenLoadAnim::OverRight => {
                a_new.set_exec_cb(set_x_anim());
                a_new.set_values(hor_res.wrapping_neg(), 0);
            }
            ScreenLoadAnim::OverTop => {
                a_new.set_exec_cb(set_y_anim());
                a_new.set_values(ver_res, 0);
            }
            ScreenLoadAnim::OverBottom => {
                a_new.set_exec_cb(set_y_anim());
                a_new.set_values(ver_res.wrapping_neg(), 0);
            }
            ScreenLoadAnim::MoveLeft => {
                a_new.set_exec_cb(set_x_anim());
                a_new.set_values(hor_res, 0);

                a_old.set_exec_cb(set_x_anim());
                a_old.set_values(0, hor_res.wrapping_neg());
            }
            ScreenLoadAnim::MoveRight => {
                a_new.set_exec_cb(set_x_anim());
                a_new.set_values(hor_res.wrapping_neg(), 0);

                a_old.set_exec_cb(set_x_anim());
                a_old.set_values(0, hor_res);
            }
            ScreenLoadAnim::MoveTop => {
                a_new.set_exec_cb(set_y_anim());
                a_new.set_values(ver_res, 0);

                a_old.set_exec_cb(set_y_anim());
                a_old.set_values(0, ver_res.wrapping_neg());
            }
            ScreenLoadAnim::MoveBottom => {
                a_new.set_exec_cb(set_y_anim());
                a_new.set_values(ver_res.wrapping_neg(), 0);

                a_old.set_exec_cb(set_y_anim());
                a_old.set_values(0, ver_res);
            }
            ScreenLoadAnim::FadeIn => {
                a_new.set_exec_cb(opa_scale_anim());
                a_new.set_values(OPA_TRANSP as i32, OPA_COVER as i32);
            }
            ScreenLoadAnim::FadeOut => {
                a_old.set_exec_cb(opa_scale_anim());
                a_old.set_values(OPA_COVER as i32, OPA_TRANSP as i32);
            }
            ScreenLoadAnim::OutLeft => {
                a_old.set_exec_cb(set_x_anim());
                a_old.set_values(0, hor_res.wrapping_neg());
            }
            ScreenLoadAnim::OutRight => {
                a_old.set_exec_cb(set_x_anim());
                a_old.set_values(0, hor_res);
            }
            ScreenLoadAnim::OutTop => {
                a_old.set_exec_cb(set_y_anim());
                a_old.set_values(0, ver_res.wrapping_neg());
            }
            ScreenLoadAnim::OutBottom => {
                a_old.set_exec_cb(set_y_anim());
                a_old.set_values(0, ver_res);
            }
        }

        if let Some(act) = act_scr {
            self.obj_send_event(act, EventCode::SCREEN_UNLOAD_START, EventParam::None);
        }

        self.anim_start(&a_new);
        // A deleted handle would be a dangling pointer in C.
        if let Some(act) = act_scr
            && self.resolve(act.0).is_some()
        {
            self.anim_start(&a_old);
        }
    }

    /*---------------------
     * OTHERS
     *--------------------*/

    /// `lv_display_add_event_cb()`
    pub fn display_add_event_cb(
        &mut self,
        disp: DisplayId,
        cb: EventCallback,
        filter: impl Into<EventFilter>,
        user_data: Option<Rc<dyn Any>>,
    ) -> EventDscId {
        let filter = filter.into();
        if !self.displays.contains(disp.0) {
            check_arg_failed("display_add_event_cb: disp != NULL");
            // `NULL`: a handle that names no descriptor (a freed slot).
            let dsc = EventDsc {
                ext_data: Default::default(),
                cb,
                user_data: None,
                filter: filter.to_u32(),
            };
            let key = self.event_dscs.insert(dsc);
            self.event_dscs.remove(key);
            return EventDscId(key);
        }
        self.display_with_event_list(disp, |ui, list| ui.event_add(list, cb, filter, user_data))
            .expect("display checked above")
    }

    /// `lv_display_get_event_count()`
    pub fn display_get_event_count(&self, disp: DisplayId) -> u32 {
        let Some(d) = self.disp_checked(Some(disp), "display_get_event_count: disp != NULL") else {
            return 0;
        };
        d.event_list.len() as u32
    }

    /// `lv_display_get_event_dsc()`
    pub fn display_get_event_dsc(&self, disp: DisplayId, index: u32) -> Option<&EventDsc> {
        let d = self.disp_checked(Some(disp), "display_get_event_dsc: disp != NULL")?;
        self.event_get_dsc(&d.event_list, index as usize)
    }

    /// `lv_display_remove_event()`
    pub fn display_remove_event(&mut self, disp: DisplayId, index: u32) -> bool {
        match self.display_with_event_list(disp, |ui, list| ui.event_remove(list, index as usize)) {
            Some(res) => res,
            None => {
                check_arg_failed("display_remove_event: disp != NULL");
                false
            }
        }
    }

    /// `lv_display_remove_event_cb_with_user_data()`
    pub fn display_remove_event_cb_with_user_data(
        &mut self,
        disp: DisplayId,
        cb: &EventCallback,
        user_data: &Rc<dyn Any>,
    ) -> u32 {
        self.display_remove_event_cb_with_user_data_opt(disp, cb, Some(user_data))
    }

    /// `lv_display_remove_event_cb_with_user_data()` with a nullable
    /// `user_data` (`NULL` only matches descriptors without user data).
    pub(crate) fn display_remove_event_cb_with_user_data_opt(
        &mut self,
        disp: DisplayId,
        cb: &EventCallback,
        user_data: Option<&Rc<dyn Any>>,
    ) -> u32 {
        if !self.displays.contains(disp.0) {
            check_arg_failed("display_remove_event_cb_with_user_data: disp != NULL");
            return 0;
        }

        let event_cnt = self.display_get_event_count(disp);
        let mut removed_count: u32 = 0;
        let mut i = event_cnt.wrapping_sub(1) as i32;

        while i >= 0 {
            let matches = self.display_get_event_dsc(disp, i as u32).is_some_and(|dsc| {
                Rc::ptr_eq(&dsc.cb, cb) && user_data_eq(dsc.user_data.as_ref(), user_data)
            });
            if matches {
                self.display_remove_event(disp, i as u32);
                removed_count = removed_count.wrapping_add(1);
            }
            i -= 1;
        }

        removed_count
    }

    /// `lv_display_send_event()`
    ///
    /// The display's event list is passed to the event module as a clone
    /// (the live list stays in the display, so callbacks may add / remove
    /// callbacks or delete the display). When the display survives, the
    /// event is then forwarded to [`DisplayDriver::on_event`].
    pub fn display_send_event(
        &mut self,
        disp: DisplayId,
        code: EventCode,
        param: EventParam<'_>,
    ) -> LvResult {
        let Some(d) = self.disp_checked(Some(disp), "display_send_event: disp != NULL") else {
            return LvResult::Invalid;
        };
        let list = d.event_list.clone();
        let res = self.event_push_and_send(list, code, EventTarget::Display(disp), param);

        if res == LvResult::Ok
            && let Some(drv) = self.displays.get_mut(disp.0).and_then(|d| d.driver.as_mut())
        {
            drv.on_event(code);
        }

        res
    }

    /// `lv_display_set_theme()`
    pub fn display_set_theme(&mut self, disp: DisplayId, th: Option<ThemeId>) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_theme: disp != NULL") else {
            return;
        };

        d.theme = th;

        let screens = d.screens.clone();
        if screens.len() == 4
            && self.child_count_of(screens[0]) == 0
            && self.child_count_of(screens[1]) == 0
            && self.child_count_of(screens[2]) == 0
        {
            if let Some(s) = self.resolve(screens[0]) {
                self.theme_apply(s);
            }

            if th.is_none() {
                /* When th is NULL, clear all styles */
                let mut i = 1;
                while let Some(id) = self.display(disp).and_then(|d| d.screens.get(i).copied()) {
                    if let Some(s) = self.resolve(id) {
                        self.theme_apply(s);
                    }
                    i += 1;
                }
            }
        }
    }

    /// `lv_display_get_theme()`
    pub fn display_get_theme(&self, disp: Option<DisplayId>) -> Option<ThemeId> {
        let d = self.disp_checked(disp, "display_get_theme: disp != NULL")?;
        d.theme
    }

    /// `lv_display_get_inactive_time()`: `None` returns the smallest inactive
    /// time of all displays.
    pub fn display_get_inactive_time(&self, disp: Option<DisplayId>) -> u32 {
        if let Some(disp) = disp {
            return match self.display(disp) {
                Some(d) => self.tick_elaps(d.last_activity_time),
                None => {
                    check_arg_failed("display_get_inactive_time: invalid display");
                    0
                }
            };
        }

        let mut t = u32::MAX;
        let mut d = self.display_get_next(None);
        while let Some(id) = d {
            if let Some(dd) = self.display(id) {
                let elaps = self.tick_elaps(dd.last_activity_time);
                t = t.min(elaps);
            }
            d = self.display_get_next(Some(id));
        }

        t
    }

    /// `lv_display_trigger_activity()`
    pub fn display_trigger_activity(&mut self, disp: Option<DisplayId>) {
        let tick = self.tick_get();
        let Some(d) = self.disp_checked_mut(disp, "display_trigger_activity: disp != NULL") else {
            return;
        };

        d.last_activity_time = tick;
    }

    /// `lv_display_enable_invalidation()`
    pub fn display_enable_invalidation(&mut self, disp: Option<DisplayId>, en: bool) {
        let Some(d) = self.disp_checked_mut(disp, "display_enable_invalidation: disp != NULL") else {
            return;
        };
        d.inv_en_cnt = d.inv_en_cnt.wrapping_add(if en { 1 } else { -1 });
    }

    /// `lv_display_is_invalidation_enabled()`
    pub fn display_is_invalidation_enabled(&self, disp: Option<DisplayId>) -> bool {
        let Some(d) = self.disp_checked(disp, "display_is_invalidation_enabled: disp != NULL") else {
            return false;
        };
        d.inv_en_cnt > 0
    }

    /// `lv_display_get_refr_timer()`
    pub fn display_get_refr_timer(&self, disp: Option<DisplayId>) -> Option<TimerId> {
        let d = self.disp_checked(disp, "display_get_refr_timer: disp != NULL")?;
        d.refr_timer
    }

    /// `lv_display_delete_refr_timer()`
    pub fn display_delete_refr_timer(&mut self, disp: Option<DisplayId>) {
        let Some(d) = self.disp_checked_mut(disp, "display_delete_refr_timer: disp != NULL") else {
            return;
        };

        let timer = d.refr_timer.take();
        // `lv_timer_delete(NULL)` is a no-op.
        if let Some(t) = timer {
            self.timer_delete(t);
        }
    }

    /// `lv_display_send_vsync_event()`
    pub fn display_send_vsync_event(&mut self, disp: DisplayId, param: EventParam<'_>) -> LvResult {
        let Some(d) = self.disp_checked(Some(disp), "display_send_vsync_event: disp != NULL") else {
            return LvResult::Invalid;
        };

        if d.vsync_count > 0 {
            return self.display_send_event(disp, EventCode::VSYNC, param);
        }

        LvResult::Invalid
    }

    /// `lv_display_register_vsync_event()`
    pub fn display_register_vsync_event(
        &mut self,
        disp: DisplayId,
        cb: EventCallback,
        user_data: Option<Rc<dyn Any>>,
    ) -> bool {
        if !self.displays.contains(disp.0) {
            check_arg_failed("display_register_vsync_event: disp != NULL");
            return false;
        }

        self.display_add_event_cb(disp, cb, EventCode::VSYNC, user_data);

        /*only send once*/
        if self.display(disp).is_some_and(|d| d.vsync_count == 0) {
            let param = disp;
            self.display_send_event(disp, EventCode::VSYNC_REQUEST, EventParam::Any(&param));
        }

        if let Some(d) = self.display_mut(disp) {
            d.vsync_count = d.vsync_count.wrapping_add(1);
        }
        true
    }

    /// `lv_display_unregister_vsync_event()`: the callback is matched with
    /// `Rc::ptr_eq`, `user_data` by pointer identity (`None` = `NULL`).
    pub fn display_unregister_vsync_event(
        &mut self,
        disp: DisplayId,
        cb: &EventCallback,
        user_data: Option<&Rc<dyn Any>>,
    ) -> bool {
        if !self.displays.contains(disp.0) {
            check_arg_failed("display_unregister_vsync_event: disp != NULL");
            return false;
        }

        let removed_count = self.display_remove_event_cb_with_user_data_opt(disp, cb, user_data);
        if removed_count == 0 {
            return false;
        }

        let Some(d) = self.display_mut(disp) else {
            return true;
        };
        d.vsync_count = d.vsync_count.wrapping_sub(removed_count);
        /*only send once*/
        if d.vsync_count == 0 {
            self.display_send_event(disp, EventCode::VSYNC_REQUEST, EventParam::None);
        }

        true
    }

    /// `lv_display_set_user_data()`
    pub fn display_set_user_data(&mut self, disp: DisplayId, user_data: Option<Rc<dyn Any>>) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_user_data: disp != NULL") else {
            return;
        };
        d.user_data = user_data;
    }

    /// `lv_display_set_driver_data()`
    pub fn display_set_driver_data(&mut self, disp: DisplayId, driver_data: Option<Box<dyn Any>>) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_driver_data: disp != NULL") else {
            return;
        };

        d.driver_data = driver_data;
    }

    /// `lv_display_get_user_data()`
    pub fn display_get_user_data(&self, disp: DisplayId) -> Option<Rc<dyn Any>> {
        let d = self.disp_checked(Some(disp), "display_get_user_data: disp != NULL")?;

        d.user_data.clone()
    }

    /// `lv_display_get_driver_data()`
    pub fn display_get_driver_data(&mut self, disp: DisplayId) -> Option<&mut dyn Any> {
        let d = self.disp_checked_mut(Some(disp), "display_get_driver_data: disp != NULL")?;

        match d.driver_data.as_deref_mut() {
            Some(data) => Some(data),
            None => None,
        }
    }

    /// `lv_display_get_buf_active()`
    pub fn display_get_buf_active(&self, disp: DisplayId) -> Option<&DrawBuf> {
        let d = self.disp_checked(Some(disp), "display_get_buf_active: disp != NULL")?;
        d.buf_act_ref()
    }

    /// `lv_display_rotate_area()`
    pub fn display_rotate_area(&self, disp: DisplayId, area: &mut Area) {
        let Some(d) = self.disp_checked(Some(disp), "display_rotate_area: disp != NULL") else {
            return;
        };
        let rotation = self.display_get_rotation(Some(disp));

        if rotation == Rotation::R0 {
            return;
        }

        let w = area.width();
        let h = area.height();

        match rotation {
            Rotation::R90 => {
                area.y2 = d.ver_res - area.x1 - 1;
                area.x1 = area.y1;
                area.x2 = area.x1 + h - 1;
                area.y1 = area.y2 - w + 1;
            }
            Rotation::R180 => {
                area.y2 = d.ver_res - area.y1 - 1;
                area.y1 = area.y2 - h + 1;
                area.x2 = d.hor_res - area.x1 - 1;
                area.x1 = area.x2 - w + 1;
            }
            Rotation::R270 => {
                area.x1 = d.hor_res - area.y2 - 1;
                area.y2 = area.x2;
                area.x2 = area.x1 + h - 1;
                area.y1 = area.y2 - w + 1;
            }
            Rotation::R0 => {}
        }
    }

    /// `lv_display_rotate_point()`
    pub fn display_rotate_point(&self, disp: DisplayId, point: &mut Point) {
        let Some(d) = self.disp_checked(Some(disp), "display_rotate_point: disp != NULL") else {
            return;
        };
        let rotation = self.display_get_rotation(Some(disp));

        if rotation == Rotation::R0 {
            return;
        }

        let x = point.x;
        let y = point.y;

        match rotation {
            Rotation::R90 => {
                point.x = d.ver_res - y - 1;
                point.y = x;
            }
            Rotation::R180 => {
                point.x = d.hor_res - x - 1;
                point.y = d.ver_res - y - 1;
            }
            Rotation::R270 => {
                point.x = y;
                point.y = d.hor_res - x - 1;
            }
            Rotation::R0 => {}
        }
    }

    /// `lv_display_rotate_point_ccw()`
    pub fn display_rotate_point_ccw(&self, disp: DisplayId, point: &mut Point) {
        let Some(d) = self.disp_checked(Some(disp), "display_rotate_point_ccw: disp != NULL") else {
            return;
        };

        let rotation = self.display_get_rotation(Some(disp));

        if rotation == Rotation::R0 {
            return;
        }

        let x = point.x;
        let y = point.y;

        match rotation {
            Rotation::R90 => {
                point.x = y;
                point.y = d.hor_res - x - 1;
            }
            Rotation::R180 => {
                point.x = d.hor_res - x - 1;
                point.y = d.ver_res - y - 1;
            }
            Rotation::R270 => {
                point.x = d.ver_res - y - 1;
                point.y = x;
            }
            Rotation::R0 => {}
        }
    }

    /// `lv_display_get_draw_buf_size()`
    pub fn display_get_draw_buf_size(&self, disp: DisplayId) -> u32 {
        let Some(d) = self.disp_checked(Some(disp), "display_get_draw_buf_size: disp != NULL") else {
            return 0;
        };
        let Some(b) = d.buf_1.as_ref() else {
            check_arg_failed("display_get_draw_buf_size: buf_1 != NULL");
            return 0;
        };
        b.data_size
    }

    /// `lv_display_get_invalidated_draw_buf_size()`
    pub fn display_get_invalidated_draw_buf_size(&self, disp: DisplayId, width: u32, height: u32) -> u32 {
        let Some(d) = self.disp_checked(Some(disp), "display_get_invalidated_draw_buf_size: disp != NULL")
        else {
            return 0;
        };

        let mut width = width;
        let mut height = height;
        if d.render_mode == RenderMode::Full {
            width = self.display_get_horizontal_resolution(Some(disp)) as u32;
            height = self.display_get_vertical_resolution(Some(disp)) as u32;
        }

        let cf = self.display_get_color_format(Some(disp));
        let stride = DrawBuf::width_to_stride(width, cf);
        let buf_size = stride.wrapping_mul(height);

        debug_assert!(d.buf_1.as_ref().is_none_or(|b| b.data_size >= buf_size));
        debug_assert!(d.buf_2.as_ref().is_none_or(|b| b.data_size >= buf_size));
        debug_assert!(d.buf_3.as_ref().is_none_or(|b| b.data_size >= buf_size));

        buf_size
    }

    /// `lv_screen_active()`
    pub fn screen_active(&self) -> Option<Obj> {
        self.display_get_screen_active(self.display_get_default())
    }

    /// `lv_layer_top()`
    pub fn layer_top(&self) -> Option<Obj> {
        self.display_get_layer_top(self.display_get_default())
    }

    /// `lv_layer_sys()`
    pub fn layer_sys(&self) -> Option<Obj> {
        self.display_get_layer_sys(self.display_get_default())
    }

    /// `lv_layer_bottom()`
    pub fn layer_bottom(&self) -> Option<Obj> {
        self.display_get_layer_bottom(self.display_get_default())
    }

    /// `lv_dpx()`: `LV_DPX(n)` with the default display's DPI.
    pub fn dpx(&self, n: i32) -> i32 {
        dpx_calc(self.display_get_dpi(None), n)
    }

    /// `lv_display_dpx()`
    pub fn display_dpx(&self, disp: Option<DisplayId>, n: i32) -> i32 {
        if self.disp_checked(disp, "display_dpx: disp != NULL").is_none() {
            return self.config().dpi_def;
        }
        dpx_calc(self.display_get_dpi(disp), n)
    }

    /// `lv_display_set_external_data()` (`LV_USE_EXT_DATA`): `free_cb` is
    /// called with `data` when the display is deleted.
    pub fn display_set_external_data(
        &mut self,
        disp: DisplayId,
        data: Option<Box<dyn Any>>,
        free_cb: Option<ExtDataFreeCb>,
    ) {
        let Some(d) = self.disp_checked_mut(Some(disp), "display_set_external_data: disp != NULL") else {
            return;
        };

        d.ext_data.data = data;
        d.ext_data.free_cb = free_cb;
    }

    /// Borrow a display.
    #[inline]
    pub fn display(&self, disp: DisplayId) -> Option<&DisplayData> {
        self.displays.get(disp.0)
    }

    /// Mutably borrow a display.
    #[inline]
    pub fn display_mut(&mut self, disp: DisplayId) -> Option<&mut DisplayData> {
        self.displays.get_mut(disp.0)
    }

    /**********************
     *   STATIC FUNCTIONS
     **********************/

    /// `disp == NULL ? lv_display_get_default() : disp` followed by
    /// `LV_CHECK_ARG(disp != NULL)` (a stale handle counts as `NULL`).
    fn disp_checked(&self, disp: Option<DisplayId>, msg: &str) -> Option<&DisplayData> {
        let d = disp.or(self.disp_default).and_then(|d| self.displays.get(d.0));
        if d.is_none() {
            check_arg_failed(msg);
        }
        d
    }

    /// Mutable form of [`Ui::disp_checked`].
    fn disp_checked_mut(&mut self, disp: Option<DisplayId>, msg: &str) -> Option<&mut DisplayData> {
        let Some(id) = disp.or(self.disp_default).filter(|d| self.displays.contains(d.0)) else {
            check_arg_failed(msg);
            return None;
        };
        self.displays.get_mut(id.0)
    }

    /// The `LV_USE_EXT_DATA` block of `lv_display_delete()`:
    /// `if(free_cb) { free_cb(data); data = NULL; }`.
    fn display_ext_data_free(&mut self, disp: DisplayId) {
        if let Some(d) = self.display_mut(disp)
            && let Some(free_cb) = d.ext_data.free_cb
        {
            let data = d.ext_data.data.take();
            free_cb(data);
        }
    }

    /// Run `f` on the live event list of `disp` (moved out of the display for
    /// the duration of the call, so its traversal state is preserved).
    /// `None` if `disp` is not a display.
    fn display_with_event_list<R>(
        &mut self,
        disp: DisplayId,
        f: impl FnOnce(&mut Ui, &mut EventList) -> R,
    ) -> Option<R> {
        let mut list = core::mem::take(&mut self.displays.get_mut(disp.0)?.event_list);
        let res = f(self, &mut list);
        if let Some(d) = self.displays.get_mut(disp.0) {
            d.event_list = list;
        }
        Some(res)
    }

    /// `lv_obj_invalidate()` on a nullable layer handle.
    fn invalidate_opt(&mut self, obj: Option<ObjId>) {
        if let Some(o) = obj.and_then(|id| self.resolve(id)) {
            let _ = self.obj_invalidate(o);
        }
    }

    /// `lv_obj_get_child_count()` of a screen handle (0 if deleted).
    fn child_count_of(&self, id: ObjId) -> u32 {
        match self.resolve(id) {
            Some(o) => self.obj_get_child_count(o),
            None => 0,
        }
    }

    /// `lv_area_set_width/height(&obj->coords, ...)` + `LV_EVENT_SIZE_CHANGED`
    /// with the previous coordinates, as done by `update_resolution()`.
    fn update_resolution_obj(&mut self, id: Option<ObjId>, hor_res: i32, ver_res: i32, prev_coords: &Area) {
        let Some(obj) = id.and_then(|id| self.resolve(id)) else {
            return;
        };
        if let Some(o) = self.objs.get_mut(obj.0.0) {
            o.coords.set_width(hor_res);
            o.coords.set_height(ver_res);
        }
        self.obj_send_event(obj, EventCode::SIZE_CHANGED, EventParam::Area(*prev_coords));
    }

    /// `update_resolution()`
    fn update_resolution(&mut self, disp: DisplayId) {
        let hor_res = self.display_get_horizontal_resolution(Some(disp));
        let ver_res = self.display_get_vertical_resolution(Some(disp));
        debug_assert!(hor_res > 0);
        debug_assert!(ver_res > 0);

        let Some(d) = self.display(disp) else {
            return;
        };
        let prev_coords = match d.sys_layer.and_then(|id| self.resolve(id)) {
            Some(sys) => self.obj_get_coords(sys),
            None => Area::default(),
        };
        let mut i = 0;
        while let Some(scr) = self.display(disp).and_then(|d| d.screens.get(i).copied()) {
            self.update_resolution_obj(Some(scr), hor_res, ver_res, &prev_coords);
            i += 1;
        }

        let top = self.display(disp).and_then(|d| d.top_layer);
        self.update_resolution_obj(top, hor_res, ver_res, &prev_coords);

        let sys = self.display(disp).and_then(|d| d.sys_layer);
        self.update_resolution_obj(sys, hor_res, ver_res, &prev_coords);

        let bottom = self.display(disp).and_then(|d| d.bottom_layer);
        self.update_resolution_obj(bottom, hor_res, ver_res, &prev_coords);

        let Some(d) = self.display_mut(disp) else {
            return;
        };
        d.inv_areas.iter_mut().for_each(|a| *a = Area::default());
        d.inv_area_joined.iter_mut().for_each(|j| *j = false);
        d.inv_p = 0;
        let sys = d.sys_layer;
        self.invalidate_opt(sys);

        self.obj_tree_walk(None, &mut invalidate_layout_cb);

        self.display_send_event(disp, EventCode::RESOLUTION_CHANGED, EventParam::None);
    }

    /// `load_new_screen()`: load `scr` and report which screens survived the
    /// load / unload events.
    fn load_new_screen(&mut self, scr: Obj) -> LoadScreenResult {
        /*scr must not be NULL, but d->act_scr might be*/
        let Some(d) = self.obj_get_display(scr) else {
            return LoadScreenResult::DisplayDeleted;
        };

        let old = self
            .display(d)
            .and_then(|dd| dd.act_scr)
            .and_then(|id| self.resolve(id));
        // The C locals `old_scr` / `scr` whose address is the user data of
        // `screen_event_delete_cb`.
        let old_scr: Rc<Cell<Option<Obj>>> = Rc::new(Cell::new(old));
        let new_scr: Rc<Cell<Option<Obj>>> = Rc::new(Cell::new(Some(scr)));
        let delete_cb: EventCallback = Rc::new(screen_event_delete_cb);

        /* Attach an event delete cb to the screen so we know if the screen is deleted during an event*/
        if let Some(o) = old_scr.get() {
            let ud: Rc<dyn Any> = old_scr.clone();
            self.obj_add_event_cb(o, delete_cb.clone(), EventCode::DELETE, Some(ud));
        }
        let ud: Rc<dyn Any> = new_scr.clone();
        self.obj_add_event_cb(scr, delete_cb.clone(), EventCode::DELETE, Some(ud));

        if let Some(o) = old_scr.get() {
            if self.display_send_event(d, EventCode::SCREEN_UNLOAD_START, EventParam::Obj(o))
                == LvResult::Invalid
            {
                return LoadScreenResult::DisplayDeleted;
            }
            if let Some(o) = old_scr.get()
                && self.obj_send_event(o, EventCode::SCREEN_UNLOAD_START, EventParam::None)
                    == LvResult::Invalid
            {
                old_scr.set(None);
            }
        }

        let param = match new_scr.get() {
            Some(s) => EventParam::Obj(s),
            None => EventParam::None,
        };
        if self.display_send_event(d, EventCode::SCREEN_LOAD_START, param) == LvResult::Invalid {
            return LoadScreenResult::DisplayDeleted;
        }

        if let Some(s) = new_scr.get()
            && self.obj_send_event(s, EventCode::SCREEN_LOAD_START, EventParam::None) == LvResult::Invalid
        {
            new_scr.set(None);
        }

        if let Some(dd) = self.display_mut(d) {
            dd.act_scr = new_scr.get().map(|s| s.0);
            dd.scr_to_load = None;
        }

        if let Some(s) = new_scr.get()
            && self.display_send_event(d, EventCode::SCREEN_LOADED, EventParam::Obj(s)) == LvResult::Invalid
        {
            return LoadScreenResult::DisplayDeleted;
        }

        if let Some(s) = new_scr.get()
            && self.obj_send_event(s, EventCode::SCREEN_LOADED, EventParam::None) == LvResult::Invalid
        {
            if let Some(dd) = self.display_mut(d) {
                dd.act_scr = None;
            }
            new_scr.set(None);
        }

        if let Some(o) = old_scr.get() {
            if self.display_send_event(d, EventCode::SCREEN_UNLOADED, EventParam::Obj(o)) == LvResult::Invalid
            {
                return LoadScreenResult::DisplayDeleted;
            }
            if let Some(o) = old_scr.get()
                && self.obj_send_event(o, EventCode::SCREEN_UNLOADED, EventParam::None) == LvResult::Invalid
            {
                old_scr.set(None);
            }
        }

        if let Some(s) = new_scr.get() {
            let _ = self.obj_invalidate(s);
            self.obj_remove_event_cb(s, &delete_cb);
        }

        let Some(o) = old_scr.get() else {
            return if new_scr.get().is_some() {
                LoadScreenResult::OldScreenDeleted
            } else {
                LoadScreenResult::BothScreensDeleted
            };
        };
        self.obj_remove_event_cb(o, &delete_cb);
        if new_scr.get().is_some() {
            LoadScreenResult::Ok
        } else {
            LoadScreenResult::NewScreenDeleted
        }
    }

    /// `scr_load_anim_start()`: the `start_cb` of the new screen's animation.
    fn scr_load_anim_start(&mut self, a: AnimId) {
        let Some(AnimTarget::Obj(var)) = self.anim(a).map(|a| a.var) else {
            return;
        };
        let Some(d) = self.obj_get_display(var) else {
            return;
        };
        let Some(dd) = self.display_mut(d) else {
            return;
        };

        dd.prev_scr = dd.act_scr;
        dd.act_scr = Some(var.0);

        self.obj_send_event(var, EventCode::SCREEN_LOAD_START, EventParam::None);
    }

    /// `scr_anim_completed()`: the `completed_cb` of the new screen's animation.
    fn scr_anim_completed(&mut self, a: AnimId) {
        let Some(AnimTarget::Obj(var)) = self.anim(a).map(|a| a.var) else {
            return;
        };
        let Some(d) = self.obj_get_display(var) else {
            return;
        };

        // `lv_obj_send_event(NULL, ...)` fails the object check and returns
        // `LV_RESULT_INVALID`.
        let act = self
            .display(d)
            .and_then(|dd| dd.act_scr)
            .and_then(|id| self.resolve(id));
        let res = match act {
            Some(o) => self.obj_send_event(o, EventCode::SCREEN_LOADED, EventParam::None),
            None => LvResult::Invalid,
        };
        if res == LvResult::Invalid
            && let Some(dd) = self.display_mut(d)
        {
            dd.act_scr = None;
        }

        let prev = self
            .display(d)
            .and_then(|dd| dd.prev_scr)
            .and_then(|id| self.resolve(id));
        if let Some(p) = prev
            && self.obj_send_event(p, EventCode::SCREEN_UNLOADED, EventParam::None) != LvResult::Invalid
        {
            let del_prev = self.display(d).is_some_and(|dd| dd.del_prev);
            if del_prev
                && let Some(p) = self
                    .display(d)
                    .and_then(|dd| dd.prev_scr)
                    .and_then(|id| self.resolve(id))
            {
                self.obj_delete(p);
            }
        }
        let Some(dd) = self.display_mut(d) else {
            return;
        };
        dd.prev_scr = None;
        dd.draw_prev_over_act = false;
        dd.scr_to_load = None;
        let act = dd.act_scr;
        if self.resolve(var.0).is_some() {
            self.obj_remove_local_style_prop(var, StyleProp::OPA, StyleSelector::DEFAULT);
        }
        self.invalidate_opt(act);
    }
}

/// `old_screen_deleted()`
fn old_screen_deleted(res: LoadScreenResult) -> bool {
    res == LoadScreenResult::BothScreensDeleted || res == LoadScreenResult::OldScreenDeleted
}

/// `new_screen_deleted()`
fn new_screen_deleted(res: LoadScreenResult) -> bool {
    res == LoadScreenResult::BothScreensDeleted || res == LoadScreenResult::NewScreenDeleted
}

/// `invalidate_layout_cb()`
fn invalidate_layout_cb(ui: &mut Ui, obj: Obj) -> ObjTreeWalkRes {
    ui.obj_mark_layout_as_dirty(obj);
    ObjTreeWalkRes::Next
}

/// `screen_event_delete_cb()`: clear the `lv_obj_t *` variable the user data
/// points to.
fn screen_event_delete_cb(_ui: &mut Ui, e: &mut Event<'_>) {
    if let Some(screen_var) = e.user_data().and_then(|u| u.downcast_ref::<Cell<Option<Obj>>>()) {
        screen_var.set(None);
    }
}

/// `opa_scale_anim()`: `lv_obj_set_style_opa(obj, v, 0)` (`v` is passed as
/// `lv_opa_t`).
fn opa_scale_anim() -> AnimExec {
    AnimExec::Custom {
        tag: ANIM_TAG_OPA_SCALE,
        f: Rc::new(|ui: &mut Ui, var: AnimTarget, v: i32| {
            if let AnimTarget::Obj(obj) = var {
                ui.obj_set_local_style_prop(
                    obj,
                    StyleProp::OPA,
                    StyleValue::Num(v as u8 as i32),
                    StyleSelector::DEFAULT,
                );
            }
        }),
    }
}

/// `set_x_anim()`: `lv_obj_set_x(obj, v)`.
fn set_x_anim() -> AnimExec {
    AnimExec::Custom {
        tag: ANIM_TAG_SET_X,
        f: Rc::new(|ui: &mut Ui, var: AnimTarget, v: i32| {
            if let AnimTarget::Obj(obj) = var {
                ui.obj_set_x(obj, v);
            }
        }),
    }
}

/// `set_y_anim()`: `lv_obj_set_y(obj, v)`.
fn set_y_anim() -> AnimExec {
    AnimExec::Custom {
        tag: ANIM_TAG_SET_Y,
        f: Rc::new(|ui: &mut Ui, var: AnimTarget, v: i32| {
            if let AnimTarget::Obj(obj) = var {
                ui.obj_set_y(obj, v);
            }
        }),
    }
}

/// `is_out_anim()`
fn is_out_anim(anim_type: ScreenLoadAnim) -> bool {
    anim_type == ScreenLoadAnim::FadeOut
        || anim_type == ScreenLoadAnim::OutLeft
        || anim_type == ScreenLoadAnim::OutRight
        || anim_type == ScreenLoadAnim::OutTop
        || anim_type == ScreenLoadAnim::OutBottom
}

/// `disp_event_cb()`: `LV_EVENT_REFR_REQUEST` resumes the refresh timer.
fn disp_event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    debug_assert!(e.code() == EventCode::REFR_REQUEST);

    let EventTarget::Display(disp) = e.target() else {
        return;
    };

    if let Some(t) = ui.display(disp).and_then(|d| d.refr_timer) {
        ui.timer_resume(t);
    }
}

/// `LV_DPX_CALC(dpi, n)`
#[inline]
pub const fn dpx_calc(dpi: i32, n: i32) -> i32 {
    if n == 0 {
        0
    } else {
        let v = (dpi * n + 80) / 160;
        if v > 1 { v } else { 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::UiConfig;
    use alloc::string::String;
    use core::cell::RefCell;

    #[test]
    fn enum_values_and_dpx() {
        assert_eq!(Rotation::R270 as u8, 3);
        assert_eq!(RenderMode::Full as u8, 2);
        assert_eq!(ScreenLoadAnim::FadeIn as u8, 9);
        assert_eq!(ScreenLoadAnim::FADE_ON, ScreenLoadAnim::FadeIn);
        assert_eq!(ScreenLoadAnim::OutBottom as u8, 14);
        assert_eq!(dpx_calc(130, 0), 0);
        assert_eq!(dpx_calc(130, 1), 1);
        assert_eq!(dpx_calc(160, 100), 100);
        assert_eq!(dpx_calc(130, 100), 81);
    }

    /// `lv_display_create()` without the parts that need the object tree
    /// (screens, layers, theme): the display data, the list order, the
    /// draw layer, the refresh timer and the internal `disp_event_cb`.
    fn add_display(ui: &mut Ui, hor_res: i32, ver_res: i32) -> DisplayId {
        let dpi = ui.config().dpi_def;
        let n = ui.config().inv_buf_size;
        let disp = DisplayId(ui.displays.insert(DisplayData::new(hor_res, ver_res, dpi, n)));
        ui.display_order.insert(0, disp);
        let area = Area::new(0, 0, hor_res - 1, ver_res - 1);
        let orig = ui.refr_get_disp_refreshing();
        ui.refr_set_disp_refreshing(Some(disp));
        let layer = ui.draw_layer_create(None, COLOR_FORMAT_DEFAULT, &area);
        ui.refr_set_disp_refreshing(orig);
        let tick = ui.tick_get();
        let cb: TimerCallback = Rc::new(|_: &mut Ui, _: TimerId| {});
        let timer = ui.timer_create(cb, ui.config().refr_period_ms, None);
        {
            let d = ui.display_mut(disp).unwrap();
            d.layer_head = Some(layer);
            d.inv_en_cnt = 1;
            d.last_activity_time = tick;
            d.refr_timer = Some(timer);
        }
        if ui.disp_default.is_none() {
            ui.disp_default = Some(disp);
        }
        ui.display_add_event_cb(disp, Rc::new(disp_event_cb), EventCode::REFR_REQUEST, None);
        disp
    }

    fn new_ui() -> Ui {
        Ui::new(UiConfig::default())
    }

    fn logger(log: &Rc<RefCell<String>>, c: char) -> EventCallback {
        let log = log.clone();
        Rc::new(move |_: &mut Ui, _: &mut Event<'_>| log.borrow_mut().push(c))
    }

    fn ud<T: Any>(v: T) -> Rc<dyn Any> {
        Rc::new(v)
    }

    #[test]
    fn create_defaults() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 480, 320);
        let d = ui.display(disp).unwrap();
        assert_eq!(d.physical_hor_res, -1);
        assert_eq!(d.physical_ver_res, -1);
        assert_eq!(d.inv_areas.len(), INV_BUF_SIZE);
        assert_eq!(d.inv_area_joined.len(), INV_BUF_SIZE);
        assert_eq!(ui.display_get_color_format(Some(disp)), ColorFormat::Xrgb8888);
        assert_eq!(ui.display_get_dpi(Some(disp)), ui.config().dpi_def);
        assert_eq!(ui.display_get_tile_cnt(disp), 1);
        assert!(ui.display_get_antialiasing(Some(disp)));
        assert_eq!(ui.display_get_render_mode(disp), RenderMode::Partial);
        assert!(ui.display_is_invalidation_enabled(Some(disp)));
        assert_eq!(ui.display_get_rotation(Some(disp)), Rotation::R0);
        assert!(ui.display_get_theme(Some(disp)).is_none());
        assert!(ui.display_get_buf_active(disp).is_none());
        assert!(!ui.display_is_double_buffered(disp));
    }

    #[test]
    fn display_list_order_and_default() {
        let mut ui = new_ui();
        assert_eq!(ui.display_get_next(None), None);
        let d1 = add_display(&mut ui, 10, 10);
        let d2 = add_display(&mut ui, 20, 20);
        // lv_ll_ins_head: the newest display is the head.
        assert_eq!(ui.display_get_next(None), Some(d2));
        assert_eq!(ui.display_get_next(Some(d2)), Some(d1));
        assert_eq!(ui.display_get_next(Some(d1)), None);
        assert_eq!(ui.display_get_default(), Some(d1));
        ui.display_set_default(d2);
        assert_eq!(ui.display_get_default(), Some(d2));
        assert_eq!(ui.display_get_horizontal_resolution(None), 20);
    }

    /// `test_display_resolution_full_rotation()` of `test_display.c`.
    fn check_resolution_full_rotation(
        ui: &mut Ui,
        disp: DisplayId,
        ori_hor_res: i32,
        ori_ver_res: i32,
        ori_physical_hor_res: i32,
        ori_physical_ver_res: i32,
        ori_offset_x: i32,
        ori_offset_y: i32,
    ) {
        for rot in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
            // `lv_display_set_rotation()` also runs `update_resolution()`,
            // which needs the object tree.
            ui.display_mut(disp).unwrap().rotation = rot;
            assert_eq!(ui.display_get_rotation(Some(disp)), rot);
            assert_eq!(
                ui.display_get_original_horizontal_resolution(Some(disp)),
                ori_hor_res
            );
            assert_eq!(
                ui.display_get_original_vertical_resolution(Some(disp)),
                ori_ver_res
            );
            let d = Some(disp);
            match rot {
                Rotation::R0 | Rotation::R180 => {
                    assert_eq!(
                        ui.display_get_physical_horizontal_resolution(d),
                        ori_physical_hor_res
                    );
                    assert_eq!(
                        ui.display_get_physical_vertical_resolution(d),
                        ori_physical_ver_res
                    );
                    assert_eq!(ui.display_get_horizontal_resolution(d), ori_hor_res);
                    assert_eq!(ui.display_get_vertical_resolution(d), ori_ver_res);
                }
                Rotation::R90 | Rotation::R270 => {
                    assert_eq!(
                        ui.display_get_physical_horizontal_resolution(d),
                        ori_physical_ver_res
                    );
                    assert_eq!(
                        ui.display_get_physical_vertical_resolution(d),
                        ori_physical_hor_res
                    );
                    assert_eq!(ui.display_get_horizontal_resolution(d), ori_ver_res);
                    assert_eq!(ui.display_get_vertical_resolution(d), ori_hor_res);
                }
            }
            match rot {
                Rotation::R0 => {
                    assert_eq!(ui.display_get_offset_x(d), ori_offset_x);
                    assert_eq!(ui.display_get_offset_y(d), ori_offset_y);
                }
                Rotation::R180 => {
                    assert_eq!(ui.display_get_offset_x(d), ori_physical_hor_res - ori_offset_x);
                    assert_eq!(ui.display_get_offset_y(d), ori_physical_ver_res - ori_offset_y);
                }
                Rotation::R90 => {
                    assert_eq!(ui.display_get_offset_x(d), ori_offset_y);
                    assert_eq!(ui.display_get_offset_y(d), ori_offset_x);
                }
                Rotation::R270 => {
                    assert_eq!(ui.display_get_offset_x(d), ori_physical_ver_res - ori_offset_y);
                    assert_eq!(ui.display_get_offset_y(d), ori_physical_hor_res - ori_offset_x);
                }
            }
        }
        ui.display_mut(disp).unwrap().rotation = Rotation::R0;
    }

    #[test]
    fn test_display_resolution() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 480, 320);
        check_resolution_full_rotation(&mut ui, disp, 480, 320, 480, 320, 0, 0);

        // `lv_display_set_resolution()` would run `update_resolution()`.
        ui.display_mut(disp).unwrap().hor_res = 320;
        ui.display_mut(disp).unwrap().ver_res = 480;
        ui.display_set_physical_resolution(disp, 240, 320);
        ui.display_set_offset(disp, 10, 20);
        check_resolution_full_rotation(&mut ui, disp, 320, 480, 240, 320, 10, 20);

        /* Set same resolution again: early return */
        ui.display_set_resolution(disp, 320, 480);
        check_resolution_full_rotation(&mut ui, disp, 320, 480, 240, 320, 10, 20);
    }

    #[test]
    fn test_display_dpi_tile_cnt_antialiasing() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 480, 320);
        ui.display_set_default(disp);

        ui.display_set_dpi(disp, 80);
        assert_eq!(ui.display_get_dpi(Some(disp)), 80);
        assert_eq!(ui.dpx(0), dpx_calc(80, 0));
        assert_eq!(ui.dpx(10), dpx_calc(80, 10));
        assert_eq!(ui.display_dpx(Some(disp), 100), dpx_calc(80, 100));
        assert_eq!(ui.dpx(10), 5);

        ui.display_set_antialiasing(disp, false);
        assert!(!ui.display_get_antialiasing(Some(disp)));
        ui.display_set_antialiasing(disp, true);
        assert!(ui.display_get_antialiasing(None));
        ui.display_set_tile_cnt(disp, 10);
        assert_eq!(ui.display_get_tile_cnt(disp), 10);
        ui.display_set_tile_cnt(disp, 255);
        assert_eq!(ui.display_get_tile_cnt(disp), 255);
    }

    #[test]
    fn test_display_rotate_point() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 480, 320);
        let cases = [
            (Rotation::R0, Point::new(10, 20)),
            (Rotation::R90, Point::new(320 - 20 - 1, 10)),
            (Rotation::R180, Point::new(480 - 10 - 1, 320 - 20 - 1)),
            (Rotation::R270, Point::new(20, 480 - 10 - 1)),
        ];
        for (rot, expected) in cases {
            ui.display_mut(disp).unwrap().rotation = rot;
            let mut p = Point::new(10, 20);
            ui.display_rotate_point(disp, &mut p);
            assert_eq!(p, expected, "{rot:?}");
        }
    }

    #[test]
    fn test_display_rotate_point_ccw() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 480, 320);
        let cases = [
            (Rotation::R0, Point::new(10, 20)),
            (Rotation::R90, Point::new(20, 480 - 10 - 1)),
            (Rotation::R180, Point::new(480 - 10 - 1, 320 - 20 - 1)),
            (Rotation::R270, Point::new(320 - 20 - 1, 10)),
        ];
        for (rot, expected) in cases {
            ui.display_mut(disp).unwrap().rotation = rot;
            let mut p = Point::new(10, 20);
            ui.display_rotate_point_ccw(disp, &mut p);
            assert_eq!(p, expected, "{rot:?}");
        }
    }

    #[test]
    fn rotate_area_values() {
        // Expected values computed by hand from the C formulas of
        // `lv_display_rotate_area()` (800x480, area 100x50 at (10, 20)).
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 800, 480);
        for (rot, expected) in [
            (Rotation::R0, Area::new(10, 20, 109, 69)),
            (Rotation::R90, Area::new(20, 370, 69, 469)),
            (Rotation::R180, Area::new(690, 410, 789, 459)),
            (Rotation::R270, Area::new(730, 10, 779, 109)),
        ] {
            ui.display_mut(disp).unwrap().rotation = rot;
            let ori = Area::new(10, 20, 109, 69);
            let mut a = ori;
            ui.display_rotate_area(disp, &mut a);
            assert_eq!(a, expected, "{rot:?}");
            match rot {
                Rotation::R90 | Rotation::R270 => {
                    assert_eq!(a.width(), ori.height());
                    assert_eq!(a.height(), ori.width());
                }
                _ => {
                    assert_eq!(a.width(), ori.width());
                    assert_eq!(a.height(), ori.height());
                }
            }
        }
        ui.display_mut(disp).unwrap().rotation = Rotation::R90;
        let mut a = Area::new(0, 0, 9, 19);
        ui.display_rotate_area(disp, &mut a);
        assert_eq!(a, Area::new(0, 470, 19, 479));
    }

    fn owned(bytes: usize) -> BufStorage {
        BufStorage::Owned(vec![0u64; bytes.div_ceil(8)].into_boxed_slice())
    }

    #[test]
    fn test_get_drawbuf_size() {
        for double in [false, true] {
            let mut ui = new_ui();
            let disp = add_display(&mut ui, 10, 20);
            ui.display_set_color_format(disp, ColorFormat::Rgb888);
            let buf2 = if double { Some(owned(200)) } else { None };
            ui.display_set_buffers(disp, owned(200), buf2, 200, RenderMode::Partial);
            assert_eq!(ui.display_get_draw_buf_size(disp), 200);
            assert_eq!(ui.display_is_double_buffered(disp), double);
            let d = ui.display(disp).unwrap();
            // stride 30 -> 6 lines fit in 200 bytes
            let b1 = d.buf_1.as_ref().unwrap();
            assert_eq!(b1.header.stride, 30);
            assert_eq!(b1.header.h, 6);
            assert_eq!(b1.header.w, 10);
            assert_eq!(b1.header.cf, ColorFormat::Rgb888);
            assert!(d.stride_is_auto);
            assert_eq!(d.buf_act, 1);
        }
    }

    fn exec_invalidated_drawbuf_size_test(
        width: u32,
        height: u32,
        cf: ColorFormat,
        render_mode: RenderMode,
        inv_w: u32,
        inv_h: u32,
        expected: u32,
    ) {
        let buffer_size = DrawBuf::width_to_stride(width, cf) * height;
        let mut ui = new_ui();
        let disp = add_display(&mut ui, width as i32, height as i32);
        ui.display_set_color_format(disp, cf);
        ui.display_set_buffers(disp, owned(buffer_size as usize), None, buffer_size, render_mode);
        assert_eq!(
            ui.display_get_invalidated_draw_buf_size(disp, inv_w, inv_h),
            expected
        );
    }

    #[test]
    fn test_get_invalidated_drawbuf_size() {
        let rgb = ColorFormat::Rgb888;
        let i1 = ColorFormat::I1;
        exec_invalidated_drawbuf_size_test(
            10,
            20,
            rgb,
            RenderMode::Partial,
            5,
            5,
            DrawBuf::width_to_stride(5, rgb) * 5,
        );
        exec_invalidated_drawbuf_size_test(
            10,
            20,
            rgb,
            RenderMode::Full,
            10,
            20,
            DrawBuf::width_to_stride(10, rgb) * 20,
        );
        exec_invalidated_drawbuf_size_test(
            180,
            90,
            i1,
            RenderMode::Full,
            180,
            90,
            DrawBuf::width_to_stride(180, i1) * 90,
        );
        exec_invalidated_drawbuf_size_test(
            180,
            90,
            i1,
            RenderMode::Partial,
            180,
            10,
            DrawBuf::width_to_stride(180, i1) * 10,
        );
        // FULL mode ignores the requested size.
        exec_invalidated_drawbuf_size_test(
            10,
            20,
            rgb,
            RenderMode::Full,
            1,
            1,
            DrawBuf::width_to_stride(10, rgb) * 20,
        );
    }

    #[test]
    fn test_display_buffers_with_stride() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 32, 64);
        let cf = ColorFormat::Argb8888;
        ui.display_set_color_format(disp, cf);
        let size = DrawBuf::width_to_stride(32, cf) * 64;
        let s1: Rc<[u8]> = Rc::from(vec![0u8; size as usize]);
        let s2: Rc<[u8]> = Rc::from(vec![0u8; size as usize]);
        let p1 = s1.as_ptr();
        let p2 = s2.as_ptr();
        for (stride, mode) in [
            (STRIDE_AUTO, RenderMode::Partial),
            (32 * 4, RenderMode::Partial),
            (32 * 4, RenderMode::Direct),
        ] {
            ui.display_set_buffers_with_stride(
                disp,
                BufStorage::Shared(s1.clone()),
                Some(BufStorage::Shared(s2.clone())),
                size,
                stride,
                mode,
            );
            let d = ui.display(disp).unwrap();
            assert_eq!(d.buf_1.as_ref().unwrap().data().as_ptr(), p1);
            assert_eq!(d.buf_2.as_ref().unwrap().data().as_ptr(), p2);
            assert_eq!(d.render_mode, mode);
            assert_eq!(d.stride_is_auto, stride == STRIDE_AUTO);
            assert_eq!(d.buf_1.as_ref().unwrap().header.h, 64);
        }
    }

    #[cfg(feature = "check-arg-panic")]
    #[test]
    #[should_panic(expected = "buf1 is not properly aligned")]
    fn unaligned_buffer_is_rejected() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 4, 4);
        let leaked: &'static [u8] = Box::leak(vec![0u8; 256].into_boxed_slice());
        let start = if (leaked.as_ptr() as usize).is_multiple_of(4) {
            1
        } else {
            0
        };
        ui.display_set_buffers(
            disp,
            BufStorage::Static(&leaked[start..]),
            None,
            64,
            RenderMode::Partial,
        );
    }

    #[cfg(feature = "check-arg-panic")]
    #[test]
    #[should_panic(expected = "requires screen sized buffer")]
    fn full_mode_needs_screen_sized_buffer() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 10, 10);
        ui.display_set_buffers(disp, owned(64), None, 64, RenderMode::Full);
    }

    #[test]
    fn test_display_triple_buffer_setup() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 48, 32);
        let mk = || DrawBuf::create(48, 32, COLOR_FORMAT_DEFAULT, 0).unwrap();
        ui.display_set_render_mode(disp, RenderMode::Direct);
        ui.display_set_draw_buffers(disp, Some(mk()), Some(mk()));
        ui.display_set_3rd_draw_buffer(disp, Some(mk()));
        let d = ui.display(disp).unwrap();
        assert!(d.buf_3.is_some());
        assert!(!d.stride_is_auto);
        let act = ui.display_get_buf_active(disp).unwrap() as *const DrawBuf;
        assert_eq!(act, d.buf_1.as_ref().unwrap() as *const DrawBuf);
        assert!(ui.display_is_double_buffered(disp));
        assert_eq!(ui.display_get_render_mode(disp), RenderMode::Direct);
    }

    #[test]
    fn color_format_change_updates_buffers_and_layer() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 8, 8);
        let log = Rc::new(RefCell::new(String::new()));
        ui.display_add_event_cb(disp, logger(&log, 'c'), EventCode::COLOR_FORMAT_CHANGED, None);
        let mk = || DrawBuf::create(8, 8, COLOR_FORMAT_DEFAULT, 0).unwrap();
        ui.display_set_draw_buffers(disp, Some(mk()), Some(mk()));
        ui.display_set_color_format(disp, ColorFormat::Rgb565);
        let d = ui.display(disp).unwrap();
        assert_eq!(d.buf_1.as_ref().unwrap().header.cf, ColorFormat::Rgb565);
        assert_eq!(d.buf_2.as_ref().unwrap().header.cf, ColorFormat::Rgb565);
        let layer = d.layer_head.unwrap();
        assert_eq!(ui.draw.layer(layer).unwrap().color_format, ColorFormat::Rgb565);
        assert_eq!(&*log.borrow(), "c");
    }

    #[test]
    #[cfg(not(feature = "check-arg-panic"))]
    fn create_ex_rejects_non_positive_resolution() {
        // LV_CHECK_ARG(hor_res > 0, return NULL): nothing is created and the
        // default display is unchanged (no object is created, so this does
        // not reach `obj_create`).
        let mut ui = new_ui();
        assert_eq!(ui.display_create_ex(0, 480, None), None);
        assert_eq!(ui.display_create_ex(800, -1, None), None);
        assert_eq!(ui.display_get_next(None), None);
        assert_eq!(ui.display_get_default(), None);
        assert!(ui.displays.is_empty());

        let d1 = add_display(&mut ui, 10, 10);
        assert_eq!(ui.display_create_ex(-5, 10, None), None);
        assert_eq!(ui.display_get_next(None), Some(d1));
        assert_eq!(ui.display_get_next(Some(d1)), None);
        assert_eq!(ui.display_get_default(), Some(d1));
    }

    #[test]
    #[cfg(feature = "check-arg-panic")]
    #[should_panic(expected = "hor_res > 0")]
    fn create_ex_rejects_non_positive_resolution() {
        let mut ui = new_ui();
        let _ = ui.display_create_ex(0, 480, None);
    }

    #[test]
    fn external_data_free_cb() {
        /* The data is a counter the callback bumps. */
        fn free_cb(data: Option<Box<dyn Any>>) {
            if let Some(c) = data.as_ref().and_then(|d| d.downcast_ref::<Rc<Cell<u32>>>()) {
                c.set(c.get() + 1);
            }
        }

        let mut ui = new_ui();
        let disp = add_display(&mut ui, 10, 10);
        {
            let d = ui.display(disp).unwrap();
            assert!(d.ext_data.data.is_none());
            assert!(d.ext_data.free_cb.is_none());
        }
        let freed = Rc::new(Cell::new(0u32));
        // Without a free_cb nothing is called.
        ui.display_set_external_data(disp, Some(Box::new(freed.clone())), None);
        ui.display_ext_data_free(disp);
        assert_eq!(freed.get(), 0);

        ui.display_set_external_data(disp, Some(Box::new(freed.clone())), Some(free_cb));
        assert!(ui.display(disp).unwrap().ext_data.free_cb.is_some());
        ui.display_ext_data_free(disp);
        assert_eq!(freed.get(), 1);
        // `data = NULL` after the call.
        assert!(ui.display(disp).unwrap().ext_data.data.is_none());
    }

    #[test]
    fn test_display_delete_refr_timer() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 48, 32);
        let t = ui.display_get_refr_timer(Some(disp)).unwrap();
        assert!(ui.timer(t).is_some());
        ui.display_delete_refr_timer(Some(disp));
        assert!(ui.display_get_refr_timer(Some(disp)).is_none());
        assert!(ui.timer(t).is_none());
        // A second delete is a no-op.
        ui.display_delete_refr_timer(Some(disp));
    }

    #[test]
    fn refr_request_resumes_refr_timer() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 48, 32);
        let t = ui.display_get_refr_timer(Some(disp)).unwrap();
        ui.timer_pause(t);
        assert!(ui.timer(t).unwrap().paused);
        assert_eq!(
            ui.display_send_event(disp, EventCode::REFR_REQUEST, EventParam::None),
            LvResult::Ok
        );
        assert!(!ui.timer(t).unwrap().paused);
    }

    #[test]
    fn test_display_active_time() {
        let mut ui = new_ui();
        let d1 = add_display(&mut ui, 10, 10);
        ui.display_trigger_activity(Some(d1));
        ui.tick_inc(1000);
        assert_eq!(ui.display_get_inactive_time(Some(d1)), 1000);

        ui.display_trigger_activity(None);
        ui.tick_inc(1000);
        assert_eq!(ui.display_get_inactive_time(ui.display_get_default()), 1000);

        let d2 = add_display(&mut ui, 10, 10);
        ui.tick_inc(300);
        ui.display_trigger_activity(Some(d2));
        ui.tick_inc(200);
        // The smallest inactive time of all displays.
        assert_eq!(ui.display_get_inactive_time(None), 200);
        assert_eq!(ui.display_get_inactive_time(Some(d1)), 1500);
    }

    #[test]
    fn invalidation_counter() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 10, 10);
        ui.display_enable_invalidation(Some(disp), false);
        assert!(!ui.display_is_invalidation_enabled(Some(disp)));
        ui.display_enable_invalidation(Some(disp), false);
        ui.display_enable_invalidation(Some(disp), true);
        assert!(!ui.display_is_invalidation_enabled(Some(disp)));
        ui.display_enable_invalidation(None, true);
        assert!(ui.display_is_invalidation_enabled(None));
    }

    #[test]
    fn flush_and_sync_flags() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 10, 10);
        {
            let d = ui.display_mut(disp).unwrap();
            d.flushing = true;
            d.flushing_last = true;
            d.syncing = true;
            d.syncing_last = true;
        }
        assert!(ui.display_flush_is_last(disp));
        assert!(ui.display_sync_is_last(disp));
        ui.display_flush_ready(disp);
        ui.display_sync_ready(disp);
        let d = ui.display(disp).unwrap();
        assert!(!d.flushing);
        assert!(!d.syncing);
    }

    struct TestDriver {
        events: Rc<RefCell<Vec<EventCode>>>,
    }

    impl DisplayDriver for TestDriver {
        fn flush(&mut self, _disp: DisplayId, _area: &Area, _px_map: &mut [u8]) -> FlushResult {
            FlushResult::Done
        }
        fn on_event(&mut self, code: EventCode) {
            self.events.borrow_mut().push(code);
        }
    }

    #[test]
    fn driver_and_user_data() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 10, 10);
        assert!(ui.display_driver_mut(disp).is_none());
        let events = Rc::new(RefCell::new(Vec::new()));
        ui.display_set_driver(
            disp,
            Some(Box::new(TestDriver {
                events: events.clone(),
            })),
        );
        let mut px = [0u8; 4];
        let r = ui
            .display_driver_mut(disp)
            .unwrap()
            .flush(disp, &Area::new(0, 0, 0, 0), &mut px);
        assert_eq!(r, FlushResult::Done);
        ui.display_send_event(disp, EventCode::RESOLUTION_CHANGED, EventParam::None);
        assert_eq!(&*events.borrow(), &[EventCode::RESOLUTION_CHANGED]);

        let u = ud(5u32);
        ui.display_set_user_data(disp, Some(u.clone()));
        assert!(Rc::ptr_eq(&ui.display_get_user_data(disp).unwrap(), &u));
        ui.display_set_driver_data(disp, Some(Box::new(7i32)));
        let dd = ui.display_get_driver_data(disp).unwrap();
        *dd.downcast_mut::<i32>().unwrap() += 1;
        assert_eq!(
            ui.display_get_driver_data(disp).unwrap().downcast_ref::<i32>(),
            Some(&8)
        );
    }

    #[test]
    fn set_theme_without_screens_only_stores() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 10, 10);
        let th = ThemeId(ui.themes.insert(crate::theme::Theme::new()));
        ui.display_set_theme(disp, Some(th));
        assert_eq!(ui.display_get_theme(Some(disp)), Some(th));
        ui.display_set_theme(disp, None);
        assert_eq!(ui.display_get_theme(None), None);
    }

    #[test]
    fn screen_getters_without_screens() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 10, 10);
        assert!(ui.screen_active().is_none());
        assert!(ui.layer_top().is_none());
        assert!(ui.layer_sys().is_none());
        assert!(ui.layer_bottom().is_none());
        assert!(ui.display_get_screen_prev(Some(disp)).is_none());
        assert!(ui.display_get_screen_loading(Some(disp)).is_none());
        assert!(ui.display_get_screen_by_name(Some(disp), "main").is_none());
    }

    #[test]
    fn screen_load_anim_helpers() {
        use ScreenLoadAnim as A;
        let out = [A::FadeOut, A::OutLeft, A::OutRight, A::OutTop, A::OutBottom];
        for a in [
            A::None,
            A::OverLeft,
            A::OverRight,
            A::OverTop,
            A::OverBottom,
            A::MoveLeft,
            A::MoveRight,
            A::MoveTop,
            A::MoveBottom,
            A::FadeIn,
            A::FadeOut,
            A::OutLeft,
            A::OutRight,
            A::OutTop,
            A::OutBottom,
        ] {
            assert_eq!(is_out_anim(a), out.contains(&a), "{a:?}");
        }
        use LoadScreenResult as R;
        assert!(!old_screen_deleted(R::Ok) && !new_screen_deleted(R::Ok));
        assert!(old_screen_deleted(R::OldScreenDeleted) && !new_screen_deleted(R::OldScreenDeleted));
        assert!(!old_screen_deleted(R::NewScreenDeleted) && new_screen_deleted(R::NewScreenDeleted));
        assert!(old_screen_deleted(R::BothScreensDeleted) && new_screen_deleted(R::BothScreensDeleted));
        assert!(!old_screen_deleted(R::DisplayDeleted) && !new_screen_deleted(R::DisplayDeleted));
        // The exec callbacks keep distinct identities.
        assert!(set_x_anim() == set_x_anim());
        assert!(set_x_anim() != set_y_anim());
        assert!(set_x_anim() != AnimExec::ObjX);
        assert!(opa_scale_anim() != set_y_anim());
    }

    #[test]
    fn screen_event_delete_cb_clears_variable() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 10, 10);
        let var: Rc<Cell<Option<Obj>>> = Rc::new(Cell::new(Some(Obj(ObjId(disp.0)))));
        let u: Rc<dyn Any> = var.clone();
        ui.display_add_event_cb(disp, Rc::new(screen_event_delete_cb), EventCode::DELETE, Some(u));
        ui.display_send_event(disp, EventCode::DELETE, EventParam::None);
        assert!(var.get().is_none());
    }

    /* test_display_event.c */

    #[test]
    fn test_display_event_fresh_display_has_internal_cb() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 200, 100);
        assert!(ui.display_get_event_count(disp) > 0);
        assert!(ui.display_get_event_dsc(disp, 0).is_some());
    }

    #[test]
    fn test_display_event_add_and_query_dsc() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 200, 100);
        let base_cnt = ui.display_get_event_count(disp);
        let log = Rc::new(RefCell::new(String::new()));
        let cb_a = logger(&log, 'a');
        let cb_b = logger(&log, 'b');
        let ud_a = ud(0i32);
        let ud_b = ud(0i32);
        let dsc_a = ui.display_add_event_cb(disp, cb_a.clone(), EventCode::REFR_START, Some(ud_a.clone()));
        let dsc_b = ui.display_add_event_cb(disp, cb_b.clone(), EventCode::ALL, Some(ud_b.clone()));
        assert_ne!(dsc_a, dsc_b);
        assert_eq!(ui.display_get_event_count(disp), base_cnt + 2);
        let a = ui.display_get_event_dsc(disp, base_cnt).unwrap();
        assert!(Rc::ptr_eq(&a.cb, &cb_a));
        assert!(Rc::ptr_eq(a.user_data.as_ref().unwrap(), &ud_a));
        let b = ui.display_get_event_dsc(disp, base_cnt + 1).unwrap();
        assert!(Rc::ptr_eq(&b.cb, &cb_b));
        assert!(Rc::ptr_eq(&ui.event_dsc_get_user_data(dsc_b).unwrap(), &ud_b));
        assert!(ui.display_get_event_dsc(disp, base_cnt + 2).is_none());
    }

    #[test]
    fn test_display_event_filter() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 200, 100);
        let log = Rc::new(RefCell::new(String::new()));
        ui.display_add_event_cb(disp, logger(&log, 'a'), EventCode::RESOLUTION_CHANGED, None);
        ui.display_add_event_cb(disp, logger(&log, 'b'), EventCode::ALL, None);
        ui.display_send_event(disp, EventCode::RESOLUTION_CHANGED, EventParam::None);
        assert_eq!(&*log.borrow(), "ab");
        ui.display_send_event(disp, EventCode::COLOR_FORMAT_CHANGED, EventParam::None);
        assert_eq!(&*log.borrow(), "abb");
    }

    #[test]
    fn test_display_event_target_and_preprocess() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 200, 100);
        let log = Rc::new(RefCell::new(String::new()));
        let seen = Rc::new(Cell::new(None));
        let s = seen.clone();
        let rec: EventCallback = Rc::new(move |_: &mut Ui, e: &mut Event<'_>| {
            s.set(Some((e.target(), e.current_target(), e.code())));
        });
        ui.display_add_event_cb(disp, rec, EventCode::REFR_START, None);
        ui.display_add_event_cb(
            disp,
            logger(&log, 'A'),
            EventFilter::new(EventCode::REFR_START, true),
            None,
        );
        ui.display_send_event(disp, EventCode::REFR_START, EventParam::None);
        assert_eq!(
            seen.get(),
            Some((
                EventTarget::Display(disp),
                EventTarget::Display(disp),
                EventCode::REFR_START
            ))
        );
        assert_eq!(&*log.borrow(), "A");
    }

    #[test]
    fn test_display_event_remove_by_index() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 200, 100);
        let base_cnt = ui.display_get_event_count(disp);
        let log = Rc::new(RefCell::new(String::new()));
        ui.display_add_event_cb(disp, logger(&log, 'a'), EventCode::REFR_START, None);
        ui.display_add_event_cb(disp, logger(&log, 'b'), EventCode::REFR_START, None);
        assert!(ui.display_remove_event(disp, base_cnt));
        assert_eq!(ui.display_get_event_count(disp), base_cnt + 1);
        ui.display_send_event(disp, EventCode::REFR_START, EventParam::None);
        assert_eq!(&*log.borrow(), "b");
        assert!(!ui.display_remove_event(disp, base_cnt + 1));
        assert_eq!(ui.display_get_event_count(disp), base_cnt + 1);
    }

    #[test]
    fn test_display_event_remove_cb_with_user_data() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 200, 100);
        let base_cnt = ui.display_get_event_count(disp);
        let log = Rc::new(RefCell::new(String::new()));
        let cb_a = logger(&log, 'a');
        let cb_b = logger(&log, 'b');
        let ud1 = ud(1i32);
        let ud2 = ud(2i32);
        ui.display_add_event_cb(disp, cb_a.clone(), EventCode::REFR_START, Some(ud1.clone()));
        ui.display_add_event_cb(disp, cb_a.clone(), EventCode::REFR_READY, Some(ud1.clone()));
        ui.display_add_event_cb(disp, cb_a.clone(), EventCode::REFR_START, Some(ud2.clone()));
        ui.display_add_event_cb(disp, cb_b.clone(), EventCode::REFR_START, Some(ud1.clone()));

        assert_eq!(ui.display_remove_event_cb_with_user_data(disp, &cb_a, &ud1), 2);
        assert_eq!(ui.display_get_event_count(disp), base_cnt + 2);
        ui.display_send_event(disp, EventCode::REFR_START, EventParam::None);
        assert_eq!(&*log.borrow(), "ab");
        assert_eq!(ui.display_remove_event_cb_with_user_data(disp, &cb_a, &ud1), 0);
    }

    #[test]
    fn test_display_event_remove_own_cb_while_sending() {
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 200, 100);
        let base_cnt = ui.display_get_event_count(disp);
        let log = Rc::new(RefCell::new(String::new()));
        let cnt = Rc::new(Cell::new(0u32));
        let slot: Rc<RefCell<Option<EventCallback>>> = Rc::new(RefCell::new(None));
        let c = cnt.clone();
        let sl = slot.clone();
        let remove_self: EventCallback = Rc::new(move |ui: &mut Ui, e: &mut Event<'_>| {
            c.set(c.get() + 1);
            let EventTarget::Display(d) = e.current_target() else {
                panic!()
            };
            let me = sl.borrow().clone().unwrap();
            ui.display_remove_event_cb_with_user_data_opt(d, &me, None);
        });
        *slot.borrow_mut() = Some(remove_self.clone());
        ui.display_add_event_cb(disp, remove_self, EventCode::REFR_START, None);
        ui.display_add_event_cb(disp, logger(&log, 'a'), EventCode::REFR_START, None);

        ui.display_send_event(disp, EventCode::REFR_START, EventParam::None);
        assert_eq!(cnt.get(), 1);
        assert_eq!(&*log.borrow(), "a");
        assert_eq!(ui.display_get_event_count(disp), base_cnt + 1);

        ui.display_send_event(disp, EventCode::REFR_START, EventParam::None);
        assert_eq!(cnt.get(), 1);
        assert_eq!(&*log.borrow(), "aa");
        *slot.borrow_mut() = None;
    }

    struct VsyncFixture {
        ui: Ui,
        disp: DisplayId,
        log: Rc<RefCell<String>>,
        cnt: Rc<Cell<u32>>,
        last_user_data: Rc<RefCell<Option<Rc<dyn Any>>>>,
        last_code: Rc<Cell<EventCode>>,
    }

    fn vsync_fixture() -> VsyncFixture {
        let mut ui = new_ui();
        let _other = add_display(&mut ui, 10, 10);
        let disp = add_display(&mut ui, 200, 100);
        let log = Rc::new(RefCell::new(String::new()));
        let l = log.clone();
        // cb_vsync_request: '+' when asked to start, '-' when asked to stop.
        let req: EventCallback = Rc::new(move |_: &mut Ui, e: &mut Event<'_>| {
            let start = !matches!(e.param(), EventParam::None);
            l.borrow_mut().push(if start { '+' } else { '-' });
        });
        ui.display_add_event_cb(disp, req, EventCode::VSYNC_REQUEST, None);
        VsyncFixture {
            ui,
            disp,
            log,
            cnt: Rc::new(Cell::new(0)),
            last_user_data: Rc::new(RefCell::new(None)),
            last_code: Rc::new(Cell::new(EventCode::ALL)),
        }
    }

    impl VsyncFixture {
        fn cb_record(&self) -> EventCallback {
            let cnt = self.cnt.clone();
            let lud = self.last_user_data.clone();
            let lc = self.last_code.clone();
            Rc::new(move |_: &mut Ui, e: &mut Event<'_>| {
                cnt.set(cnt.get() + 1);
                lc.set(e.code());
                *lud.borrow_mut() = e.user_data().cloned();
            })
        }
        fn cb_count(&self) -> EventCallback {
            let cnt = self.cnt.clone();
            Rc::new(move |_: &mut Ui, _: &mut Event<'_>| cnt.set(cnt.get() + 1))
        }
    }

    #[test]
    fn test_display_event_vsync_register_and_send() {
        let mut f = vsync_fixture();
        let disp = f.disp;
        let user_data = ud(0i32);
        let param = 0i32;
        assert_eq!(
            f.ui.display_send_vsync_event(disp, EventParam::Any(&param)),
            LvResult::Invalid
        );
        let rec = f.cb_record();
        assert!(f.ui.display_register_vsync_event(disp, rec, Some(user_data.clone())));
        assert_eq!(&*f.log.borrow(), "+");
        let cnt_cb = f.cb_count();
        assert!(f.ui.display_register_vsync_event(disp, cnt_cb, Some(user_data.clone())));
        assert_eq!(&*f.log.borrow(), "+");
        assert_eq!(
            f.ui.display_send_vsync_event(disp, EventParam::Any(&param)),
            LvResult::Ok
        );
        assert_eq!(f.cnt.get(), 2);
        assert_eq!(f.last_code.get(), EventCode::VSYNC);
        assert!(Rc::ptr_eq(
            f.last_user_data.borrow().as_ref().unwrap(),
            &user_data
        ));
    }

    #[test]
    fn test_display_event_vsync_unregister() {
        let mut f = vsync_fixture();
        let disp = f.disp;
        let user_data = ud(0i32);
        let other = ud(1i32);
        let rec = f.cb_record();
        let cnt_cb = f.cb_count();
        let never: EventCallback = Rc::new(|_: &mut Ui, _: &mut Event<'_>| {});
        f.ui.display_register_vsync_event(disp, rec.clone(), Some(user_data.clone()));
        f.ui.display_register_vsync_event(disp, cnt_cb.clone(), Some(user_data.clone()));
        assert_eq!(&*f.log.borrow(), "+");

        assert!(
            !f.ui
                .display_unregister_vsync_event(disp, &never, Some(&user_data))
        );
        assert!(!f.ui.display_unregister_vsync_event(disp, &rec, Some(&other)));

        assert!(f.ui.display_unregister_vsync_event(disp, &cnt_cb, Some(&user_data)));
        assert_eq!(&*f.log.borrow(), "+");
        assert_eq!(
            f.ui.display_send_vsync_event(disp, EventParam::None),
            LvResult::Ok
        );
        assert_eq!(f.cnt.get(), 1);

        assert!(f.ui.display_unregister_vsync_event(disp, &rec, Some(&user_data)));
        assert_eq!(&*f.log.borrow(), "+-");
        assert_eq!(
            f.ui.display_send_vsync_event(disp, EventParam::None),
            LvResult::Invalid
        );
        assert_eq!(f.cnt.get(), 1);
    }

    #[test]
    fn test_display_event_vsync_default_display() {
        let mut f = vsync_fixture();
        let def = f.ui.display_get_default().unwrap();
        assert_ne!(def, f.disp);
        let base_cnt = f.ui.display_get_event_count(def);
        let cnt_cb = f.cb_count();
        // misc/anim.rs registers on the default display with `None` user data.
        assert!(f.ui.display_register_vsync_event(def, cnt_cb.clone(), None));
        assert_eq!(f.ui.display_get_event_count(def), base_cnt + 1);
        assert_eq!(f.ui.display_send_vsync_event(def, EventParam::None), LvResult::Ok);
        assert_eq!(f.cnt.get(), 1);
        // A different `Rc` with the same behaviour is a different callback.
        let clone_of_behaviour = f.cb_count();
        assert!(
            !f.ui
                .display_unregister_vsync_event(def, &clone_of_behaviour, None)
        );
        assert!(f.ui.display_unregister_vsync_event(def, &cnt_cb, None));
        assert_eq!(f.ui.display_get_event_count(def), base_cnt);
        assert_eq!(
            f.ui.display_send_vsync_event(def, EventParam::None),
            LvResult::Invalid
        );
    }

    #[test]
    fn test_display_event_vsync_removed_with_plain_api() {
        let mut f = vsync_fixture();
        let disp = f.disp;
        let cnt_cb = f.cb_count();
        f.ui.display_register_vsync_event(disp, cnt_cb.clone(), None);
        f.ui.display_remove_event_cb_with_user_data_opt(disp, &cnt_cb, None);
        f.ui.display_send_vsync_event(disp, EventParam::None);
        assert_eq!(f.cnt.get(), 0);
    }

    #[test]
    fn delete_display_from_event_reports_invalid() {
        // A callback that removes the display from the arena and marks the
        // running events as deleted (what `lv_display_delete` does first).
        let mut ui = new_ui();
        let disp = add_display(&mut ui, 10, 10);
        let log = Rc::new(RefCell::new(String::new()));
        let killer: EventCallback = Rc::new(|ui: &mut Ui, e: &mut Event<'_>| {
            let EventTarget::Display(d) = e.current_target() else {
                panic!()
            };
            ui.event_mark_deleted(EventTarget::Display(d));
        });
        ui.display_add_event_cb(disp, killer, EventCode::SCREEN_LOAD_START, None);
        ui.display_add_event_cb(disp, logger(&log, 'x'), EventCode::SCREEN_LOAD_START, None);
        assert_eq!(
            ui.display_send_event(disp, EventCode::SCREEN_LOAD_START, EventParam::None),
            LvResult::Invalid
        );
        assert_eq!(&*log.borrow(), "");
    }
}
