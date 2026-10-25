//! Ported from lvgl v9.6.0: `src/core/lv_refr.c`
//! (with `include/lvgl/core/lv_refr.h`, `src/core/lv_refr_private.h` and the
//! blur invalidation expansion of `src/core/lv_obj_pos.c`).
//!
//! Invalidation and the refresh pipeline: join areas, render each invalid
//! area (chunked for partial buffers), draw objects into layers, dispatch
//! and flush.
//!
//! Configuration: the parity build (`tests/configs/full.defconfig`) has
//! `LV_DRAW_TRANSFORM_USE_MATRIX = 0`, `LV_USE_PERF_MONITOR = 1` (so the
//! refresh timer is *not* paused after a refresh), `LV_USE_REFR_DEBUG = 0`,
//! no `LV_COLOR_16_SWAP` and `LV_DRAW_SW_COMPLEX = 1`; only those branches
//! are ported.
//!
//! Buffer ownership: upstream points `disp->layer_head->draw_buf` at
//! `disp->buf_act`. Here a [`DrawBuf`] has one owner, so [`Ui::refr_area`]
//! moves the active display buffer (`buf_1` / `buf_2` / `buf_3` selected
//! by `buf_act`) into the display layer's `draw_buf` and `draw_buf_flush()`
//! moves it back into its slot *before* calling the driver (which gets
//! `px_map` borrowed from that slot), so during the flush and the
//! `FLUSH_START` / `FLUSH_FINISH` events `buf_act` is valid as upstream.
//! While an area is being rendered (from `refr_area()` until the flush)
//! the active `DisplayData::buf_n` slot is `None` and the buffer is
//! `layer_head`'s `draw_buf`; `disp_is_double_buffered` accounts for
//! that, and `lv_display_get_buf_active()` must fall back to
//! `layer_head.draw_buf` while the slot is empty (display owner contract).
//!
//! Driver mapping: `flush_cb` is [`DisplayDriver::flush`]
//! ([`FlushResult::Done`] means `lv_display_flush_ready()` was already
//! called), `flush_wait_cb` is [`DisplayDriver::flush_wait`] (always present
//! when a driver is installed, because a single-threaded `while(flushing);`
//! could never end), `sync_cb` / `sync_wait_cb` are
//! [`DisplayDriver::sync`] / [`DisplayDriver::sync_wait`].
//!
//! [`DisplayDriver::flush`]: crate::display::DisplayDriver::flush
//! [`DisplayDriver::flush_wait`]: crate::display::DisplayDriver::flush_wait
//! [`DisplayDriver::sync`]: crate::display::DisplayDriver::sync
//! [`DisplayDriver::sync_wait`]: crate::display::DisplayDriver::sync_wait
//! [`FlushResult::Done`]: crate::display::FlushResult::Done

use alloc::vec::Vec;

use crate::core::Ui;
use crate::core::obj::{LayerType, Obj};
use crate::core::obj_pos::ObjPointTransformFlag;
use crate::display::{DisplayData, DisplayId, FlushResult, RenderMode};
use crate::draw::draw_buf::{DrawBuf, STRIDE_AUTO};
use crate::draw::image::DrawImageDsc;
use crate::draw::mask::MaskRectDsc;
use crate::draw::{Layer, LayerId};
use crate::misc::area::{Area, Point};
use crate::misc::color::{ColorFormat, OPA_MAX, OPA_MIN, opa_mix2};
use crate::misc::event::{CoverCheckInfo, CoverRes, EventCode, EventParam, LvResult};
use crate::misc::style::{BlendMode, ImageSrc, Part, StyleProp, StyleValue};
use crate::misc::timer::TimerId;
use crate::misc::types::coord;

/// `LV_COLOR_FORMAT_DEFAULT` of the parity configuration
/// (`LV_COLOR_DEPTH 32`): the format of opaque simple layers.
const COLOR_FORMAT_DEFAULT: ColorFormat = ColorFormat::Xrgb8888;

/// Name of the software draw unit (`ferrovgl_draw_sw::DRAW_UNIT_NAME`),
/// the owner of `lv_draw_sw_mask_cleanup()`'s circle cache.
const DRAW_SW_UNIT_NAME: &str = "SW";

/// `LV_DRAW_TRANSFORM_USE_MATRIX` (off in every upstream test configuration).
const DRAW_TRANSFORM_USE_MATRIX: bool = false;

/*---------------------------------------------------------------------------
 * DisplayData helpers (fixed size arrays of upstream are `Vec`s here)
 *-------------------------------------------------------------------------*/

/// `disp->inv_areas[i]` (a missing slot reads as the zeroed area).
#[inline]
fn inv_area_at(d: &DisplayData, i: usize) -> Area {
    d.inv_areas.get(i).copied().unwrap_or_default()
}

/// `disp->inv_area_joined[i]` (a missing slot reads as 0).
#[inline]
fn inv_joined_at(d: &DisplayData, i: usize) -> bool {
    d.inv_area_joined.get(i).copied().unwrap_or(false)
}

/// `disp->inv_areas[i] = a`, growing the vector if the display did not
/// pre-allocate `LV_INV_BUF_SIZE` slots.
fn set_inv_area(d: &mut DisplayData, i: usize, a: Area) {
    if i >= d.inv_areas.len() {
        d.inv_areas.resize(i + 1, Area::default());
    }
    d.inv_areas[i] = a;
}

/// `disp->inv_area_joined[i] = v`
fn set_inv_joined(d: &mut DisplayData, i: usize, v: bool) {
    if i >= d.inv_area_joined.len() {
        d.inv_area_joined.resize(i + 1, false);
    }
    d.inv_area_joined[i] = v;
}

/// `lv_display_is_double_buffered()` that also holds while the active
/// buffer is lent to the display layer (`buf_act == 2` implies `buf_2`).
#[inline]
fn disp_is_double_buffered(d: &DisplayData) -> bool {
    d.buf_2.is_some() || d.buf_act == 2
}

/// `disp->buf_3 != NULL`, including while `buf_3` is lent to the layer.
#[inline]
fn disp_has_buf_3(d: &DisplayData) -> bool {
    d.buf_3.is_some() || d.buf_act == 3
}

/// `disp->sync_cb != NULL`.
#[inline]
fn disp_has_sync_cb(d: &DisplayData) -> bool {
    d.driver.as_ref().is_some_and(|driver| driver.has_sync())
}

/// The `buf_1` / `buf_2` / `buf_3` slot selected by `idx` (1..=3).
fn buf_slot(d: &mut DisplayData, idx: u8) -> Option<&mut Option<DrawBuf>> {
    match idx {
        1 => Some(&mut d.buf_1),
        2 => Some(&mut d.buf_2),
        3 => Some(&mut d.buf_3),
        _ => None,
    }
}

/// Borrow `buf_1` / `buf_2` / `buf_3` by index.
fn buf_ref(d: &DisplayData, idx: u8) -> Option<&DrawBuf> {
    match idx {
        1 => d.buf_1.as_ref(),
        2 => d.buf_2.as_ref(),
        3 => d.buf_3.as_ref(),
        _ => None,
    }
}

/// `lv_draw_buf_copy(buf[dest], area, buf[src], area)` between two display
/// buffers.
fn copy_disp_buf(d: &mut DisplayData, dest: u8, src: u8, area: &Area) {
    if dest == src {
        /*Copying a buffer onto itself is a no-op*/
        return;
    }
    let Some(mut dest_buf) = buf_slot(d, dest).and_then(|s| s.take()) else {
        return;
    };
    if let Some(src_buf) = buf_ref(d, src) {
        DrawBuf::copy(&mut dest_buf, Some(area), src_buf, Some(area));
    }
    if let Some(slot) = buf_slot(d, dest) {
        *slot = Some(dest_buf);
    }
}

/// The buffer swap at the end of `draw_buf_flush()`: the next `buf_act`.
fn next_buf_act(d: &DisplayData) -> u8 {
    if d.buf_act == 1 {
        2
    } else if d.buf_act == 2 {
        if disp_has_buf_3(d) { 3 } else { 1 }
    } else {
        1
    }
}

/// `refr_sync_areas()`: `(off_screen2, on_screen)` buffer indexes for the
/// automatic double buffered direct mode sync.
fn sync_buf_relations(d: &DisplayData) -> (u8, u8) {
    let has3 = disp_has_buf_3(d);
    if d.buf_act == 1 {
        (2, if has3 { 3 } else { 2 })
    } else if d.buf_act == 2 {
        (if has3 { 3 } else { 1 }, 1)
    } else {
        (1, 2)
    }
}

/// The "save only if this area is not in one of the saved areas" part of
/// `lv_inv_area()`. Returns `false` if `com_area` was already covered (and
/// nothing was stored).
fn inv_area_store(d: &mut DisplayData, com_area: &Area, scr_area: &Area, inv_buf_size: usize) -> bool {
    /*Save only if this area is not in one of the saved areas*/
    for i in 0..d.inv_p as usize {
        if com_area.is_in(&inv_area_at(d, i), 0) {
            return false;
        }
    }

    /*Save the area*/
    let mut tmp_area = *com_area;
    if d.inv_p as usize >= inv_buf_size {
        /*If no place for the area add the screen*/
        d.inv_p = 0;
        tmp_area = *scr_area;
    }
    let p = d.inv_p as usize;
    set_inv_area(d, p, tmp_area);
    d.inv_p += 1;
    true
}

/// `lv_refr_join_area()` on the display data.
fn join_areas(d: &mut DisplayData) {
    let inv_p = d.inv_p as usize;
    for join_in in 0..inv_p {
        if inv_joined_at(d, join_in) {
            continue;
        }

        /*Check all areas to join them in 'join_in'*/
        for join_from in 0..inv_p {
            /*Handle only unjoined areas and ignore itself*/
            if inv_joined_at(d, join_from) || join_in == join_from {
                continue;
            }

            let a_in = inv_area_at(d, join_in);
            let a_from = inv_area_at(d, join_from);

            /*Check if the areas are on each other*/
            if !a_in.is_on(&a_from) {
                continue;
            }

            let joined_area = Area::join(&a_in, &a_from);

            /*Join two area only if the joined area size is smaller*/
            if joined_area.size() < a_in.size().wrapping_add(a_from.size()) {
                set_inv_area(d, join_in, joined_area);

                /*Mark 'join_form' is joined into 'join_in'*/
                set_inv_joined(d, join_from, true);
            }
        }
    }
}

/// `lv_area_diff()` of every sync area against `inv_area`, replacing each
/// touched sync area with its remaining parts (in place, keeping order).
fn sync_areas_remove(sync_areas: &mut Vec<Area>, inv_area: &Area) {
    let mut out: Vec<Area> = Vec::with_capacity(sync_areas.len());
    for sync_area in sync_areas.drain(..) {
        let mut res = [Area::default(); 4];
        /*Remove intersect of redraw area from sync area and get remaining areas*/
        match sync_area.diff(inv_area, &mut res) {
            /*New sub areas created after removing intersect*/
            Some(res_c) => out.extend_from_slice(&res[..res_c]),
            None => out.push(sync_area),
        }
    }
    *sync_areas = out;
}

