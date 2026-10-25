//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_line.h`, `src/draw/lv_draw_line.c`.

use alloc::boxed::Box;
use alloc::rc::Rc;

use crate::core::Ui;
use crate::draw::rect::{TaskSink, set_task_dsc};

use crate::draw::{DrawCtx, DrawDsc, DrawDscBase, DrawTaskRef, DrawTaskType, LayerId};
use crate::misc::area::{Area, PointPrecise};
use crate::misc::color::{Color, OPA_COVER, OPA_MIN, Opa};
use crate::misc::types::{ValuePrecise, coord};

/// `LV_DRAW_LINE_POINT_NONE`
#[cfg(feature = "float")]
pub const DRAW_LINE_POINT_NONE: ValuePrecise = f32::MAX;
/// `LV_DRAW_LINE_POINT_NONE`
#[cfg(not(feature = "float"))]
pub const DRAW_LINE_POINT_NONE: ValuePrecise = i32::MAX;

/// `lv_draw_line_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct LineDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `p1`
    pub p1: PointPrecise,
    /// `p2`
    pub p2: PointPrecise,
    /// `points` / `point_cnt`: a polyline (used by `lv_draw_line_iterate`).
    pub points: Option<Rc<[PointPrecise]>>,
    /// `color`
    pub color: Color,
    /// `width`
    pub width: i32,
    /// `dash_width`
    pub dash_width: i32,
    /// `dash_gap`
    pub dash_gap: i32,
    /// `opa`
    pub opa: Opa,
    /// `round_start`
    pub round_start: bool,
    /// `round_end`
    pub round_end: bool,
    /// `raw_end`: do not bother with perpendicular line ending if it's not visible.
    pub raw_end: bool,
}

impl LineDsc {
    /// `lv_draw_line_dsc_init()`
    pub fn new() -> Self {
        Self {
            base: DrawDscBase::new(),
            p1: PointPrecise::default(),
            p2: PointPrecise::default(),
            points: None,
            color: Color::BLACK,
            width: 1,
            dash_width: 0,
            dash_gap: 0,
            opa: OPA_COVER,
            round_start: false,
            round_end: false,
            raw_end: false,
        }
    }
}

impl Default for LineDsc {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawCtx {
    /// `lv_draw_line()`
    pub fn draw_line(&mut self, layer: LayerId, dsc: &LineDsc) {
        draw_line_impl(self, layer, dsc);
    }

    /// `lv_draw_task_get_line_dsc()`
    pub fn task_get_line_dsc(&mut self, task: DrawTaskRef) -> Option<&mut LineDsc> {
        let task = self.task_mut(task)?;
        if task.ty != DrawTaskType::Line {
            return None;
        }
        match &mut task.dsc {
            DrawDsc::Line(dsc) => Some(dsc),
            _ => None,
        }
    }

