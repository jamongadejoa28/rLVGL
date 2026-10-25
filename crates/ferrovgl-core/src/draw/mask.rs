//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_mask.h`, `src/draw/lv_draw_mask.c`.
//!
//! Like `rect.rs`, `DrawCtx::draw_mask_rect` is the unit-less / object-less
//! variant and `Ui::draw_mask_rect` the full upstream flow (including the
//! `LV_EVENT_DRAW_TASK_ADDED` sent by `lv_draw_mask_rect()` itself).

use alloc::boxed::Box;

use crate::core::Ui;
use crate::draw::rect::{TaskSink, check_layer, set_task_dsc};
use crate::draw::{DrawCtx, DrawDsc, DrawDscBase, DrawTaskRef, DrawTaskType, LayerId};
use crate::misc::area::Area;
use crate::misc::style::ImageSrc;

/// `lv_draw_mask_rect_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct MaskRectDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `area`
    pub area: Area,
    /// `radius`
    pub radius: i32,
    /// `keep_outside`: keep (instead of clearing) the pixels outside `area`.
    pub keep_outside: bool,
}

impl MaskRectDsc {
    /// `lv_draw_mask_rect_dsc_init()`
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: DrawDscBase::new(),
            area: Area::new(0, 0, 0, 0),
            radius: 0,
            keep_outside: false,
        }
    }
}

impl Default for MaskRectDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// Descriptor of a `LV_DRAW_TASK_TYPE_MASK_BITMAP` task. Upstream has no
/// struct for it ("no struct match", `lv_draw.c`); the bitmap mask of a layer
/// is applied from `LV_STYLE_BITMAP_MASK_SRC`, carried here.
#[derive(Clone, Debug, PartialEq)]
pub struct MaskBitmapDsc {
    /// `base`
    pub base: DrawDscBase,
    /// The A8 mask image.
    pub src: ImageSrc,
    /// Where the mask is placed.
    pub area: Area,
}

impl MaskBitmapDsc {
    /// A zeroed descriptor (`lv_draw.c` zeroes the `MASK_BITMAP` slot).
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: DrawDscBase::new(),
            src: ImageSrc::None,
            area: Area::new(0, 0, 0, 0),
        }
    }
}

impl Default for MaskBitmapDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_mask_rect()`
fn draw_mask_rect_impl<S: TaskSink + ?Sized>(s: &mut S, layer: LayerId, dsc: &MaskRectDsc) {
    if !check_layer(s.ctx(), layer) {
        return;
    }

    let (color_format, buf_area) = {
        let l = &s.ctx().layers[layer.0];
        (l.color_format, l.buf_area)
    };
    if !color_format.has_alpha() {
        #[cfg(feature = "log")]
        log::warn!("Only layers with alpha channel can be masked");
        return;
    }

    let t = s.ctx().add_task(layer, &buf_area, DrawTaskType::MaskRectangle);

    let mut task_dsc = dsc.clone();
    task_dsc.base.layer = Some(layer);
    set_task_dsc(s.ctx(), t, DrawDsc::MaskRect(Box::new(task_dsc)));

    s.send_mask_task_added(t);

    s.finalize(t);
}

impl DrawCtx {
    /// `lv_draw_mask_rect()`
    pub fn draw_mask_rect(&mut self, layer: LayerId, dsc: &MaskRectDsc) {
        draw_mask_rect_impl(self, layer, dsc);
    }

    /// `lv_draw_task_get_mask_rect_dsc()`
    #[must_use]
    pub fn task_get_mask_rect_dsc(&mut self, task: DrawTaskRef) -> Option<&mut MaskRectDsc> {
        let task = self.task_mut(task)?;
        if task.ty != DrawTaskType::MaskRectangle {
            return None;
        }
        match &mut task.dsc {
            DrawDsc::MaskRect(d) => Some(d),
            _ => None,
        }
    }
}

impl Ui {
    /// `lv_draw_mask_rect()` with `LV_EVENT_DRAW_TASK_ADDED` and the full
    /// task finalization.
    pub fn draw_mask_rect(&mut self, layer: LayerId, dsc: &MaskRectDsc) {
        draw_mask_rect_impl(self, layer, dsc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::misc::color::ColorFormat;

    #[test]
    fn dsc_init_is_zeroed() {
        let d = MaskRectDsc::new();
        assert_eq!(d.area, Area::new(0, 0, 0, 0));
        assert_eq!(d.radius, 0);
        assert!(!d.keep_outside);
        assert_eq!(d.base, DrawDscBase::new());
        assert_eq!(MaskBitmapDsc::new().src, ImageSrc::None);
    }

    #[test]
    fn mask_rect_covers_the_whole_layer_buffer() {
        let mut ctx = DrawCtx::new();
        let buf_area = Area::new(10, 20, 59, 69);
        let layer = ctx.layer_create(None, ColorFormat::Argb8888, &buf_area);
        ctx.layer_mut(layer).unwrap().clip_area = Area::new(15, 25, 30, 40);

        let mut dsc = MaskRectDsc::new();
        dsc.area = Area::new(12, 22, 40, 50);
        dsc.radius = 5;
        ctx.draw_mask_rect(layer, &dsc);

        let l = ctx.layer(layer).unwrap();
        assert_eq!(l.tasks.len(), 1);
        let t = &l.tasks[0];
        assert_eq!(t.ty, DrawTaskType::MaskRectangle);
        assert_eq!(t.area, buf_area);
        assert_eq!(t.real_area, buf_area);
        assert_eq!(t.clip_area, Area::new(15, 25, 30, 40));

        let tref = DrawTaskRef { layer, index: 0 };
        let m = ctx.task_get_mask_rect_dsc(tref).unwrap();
        assert_eq!(m.area, Area::new(12, 22, 40, 50));
        assert_eq!(m.radius, 5);
        assert_eq!(m.base.layer, Some(layer));
        assert!(ctx.task_get_fill_dsc(tref).is_none());
    }

    #[test]
    fn mask_rect_needs_an_alpha_layer() {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Rgb565, &Area::new(0, 0, 9, 9));
        ctx.draw_mask_rect(layer, &MaskRectDsc::new());
        assert!(ctx.layer(layer).unwrap().tasks.is_empty());
    }
}