/// The sub-areas of the partial mode chunking in `refr_invalid_areas()`:
/// `(sub_area, y_offset, last_part)` in rendering order.
fn partial_sub_areas(inv_a: &Area, max_row: i32) -> Vec<(Area, i32, bool)> {
    let mut out = Vec::new();
    let mut row_last = 0;
    let mut sub_area = Area::new(inv_a.x1, 0, inv_a.x2, 0);
    let mut y_off = 0;
    let mut row = inv_a.y1;
    /*Upstream loops forever when `max_row == 0`; kept as is.*/
    while row + max_row - 1 <= inv_a.y2 {
        /*Calc. the next y coordinates of draw_buf*/
        sub_area.y1 = row;
        sub_area.y2 = row + max_row - 1;
        if sub_area.y2 > inv_a.y2 {
            sub_area.y2 = inv_a.y2;
        }
        row_last = sub_area.y2;
        let last_part = inv_a.y2 == row_last;
        out.push((sub_area, y_off, last_part));
        y_off += sub_area.height();
        row += max_row;
    }

    /*If the last y coordinates are not handled yet ...*/
    if inv_a.y2 != row_last {
        /*Calc. the next y coordinates of draw_buf*/
        sub_area.y1 = row;
        sub_area.y2 = inv_a.y2;
        out.push((sub_area, y_off, true));
    }
    out
}

/// The tile split of `refr_area()`: `(tile_cnt, tile_h)`.
fn tile_split(area: &Area, color_format: ColorFormat, max_tile_cnt: u32, total_buf_size: u32) -> (u32, i32) {
    let mut tile_cnt: u32 = 1;
    let mut tile_h = area.height();
    if !color_format.is_indexed() {
        /* Assume that the buffer size (can be screen sized or smaller in case of partial mode)
         * and max tile size are the optimal scenario. From this calculate the ideal tile size
         * and set the tile count and tile height accordingly.
         */
        let ideal_tile_size = total_buf_size / max_tile_cnt;
        let area_buf_size = area.size().wrapping_mul(u32::from(color_format.size()));

        tile_cnt = area_buf_size.wrapping_add(ideal_tile_size.wrapping_sub(1)) / ideal_tile_size; /*Round up*/
        tile_h = ((area.height() as u32) / tile_cnt) as i32;
    }
    (tile_cnt, tile_h)
}

/// The area of tile `i` of `refr_area()` (C `uint32_t` arithmetic).
fn tile_area(area: &Area, i: u32, tile_cnt: u32, tile_h: i32) -> Area {
    let mut tile_area = Area::new(
        area.x1,
        (area.y1 as u32).wrapping_add(i.wrapping_mul(tile_h as u32)) as i32,
        area.x2,
        (area.y1 as u32)
            .wrapping_add((i + 1).wrapping_mul(tile_h as u32))
            .wrapping_sub(1) as i32,
    );
    if i == tile_cnt - 1 {
        tile_area.y2 = area.y2;
    }
    tile_area
}

impl Ui {
    /*=====================
     * GLOBAL FUNCTIONS
     *====================*/

    /// `lv_refr_init()`: initialize the screen refresh subsystem (nothing to do).
    pub fn refr_init(&mut self) {}

    /// `lv_refr_deinit()`: deinitialize the screen refresh subsystem (nothing to do).
    pub fn refr_deinit(&mut self) {}

    /// `lv_refr_now()`: refresh `disp` (or every display) immediately.
    pub fn refr_now(&mut self, disp: Option<DisplayId>) {
        self.anim_refr_now();

        if let Some(disp) = disp {
            if let Some(t) = self.display(disp).and_then(|d| d.refr_timer) {
                self.display_refr_timer(Some(t));
            }
        } else {
            let mut d = self.display_get_next(None);
            while let Some(cur) = d {
                if let Some(t) = self.display(cur).and_then(|x| x.refr_timer) {
                    self.display_refr_timer(Some(t));
                }
                d = self.display_get_next(Some(cur));
            }
        }
    }

    /// `lv_obj_redraw()`: draw one object (without children) into `layer`.
    pub fn obj_redraw(&mut self, layer: LayerId, obj: Obj) {
        if !self.draw.layers.contains(layer.0) {
            crate::misc::check_arg_failed("obj_redraw: layer is NULL");
            return;
        }
        if !self.obj_is_valid(obj) {
            crate::misc::check_arg_failed("obj_redraw: obj is NULL");
            return;
        }

        let clip_area_ori = self.draw.layers[layer.0].clip_area;

        /*The widget will be rendered.
         *So setters from now should use animations. */
        if let Some(o) = self.obj_mut(obj) {
            o.rendered = true;
        }

        /*Truncate the clip area to `obj size + ext size` area*/
        let mut obj_coords_ext = self.obj_get_coords(obj);
        let ext_draw_size = self.obj_get_ext_draw_size(obj);
        obj_coords_ext.increase(ext_draw_size, ext_draw_size);

        let Some(clip_coords_for_obj) = Area::intersect(&clip_area_ori, &obj_coords_ext) else {
            return;
        };
        /*If the object is visible on the current clip area*/
        self.set_layer_clip(layer, clip_coords_for_obj);

        for code in [
            EventCode::DRAW_MAIN_BEGIN,
            EventCode::DRAW_MAIN,
            EventCode::DRAW_MAIN_END,
        ] {
            self.obj_send_event(obj, code, EventParam::Layer(layer));
            if !self.obj_is_valid(obj) || !self.draw.layers.contains(layer.0) {
                self.set_layer_clip(layer, clip_area_ori);
                return;
            }
        }

        let obj_coords = if self.obj_is_overflow_visible(obj) {
            obj_coords_ext
        } else {
            self.obj_field_coords(obj)
        };

        let (layer_clip, layer_opa) = {
            let l = &self.draw.layers[layer.0];
            (l.clip_area, l.opa)
        };
        let clip_coords_for_children = match Area::intersect(&layer_clip, &obj_coords) {
            Some(c) if layer_opa > OPA_MIN => Some(c),
            _ => None,
        };

        if let Some(clip_coords_for_children) = clip_coords_for_children {
            let child_cnt = self.obj_get_child_count(obj);
            if child_cnt == 0 {
                /*If the object was visible on the clip area call the post draw events too*/
                /*If all the children are redrawn make 'post draw' draw*/
                self.send_draw_post_events(obj, layer);
            } else {
                self.set_layer_clip(layer, clip_coords_for_children);
                let mut clip_corner = self.style_num(obj, Part::MAIN, StyleProp::CLIP_CORNER) != 0;

                let mut radius = 0;
                if clip_corner {
                    radius = self.style_num(obj, Part::MAIN, StyleProp::RADIUS);
                    if radius == 0 {
                        clip_corner = false;
                    }
                }

                if !clip_corner {
                    self.refr_children_into(obj, child_cnt, layer);

                    /*If the object was visible on the clip area call the post draw events too*/
                    /*If all the children are redrawn make 'post draw' draw*/
                    if self.obj_is_valid(obj) {
                        self.send_draw_post_events(obj, layer);
                    }
                } else {
                    self.obj_redraw_clip_corner(layer, obj, child_cnt, radius);
                }
            }
        }

        self.set_layer_clip(layer, clip_area_ori);
    }

    /// The `clip_corner` branch of `lv_obj_redraw()`: draw the children into
    /// ARGB8888 layers masked by the rounded rectangle at the top and the
    /// bottom, and directly in the middle.
    fn obj_redraw_clip_corner(&mut self, layer: LayerId, obj: Obj, child_cnt: u32, radius: i32) {
        let coords = self.obj_field_coords(obj);

        let mut mask_draw_dsc = MaskRectDsc::new();
        mask_draw_dsc.radius = radius;
        mask_draw_dsc.area = coords;

        let mut img_draw_dsc = DrawImageDsc::new();

        let short_side = coords.width().min(coords.height());
        let rout = radius.min(short_side >> 1);

        let mut bottom = coords;
        bottom.y1 = bottom.y2 - rout + 1;
        if let Some(bottom) = Area::intersect(&bottom, &self.draw.layers[layer.0].clip_area) {
            let layer_children = self.draw_layer_create(Some(layer), ColorFormat::Argb8888, &bottom);

            self.refr_children_into(obj, child_cnt, layer_children);

            /*If all the children are redrawn send 'post draw' draw*/
            if !self.obj_is_valid(obj) {
                self.draw_layer_delete(layer_children);
                return;
            }
            self.send_draw_post_events(obj, layer_children);

            self.draw_mask_rect(layer_children, &mask_draw_dsc);

            img_draw_dsc.src = ImageSrc::Layer(layer_children);
            self.draw_layer(layer, &img_draw_dsc, &bottom);
        }

        if !self.obj_is_valid(obj) || !self.draw.layers.contains(layer.0) {
            return;
        }

        let mut top = coords;
        top.y2 = top.y1 + rout - 1;
        if let Some(top) = Area::intersect(&top, &self.draw.layers[layer.0].clip_area) {
            let layer_children = self.draw_layer_create(Some(layer), ColorFormat::Argb8888, &top);

            self.refr_children_into(obj, child_cnt, layer_children);

            /*If all the children are redrawn send 'post draw' draw*/
            if !self.obj_is_valid(obj) {
                self.draw_layer_delete(layer_children);
                return;
            }
            self.send_draw_post_events(obj, layer_children);

            self.draw_mask_rect(layer_children, &mask_draw_dsc);

            img_draw_dsc.src = ImageSrc::Layer(layer_children);
            self.draw_layer(layer, &img_draw_dsc, &top);
        }

        if !self.obj_is_valid(obj) || !self.draw.layers.contains(layer.0) {
            return;
        }

        let mut mid = coords;
        mid.y1 += rout;
        mid.y2 -= rout;
        if let Some(mid) = Area::intersect(&mid, &self.draw.layers[layer.0].clip_area) {
            self.set_layer_clip(layer, mid);
            self.refr_children_into(obj, child_cnt, layer);

            /*If all the children are redrawn make 'post draw' draw*/
            if self.obj_is_valid(obj) {
                self.send_draw_post_events(obj, layer);
            }
        }
    }

    /// `lv_inv_area()`: invalidate `area` (or, with `None`, clear the
    /// invalidated areas) on `disp` (`None`: the default display).
    pub fn inv_area(&mut self, disp: Option<DisplayId>, area: Option<&Area>) -> LvResult {
        let disp = match disp {
            Some(d) => Some(d),
            None => {
                #[cfg(feature = "log")]
                log::warn!("lv_inv_area: using the default display (NULL display is deprecated)");
                self.display_get_default()
            }
        };
        let Some(disp) = disp else {
            return LvResult::Invalid;
        };
        if self.display(disp).is_none() {
            return LvResult::Invalid;
        }
        if !self.display_is_invalidation_enabled(Some(disp)) {
            return LvResult::Invalid;
        }

        /*
         * Invalidating during rendering means that the LVGL API is used across
         * threads or a custom widget modifies properties in a DRAW event.
         */
        debug_assert!(
            !self.display(disp).is_some_and(|d| d.rendering_in_progress),
            "Invalidate area is not allowed during rendering."
        );

        /*Clear the invalidate buffer if the parameter is NULL*/
        let Some(area_p) = area else {
            if let Some(d) = self.display_mut(disp) {
                d.inv_p = 0;
            }
            return LvResult::Ok;
        };

        let scr_area = Area::new(
            0,
            0,
            self.display_get_horizontal_resolution(Some(disp)) - 1,
            self.display_get_vertical_resolution(Some(disp)) - 1,
        );

        let Some(mut com_area) = Area::intersect(area_p, &scr_area) else {
            return LvResult::Invalid; /*Out of the screen*/
        };

        let Some(d) = self.display_mut(disp) else {
            return LvResult::Invalid;
        };

        if d.color_format == ColorFormat::I1 {
            /*Make sure that the X coordinates start and end on byte boundary.
             *E.g. convert 11;27 to 8;31*/
            com_area.x1 &= !0x7; /*Round down: Nx8*/
            com_area.x2 |= 0x7; /*Round up: Nx8 - 1*/
        }

        /*If there were at least 1 invalid area in full refresh mode, redraw the whole screen*/
        if d.render_mode == RenderMode::Full {
            set_inv_area(d, 0, scr_area);
            d.inv_p = 1;
            self.display_send_event(disp, EventCode::REFR_REQUEST, EventParam::None);
            return LvResult::Ok;
        }

        let res = self.display_send_event(
            disp,
            EventCode::INVALIDATE_AREA,
            EventParam::AreaMut(&mut com_area),
        );
        if res != LvResult::Ok {
            return LvResult::Invalid;
        }

        let inv_buf_size = self.config().inv_buf_size;
        let Some(d) = self.display_mut(disp) else {
            return LvResult::Invalid;
        };
        if !inv_area_store(d, &com_area, &scr_area, inv_buf_size) {
            return LvResult::Ok;
        }

        self.display_send_event(disp, EventCode::REFR_REQUEST, EventParam::None);

        LvResult::Ok
    }

