//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_arc.h`, `src/draw/lv_draw_arc.c`.

use alloc::boxed::Box;

use crate::core::Ui;
use crate::draw::rect::{TaskSink, check_layer, set_task_dsc};
use crate::draw::{DrawCtx, DrawDsc, DrawDscBase, DrawTaskRef, DrawTaskType, LayerId};
use crate::misc::area::{Area, Point};
use crate::misc::color::{Color, OPA_COVER, OPA_MIN, Opa};
use crate::misc::math::{TRIGO_SHIFT, trigo_sin};
use crate::misc::style::ImageSrc;
use crate::misc::types::ValuePrecise;

/// `lv_draw_arc_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct ArcDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `color`
    pub color: Color,
    /// `width`
    pub width: i32,
    /// `start_angle`
    pub start_angle: ValuePrecise,
    /// `end_angle`
    pub end_angle: ValuePrecise,
    /// `center`
    pub center: Point,
    /// `img_src`
    pub img_src: ImageSrc,
    /// `radius`
    pub radius: u16,
    /// `opa`
    pub opa: Opa,
    /// `rounded`
    pub rounded: bool,
}

impl ArcDsc {
    /// `lv_draw_arc_dsc_init()`
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: DrawDscBase::new(),
            color: Color::BLACK,
            width: 1,
            start_angle: ValuePrecise::default(),
            end_angle: ValuePrecise::default(),
            center: Point::default(),
            img_src: ImageSrc::None,
            radius: 0,
            opa: OPA_COVER,
            rounded: false,
        }
    }
}

