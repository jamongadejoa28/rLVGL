//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw.c`, `src/draw/sw/lv_draw_sw_private.h`.
//!
//! Software renderer for ferrovgl. Each colour format is behind a `cf-*`
//! feature so that unused blend paths are not compiled on small targets.
//!
//! The parity configuration has `LV_USE_OS == LV_OS_NONE`, so only the
//! single threaded unit is ported: `dispatch()` renders the taken task
//! synchronously, there is no `render_thread_cb()` / `wait_for_finish()`
//! and `delete_cb` is `NULL`. `LV_USE_PARALLEL_DRAW_DEBUG`,
//! `LV_USE_DRAW_SW_ASM` (the unit is always called `"SW"`),
//! `LV_USE_3DTEXTURE` and the ThorVG vector path (no
//! `LV_DRAW_TASK_TYPE_VECTOR` task type exists here) are left out.
//!
//! The global state of the software renderer (`sw_circle_cache`,
//! `sw_shadow_cache` and `draw_sw_blend_handler_ll` of `lv_global_t`) is
//! owned by [`SwDrawUnit`].
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
use ferrovgl_core::draw::{
    DRAW_UNIT_IDLE, DrawCtx, DrawDsc, DrawTask, DrawTaskRef, DrawTaskState, DrawTaskType, DrawUnit, LayerId,
};
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

/// `LV_COLOR_FORMAT_PROPRIETARY_START`: the first colour format value
/// reserved for vendor (GPU) formats, which the software renderer can't read.
const COLOR_FORMAT_PROPRIETARY_START: u8 = 0x30;

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
    ///
    /// A task whose descriptor does not match its type (impossible upstream,
    /// where `draw_dsc` is allocated for the type) is skipped like the
    /// `default:` case.
    pub fn execute_drawing(&mut self, ctx: &mut DrawCtx, t: DrawTaskRef) {
        let Some(task) = ctx.task(t).cloned() else {
            return;
        };

        /*Render the draw task*/
        match (task.ty, &task.dsc) {
            (DrawTaskType::Fill, DrawDsc::Fill(dsc)) => fill::fill(self, ctx, &task, dsc, &task.area),
            (DrawTaskType::Border, DrawDsc::Border(dsc)) => border::border(self, ctx, &task, dsc, &task.area),
            (DrawTaskType::BoxShadow, DrawDsc::BoxShadow(dsc)) => {
                box_shadow::box_shadow(self, ctx, &task, dsc, &task.area)
            }
            (DrawTaskType::Letter, DrawDsc::Letter(dsc)) => letter::letter(self, ctx, &task, dsc, &task.area),
            (DrawTaskType::Label, DrawDsc::Label(dsc)) => label::label(self, ctx, &task, dsc, &task.area),
            (DrawTaskType::Image, DrawDsc::Image(dsc)) => img::image(self, ctx, &task, dsc, &task.area),
            (DrawTaskType::Arc, DrawDsc::Arc(dsc)) => arc::arc(self, ctx, &task, dsc, &task.area),
            (DrawTaskType::Line, DrawDsc::Line(dsc)) => {
                /*lv_draw_line_iterate(t, t->draw_dsc, lv_draw_sw_line)*/
                let mut line_dsc = (**dsc).clone();
                ctx.draw_line_iterate(t, &mut line_dsc, &mut |ctx, _t, d| {
                    line::line(self, ctx, &task, d);
                });
            }
            (DrawTaskType::Blur, DrawDsc::Blur(dsc)) => blur::blur(self, ctx, &task, dsc, &task.area),
            (DrawTaskType::Triangle, DrawDsc::Triangle(dsc)) => triangle::triangle(self, ctx, &task, dsc),
            (DrawTaskType::Layer, DrawDsc::Layer(dsc)) => layer::layer(self, ctx, &task, dsc, &task.area),
            (DrawTaskType::MaskRectangle, DrawDsc::MaskRect(dsc)) => {
                mask_rect::mask_rect(self, ctx, &task, dsc)
            }
            _ => {}
        }
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

    /// `evaluate()`: claim every task with the base preference score (100),
    /// except images / layers that need skew, a bitmap mask on an A8 /
    /// RGB565A8 image, or a proprietary colour format.
    fn evaluate(&mut self, task: &mut DrawTask) -> i32 {
        if let DrawTaskType::Image | DrawTaskType::Layer = task.ty {
            let draw_dsc = match &task.dsc {
                DrawDsc::Image(d) | DrawDsc::Layer(d) => d,
                /*`draw_dsc` is always an image descriptor for these types upstream*/
                _ => return 0,
            };

            /* not support skew */
            if draw_dsc.skew_x != 0 || draw_dsc.skew_y != 0 {
                return 0;
            }

            let masked = !draw_dsc.bitmap_mask_src.is_none();

            let cf = draw_dsc.header.cf;
            if masked && (cf == ColorFormat::A8 || cf == ColorFormat::Rgb565A8) {
                return 0;
            }

            if cf as u8 >= COLOR_FORMAT_PROPRIETARY_START {
                return 0;
            }
        }

        if task.preference_score >= 100 {
            task.preference_score = 100;
            task.preferred_draw_unit_id = DRAW_UNIT_ID_SW;
        }

        0
    }

    /// `dispatch()` (`LV_USE_OS == 0`): take the first available task of
    /// `layer`, allocate the layer buffer if needed and render the task
    /// synchronously.
    fn dispatch(&mut self, ctx: &mut DrawCtx, layer: LayerId) -> i32 {
        /*Return immediately if it's busy with draw task*/
        if self.task_act.is_some() {
            return 0;
        }

        let Some(t) = ctx.get_available_task(layer, None, DRAW_UNIT_ID_SW) else {
            return DRAW_UNIT_IDLE; /*Couldn't start rendering*/
        };

        if !ctx.layer_alloc_buf(layer) {
            if let Some(task) = ctx.task_mut(t) {
                task.state = DrawTaskState::Failed;
            }
            return DRAW_UNIT_IDLE; /*Couldn't start rendering*/
        }

        if let Some(task) = ctx.task_mut(t) {
            task.state = DrawTaskState::InProgress;
        }
        self.task_act = Some(t);

        self.execute_drawing(ctx, t);
        if let Some(task_act) = self.task_act
            && let Some(task) = ctx.task_mut(task_act)
        {
            task.state = DrawTaskState::Finished;
        }
        self.task_act = None;

        /*The draw unit is free now. Request a new dispatching as it can get a new task*/
        ctx.dispatch_request();

        1
    }

    /// `lv_draw_sw_delete()`: without OS support there are no render
    /// threads to stop (upstream even leaves `delete_cb` `NULL`).
    fn delete(&mut self) {}

    fn on_event(&mut self, code: EventCode, param: &mut EventParam<'_>) {
        let _ = (code, param);
    }
}