    /// `lv_refr_get_disp_refreshing()`: the display being refreshed.
    #[must_use]
    pub fn refr_get_disp_refreshing(&self) -> Option<DisplayId> {
        self.disp_refresh
    }

    /// `lv_refr_set_disp_refreshing()`: set the display being refreshed.
    pub fn refr_set_disp_refreshing(&mut self, disp: Option<DisplayId>) {
        self.disp_refresh = disp;
    }

    /// `lv_display_refr_timer()`: the periodic refresh (`timer == None`
    /// refreshes the default display).
    pub fn display_refr_timer(&mut self, timer: Option<TimerId>) {
        self.disp_refresh = match timer {
            Some(t) => self.refr_timer_display(t),
            None => self.display_get_default(),
        };

        let Some(disp) = self.disp_refresh else {
            #[cfg(feature = "log")]
            log::warn!("No display registered");
            return;
        };

        let has_buf = self
            .display(disp)
            .and_then(|d| buf_ref(d, d.buf_act))
            .is_some_and(|b| !b.data().is_empty() && b.data_size != 0);
        if !has_buf {
            #[cfg(feature = "log")]
            log::warn!("No draw buffer");
            return;
        }

        /* LV_USE_PERF_MONITOR is enabled in the parity configuration, so the
         * timer keeps running to count the FPS (no `lv_timer_pause()`). */

        let res = self.display_send_event(disp, EventCode::REFR_START, EventParam::None);
        if res == LvResult::Invalid {
            /*deleted*/
            return;
        }

        /*Refresh the screen's layout if required*/
        let Some(d) = self.display(disp) else {
            return;
        };
        let (act_scr, prev_scr) = (d.act_scr, d.prev_scr);
        if let Some(s) = act_scr {
            self.obj_update_layout(Obj(s));
        }
        if let Some(s) = prev_scr {
            self.obj_update_layout(Obj(s));
        }
        for get in [
            |d: &DisplayData| d.bottom_layer,
            |d: &DisplayData| d.top_layer,
            |d: &DisplayData| d.sys_layer,
        ] {
            if let Some(s) = self.display(disp).and_then(get) {
                self.obj_update_layout(Obj(s));
            }
        }

        let Some(d) = self.display_mut(disp) else {
            return;
        };

        /*Do nothing if there is no active screen*/
        if d.act_scr.is_none() {
            d.inv_p = 0;
            #[cfg(feature = "log")]
            log::warn!("there is no active screen");
        } else {
            /*Expand the invalidated areas for blur objects before joining them*/
            self.obj_invalidate_expand_blur(disp);
            self.refr_join_areas(disp);
            self.refr_sync_areas(disp);
            self.refr_invalid_areas(disp);

            if let Some(d) = self.display_mut(disp)
                && d.inv_p != 0
            {
                /*In double buffered direct mode or if sync callback is set, save the updated areas.
                 *They will be used on the next call to synchronize the buffers.*/
                if (disp_is_double_buffered(d) && d.render_mode == RenderMode::Direct) || disp_has_sync_cb(d)
                {
                    for i in 0..d.inv_p as usize {
                        if inv_joined_at(d, i) {
                            continue;
                        }
                        let a = inv_area_at(d, i);
                        d.sync_areas.push(a);
                    }
                }

                d.inv_areas.iter_mut().for_each(|a| *a = Area::default());
                d.inv_area_joined.iter_mut().for_each(|j| *j = false);
                d.inv_p = 0;
            }
        }

        /*refr_finish:*/

        self.refr_finish_draw_sw_mask_cleanup();

        if self.display(disp).is_some() {
            self.display_send_event(disp, EventCode::REFR_READY, EventParam::None);
        }
    }

    /// `#if LV_DRAW_SW_COMPLEX == 1 lv_draw_sw_mask_cleanup(); #endif` of
    /// `refr_finish`.
    ///
    /// The circle cache (`_circle_cache`) lives in the software draw unit
    /// of `ferrovgl-draw-sw`, which this crate cannot call, so the cleanup
    /// is requested with `LV_EVENT_REFR_READY` sent only to the units named
    /// [`DRAW_SW_UNIT_NAME`] (other units get no extra notification).
    /// Cross-crate contract: the SW unit's `on_event` must handle
    /// `REFR_READY` with `ferrovgl_draw_sw::mask::cleanup(&mut
    /// self.circle_cache)`, which frees and zeroes every entry.
    fn refr_finish_draw_sw_mask_cleanup(&mut self) {
        self.draw_unit_send_event(Some(DRAW_SW_UNIT_NAME), EventCode::REFR_READY, EventParam::None);
    }

    /// `lv_refr_get_top_obj()`: the topmost object fully covering `area`,
    /// starting the search from `obj` (typically a screen).
    #[must_use]
    pub fn refr_get_top_obj(&mut self, area: &Area, obj: Obj) -> Option<Obj> {
        if !self.obj_is_valid(obj) {
            crate::misc::check_arg_failed("refr_get_top_obj: obj is NULL");
            return None;
        }

        let mut found_p = None;

        if !area.is_in(&self.obj_field_coords(obj), 0) {
            return None;
        }
        if self.obj_is_hidden(obj) {
            return None;
        }
        if self.obj_get_layer_type(obj) != LayerType::None {
            return None;
        }
        if (self.style_num(obj, Part::MAIN, StyleProp::OPA) as u8) < OPA_MAX {
            return None;
        }

        /*If this object is fully cover the draw area then check the children too*/
        let mut info = CoverCheckInfo {
            res: CoverRes::Cover,
            area: *area,
        };
        self.obj_send_event(obj, EventCode::COVER_CHECK, EventParam::CoverCheck(&mut info));
        if !self.obj_is_valid(obj) {
            return None;
        }
        if info.res == CoverRes::Masked {
            return None;
        }

        let child_cnt = self.obj_get_child_count(obj) as i32;
        let mut i = child_cnt - 1;
        while i >= 0 {
            if let Some(child) = self.child_at(obj, i as u32) {
                found_p = self.refr_get_top_obj(area, child);

                /*If a children is ok then break*/
                if found_p.is_some() {
                    break;
                }
            }
            if !self.obj_is_valid(obj) {
                return None;
            }
            i -= 1;
        }

        /*If no better children use this object*/
        if found_p.is_none() && info.res == CoverRes::Cover {
            found_p = Some(obj);
        }

        found_p
    }

    /// `lv_obj_refr()`: draw `obj` and its children into `layer`, creating
    /// sub-layers for opacity / transforms.
    pub fn obj_refr(&mut self, layer: LayerId, obj: Obj) {
        if !self.draw.layers.contains(layer.0) {
            crate::misc::check_arg_failed("obj_refr: layer is NULL");
            return;
        }
        if !self.obj_is_valid(obj) {
            crate::misc::check_arg_failed("obj_refr: obj is NULL");
            return;
        }

        if self.obj_is_hidden(obj) {
            return;
        }

        /*If `opa_layered != LV_OPA_COVER` draw the widget on a new layer and blend that layer with the given opacity.*/
        let opa_layered = self.style_num(obj, Part::MAIN, StyleProp::OPA_LAYERED) as u8;
        if opa_layered <= OPA_MIN {
            return;
        }

        if self.style_num(obj, Part::MAIN, StyleProp::TRANSFORM_SCALE_X) <= 0 {
            return;
        }
        if self.style_num(obj, Part::MAIN, StyleProp::TRANSFORM_SCALE_Y) <= 0 {
            return;
        }

        let (layer_opa_ori, layer_recolor) = {
            let l = &self.draw.layers[layer.0];
            (l.opa, l.recolor)
        };

        /*Normal `opa` (not layered) will just scale down `bg_opa`, `text_opa`, etc, in the upcoming drawings.*/
        let opa_main = self.style_num(obj, Part::MAIN, StyleProp::OPA) as u8;
        if opa_main < OPA_MAX {
            self.draw.layers[layer.0].opa = opa_mix2(layer_opa_ori, opa_main);
        }

        let recolor = self.obj_style_apply_recolor(obj, Part::MAIN, self.draw.layers[layer.0].recolor);
        self.draw.layers[layer.0].recolor = recolor;

        let layer_type = self.obj_get_layer_type(obj);
        if layer_type == LayerType::None {
            self.obj_redraw(layer, obj);
        }
        /* LV_DRAW_TRANSFORM_USE_MATRIX is off: no matrix transform branch */
        else if let Some((layer_area_full, obj_draw_size)) = self.layer_get_area(layer, obj, layer_type) {
            self.obj_refr_layered(
                layer,
                obj,
                layer_type,
                opa_layered,
                &layer_area_full,
                &obj_draw_size,
            );
        }

        /* Restore the original layer opa and recolor */
        if let Some(l) = self.draw.layers.get_mut(layer.0) {
            l.opa = layer_opa_ori;
            l.recolor = layer_recolor;
        }
    }

