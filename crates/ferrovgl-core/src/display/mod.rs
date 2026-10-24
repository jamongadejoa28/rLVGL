//! Ported from lvgl v9.6.0: `include/lvgl/display/lv_display.h`, `src/display/lv_display_private.h`,
//! `src/display/lv_display.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::{Obj, ObjId};
use crate::draw::LayerId;
use crate::draw::draw_buf::DrawBuf;
use crate::misc::area::{Area, Point};
use crate::misc::arena::Key;
use crate::misc::color::ColorFormat;
use crate::misc::event::{
    EventCallback, EventCode, EventDsc, EventDscId, EventFilter, EventList, EventParam, LvResult,
};
use crate::misc::timer::TimerId;
use crate::theme::ThemeId;

/// `LV_INV_BUF_SIZE`
pub const INV_BUF_SIZE: usize = 32;

/// Handle of a display (`lv_display_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DisplayId(pub Key);

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
    /// `sync_cb` (direct mode): copy `area` from the active to the inactive buffer.
    fn sync(&mut self, _disp: DisplayId, _area: &Area) {}
    /// `sync_wait_cb`
    fn sync_wait(&mut self, _disp: DisplayId) {}
    /// Display events such as `LV_EVENT_RESOLUTION_CHANGED`.
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
    /// `inv_areas` (capacity `inv_buf_size`)
    pub inv_areas: Vec<Area>,
    /// `inv_area_joined`
    pub inv_area_joined: Vec<bool>,
    /// `inv_p`
    pub inv_p: u32,
    /// `inv_en_cnt`
    pub inv_en_cnt: i32,
    /// `layer_head`
    pub layer_head: Option<LayerId>,
    /// `screens`
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
}

impl Ui {
    /// `lv_display_create()`
    pub fn display_create(&mut self, hor_res: i32, ver_res: i32) -> DisplayId {
        todo!()
    }

    /// `lv_display_delete()`
    pub fn display_delete(&mut self, disp: DisplayId) {
        todo!()
    }

    /// `lv_display_set_default()`
    pub fn display_set_default(&mut self, disp: DisplayId) {
        self.disp_default = Some(disp);
    }

    /// `lv_display_get_default()`
    pub fn display_get_default(&self) -> Option<DisplayId> {
        self.disp_default
    }

    /// `lv_display_get_next()`
    pub fn display_get_next(&self, disp: Option<DisplayId>) -> Option<DisplayId> {
        todo!()
    }

    /// `lv_display_set_resolution()`
    pub fn display_set_resolution(&mut self, disp: DisplayId, hor_res: i32, ver_res: i32) {
        todo!()
    }

    /// `lv_display_set_physical_resolution()`
    pub fn display_set_physical_resolution(&mut self, disp: DisplayId, hor_res: i32, ver_res: i32) {
        todo!()
    }

    /// `lv_display_set_offset()`
    pub fn display_set_offset(&mut self, disp: DisplayId, x: i32, y: i32) {
        todo!()
    }

    /// `lv_display_set_rotation()`
    pub fn display_set_rotation(&mut self, disp: DisplayId, rotation: Rotation) {
        todo!()
    }

    /// `lv_display_set_matrix_rotation()`
    pub fn display_set_matrix_rotation(&mut self, disp: DisplayId, enable: bool) {
        todo!()
    }

    /// `lv_display_set_dpi()`
    pub fn display_set_dpi(&mut self, disp: DisplayId, dpi: i32) {
        todo!()
    }

    /// `lv_display_get_horizontal_resolution()`
    pub fn display_get_horizontal_resolution(&self, disp: Option<DisplayId>) -> i32 {
        todo!()
    }

    /// `lv_display_get_vertical_resolution()`
    pub fn display_get_vertical_resolution(&self, disp: Option<DisplayId>) -> i32 {
        todo!()
    }

    /// `lv_display_get_original_horizontal_resolution()`
    pub fn display_get_original_horizontal_resolution(&self, disp: Option<DisplayId>) -> i32 {
        todo!()
    }

    /// `lv_display_get_original_vertical_resolution()`
    pub fn display_get_original_vertical_resolution(&self, disp: Option<DisplayId>) -> i32 {
        todo!()
    }

