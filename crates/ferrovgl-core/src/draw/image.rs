//! Ported from lvgl v9.6.0: `src/draw/lv_draw_image.c`
//! (with `include/lvgl/draw/lv_draw_image.h` and `src/draw/lv_draw_image_private.h`).
//!
//! Task creation (`lv_draw_image`, `lv_draw_layer`) and the decode helpers
//! shared by every renderer (`lv_draw_image_normal_helper`,
//! `lv_draw_image_tiled_helper`).
//!
//! # Task creation
//!
//! Like `lv_draw_rect.c` (see `draw::rect`), every creator exists twice:
//! `DrawCtx::draw_*` (unit-less finalization) and `Ui::draw_*` (the full
//! `lv_draw_finalize_task_creation()`), sharing one generic body.
//!
//! # The decode seam
//!
//! There is no image decoder registry yet. `lv_image_decoder_get_info()` /
//! `lv_image_decoder_open()` are reduced to what LVGL's built-in binary
//! decoder (`lv_bin_decoder.c`) does for in-memory sources, which is what
//! the reference images exercise:
//!
//! - [`ImageSrc::Static`] / [`ImageSrc::Shared`] (`lv_image_dsc_t`): plain
//!   formats are used in place (`lv_draw_buf_from_image`), `A1`/`A2`/`A4` are
//!   expanded to `A8` (`decode_alpha_only`), indexed images are decoded line
//!   by line to `ARGB8888` (`LV_BIN_DECODER_RAM_LOAD == 0`:
//!   `lv_bin_decoder_get_area`, the "draw in smaller pieces" path).
//! - [`ImageSrc::DrawBuf`]: like an `lv_draw_buf_t` passed as `src`
//!   (`LV_IMAGE_FLAGS_ALLOCATED`), used in place.
//! - [`ImageSrc::Layer`]: the layer's `draw_buf` (what `lv_draw_sw_layer`
//!   substitutes for the source). The buffer is taken out of the layer for
//!   the duration of the core callback and put back afterwards, so the
//!   callback can borrow the [`DrawCtx`] mutably.
//! - [`ImageSrc::Path`] (needs a file system and a decoder registry),
//!   [`ImageSrc::Symbol`] (the binary decoder only reports its info) and
//!   compressed images (RLE / LZ4) cannot be opened: the open fails and
//!   nothing is drawn, like an upstream build without a matching decoder.
//!
//! Custom drawn images (`LV_IMAGE_FLAGS_CUSTOM_DRAW`, a decoder's
//! `custom_draw_cb`) need a decoder too; that branch of `lv_draw_image` is a
//! no-op.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::core::Ui;
use crate::draw::draw_buf::{
    DrawBuf, IMAGE_FLAGS_USER_MASK, IMAGE_HEADER_LEGACY, IMAGE_HEADER_MAGIC, ImageDsc, ImageFlags,
    ImageHeader, STRIDE_AUTO,
};
use crate::draw::rect::{TaskSink, check_layer, set_task_dsc};
use crate::draw::{DrawCtx, DrawDsc, DrawDscBase, DrawTaskRef, DrawTaskState, DrawTaskType, LayerId};
use crate::misc::area::{Area, Point};
use crate::misc::color::{Color, Color32, ColorFormat, OPA_COVER, OPA_MIN, Opa};
use crate::misc::style::{BlendMode, ImageColorkey, ImageSrc, ImageSrcType, SCALE_NONE};
use crate::misc::types::coord;

/// `LV_COLOR_DEPTH` of the parity configuration (800x480 XRGB8888).
const COLOR_DEPTH: u32 = 32;

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
        Self {
            /*lv_memzero(dsc, sizeof(lv_draw_image_dsc_t))*/
            base: DrawDscBase::new(),
            src: ImageSrc::None,
            header: ImageHeader::default(),
            clip_radius: 0,
            rotation: 0,
            scale_x: SCALE_NONE,
            scale_y: SCALE_NONE,
            skew_x: 0,
            skew_y: 0,
            pivot: Point::new(0, 0),
            recolor: Color::black(),
            recolor_opa: 0,
            opa: OPA_COVER,
            blend_mode: BlendMode::Normal,
            antialias: COLOR_DEPTH > 8,
            tile: false,
            colorkey: None,
            sup: None,
            /*Indicate invalid area by default by setting a negative size*/
            image_area: Area::new(0, 0, coord::MIN, 0),
            bitmap_mask_src: ImageSrc::None,
        }
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

/*
 * Task creation
 */

/// `lv_draw_layer()`
fn draw_layer_impl<S: TaskSink + ?Sized>(s: &mut S, layer: LayerId, dsc: &DrawImageDsc, coords: &Area) {
    if !check_layer(s.ctx(), layer) {
        return;
    }

    if dsc.scale_x <= 0 || dsc.scale_y <= 0 {
        /* NOT draw if scale is negative or zero */
        return;
    }

    let t = s.ctx().add_task(layer, coords, DrawTaskType::Layer);
    let mut new_image_dsc = dsc.clone();

    let mut real_area = Area::default();
    image_buf_get_transformed_area(
        &mut real_area,
        coords.width(),
        coords.height(),
        dsc.rotation,
        dsc.scale_x as u16,
        dsc.scale_y as u16,
        dsc.pivot,
    );
    real_area.move_by(coords.x1, coords.y1);

    /*If the image_area is not set assume that it's the same as the rendering area */
    if new_image_dsc.image_area.x2 == coord::MIN {
        new_image_dsc.image_area = *coords;
    }

    if let Some(task) = s.ctx().task_mut(t) {
        task.state = DrawTaskState::Blocked;
        task.real_area = real_area;
    }
    set_task_dsc(s.ctx(), t, DrawDsc::Layer(Box::new(new_image_dsc)));

    /*`(lv_layer_t *)dsc->src`: only a layer source can be marked complete*/
    if let ImageSrc::Layer(layer_to_draw) = dsc.src
        && let Some(l) = s.ctx().layer_mut(layer_to_draw)
    {
        l.all_tasks_added = true;
    }

    s.finalize(t);
}

/// `lv_draw_image()`
fn draw_image_impl<S: TaskSink + ?Sized>(s: &mut S, layer: LayerId, dsc: &DrawImageDsc, coords: &Area) {
    if !check_layer(s.ctx(), layer) {
        return;
    }

    if dsc.src.is_none() {
        #[cfg(feature = "log")]
        log::warn!("Image draw: src is NULL");
        return;
    }
    if dsc.opa <= OPA_MIN {
        return;
    }

    if dsc.scale_x <= 0 || dsc.scale_y <= 0 {
        /* NOT draw if scale is negative or zero */
        return;
    }

    if dsc.base.drop_shadow_opa != 0
        && let Some(ds_layer) = s.ctx().layer_create_drop_shadow(layer, &dsc.base, coords)
    {
        let mut ds_dsc = dsc.clone();
        ds_dsc.base.drop_shadow_opa = 0; /*Disable drop shadow so rendering below will render plain image*/
        draw_image_impl(s, ds_layer, &ds_dsc, coords);
        s.ctx().layer_finish_drop_shadow(ds_layer, &dsc.base);
    }

    let mut new_image_dsc = dsc.clone();
    let Some(header) = image_decoder_get_info(s.ctx(), &new_image_dsc.src) else {
        #[cfg(feature = "log")]
        log::warn!("Couldn't get info about the image");
        return;
    };
    new_image_dsc.header = header;

    /*If the image_area is not set assume that it's the same as the rendering area */
    if new_image_dsc.image_area.x2 == coord::MIN {
        new_image_dsc.image_area = *coords;
    }

    /*Typical case, draw the image as bitmap*/
    if !new_image_dsc.header.flags.contains(ImageFlags::CUSTOM_DRAW) {
        let t = s.ctx().add_task(layer, coords, DrawTaskType::Image);
        set_task_dsc(s.ctx(), t, DrawDsc::Image(Box::new(new_image_dsc)));

        let mut real_area = Area::default();
        image_buf_get_transformed_area(
            &mut real_area,
            coords.width(),
            coords.height(),
            dsc.rotation,
            dsc.scale_x as u16,
            dsc.scale_y as u16,
            dsc.pivot,
        );
        real_area.move_by(coords.x1, coords.y1);
        if let Some(task) = s.ctx().task_mut(t) {
            task.real_area = real_area;
        }

        s.finalize(t);
    }
    /*Use a custom draw callback*/
    else {
        /*`lv_image_decoder_open()` + `decoder->custom_draw_cb`: no decoder
         *registry exists (see the module docs), so there is no
         *`custom_draw_cb` to call.*/
        #[cfg(feature = "log")]
        log::error!("Failed to open image");
    }
}