    /// The sub-layer branch of `lv_obj_refr()`: render `obj` into (possibly
    /// several, for simple layers) new layers and blend them with a
    /// `LV_DRAW_TASK_TYPE_LAYER` task.
    fn obj_refr_layered(
        &mut self,
        layer: LayerId,
        obj: Obj,
        layer_type: LayerType,
        opa_layered: u8,
        layer_area_full: &Area,
        obj_draw_size: &Area,
    ) {
        /*Simple layers can be subdivided into smaller layers*/
        let mut max_rgb_row_height = layer_area_full.height() as u32;
        let mut max_argb_row_height = layer_area_full.height() as u32;
        if layer_type == LayerType::Simple {
            let w = layer_area_full.width();
            let disp_cf = self
                .disp_refresh
                .or(self.draw.layers[layer.0].display)
                .and_then(|d| self.display(d))
                .map_or(ColorFormat::Unknown, |d| d.color_format);
            let px_size = i32::from(disp_cf.size());
            let simple_buf_size = self.config().draw_layer_simple_buf_size as i32;
            max_rgb_row_height = (simple_buf_size / w / px_size) as u32;
            max_argb_row_height = (simple_buf_size / w / 4) as u32; /*sizeof(lv_color32_t)*/
        }

        let mut layer_area_act = Area::new(
            layer_area_full.x1,
            layer_area_full.y1,
            layer_area_full.x2,
            layer_area_full.y1,
        );

        while layer_area_act.y2 < layer_area_full.y2 {
            /* Test with an RGB layer size (which is larger than the ARGB layer size)
             * If it really doesn't need alpha use it. Else switch to the ARGB size*/
            layer_area_act.y2 = (layer_area_act.y1 as u32)
                .wrapping_add(max_rgb_row_height)
                .wrapping_sub(1) as i32;
            if layer_area_act.y2 > layer_area_full.y2 {
                layer_area_act.y2 = layer_area_full.y2;
            }

            let bitmap_mask_src = match self.obj_get_style_prop(obj, Part::MAIN, StyleProp::BITMAP_MASK_SRC) {
                StyleValue::Image(src) => src,
                _ => ImageSrc::None,
            };
            let area_need_alpha = !matches!(bitmap_mask_src, ImageSrc::None)
                || self.alpha_test_area_on_obj(obj, &layer_area_act);
            if !self.obj_is_valid(obj) || !self.draw.layers.contains(layer.0) {
                return;
            }

            if area_need_alpha {
                layer_area_act.y2 = (layer_area_act.y1 as u32)
                    .wrapping_add(max_argb_row_height)
                    .wrapping_sub(1) as i32;
                if layer_area_act.y2 > layer_area_full.y2 {
                    layer_area_act.y2 = layer_area_full.y2;
                }
            }

            let new_layer = self.draw_layer_create(
                Some(layer),
                if area_need_alpha {
                    ColorFormat::Argb8888
                } else {
                    COLOR_FORMAT_DEFAULT
                },
                &layer_area_act,
            );
            self.obj_redraw(new_layer, obj);
            if !self.obj_is_valid(obj) || !self.draw.layers.contains(layer.0) {
                self.draw_layer_delete(new_layer);
                return;
            }

            let coords = self.obj_field_coords(obj);
            let mut pivot = Point::new(
                self.style_num(obj, Part::MAIN, StyleProp::TRANSFORM_PIVOT_X),
                self.style_num(obj, Part::MAIN, StyleProp::TRANSFORM_PIVOT_Y),
            );

            if coord::is_pct(pivot.x) {
                pivot.x = (coord::get_pct(pivot.x) * coords.width()) / 100;
            }
            if coord::is_pct(pivot.y) {
                pivot.y = (coord::get_pct(pivot.y) * coords.height()) / 100;
            }

            let new_buf_area = self
                .draw
                .layers
                .get(new_layer.0)
                .map_or(layer_area_act, |l| l.buf_area);

            let mut layer_draw_dsc = DrawImageDsc::new();
            layer_draw_dsc.pivot.x = coords.x1 + pivot.x - new_buf_area.x1;
            layer_draw_dsc.pivot.y = coords.y1 + pivot.y - new_buf_area.y1;

            layer_draw_dsc.opa = opa_layered;
            layer_draw_dsc.rotation = self.style_num(obj, Part::MAIN, StyleProp::TRANSFORM_ROTATION);
            while layer_draw_dsc.rotation > 3600 {
                layer_draw_dsc.rotation -= 3600;
            }
            while layer_draw_dsc.rotation < 0 {
                layer_draw_dsc.rotation += 3600;
            }
            layer_draw_dsc.scale_x = self.style_num(obj, Part::MAIN, StyleProp::TRANSFORM_SCALE_X);
            layer_draw_dsc.scale_y = self.style_num(obj, Part::MAIN, StyleProp::TRANSFORM_SCALE_Y);
            layer_draw_dsc.skew_x = self.style_num(obj, Part::MAIN, StyleProp::TRANSFORM_SKEW_X);
            layer_draw_dsc.skew_y = self.style_num(obj, Part::MAIN, StyleProp::TRANSFORM_SKEW_Y);
            layer_draw_dsc.blend_mode =
                BlendMode::from_u8(self.style_num(obj, Part::MAIN, StyleProp::BLEND_MODE) as u8);
            layer_draw_dsc.antialias = self
                .disp_refresh
                .and_then(|d| self.display(d))
                .is_some_and(|d| d.antialiasing);
            layer_draw_dsc.bitmap_mask_src = bitmap_mask_src;
            layer_draw_dsc.image_area = *obj_draw_size;
            layer_draw_dsc.src = ImageSrc::Layer(new_layer);

            self.draw_layer(layer, &layer_draw_dsc, &layer_area_act);

            layer_area_act.y1 = layer_area_act.y2 + 1;
        }
    }

    /*=====================
     * STATIC FUNCTIONS
     *====================*/

    /// `lv_refr_join_area()`: join the invalid areas which have common parts
    /// when the joined area is smaller than the two areas together.
    pub(crate) fn refr_join_areas(&mut self, disp: DisplayId) {
        if let Some(d) = self.display_mut(disp) {
            join_areas(d);
        }
    }

    /// `refr_sync_areas()`: in double buffered direct mode (or with a sync
    /// callback) copy the areas rendered in the previous frame and not
    /// redrawn now to the buffer that will be rendered.
    fn refr_sync_areas(&mut self, disp: DisplayId) {
        let Some(d) = self.display(disp) else {
            return;
        };
        /*Do not sync if not direct double buffered and no sync callback set*/
        let auto_sync = d.render_mode == RenderMode::Direct && disp_is_double_buffered(d);
        let user_sync = disp_has_sync_cb(d);
        if !auto_sync && !user_sync {
            return;
        }

        /*Do not sync if no sync areas*/
        if d.sync_areas.is_empty() {
            return;
        }

        /*With double buffered direct mode synchronize the rendered areas to the other buffer*/
        /*We need to wait for ready here to not mess up the active screen*/
        self.wait_for_flushing(disp);

        let Some(d) = self.display_mut(disp) else {
            return;
        };

        /*Iterate through invalidated areas to see if sync area should be copied*/
        for i in 0..d.inv_p as usize {
            /*Skip joined areas*/
            if inv_joined_at(d, i) {
                continue;
            }
            let inv = inv_area_at(d, i);
            sync_areas_remove(&mut d.sync_areas, &inv);
        }

        /*The buffers are already swapped.
         *So the active buffer is the off screen buffer where LVGL will render*/
        let off_screen = d.buf_act;
        /*Triple buffer sync buffer for off-screen2 updates.*/
        let mut off_screen2 = 0;
        let mut on_screen = 0;

        /* Only compute buffer relationships when auto_sync (direct double-buffered) is used.
         * When only user_sync is active, these pointers are not needed. */
        if auto_sync {
            (off_screen2, on_screen) = sync_buf_relations(d);
        }

        let hor_res = self.display_get_horizontal_resolution(Some(disp)) as u32;
        let ver_res = self.display_get_vertical_resolution(Some(disp)) as u32;

        let disp_area = Area::new(0, 0, hor_res as i32 - 1, ver_res as i32 - 1);

        /*Copy sync areas (if any remaining)*/
        let mut idx = 0;
        loop {
            let Some(d) = self.display_mut(disp) else {
                return;
            };
            let Some(sync_area) = d.sync_areas.get(idx).copied() else {
                break;
            };
            let is_tail = idx + 1 == d.sync_areas.len();
            idx += 1;

            /*Resize SDL window will trigger crash because of sync_area is larger than disp_area*/
            let Some(sync_area) = Area::intersect(&sync_area, &disp_area) else {
                continue;
            };
            d.sync_areas[idx - 1] = sync_area;

            /* LV_DRAW_TRANSFORM_USE_MATRIX is off: no rotation of the sync area */

            /*Call sync callback (if set)*/
            if disp_has_sync_cb(d) {
                /*Set syncing flags*/
                d.syncing = true;
                d.syncing_last = is_tail;

                /*Call sync callback and wait for sync to complete*/
                self.call_sync_cb(disp, &sync_area);
                self.wait_for_syncing(disp);
            }
            /*Fallback to internal double buffered direct mode sync*/
            else {
                copy_disp_buf(d, off_screen, on_screen, &sync_area);
                if off_screen2 != on_screen {
                    copy_disp_buf(d, off_screen2, on_screen, &sync_area);
                }
            }
        }

        /*Clear sync areas*/
        if let Some(d) = self.display_mut(disp) {
            d.sync_areas.clear();
        }
    }

    /// `refr_invalid_areas()`: render and flush every unjoined invalid area.
    pub(crate) fn refr_invalid_areas(&mut self, disp: DisplayId) {
        let Some(d) = self.display(disp) else {
            return;
        };
        if d.inv_p == 0 {
            return;
        }

        /*Notify the display driven rendering has started*/
        self.display_send_event(disp, EventCode::RENDER_START, EventParam::None);

        let Some(d) = self.display_mut(disp) else {
            return;
        };

        /*Find the last area which will be drawn*/
        let mut last_i: i32 = 0;
        let mut i = d.inv_p as i32 - 1;
        while i >= 0 {
            if !inv_joined_at(d, i as usize) {
                last_i = i;
                break;
            }
            i -= 1;
        }

        d.last_area = false;
        d.last_part = false;
        d.rendering_in_progress = true;

        let mut i: i32 = 0;
        loop {
            let Some(d) = self.display_mut(disp) else {
                return;
            };
            if i >= d.inv_p as i32 {
                break;
            }
            let iu = i as usize;
            i += 1;

            /*Refresh the unjoined areas*/
            if inv_joined_at(d, iu) {
                continue;
            }

            if iu as i32 == last_i {
                d.last_area = true;
            }
            d.last_part = false;

            let inv_a = inv_area_at(d, iu);
            match d.render_mode {
                RenderMode::Partial => {
                    /*Calculate the max row num*/
                    let w = inv_a.width();
                    let h = inv_a.height();

                    let max_row = self.get_max_row(disp, w, h) as i32;

                    for (sub_area, y_off, last_part) in partial_sub_areas(&inv_a, max_row) {
                        if last_part && let Some(d) = self.display_mut(disp) {
                            d.last_part = true;
                        }
                        self.refr_area(disp, &sub_area, y_off);
                        self.draw_buf_flush(disp);
                    }
                }
                RenderMode::Full | RenderMode::Direct => {
                    d.last_part = true;
                    self.refr_area(disp, &inv_a, 0);
                    self.draw_buf_flush(disp);
                }
            }
        }

        self.display_send_event(disp, EventCode::RENDER_READY, EventParam::None);
        if let Some(d) = self.display_mut(disp) {
            d.rendering_in_progress = false;
        }
    }

    /// `layer_reshape_draw_buf()`: reshape the layer's draw buffer to its
    /// `buf_area`.
    fn layer_reshape_draw_buf(&mut self, layer: LayerId, stride: u32) {
        let Some(l) = self.draw.layers.get_mut(layer.0) else {
            return;
        };
        let (cf, w, h) = (l.color_format, l.buf_area.width(), l.buf_area.height());
        let ret = l
            .draw_buf
            .as_mut()
            .is_some_and(|b| b.reshape(cf, w as u32, h as u32, stride));
        debug_assert!(ret, "layer_reshape_draw_buf: draw buffer too small");
    }