    /// `lv_display_get_physical_horizontal_resolution()`
    pub fn display_get_physical_horizontal_resolution(&self, disp: Option<DisplayId>) -> i32 {
        todo!()
    }

    /// `lv_display_get_physical_vertical_resolution()`
    pub fn display_get_physical_vertical_resolution(&self, disp: Option<DisplayId>) -> i32 {
        todo!()
    }

    /// `lv_display_get_offset_x()`
    pub fn display_get_offset_x(&self, disp: Option<DisplayId>) -> i32 {
        todo!()
    }

    /// `lv_display_get_offset_y()`
    pub fn display_get_offset_y(&self, disp: Option<DisplayId>) -> i32 {
        todo!()
    }

    /// `lv_display_get_rotation()`
    pub fn display_get_rotation(&self, disp: Option<DisplayId>) -> Rotation {
        todo!()
    }

    /// `lv_display_get_matrix_rotation()`
    pub fn display_get_matrix_rotation(&self, disp: Option<DisplayId>) -> bool {
        todo!()
    }

    /// `lv_display_get_dpi()`
    pub fn display_get_dpi(&self, disp: Option<DisplayId>) -> i32 {
        todo!()
    }

    /// `lv_display_set_buffers()`: `buf_size` is in bytes.
    pub fn display_set_buffers(
        &mut self,
        disp: DisplayId,
        buf1: crate::draw::draw_buf::BufStorage,
        buf2: Option<crate::draw::draw_buf::BufStorage>,
        buf_size: u32,
        render_mode: RenderMode,
    ) {
        todo!()
    }

    /// `lv_display_set_buffers_with_stride()`
    pub fn display_set_buffers_with_stride(
        &mut self,
        disp: DisplayId,
        buf1: crate::draw::draw_buf::BufStorage,
        buf2: Option<crate::draw::draw_buf::BufStorage>,
        buf_size: u32,
        stride: u32,
        render_mode: RenderMode,
    ) {
        todo!()
    }

    /// `lv_display_set_draw_buffers()`
    pub fn display_set_draw_buffers(
        &mut self,
        disp: DisplayId,
        buf1: Option<DrawBuf>,
        buf2: Option<DrawBuf>,
    ) {
        todo!()
    }

    /// `lv_display_set_3rd_draw_buffer()`
    pub fn display_set_3rd_draw_buffer(&mut self, disp: DisplayId, buf3: Option<DrawBuf>) {
        todo!()
    }

    /// `lv_display_set_render_mode()`
    pub fn display_set_render_mode(&mut self, disp: DisplayId, render_mode: RenderMode) {
        todo!()
    }

    /// `lv_display_set_flush_cb()` and friends: install the driver.
    pub fn display_set_driver(&mut self, disp: DisplayId, driver: Option<Box<dyn DisplayDriver>>) {
        todo!()
    }

    /// Borrow the driver.
    pub fn display_driver_mut(&mut self, disp: DisplayId) -> Option<&mut dyn DisplayDriver> {
        todo!()
    }

    /// `lv_display_set_color_format()`
    pub fn display_set_color_format(&mut self, disp: DisplayId, color_format: ColorFormat) {
        todo!()
    }

    /// `lv_display_get_color_format()`
    pub fn display_get_color_format(&self, disp: Option<DisplayId>) -> ColorFormat {
        todo!()
    }

    /// `lv_display_set_tile_cnt()`
    pub fn display_set_tile_cnt(&mut self, disp: DisplayId, tile_cnt: u32) {
        todo!()
    }

    /// `lv_display_get_tile_cnt()`
    pub fn display_get_tile_cnt(&self, disp: DisplayId) -> u32 {
        todo!()
    }

    /// `lv_display_set_antialiasing()`
    pub fn display_set_antialiasing(&mut self, disp: DisplayId, en: bool) {
        todo!()
    }

    /// `lv_display_get_antialiasing()`
    pub fn display_get_antialiasing(&self, disp: Option<DisplayId>) -> bool {
        todo!()
    }