impl DrawCtx {
    /// `lv_draw_image()`
    pub fn draw_image(&mut self, layer: LayerId, dsc: &DrawImageDsc, coords: &Area) {
        draw_image_impl(self, layer, dsc, coords);
    }

    /// `lv_draw_layer()`: draw a sub-layer (`dsc.src` is `ImageSrc::Layer`).
    pub fn draw_layer(&mut self, layer: LayerId, dsc: &DrawImageDsc, coords: &Area) {
        draw_layer_impl(self, layer, dsc, coords);
    }

    /// `lv_draw_task_get_image_dsc()`
    pub fn task_get_image_dsc(&mut self, task: DrawTaskRef) -> Option<&mut DrawImageDsc> {
        let task = self.task_mut(task)?;
        if task.ty != DrawTaskType::Image {
            return None;
        }
        match &mut task.dsc {
            DrawDsc::Image(d) => Some(d),
            _ => None,
        }
    }

    /// `lv_draw_image_normal_helper()`: decode `dsc.src`, clip, build the
    /// `sup` and call `draw_core_cb` once (shared by every renderer).
    ///
    /// `decoder_args` is always `NULL` (the defaults).
    pub fn image_normal_helper(
        &mut self,
        t: DrawTaskRef,
        dsc: &DrawImageDsc,
        coords: &Area,
        draw_core_cb: DrawImageCoreCb<'_>,
    ) {
        let draw_dsc = dsc;
        let mut draw_area = *coords;
        if draw_dsc.rotation != 0 || draw_dsc.scale_x != SCALE_NONE || draw_dsc.scale_y != SCALE_NONE {
            let w = coords.width();
            let h = coords.height();

            image_buf_get_transformed_area(
                &mut draw_area,
                w,
                h,
                draw_dsc.rotation,
                draw_dsc.scale_x as u16,
                draw_dsc.scale_y as u16,
                draw_dsc.pivot,
            );

            draw_area.x1 += coords.x1;
            draw_area.y1 += coords.y1;
            draw_area.x2 += coords.x1;
            draw_area.y2 += coords.y1;
        }

        let Some(clip_area) = self.task(t).map(|task| task.clip_area) else {
            crate::misc::check_arg_failed("t != NULL");
            return;
        };
        let Some(clipped_img_area) = Area::intersect(&draw_area, &clip_area) else {
            return;
        };

        let Some(mut decoder_dsc) = image_decoder_open(self, &draw_dsc.src) else {
            #[cfg(feature = "log")]
            log::error!("Failed to open image");
            return;
        };

        img_decode_and_draw(
            self,
            t,
            draw_dsc,
            &mut decoder_dsc,
            None,
            coords,
            &clipped_img_area,
            draw_core_cb,
        );

        image_decoder_close(self, decoder_dsc);
    }

    /// `lv_draw_image_tiled_helper()`: like [`Self::image_normal_helper`]
    /// but calls `draw_core_cb` for every tile.
    ///
    /// Deviation: an image of zero (or negative) width or height returns
    /// before the tile loop, which would never terminate upstream.
    pub fn image_tiled_helper(
        &mut self,
        t: DrawTaskRef,
        dsc: &DrawImageDsc,
        coords: &Area,
        draw_core_cb: DrawImageCoreCb<'_>,
    ) {
        let draw_dsc = dsc;
        let Some(mut decoder_dsc) = image_decoder_open(self, &draw_dsc.src) else {
            #[cfg(feature = "log")]
            log::error!("Failed to open image");
            return;
        };

        let img_w = i32::from(draw_dsc.header.w);
        let img_h = i32::from(draw_dsc.header.h);
        if img_w <= 0 || img_h <= 0 {
            image_decoder_close(self, decoder_dsc);
            return;
        }

        let mut tile_area = if draw_dsc.image_area.width() >= 0 {
            draw_dsc.image_area
        } else {
            *coords
        };
        tile_area.set_width(img_w);
        tile_area.set_height(img_h);

        let tile_x_start = tile_area.x1;

        let mut relative_decoded_area = Area::new(coord::MIN, coord::MIN, coord::MIN, coord::MIN);

        while tile_area.y1 <= coords.y2 {
            while tile_area.x1 <= coords.x2 {
                if let Some(clipped_img_area) = Area::intersect(&tile_area, coords) {
                    img_decode_and_draw(
                        self,
                        t,
                        draw_dsc,
                        &mut decoder_dsc,
                        Some(&mut relative_decoded_area),
                        &tile_area,
                        &clipped_img_area,
                        &mut *draw_core_cb,
                    );
                }

                tile_area.x1 += img_w;
                tile_area.x2 += img_w;
            }

            tile_area.y1 += img_h;
            tile_area.y2 += img_h;
            tile_area.x1 = tile_x_start;
            tile_area.x2 = tile_x_start + img_w - 1;
        }

        image_decoder_close(self, decoder_dsc);
    }
}

impl Ui {
    /// `lv_draw_image()` with the full `lv_draw_finalize_task_creation()`.
    pub fn draw_image(&mut self, layer: LayerId, dsc: &DrawImageDsc, coords: &Area) {
        draw_image_impl(self, layer, dsc, coords);
    }

    /// `lv_draw_layer()` with the full `lv_draw_finalize_task_creation()`.
    pub fn draw_layer(&mut self, layer: LayerId, dsc: &DrawImageDsc, coords: &Area) {
        draw_layer_impl(self, layer, dsc, coords);
    }
}