    /// `refr_area()`: render `area` into the display buffer (`y_offset` is
    /// the row offset of this chunk in partial mode).
    pub fn refr_area(&mut self, disp: DisplayId, area: &Area, y_offset: i32) {
        let hor_res = self.display_get_horizontal_resolution(Some(disp));
        let ver_res = self.display_get_vertical_resolution(Some(disp));

        let Some(d) = self.display_mut(disp) else {
            return;
        };
        let Some(layer) = d.layer_head else {
            return;
        };
        let render_mode = d.render_mode;
        let stride_is_auto = d.stride_is_auto;
        let max_tile_cnt = u32::from(d.tile_cnt);
        /*layer->draw_buf = disp_refr->buf_act (lent, see the module docs)*/
        let buf_act = d.buf_act;
        let buf = buf_slot(d, buf_act).and_then(|s| s.take());

        let Some(l) = self.draw.layers.get_mut(layer.0) else {
            /*Give the buffer back*/
            if let Some(slot) = self.display_mut(disp).and_then(|d| buf_slot(d, buf_act)) {
                *slot = buf;
            }
            return;
        };
        if let Some(buf) = buf {
            l.draw_buf = Some(buf);
            l.buffer_owned = false;
        }
        l.clip_area = *area;
        l.phy_clip_area = *area;
        l.partial_y_offset = y_offset;
        l.all_tasks_added = false;

        if render_mode == RenderMode::Partial {
            /*In partial mode render this area to the buffer*/
            l.buf_area = *area;
            self.layer_reshape_draw_buf(layer, STRIDE_AUTO);
        } else {
            /*In direct mode and full mode the buffer area is always the whole screen, not considering rotation*/
            l.buf_area.x1 = 0;
            l.buf_area.y1 = 0;

            let (x2, y2) = if DRAW_TRANSFORM_USE_MATRIX && self.display_get_matrix_rotation(Some(disp)) {
                (
                    self.display_get_original_horizontal_resolution(Some(disp)) - 1,
                    self.display_get_original_vertical_resolution(Some(disp)) - 1,
                )
            } else {
                (hor_res - 1, ver_res - 1)
            };
            let l = &mut self.draw.layers[layer.0];
            l.buf_area.x2 = x2;
            l.buf_area.y2 = y2;
            let stride = if stride_is_auto {
                STRIDE_AUTO
            } else {
                l.draw_buf
                    .as_ref()
                    .map_or(STRIDE_AUTO, |b| u32::from(b.header.stride))
            };
            self.layer_reshape_draw_buf(layer, stride);
        }

        /*Try to divide the area to smaller tiles*/
        let (cf, total_buf_size) = {
            let l = &self.draw.layers[layer.0];
            (l.color_format, l.draw_buf.as_ref().map_or(0, |b| b.data_size))
        };
        let (tile_cnt, tile_h) = tile_split(area, cf, max_tile_cnt, total_buf_size);

        if tile_cnt == 1 {
            self.refr_area_part(disp, layer);
            if let Some(l) = self.draw.layers.get_mut(layer.0) {
                l.all_tasks_added = true;
            }
        } else {
            /* Don't draw to the layers buffer of the display but create smaller dummy layers which are using the
             * display's layer buffer. These will be the tiles. By using tiles it's more likely that there will
             * be independent areas for each draw unit.
             *
             * The display buffer has a single owner here, so each tile borrows it in turn and is finished
             * (all its tasks dispatched) before the next tile is configured. The tiles cover disjoint rows,
             * so the rendered pixels are the same as upstream's "configure all, then wait" order. */
            for i in 0..tile_cnt {
                let tile_area = tile_area(area, i, tile_cnt, tile_h);

                let tile_layer = LayerId(self.draw.layers.insert(Layer::new()));
                self.draw_layer_init(tile_layer, None, cf, &tile_area);
                let (buf_area, buf) = {
                    let l = &mut self.draw.layers[layer.0];
                    (l.buf_area, l.draw_buf.take())
                };
                {
                    let t = &mut self.draw.layers[tile_layer.0];
                    t.buf_area = buf_area; /*the buffer is still large*/
                    t.draw_buf = buf;
                    t.buffer_owned = false;
                }
                self.refr_area_part(disp, tile_layer);
                if let Some(t) = self.draw.layers.get_mut(tile_layer.0) {
                    t.all_tasks_added = true;
                }

                /*Wait until the tile is ready and remove it*/
                self.wait_for_layer_tasks(tile_layer);
                let buf = self
                    .draw
                    .layers
                    .get_mut(tile_layer.0)
                    .and_then(|t| t.draw_buf.take());
                if let Some(l) = self.draw.layers.get_mut(layer.0) {
                    l.draw_buf = buf;
                }
                self.draw_layer_deinit(tile_layer);
                self.draw.layers.remove(tile_layer.0);
            }

            if let Some(l) = self.draw.layers.get_mut(layer.0) {
                l.all_tasks_added = true;
            }
        }

        if let Some(d) = self.display_mut(disp) {
            d.refreshed_area = *area;
        }
    }

    /// `refr_configured_layer()`: draw the bottom layer (if nothing covers
    /// the area), the previous and active screens, the top and the system
    /// layer into `layer`.
    pub(crate) fn refr_area_part(&mut self, disp: DisplayId, layer: LayerId) {
        let Some(l) = self.draw.layers.get_mut(layer.0) else {
            return;
        };
        l.reset();

        /* LV_DRAW_TRANSFORM_USE_MATRIX is off: no display rotation matrix */

        let Some(d) = self.display(disp) else {
            return;
        };
        /* In single buffered mode wait here until the buffer is freed.
         * Else we would draw into the buffer while it's still being transferred to the display*/
        if !disp_is_double_buffered(d) {
            self.wait_for_flushing(disp);
        }
        let Some(d) = self.display(disp) else {
            return;
        };
        /*If the screen is transparent initialize it when the flushing is ready*/
        if d.color_format.has_alpha()
            && let Some(l) = self.draw.layers.get_mut(layer.0)
        {
            let mut clear_area = l.clip_area;
            clear_area.move_by(-l.buf_area.x1, -l.buf_area.y1);
            if let Some(buf) = l.draw_buf.as_mut() {
                buf.clear_ex(Some(&clear_area), Some(layer));
            }
        }

        let Some(clip) = self.draw.layers.get(layer.0).map(|l| l.clip_area) else {
            return;
        };

        /*Get the most top object which is not covered by others*/
        let mut top_act_scr = match self.display_get_screen_active(Some(disp)) {
            Some(scr) => self.refr_get_top_obj(&clip, scr),
            None => None,
        };
        let mut top_prev_scr = None;
        if let Some(prev) = self.display(disp).and_then(|d| d.prev_scr) {
            top_prev_scr = self.refr_get_top_obj(&clip, Obj(prev));
        }

        /*Draw a bottom layer background if there is no top object*/
        if top_act_scr.is_none() && top_prev_scr.is_none() {
            let bottom = self.display_get_layer_bottom(Some(disp));
            self.refr_obj_and_children(layer, bottom);
        }

        let Some(d) = self.display(disp) else {
            return;
        };
        let (draw_prev_over_act, act_scr) = (d.draw_prev_over_act, d.act_scr.map(Obj));
        if draw_prev_over_act {
            if top_act_scr.is_none() {
                top_act_scr = act_scr;
            }
            self.refr_obj_and_children(layer, top_act_scr);

            /*Refresh the previous screen if any*/
            if let Some(prev) = self.display(disp).and_then(|d| d.prev_scr) {
                if top_prev_scr.is_none() {
                    top_prev_scr = Some(Obj(prev));
                }
                self.refr_obj_and_children(layer, top_prev_scr);
            }
        } else {
            /*Refresh the previous screen if any*/
            if let Some(prev) = self.display(disp).and_then(|d| d.prev_scr) {
                if top_prev_scr.is_none() {
                    top_prev_scr = Some(Obj(prev));
                }
                self.refr_obj_and_children(layer, top_prev_scr);
            }

            if top_act_scr.is_none() {
                top_act_scr = self.display(disp).and_then(|d| d.act_scr).map(Obj);
            }
            self.refr_obj_and_children(layer, top_act_scr);
        }

        /*Also refresh top and sys layer unconditionally*/
        let top = self.display_get_layer_top(Some(disp));
        self.refr_obj_and_children(layer, top);
        let sys = self.display_get_layer_sys(Some(disp));
        self.refr_obj_and_children(layer, sys);
    }

    /// `refr_obj_and_children()`: draw `top_obj`, its children and the
    /// younger siblings of it and of its parents, then the parents' post
    /// draw events.
    pub(crate) fn refr_obj_and_children(&mut self, layer: LayerId, top_obj: Option<Obj>) {
        /*Normally always will be a top_obj (at least the screen)
         *but in special cases (e.g. if the screen has alpha) it won't.
         *In this case use the screen directly*/
        let top_obj = match top_obj {
            Some(o) => Some(o),
            None => self.display_get_screen_active(self.disp_refresh),
        };
        let Some(top_obj) = top_obj else {
            return; /*Shouldn't happen*/
        };
        if !self.obj_is_valid(top_obj) || !self.draw.layers.contains(layer.0) {
            return;
        }

        /*Draw the 'younger' sibling objects because they can be on top_obj*/
        let mut border_p = top_obj;

        let mut parent = self.obj_get_parent(top_obj);

        /*Calculate the recolor before the parent*/
        if let Some(p) = parent {
            let recolor = self.obj_get_style_recolor_recursive(p, Part::MAIN);
            self.draw.layers[layer.0].recolor = recolor;
        }

        /*Refresh the top object and its children*/
        self.obj_refr(layer, top_obj);

        /*Do until not reach the screen*/
        while let Some(p) = parent {
            if !self.obj_is_valid(p) || !self.draw.layers.contains(layer.0) {
                return;
            }
            let mut go = false;
            let child_cnt = self.obj_get_child_count(p);
            for i in 0..child_cnt {
                let Some(child) = self.child_at(p, i) else {
                    break;
                };
                if !go {
                    if child == border_p {
                        go = true;
                    }
                } else {
                    /*Refresh the objects*/
                    self.obj_refr(layer, child);
                    if !self.obj_is_valid(p) {
                        return;
                    }
                }
            }

            /*Call the post draw function of the parents of the to object*/
            self.send_draw_post_events(p, layer);
            if !self.obj_is_valid(p) {
                return;
            }

            /*The new border will be the last parents,
             *so the 'younger' brothers of parent will be refreshed*/
            border_p = p;
            /*Go a level deeper*/
            parent = self.obj_get_parent(p);
        }
    }

    /// `layer_get_area()`: the area of the sub-layer of `obj` (clipped to
    /// `layer`) and the object's draw area (`coords` + ext draw size).
    /// `None` is `LV_RESULT_INVALID`.
    fn layer_get_area(&mut self, layer: LayerId, obj: Obj, layer_type: LayerType) -> Option<(Area, Area)> {
        let ext_draw_size = self.obj_get_ext_draw_size(obj);
        let mut obj_draw_size = self.obj_get_coords(obj);
        obj_draw_size.increase(ext_draw_size, ext_draw_size);
        let clip_area = self.draw.layers[layer.0].clip_area;

        if layer_type == LayerType::Transform {
            /*Get the transformed area and clip it to the current clip area.
             *This area needs to be updated on the screen.*/
            let mut tranf_coords = obj_draw_size;
            self.obj_get_transformed_area(obj, &mut tranf_coords, ObjPointTransformFlag::None);
            let clip_coords_for_obj = Area::intersect(&clip_area, &tranf_coords)?;

            /*Transform back (inverse) the transformed area.
             *It will tell which area of the non-transformed widget needs to be redrawn
             *in order to cover transformed area after transformation.*/
            let mut inverse_clip_coords_for_obj = clip_coords_for_obj;
            self.obj_get_transformed_area(
                obj,
                &mut inverse_clip_coords_for_obj,
                ObjPointTransformFlag::Inverse,
            );
            let inverse_clip_coords_for_obj = Area::intersect(&inverse_clip_coords_for_obj, &obj_draw_size)?;

            let mut layer_area_out = inverse_clip_coords_for_obj;
            layer_area_out.increase(5, 5); /*To avoid rounding error*/
            Some((layer_area_out, obj_draw_size))
        } else if layer_type == LayerType::Simple {
            let clip_coords_for_obj = Area::intersect(&clip_area, &obj_draw_size)?;
            Some((clip_coords_for_obj, obj_draw_size))
        } else {
            #[cfg(feature = "log")]
            log::warn!("Unhandled layer type");
            None
        }
    }

    /// `alpha_test_area_on_obj()`: `true` if `area` of the layer of `obj`
    /// may need an alpha channel.
    fn alpha_test_area_on_obj(&mut self, obj: Obj, area: &Area) -> bool {
        /*Test for alpha by assuming there is no alpha. If it fails, fall back to rendering with alpha*/
        /*If the layer area is not fully on the object, it can't fully cover it*/
        if !area.is_on(&self.obj_field_coords(obj)) {
            return true;
        }

        let mut info = CoverCheckInfo {
            res: CoverRes::Cover,
            area: *area,
        };
        self.obj_send_event(obj, EventCode::COVER_CHECK, EventParam::CoverCheck(&mut info));
        info.res != CoverRes::Cover
    }

