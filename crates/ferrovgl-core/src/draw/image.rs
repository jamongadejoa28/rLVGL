//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_image.h`, `src/draw/lv_draw_image_private.h`,
//! `src/draw/lv_draw_image.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::vec::Vec;

use crate::draw::draw_buf::{DrawBuf, ImageDsc, ImageHeader};
use crate::draw::{DrawCtx, DrawDscBase, DrawTaskRef, LayerId};
use crate::misc::area::{Area, Point};
use crate::misc::color::{Color, Color32, Opa};
use crate::misc::style::{BlendMode, ImageColorkey, ImageSrc, ImageSrcType};

/// `lv_draw_image_sup_t`: supplementary data for indexed / alpha-only images.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DrawImageSup {
    /// `alpha_color`: colour of A1..A8 images.
    pub alpha_color: Color,
    /// `palette`
    pub palette: Vec<Color32>,
    /// `palette_size`
    pub palette_size: u16,
}

/// `lv_draw_image_dsc_t`
#[derive(Clone, Debug, PartialEq)]
pub struct DrawImageDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `src`
    pub src: ImageSrc,
    /// `header`
    pub header: ImageHeader,
    /// `clip_radius`
    pub clip_radius: i32,
    /// `rotation`: in 0.1 degree.
    pub rotation: i32,
    /// `scale_x`: 256 = 1x.
    pub scale_x: i32,
    /// `scale_y`
    pub scale_y: i32,
    /// `skew_x`
    pub skew_x: i32,
    /// `skew_y`
    pub skew_y: i32,
    /// `pivot`
    pub pivot: Point,
    /// `recolor`
    pub recolor: Color,
    /// `recolor_opa`
    pub recolor_opa: Opa,
    /// `opa`
    pub opa: Opa,
    /// `blend_mode`
    pub blend_mode: BlendMode,
    /// `antialias`
    pub antialias: bool,
    /// `tile`
    pub tile: bool,
    /// `colorkey`
    pub colorkey: Option<ImageColorkey>,
    /// `sup`
    pub sup: Option<DrawImageSup>,
    /// `image_area`: the full image area (for tiling / clipping).
    pub image_area: Area,
    /// `bitmap_mask_src`
    pub bitmap_mask_src: ImageSrc,
}

impl DrawImageDsc {
    /// `lv_draw_image_dsc_init()`
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for DrawImageDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// The subset of `lv_image_decoder_dsc_t` the renderers consume
/// (`decoder_dsc->header / decoded / palette / palette_size`).
///
/// The decode seam: `lv_draw_image_normal_helper` / `lv_draw_image_tiled_helper`
/// open the source (`ImageSrc::Static` / `Shared` / `DrawBuf` through
/// [`image_dsc_of`] and [`DrawBuf`], `ImageSrc::Layer(id)` through
/// `DrawCtx::layer(id).draw_buf`, which the `lv_draw_sw_layer` path uses in
/// place of `new_draw_dsc.src = layer_to_draw->draw_buf`; take the buffer
/// out of that other layer for the duration of the callback), build `sup`
/// from the palette and call the renderer's core callback with it.
#[derive(Clone, Copy, Debug)]
pub struct DecodedImage<'a> {
    /// `header`
    pub header: ImageHeader,
    /// `decoded`
    pub decoded: &'a DrawBuf,
    /// `palette` (`palette_size` entries; empty for non-indexed images)
    pub palette: &'a [Color32],
    /// `palette_size`
    pub palette_size: u16,
}

/// `lv_draw_image_core_cb`: a renderer's
/// `(t, draw_dsc, decoder_dsc, sup, img_coords, clipped_img_area)` core.
pub type DrawImageCoreCb<'a> = &'a mut dyn FnMut(
    &mut DrawCtx,
    DrawTaskRef,
    &DrawImageDsc,
    &DecodedImage<'_>,
    Option<&DrawImageSup>,
    &Area,
    &Area,
);

/// `lv_image_src_get_type()`
pub fn image_src_get_type(src: &ImageSrc) -> ImageSrcType {
    src.src_type()
}

/// `lv_image_buf_get_transformed_area()`: bounding box of a transformed image.
pub fn image_buf_get_transformed_area(
    res: &mut Area,
    w: i32,
    h: i32,
    angle: i32,
    scale_x: u16,
    scale_y: u16,
    pivot: Point,
) {
    todo!()
}

impl DrawCtx {
    /// `lv_draw_image()`
    pub fn draw_image(&mut self, layer: LayerId, dsc: &DrawImageDsc, coords: &Area) {
        todo!()
    }

    /// `lv_draw_layer()`: draw a sub-layer (`dsc.src` is `ImageSrc::Layer`).
    pub fn draw_layer(&mut self, layer: LayerId, dsc: &DrawImageDsc, coords: &Area) {
        todo!()
    }

    /// `lv_draw_task_get_image_dsc()`
    pub fn task_get_image_dsc(&mut self, task: DrawTaskRef) -> Option<&mut DrawImageDsc> {
        todo!()
    }

    /// `lv_draw_image_normal_helper()`: decode `dsc.src`, clip, build the
    /// `sup` and call `draw_core_cb` once (shared by every renderer).
    pub fn image_normal_helper(
        &mut self,
        t: DrawTaskRef,
        dsc: &DrawImageDsc,
        coords: &Area,
        draw_core_cb: DrawImageCoreCb<'_>,
    ) {
        todo!()
    }

    /// `lv_draw_image_tiled_helper()`: like [`Self::image_normal_helper`]
    /// but calls `draw_core_cb` for every tile.
    pub fn image_tiled_helper(
        &mut self,
        t: DrawTaskRef,
        dsc: &DrawImageDsc,
        coords: &Area,
        draw_core_cb: DrawImageCoreCb<'_>,
    ) {
        todo!()
    }
}

/// Resolve the header of an image source (`lv_image_decoder_get_info()`
/// for in-memory images).
pub fn image_header_of(src: &ImageSrc) -> Option<ImageHeader> {
    todo!()
}

/// Borrow the in-memory descriptor behind a variable image source.
pub fn image_dsc_of(src: &ImageSrc) -> Option<&ImageDsc> {
    todo!()
}
