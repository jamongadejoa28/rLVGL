//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_blur.h`, `src/draw/lv_draw_blur.c`.

use alloc::boxed::Box;

use crate::core::Ui;
use crate::draw::rect::{TaskSink, set_task_dsc};
use crate::draw::{DrawCtx, DrawDsc, DrawDscBase, DrawTaskRef, DrawTaskType, LayerId};
use crate::misc::area::Area;
use crate::misc::style::BlurQuality;

/// `lv_draw_blur_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct BlurDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `blur_radius`
    pub blur_radius: i32,
    /// `corner_radius`
    pub corner_radius: i32,
    /// `quality`
    pub quality: BlurQuality,
}

impl BlurDsc {
    /// `lv_draw_blur_dsc_init()`
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: DrawDscBase::new(),
            blur_radius: 0,
            corner_radius: 0,
            quality: BlurQuality::Auto,
        }
    }
}

impl Default for BlurDsc {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawCtx {
    /// `lv_draw_blur()`
    pub fn draw_blur(&mut self, layer: LayerId, dsc: &BlurDsc, coords: &Area) {
        draw_blur_impl(self, layer, dsc, coords);
    }

    /// `lv_draw_task_get_blur_dsc()`
    #[must_use]
    pub fn task_get_blur_dsc(&mut self, task: DrawTaskRef) -> Option<&mut BlurDsc> {
        let task = self.task_mut(task)?;
        if task.ty != DrawTaskType::Blur {
            return None;
        }
        match &mut task.dsc {
            DrawDsc::Blur(dsc) => Some(dsc),
            _ => None,
        }
    }
}

impl Ui {
    /// `lv_draw_blur()` with object events, draw-unit evaluation and dispatch.
    /// The software draw unit implements the four-direction IIR rasterizer.
    pub fn draw_blur(&mut self, layer: LayerId, dsc: &BlurDsc, coords: &Area) {
        draw_blur_impl(self, layer, dsc, coords);
    }
}

fn draw_blur_impl<S: TaskSink + ?Sized>(sink: &mut S, layer: LayerId, dsc: &BlurDsc, coords: &Area) {
    if dsc.blur_radius <= 0 || sink.ctx().layer(layer).is_none() {
        return;
    }
    let task = sink.ctx().add_task(layer, coords, DrawTaskType::Blur);
    set_task_dsc(sink.ctx(), task, DrawDsc::Blur(Box::new(dsc.clone())));
    sink.finalize(task);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::misc::color::ColorFormat;

    #[test]
    fn disabled_blur_has_no_task_and_positive_radius_copies_descriptor() {
        let mut ctx = DrawCtx::new();
        let area = Area::new(0, 0, 15, 15);
        let layer = ctx.layer_create(None, ColorFormat::Argb8888, &area);
        let mut dsc = BlurDsc::new();
        assert_eq!(dsc.base, DrawDscBase::new());
        assert_eq!(dsc.quality, BlurQuality::Auto);
        assert_eq!(dsc.corner_radius, 0);
        for radius in [-1, 0] {
            dsc.blur_radius = radius;
            ctx.draw_blur(layer, &dsc, &area);
            assert!(ctx.layer(layer).unwrap().tasks.is_empty());
        }
        dsc.blur_radius = 4;
        dsc.corner_radius = 3;
        ctx.draw_blur(layer, &dsc, &area);
        let task = DrawTaskRef { layer, index: 0 };
        let stored = ctx.task_get_blur_dsc(task).unwrap();
        assert_eq!(stored.blur_radius, 4);
        assert_eq!(stored.corner_radius, 3);
        assert_eq!(stored.base.layer, Some(layer));
        let fill = ctx.add_task(layer, &area, DrawTaskType::Fill);
        assert!(ctx.task_get_blur_dsc(fill).is_none());
    }
}