    /// `get_max_row()`: how many rows of `area_w` width fit into the active
    /// buffer, rounded down through the display's `LV_EVENT_INVALIDATE_AREA`
    /// handlers (rounder). 0 on error.
    fn get_max_row(&mut self, disp: DisplayId, area_w: i32, area_h: i32) -> u32 {
        let Some(d) = self.display(disp) else {
            return 0;
        };
        let cf = d.color_format;
        let stride = DrawBuf::width_to_stride(area_w as u32, cf);
        let overhead = cf.indexed_palette_size() * 4; /*sizeof(lv_color32_t)*/

        if stride == 0 {
            #[cfg(feature = "log")]
            log::warn!("Invalid stride. Value is 0");
            return 0;
        }

        let data_size = buf_ref(d, d.buf_act).map_or(0, |b| b.data_size);
        let mut max_row = (data_size.wrapping_sub(overhead) / stride) as i32;

        if max_row > area_h {
            max_row = area_h;
        }

        /*Round down the lines of draw_buf if rounding is added*/
        let mut tmp = Area::new(0, 0, 0, 0);

        let mut h_tmp = max_row;
        loop {
            tmp.y2 = h_tmp - 1;
            let target = self.disp_refresh.unwrap_or(disp);
            self.display_send_event(target, EventCode::INVALIDATE_AREA, EventParam::AreaMut(&mut tmp));

            /*If this height fits into `max_row` then fine*/
            if tmp.height() <= max_row {
                break;
            }

            /*Decrement the height of the area until it fits into `max_row` after rounding*/
            h_tmp -= 1;
            if h_tmp <= 0 {
                break;
            }
        }

        if h_tmp <= 0 {
            #[cfg(feature = "log")]
            log::warn!(
                "Can't set draw_buf height using the round function. (Wrong round_cb or too small draw_buf)"
            );
            0
        } else {
            max_row = tmp.y2 + 1;
            max_row as u32
        }
    }

    /// `draw_buf_flush()`: finish the drawing of the display layer, flush
    /// the rendered buffer and swap the buffers if double buffered.
    fn draw_buf_flush(&mut self, disp: DisplayId) {
        /*Flush the rendered content to the display*/
        let Some(layer) = self.display(disp).and_then(|d| d.layer_head) else {
            return;
        };

        self.wait_for_layer_tasks(layer);

        /* Every layer must've been drawn*/
        debug_assert!(
            self.draw.layers.get(layer.0).is_none_or(|l| l.next.is_none()),
            "Expected every layer to have been drawn at this point"
        );

        /* In double buffered mode wait until the other buffer is freed
         * and driver is ready to receive the new buffer.
         * If we need to wait here it means that the content of one buffer is being sent to display
         * and other buffer already contains the new rendered image. */
        if self.display(disp).is_some_and(disp_is_double_buffered) {
            let target = self.disp_refresh.unwrap_or(disp);
            self.wait_for_flushing(target);
        }

        let Some(d) = self.display_mut(disp) else {
            return;
        };
        d.flushing = true;

        d.flushing_last = d.last_area && d.last_part;

        let flushing_last = d.flushing_last;
        let has_flush_cb = d.driver.is_some();
        let refreshed_area = d.refreshed_area;

        /*Take the rendered buffer back from the display layer and return it to its `buf_act` slot
         *before flushing, so `disp->buf_act` is valid while the flush callback and the
         *FLUSH_START / FLUSH_FINISH handlers run (like upstream)*/
        let buf = self.draw.layers.get_mut(layer.0).and_then(|l| l.draw_buf.take());
        let Some(d) = self.display_mut(disp) else {
            return;
        };
        let buf_act = d.buf_act;
        if let Some(slot) = buf_slot(d, buf_act)
            && slot.is_none()
        {
            *slot = buf;
        }

        if has_flush_cb {
            /*px_map = layer->draw_buf->data, i.e. the data of `buf_act`*/
            self.call_flush_cb(disp, &refreshed_area, buf_act);
        }

        let Some(d) = self.display_mut(disp) else {
            return;
        };

        /*If there are 2 buffers swap them. With direct mode swap only on the last area*/
        if disp_is_double_buffered(d) && (d.render_mode != RenderMode::Direct || flushing_last) {
            d.buf_act = next_buf_act(d);
        }
    }

    /// `call_flush_cb()`: send `LV_EVENT_FLUSH_START`, call the driver's
    /// `flush` with the offset area and send `LV_EVENT_FLUSH_FINISH`.
    ///
    /// `px_buf` selects the display buffer (`buf_1..=3`) whose data is the
    /// C `px_map`; it is borrowed from its slot only for the driver call,
    /// so the buffer stays reachable through `buf_act` for the event
    /// handlers.
    fn call_flush_cb(&mut self, disp: DisplayId, area: &Area, px_buf: u8) {
        let Some(d) = self.display(disp) else {
            return;
        };
        let mut offset_area = Area::new(
            area.x1 + d.offset_x,
            area.y1 + d.offset_y,
            area.x2 + d.offset_x,
            area.y2 + d.offset_y,
        );

        self.display_send_event(
            disp,
            EventCode::FLUSH_START,
            EventParam::AreaMut(&mut offset_area),
        );

        /* LV_COLOR_16_SWAP is not defined: no RGB565 swap */

        if let Some(d) = self.display_mut(disp)
            && let Some(mut driver) = d.driver.take()
        {
            let mut empty: [u8; 0] = [];
            let px_map: &mut [u8] = match buf_slot(d, px_buf)
                .and_then(|s| s.as_mut())
                .and_then(|b| b.data_mut())
            {
                Some(px) => px,
                None => &mut empty,
            };
            let res = driver.flush(disp, &offset_area, px_map);
            if let Some(d) = self.display_mut(disp) {
                if d.driver.is_none() {
                    d.driver = Some(driver);
                }
                if res == FlushResult::Done {
                    /*The driver called lv_display_flush_ready()*/
                    d.flushing = false;
                }
            }
        }
        self.display_send_event(
            disp,
            EventCode::FLUSH_FINISH,
            EventParam::AreaMut(&mut offset_area),
        );
    }

    /// `wait_for_flushing()`: wait until the driver finished the pending
    /// flush.
    fn wait_for_flushing(&mut self, disp: DisplayId) {
        self.display_send_event(disp, EventCode::FLUSH_WAIT_START, EventParam::None);

        let Some(d) = self.display_mut(disp) else {
            return;
        };
        if let Some(mut driver) = d.driver.take() {
            /*flush_wait_cb*/
            if d.flushing {
                driver.flush_wait(disp);
                d.flushing = false;
            }
            if d.driver.is_none() {
                d.driver = Some(driver);
            }
        } else {
            /* `while(disp->flushing);`: nothing can clear the flag on this
             * thread without a driver, so the wait ends immediately. */
            d.flushing = false;
        }
        d.flushing_last = false;

        self.display_send_event(disp, EventCode::FLUSH_WAIT_FINISH, EventParam::None);
    }

    /// `call_sync_cb()`: send `LV_EVENT_SYNC_START`, call the driver's
    /// `sync` with the offset area and send `LV_EVENT_SYNC_FINISH`.
    fn call_sync_cb(&mut self, disp: DisplayId, area: &Area) {
        let Some(d) = self.display(disp) else {
            return;
        };
        /* Apply display offsets to the sync area for consistency with flush_cb */
        let mut offset_area = *area;
        offset_area.x1 += d.offset_x;
        offset_area.x2 += d.offset_x;
        offset_area.y1 += d.offset_y;
        offset_area.y2 += d.offset_y;

        self.display_send_event(disp, EventCode::SYNC_START, EventParam::AreaMut(&mut offset_area));

        if let Some(mut driver) = self.display_mut(disp).and_then(|d| d.driver.take()) {
            driver.sync(disp, &offset_area);
            if let Some(d) = self.display_mut(disp)
                && d.driver.is_none()
            {
                d.driver = Some(driver);
            }
        }

        self.display_send_event(
            disp,
            EventCode::SYNC_FINISH,
            EventParam::AreaMut(&mut offset_area),
        );
    }

    /// `wait_for_syncing()`: wait until the driver finished the pending sync.
    fn wait_for_syncing(&mut self, disp: DisplayId) {
        self.display_send_event(disp, EventCode::SYNC_WAIT_START, EventParam::None);

        let Some(d) = self.display_mut(disp) else {
            return;
        };
        if let Some(mut driver) = d.driver.take() {
            /*sync_wait_cb*/
            if d.syncing {
                driver.sync_wait(disp);
                d.syncing = false;
            }
            if d.driver.is_none() {
                d.driver = Some(driver);
            }
        } else {
            /* `while(disp->syncing);` cannot end on this thread */
            d.syncing = false;
        }
        d.syncing_last = false;

        self.display_send_event(disp, EventCode::SYNC_WAIT_FINISH, EventParam::None);
    }

    /// `lv_obj_invalidate_expand_blur()` (`lv_obj_pos.c`): add the full
    /// extent of every blur object overlapping an invalidated area, until a
    /// pass adds no new area.
    pub fn obj_invalidate_expand_blur(&mut self, disp: DisplayId) {
        self.obj_pos_invalidate_expand_blur(disp);
    }

    /*=====================
     * LOCAL HELPERS
     *====================*/

    /// The display of the refresh timer `timer` (`timer->user_data`).
    fn refr_timer_display(&self, timer: TimerId) -> Option<DisplayId> {
        if let Some(ud) = self.timer_get_user_data(timer)
            && let Some(d) = ud.downcast_ref::<DisplayId>()
        {
            return Some(*d);
        }
        /*Fall back to the display owning this refresh timer*/
        self.display_order
            .iter()
            .copied()
            .find(|&d| self.display(d).is_some_and(|x| x.refr_timer == Some(timer)))
    }

    /// `while(layer->draw_task_head) { lv_draw_dispatch_wait_for_request(); lv_draw_dispatch(); }`
    fn wait_for_layer_tasks(&mut self, layer: LayerId) {
        while self.draw.layers.get(layer.0).is_some_and(|l| !l.tasks.is_empty()) {
            self.draw.dispatch_wait_for_request();
            self.draw_dispatch();
        }
    }

    /// `layer->_clip_area = area` (ignored for a stale layer).
    #[inline]
    fn set_layer_clip(&mut self, layer: LayerId, area: Area) {
        if let Some(l) = self.draw.layers.get_mut(layer.0) {
            l.clip_area = area;
        }
    }

    /// `obj->coords` (the zeroed area for a stale handle).
    #[inline]
    fn obj_field_coords(&self, obj: Obj) -> Area {
        self.obj(obj).map_or_else(Area::default, |o| o.coords)
    }

    /// `lv_obj_refr()` on the first `child_cnt` children of `obj`.
    fn refr_children_into(&mut self, obj: Obj, child_cnt: u32, layer: LayerId) {
        for i in 0..child_cnt {
            let Some(child) = self.child_at(obj, i) else {
                break;
            };
            self.obj_refr(layer, child);
            if !self.obj_is_valid(obj) || !self.draw.layers.contains(layer.0) {
                return;
            }
        }
    }

