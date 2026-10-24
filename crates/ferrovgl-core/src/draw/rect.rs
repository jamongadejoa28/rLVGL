//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_rect.h`, `src/draw/lv_draw_rect.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::draw::grad::GradDsc;
use crate::draw::{DrawCtx, DrawDscBase, DrawTaskRef, LayerId};
use crate::font::FontRef;
use crate::misc::area::Area;
use crate::misc::color::{Color, Opa};
use crate::misc::style::{BorderSide, ImageColorkey, ImageSrc};

/// `LV_RADIUS_CIRCLE`: a radius large enough to always produce a circle.
pub const RADIUS_CIRCLE: i32 = 0x7FFF;

/// `lv_draw_rect_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct RectDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `radius`
    pub radius: i32,
    /// `bg_image_src`
    pub bg_image_src: ImageSrc,
    /// `bg_image_symbol_font`
    pub bg_image_symbol_font: Option<FontRef>,
    /// `bg_image_recolor`
    pub bg_image_recolor: Color,
    /// `bg_image_opa`
    pub bg_image_opa: Opa,
    /// `bg_image_recolor_opa`
    pub bg_image_recolor_opa: Opa,
    /// `bg_image_tiled`
    pub bg_image_tiled: bool,
    /// `bg_opa`
    pub bg_opa: Opa,
    /// `border_opa`
    pub border_opa: Opa,
    /// `outline_opa`
    pub outline_opa: Opa,
    /// `shadow_opa`
    pub shadow_opa: Opa,
    /// `bg_color`
    pub bg_color: Color,
    /// `bg_grad`
    pub bg_grad: GradDsc,
    /// `bg_image_colorkey`
    pub bg_image_colorkey: Option<ImageColorkey>,
    /// `border_color`
    pub border_color: Color,
    /// `border_width`
    pub border_width: i32,
    /// `border_side`
    pub border_side: BorderSide,
    /// `border_post`: draw the border after the children.
    pub border_post: bool,
    /// `outline_color`
    pub outline_color: Color,
    /// `outline_width`
    pub outline_width: i32,
    /// `outline_pad`
    pub outline_pad: i32,
    /// `shadow_color`
    pub shadow_color: Color,
    /// `shadow_width`
    pub shadow_width: i32,
    /// `shadow_offset_x`
    pub shadow_offset_x: i32,
    /// `shadow_offset_y`
    pub shadow_offset_y: i32,
    /// `shadow_spread`
    pub shadow_spread: i32,
}

impl RectDsc {
    /// `lv_draw_rect_dsc_init()`
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for RectDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_fill_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct FillDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `radius`
    pub radius: i32,
    /// `opa`
    pub opa: Opa,
    /// `color`
    pub color: Color,
    /// `grad`
    pub grad: GradDsc,
}

impl FillDsc {
    /// `lv_draw_fill_dsc_init()`
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for FillDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_border_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct BorderDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `radius`
    pub radius: i32,
    /// `color`
    pub color: Color,
    /// `width`
    pub width: i32,
    /// `opa`
    pub opa: Opa,
    /// `side`
    pub side: BorderSide,
}

impl BorderDsc {
    /// `lv_draw_border_dsc_init()`
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for BorderDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_box_shadow_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct BoxShadowDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `radius`
    pub radius: i32,
    /// `color`
    pub color: Color,
    /// `width`
    pub width: i32,
    /// `spread`
    pub spread: i32,
    /// `ofs_x`
    pub ofs_x: i32,
    /// `ofs_y`
    pub ofs_y: i32,
    /// `opa`
    pub opa: Opa,
    /// `bg_cover`: the background fully covers the rectangle, so the shadow
    /// below it can be skipped.
    pub bg_cover: bool,
}

impl BoxShadowDsc {
    /// `lv_draw_box_shadow_dsc_init()`
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for BoxShadowDsc {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawCtx {
    /// `lv_draw_rect()`: split into shadow / fill / border / outline tasks.
    pub fn draw_rect(&mut self, layer: LayerId, dsc: &RectDsc, coords: &Area) {
        todo!()
    }

    /// `lv_draw_fill()`
    pub fn draw_fill(&mut self, layer: LayerId, dsc: &FillDsc, coords: &Area) {
        todo!()
    }

    /// `lv_draw_border()`
    pub fn draw_border(&mut self, layer: LayerId, dsc: &BorderDsc, coords: &Area) {
        todo!()
    }

    /// `lv_draw_box_shadow()`
    pub fn draw_box_shadow(&mut self, layer: LayerId, dsc: &BoxShadowDsc, coords: &Area) {
        todo!()
    }

    /// `lv_draw_task_get_fill_dsc()`
    pub fn task_get_fill_dsc(&mut self, task: DrawTaskRef) -> Option<&mut FillDsc> {
        todo!()
    }

    /// `lv_draw_task_get_border_dsc()`
    pub fn task_get_border_dsc(&mut self, task: DrawTaskRef) -> Option<&mut BorderDsc> {
        todo!()
    }

    /// `lv_draw_task_get_box_shadow_dsc()`
    pub fn task_get_box_shadow_dsc(&mut self, task: DrawTaskRef) -> Option<&mut BoxShadowDsc> {
        todo!()
    }
}