    /// `lv_display_flush_ready()`
    pub fn display_flush_ready(&mut self, disp: DisplayId) {
        todo!()
    }

    /// `lv_display_flush_is_last()`
    pub fn display_flush_is_last(&self, disp: DisplayId) -> bool {
        todo!()
    }

    /// `lv_display_sync_ready()`
    pub fn display_sync_ready(&mut self, disp: DisplayId) {
        todo!()
    }

    /// `lv_display_sync_is_last()`
    pub fn display_sync_is_last(&self, disp: DisplayId) -> bool {
        todo!()
    }

    /// `lv_display_is_double_buffered()`
    pub fn display_is_double_buffered(&self, disp: DisplayId) -> bool {
        todo!()
    }

    /// `lv_display_get_render_mode()`
    pub fn display_get_render_mode(&self, disp: DisplayId) -> RenderMode {
        todo!()
    }

    /// `lv_display_get_screen_active()`
    pub fn display_get_screen_active(&self, disp: Option<DisplayId>) -> Option<Obj> {
        todo!()
    }

    /// `lv_display_get_screen_prev()`
    pub fn display_get_screen_prev(&self, disp: Option<DisplayId>) -> Option<Obj> {
        todo!()
    }

    /// `lv_display_get_screen_loading()`
    pub fn display_get_screen_loading(&self, disp: Option<DisplayId>) -> Option<Obj> {
        todo!()
    }

    /// `lv_display_get_layer_top()`
    pub fn display_get_layer_top(&self, disp: Option<DisplayId>) -> Option<Obj> {
        todo!()
    }

    /// `lv_display_get_layer_sys()`
    pub fn display_get_layer_sys(&self, disp: Option<DisplayId>) -> Option<Obj> {
        todo!()
    }

    /// `lv_display_get_layer_bottom()`
    pub fn display_get_layer_bottom(&self, disp: Option<DisplayId>) -> Option<Obj> {
        todo!()
    }

    /// `lv_display_get_screen_by_name()`
    pub fn display_get_screen_by_name(&self, disp: Option<DisplayId>, screen_name: &str) -> Option<Obj> {
        todo!()
    }

    /// `lv_screen_load()`
    pub fn screen_load(&mut self, scr: Obj) {
        todo!()
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
        todo!()
    }

    /// `lv_screen_active()`
    pub fn screen_active(&self) -> Option<Obj> {
        self.display_get_screen_active(None)
    }

    /// `lv_layer_top()`
    pub fn layer_top(&self) -> Option<Obj> {
        self.display_get_layer_top(None)
    }

    /// `lv_layer_sys()`
    pub fn layer_sys(&self) -> Option<Obj> {
        self.display_get_layer_sys(None)
    }

    /// `lv_layer_bottom()`
    pub fn layer_bottom(&self) -> Option<Obj> {
        self.display_get_layer_bottom(None)
    }

    /// `lv_display_add_event_cb()`
    pub fn display_add_event_cb(
        &mut self,
        disp: DisplayId,
        cb: EventCallback,
        filter: impl Into<EventFilter>,
        user_data: Option<Rc<dyn Any>>,
    ) -> EventDscId {
        todo!()
    }

    /// `lv_display_get_event_count()`
    pub fn display_get_event_count(&self, disp: DisplayId) -> u32 {
        todo!()
    }

    /// `lv_display_get_event_dsc()`
    pub fn display_get_event_dsc(&self, disp: DisplayId, index: u32) -> Option<&EventDsc> {
        todo!()
    }

    /// `lv_display_remove_event()`
    pub fn display_remove_event(&mut self, disp: DisplayId, index: u32) -> bool {
        todo!()
    }

    /// `lv_display_remove_event_cb_with_user_data()`
    pub fn display_remove_event_cb_with_user_data(
        &mut self,
        disp: DisplayId,
        cb: &EventCallback,
        user_data: &Rc<dyn Any>,
    ) -> u32 {
        todo!()
    }

    /// `lv_display_send_event()`
    pub fn display_send_event(
        &mut self,
        disp: DisplayId,
        code: EventCode,
        param: EventParam<'_>,
    ) -> LvResult {
        todo!()
    }