/// `lv_draw_sw_init()`: register the software draw unit
/// (`Ui::draw_create_unit`).
///
/// `lv_draw_sw_mask_init()` only creates the circle cache mutex upstream;
/// the cache itself starts empty in [`SwDrawUnit::new`], as does the custom
/// blend handler list (`lv_ll_init(&draw_sw_blend_handler_ll)`). The unit's
/// `idx` is the unit count after the registration: `lv_draw_create_unit()`
/// increments `unit_cnt` before assigning it, so the first unit gets 1.
pub fn init(ui: &mut Ui) {
    let draw_sw_unit = SwDrawUnit::new(ui.draw_unit_count() + 1);
    ui.draw_create_unit(Box::new(draw_sw_unit));
}

/// `lv_draw_sw_deinit()`: `lv_draw_sw_mask_deinit()` only deletes the
/// circle cache mutex and ThorVG is not used, so nothing is left to do. The
/// unit (with its caches) is dropped with `Ui::draw_units`, like
/// `lv_draw_deinit()` frees the units upstream.
pub fn deinit(ui: &mut Ui) {
    let _ = ui;
}

/// Convenience: box a unit for `Ui::draw_create_unit`.
pub fn boxed_unit(idx: u32) -> Box<dyn DrawUnit> {
    Box::new(SwDrawUnit::new(idx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend::BlendDsc;
    use ferrovgl_core::draw::image::DrawImageDsc;
    use ferrovgl_core::draw::{DRAW_UNIT_NONE, DrawDscBase, ImageHeader, Layer};
    use ferrovgl_core::misc::area::{Area, Point};
    use ferrovgl_core::misc::color::{Color, OPA_COVER};
    use ferrovgl_core::misc::style::{BlendMode, ImageSrc, SharedRef};

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

    /// An `lv_draw_image_dsc_t` as `lv_draw_image_dsc_init()` leaves it
    /// (built by hand: the constructor lives in another module).
    fn image_dsc(src: ImageSrc) -> DrawImageDsc {
        DrawImageDsc {
            base: DrawDscBase::new(),
            src,
            header: ImageHeader::default(),
            clip_radius: 0,
            rotation: 0,
            scale_x: 256,
            scale_y: 256,
            skew_x: 0,
            skew_y: 0,
            pivot: Point::new(0, 0),
            recolor: Color::BLACK,
            recolor_opa: 0,
            opa: OPA_COVER,
            blend_mode: BlendMode::Normal,
            antialias: false,
            tile: false,
            colorkey: None,
            sup: None,
            image_area: Area::default(),
            bitmap_mask_src: ImageSrc::None,
        }
    }

    /// A task as `lv_draw_finalize_task_creation()` hands it to `evaluate`.
    fn task(ty: DrawTaskType, dsc: DrawDsc) -> DrawTask {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Argb8888, &Area::new(0, 0, 9, 9));
        let t = ctx.add_task(layer, &Area::new(0, 0, 9, 9), ty);
        let mut task = ctx.task(t).unwrap().clone();
        task.dsc = dsc;
        task.preference_score = 100;
        task.preferred_draw_unit_id = 0;
        task
    }

    #[test]
    fn evaluate_claims_tasks_nobody_prefers() {
        let mut u = SwDrawUnit::new(0);
        let mut t = task(DrawTaskType::Fill, DrawDsc::None);
        assert_eq!(u.evaluate(&mut t), 0);
        assert_eq!(t.preference_score, 100);
        assert_eq!(t.preferred_draw_unit_id, DRAW_UNIT_ID_SW);

        // A unit that already offered a better score keeps the task.
        let mut t = task(DrawTaskType::Border, DrawDsc::None);
        t.preference_score = 80;
        t.preferred_draw_unit_id = 7;
        u.evaluate(&mut t);
        assert_eq!((t.preference_score, t.preferred_draw_unit_id), (80, 7));

        // Not special cased: bitmap masks are claimed too.
        let mut t = task(DrawTaskType::MaskBitmap, DrawDsc::MaskBitmap(Box::default()));
        u.evaluate(&mut t);
        assert_eq!(t.preferred_draw_unit_id, DRAW_UNIT_ID_SW);
    }

    #[test]
    fn evaluate_rejects_unsupported_images() {
        let mut u = SwDrawUnit::new(0);
        let claimed = |u: &mut SwDrawUnit, ty: DrawTaskType, d: DrawImageDsc| {
            let dsc = if ty == DrawTaskType::Layer {
                DrawDsc::Layer(Box::new(d))
            } else {
                DrawDsc::Image(Box::new(d))
            };
            let mut t = task(ty, dsc);
            u.evaluate(&mut t);
            t.preferred_draw_unit_id == DRAW_UNIT_ID_SW
        };

        let mut d = image_dsc(ImageSrc::None);
        d.header.cf = ColorFormat::Argb8888;
        assert!(claimed(&mut u, DrawTaskType::Image, d.clone()));
        assert!(claimed(&mut u, DrawTaskType::Layer, d.clone()));

        let mut skew = d.clone();
        skew.skew_y = 10;
        assert!(!claimed(&mut u, DrawTaskType::Image, skew.clone()));
        assert!(!claimed(&mut u, DrawTaskType::Layer, skew));

        // A bitmap mask can't be applied to A8 / RGB565A8 images...
        let mask = ImageSrc::Symbol(SharedRef::Static("mask"));
        for cf in [ColorFormat::A8, ColorFormat::Rgb565A8] {
            let mut m = d.clone();
            m.header.cf = cf;
            assert!(claimed(&mut u, DrawTaskType::Image, m.clone()));
            m.bitmap_mask_src = mask.clone();
            assert!(!claimed(&mut u, DrawTaskType::Image, m));
        }
        // ...but to the others.
        let mut m = d.clone();
        m.bitmap_mask_src = mask;
        assert!(claimed(&mut u, DrawTaskType::Image, m));

        // Vendor formats start at LV_COLOR_FORMAT_PROPRIETARY_START (0x30).
        let mut p = d.clone();
        p.header.cf = ColorFormat::Uyvy;
        assert!(claimed(&mut u, DrawTaskType::Image, p.clone()));
        p.header.cf = ColorFormat::NemaTsc4;
        assert!(!claimed(&mut u, DrawTaskType::Image, p));
    }

    fn ctx_with_layer() -> (DrawCtx, LayerId) {
        let mut ctx = DrawCtx::new();
        ctx.unit_cnt = 1;
        let layer = ctx.layer_create(None, ColorFormat::Argb8888, &Area::new(0, 0, 19, 9));
        (ctx, layer)
    }

    /// `DrawDsc::None` / `MaskBitmap` tasks hit the `default:` case of
    /// `execute_drawing()`, so they can be dispatched without any renderer.
    fn add_noop_task(ctx: &mut DrawCtx, layer: LayerId, area: Area) -> DrawTaskRef {
        let t = ctx.add_task(layer, &area, DrawTaskType::MaskBitmap);
        let task = ctx.task_mut(t).unwrap();
        task.dsc = DrawDsc::MaskBitmap(Box::default());
        task.preferred_draw_unit_id = DRAW_UNIT_ID_SW;
        t
    }

    #[test]
    fn dispatch_renders_one_task_synchronously() {
        let (mut ctx, layer) = ctx_with_layer();
        let mut u = SwDrawUnit::new(0);

        // Nothing to do.
        assert_eq!(u.dispatch(&mut ctx, layer), DRAW_UNIT_IDLE);
        assert!(ctx.layer(layer).unwrap().draw_buf.is_none());

        let t0 = add_noop_task(&mut ctx, layer, Area::new(0, 0, 4, 4));
        let t1 = add_noop_task(&mut ctx, layer, Area::new(10, 0, 14, 4));
        ctx.dispatch_req = false;

        assert_eq!(u.dispatch(&mut ctx, layer), 1);
        assert_eq!(ctx.task(t0).unwrap().state, DrawTaskState::Finished);
        assert_eq!(ctx.task(t1).unwrap().state, DrawTaskState::Waiting);
        assert!(u.task_act.is_none());
        assert!(ctx.dispatch_req, "a new dispatch is requested");
        // The layer buffer was allocated lazily (and cleared: it has alpha).
        let buf = ctx.layer(layer).unwrap().draw_buf.as_ref().unwrap();
        assert_eq!((buf.header.w, buf.header.h), (20, 10));
        assert!(buf.data().iter().all(|&b| b == 0));

        // With a single unit the tasks are consumed strictly in order: the
        // finished first task blocks the queue until it is removed.
        assert_eq!(u.dispatch(&mut ctx, layer), DRAW_UNIT_IDLE);
        ctx.layer_mut(layer).unwrap().tasks.pop_front();
        let t1 = DrawTaskRef { layer, index: 0 };
        assert_eq!(u.dispatch(&mut ctx, layer), 1);
        assert_eq!(ctx.task(t1).unwrap().state, DrawTaskState::Finished);
    }

    #[test]
    fn dispatch_returns_zero_while_busy() {
        let (mut ctx, layer) = ctx_with_layer();
        let t = add_noop_task(&mut ctx, layer, Area::new(0, 0, 4, 4));
        let mut u = SwDrawUnit::new(0);
        u.task_act = Some(t);
        assert_eq!(u.dispatch(&mut ctx, layer), DRAW_UNIT_NONE);
        assert_eq!(ctx.task(t).unwrap().state, DrawTaskState::Waiting);
    }

    #[test]
    fn layer_task_of_an_empty_layer_draws_nothing() {
        // `lv_draw_sw_layer()` returns early when nothing was drawn on the
        // child layer (its buffer was never allocated).
        let (mut ctx, layer) = ctx_with_layer();
        let child = ctx.layer_create(Some(layer), ColorFormat::Argb8888, &Area::new(0, 0, 4, 4));
        let t = ctx.add_task(layer, &Area::new(0, 0, 4, 4), DrawTaskType::Layer);
        ctx.task_mut(t).unwrap().dsc = DrawDsc::Layer(Box::new(image_dsc(ImageSrc::Layer(child))));

        let mut u = SwDrawUnit::new(0);
        assert_eq!(u.dispatch(&mut ctx, layer), 1);
        assert_eq!(ctx.task(t).unwrap().state, DrawTaskState::Finished);
        let buf = ctx.layer(layer).unwrap().draw_buf.as_ref().unwrap();
        assert!(buf.data().iter().all(|&b| b == 0));
    }

    #[test]
    fn mismatched_descriptor_is_skipped() {
        let (mut ctx, layer) = ctx_with_layer();
        let t = ctx.add_task(layer, &Area::new(0, 0, 4, 4), DrawTaskType::Fill);
        let mut u = SwDrawUnit::new(0);
        u.execute_drawing(&mut ctx, t);
        assert_eq!(ctx.task(t).unwrap().state, DrawTaskState::Waiting);
        // A stale handle is ignored too.
        u.execute_drawing(&mut ctx, DrawTaskRef { layer, index: 5 });
    }

    #[test]
    fn unit_identity() {
        let u = boxed_unit(3);
        assert_eq!(u.name(), "SW");
        assert_eq!(SwDrawUnit::default().idx, 0);
        assert_eq!(SwDrawUnit::new(3).idx, 3);
    }
}
