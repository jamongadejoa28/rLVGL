//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_img.c` (`lv_draw_sw_layer`, `apply_mask`).
//!
//! `LV_DRAW_TASK_TYPE_LAYER`: blend a finished child layer into its parent
//! by drawing the child's buffer as an image. `LV_USE_LAYER_DEBUG` and
//! `LV_USE_PARALLEL_DRAW_DEBUG` are off in the parity configuration, so
//! their overlays are not ported.
//!
//! The mask image of `bitmap_mask_src` is decoded like the built-in binary
//! decoder (`src/image/lv_bin_decoder.c`, `lv_bin_decoder_open()`) opens an
//! in-memory (`LV_IMAGE_SRC_VARIABLE`) image: compressed images fail
//! (`decode_compressed()` needs `LV_BIN_DECODER_RAM_LOAD`, off in the parity
//! configuration), A1 / A2 / A4 are expanded to a new A8 buffer
//! (`decode_alpha_only()`), an A8 `DrawBuf` source is used as it is and any
//! other `ImageDsc` (static or shared) is wrapped with
//! `lv_draw_buf_from_image()`. Indexed images are wrapped too: upstream
//! converts them to ARGB8888, which fails the A8 / L8 check just the same.
//!
//! Known gap: file (`ImageSrc::Path`) masks fail to open because there is
//! no file system / image decoder seam yet, so the mask is not applied.
//! Upstream's `test_draw_layer_bitmap_mask_from_image`
//! (`draw/draw_layer_bitmap_mask.png`, mask `"A:src/test_files/binimages/cogwheel.A8.bin"`)
//! is therefore an expected parity failure until mask opening is routed
//! through that seam.

use alloc::rc::Rc;

use ferrovgl_core::draw::draw_buf::{DrawBuf, ImageFlags, ImageHeader};
use ferrovgl_core::draw::image::DrawImageDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask, LayerId};
use ferrovgl_core::misc::area::{Align, Area};
use ferrovgl_core::misc::color::{ColorFormat, opa_mix2};
use ferrovgl_core::misc::style::ImageSrc;

use crate::SwDrawUnit;

/// `lv_draw_sw_layer()`: draw a finished sub-layer as an image.
///
/// `dsc.src` must be [`ImageSrc::Layer`]. Upstream copies the descriptor
/// and points `src` at `layer_to_draw->draw_buf`; here `src` stays the
/// layer, which the image helpers resolve to that layer's `draw_buf`
/// (see `ferrovgl_core::draw::image::DecodedImage`), so `dsc` is passed on
/// unchanged.
pub fn layer(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &DrawImageDsc, coords: &Area) {
    /*`lv_layer_t * layer_to_draw = (lv_layer_t *)draw_dsc->src;`*/
    let ImageSrc::Layer(layer_to_draw) = dsc.src else {
        return;
    };
    let Some(l) = ctx.layer(layer_to_draw) else {
        return;
    };

    /*It can happen that nothing was draw on a layer and therefore its buffer is not allocated.
     *In this case just return. */
    if l.draw_buf.is_none() {
        return;
    }

    if !dsc.bitmap_mask_src.is_none() {
        let visible = apply_mask(ctx, dsc, layer_to_draw);
        if !visible {
            return;
        }
    }

    /*The source should be a draw_buf, not a layer (resolved by the image helpers)*/
    crate::img::image(unit, ctx, t, dsc, coords);
}

/// The decoded mask image (`mask_decoder_dsc.decoded`).
enum MaskImage {
    /// Wrapped from an `lv_image_dsc_t` (`lv_draw_buf_from_image()`).
    Wrapped(DrawBuf),
    /// A draw buffer source, used as it is.
    Shared(Rc<DrawBuf>),
}

impl MaskImage {
    /// The draw buffer of the image.
    fn buf(&self) -> &DrawBuf {
        match self {
            MaskImage::Wrapped(b) => b,
            MaskImage::Shared(b) => b,
        }
    }
}

