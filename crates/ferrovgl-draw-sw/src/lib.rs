//! Software renderer for ferrovgl.
//!
//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw.c`, `src/draw/sw/lv_draw_sw_private.h`.
//! Each colour format is behind a `cf-*` feature so that unused blend paths
//! are not compiled on small targets.
//!
//! The only `unsafe` code of this crate lives in `blend::words`.

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![allow(unused_variables)] // skeleton: bodies are todo!()

extern crate alloc;

pub mod arc;
pub mod blend;
pub mod blur;
pub mod border;
pub mod box_shadow;
pub mod fill;
pub mod grad;
pub mod img;
pub mod label;
pub mod layer;
pub mod letter;
pub mod line;
pub mod mask;
pub mod mask_rect;
pub mod transform;
pub mod triangle;

use alloc::boxed::Box;
use alloc::vec::Vec;

use ferrovgl_core::core::Ui;
use ferrovgl_core::draw::{DrawCtx, DrawTask, DrawTaskRef, DrawUnit, LayerId};
use ferrovgl_core::misc::color::ColorFormat;
use ferrovgl_core::misc::event::{EventCode, EventParam};

use crate::blend::{BlendHandler, CustomBlendHandler};

/// Name reported by the software draw unit.
pub const DRAW_UNIT_NAME: &str = "SW";

/// `LV_DRAW_SW_DRAW_UNIT_CNT` (lv_conf default).
pub const DRAW_UNIT_CNT: u32 = 1;

/// `DRAW_UNIT_ID_SW`: the `preferred_draw_unit_id` this unit claims in
/// `evaluate()`.
pub const DRAW_UNIT_ID_SW: u8 = 1;

/// `lv_draw_sw_unit_t`: the software draw unit (single threaded).
pub struct SwDrawUnit {
    /// `task_act`: the task being rendered.
    pub task_act: Option<DrawTaskRef>,
    /// `sw_circle_cache` of `lv_global_t`.
    pub circle_cache: mask::CircleCache,
    /// `sw_shadow_cache` of `lv_global_t`.
    pub shadow_cache: box_shadow::ShadowCache,
    /// `idx`: index of this unit (`draw_unit_id`).
    pub idx: u32,
    /// `draw_sw_blend_handler_ll` of `lv_global_t`: custom blend handlers,
    /// newest first.
    pub blend_handlers: Vec<CustomBlendHandler>,
}

impl SwDrawUnit {
    /// A fresh unit with empty caches.
    pub fn new(idx: u32) -> Self {
        Self {
            task_act: None,
            circle_cache: mask::CircleCache::default(),
            shadow_cache: box_shadow::ShadowCache::default(),
            idx,
            blend_handlers: Vec::new(),
        }
    }

    /// `lv_draw_sw_register_blend_handler()`: replace the handler registered
    /// for `handler.dest_cf`, or insert it at the head. Always `true` (the
    /// C `false` is an allocation failure).
    pub fn register_blend_handler(&mut self, handler: CustomBlendHandler) -> bool {
        if let Some(existing) = self
            .blend_handlers
            .iter_mut()
            .find(|h| h.dest_cf == handler.dest_cf)
        {
            *existing = handler;
        } else {
            self.blend_handlers.insert(0, handler);
        }
        true
    }

    /// `lv_draw_sw_unregister_blend_handler()`: `true` if one was removed.
    pub fn unregister_blend_handler(&mut self, dest_cf: ColorFormat) -> bool {
        match self.blend_handlers.iter().position(|h| h.dest_cf == dest_cf) {
            Some(i) => {
                self.blend_handlers.remove(i);
                true
            }
            None => false,
        }
    }

    /// `lv_draw_sw_get_blend_handler()`
    pub fn get_blend_handler(&self, dest_cf: ColorFormat) -> Option<BlendHandler> {
        blend::get_blend_handler(&self.blend_handlers, dest_cf)
    }

    /// `execute_drawing()`: render one task. Clones the [`DrawTask`] out of
    /// the layer queue (it stays linked there) so that `ctx` and the
    /// descriptor can be handed to the `lv_draw_sw_*` entry points together.
    pub fn execute_drawing(&mut self, ctx: &mut DrawCtx, t: DrawTaskRef) {
        todo!()
    }
}

impl Default for SwDrawUnit {
    fn default() -> Self {
        Self::new(0)
    }
}

impl DrawUnit for SwDrawUnit {
    fn name(&self) -> &'static str {
        DRAW_UNIT_NAME
    }

    /// `evaluate()`
    fn evaluate(&mut self, task: &mut DrawTask) -> i32 {
        todo!()
    }

    /// `dispatch()`
    fn dispatch(&mut self, ctx: &mut DrawCtx, layer: LayerId) -> i32 {
        todo!()
    }

    /// `delete()`
    fn delete(&mut self) {
        todo!()
    }

    fn on_event(&mut self, code: EventCode, param: &mut EventParam<'_>) {
        let _ = (code, param);
    }
}

/// `lv_draw_sw_init()`: register the software draw unit(s)
/// (`Ui::draw_create_unit`).
pub fn init(ui: &mut Ui) {
    todo!()
}

/// `lv_draw_sw_deinit()`
pub fn deinit(ui: &mut Ui) {
    todo!()
}

/// Convenience: box a unit for `Ui::draw_create_unit`.
pub fn boxed_unit(idx: u32) -> Box<dyn DrawUnit> {
    Box::new(SwDrawUnit::new(idx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend::BlendDsc;
    use ferrovgl_core::draw::Layer;

    fn h1(_: &DrawTask, _: &mut Layer, _: &BlendDsc<'_>) {}
    fn h2(_: &DrawTask, _: &mut Layer, _: &BlendDsc<'_>) {}

    #[test]
    fn blend_handler_registry() {
        let mut u = SwDrawUnit::new(0);
        assert!(u.get_blend_handler(ColorFormat::Rgb565).is_none());
        assert!(u.register_blend_handler(CustomBlendHandler {
            dest_cf: ColorFormat::Rgb565,
            handler: h1,
        }));
        assert!(u.register_blend_handler(CustomBlendHandler {
            dest_cf: ColorFormat::Argb8888,
            handler: h1,
        }));
        // Newest first, like `lv_ll_ins_head`.
        assert_eq!(u.blend_handlers[0].dest_cf, ColorFormat::Argb8888);
        // Re-registering replaces in place.
        assert!(u.register_blend_handler(CustomBlendHandler {
            dest_cf: ColorFormat::Rgb565,
            handler: h2,
        }));
        assert_eq!(u.blend_handlers.len(), 2);
        assert_eq!(u.blend_handlers[1].dest_cf, ColorFormat::Rgb565);
        assert!(u.get_blend_handler(ColorFormat::Rgb565).is_some());
        assert!(u.unregister_blend_handler(ColorFormat::Rgb565));
        assert!(!u.unregister_blend_handler(ColorFormat::Rgb565));
        assert_eq!(u.blend_handlers.len(), 1);
    }
}