/// `lv_image_src_get_type()`
///
/// The variant decides between variable, file and symbol; like upstream a
/// variable source whose header magic is neither `LV_IMAGE_HEADER_MAGIC` nor
/// `LV_IMAGE_HEADER_LEGACY` (a freed or corrupted buffer) is
/// `LV_IMAGE_SRC_UNKNOWN`.
pub fn image_src_get_type(src: &ImageSrc) -> ImageSrcType {
    let magic = match src {
        ImageSrc::Static(d) => d.header.magic,
        ImageSrc::Shared(d) => d.header.magic,
        ImageSrc::DrawBuf(b) => b.header.magic,
        _ => return src.src_type(),
    };

    /*Only LV_IMAGE_HEADER_MAGIC and LV_IMAGE_HEADER_LEGACY are valid.
     *Any other value indicates a freed or corrupted buffer.*/
    if magic == IMAGE_HEADER_MAGIC || magic == IMAGE_HEADER_LEGACY {
        return ImageSrcType::Variable;
    }

    #[cfg(feature = "log")]
    log::error!("image src invalid magic: 0x{:02X}", magic);
    ImageSrcType::Unknown
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
    if angle == 0 && i32::from(scale_x) == SCALE_NONE && i32::from(scale_y) == SCALE_NONE {
        res.x1 = 0;
        res.y1 = 0;
        res.x2 = w - 1;
        res.y2 = h - 1;
        return;
    }

    let mut p = [
        Point::new(0, 0),
        Point::new(w - 1, 0),
        Point::new(0, h - 1),
        Point::new(w - 1, h - 1),
    ];
    for pt in p.iter_mut() {
        pt.transform(angle, i32::from(scale_x), i32::from(scale_y), pivot, true);
    }
    res.x1 = p[0].x.min(p[1].x).min(p[2].x.min(p[3].x));
    res.x2 = p[0].x.max(p[1].x).max(p[2].x.max(p[3].x));
    res.y1 = p[0].y.min(p[1].y).min(p[2].y.min(p[3].y));
    res.y2 = p[0].y.max(p[1].y).max(p[2].y.max(p[3].y));
}

/*
 * Static functions
 */

/// `img_decode_and_draw()`
#[allow(clippy::too_many_arguments)] // mirrors the C signature
fn img_decode_and_draw(
    ctx: &mut DrawCtx,
    t: DrawTaskRef,
    draw_dsc: &DrawImageDsc,
    decoder_dsc: &mut DecoderDsc,
    relative_decoded_area: Option<&mut Area>,
    img_area: &Area,
    clipped_img_area: &Area,
    draw_core_cb: DrawImageCoreCb<'_>,
) {
    let sup = DrawImageSup {
        alpha_color: draw_dsc.recolor,
        palette: decoder_dsc.palette.clone(),
        palette_size: decoder_dsc.palette_size,
    };

    /*The whole image is available, just draw it*/
    let whole =
        decoder_dsc.decoded.is_some() && relative_decoded_area.as_ref().is_none_or(|a| a.x1 == coord::MIN);
    if whole {
        if let Some(decoded) = decoder_dsc.decoded.as_ref() {
            let img = DecodedImage {
                header: decoder_dsc.header,
                decoded: decoded.buf(),
                palette: &decoder_dsc.palette,
                palette_size: decoder_dsc.palette_size,
            };
            draw_core_cb(ctx, t, draw_dsc, &img, Some(&sup), img_area, clipped_img_area);
        }
    }
    /*Draw in smaller pieces*/
    else {
        if draw_dsc.rotation != 0 || draw_dsc.scale_x != SCALE_NONE || draw_dsc.scale_y != SCALE_NONE {
            #[cfg(feature = "log")]
            log::warn!("Partially decoded images cannot be transformed");
            return;
        }

        let mut relative_full_area_to_decode = *clipped_img_area;
        relative_full_area_to_decode.move_by(-img_area.x1, -img_area.y1);
        let mut tmp = Area::default();
        let relative_decoded_area = match relative_decoded_area {
            Some(a) => a,
            None => &mut tmp,
        };
        *relative_decoded_area = Area::new(coord::MIN, coord::MIN, coord::MIN, coord::MIN);
        let mut res_ok = true;

        while res_ok {
            res_ok = decoder_dsc.get_area(&relative_full_area_to_decode, relative_decoded_area);

            let mut absolute_decoded_area = *relative_decoded_area;
            absolute_decoded_area.move_by(img_area.x1, img_area.y1);
            if res_ok {
                /*Limit draw area to the current decoded area and draw the image*/
                if let Some(clipped_img_area_sub) = Area::intersect(clipped_img_area, &absolute_decoded_area)
                    && let Some(partial) = decoder_dsc.decoded_partial.as_ref()
                {
                    let img = DecodedImage {
                        header: decoder_dsc.header,
                        decoded: partial,
                        palette: &decoder_dsc.palette,
                        palette_size: decoder_dsc.palette_size,
                    };
                    draw_core_cb(
                        ctx,
                        t,
                        draw_dsc,
                        &img,
                        Some(&sup),
                        &absolute_decoded_area,
                        &clipped_img_area_sub,
                    );
                }
            }
        }
    }
}

/*
 * The in-memory decoder (`lv_image_decoder.c` + `lv_bin_decoder.c`, see the
 * module docs)
 */

/// Resolve the header of an image source (`lv_image_decoder_get_info()`
/// for in-memory images).
///
/// [`ImageSrc::Layer`] needs the [`DrawCtx`] and returns `None` here;
/// [`ImageSrc::Path`] needs a file system and returns `None` too.
pub fn image_header_of(src: &ImageSrc) -> Option<ImageHeader> {
    let header = match src {
        ImageSrc::None => {
            crate::misc::check_arg_failed("src != NULL");
            return None;
        }
        ImageSrc::Static(_) | ImageSrc::Shared(_) | ImageSrc::DrawBuf(_) => {
            let (header, data) = variable_parts(src)?;
            /*image_decoder_get_info(): the variables should have valid data*/
            if data.is_empty() {
                return None;
            }
            /*lv_bin_decoder_info(): lv_memcpy(header, &image->header, ...)*/
            header
        }
        ImageSrc::Symbol(_) => {
            /*The size depend on the font but it is unknown here. It should be handled outside of the
             *function*/
            /*Symbols always have transparent parts. Important because of cover check in the draw
             *function. The actual value doesn't matter because lv_draw_label will draw it*/
            ImageHeader {
                w: 1,
                h: 1,
                cf: ColorFormat::A8,
                ..ImageHeader::default()
            }
        }
        /*No file system / decoder registry; a layer needs the draw context*/
        ImageSrc::Path(_) | ImageSrc::Layer(_) => return None,
    };
    finish_info(header)
}

/// Borrow the in-memory descriptor behind a variable image source.
pub fn image_dsc_of(src: &ImageSrc) -> Option<&ImageDsc> {
    match src {
        ImageSrc::Static(d) => Some(d),
        ImageSrc::Shared(d) => Some(d),
        _ => None,
    }
}

/// The tail of `lv_bin_decoder_info()` + `image_decoder_get_info()`.
fn finish_info(mut header: ImageHeader) -> Option<ImageHeader> {
    if header.cf == ColorFormat::Unknown {
        #[cfg(feature = "log")]
        log::warn!("Image color format is unknown");
        return None;
    }

    /*For backward compatibility, all images are not premultiplied for now.*/
    if header.magic != IMAGE_HEADER_MAGIC {
        header.flags.remove(ImageFlags::PREMULTIPLIED);
    }

    if header.stride == 0 {
        /*Image decoder didn't set stride. Calculate it from width.*/
        header.stride = img_width_to_stride(&header) as u16;
    }
    Some(header)
}

/// `img_width_to_stride()`
fn img_width_to_stride(header: &ImageHeader) -> u32 {
    if header.cf == ColorFormat::Rgb565A8 {
        u32::from(header.w) * 2
    } else {
        (u32::from(header.w) * u32::from(header.cf.bpp()) + 7) >> 3
    }
}

/// `(header, data)` of an `lv_image_dsc_t`-like source.
fn variable_parts(src: &ImageSrc) -> Option<(ImageHeader, &[u8])> {
    match src {
        ImageSrc::Static(d) => Some((d.header, d.data)),
        ImageSrc::Shared(d) => Some((d.header, d.data)),
        ImageSrc::DrawBuf(b) => Some((b.header, b.data())),
        _ => None,
    }
}