/// `lv_image_decoder_open(&mask_decoder_dsc, draw_dsc->bitmap_mask_src, NULL)`
/// for the in-memory sources (the `LV_IMAGE_SRC_VARIABLE` branch of
/// `lv_bin_decoder_open()`): `None` if the source can't be opened.
fn open_mask(src: &ImageSrc) -> Option<MaskImage> {
    /*`lv_bin_decoder_info()` copies the header of the variable*/
    let (header, data): (ImageHeader, &[u8]) = match src {
        ImageSrc::Static(img) => (img.header, img.data),
        ImageSrc::Shared(img) => (img.header, img.data),
        ImageSrc::DrawBuf(buf) => (buf.header, buf.data()),
        /*No file system, symbol or layer decoder for masks (see the module docs)*/
        ImageSrc::None | ImageSrc::Path(_) | ImageSrc::Symbol(_) | ImageSrc::Layer(_) => return None,
    };

    if header.cf == ColorFormat::Unknown {
        /*LV_LOG_WARN("Image color format is unknown")*/
        return None;
    }

    if header.flags.contains(ImageFlags::COMPRESSED) {
        /*decode_compressed(): "Need LV_BIN_DECODER_RAM_LOAD to be enabled"*/
        return None;
    }

    let cf = header.cf;
    if cf.is_alpha_only() && cf != ColorFormat::A8 {
        let mut header = header;
        if header.stride == 0 {
            /*`lv_image_decoder_get_info()`: "Image decoder didn't set stride. Calculate it from width."*/
            header.stride = img_width_to_stride(&header) as u16;
        }
        return decode_alpha_only(&header, data).map(MaskImage::Wrapped);
    }

    match src {
        ImageSrc::Static(img) => DrawBuf::from_image(img).ok().map(MaskImage::Wrapped),
        ImageSrc::Shared(img) => DrawBuf::from_image(img).ok().map(MaskImage::Wrapped),
        ImageSrc::DrawBuf(buf) => Some(MaskImage::Shared(buf.clone())),
        ImageSrc::None | ImageSrc::Path(_) | ImageSrc::Symbol(_) | ImageSrc::Layer(_) => None,
    }
}

/// `img_width_to_stride()` of `src/image/lv_image_decoder.c`.
fn img_width_to_stride(header: &ImageHeader) -> u32 {
    if header.cf == ColorFormat::Rgb565A8 {
        header.w as u32 * 2
    } else {
        (header.w as u32 * header.cf.bpp() as u32 + 7) >> 3
    }
}

/// `bit_extend()` of `src/image/lv_bin_decoder.c`: replicate a `bpp` bit
/// value over 8 bits.
fn bit_extend(value: u8, bpp: u8) -> u8 {
    if value == 0 {
        return 0;
    }

    let mut res = value;
    let mut bpp_now = bpp;
    while bpp_now < 8 {
        res |= ((value as u32) << (8 - bpp_now)) as u8;
        bpp_now += bpp;
    }

    res
}

/// `decode_alpha_only()` of `src/image/lv_bin_decoder.c` for a variable
/// source: copy `stride * h` bytes of `data` into a new A8 buffer and expand
/// the A1 / A2 / A4 values in place, from the last pixel to the first.
fn decode_alpha_only(header: &ImageHeader, data: &[u8]) -> Option<DrawBuf> {
    let bpp = header.cf.bpp();

    if bpp == 0 {
        /*LV_LOG_ERROR("Error color format: %d", dsc->header.cf)*/
        return None;
    }

    let w = (header.stride as u32 * 8) / bpp as u32;
    let buf_stride = (w * 8 + 7) >> 3; /*stride for img_data*/
    let buf_len = w * header.h as u32; /*always decode to A8 format*/
    let file_len = header.stride as u32 * header.h as u32;

    let mut decoded = DrawBuf::create(header.w as u32, header.h as u32, ColorFormat::A8, buf_stride)?;
    let img_data = decoded.data_mut()?;

    /*Copy from image data*/
    let n = (file_len as usize).min(data.len()).min(img_data.len());
    img_data[..n].copy_from_slice(&data[..n]);

    if header.cf != ColorFormat::A8 {
        /*Convert A1/2/4 to A8 from last pixel to first pixel*/
        let mut in_i = (file_len as usize).wrapping_sub(1);
        let mut out_i = (buf_len as usize).wrapping_sub(1);
        let mask: u8 = ((1u32 << bpp) - 1) as u8;
        let mut shift: u8 = 0;
        for _ in 0..buf_len {
            let value = (img_data[in_i] >> shift) & mask;
            img_data[out_i] = bit_extend(value, bpp);
            shift += bpp;
            if shift >= 8 {
                shift = 0;
                in_i = in_i.wrapping_sub(1);
            }
            out_i = out_i.wrapping_sub(1);
        }
    }

    Some(decoded)
}