    /// `lv_draw_line_iterate()`: call `draw_line_cb` for every segment of `dsc.points`.
    pub fn draw_line_iterate(
        &mut self,
        t: DrawTaskRef,
        dsc: &mut LineDsc,
        draw_line_cb: &mut dyn FnMut(&mut DrawCtx, DrawTaskRef, &LineDsc),
    ) {
        if self.task(t).is_none() {
            return;
        }
        if let Some(points) = dsc.points.as_ref() {
            let mut segment = dsc.clone();
            segment.points = None;
            for pair in points.windows(2) {
                if point_is_none(pair[0]) || point_is_none(pair[1]) {
                    continue;
                }
                segment.p1 = pair[0];
                segment.p2 = pair[1];
                draw_line_cb(self, t, &segment);
            }
        } else {
            draw_line_cb(self, t, dsc);
        }
    }
}

impl Ui {
    /// `lv_draw_line()` with object events, draw-unit evaluation and dispatch.
    pub fn draw_line(&mut self, layer: LayerId, dsc: &LineDsc) {
        draw_line_impl(self, layer, dsc);
    }
}

fn point_is_none(point: PointPrecise) -> bool {
    point.x == DRAW_LINE_POINT_NONE || point.y == DRAW_LINE_POINT_NONE
}

#[allow(clippy::unnecessary_cast)] // ValuePrecise is f32 or i32.
fn draw_line_impl<S: TaskSink + ?Sized>(sink: &mut S, layer: LayerId, dsc: &LineDsc) {
    if dsc.width == 0 || dsc.opa <= OPA_MIN || sink.ctx().layer(layer).is_none() {
        return;
    }
    let (area, points) = if let Some(points) = dsc.points.as_ref() {
        if points.len() <= 1 {
            return;
        }
        let mut area = Area::new(coord::MAX, coord::MAX, coord::MIN, coord::MIN);
        for &point in points.iter() {
            if point_is_none(point) {
                continue;
            }
            // Preserve the upstream repeated expansion inside the point loop,
            // followed by one additional expansion after all points.
            area.x1 = (area.x1 as ValuePrecise).min(point.x) as i32 - dsc.width;
            area.x2 = (area.x2 as ValuePrecise).max(point.x) as i32 + dsc.width;
            area.y1 = (area.y1 as ValuePrecise).min(point.y) as i32 - dsc.width;
            area.y2 = (area.y2 as ValuePrecise).max(point.y) as i32 + dsc.width;
        }
        if area.x1 == coord::MAX {
            return;
        }
        area.increase(dsc.width, dsc.width);
        (area, Some(Rc::from(points.as_ref())))
    } else {
        (
            Area::new(
                dsc.p1.x.min(dsc.p2.x) as i32 - dsc.width,
                dsc.p1.y.min(dsc.p2.y) as i32 - dsc.width,
                dsc.p1.x.max(dsc.p2.x) as i32 + dsc.width,
                dsc.p1.y.max(dsc.p2.y) as i32 + dsc.width,
            ),
            None,
        )
    };
    if dsc.base.drop_shadow_opa != 0
        && let Some(shadow) = sink.create_drop_shadow(layer, &dsc.base, &area)
    {
        let mut shadow_dsc = dsc.clone();
        shadow_dsc.base.drop_shadow_opa = 0;
        draw_line_impl(sink, shadow, &shadow_dsc);
        sink.finish_drop_shadow(shadow, &dsc.base);
    }
    let task = sink.ctx().add_task(layer, &area, DrawTaskType::Line);
    let mut copied = dsc.clone();
    copied.points = points;
    set_task_dsc(sink.ctx(), task, DrawDsc::Line(Box::new(copied)));
    sink.finalize(task);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::misc::color::ColorFormat;
    use alloc::vec::Vec;

    fn point(x: i32, y: i32) -> PointPrecise {
        PointPrecise::new(x as ValuePrecise, y as ValuePrecise)
    }

    #[test]
    fn descriptor_defaults_and_single_segment_area() {
        let dsc = LineDsc::new();
        assert_eq!(dsc.width, 1);
        assert_eq!(dsc.opa, 255);
        assert_eq!(dsc.color, Color::BLACK);
        assert!(!dsc.round_start && !dsc.round_end && !dsc.raw_end);
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Argb8888, &Area::new(0, 0, 100, 100));
        let dsc = LineDsc {
            p1: point(30, 10),
            p2: point(10, 40),
            width: 4,
            ..dsc
        };
        ctx.draw_line(layer, &dsc);
        let task = DrawTaskRef { layer, index: 0 };
        assert_eq!(ctx.task(task).unwrap().area, Area::new(6, 6, 34, 44));
        assert_eq!(ctx.task_get_line_dsc(task).unwrap().base.layer, Some(layer));
        let wrong = ctx.add_task(layer, &Area::new(0, 0, 1, 1), DrawTaskType::Fill);
        assert!(ctx.task_get_line_dsc(wrong).is_none());
    }

    #[test]
    fn polyline_copies_points_and_preserves_upstream_repeated_bound_expansion() {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Argb8888, &Area::new(0, 0, 100, 100));
        let points: Rc<[PointPrecise]> = Rc::from([point(10, 20), point(30, 40), point(5, 15)]);
        let dsc = LineDsc {
            width: 2,
            points: Some(points.clone()),
            ..LineDsc::new()
        };
        ctx.draw_line(layer, &dsc);
        let task = DrawTaskRef { layer, index: 0 };
        assert_eq!(ctx.task(task).unwrap().area, Area::new(1, 11, 36, 46));
        let copied = ctx.task_get_line_dsc(task).unwrap().points.as_ref().unwrap();
        assert_eq!(copied.as_ref(), points.as_ref());
        assert!(!Rc::ptr_eq(copied, &points));
    }

    #[test]
    fn disabled_and_invalid_polylines_do_not_create_tasks() {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Argb8888, &Area::new(0, 0, 100, 100));
        let missing = PointPrecise::new(DRAW_LINE_POINT_NONE, DRAW_LINE_POINT_NONE);
        for dsc in [
            LineDsc {
                width: 0,
                ..LineDsc::new()
            },
            LineDsc {
                opa: OPA_MIN,
                ..LineDsc::new()
            },
            LineDsc {
                points: Some(Rc::from([point(10, 20)])),
                ..LineDsc::new()
            },
            LineDsc {
                points: Some(Rc::from([missing, missing])),
                ..LineDsc::new()
            },
        ] {
            ctx.draw_line(layer, &dsc);
        }
        assert!(ctx.layer(layer).unwrap().tasks.is_empty());
    }

    #[test]
    fn iteration_breaks_at_none_without_joining_unrelated_subpaths() {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Argb8888, &Area::new(0, 0, 100, 100));
        let missing = PointPrecise::new(DRAW_LINE_POINT_NONE, 0 as ValuePrecise);
        let mut dsc = LineDsc {
            points: Some(Rc::from([
                point(1, 2),
                point(3, 4),
                missing,
                point(5, 6),
                point(7, 8),
            ])),
            round_start: true,
            ..LineDsc::new()
        };
        ctx.draw_line(layer, &dsc);
        let mut segments = Vec::new();
        ctx.draw_line_iterate(DrawTaskRef { layer, index: 0 }, &mut dsc, &mut |_, _, segment| {
            assert!(segment.points.is_none());
            assert!(segment.round_start);
            segments.push((segment.p1, segment.p2));
        });
        assert_eq!(segments, [(point(1, 2), point(3, 4)), (point(5, 6), point(7, 8))]);
        assert_eq!(dsc.points.as_ref().unwrap().len(), 5);
    }
}