    /// `LV_EVENT_DRAW_POST_BEGIN`, `LV_EVENT_DRAW_POST`, `LV_EVENT_DRAW_POST_END`.
    fn send_draw_post_events(&mut self, obj: Obj, layer: LayerId) {
        for code in [
            EventCode::DRAW_POST_BEGIN,
            EventCode::DRAW_POST,
            EventCode::DRAW_POST_END,
        ] {
            if self.obj_send_event(obj, code, EventParam::Layer(layer)) == LvResult::Invalid
                && !self.obj_is_valid(obj)
            {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::UiConfig;
    use crate::misc::event::EventList;
    use alloc::boxed::Box;
    use alloc::rc::Rc;
    use alloc::vec;
    use core::any::Any;

    /// A zeroed `lv_display_t` (`lv_memzero` in `lv_display_create`) with the
    /// resolution and `LV_INV_BUF_SIZE` invalidation slots.
    fn disp_data(hor_res: i32, ver_res: i32) -> DisplayData {
        DisplayData {
            hor_res,
            ver_res,
            physical_hor_res: -1,
            physical_ver_res: -1,
            offset_x: 0,
            offset_y: 0,
            dpi: 130,
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
            antialiasing: true,
            tile_cnt: 1,
            stride_is_auto: true,
            rendering_in_progress: false,
            color_format: ColorFormat::Xrgb8888,
            inv_areas: vec![Area::default(); crate::display::INV_BUF_SIZE],
            inv_area_joined: vec![false; crate::display::INV_BUF_SIZE],
            inv_p: 0,
            inv_en_cnt: 1,
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
            rotation: crate::display::Rotation::R0,
            matrix_rotation: false,
            theme: None,
            refr_timer: None,
            last_activity_time: 0,
            refreshed_area: Area::default(),
            vsync_count: 0,
            ext_data: crate::display::ExtData {
                free_cb: None,
                data: None,
            },
        }
    }

    fn add_display(ui: &mut Ui, d: DisplayData) -> DisplayId {
        let id = DisplayId(ui.displays.insert(d));
        ui.display_order.insert(0, id);
        id
    }

    #[test]
    fn refresh_compositing_evaluates_layer_tasks() {
        struct LayerUnit;
        impl crate::draw::DrawUnit for LayerUnit {
            fn name(&self) -> &'static str {
                "layer-test"
            }

            fn evaluate(&mut self, task: &mut crate::draw::DrawTask) -> i32 {
                if task.ty == crate::draw::DrawTaskType::Layer {
                    task.preferred_draw_unit_id = 1;
                    task.preference_score = 10;
                    1
                } else {
                    0
                }
            }

            fn dispatch(&mut self, _ctx: &mut crate::draw::DrawCtx, _layer: LayerId) -> i32 {
                -1
            }
        }

        let mut ui = Ui::new(UiConfig::default());
        let disp = add_display(&mut ui, disp_data(10, 10));
        ui.disp_refresh = Some(disp);
        ui.draw_create_unit(Box::new(LayerUnit));
        let coords = Area::new(1, 1, 8, 8);
        let obj = crate::core::obj_class::tests::raw_obj(&mut ui, &crate::core::obj::OBJ_CLASS, None);
        ui.obj_mut(obj).unwrap().coords = coords;
        let layer = ui.draw_layer_create(None, ColorFormat::Xrgb8888, &Area::new(0, 0, 9, 9));

        // Keep tasks queued so the test can inspect the evaluated composite
        // from both rounded corner bands and the opacity/transform path.
        ui.draw.task_running = true;
        ui.obj_redraw_clip_corner(layer, obj, 0, 2);
        ui.obj_refr_layered(layer, obj, LayerType::Simple, 128, &coords, &coords);
        ui.draw.task_running = false;

        let tasks: Vec<_> = ui
            .draw
            .layer(layer)
            .unwrap()
            .tasks
            .iter()
            .filter(|task| task.ty == crate::draw::DrawTaskType::Layer)
            .collect();
        assert_eq!(tasks.len(), 3);
        for task in tasks {
            assert_eq!(task.preferred_draw_unit_id, 1);
            assert_eq!(task.preference_score, 10);
            // Composition stays blocked until dispatch finishes the source
            // layer; selecting a draw unit must already have happened.
            assert_eq!(task.state, crate::draw::DrawTaskState::Blocked);
        }
    }

    #[test]
    fn user_sync_copies_only_remaining_areas_and_waits() {
        use core::cell::{Cell, RefCell};
        struct SyncDriver {
            enabled: bool,
            copied: Rc<RefCell<Vec<Area>>>,
            waits: Rc<Cell<usize>>,
        }

        impl crate::display::DisplayDriver for SyncDriver {
            fn flush(&mut self, _disp: DisplayId, _area: &Area, _px_map: &mut [u8]) -> FlushResult {
                FlushResult::Done
            }

            fn has_sync(&self) -> bool {
                self.enabled
            }

            fn sync(&mut self, _disp: DisplayId, area: &Area) {
                self.copied.borrow_mut().push(*area);
            }

            fn sync_wait(&mut self, _disp: DisplayId) {
                self.waits.set(self.waits.get() + 1);
            }
        }

        let mut ui = Ui::new(UiConfig::default());
        let copied = Rc::new(RefCell::new(Vec::new()));
        let waits = Rc::new(Cell::new(0));
        let mut data = disp_data(10, 10);
        data.offset_x = 2;
        data.offset_y = 3;
        data.sync_areas = vec![Area::new(0, 0, 5, 5), Area::new(8, 8, 12, 12)];
        data.inv_areas[0] = Area::new(0, 0, 5, 2);
        data.inv_p = 1;
        let disp = add_display(&mut ui, data);
        // Partial mode with one buffer has no automatic sync; a driver must
        // explicitly advertise sync support before the callback is called.
        for enabled in [false, true] {
            ui.display_set_driver(
                disp,
                Some(Box::new(SyncDriver {
                    enabled,
                    copied: copied.clone(),
                    waits: waits.clone(),
                })),
            );
            ui.refr_sync_areas(disp);
            if !enabled {
                assert!(copied.borrow().is_empty());
                assert_eq!(ui.display(disp).unwrap().sync_areas.len(), 2);
            }
        }
        assert_eq!(
            *copied.borrow(),
            vec![Area::new(2, 6, 7, 8), Area::new(10, 11, 11, 12)]
        );
        assert_eq!(waits.get(), 2);
        let data = ui.display(disp).unwrap();
        assert!(data.sync_areas.is_empty());
        assert!(!data.syncing);
        assert!(!data.syncing_last);
    }

    #[test]
    fn inv_area_store_dedups_and_overflows_to_screen() {
        let mut d = disp_data(800, 480);
        let scr = Area::new(0, 0, 799, 479);
        assert!(inv_area_store(&mut d, &Area::new(10, 10, 50, 50), &scr, 32));
        assert_eq!(d.inv_p, 1);
        /*Fully inside a saved area: not saved again*/
        assert!(!inv_area_store(&mut d, &Area::new(20, 20, 30, 30), &scr, 32));
        assert_eq!(d.inv_p, 1);
        /*Partially outside: saved*/
        assert!(inv_area_store(&mut d, &Area::new(40, 40, 60, 60), &scr, 32));
        assert_eq!(d.inv_p, 2);
        assert_eq!(d.inv_areas[1], Area::new(40, 40, 60, 60));

        /*Fill every slot, the next one collapses to the whole screen*/
        let mut d = disp_data(800, 480);
        for i in 0..32 {
            assert!(inv_area_store(
                &mut d,
                &Area::new(i * 20, 0, i * 20 + 5, 5),
                &scr,
                32
            ));
        }
        assert_eq!(d.inv_p, 32);
        assert!(inv_area_store(&mut d, &Area::new(700, 400, 710, 410), &scr, 32));
        assert_eq!(d.inv_p, 1);
        assert_eq!(d.inv_areas[0], scr);
        /*Afterwards everything is inside the screen area*/
        assert!(!inv_area_store(&mut d, &Area::new(1, 1, 2, 2), &scr, 32));
    }

    #[test]
    fn inv_area_store_grows_unallocated_slots() {
        let mut d = disp_data(100, 100);
        d.inv_areas.clear();
        d.inv_area_joined.clear();
        let scr = Area::new(0, 0, 99, 99);
        assert!(inv_area_store(&mut d, &Area::new(1, 1, 5, 5), &scr, 32));
        assert!(inv_area_store(&mut d, &Area::new(10, 10, 15, 15), &scr, 32));
        assert_eq!(
            d.inv_areas,
            vec![Area::new(1, 1, 5, 5), Area::new(10, 10, 15, 15)]
        );
        assert!(!inv_joined_at(&d, 1));
    }

    #[test]
    fn join_only_when_the_joined_area_is_smaller() {
        let mut d = disp_data(800, 480);
        d.inv_areas[0] = Area::new(0, 0, 9, 9);
        d.inv_areas[1] = Area::new(0, 5, 9, 14); /*joined: 10x15 = 150 < 200*/
        d.inv_areas[2] = Area::new(100, 100, 109, 109); /*not touching*/
        d.inv_areas[3] = Area::new(105, 105, 114, 114); /*joined: 15x15 = 225 > 200*/
        d.inv_p = 4;
        join_areas(&mut d);
        assert_eq!(d.inv_areas[0], Area::new(0, 0, 9, 14));
        assert_eq!(&d.inv_area_joined[..4], &[false, true, false, false]);
        assert_eq!(d.inv_areas[2], Area::new(100, 100, 109, 109));
        assert_eq!(d.inv_areas[3], Area::new(105, 105, 114, 114));
    }

    #[test]
    fn join_chains_into_the_first_area() {
        /*`join_in` keeps growing while `join_from` walks every other area*/
        let mut d = disp_data(800, 480);
        d.inv_areas[0] = Area::new(0, 0, 9, 9);
        d.inv_areas[1] = Area::new(0, 8, 9, 17);
        d.inv_areas[2] = Area::new(0, 16, 9, 25);
        d.inv_p = 3;
        join_areas(&mut d);
        assert_eq!(d.inv_areas[0], Area::new(0, 0, 9, 25));
        assert_eq!(&d.inv_area_joined[..3], &[false, true, true]);
    }

    #[test]
    fn refr_join_areas_on_a_display() {
        let mut ui = Ui::new(UiConfig::default());
        let mut d = disp_data(800, 480);
        d.inv_areas[0] = Area::new(0, 0, 99, 9);
        d.inv_areas[1] = Area::new(0, 5, 99, 19);
        d.inv_p = 2;
        let disp = add_display(&mut ui, d);
        ui.refr_join_areas(disp);
        let d = ui.display(disp).unwrap();
        assert_eq!(d.inv_areas[0], Area::new(0, 0, 99, 19));
        assert!(d.inv_area_joined[1]);
    }

    #[test]
    fn sync_areas_keep_only_the_parts_not_redrawn() {
        let mut sync = vec![Area::new(0, 0, 99, 99), Area::new(200, 200, 209, 209)];
        sync_areas_remove(&mut sync, &Area::new(0, 0, 99, 49));
        /*The first area loses its top half, the second is untouched*/
        assert_eq!(
            sync,
            vec![Area::new(0, 50, 99, 99), Area::new(200, 200, 209, 209)]
        );
        /*A fully covered area disappears*/
        sync_areas_remove(&mut sync, &Area::new(150, 150, 250, 250));
        assert_eq!(sync, vec![Area::new(0, 50, 99, 99)]);
        /*A hole in the middle leaves four parts in diff order*/
        let mut sync = vec![Area::new(0, 0, 9, 9)];
        sync_areas_remove(&mut sync, &Area::new(3, 3, 5, 5));
        assert_eq!(
            sync,
            vec![
                Area::new(0, 0, 9, 2),
                Area::new(0, 6, 9, 9),
                Area::new(0, 3, 2, 5),
                Area::new(6, 3, 9, 5),
            ]
        );
    }

    #[test]
    fn buffer_rotation_matches_draw_buf_flush() {
        let buf = || DrawBuf::create(4, 4, ColorFormat::Xrgb8888, 0);
        let mut d = disp_data(4, 4);
        d.buf_1 = buf();
        d.buf_act = 1;
        assert!(!disp_is_double_buffered(&d));
        assert_eq!(next_buf_act(&d), 2);

        d.buf_2 = buf();
        assert!(disp_is_double_buffered(&d));
        d.buf_act = 2;
        assert_eq!(next_buf_act(&d), 1);
        /*buf_2 lent to the display layer: still double buffered*/
        let lent = d.buf_2.take();
        assert!(disp_is_double_buffered(&d));
        d.buf_2 = lent;

        d.buf_3 = buf();
        assert_eq!(next_buf_act(&d), 3);
        d.buf_act = 3;
        let lent = d.buf_3.take();
        assert!(disp_has_buf_3(&d));
        assert_eq!(next_buf_act(&d), 1);
        d.buf_3 = lent;
    }

    #[test]
    fn sync_buffer_relations() {
        let buf = || DrawBuf::create(4, 4, ColorFormat::Xrgb8888, 0);
        let mut d = disp_data(4, 4);
        d.buf_1 = buf();
        d.buf_2 = buf();
        d.buf_act = 1;
        assert_eq!(sync_buf_relations(&d), (2, 2));
        d.buf_act = 2;
        assert_eq!(sync_buf_relations(&d), (1, 1));
        d.buf_3 = buf();
        d.buf_act = 1;
        assert_eq!(sync_buf_relations(&d), (2, 3));
        d.buf_act = 2;
        assert_eq!(sync_buf_relations(&d), (3, 1));
        d.buf_act = 3;
        assert_eq!(sync_buf_relations(&d), (1, 2));
    }

    #[test]
    fn copy_between_display_buffers() {
        let mut d = disp_data(4, 4);
        let mut src = DrawBuf::create(4, 4, ColorFormat::Xrgb8888, 0).unwrap();
        src.data_mut().unwrap().fill(0xAB);
        d.buf_1 = DrawBuf::create(4, 4, ColorFormat::Xrgb8888, 0);
        d.buf_2 = Some(src);
        copy_disp_buf(&mut d, 1, 2, &Area::new(1, 1, 2, 2));
        let dst = d.buf_1.as_ref().unwrap();
        let stride = dst.header.stride as usize;
        for y in 0..4 {
            for x in 0..4 {
                let inside = (1..=2).contains(&x) && (1..=2).contains(&y);
                let px = &dst.data()[y * stride + x * 4..y * stride + x * 4 + 4];
                assert_eq!(px == [0xAB; 4], inside, "({x};{y})");
            }
        }
        assert!(d.buf_2.is_some());
        /*Copying onto itself or from a missing buffer is a no-op*/
        copy_disp_buf(&mut d, 1, 1, &Area::new(0, 0, 3, 3));
        copy_disp_buf(&mut d, 1, 3, &Area::new(0, 0, 3, 3));
        assert!(d.buf_1.is_some());
    }

    #[test]
    fn partial_mode_chunking() {
        let inv = Area::new(5, 10, 104, 59); /*50 rows*/
        let parts = partial_sub_areas(&inv, 20);
        assert_eq!(
            parts,
            vec![
                (Area::new(5, 10, 104, 29), 0, false),
                (Area::new(5, 30, 104, 49), 20, false),
                (Area::new(5, 50, 104, 59), 40, true),
            ]
        );
        /*An exact multiple ends inside the loop*/
        let parts = partial_sub_areas(&Area::new(0, 0, 9, 39), 20);
        assert_eq!(
            parts,
            vec![
                (Area::new(0, 0, 9, 19), 0, false),
                (Area::new(0, 20, 9, 39), 20, true)
            ]
        );
        /*A buffer larger than the area: one part*/
        let parts = partial_sub_areas(&Area::new(0, 0, 9, 9), 10);
        assert_eq!(parts, vec![(Area::new(0, 0, 9, 9), 0, true)]);
    }

    #[test]
    fn tile_split_and_tile_areas() {
        let area = Area::new(0, 0, 799, 479);
        let full = 800 * 480 * 4;
        assert_eq!(tile_split(&area, ColorFormat::Xrgb8888, 1, full), (1, 480));
        let (cnt, h) = tile_split(&area, ColorFormat::Xrgb8888, 4, full);
        assert_eq!((cnt, h), (4, 120));
        assert_eq!(tile_area(&area, 0, cnt, h), Area::new(0, 0, 799, 119));
        assert_eq!(tile_area(&area, 3, cnt, h), Area::new(0, 360, 799, 479));
        /*Uneven split: the last tile takes the remainder*/
        let area = Area::new(0, 10, 99, 110); /*101 rows*/
        let (cnt, h) = tile_split(&area, ColorFormat::Xrgb8888, 2, 100 * 101 * 4);
        assert_eq!((cnt, h), (2, 50));
        assert_eq!(tile_area(&area, 0, cnt, h), Area::new(0, 10, 99, 59));
        assert_eq!(tile_area(&area, 1, cnt, h), Area::new(0, 60, 99, 110));
        /*Indexed formats are never tiled*/
        assert_eq!(tile_split(&area, ColorFormat::I1, 4, 100), (1, 101));
    }

    #[test]
    fn disp_refreshing_roundtrip() {
        let mut ui = Ui::new(UiConfig::default());
        assert_eq!(ui.refr_get_disp_refreshing(), None);
        let disp = add_display(&mut ui, disp_data(10, 10));
        ui.refr_set_disp_refreshing(Some(disp));
        assert_eq!(ui.refr_get_disp_refreshing(), Some(disp));
        ui.refr_set_disp_refreshing(None);
        assert_eq!(ui.refr_get_disp_refreshing(), None);
        ui.refr_init();
        ui.refr_deinit();
    }

    #[test]
    fn refr_timer_resolves_its_display() {
        let mut ui = Ui::new(UiConfig::default());
        let disp = add_display(&mut ui, disp_data(10, 10));
        let cb: crate::misc::timer::TimerCallback = Rc::new(|_, _| {});
        let ud: Rc<dyn Any> = Rc::new(disp);
        let t = ui.timer_create(cb.clone(), 33, Some(ud));
        assert_eq!(ui.refr_timer_display(t), Some(disp));

        /*Without user data: the display owning the timer*/
        let t2 = ui.timer_create(cb, 33, None);
        assert_eq!(ui.refr_timer_display(t2), None);
        ui.display_mut(disp).unwrap().refr_timer = Some(t2);
        assert_eq!(ui.refr_timer_display(t2), Some(disp));
    }

    #[test]
    fn refr_timer_without_display_or_buffer_returns_early() {
        let mut ui = Ui::new(UiConfig::default());
        /*No display registered*/
        ui.display_refr_timer(None);
        assert_eq!(ui.refr_get_disp_refreshing(), None);

        /*A default display without draw buffer: nothing is sent*/
        let mut d = disp_data(10, 10);
        d.inv_p = 3;
        let disp = add_display(&mut ui, d);
        ui.disp_default = Some(disp);
        ui.display_refr_timer(None);
        assert_eq!(ui.refr_get_disp_refreshing(), Some(disp));
        assert_eq!(ui.display(disp).unwrap().inv_p, 3);

        /*An empty buffer counts as no buffer*/
        let d = ui.display_mut(disp).unwrap();
        d.buf_1 = DrawBuf::create(0, 0, ColorFormat::Xrgb8888, 0);
        d.buf_act = 1;
        ui.display_refr_timer(None);
        assert_eq!(ui.display(disp).unwrap().inv_p, 3);
    }

    #[test]
    fn early_returns_without_work() {
        let mut ui = Ui::new(UiConfig::default());
        /*No default display*/
        assert_eq!(ui.inv_area(None, Some(&Area::new(0, 0, 1, 1))), LvResult::Invalid);

        let disp = add_display(&mut ui, disp_data(10, 10));
        /*Nothing invalidated: no RENDER_START / blur walk*/
        ui.refr_invalid_areas(disp);
        ui.obj_invalidate_expand_blur(disp);
        assert!(!ui.display(disp).unwrap().rendering_in_progress);

        /*Invalidated areas but no blur object: no walk either*/
        ui.display_mut(disp).unwrap().inv_p = 1;
        ui.obj_invalidate_expand_blur(disp);
        assert_eq!(ui.display(disp).unwrap().inv_p, 1);
    }

    #[test]
    fn blur_expansion_repeats_until_overlapping_objects_are_covered() {
        use crate::core::obj::OBJ_CLASS;
        use crate::core::obj_class::tests::raw_obj;

        let mut ui = Ui::new(UiConfig::default());
        let screen = raw_obj(&mut ui, &OBJ_CLASS, None);
        ui.obj_mut(screen).unwrap().coords = Area::new(0, 0, 99, 99);
        let mut data = disp_data(100, 100);
        data.screens.push(screen.id());
        data.act_scr = Some(screen.id());
        let disp = add_display(&mut ui, data);
        ui.disp_default = Some(disp);

        // Walk the furthest object first. It only overlaps an area added
        // when the second object is visited, so another pass is required.
        let farther = raw_obj(&mut ui, &OBJ_CLASS, Some(screen));
        let nearer = raw_obj(&mut ui, &OBJ_CLASS, Some(screen));
        let farther_area = Area::new(25, 10, 39, 19);
        let nearer_area = Area::new(10, 10, 29, 19);
        for (obj, coords) in [(farther, farther_area), (nearer, nearer_area)] {
            let data = ui.obj_mut(obj).unwrap();
            data.coords = coords;
            data.has_blur = true;
        }
        ui.blur_obj_cnt = 2;
        ui.inv_area(Some(disp), Some(&Area::new(10, 10, 11, 11)));
        ui.obj_invalidate_expand_blur(disp);

        let data = ui.display(disp).unwrap();
        assert_eq!(data.inv_p, 3);
        assert_eq!(&data.inv_areas[1..3], &[nearer_area, farther_area]);
        ui.obj_invalidate_expand_blur(disp);
        assert_eq!(ui.display(disp).unwrap().inv_p, 3);
    }

    struct EventLogUnit {
        name: &'static str,
        events: Rc<core::cell::RefCell<Vec<(&'static str, EventCode)>>>,
    }

    impl crate::draw::DrawUnit for EventLogUnit {
        fn name(&self) -> &'static str {
            self.name
        }

        fn evaluate(&mut self, _task: &mut crate::draw::DrawTask) -> i32 {
            0
        }

        fn dispatch(&mut self, _ctx: &mut crate::draw::DrawCtx, _layer: LayerId) -> i32 {
            -1
        }

        fn on_event(&mut self, code: EventCode, _param: &mut EventParam<'_>) {
            self.events.borrow_mut().push((self.name, code));
        }
    }

    #[test]
    fn refr_finish_cleans_only_the_sw_mask_cache() {
        /*`lv_draw_sw_mask_cleanup()` is the only draw-unit side effect of refr_finish:
         *the request reaches the SW unit, no other unit is notified*/
        let mut ui = Ui::new(UiConfig::default());
        let events = Rc::new(core::cell::RefCell::new(Vec::new()));
        for name in ["OTHER", DRAW_SW_UNIT_NAME] {
            ui.draw_units.push(Box::new(EventLogUnit {
                name,
                events: events.clone(),
            }));
        }
        ui.refr_finish_draw_sw_mask_cleanup();
        assert_eq!(*events.borrow(), vec![(DRAW_SW_UNIT_NAME, EventCode::REFR_READY)]);
    }
}