/// `apply_mask()`: multiply the alpha channel of the (ARGB8888) buffer of
/// `layer_to_draw` by the A8 / L8 `bitmap_mask_src`, centred on
/// `draw_dsc.image_area`, and clear what is outside the mask. Returns
/// `false` if nothing remains visible.
fn apply_mask(ctx: &mut DrawCtx, draw_dsc: &DrawImageDsc, layer_to_draw: LayerId) -> bool {
    let Some(mask_decoded) = open_mask(&draw_dsc.bitmap_mask_src) else {
        /*LV_LOG_WARN("Could open the mask. The mask is not applied.")*/
        return true;
    };
    let mask_draw_buf = mask_decoded.buf();

    if mask_draw_buf.header.cf != ColorFormat::A8 && mask_draw_buf.header.cf != ColorFormat::L8 {
        /*LV_LOG_WARN("The mask image is not A8/L8 format. The mask is not applied.")*/
        return true;
    }

    let mask_stride = mask_draw_buf.header.stride as u32;

    /*Align the mask to the center*/
    let mut image_area = draw_dsc.image_area; /*Use the whole image area for the alignment*/
    let mut mask_area = Area::default();
    mask_area.set(
        0,
        0,
        mask_draw_buf.header.w as i32 - 1,
        mask_draw_buf.header.h as i32 - 1,
    );
    Area::align(&image_area, &mut mask_area, Align::Center, 0, 0);

    let Some(l) = ctx.layer_mut(layer_to_draw) else {
        return true;
    };
    image_area = l.buf_area; /*The image can be smaller if only a part was rendered. Use this are during rendering*/

    /*Only the intersection of the mask and image needs to be rendered
     *If if there is no intersection there is nothing to render as the image is out of the mask.*/
    let Some(masked_area) = Area::intersect(&mask_area, &image_area) else {
        return false;
    };

    /*`lv_draw_sw_layer()` checked that the buffer exists*/
    let Some(image_draw_buf) = l.draw_buf.as_mut() else {
        return true;
    };
    let buf_w = image_draw_buf.header.w as i32;
    let buf_h = image_draw_buf.header.h as i32;

    /*Clear the sides if any*/
    let mut side_area = Area::new(0, 0, 0, 0);
    /*Top*/
    side_area.x2 = buf_w - 1;
    side_area.y2 = masked_area.y1 - 1 - image_area.y1;
    image_draw_buf.clear(Some(&side_area));

    /*Bottom*/
    side_area.y1 = masked_area.y2 + 1 - image_area.y1;
    side_area.y2 = buf_h - 1;
    image_draw_buf.clear(Some(&side_area));

    /*Left*/
    side_area.y1 = 0;
    side_area.x1 = 0;
    side_area.x2 = masked_area.x1 - 1 - image_area.x1;
    image_draw_buf.clear(Some(&side_area));

    /*Right*/
    side_area.x1 = masked_area.x2 + 1 - image_area.x1;
    side_area.x2 = buf_w - 1;
    image_draw_buf.clear(Some(&side_area));

    /*Seek to the first of the image and mask on the masked area*/
    let mut img_start = image_draw_buf.goto_xy(
        (masked_area.x1 - image_area.x1) as u32,
        (masked_area.y1 - image_area.y1) as u32,
    );
    let mut mask_start = mask_draw_buf.goto_xy(
        (masked_area.x1 - mask_area.x1) as u32,
        (masked_area.y1 - mask_area.y1) as u32,
    );

    let h = masked_area.height();
    let w = masked_area.width();
    let img_stride = image_draw_buf.header.stride as u32;

    let Some(img) = image_draw_buf.data_mut() else {
        return true;
    };
    let mask = mask_draw_buf.data();

    for _y in 0..h {
        for x in 0..w as usize {
            let a = img_start + x * 4 + 3;
            img[a] = opa_mix2(mask[mask_start + x], img[a]);
        }
        img_start += img_stride as usize;
        mask_start += mask_stride as usize;
    }

    /*lv_image_decoder_close(&mask_decoder_dsc)*/
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SwDrawUnit;
    use ferrovgl_core::draw::draw_buf::{ImageDsc, ImageHeader};
    use ferrovgl_core::draw::{DrawDscBase, DrawTaskType};
    use ferrovgl_core::misc::area::Point;
    use ferrovgl_core::misc::color::{Color, OPA_COVER};
    use ferrovgl_core::misc::style::{BlendMode, SharedRef};

    fn image_dsc(src: ImageSrc, image_area: Area, mask: ImageSrc) -> DrawImageDsc {
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
            image_area,
            bitmap_mask_src: mask,
        }
    }

    /// A child layer at `area` with an allocated ARGB8888 buffer whose
    /// every byte is `0xFF` (opaque white).
    fn filled_layer(ctx: &mut DrawCtx, area: Area) -> LayerId {
        let l = ctx.layer_create(None, ColorFormat::Argb8888, &area);
        assert!(ctx.layer_alloc_buf(l));
        let buf = ctx.layer_mut(l).unwrap().draw_buf.as_mut().unwrap();
        buf.data_mut().unwrap().fill(0xFF);
        l
    }

    /// An A8 mask of `w x h` with the value `f(x, y)`.
    fn a8_mask(w: u32, h: u32, f: impl Fn(u32, u32) -> u8) -> ImageSrc {
        let mut buf = DrawBuf::create(w, h, ColorFormat::A8, 0).unwrap();
        let stride = buf.header.stride as usize;
        let data = buf.data_mut().unwrap();
        for y in 0..h {
            for x in 0..w {
                data[y as usize * stride + x as usize] = f(x, y);
            }
        }
        ImageSrc::DrawBuf(Rc::new(buf))
    }

    fn alpha(ctx: &DrawCtx, l: LayerId, x: u32, y: u32) -> u8 {
        let buf = ctx.layer(l).unwrap().draw_buf.as_ref().unwrap();
        buf.data()[buf.goto_xy(x, y) + 3]
    }

    fn pixel(ctx: &DrawCtx, l: LayerId, x: u32, y: u32) -> [u8; 4] {
        let buf = ctx.layer(l).unwrap().draw_buf.as_ref().unwrap();
        let o = buf.goto_xy(x, y);
        let d = buf.data();
        [d[o], d[o + 1], d[o + 2], d[o + 3]]
    }

    #[test]
    fn mask_is_centred_and_the_sides_are_cleared() {
        let mut ctx = DrawCtx::new();
        let area = Area::new(10, 20, 19, 29); /*10 x 10*/
        let l = filled_layer(&mut ctx, area);
        // A 4 x 2 mask, centred on the 10 x 10 image: x 3..=6, y 4..=5.
        let mask = a8_mask(4, 2, |x, _| [255, 128, 64, 0][x as usize]);
        let dsc = image_dsc(ImageSrc::Layer(l), area, mask);

        assert!(apply_mask(&mut ctx, &dsc, l));

        for y in 0..10 {
            for x in 0..10 {
                let inside = (3..=6).contains(&x) && (4..=5).contains(&y);
                if !inside {
                    assert_eq!(pixel(&ctx, l, x, y), [0; 4], "({x}, {y}) is cleared");
                }
            }
        }
        // LV_OPA_MIX2(mask, 255) = mask * 255 >> 8
        for (x, m) in [(3, 255u32), (4, 128), (5, 64), (6, 0)] {
            for y in 4..=5 {
                assert_eq!(alpha(&ctx, l, x, y) as u32, (m * 255) >> 8);
                assert_eq!(pixel(&ctx, l, x, y)[..3], [0xFF; 3], "colour is kept");
            }
        }
    }

    #[test]
    fn mask_uses_the_rendered_part_of_the_image() {
        // The whole image is 0..=19 x 0..=9 but only its right half was
        // rendered into the layer: the mask (centred on the whole image)
        // is offset into the buffer by `buf_area`.
        let mut ctx = DrawCtx::new();
        let image_area = Area::new(0, 0, 19, 9);
        let l = filled_layer(&mut ctx, Area::new(10, 0, 19, 9));
        // 12 x 10 mask, centred: absolute x 4..=15.
        let mask = a8_mask(12, 10, |x, _| 100 + x as u8);
        let dsc = image_dsc(ImageSrc::Layer(l), image_area, mask);
        assert!(apply_mask(&mut ctx, &dsc, l));
        // Buffer x 0..=5 (absolute 10..=15) is masked with mask x 6..=11.
        for bx in 0..=5u32 {
            assert_eq!(alpha(&ctx, l, bx, 3) as u32, ((100 + 6 + bx) * 255) >> 8);
        }
        // Absolute 16..=19 is outside of the mask.
        for bx in 6..10 {
            assert_eq!(pixel(&ctx, l, bx, 3), [0; 4]);
        }
    }

    #[test]
    fn mask_outside_the_rendered_area_hides_the_layer() {
        let mut ctx = DrawCtx::new();
        let l = filled_layer(&mut ctx, Area::new(50, 50, 59, 59));
        let mask = a8_mask(4, 4, |_, _| 255);
        // Centred on an image area far from the layer.
        let dsc = image_dsc(ImageSrc::Layer(l), Area::new(0, 0, 9, 9), mask);
        assert!(!apply_mask(&mut ctx, &dsc, l));
        // Nothing was touched.
        assert_eq!(pixel(&ctx, l, 0, 0), [0xFF; 4]);
    }

    #[test]
    fn unusable_masks_are_not_applied() {
        let mut ctx = DrawCtx::new();
        let area = Area::new(0, 0, 7, 7);
        let l = filled_layer(&mut ctx, area);

        // Not A8 / L8.
        let rgb = DrawBuf::create(2, 2, ColorFormat::Rgb565, 0).unwrap();
        let dsc = image_dsc(ImageSrc::Layer(l), area, ImageSrc::DrawBuf(Rc::new(rgb)));
        assert!(apply_mask(&mut ctx, &dsc, l));
        // A source that can't be opened.
        let dsc = image_dsc(
            ImageSrc::Layer(l),
            area,
            ImageSrc::Path(SharedRef::Static("A:m.bin")),
        );
        assert!(apply_mask(&mut ctx, &dsc, l));

        assert!(
            ctx.layer(l)
                .unwrap()
                .draw_buf
                .as_ref()
                .unwrap()
                .data()
                .iter()
                .all(|&b| b == 0xFF)
        );
    }

    #[test]
    fn l8_image_dsc_masks_are_wrapped() {
        static MASK_PX: [u8; 4] = [0, 255, 255, 0];
        static MASK: ImageDsc = ImageDsc::new(ImageHeader::new(ColorFormat::L8, 2, 2, 2), &MASK_PX);
        let mut ctx = DrawCtx::new();
        let area = Area::new(0, 0, 1, 1);
        let l = filled_layer(&mut ctx, area);
        let dsc = image_dsc(ImageSrc::Layer(l), area, ImageSrc::Static(&MASK));
        assert!(apply_mask(&mut ctx, &dsc, l));
        assert_eq!(alpha(&ctx, l, 0, 0), 0);
        assert_eq!(alpha(&ctx, l, 1, 0), 254);
        assert_eq!(alpha(&ctx, l, 0, 1), 254);
        assert_eq!(alpha(&ctx, l, 1, 1), 0);
    }

    #[test]
    fn bit_extend_replicates_the_value() {
        assert_eq!(bit_extend(0, 1), 0);
        assert_eq!(bit_extend(1, 1), 0xFF);
        assert_eq!(bit_extend(1, 2), 0x55);
        assert_eq!(bit_extend(2, 2), 0xAA);
        assert_eq!(bit_extend(3, 2), 0xFF);
        assert_eq!(bit_extend(0x1, 4), 0x11);
        assert_eq!(bit_extend(0x8, 4), 0x88);
        assert_eq!(bit_extend(0xF, 4), 0xFF);
    }

    #[test]
    fn decode_alpha_only_expands_to_a8_with_the_padded_width() {
        // A4, w = 3 but stride 2: the decoded width is stride * 8 / bpp = 4.
        let header = ImageHeader::new(ColorFormat::A4, 3, 2, 2);
        let d = decode_alpha_only(&header, &[0x12, 0x30, 0x45, 0x6F]).unwrap();
        assert_eq!(d.header.cf, ColorFormat::A8);
        assert_eq!((d.header.w, d.header.h, d.header.stride), (3, 2, 4));
        assert_eq!(&d.data()[..8], &[0x11, 0x22, 0x33, 0x00, 0x44, 0x55, 0x66, 0xFF]);

        // A1: the most significant bit is the first pixel.
        let header = ImageHeader::new(ColorFormat::A1, 8, 1, 1);
        let d = decode_alpha_only(&header, &[0b1100_0001]).unwrap();
        assert_eq!(&d.data()[..8], &[0xFF, 0xFF, 0, 0, 0, 0, 0, 0xFF]);

        // A2
        let header = ImageHeader::new(ColorFormat::A2, 4, 1, 1);
        let d = decode_alpha_only(&header, &[0b00_01_10_11]).unwrap();
        assert_eq!(&d.data()[..4], &[0x00, 0x55, 0xAA, 0xFF]);
    }

    #[test]
    fn a4_image_dsc_mask_is_decoded_and_applied() {
        // 4 x 4 A4 mask, every nibble 0xF, centred on a 10 x 10 layer: x, y 3..=6.
        static MASK_PX: [u8; 8] = [0xFF; 8];
        static MASK: ImageDsc = ImageDsc::new(ImageHeader::new(ColorFormat::A4, 4, 4, 2), &MASK_PX);
        let mut ctx = DrawCtx::new();
        let area = Area::new(0, 0, 9, 9);
        let l = filled_layer(&mut ctx, area);
        let dsc = image_dsc(ImageSrc::Layer(l), area, ImageSrc::Static(&MASK));
        assert!(apply_mask(&mut ctx, &dsc, l));
        for y in 0..10 {
            for x in 0..10 {
                let inside = (3..=6).contains(&x) && (3..=6).contains(&y);
                if inside {
                    assert_eq!(pixel(&ctx, l, x, y), [0xFF, 0xFF, 0xFF, 254], "({x}, {y})");
                } else {
                    assert_eq!(pixel(&ctx, l, x, y), [0; 4], "({x}, {y}) is cleared");
                }
            }
        }
    }

    #[test]
    fn a2_draw_buf_and_stride_0_a1_masks_are_decoded() {
        // A2 draw buffer source, 4 x 1: values 0, 1, 2, 3.
        let mut buf = DrawBuf::create(4, 1, ColorFormat::A2, 0).unwrap();
        buf.data_mut().unwrap()[0] = 0b00_01_10_11;
        let mut ctx = DrawCtx::new();
        let area = Area::new(0, 0, 3, 0);
        let l = filled_layer(&mut ctx, area);
        let dsc = image_dsc(ImageSrc::Layer(l), area, ImageSrc::DrawBuf(Rc::new(buf)));
        assert!(apply_mask(&mut ctx, &dsc, l));
        for (x, m) in [(0, 0x00u32), (1, 0x55), (2, 0xAA), (3, 0xFF)] {
            assert_eq!(alpha(&ctx, l, x, 0) as u32, (m * 255) >> 8);
        }

        // A1 image without stride (legacy): stride = (w * bpp + 7) >> 3.
        static A1_PX: [u8; 2] = [0b1000_0000, 0b0100_0000];
        static A1: ImageDsc = ImageDsc::new(ImageHeader::new(ColorFormat::A1, 2, 2, 0), &A1_PX);
        let mut ctx = DrawCtx::new();
        let area = Area::new(0, 0, 1, 1);
        let l = filled_layer(&mut ctx, area);
        let dsc = image_dsc(ImageSrc::Layer(l), area, ImageSrc::Static(&A1));
        assert!(apply_mask(&mut ctx, &dsc, l));
        assert_eq!(alpha(&ctx, l, 0, 0), 254);
        assert_eq!(alpha(&ctx, l, 1, 0), 0);
        assert_eq!(alpha(&ctx, l, 0, 1), 0);
        assert_eq!(alpha(&ctx, l, 1, 1), 254);
    }

    #[test]
    fn compressed_masks_are_not_applied() {
        static MASK_PX: [u8; 4] = [0; 4];
        static MASK: ImageDsc = ImageDsc {
            header: ImageHeader {
                flags: ImageFlags::COMPRESSED,
                ..ImageHeader::new(ColorFormat::A8, 2, 2, 2)
            },
            data_size: 4,
            data: &MASK_PX,
        };
        let mut ctx = DrawCtx::new();
        let area = Area::new(0, 0, 1, 1);
        let l = filled_layer(&mut ctx, area);
        let dsc = image_dsc(ImageSrc::Layer(l), area, ImageSrc::Static(&MASK));
        assert!(apply_mask(&mut ctx, &dsc, l));
        assert_eq!(pixel(&ctx, l, 0, 0), [0xFF; 4]);
        assert_eq!(pixel(&ctx, l, 1, 1), [0xFF; 4]);
    }

    #[test]
    fn layer_without_buffer_or_hidden_by_its_mask_draws_nothing() {
        let mut ctx = DrawCtx::new();
        let parent = ctx.layer_create(None, ColorFormat::Argb8888, &Area::new(0, 0, 9, 9));
        let t = ctx.add_task(parent, &Area::new(0, 0, 9, 9), DrawTaskType::Layer);
        let task = ctx.task(t).unwrap().clone();
        let mut unit = SwDrawUnit::new(0);

        // Nothing drawn on the child: no buffer.
        let child = ctx.layer_create(Some(parent), ColorFormat::Argb8888, &Area::new(0, 0, 9, 9));
        let dsc = image_dsc(ImageSrc::Layer(child), Area::new(0, 0, 9, 9), ImageSrc::None);
        layer(&mut unit, &mut ctx, &task, &dsc, &Area::new(0, 0, 9, 9));

        // The mask is out of the rendered part: `img::image` isn't reached.
        let child = filled_layer(&mut ctx, Area::new(0, 0, 9, 9));
        let dsc = image_dsc(
            ImageSrc::Layer(child),
            Area::new(100, 100, 109, 109),
            a8_mask(2, 2, |_, _| 255),
        );
        layer(&mut unit, &mut ctx, &task, &dsc, &Area::new(0, 0, 9, 9));

        // Not a layer source.
        let dsc = image_dsc(ImageSrc::None, Area::new(0, 0, 9, 9), ImageSrc::None);
        layer(&mut unit, &mut ctx, &task, &dsc, &Area::new(0, 0, 9, 9));
    }
}
