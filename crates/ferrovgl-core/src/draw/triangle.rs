//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_triangle.h`, `src/draw/lv_draw_triangle.c`.

use alloc::boxed::Box;

use crate::core::Ui;
use crate::draw::grad::GradDsc;
use crate::draw::rect::{TaskSink, check_layer, set_task_dsc};
use crate::draw::{DrawCtx, DrawDsc, DrawDscBase, DrawTaskRef, DrawTaskType, LayerId};
use crate::misc::area::{Area, PointPrecise};
use crate::misc::color::{Color, OPA_COVER, OPA_MIN, Opa};

/// `lv_draw_triangle_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct TriangleDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `p`
    pub p: [PointPrecise; 3],
    /// `color`
    pub color: Color,
    /// `opa`
    pub opa: Opa,
    /// `grad`
    pub grad: GradDsc,
}

impl TriangleDsc {
    /// `lv_draw_triangle_dsc_init()`
    #[must_use]
    pub fn new() -> Self {
        let mut grad = GradDsc::new();
        grad.stops[0].color = Color::WHITE;
        grad.stops[1].color = Color::BLACK;
        grad.stops[1].frac = 255;
        grad.stops_count = 2;
        Self {
            base: DrawDscBase::new(),
            p: [PointPrecise::default(); 3],
            color: Color::WHITE,
            opa: OPA_COVER,
            grad,
        }
    }
}

impl Default for TriangleDsc {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawCtx {
    /// `lv_draw_triangle()`
    pub fn draw_triangle(&mut self, layer: LayerId, dsc: &TriangleDsc) {
        draw_triangle_impl(self, layer, dsc);
    }

    /// `lv_draw_task_get_triangle_dsc()`
    #[must_use]
    pub fn task_get_triangle_dsc(&mut self, task: DrawTaskRef) -> Option<&mut TriangleDsc> {
        let task = self.task_mut(task)?;
        if task.ty != DrawTaskType::Triangle {
            return None;
        }
        match &mut task.dsc {
            DrawDsc::Triangle(dsc) => Some(dsc),
            _ => None,
        }
    }
}

impl Ui {
    /// `lv_draw_triangle()` with object events, draw-unit evaluation and dispatch.
    pub fn draw_triangle(&mut self, layer: LayerId, dsc: &TriangleDsc) {
        draw_triangle_impl(self, layer, dsc);
    }
}

fn draw_triangle_impl<S: TaskSink + ?Sized>(sink: &mut S, layer: LayerId, dsc: &TriangleDsc) {
    if !check_layer(sink.ctx(), layer) || dsc.opa <= OPA_MIN {
        return;
    }
    let p = dsc.p.map(PointPrecise::to_point);
    let bounds = Area::new(
        p[0].x.min(p[1].x).min(p[2].x),
        p[0].y.min(p[1].y).min(p[2].y),
        p[0].x.max(p[1].x).max(p[2].x),
        p[0].y.max(p[1].y).max(p[2].y),
    );
    if dsc.base.drop_shadow_opa != 0
        && let Some(shadow) = sink.create_drop_shadow(layer, &dsc.base, &bounds)
    {
        let mut silhouette = dsc.clone();
        silhouette.base.drop_shadow_opa = 0;
        draw_triangle_impl(sink, shadow, &silhouette);
        sink.finish_drop_shadow(shadow, &dsc.base);
    }
    let task = sink.ctx().add_task(layer, &bounds, DrawTaskType::Triangle);
    set_task_dsc(sink.ctx(), task, DrawDsc::Triangle(Box::new(dsc.clone())));
    sink.finalize(task);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::misc::color::ColorFormat;
    use crate::misc::types::ValuePrecise;

    #[test]
    fn triangle_defaults_preserve_zeroed_gradient_opacity() {
        let dsc = TriangleDsc::new();
        assert_eq!(dsc.color, Color::WHITE);
        assert_eq!(dsc.opa, 255);
        assert_eq!(dsc.grad.stops_count, 2);
        assert_eq!(dsc.grad.stops[0].color, Color::WHITE);
        assert_eq!(dsc.grad.stops[1].color, Color::BLACK);
        assert_eq!(dsc.grad.stops[1].frac, 255);
        assert_eq!(dsc.grad.stops[0].opa, 0);
        assert_eq!(dsc.grad.stops[1].opa, 0);
    }

    #[test]
    fn triangle_tasks_copy_descriptor_bind_layer_and_truncate_bounds() {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Xrgb8888, &Area::new(-30, -30, 40, 40));
        let mut dsc = TriangleDsc::new();
        dsc.p = [
            PointPrecise::new(-2.8 as ValuePrecise, 1.9 as ValuePrecise),
            PointPrecise::new(30.5 as ValuePrecise, 6.7 as ValuePrecise),
            PointPrecise::new(9.5 as ValuePrecise, -10.4 as ValuePrecise),
        ];
        dsc.opa = OPA_MIN;
        ctx.draw_triangle(layer, &dsc);
        assert!(ctx.layer(layer).unwrap().tasks.is_empty());
        dsc.opa = 190;
        ctx.draw_triangle(layer, &dsc);
        let task = DrawTaskRef { layer, index: 0 };
        assert_eq!(ctx.task(task).unwrap().area, Area::new(-2, -10, 30, 6));
        assert_eq!(ctx.task_get_triangle_dsc(task).unwrap().base.layer, Some(layer));
        dsc.opa = 1;
        assert_ne!(dsc.opa, ctx.task_get_triangle_dsc(task).unwrap().opa);
        assert_eq!(ctx.task_get_triangle_dsc(task).unwrap().opa, 190);
        let other = ctx.add_task(layer, &Area::new(0, 0, 1, 1), DrawTaskType::Fill);
        assert!(ctx.task_get_triangle_dsc(other).is_none());
    }
}
