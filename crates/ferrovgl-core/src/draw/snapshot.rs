//! Object snapshots, ported from LVGL v9.6.0 `src/draw/snapshot/lv_snapshot.c`.
//!
//! Snapshots own a [`DrawBuf`] and can reuse its allocation. Rust's `Drop` and
//! [`DrawBuf::init`] replace the deprecated `lv_snapshot_free` / raw-pointer
//! `lv_snapshot_take_to_buf` functions. Rendering uses the registered draw units;
//! the accepted formats below do not imply that every backend implements them.
//!
//! The requested format is used when reshaping, as documented by `lv_snapshot.h`.
//! Upstream's implementation instead retains the previous format, which can
//! leave the header/stride inconsistent with the requested destination format.

use alloc::vec::Vec;

use super::draw_buf::{BufStorage, DRAW_BUF_ALIGN, DrawBuf, ImageHeader, STRIDE_AUTO};
use super::{LayerId, send_layer_event};
use crate::LvResult;
use crate::core::Ui;
use crate::core::obj::Obj;
use crate::misc::area::Area;
use crate::misc::color::ColorFormat;
use crate::misc::event::{EventCode, EventParam};

const MAX_STALLED_DISPATCHES: u32 = 512;

impl Ui {
    /// `lv_snapshot_create_draw_buf()`: allocate for the object's current size,
    /// including its extended draw area. Updates layout before measuring.
    /// Invalid/deleted objects and unrepresentable or empty sizes return `None`.
    pub fn snapshot_create_draw_buf(&mut self, obj: Obj, cf: ColorFormat) -> Option<DrawBuf> {
        let area = self.snapshot_area(obj)?;
        let (w, h, _) = checked_geometry(&area, cf)?;
        DrawBuf::create(w, h, cf, STRIDE_AUTO)
    }

    /// `lv_snapshot_reshape_draw_buf()`: update layout and reshape an existing
    /// writable buffer, keeping its format. Never reallocates; failure leaves
    /// the buffer's header and storage unchanged.
    pub fn snapshot_reshape_draw_buf(&mut self, obj: Obj, draw_buf: &mut DrawBuf) -> LvResult {
        let Some(area) = self.snapshot_area(obj) else {
            return LvResult::Invalid;
        };
        reshape_checked(draw_buf, &area, draw_buf.header.cf)
    }

    /// `lv_snapshot_take()`: render an object and its descendants into a newly
    /// allocated buffer. Its own transform/opacity is treated like upstream's
    /// `lv_obj_redraw`; transforms on descendants are rendered normally.
    pub fn snapshot_take(&mut self, obj: Obj, cf: ColorFormat) -> Option<DrawBuf> {
        if !supported_format(cf) {
            return None;
        }
        let mut draw_buf = self.snapshot_create_draw_buf(obj, cf)?;
        (self.snapshot_take_to_draw_buf(obj, cf, &mut draw_buf) == LvResult::Ok).then_some(draw_buf)
    }