impl Default for ArcDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_arc_get_area()`: bounding area of an arc.
#[allow(clippy::unnecessary_cast)] // ValuePrecise is f32 or i32.
pub fn get_area(
    x: i32,
    y: i32,
    radius: u16,
    start_angle: ValuePrecise,
    end_angle: ValuePrecise,
    w: i32,
    rounded: bool,
    area: &mut Area,
) {
    let rout = i32::from(radius);
    let mut start = start_angle as i32;
    let mut end = end_angle as i32;
    let outer = Area::new(x - rout, y - rout, x + rout, y + rout);
    // Preserve the full-circle and excessive-stroke invalidation guards.
    if rout <= w || end == start.wrapping_add(360) {
        *area = outer;
        return;
    }
    if start > 360 {
        start -= 360;
    }
    if end > 360 {
        end -= 360;
    }
    let rin = rout - w;
    let extra = if rounded { w / 2 + 1 } else { 0 };
    let mut start_quarter = (start / 90) as u8;
    let mut end_quarter = (end / 90) as u8;
    if start_quarter == 4 {
        start_quarter = 3;
    }
    if end_quarter == 4 {
        end_quarter = 3;
    }
    let sin = |angle: i32| trigo_sin(angle as i16);
    let scaled = |angle: i32, radius: i32| sin(angle).wrapping_mul(radius) >> TRIGO_SHIFT;
    if start_quarter == end_quarter && start <= end {
        match start_quarter {
            0 => {
                area.y1 = y + scaled(start, rin) - extra;
                area.x2 = x + scaled(start + 90, rout) + extra;
                area.y2 = y + scaled(end, rout) + extra;
                area.x1 = x + scaled(end + 90, rin) - extra;
            }
            1 => {
                area.y2 = y + scaled(start, rout) + extra;
                area.x2 = x + scaled(start + 90, rin) + extra;
                area.y1 = y + scaled(end, rin) - extra;
                area.x1 = x + scaled(end + 90, rout) - extra;
            }
            2 => {
                area.x1 = x + scaled(start + 90, rout) - extra;
                area.y2 = y + scaled(start, rin) + extra;
                area.y1 = y + scaled(end, rout) - extra;
                area.x2 = x + scaled(end + 90, rin) + extra;
            }
            3 => {
                area.x1 = x + scaled(start + 90, rin) - extra;
                area.y1 = y + scaled(start, rout) - extra;
                area.x2 = x + scaled(end + 90, rout) + extra;
                area.y2 = y + scaled(end, rin) + extra;
            }
            _ => {}
        }
    } else {
        match (start_quarter, end_quarter) {
            (0, 1) => {
                area.x1 = x + scaled(end + 90, rout) - extra;
                area.y1 = y + (sin(end).min(sin(start)).wrapping_mul(rin) >> TRIGO_SHIFT) - extra;
                area.x2 = x + scaled(start + 90, rout) + extra;
                area.y2 = y + rout + extra;
            }
            (1, 2) => {
                area.x1 = x - rout - extra;
                area.y1 = y + scaled(end, rout) - extra;
                area.x2 = x + (sin(start + 90).max(sin(end + 90)).wrapping_mul(rin) >> TRIGO_SHIFT) + extra;
                area.y2 = y + scaled(start, rout) + extra;
            }
            (2, 3) => {
                area.x1 = x + scaled(start + 90, rout) - extra;
                area.y1 = y - rout - extra;
                area.x2 = x + scaled(end + 90, rout) + extra;
                area.y2 =
                    y + (sin(end).wrapping_mul(rin).max(sin(start).wrapping_mul(rin)) >> TRIGO_SHIFT) + extra;
            }
            (3, 0) => {
                area.x1 = x + (sin(end + 90).min(sin(start + 90)).wrapping_mul(rin) >> TRIGO_SHIFT) - extra;
                area.y1 = y + scaled(start, rout) - extra;
                area.x2 = x + rout + extra;
                area.y2 = y + scaled(end, rout) + extra;
            }
            _ => *area = outer,
        }
    }
}

impl DrawCtx {
    /// `lv_draw_arc()`
    pub fn draw_arc(&mut self, layer: LayerId, dsc: &ArcDsc) {
        draw_arc_impl(self, layer, dsc);
    }

    /// `lv_draw_task_get_arc_dsc()`
    #[must_use]
    pub fn task_get_arc_dsc(&mut self, task: DrawTaskRef) -> Option<&mut ArcDsc> {
        let task = self.task_mut(task)?;
        if task.ty != DrawTaskType::Arc {
            return None;
        }
        match &mut task.dsc {
            DrawDsc::Arc(dsc) => Some(dsc),
            _ => None,
        }
    }
}

impl Ui {
    /// `lv_draw_arc()` with object events, draw-unit evaluation and dispatch.
    pub fn draw_arc(&mut self, layer: LayerId, dsc: &ArcDsc) {
        draw_arc_impl(self, layer, dsc);
    }
}

fn draw_arc_impl<S: TaskSink + ?Sized>(sink: &mut S, layer: LayerId, dsc: &ArcDsc) {
    if !check_layer(sink.ctx(), layer)
        || dsc.opa <= OPA_MIN
        || dsc.width == 0
        || dsc.start_angle == dsc.end_angle
    {
        return;
    }
    let radius = i32::from(dsc.radius);
    let area = Area::new(
        dsc.center.x - radius,
        dsc.center.y - radius,
        dsc.center.x + radius - 1,
        dsc.center.y + radius - 1,
    );
    if dsc.base.drop_shadow_opa != 0
        && let Some(shadow) = sink.create_drop_shadow(layer, &dsc.base, &area)
    {
        let mut shadow_dsc = dsc.clone();
        shadow_dsc.base.drop_shadow_opa = 0;
        draw_arc_impl(sink, shadow, &shadow_dsc);
        sink.finish_drop_shadow(shadow, &dsc.base);
    }
    let task = sink.ctx().add_task(layer, &area, DrawTaskType::Arc);
    set_task_dsc(sink.ctx(), task, DrawDsc::Arc(Box::new(dsc.clone())));
    sink.finalize(task);
}

#[cfg(test)]
#[allow(clippy::unnecessary_cast)] // Test angles also support integer ValuePrecise.
mod tests {
    use super::*;
    use crate::misc::color::ColorFormat;

    #[test]
    fn arc_defaults_and_task_copy_follow_descriptor_and_bounds() {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Xrgb8888, &Area::new(0, 0, 99, 99));
        let mut dsc = ArcDsc::new();
        assert_eq!(dsc.base, DrawDscBase::new());
        assert_eq!((dsc.width, dsc.opa, dsc.color), (1, 255, Color::BLACK));
        assert!(dsc.img_src.is_none());
        assert!(!dsc.rounded);
        dsc.center = Point::new(50, 40);
        dsc.radius = 20;
        dsc.end_angle = 90 as ValuePrecise;
        ctx.draw_arc(layer, &dsc);
        let task = DrawTaskRef { layer, index: 0 };
        assert_eq!(ctx.task(task).unwrap().area, Area::new(30, 20, 69, 59));
        let stored = ctx.task_get_arc_dsc(task).unwrap();
        assert_eq!(stored.base.layer, Some(layer));
        assert_eq!(stored.center, dsc.center);
        assert_eq!(stored.end_angle, 90 as ValuePrecise);
        let fill = ctx.add_task(layer, &Area::new(0, 0, 1, 1), DrawTaskType::Fill);
        assert!(ctx.task_get_arc_dsc(fill).is_none());
    }

    #[test]
    fn arc_skips_transparent_zero_width_and_equal_angles() {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Xrgb8888, &Area::new(0, 0, 39, 39));
        let base = ArcDsc {
            radius: 10,
            center: Point::new(20, 20),
            end_angle: 90 as ValuePrecise,
            ..ArcDsc::new()
        };
        for dsc in [
            ArcDsc {
                opa: OPA_MIN,
                ..base.clone()
            },
            ArcDsc {
                width: 0,
                ..base.clone()
            },
            ArcDsc {
                end_angle: 0 as ValuePrecise,
                ..base
            },
        ] {
            ctx.draw_arc(layer, &dsc);
        }
        assert!(ctx.layer(layer).unwrap().tasks.is_empty());
    }

    #[test]
    fn arc_bounds_preserve_quadrants_wrapping_and_integer_rounding() {
        for (start, end, rounded, expected) in [
            (0, 90, false, Area::new(100, 200, 120, 220)),
            (90, 180, false, Area::new(80, 200, 100, 220)),
            (180, 270, false, Area::new(80, 180, 100, 200)),
            (270, 360, false, Area::new(100, 180, 120, 200)),
            (30, 60, false, Area::new(108, 208, 117, 217)),
            (30, 60, true, Area::new(105, 205, 120, 220)),
            (390, 420, false, Area::new(108, 208, 117, 217)),
            (0, 360, true, Area::new(80, 180, 120, 220)),
            (60, 30, false, Area::new(80, 180, 120, 220)),
        ] {
            let mut area = Area::default();
            get_area(
                100,
                200,
                20,
                start as ValuePrecise,
                end as ValuePrecise,
                4,
                rounded,
                &mut area,
            );
            assert_eq!(area, expected, "angles {start}..{end}, rounded={rounded}");
        }
        for width in [20, 21, 100] {
            let mut area = Area::default();
            get_area(
                100,
                200,
                20,
                30 as ValuePrecise,
                60 as ValuePrecise,
                width,
                true,
                &mut area,
            );
            assert_eq!(area, Area::new(80, 180, 120, 220));
        }
    }

    #[cfg(feature = "float")]
    #[test]
    fn arc_bounds_truncate_fractional_angles_like_c() {
        let mut area = Area::default();
        get_area(100, 200, 20, 30.99, 60.99, 4, false, &mut area);
        assert_eq!(area, Area::new(108, 208, 117, 217));
    }
}