    /// `lv_display_set_theme()`
    pub fn display_set_theme(&mut self, disp: DisplayId, th: Option<ThemeId>) {
        todo!()
    }

    /// `lv_display_get_theme()`
    pub fn display_get_theme(&self, disp: Option<DisplayId>) -> Option<ThemeId> {
        todo!()
    }

    /// `lv_display_get_inactive_time()`
    pub fn display_get_inactive_time(&self, disp: Option<DisplayId>) -> u32 {
        todo!()
    }

    /// `lv_display_trigger_activity()`
    pub fn display_trigger_activity(&mut self, disp: Option<DisplayId>) {
        todo!()
    }

    /// `lv_display_enable_invalidation()`
    pub fn display_enable_invalidation(&mut self, disp: Option<DisplayId>, en: bool) {
        todo!()
    }

    /// `lv_display_is_invalidation_enabled()`
    pub fn display_is_invalidation_enabled(&self, disp: Option<DisplayId>) -> bool {
        todo!()
    }

    /// `lv_display_get_refr_timer()`
    pub fn display_get_refr_timer(&self, disp: Option<DisplayId>) -> Option<TimerId> {
        todo!()
    }

    /// `lv_display_delete_refr_timer()`
    pub fn display_delete_refr_timer(&mut self, disp: Option<DisplayId>) {
        todo!()
    }

    /// `lv_display_register_vsync_event()`
    pub fn display_register_vsync_event(
        &mut self,
        disp: DisplayId,
        cb: EventCallback,
        user_data: Option<Rc<dyn Any>>,
    ) -> bool {
        todo!()
    }

    /// `lv_display_unregister_vsync_event()`
    pub fn display_unregister_vsync_event(
        &mut self,
        disp: DisplayId,
        cb: &EventCallback,
        user_data: Option<&Rc<dyn Any>>,
    ) -> bool {
        todo!()
    }

    /// `lv_display_send_vsync_event()`
    pub fn display_send_vsync_event(&mut self, disp: DisplayId, param: EventParam<'_>) -> LvResult {
        todo!()
    }

    /// `lv_display_set_user_data()`
    pub fn display_set_user_data(&mut self, disp: DisplayId, user_data: Option<Rc<dyn Any>>) {
        todo!()
    }

    /// `lv_display_set_driver_data()`
    pub fn display_set_driver_data(&mut self, disp: DisplayId, driver_data: Option<Box<dyn Any>>) {
        todo!()
    }

    /// `lv_display_get_user_data()`
    pub fn display_get_user_data(&self, disp: DisplayId) -> Option<Rc<dyn Any>> {
        todo!()
    }

    /// `lv_display_get_driver_data()`
    pub fn display_get_driver_data(&mut self, disp: DisplayId) -> Option<&mut dyn Any> {
        todo!()
    }

    /// `lv_display_get_buf_active()`
    pub fn display_get_buf_active(&self, disp: DisplayId) -> Option<&DrawBuf> {
        todo!()
    }

    /// `lv_display_rotate_area()`
    pub fn display_rotate_area(&self, disp: DisplayId, area: &mut Area) {
        todo!()
    }

    /// `lv_display_rotate_point()`
    pub fn display_rotate_point(&self, disp: DisplayId, point: &mut Point) {
        todo!()
    }

    /// `lv_display_rotate_point_ccw()`
    pub fn display_rotate_point_ccw(&self, disp: DisplayId, point: &mut Point) {
        todo!()
    }

    /// `lv_display_get_draw_buf_size()`
    pub fn display_get_draw_buf_size(&self, disp: DisplayId) -> u32 {
        todo!()
    }

    /// `lv_display_get_invalidated_draw_buf_size()`
    pub fn display_get_invalidated_draw_buf_size(&self, disp: DisplayId, width: u32, height: u32) -> u32 {
        todo!()
    }

    /// `lv_dpx()`
    pub fn dpx(&self, n: i32) -> i32 {
        todo!()
    }

    /// `lv_display_dpx()`
    pub fn display_dpx(&self, disp: Option<DisplayId>, n: i32) -> i32 {
        todo!()
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
}