    /// `lv_snapshot_take_to_draw_buf()`: render into reusable storage, reshaping
    /// to `cf` if it fits. On render failure the buffer remains owned by the
    /// caller, but its pixels may be partially rendered. Object/display deletion
    /// in a callback aborts safely. Directly deleting the callback's draw layer
    /// or replacing `Ui::draw` consumes its buffer and is not supported.
    ///
    /// Restores the previous refresh display and draw queues, including for
    /// nested snapshots. A stalled draw queue fails after 513 dispatches without
    /// a change in the total pending task count, matching upstream.
    pub fn snapshot_take_to_draw_buf(
        &mut self,
        obj: Obj,
        cf: ColorFormat,
        draw_buf: &mut DrawBuf,
    ) -> LvResult {
        if !supported_format(cf) {
            return LvResult::Invalid;
        }
        let Some(area) = self.snapshot_area(obj) else {
            return LvResult::Invalid;
        };
        if reshape_checked(draw_buf, &area, cf) != LvResult::Ok {
            return LvResult::Invalid;
        }
        let Some(disp) = self.obj_get_display(obj) else {
            return LvResult::Invalid;
        };
        let top = self.refr_get_top_obj(&area, obj);
        if !self.obj_is_valid(obj) || self.display(disp).is_none() {
            return LvResult::Invalid;
        }
        if top.is_none() {
            draw_buf.clear(None);
        }
        let top = top.unwrap_or(obj);

        // DrawCtx owns the dispatch chains in Rust. Isolating these, rather than
        // changing DisplayData.layer_head, also keeps the caller-owned buffer
        // alive if a drawing callback deletes the display or deinitializes Ui.
        let previous_heads = core::mem::take(&mut self.draw.layer_heads);
        let previous_display = self.disp_refresh;
        let previous_request = self.draw.dispatch_req;
        self.disp_refresh = Some(disp);
        let layer = self.draw.layer_create_impl(None, None, cf, &area, Some(disp));
        self.draw.layers[layer.0].draw_buf = Some(core::mem::replace(draw_buf, empty_buffer()));
        send_layer_event(Some(&mut self.draw_units), EventCode::CHILD_CREATED, layer);

        let mut result = self.snapshot_redraw(layer, obj, top);
        if self.obj_is_valid(obj) && self.display(disp).is_some() {
            if let Some(data) = self.draw.layer_mut(layer) {
                data.all_tasks_added = true;
            }
            if result == LvResult::Ok {
                result = self.snapshot_finish(layer);
            }
        } else {
            result = LvResult::Invalid;
        }

        // Wait before reclaiming either the root buffer or dependent layers.
        // This is required on the stalled path for asynchronous draw units.
        if result != LvResult::Ok {
            self.draw.wait_for_finish(&mut self.draw_units);
        }
        if let Some(buffer) = self.draw.layer_mut(layer).and_then(|data| data.draw_buf.take()) {
            *draw_buf = buffer;
        } else {
            result = LvResult::Invalid;
        }

        self.disp_refresh = previous_display.filter(|d| self.display(*d).is_some());
        if result == LvResult::Ok {
            send_layer_event(Some(&mut self.draw_units), EventCode::SCREEN_LOAD_START, layer);
        }

        // Some callbacks can stop object drawing after a child layer was
        // created but before its parent's LAYER task was emitted. Retain the
        // chain before deleting the root so these otherwise orphaned layers
        // are freed too; ordinary task cleanup recursively frees owned layers.
        let layers = self.snapshot_owned_layers(layer);
        self.draw_layer_delete(layer);
        for child in layers.into_iter().rev() {
            self.draw_layer_delete(child);
        }

        // Keep any independent display/layer created by callbacks. Old heads
        // whose displays were deleted must not be resurrected on restoration.
        let new_heads = core::mem::take(&mut self.draw.layer_heads);
        self.draw.layer_heads = previous_heads
            .into_iter()
            .filter(|(display, head)| {
                self.draw.layers.contains(head.0) && display.is_none_or(|d| self.display(d).is_some())
            })
            .collect();
        for (display, head) in new_heads {
            if self.draw.layers.contains(head.0) && display.is_none_or(|d| self.display(d).is_some()) {
                self.draw.chain_append(display, head);
            }
        }
        self.draw.dispatch_req = previous_request;
        result
    }

    fn snapshot_area(&mut self, obj: Obj) -> Option<Area> {
        if !self.obj_is_valid(obj) {
            return None;
        }
        self.obj_update_layout(obj);
        if !self.obj_is_valid(obj) {
            return None;
        }
        let coords = self.obj_get_coords(obj);
        let ext = self.obj_get_ext_draw_size(obj);
        Some(Area::new(
            coords.x1.checked_sub(ext)?,
            coords.y1.checked_sub(ext)?,
            coords.x2.checked_add(ext)?,
            coords.y2.checked_add(ext)?,
        ))
    }

    fn snapshot_redraw(&mut self, layer: LayerId, obj: Obj, top: Obj) -> LvResult {
        if !self.obj_is_valid(top) {
            return LvResult::Invalid;
        }
        if top == obj {
            self.obj_redraw(layer, top);
            return if self.obj_is_valid(obj) && self.draw.layers.contains(layer.0) {
                LvResult::Ok
            } else {
                LvResult::Invalid
            };
        }
        self.obj_refr(layer, top);
        if !self.obj_is_valid(top) || !self.obj_is_valid(obj) || !self.draw.layers.contains(layer.0) {
            return LvResult::Invalid;
        }
        let mut border = top;
        while border != obj {
            let Some(parent) = self.obj_get_parent(border) else {
                return LvResult::Invalid;
            };
            let mut go = false;
            let child_count = self.obj_get_child_count(parent);
            for i in 0..child_count {
                let Some(child) = self.obj_get_child(parent, i as i32) else {
                    continue;
                };
                if go {
                    self.obj_refr(layer, child);
                    if !self.obj_is_valid(parent)
                        || !self.obj_is_valid(obj)
                        || !self.draw.layers.contains(layer.0)
                    {
                        return LvResult::Invalid;
                    }
                } else if child == border {
                    go = true;
                }
            }
            for code in [
                EventCode::DRAW_POST_BEGIN,
                EventCode::DRAW_POST,
                EventCode::DRAW_POST_END,
            ] {
                self.obj_send_event(parent, code, EventParam::Layer(layer));
                if !self.obj_is_valid(parent)
                    || !self.obj_is_valid(obj)
                    || !self.draw.layers.contains(layer.0)
                {
                    return LvResult::Invalid;
                }
            }
            border = parent;
        }
        LvResult::Ok
    }

