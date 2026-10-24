//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_mask.h`, `src/draw/lv_draw_mask.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::draw::{DrawCtx, DrawDscBase, DrawTaskRef, LayerId};
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
    pub fn new() -> Self {
        todo!()
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

impl DrawCtx {
    /// `lv_draw_mask_rect()`
    pub fn draw_mask_rect(&mut self, layer: LayerId, dsc: &MaskRectDsc) {
        todo!()
    }

    /// `lv_draw_task_get_mask_rect_dsc()`
    pub fn task_get_mask_rect_dsc(&mut self, task: DrawTaskRef) -> Option<&mut MaskRectDsc> {
        todo!()
    }
}