/// `lv_image_decoder_get_info()` with access to the layers.
fn image_decoder_get_info(ctx: &DrawCtx, src: &ImageSrc) -> Option<ImageHeader> {
    match src {
        ImageSrc::Layer(l) => {
            let buf = ctx.layer(*l)?.draw_buf.as_ref()?;
            if buf.data().is_empty() {
                return None;
            }
            finish_info(buf.header)
        }
        _ => image_header_of(src),
    }
}

/// `decoded` of an open image.
enum Decoded {
    /// A buffer owned by the decoder (`decoder_data->c_array` / `decoded`).
    Owned(DrawBuf),
    /// A draw buffer used as the source (`LV_IMAGE_FLAGS_ALLOCATED`).
    Shared(Rc<DrawBuf>),
}

impl Decoded {
    fn buf(&self) -> &DrawBuf {
        match self {
            Decoded::Owned(b) => b,
            Decoded::Shared(b) => b,
        }
    }
}

/// The source bytes `lv_bin_decoder_get_area()` reads indexed rows from.
enum SrcBytes {
    /// `lv_image_dsc_t::data`
    Static(&'static [u8]),
    /// `lv_draw_buf_t::data`
    Buf(Rc<DrawBuf>),
}

impl SrcBytes {
    fn bytes(&self) -> &[u8] {
        match self {
            SrcBytes::Static(s) => s,
            SrcBytes::Buf(b) => b.data(),
        }
    }
}

/// `lv_image_decoder_dsc_t` of the in-memory decoder.
struct DecoderDsc {
    /// `header`
    header: ImageHeader,
    /// `palette`
    palette: Vec<Color32>,
    /// `palette_size`
    palette_size: u16,
    /// `decoded`: the whole image, `None` when it has to be read with
    /// [`DecoderDsc::get_area`].
    decoded: Option<Decoded>,
    /// `decoder_data->decoded_partial`
    decoded_partial: Option<DrawBuf>,
    /// The source data for [`DecoderDsc::get_area`] (`dsc->src`).
    src_bytes: Option<SrcBytes>,
    /// The layer whose buffer is in `decoded` (put back on close).
    layer: Option<LayerId>,
}

impl DecoderDsc {
    fn new(header: ImageHeader) -> Self {
        Self {
            header,
            palette: Vec::new(),
            palette_size: 0,
            decoded: None,
            decoded_partial: None,
            src_bytes: None,
            layer: None,
        }
    }