    fn snapshot_owned_layers(&self, root: LayerId) -> Vec<LayerId> {
        let mut layers = Vec::new();
        let mut current = Some(root);
        while let Some(layer) = current {
            let Some(data) = self.draw.layer(layer) else {
                break;
            };
            // `next` is a display dispatch chain, not an ownership chain. A
            // callback can append an independent root (and its children) to
            // the same display while this snapshot is being drawn.
            let mut ancestor = Some(layer);
            while let Some(id) = ancestor {
                if id == root {
                    layers.push(layer);
                    break;
                }
                ancestor = self.draw.layer(id).and_then(|data| data.parent);
            }
            current = data.next;
        }
        layers
    }

    fn snapshot_pending_count(&self, root: LayerId) -> usize {
        let mut count = 0;
        let mut current = Some(root);
        while let Some(layer) = current {
            let Some(data) = self.draw.layer(layer) else {
                break;
            };
            count += data.tasks.len();
            current = data.next;
        }
        count
    }

    fn snapshot_finish(&mut self, layer: LayerId) -> LvResult {
        let mut stalled = 0;
        let mut last_count = self.snapshot_pending_count(layer);
        loop {
            let Some(data) = self.draw.layer(layer) else {
                return LvResult::Invalid;
            };
            if data.tasks.is_empty() {
                return LvResult::Ok;
            }
            self.draw_dispatch();
            let count = self.snapshot_pending_count(layer);
            if count != last_count {
                last_count = count;
                stalled = 0;
            } else {
                stalled += 1;
                if stalled > MAX_STALLED_DISPATCHES {
                    return LvResult::Invalid;
                }
            }
        }
    }
}

fn supported_format(cf: ColorFormat) -> bool {
    matches!(
        cf,
        ColorFormat::Rgb565
            | ColorFormat::Argb8565
            | ColorFormat::Rgb888
            | ColorFormat::Xrgb8888
            | ColorFormat::Argb8888
            | ColorFormat::A8
            | ColorFormat::L8
            | ColorFormat::I1
            | ColorFormat::Argb2222
            | ColorFormat::Argb4444
            | ColorFormat::Argb1555
    )
}

// Validate before calling DrawBuf, whose upstream-compatible geometry uses
// wrapping arithmetic and truncates fields to u16. Include indexed palettes
// and allocation rounding in capacity checks.
fn checked_geometry(area: &Area, cf: ColorFormat) -> Option<(u32, u32, usize)> {
    let w = u32::try_from(i64::from(area.x2) - i64::from(area.x1) + 1).ok()?;
    let h = u32::try_from(i64::from(area.y2) - i64::from(area.y1) + 1).ok()?;
    if w == 0 || h == 0 || w > u32::from(u16::MAX) || h > u32::from(u16::MAX) || cf.bpp() == 0 {
        return None;
    }
    let stride = DrawBuf::width_to_stride(w, cf);
    if stride > u32::from(u16::MAX) {
        return None;
    }
    let mut size = stride.checked_mul(h)?;
    if cf == ColorFormat::Rgb565A8 {
        size = size.checked_add((stride / 2).checked_mul(h)?)?;
    } else if cf.is_indexed() {
        size = size.checked_add(cf.indexed_palette_size().checked_mul(4)?)?;
    }
    let size = size.checked_add(DRAW_BUF_ALIGN as u32 - 1)? / DRAW_BUF_ALIGN as u32 * DRAW_BUF_ALIGN as u32;
    if (size as usize).checked_add(DRAW_BUF_ALIGN - 1)? > isize::MAX as usize {
        return None;
    }
    Some((w, h, size as usize))
}

fn reshape_checked(buf: &mut DrawBuf, area: &Area, cf: ColorFormat) -> LvResult {
    let Some((w, h, size)) = checked_geometry(area, cf) else {
        return LvResult::Invalid;
    };
    if buf.data_offset.checked_add(buf.data_size as usize).is_some()
        && buf.data_size as usize >= size
        && buf.data_mut().is_some_and(|data| data.len() >= size)
        && buf.reshape(cf, w, h, STRIDE_AUTO)
    {
        LvResult::Ok
    } else {
        LvResult::Invalid
    }
}

fn empty_buffer() -> DrawBuf {
    DrawBuf {
        header: ImageHeader::default(),
        data_size: 0,
        storage: BufStorage::Static(&[]),
        data_offset: 0,
    }
}