    /// `lv_image_decoder_get_area()` -> `lv_bin_decoder_get_area()`:
    /// `true` for `LV_RESULT_OK`.
    fn get_area(&mut self, full_area: &Area, decoded_area: &mut Area) -> bool {
        let cf = self.header.cf;
        if !(cf.is_indexed()
            || cf == ColorFormat::Argb8888
            || cf == ColorFormat::Xrgb8888
            || cf == ColorFormat::Rgb888
            || cf == ColorFormat::Rgb565
            || cf == ColorFormat::Rgb565Swapped
            || cf == ColorFormat::Argb8565
            || cf == ColorFormat::Rgb565A8)
        {
            crate::misc::check_arg_failed("Unsupported color format");
            return false;
        }
        if !(full_area.x1 >= 0
            && full_area.x2 < i32::from(self.header.w)
            && full_area.y1 >= 0
            && full_area.y2 < i32::from(self.header.h))
        {
            crate::misc::check_arg_failed("Area outside image bounds");
            return false;
        }
        let Some(src) = self.src_bytes.as_ref() else {
            crate::misc::check_arg_failed("decoder data unavailable");
            return false;
        };

        let bpp = u32::from(cf.bpp());
        let w_px = full_area.width();
        /*Skip the image header: only variables, so the offset starts at 0*/
        let mut offset: u32 = 0;

        /*We only support read line by line for now*/
        if decoded_area.y1 == coord::MIN {
            /*Indexed image is converted to ARGB888*/
            let cf_decoded = if cf.is_indexed() {
                ColorFormat::Argb8888
            } else {
                cf
            };

            let reshaped = self
                .decoded_partial
                .as_mut()
                .is_some_and(|b| b.reshape(cf_decoded, w_px as u32, 1, STRIDE_AUTO));
            if !reshaped {
                self.decoded_partial = None;
                let Some(b) = DrawBuf::create(w_px as u32, 1, cf_decoded, STRIDE_AUTO) else {
                    return false;
                };
                self.decoded_partial = Some(b); /*Free on decoder close*/
            }
            *decoded_area = *full_area;
            decoded_area.y2 = decoded_area.y1;
        } else {
            decoded_area.y1 += 1;
            decoded_area.y2 += 1;
        }

        if decoded_area.y1 > full_area.y2 {
            return false;
        }

        if cf.is_indexed() {
            let x_fraction = decoded_area.x1 % (8 / bpp as i32);

            offset = offset.wrapping_add(u32::from(self.palette_size) * 4); /*Skip palette*/
            offset = offset.wrapping_add((decoded_area.y1 * i32::from(self.header.stride)) as u32);
            offset = offset.wrapping_add((decoded_area.x1 as u32).wrapping_mul(bpp) / 8); /*Move to x1*/

            let data = src.bytes();
            let input = data.get(offset as usize..).unwrap_or(&[]);
            let Some(out) = self.decoded_partial.as_mut().and_then(|b| b.data_mut()) else {
                return false;
            };
            decode_indexed_line(cf, &self.palette, x_fraction, w_px, input, out);
            return true;
        }

        /*The other formats are read from a file*/
        false
    }
}

/// `lv_image_decoder_open()` -> `lv_bin_decoder_open()` with the default
/// `lv_image_decoder_args_t` (no stride alignment, no premultiplication),
/// so `lv_image_decoder_post_process()` never changes the buffer.
fn image_decoder_open(ctx: &mut DrawCtx, src: &ImageSrc) -> Option<DecoderDsc> {
    if src.is_none() {
        return None;
    }
    let header = image_decoder_get_info(ctx, src)?;
    let mut dsc = DecoderDsc::new(header);

    match src {
        ImageSrc::Layer(l) => {
            /*The layer's `draw_buf` is used in place*/
            let buf = ctx.layer_mut(*l)?.draw_buf.take()?;
            dsc.decoded = Some(Decoded::Owned(buf));
            dsc.layer = Some(*l);
            return Some(dsc);
        }
        ImageSrc::Static(_) | ImageSrc::Shared(_) | ImageSrc::DrawBuf(_) => {}
        _ => return None,
    }

    let (image_header, data) = variable_parts(src)?;
    /*The variables should have valid data*/
    if data.is_empty() {
        return None;
    }

    let cf = image_header.cf;
    if dsc.header.flags.contains(ImageFlags::COMPRESSED) {
        /*decode_compressed(): RLE / LZ4 are not supported*/
        return None;
    } else if cf.is_indexed() {
        /*decode_indexed() with LV_BIN_DECODER_RAM_LOAD == 0: only the palette is loaded,
         *the pixels are read by get_area_cb later*/
        let palette_size = cf.indexed_palette_size();
        dsc.palette = (0..palette_size as usize)
            .map(|i| {
                let px = |k: usize| data.get(i * 4 + k).copied().unwrap_or(0);
                Color32 {
                    blue: px(0),
                    green: px(1),
                    red: px(2),
                    alpha: px(3),
                }
            })
            .collect();
        dsc.palette_size = palette_size as u16;
        dsc.src_bytes = Some(match src {
            ImageSrc::DrawBuf(b) => SrcBytes::Buf(b.clone()),
            ImageSrc::Static(d) => SrcBytes::Static(d.data),
            ImageSrc::Shared(d) => SrcBytes::Static(d.data),
            _ => return None,
        });
        /*Need to read via get_area_cb*/
        return Some(dsc);
    } else if cf.is_alpha_only() && cf != ColorFormat::A8 {
        /*Alpha only image will need decoder data to store pointer to decoded image, to free it when decoder closes*/
        let decoded = decode_alpha_only(&dsc.header, data)?;
        dsc.decoded = Some(Decoded::Owned(decoded));
    } else {
        /*In case of uncompressed formats the image stored in the ROM/RAM.
         *So simply give its pointer*/
        match src {
            ImageSrc::DrawBuf(b) => {
                /*`LV_IMAGE_FLAGS_ALLOCATED`: `decoded = (lv_draw_buf_t *)image`*/
                dsc.decoded = Some(Decoded::Shared(b.clone()));
            }
            _ => {
                let image = image_dsc_of(src)?;
                let res = if image.header.stride == 0 {
                    /*If image doesn't have stride, treat it as lvgl v8 legacy image format*/
                    let mut tmp = image.clone();
                    tmp.header.stride = ((u32::from(tmp.header.w) * u32::from(cf.bpp()) + 7) >> 3) as u16;
                    DrawBuf::from_image(&tmp)
                } else {
                    DrawBuf::from_image(image)
                };
                let Ok(mut decoded) = res else {
                    return None;
                };
                if decoded.header.stride == 0 {
                    /*Use the auto calculated value from decoder_info callback*/
                    decoded.header.stride = dsc.header.stride;
                }
                dsc.decoded = Some(Decoded::Owned(decoded));
            }
        }
    }

    if let Some(Decoded::Owned(decoded)) = dsc.decoded.as_mut() {
        if dsc.header.flags.contains(ImageFlags::PREMULTIPLIED) {
            decoded.set_flag(ImageFlags::PREMULTIPLIED);
        }

        /* Copy user flags to the decoded image */
        let user = dsc.header.flags.bits() & IMAGE_FLAGS_USER_MASK;
        if user != 0 {
            decoded.set_flag(ImageFlags::from_bits_retain(user));
        }
    }

    Some(dsc)
}

/// `lv_image_decoder_open()` + `lv_image_decoder_close()` around `f`, for
/// renderers that need a decoded image outside the draw helpers (e.g. the
/// bitmap mask of `lv_draw_sw_layer`).
///
/// `f` gets `None` when the open fails or when the image has no whole
/// `decoded` buffer (`decoder_dsc.decoded == NULL`: indexed images are read
/// in pieces). A layer source's buffer is out of its layer while `f` runs.
pub fn with_decoded_image<R>(
    ctx: &mut DrawCtx,
    src: &ImageSrc,
    f: impl FnOnce(&mut DrawCtx, Option<&DecodedImage<'_>>) -> R,
) -> R {
    let Some(dsc) = image_decoder_open(ctx, src) else {
        return f(ctx, None);
    };
    let res = match dsc.decoded.as_ref() {
        Some(decoded) => {
            let img = DecodedImage {
                header: dsc.header,
                decoded: decoded.buf(),
                palette: &dsc.palette,
                palette_size: dsc.palette_size,
            };
            f(ctx, Some(&img))
        }
        None => f(ctx, None),
    };
    image_decoder_close(ctx, dsc);
    res
}

/// `lv_image_decoder_close()`: give a layer its buffer back.
fn image_decoder_close(ctx: &mut DrawCtx, dsc: DecoderDsc) {
    if let (Some(l), Some(Decoded::Owned(buf))) = (dsc.layer, dsc.decoded)
        && let Some(layer) = ctx.layer_mut(l)
    {
        layer.draw_buf = Some(buf);
    }
}

/// `bit_extend()`
#[inline]
fn bit_extend(value: u8, bpp: u8) -> u8 {
    if value == 0 {
        return 0;
    }

    let mut res = value;
    let mut bpp_now = bpp;
    while bpp_now < 8 {
        res |= value.wrapping_shl(u32::from(8 - bpp_now));
        bpp_now += bpp;
    }

    res
}

/// `decode_alpha_only()` for a variable source: expand A1/A2/A4 to A8.
fn decode_alpha_only(header: &ImageHeader, data: &[u8]) -> Option<DrawBuf> {
    let bpp = header.cf.bpp();

    if bpp == 0 {
        #[cfg(feature = "log")]
        log::error!("Error color format: {:?}", header.cf);
        return None;
    }

    let w = (u32::from(header.stride) * 8) / u32::from(bpp);
    let buf_stride = (w * 8 + 7) >> 3; /*stride for img_data*/
    let buf_len = (w * u32::from(header.h)) as usize; /*always decode to A8 format*/
    let file_len = (u32::from(header.stride) * u32::from(header.h)) as usize;

    let mut decoded = DrawBuf::create(
        u32::from(header.w),
        u32::from(header.h),
        ColorFormat::A8,
        buf_stride,
    )?;

    let img_data = decoded.data_mut()?;

    /*Copy from image data*/
    let n = file_len.min(data.len()).min(img_data.len());
    img_data[..n].copy_from_slice(&data[..n]);

    if header.cf != ColorFormat::A8 {
        /*Convert A1/2/4 to A8 from last pixel to first pixel*/
        let mut in_i = file_len as isize - 1;
        let mut out_i = buf_len as isize - 1;
        let mask: u8 = ((1u16 << bpp) - 1) as u8;
        let mut shift: u8 = 0;
        for _ in 0..buf_len {
            let byte = usize::try_from(in_i)
                .ok()
                .and_then(|i| img_data.get(i).copied())
                .unwrap_or(0);
            let value = (byte >> shift) & mask;
            if let Some(o) = usize::try_from(out_i).ok().and_then(|o| img_data.get_mut(o)) {
                *o = bit_extend(value, bpp);
            }
            shift += bpp;
            if shift >= 8 {
                shift = 0;
                in_i -= 1;
            }
            out_i -= 1;
        }
    }

    Some(decoded)
}

/// Write `palette[index]` as `lv_color32_t` at pixel `i` of `out`.
#[inline]
fn put_color32(out: &mut [u8], i: usize, palette: &[Color32], index: u8) {
    let c = palette.get(index as usize).copied().unwrap_or_default();
    if let Some(px) = out.get_mut(i * 4..i * 4 + 4) {
        px.copy_from_slice(&[c.blue, c.green, c.red, c.alpha]);
    }
}

/// `decode_indexed_line()` (and its `_i1` / `_i2` / `_i4` / `_i8` helpers):
/// `w_px` pixels starting at pixel `x` of `input` to ARGB8888 `out`.
fn decode_indexed_line(
    cf: ColorFormat,
    palette: &[Color32],
    x: i32,
    w_px: i32,
    input: &[u8],
    out: &mut [u8],
) {
    let rd = |i: usize| input.get(i).copied().unwrap_or(0);
    let w_px = w_px.max(0) as usize;
    match cf {
        ColorFormat::I1 => {
            /*8 pixels per byte, MSB first (bit 7 is the left-most pixel)*/
            let mut p = (x >> 3) as usize;
            let mut shift: i32 = 7 - (x & 0x7);
            let mut i = 0usize;

            /*Head: consume the rest of the first byte so the main loop starts byte-aligned.*/
            if shift != 7 {
                let byte = rd(p);
                p += 1;
                while shift >= 0 && i < w_px {
                    put_color32(out, i, palette, (byte >> shift) & 0x1);
                    i += 1;
                    shift -= 1;
                }
            }

            /*Body: one full byte -> 8 pixels per iteration*/
            while i + 8 <= w_px {
                let byte = rd(p);
                p += 1;
                for k in 0..8 {
                    put_color32(out, i + k, palette, (byte >> (7 - k)) & 0x1);
                }
                i += 8;
            }

            /*Tail: the remaining 1..7 pixels of the last partial byte.*/
            if i < w_px {
                let byte = rd(p);
                let mut shift = 7;
                while i < w_px {
                    put_color32(out, i, palette, (byte >> shift) & 0x1);
                    i += 1;
                    shift -= 1;
                }
            }
        }
        ColorFormat::I2 => {
            /*4 pixels per byte, MSB first: bits 6-7, 4-5, 2-3, 0-1.*/
            let mut p = (x >> 2) as usize;
            let mut shift: i32 = 6 - 2 * (x & 0x3);
            let mut i = 0usize;

            if shift != 6 {
                let byte = rd(p);
                p += 1;
                while shift >= 0 && i < w_px {
                    put_color32(out, i, palette, (byte >> shift) & 0x3);
                    i += 1;
                    shift -= 2;
                }
            }

            while i + 4 <= w_px {
                let byte = rd(p);
                p += 1;
                for k in 0..4 {
                    put_color32(out, i + k, palette, (byte >> (6 - 2 * k)) & 0x3);
                }
                i += 4;
            }

            if i < w_px {
                let byte = rd(p);
                let mut shift = 6;
                while i < w_px {
                    put_color32(out, i, palette, (byte >> shift) & 0x3);
                    i += 1;
                    shift -= 2;
                }
            }
        }
        ColorFormat::I4 => {
            /*2 pixels per byte: high nibble (bits 4-7) is the left pixel, low nibble (bits 0-3) the right.*/
            let mut p = (x >> 1) as usize;
            let mut i = 0usize;

            /*Head: if x is odd we start mid-byte at the low nibble; consume it to byte-align.*/
            if (x & 0x1) != 0 && i < w_px {
                put_color32(out, i, palette, rd(p) & 0x0F);
                i += 1;
                p += 1;
            }

            while i + 2 <= w_px {
                let byte = rd(p);
                p += 1;
                put_color32(out, i, palette, byte >> 4);
                put_color32(out, i + 1, palette, byte & 0x0F);
                i += 2;
            }

            if i < w_px {
                put_color32(out, i, palette, rd(p) >> 4);
            }
        }
        ColorFormat::I8 => {
            let p = x.max(0) as usize;
            for i in 0..w_px {
                put_color32(out, i, palette, rd(p + i));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::draw_buf::IMAGE_HEADER_DEADBEEF;
    use crate::misc::style::SharedRef;
    use alloc::vec;

    static PX_ARGB: [u8; 16] = [
        1, 2, 3, 255, 4, 5, 6, 255, //
        7, 8, 9, 255, 10, 11, 12, 255,
    ];
    static IMG_ARGB: ImageDsc = ImageDsc::new(ImageHeader::new(ColorFormat::Argb8888, 2, 2, 8), &PX_ARGB);

    /// I1 2x8 image: palette (black, white) + 1 byte per row.
    static PX_I1: [u8; 10] = [0, 0, 0, 255, 255, 255, 255, 255, 0b1010_0101, 0b1111_0000];
    static IMG_I1: ImageDsc = ImageDsc::new(ImageHeader::new(ColorFormat::I1, 8, 2, 1), &PX_I1);

    static PX_A4: [u8; 2] = [0xF0, 0x18];
    static IMG_A4: ImageDsc = ImageDsc::new(ImageHeader::new(ColorFormat::A4, 2, 2, 1), &PX_A4);

    fn ctx_with_layer(area: Area) -> (DrawCtx, LayerId) {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, ColorFormat::Argb8888, &area);
        ctx.layer_mut(layer).unwrap().clip_area = area;
        (ctx, layer)
    }

    #[test]
    fn dsc_init_defaults() {
        let d = DrawImageDsc::new();
        assert_eq!(d.recolor, Color::black());
        assert_eq!(d.opa, OPA_COVER);
        assert_eq!(d.scale_x, SCALE_NONE);
        assert_eq!(d.scale_y, SCALE_NONE);
        assert!(d.antialias);
        assert_eq!(d.image_area.x2, coord::MIN);
        assert!(d.image_area.width() < 0);
        assert!(d.src.is_none());
        assert_eq!(d.recolor_opa, 0);
        assert!(!d.tile);
    }

    /* test_image_src_get_type.c */
    #[test]
    fn src_get_type_null() {
        assert_eq!(image_src_get_type(&ImageSrc::None), ImageSrcType::Unknown);
    }

    #[test]
    fn src_get_type_file_and_symbol() {
        assert_eq!(
            image_src_get_type(&ImageSrc::Path(SharedRef::Static("A:test.png"))),
            ImageSrcType::File
        );
        assert_eq!(
            image_src_get_type(&ImageSrc::Symbol(SharedRef::Static("\u{F00C}"))),
            ImageSrcType::Symbol
        );
    }

    #[test]
    fn src_get_type_valid_draw_buf() {
        let buf = DrawBuf::create(10, 10, ColorFormat::Argb8888, STRIDE_AUTO).unwrap();
        assert_eq!(
            image_src_get_type(&ImageSrc::DrawBuf(Rc::new(buf))),
            ImageSrcType::Variable
        );
    }

    #[test]
    fn src_get_type_legacy_and_deadbeef() {
        let mut d = ImageDsc::new(ImageHeader::new(ColorFormat::Argb8888, 10, 10, 0), &[]);
        d.header.magic = IMAGE_HEADER_LEGACY;
        d.data_size = 400;
        assert_eq!(
            image_src_get_type(&ImageSrc::Shared(Rc::new(d.clone()))),
            ImageSrcType::Variable
        );
        d.header.magic = IMAGE_HEADER_DEADBEEF;
        assert_eq!(
            image_src_get_type(&ImageSrc::Shared(Rc::new(d))),
            ImageSrcType::Unknown
        );
    }

    #[test]
    fn src_get_type_destroyed_draw_buf() {
        let mut buf = DrawBuf::create(10, 10, ColorFormat::Argb8888, STRIDE_AUTO).unwrap();
        buf.header.magic = IMAGE_HEADER_DEADBEEF;
        assert_eq!(
            image_src_get_type(&ImageSrc::DrawBuf(Rc::new(buf))),
            ImageSrcType::Unknown
        );
    }

    #[test]
    fn transformed_area_identity_and_rotation() {
        let mut a = Area::default();
        image_buf_get_transformed_area(&mut a, 10, 20, 0, 256, 256, Point::new(5, 10));
        assert_eq!(a, Area::new(0, 0, 9, 19));

        /*180 degrees around the centre keeps the bounding box*/
        image_buf_get_transformed_area(&mut a, 11, 21, 1800, 256, 256, Point::new(5, 10));
        assert_eq!(a, Area::new(0, 0, 10, 20));

        /*2x around the origin doubles the extent*/
        image_buf_get_transformed_area(&mut a, 10, 10, 0, 512, 512, Point::new(0, 0));
        assert_eq!(a.x1, 0);
        assert_eq!(a.y1, 0);
        assert_eq!(a.x2, 18);
        assert_eq!(a.y2, 18);
    }

    #[test]
    fn header_of_sources() {
        let h = image_header_of(&ImageSrc::Static(&IMG_ARGB)).unwrap();
        assert_eq!(h.cf, ColorFormat::Argb8888);
        assert_eq!((h.w, h.h, h.stride), (2, 2, 8));

        /*Symbol: 1x1 A8, no magic*/
        let h = image_header_of(&ImageSrc::Symbol(SharedRef::Static("x"))).unwrap();
        assert_eq!((h.w, h.h, h.cf, h.stride, h.magic), (1, 1, ColorFormat::A8, 1, 0));

        #[cfg(not(feature = "check-arg-panic"))]
        assert!(image_header_of(&ImageSrc::None).is_none());
        assert!(image_header_of(&ImageSrc::Path(SharedRef::Static("A:x.bin"))).is_none());

        /*Stride 0 is computed from the width; legacy magic drops PREMULTIPLIED*/
        let mut d = ImageDsc::new(ImageHeader::new(ColorFormat::Rgb565A8, 3, 1, 0), &PX_ARGB);
        d.header.magic = IMAGE_HEADER_LEGACY;
        d.header.flags = ImageFlags::PREMULTIPLIED;
        let h = image_header_of(&ImageSrc::Shared(Rc::new(d))).unwrap();
        assert_eq!(h.stride, 6);
        assert!(!h.flags.contains(ImageFlags::PREMULTIPLIED));

        /*No data: NULL data pointer*/
        let d = ImageDsc::new(ImageHeader::new(ColorFormat::Argb8888, 2, 2, 8), &[]);
        assert!(image_header_of(&ImageSrc::Shared(Rc::new(d))).is_none());

        /*Unknown colour format*/
        let d = ImageDsc::new(ImageHeader::new(ColorFormat::Unknown, 2, 2, 8), &PX_ARGB);
        assert!(image_header_of(&ImageSrc::Shared(Rc::new(d))).is_none());
    }

    #[test]
    fn dsc_of() {
        assert!(image_dsc_of(&ImageSrc::Static(&IMG_ARGB)).is_some());
        assert!(image_dsc_of(&ImageSrc::None).is_none());
    }

    #[test]
    fn bit_extend_maps_full_range() {
        assert_eq!(bit_extend(0, 4), 0);
        assert_eq!(bit_extend(0xF, 4), 0xFF);
        assert_eq!(bit_extend(0x8, 4), 0x88);
        assert_eq!(bit_extend(1, 1), 0xFF);
        assert_eq!(bit_extend(0x2, 2), 0xAA);
    }

    #[test]
    fn alpha_only_expands_to_a8() {
        let h = image_header_of(&ImageSrc::Static(&IMG_A4)).unwrap();
        let buf = decode_alpha_only(&h, &PX_A4).unwrap();
        assert_eq!(buf.header.cf, ColorFormat::A8);
        assert_eq!(&buf.data()[..4], &[0xFF, 0x00, 0x11, 0x88]);
    }

    #[test]
    fn indexed_line_decoding() {
        let pal = [Color32::make(0, 0, 0, 255), Color32::make(255, 255, 255, 255)];
        let mut out = vec![0u8; 8 * 4];
        decode_indexed_line(ColorFormat::I1, &pal, 0, 8, &[0b1010_0101], &mut out);
        let alpha_red: Vec<u8> = out.chunks(4).map(|p| p[2]).collect();
        assert_eq!(alpha_red, vec![255, 0, 255, 0, 0, 255, 0, 255]);

        /*Start mid byte*/
        let mut out = vec![0u8; 3 * 4];
        decode_indexed_line(ColorFormat::I1, &pal, 5, 3, &[0b1010_0101], &mut out);
        let red: Vec<u8> = out.chunks(4).map(|p| p[2]).collect();
        assert_eq!(red, vec![255, 0, 255]);

        let pal16: Vec<Color32> = (0..16).map(|i| Color32::make(i, 0, 0, 255)).collect();
        let mut out = vec![0u8; 3 * 4];
        decode_indexed_line(ColorFormat::I4, &pal16, 1, 3, &[0x12, 0x34], &mut out);
        let red: Vec<u8> = out.chunks(4).map(|p| p[2]).collect();
        assert_eq!(red, vec![2, 3, 4]);
    }

    #[test]
    fn draw_image_creates_a_task() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let mut dsc = DrawImageDsc::new();
        dsc.src = ImageSrc::Static(&IMG_ARGB);
        ctx.draw_image(layer, &dsc, &Area::new(10, 10, 11, 11));

        let t = DrawTaskRef { layer, index: 0 };
        assert_eq!(ctx.task_get_type(t), DrawTaskType::Image);
        let task = ctx.task(t).unwrap();
        assert_eq!(task.real_area, Area::new(10, 10, 11, 11));
        let d = ctx.task_get_image_dsc(t).unwrap();
        assert_eq!(d.header.w, 2);
        /*image_area defaults to the coordinates*/
        assert_eq!(d.image_area, Area::new(10, 10, 11, 11));
        assert_eq!(d.base.layer, Some(layer));
    }

    #[test]
    fn draw_image_early_returns() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let mut dsc = DrawImageDsc::new();
        /*NULL src*/
        ctx.draw_image(layer, &dsc, &Area::new(0, 0, 1, 1));
        dsc.src = ImageSrc::Static(&IMG_ARGB);
        dsc.opa = OPA_MIN;
        ctx.draw_image(layer, &dsc, &Area::new(0, 0, 1, 1));
        dsc.opa = OPA_COVER;
        dsc.scale_x = 0;
        ctx.draw_image(layer, &dsc, &Area::new(0, 0, 1, 1));
        dsc.scale_x = SCALE_NONE;
        dsc.src = ImageSrc::Path(SharedRef::Static("A:nope.bin"));
        ctx.draw_image(layer, &dsc, &Area::new(0, 0, 1, 1));
        assert!(ctx.layer(layer).unwrap().tasks.is_empty());
    }

    #[test]
    fn draw_image_transformed_real_area() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let mut dsc = DrawImageDsc::new();
        dsc.src = ImageSrc::Static(&IMG_ARGB);
        dsc.scale_x = 512;
        dsc.scale_y = 512;
        ctx.draw_image(layer, &dsc, &Area::new(10, 10, 19, 19));
        let task = ctx.task(DrawTaskRef { layer, index: 0 }).unwrap();
        assert_eq!(task.real_area, Area::new(10, 10, 28, 28));
    }

    #[test]
    fn draw_layer_task_is_blocked_and_marks_complete() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let child = ctx.layer_create(Some(layer), ColorFormat::Argb8888, &Area::new(5, 5, 14, 14));
        let mut dsc = DrawImageDsc::new();
        dsc.src = ImageSrc::Layer(child);
        ctx.draw_layer(layer, &dsc, &Area::new(5, 5, 14, 14));
        let t = DrawTaskRef { layer, index: 0 };
        let task = ctx.task(t).unwrap();
        assert_eq!(task.ty, DrawTaskType::Layer);
        assert_eq!(task.state, DrawTaskState::Blocked);
        assert!(ctx.layer(child).unwrap().all_tasks_added);
        /*lv_draw_task_get_image_dsc() only returns IMAGE tasks*/
        assert!(ctx.task_get_image_dsc(t).is_none());
        match &ctx.task(t).unwrap().dsc {
            DrawDsc::Layer(d) => assert_eq!(d.image_area, Area::new(5, 5, 14, 14)),
            _ => panic!("expected a layer dsc"),
        }

        /*Zero scale: nothing*/
        dsc.scale_y = 0;
        ctx.draw_layer(layer, &dsc, &Area::new(5, 5, 14, 14));
        assert_eq!(ctx.layer(layer).unwrap().tasks.len(), 1);
    }

    /// Records the `(img_coords, clipped_img_area, cf)` of every core call.
    type Calls = Vec<(Area, Area, ColorFormat, Vec<u8>)>;

    fn run_helper(ctx: &mut DrawCtx, t: DrawTaskRef, dsc: &DrawImageDsc, coords: &Area) -> Calls {
        let mut calls: Calls = Vec::new();
        let mut cb = |_: &mut DrawCtx,
                      _: DrawTaskRef,
                      _: &DrawImageDsc,
                      img: &DecodedImage<'_>,
                      sup: Option<&DrawImageSup>,
                      a: &Area,
                      c: &Area| {
            assert!(sup.is_some());
            calls.push((*a, *c, img.decoded.header.cf, img.decoded.data().to_vec()));
        };
        if dsc.tile {
            ctx.image_tiled_helper(t, dsc, coords, &mut cb);
        } else {
            ctx.image_normal_helper(t, dsc, coords, &mut cb);
        }
        calls
    }

    #[test]
    fn normal_helper_whole_image() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        ctx.layer_mut(layer).unwrap().clip_area = Area::new(0, 0, 10, 10);
        let t = ctx.add_task(layer, &Area::new(10, 10, 11, 11), DrawTaskType::Image);
        let mut dsc = DrawImageDsc::new();
        dsc.src = ImageSrc::Static(&IMG_ARGB);
        let calls = run_helper(&mut ctx, t, &dsc, &Area::new(10, 10, 11, 11));
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, Area::new(10, 10, 11, 11));
        assert_eq!(calls[0].1, Area::new(10, 10, 10, 10));
        assert_eq!(calls[0].2, ColorFormat::Argb8888);
        assert_eq!(&calls[0].3[..], &PX_ARGB[..]);

        /*Fully clipped: no call*/
        let calls = run_helper(&mut ctx, t, &dsc, &Area::new(50, 50, 51, 51));
        assert!(calls.is_empty());
    }

    #[test]
    fn normal_helper_indexed_draws_row_by_row() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let t = ctx.add_task(layer, &Area::new(0, 0, 7, 1), DrawTaskType::Image);
        let mut dsc = DrawImageDsc::new();
        dsc.src = ImageSrc::Static(&IMG_I1);
        let calls = run_helper(&mut ctx, t, &dsc, &Area::new(0, 0, 7, 1));
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, Area::new(0, 0, 7, 0));
        assert_eq!(calls[1].0, Area::new(0, 1, 7, 1));
        assert_eq!(calls[0].2, ColorFormat::Argb8888);
        let red: Vec<u8> = calls[1].3.chunks(4).map(|p| p[2]).collect();
        assert_eq!(red, vec![255, 255, 255, 255, 0, 0, 0, 0]);

        /*Partially decoded images cannot be transformed*/
        dsc.rotation = 10;
        let calls = run_helper(&mut ctx, t, &dsc, &Area::new(0, 0, 7, 1));
        assert!(calls.is_empty());
    }

    #[test]
    fn normal_helper_layer_source_returns_the_buffer() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let child = ctx.layer_create(Some(layer), ColorFormat::Argb8888, &Area::new(0, 0, 3, 3));
        assert!(ctx.layer_alloc_buf(child));
        let t = ctx.add_task(layer, &Area::new(0, 0, 3, 3), DrawTaskType::Layer);
        let mut dsc = DrawImageDsc::new();
        dsc.src = ImageSrc::Layer(child);
        let mut seen_taken = false;
        let mut cb = |c: &mut DrawCtx,
                      _: DrawTaskRef,
                      _: &DrawImageDsc,
                      img: &DecodedImage<'_>,
                      _: Option<&DrawImageSup>,
                      _: &Area,
                      _: &Area| {
            seen_taken = c.layer(child).unwrap().draw_buf.is_none();
            assert_eq!(img.decoded.header.w, 4);
        };
        ctx.image_normal_helper(t, &dsc, &Area::new(0, 0, 3, 3), &mut cb);
        assert!(seen_taken);
        assert!(ctx.layer(child).unwrap().draw_buf.is_some());
    }

    #[test]
    fn tiled_helper_covers_the_area() {
        let (mut ctx, layer) = ctx_with_layer(Area::new(0, 0, 99, 99));
        let t = ctx.add_task(layer, &Area::new(0, 0, 4, 2), DrawTaskType::Image);
        let mut dsc = DrawImageDsc::new();
        dsc.src = ImageSrc::Static(&IMG_ARGB);
        dsc.header = image_header_of(&dsc.src).unwrap();
        dsc.tile = true;
        let calls = run_helper(&mut ctx, t, &dsc, &Area::new(0, 0, 4, 2));
        /*3 columns x 2 rows of 2x2 tiles*/
        assert_eq!(calls.len(), 6);
        assert_eq!(calls[0].0, Area::new(0, 0, 1, 1));
        assert_eq!(calls[2].0, Area::new(4, 0, 5, 1));
        assert_eq!(calls[2].1, Area::new(4, 0, 4, 1));
        assert_eq!(calls[5].1, Area::new(4, 2, 4, 2));

        /*Indexed tiles are decoded row by row in every tile*/
        dsc.src = ImageSrc::Static(&IMG_I1);
        dsc.header = image_header_of(&dsc.src).unwrap();
        let calls = run_helper(&mut ctx, t, &dsc, &Area::new(0, 0, 9, 1));
        assert_eq!(calls.len(), 4);
        assert_eq!(calls[2].0, Area::new(8, 0, 9, 0));
        assert_eq!(calls[3].1, Area::new(8, 1, 9, 1));
    }

    #[test]
    fn open_variants() {
        let mut ctx = DrawCtx::new();
        /*A4 is expanded*/
        let d = image_decoder_open(&mut ctx, &ImageSrc::Static(&IMG_A4)).unwrap();
        assert_eq!(d.decoded.as_ref().unwrap().buf().header.cf, ColorFormat::A8);
        /*DrawBuf is used in place*/
        let rc = Rc::new(DrawBuf::create(3, 3, ColorFormat::Rgb565, STRIDE_AUTO).unwrap());
        let d = image_decoder_open(&mut ctx, &ImageSrc::DrawBuf(rc.clone())).unwrap();
        match d.decoded.as_ref().unwrap() {
            Decoded::Shared(b) => assert!(Rc::ptr_eq(b, &rc)),
            Decoded::Owned(_) => panic!("expected the shared buffer"),
        }
        /*Compressed / symbol / file cannot be opened*/
        let mut c = IMG_ARGB.clone();
        c.header.flags = ImageFlags::COMPRESSED;
        assert!(image_decoder_open(&mut ctx, &ImageSrc::Shared(Rc::new(c))).is_none());
        assert!(image_decoder_open(&mut ctx, &ImageSrc::Symbol(SharedRef::Static("x"))).is_none());
        assert!(image_decoder_open(&mut ctx, &ImageSrc::None).is_none());
        /*Indexed: palette loaded, no whole image*/
        let d = image_decoder_open(&mut ctx, &ImageSrc::Static(&IMG_I1)).unwrap();
        assert!(d.decoded.is_none());
        assert_eq!(d.palette_size, 2);
        assert_eq!(d.palette[1], Color32::make(255, 255, 255, 255));
    }
}
