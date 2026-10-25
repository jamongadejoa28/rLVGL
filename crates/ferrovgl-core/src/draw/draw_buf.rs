//! Ported from lvgl v9.6.0: `src/draw/lv_draw_buf.c`
//! (with `include/lvgl/draw/lv_draw_buf.h`, `src/draw/lv_draw_buf_private.h`,
//! `include/lvgl/draw/lv_image_dsc.h` and the premultiply part of
//! `src/draw/convert/lv_draw_buf_convert.c`).
//!
//! This is one of the two modules allowed to contain `unsafe`: the only
//! unsafe code is the pair of `u64` <-> byte view helpers
//! ([`words_as_bytes`] / [`words_as_bytes_mut`]) over the aligned owned
//! storage. Every block carries a `// SAFETY:` comment.
//!
//! `lv_draw_buf_handlers_t` is omitted: there is one allocator, so
//! [`DrawBuf::create`] stands in for `lv_draw_buf_create_ex(handlers, ...)`
//! (including `font_draw_buf_handlers` for glyph scratch buffers),
//! [`DrawBuf::dup`] for `dup_ex`, [`DrawBuf::align`] for `align_ex` and
//! [`DrawBuf::width_to_stride`] for `width_to_stride_ex`. The cache hooks
//! `lv_draw_buf_invalidate_cache` / `lv_draw_buf_flush_cache` are no-ops
//! here (no `LV_DRAW_BUF_INVALIDATE_CACHE` hook) and `lv_draw_buf_destroy`
//! is `Drop`.

#![allow(unsafe_code)] // the u64 <-> u8 storage views below (see module docs)

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::draw::LayerId;
use crate::misc::area::Area;
use crate::misc::color::{Color16, Color32, ColorFormat};
use crate::misc::event::LvResult;
use crate::misc::math::round_up;

/// `LV_IMAGE_HEADER_MAGIC`
pub const IMAGE_HEADER_MAGIC: u8 = 0x19;
/// `LV_IMAGE_HEADER_LEGACY`
pub const IMAGE_HEADER_LEGACY: u8 = 0x00;
/// `LV_IMAGE_HEADER_DEADBEEF`
pub const IMAGE_HEADER_DEADBEEF: u8 = 0x1D;
/// `LV_IMAGE_FLAGS_USER_MASK`
pub const IMAGE_FLAGS_USER_MASK: u16 = 0xFF00;
/// `LV_DRAW_BUF_ALIGN` (lv_conf default).
pub const DRAW_BUF_ALIGN: usize = 4;
/// `LV_DRAW_BUF_STRIDE_ALIGN` (lv_conf default).
pub const DRAW_BUF_STRIDE_ALIGN: u32 = 1;
/// `LV_STRIDE_AUTO`
pub const STRIDE_AUTO: u32 = 0;

bitflags::bitflags! {
    /// `lv_image_flags_t`
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct ImageFlags: u16 {
        /// `LV_IMAGE_FLAGS_PREMULTIPLIED`
        const PREMULTIPLIED = 0x0001;
        /// `LV_IMAGE_FLAGS_COMPRESSED`
        const COMPRESSED = 0x0008;
        /// `LV_IMAGE_FLAGS_ALLOCATED`: the data was allocated and must be freed.
        const ALLOCATED = 0x0010;
        /// `LV_IMAGE_FLAGS_MODIFIABLE`
        const MODIFIABLE = 0x0020;
        /// `LV_IMAGE_FLAGS_CUSTOM_DRAW`
        const CUSTOM_DRAW = 0x0040;
        /// `LV_IMAGE_FLAGS_USER1`
        const USER1 = 0x0100;
        /// `LV_IMAGE_FLAGS_USER2`
        const USER2 = 0x0200;
        /// `LV_IMAGE_FLAGS_USER3`
        const USER3 = 0x0400;
        /// `LV_IMAGE_FLAGS_USER4`
        const USER4 = 0x0800;
        /// `LV_IMAGE_FLAGS_USER5`
        const USER5 = 0x1000;
        /// `LV_IMAGE_FLAGS_USER6`
        const USER6 = 0x2000;
        /// `LV_IMAGE_FLAGS_USER7`
        const USER7 = 0x4000;
        /// `LV_IMAGE_FLAGS_USER8`
        const USER8 = 0x8000;
    }
}

/// `lv_image_compress_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ImageCompress {
    /// `LV_IMAGE_COMPRESS_NONE`
    #[default]
    None = 0,
    /// `LV_IMAGE_COMPRESS_RLE`
    Rle = 1,
    /// `LV_IMAGE_COMPRESS_LZ4`
    Lz4 = 2,
}

/// `lv_image_header_t` (little-endian field order).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ImageHeader {
    /// `magic`: [`IMAGE_HEADER_MAGIC`] for a valid header.
    pub magic: u8,
    /// `cf`
    pub cf: ColorFormat,
    /// `flags`
    pub flags: ImageFlags,
    /// `w`
    pub w: u16,
    /// `h`
    pub h: u16,
    /// `stride`: bytes per row, 0 if unknown.
    pub stride: u16,
    /// `reserved_2`
    pub reserved_2: u16,
}

impl ImageHeader {
    /// Byte size of the packed header.
    pub const SIZE: usize = 12;

    /// A header with the given geometry and `magic` set.
    pub const fn new(cf: ColorFormat, w: u16, h: u16, stride: u16) -> Self {
        Self {
            magic: IMAGE_HEADER_MAGIC,
            cf,
            flags: ImageFlags::empty(),
            w,
            h,
            stride,
            reserved_2: 0,
        }
    }

    /// Parse the packed little-endian header of a `.bin` image / C array
    /// (the `lv_image_header_t` bit-field layout: `magic:8 | cf:8 | flags:16`,
    /// `w:16 | h:16`, `stride:16 | reserved_2:16`).
    ///
    /// Returns `None` if fewer than [`ImageHeader::SIZE`] bytes are given or
    /// the colour format byte is not a known `lv_color_format_t` value. The
    /// magic is not checked (the decoders do that).
    pub fn from_bytes(bytes: &[u8]) -> Option<ImageHeader> {
        let b = bytes.get(..Self::SIZE)?;
        let cf = ColorFormat::from_u8(b[1])?;
        Some(ImageHeader {
            magic: b[0],
            cf,
            flags: ImageFlags::from_bits_retain(u16::from_le_bytes([b[2], b[3]])),
            w: u16::from_le_bytes([b[4], b[5]]),
            h: u16::from_le_bytes([b[6], b[7]]),
            stride: u16::from_le_bytes([b[8], b[9]]),
            reserved_2: u16::from_le_bytes([b[10], b[11]]),
        })
    }

    /// Serialise to the packed little-endian layout.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut out = [0u8; Self::SIZE];
        out[0] = self.magic;
        out[1] = self.cf as u8;
        out[2..4].copy_from_slice(&self.flags.bits().to_le_bytes());
        out[4..6].copy_from_slice(&self.w.to_le_bytes());
        out[6..8].copy_from_slice(&self.h.to_le_bytes());
        out[8..10].copy_from_slice(&self.stride.to_le_bytes());
        out[10..12].copy_from_slice(&self.reserved_2.to_le_bytes());
        out
    }
}

/// Backing storage of a [`DrawBuf`].
pub enum BufStorage {
    /// Allocated by the library; 8-byte aligned.
    Owned(Box<[u64]>),
    /// A read-only static buffer (e.g. an image in flash).
    Static(&'static [u8]),
    /// A writable static buffer (e.g. a display buffer in a `static mut`).
    StaticMut(&'static mut [u8]),
    /// A shared read-only buffer.
    Shared(Rc<[u8]>),
}

impl core::fmt::Debug for BufStorage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BufStorage::Owned(b) => write!(f, "Owned({} words)", b.len()),
            BufStorage::Static(b) => write!(f, "Static({} bytes)", b.len()),
            BufStorage::StaticMut(b) => write!(f, "StaticMut({} bytes)", b.len()),
            BufStorage::Shared(b) => write!(f, "Shared({} bytes)", b.len()),
        }
    }
}

/// Byte view of a word slice (the owned [`BufStorage::Owned`] storage).
fn words_as_bytes(words: &[u64]) -> &[u8] {
    // SAFETY: `u8` has alignment 1 and every bit pattern is a valid `u8`, so
    // any initialised `u64` memory may be read as bytes. The pointer and the
    // length (`size_of_val(words)`) describe exactly the memory of `words`,
    // and the returned slice borrows `words`, so it cannot outlive it or
    // alias a mutable borrow.
    unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u8>(), size_of_val(words)) }
}

/// Mutable byte view of a word slice (the owned [`BufStorage::Owned`] storage).
fn words_as_bytes_mut(words: &mut [u64]) -> &mut [u8] {
    // SAFETY: as in `words_as_bytes`: `u8` has alignment 1 and no invalid
    // bit patterns, and every byte pattern written through the view forms a
    // valid `u64`. The pointer/length cover exactly `words`, and the result
    // holds the unique mutable borrow of `words` for its whole lifetime.
    unsafe { core::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u8>(), size_of_val(words)) }
}

/// `_calculate_draw_buf_size()`: bytes needed for a buffer of the given
/// shape, rounded up to [`DRAW_BUF_ALIGN`].
fn calculate_draw_buf_size(w: u32, h: u32, cf: ColorFormat, stride: u32) -> u32 {
    let mut stride = stride;
    if stride == 0 {
        stride = DrawBuf::width_to_stride(w, cf);
    }

    let mut size = stride.wrapping_mul(h);
    if cf == ColorFormat::Rgb565A8 {
        size = size.wrapping_add((stride / 2).wrapping_mul(h)); /*A8 mask*/
    } else if cf.is_indexed() {
        /*@todo we have to include palette right before image data*/
        size = size.wrapping_add(cf.indexed_palette_size() * 4);
    }

    round_up(size, DRAW_BUF_ALIGN as u32)
}

/// `lv_draw_buf_convert_premultiply()` (`src/draw/convert/lv_draw_buf_convert.c`),
/// the generic C path (no `LV_DRAW_CONVERT_PREMULTIPLY_*` accelerators).
fn convert_premultiply(header: &ImageHeader, data: &mut [u8]) -> LvResult {
    /*Premultiply color with alpha, do case by case by judging color format*/
    let cf = header.cf;
    if cf.is_indexed() {
        let size = cf.indexed_palette_size() as usize;
        for px in data[..size * 4].as_chunks_mut::<4>().0.iter_mut() {
            let c = Color32 {
                blue: px[0],
                green: px[1],
                red: px[2],
                alpha: px[3],
            }
            .premultiply();
            px.copy_from_slice(&[c.blue, c.green, c.red, c.alpha]);
        }
    } else if cf == ColorFormat::Argb8888 {
        let h = header.h as usize;
        let w = header.w as usize;
        let stride = header.stride as usize;
        let mut line = 0usize;
        for _y in 0..h {
            for px in data[line..line + w * 4].as_chunks_mut::<4>().0.iter_mut() {
                let c = Color32 {
                    blue: px[0],
                    green: px[1],
                    red: px[2],
                    alpha: px[3],
                }
                .premultiply();
                px.copy_from_slice(&[c.blue, c.green, c.red, c.alpha]);
            }
            line += stride;
        }
    } else if cf == ColorFormat::Rgb565A8 {
        let h = header.h as usize;
        let w = header.w as usize;
        let stride = header.stride as usize;
        let alpha_stride = stride / 2;
        let mut line = 0usize;
        let mut alpha = stride * h;
        for _y in 0..h {
            for x in 0..w {
                let a = data[alpha + x];
                let p = line + x * 2;
                let c = Color16(u16::from_le_bytes([data[p], data[p + 1]])).premultiply(a);
                data[p..p + 2].copy_from_slice(&c.0.to_le_bytes());
            }
            line += stride;
            alpha += alpha_stride;
        }
    } else if cf == ColorFormat::Argb8565 {
        let h = header.h as usize;
        let w = header.w as usize;
        let stride = header.stride as usize;
        let mut line = 0usize;
        for _y in 0..h {
            for px in data[line..line + w * 3].as_chunks_mut::<3>().0.iter_mut() {
                let alpha = px[2];
                let c = Color16(u16::from_le_bytes([px[0], px[1]])).premultiply(alpha);
                px[..2].copy_from_slice(&c.0.to_le_bytes());
            }
            line += stride;
        }
    } else {
        /*LV_LOG_WARN("color format: %d not supported for premultiply", cf)*/
        return LvResult::Invalid;
    }

    LvResult::Ok
}

/// `lv_draw_buf_t`
#[derive(Debug)]
pub struct DrawBuf {
    /// `header`
    pub header: ImageHeader,
    /// `data_size`: usable bytes starting at `data_offset`.
    pub data_size: u32,
    /// `unaligned_data` / `data`: the storage.
    pub storage: BufStorage,
    /// Offset of `data` inside the storage (alignment padding).
    pub data_offset: usize,
}

impl DrawBuf {
    /// `lv_draw_buf_width_to_stride()` (the default `width_to_stride` handler):
    /// `ceil(w * bpp / 8)` rounded up to [`DRAW_BUF_STRIDE_ALIGN`].
    pub fn width_to_stride(w: u32, color_format: ColorFormat) -> u32 {
        let mut width_byte = w.wrapping_mul(color_format.bpp() as u32);
        width_byte = width_byte.wrapping_add(7) >> 3; /*Round up*/

        round_up(width_byte, DRAW_BUF_STRIDE_ALIGN)
    }

    /// `lv_draw_buf_align()` / `buf_align()`: round a byte address (offset)
    /// up to [`DRAW_BUF_ALIGN`] (`LV_ROUND_UP`).
    pub fn align(offset: usize, color_format: ColorFormat) -> usize {
        let _ = color_format;
        #[allow(clippy::manual_div_ceil)] // keep the upstream LV_ROUND_UP expression
        let r = offset.wrapping_add(DRAW_BUF_ALIGN - 1) / DRAW_BUF_ALIGN * DRAW_BUF_ALIGN;
        r
    }

    /// `LV_DRAW_BUF_SIZE(w, h, cf)`: size of a static buffer for
    /// `LV_DRAW_BUF_DEFINE_STATIC` (stride, plus alignment slack and palette).
    pub fn size_for(w: u32, h: u32, cf: ColorFormat) -> u32 {
        let stride = round_up(
            w.wrapping_mul(cf.bpp() as u32).wrapping_add(7) / 8,
            DRAW_BUF_STRIDE_ALIGN,
        );
        stride
            .wrapping_mul(h)
            .wrapping_add(DRAW_BUF_ALIGN as u32)
            .wrapping_add(cf.indexed_palette_size() * 4)
    }

    /// `lv_draw_buf_clear()`: zero `a` (or the whole buffer).
    pub fn clear(&mut self, a: Option<&Area>) {
        self.clear_ex(a, None);
    }

    /// `lv_draw_buf_clear_ex()` (`lv_draw_buf_private.h`): clear `a` of a
    /// layer buffer (`lv_refr.c`, `lv_draw_sw_mask_rect.c`). Without
    /// `buf_clear_cb` handlers this is [`DrawBuf::clear`]; `layer` is kept
    /// for the signature.
    pub fn clear_ex(&mut self, a: Option<&Area>, layer: Option<LayerId>) {
        /* No `buf_clear_cb` handler exists, so the layer path never applies. */
        let _ = layer;

        let header = self.header;
        let stride = header.stride as usize;

        let Some(a) = a else {
            let Some(start) = self.goto_xy_checked(0, 0) else {
                return;
            };
            let len = header.h as usize * stride;
            if let Some(data) = self.data_mut() {
                zero_range(data, start, len);
            }
            /*lv_draw_buf_flush_cache(draw_buf, a): no-op*/
            return;
        };

        let a_draw_buf = Area {
            x1: 0,
            y1: 0,
            x2: header.w as i32 - 1,
            y2: header.h as i32 - 1,
        };

        let Some(a_clipped) = Area::intersect(a, &a_draw_buf) else {
            return;
        };

        if a_clipped.width() <= 0 {
            return;
        }

        if a_clipped.height() <= 0 {
            return;
        }

        let Some(mut buf) = self.goto_xy_checked(a_clipped.x1 as u32, a_clipped.y1 as u32) else {
            return;
        };
        let bpp = header.cf.bpp() as i32;
        let line_length = ((a_clipped.width() * bpp + 7) >> 3) as u32 as usize;
        let Some(data) = self.data_mut() else {
            return;
        };
        for _y in a_clipped.y1..=a_clipped.y2 {
            zero_range(data, buf, line_length);
            buf += stride;
        }
        /*lv_draw_buf_flush_cache(draw_buf, a): no-op*/
    }

    /// `lv_draw_buf_init()`: wrap existing memory.
    ///
    /// Fails with [`LvResult::Invalid`] if `data_size < stride * h`.
    pub fn init(
        w: u32,
        h: u32,
        cf: ColorFormat,
        stride: u32,
        storage: BufStorage,
        data_size: u32,
    ) -> Result<DrawBuf, LvResult> {
        let mut stride = stride;
        if stride == STRIDE_AUTO {
            stride = DrawBuf::width_to_stride(w, cf);
        }

        /*"Data size too small, required: %d, provided: %d"*/
        if data_size < stride.wrapping_mul(h) {
            return Err(LvResult::Invalid);
        }

        let header = ImageHeader {
            magic: IMAGE_HEADER_MAGIC,
            cf,
            flags: ImageFlags::empty(),
            w: w as u16,
            h: h as u16,
            stride: stride as u16,
            reserved_2: 0,
        };

        /* `data == unaligned_data`; an unaligned pointer is only logged
         * ("Data is not aligned, ignored"). */
        Ok(DrawBuf {
            header,
            data_size,
            storage,
            data_offset: 0,
        })
    }

    /// `lv_draw_buf_create()` (and `lv_draw_buf_create_ex()` for any
    /// handler set, e.g. `font_draw_buf_handlers`): allocate a buffer
    /// (`stride` 0 = auto). Returns `None` if the allocation fails.
    ///
    /// Unlike `lv_malloc` the memory is zero-initialised (Rust requires
    /// initialised memory).
    pub fn create(w: u32, h: u32, cf: ColorFormat, stride: u32) -> Option<DrawBuf> {
        let mut stride = stride;
        if stride == STRIDE_AUTO {
            stride = DrawBuf::width_to_stride(w, cf);
        }

        let size = calculate_draw_buf_size(w, h, cf, stride);

        /* buf_malloc(): allocate larger memory to be sure it can be aligned as needed */
        let size_bytes = size as usize + DRAW_BUF_ALIGN - 1;
        let words = size_bytes.div_ceil(size_of::<u64>());
        let mut v: Vec<u64> = Vec::new();
        /*Do not assert here as LVGL or the app might just want to try creating a draw_buf*/
        if v.try_reserve_exact(words).is_err() {
            /*LV_LOG_WARN("No memory: ...")*/
            return None;
        }
        v.resize(words, 0);
        let storage = v.into_boxed_slice();

        /* data = buf_align(unaligned_data) */
        let addr = storage.as_ptr() as usize;
        let data_offset = DrawBuf::align(addr, cf) - addr;

        Some(DrawBuf {
            header: ImageHeader {
                magic: IMAGE_HEADER_MAGIC,
                cf,
                flags: ImageFlags::MODIFIABLE | ImageFlags::ALLOCATED,
                w: w as u16,
                h: h as u16,
                stride: stride as u16,
                reserved_2: 0,
            },
            data_size: size,
            storage: BufStorage::Owned(storage),
            data_offset,
        })
    }

    /// `lv_draw_buf_dup()`: allocate a new buffer of the same shape and copy
    /// the smaller of the two `data_size`s worth of bytes.
    pub fn dup(&self) -> Option<DrawBuf> {
        let header = &self.header;
        let mut new_buf = DrawBuf::create(header.w as u32, header.h as u32, header.cf, header.stride as u32)?;

        new_buf.set_flag(self.header.flags | ImageFlags::MODIFIABLE | ImageFlags::ALLOCATED);

        /*Choose the smaller size to copy*/
        let size = self.data_size.min(new_buf.data_size) as usize;
        if size > 0 {
            /*Copy image data*/
            let src = self.data();
            let size = size.min(src.len());
            if let Some(dst) = new_buf.data_mut() {
                let size = size.min(dst.len());
                dst[..size].copy_from_slice(&src[..size]);
            }
        }

        Some(new_buf)
    }

    /// `lv_draw_buf_reshape()`: reinterpret the buffer with a new geometry
    /// if it fits. Returns `false` (and leaves the buffer unchanged) otherwise.
    /// [`ColorFormat::Unknown`] keeps the current colour format.
    pub fn reshape(&mut self, cf: ColorFormat, w: u32, h: u32, stride: u32) -> bool {
        let mut cf = cf;
        let mut stride = stride;
        /*If color format is unknown, keep using the original color format.*/
        if cf == ColorFormat::Unknown {
            cf = self.header.cf;
        }
        if stride == STRIDE_AUTO {
            stride = DrawBuf::width_to_stride(w, cf);
        }

        let size = calculate_draw_buf_size(w, h, cf, stride);

        if size > self.data_size {
            /*LV_LOG_TRACE("Draw buf too small for new shape")*/
            return false;
        }

        self.header.cf = cf;
        self.header.w = w as u16;
        self.header.h = h as u16;
        self.header.stride = stride as u16;

        true
    }

    /// `lv_draw_buf_copy()` / `buf_copy()`: copy `src_area` of `src` to
    /// `dest_area` of `dest` row by row, respecting both strides. The colour
    /// formats must match (no conversion). `None` areas mean the full buffer;
    /// in that case the palette of indexed formats is copied too.
    pub fn copy(dest: &mut DrawBuf, dest_area: Option<&Area>, src: &DrawBuf, src_area: Option<&Area>) {
        #[allow(clippy::collapsible_if)] // mirrors the LV_CHECK_ARG condition
        if let (Some(sa), Some(da)) = (src_area, dest_area) {
            if !(sa.width() == da.width() && sa.height() == da.height()) {
                return;
            }
        }
        buf_copy(dest, dest_area, src, src_area);
    }

    /// `lv_draw_buf_is_position_valid()`
    pub fn is_position_valid(&self, x: u32, y: u32) -> bool {
        x < self.header.w as u32 && y < self.header.h as u32
    }

    /// `lv_draw_buf_goto_xy()`: byte offset of pixel `(x, y)` in
    /// [`DrawBuf::data`], skipping the palette of indexed formats.
    ///
    /// The offset is computed even when the position is out of range; use
    /// [`DrawBuf::goto_xy_checked`] for the upstream `NULL` return.
    pub fn goto_xy(&self, x: u32, y: u32) -> usize {
        let cf = self.header.cf;
        /*Skip palette*/
        let mut data = cf.indexed_palette_size().wrapping_mul(4);
        data = data.wrapping_add((self.header.stride as u32).wrapping_mul(y));

        if x == 0 {
            return data as usize;
        }

        data.wrapping_add(x.wrapping_mul(cf.bpp() as u32) / 8) as usize
    }

    /// `lv_draw_buf_goto_xy()` with its argument checks: `None` (upstream
    /// `NULL`) if `(x, y)` is outside the buffer.
    pub fn goto_xy_checked(&self, x: u32, y: u32) -> Option<usize> {
        /*"coordinates out of range, x: %d, y: %d, w: %d, h: %d"*/
        if !self.is_position_valid(x, y) {
            return None;
        }
        Some(self.goto_xy(x, y))
    }

    /// `lv_draw_buf_adjust_stride()`: move the rows in place to a new stride
    /// (`0` = the default stride for the width). The buffer must be
    /// modifiable and large enough.
    pub fn adjust_stride(&mut self, stride: u32) -> LvResult {
        let header = self.header;
        let w = header.w as u32;
        let h = header.h as u32;

        if !self.has_flag(ImageFlags::MODIFIABLE) {
            return LvResult::Invalid;
        }

        let mut stride = stride;
        /*Use global stride*/
        if stride == 0 {
            stride = DrawBuf::width_to_stride(w, header.cf);
        }

        /*Check if stride already match*/
        if header.stride as u32 == stride {
            return LvResult::Ok;
        }

        /*Calculate the minimal stride allowed from bpp*/
        let bpp = header.cf.bpp() as u32;
        let min_stride = w.wrapping_mul(bpp).wrapping_add(7) >> 3;
        if stride < min_stride {
            /*LV_LOG_WARN("New stride is too small. min: %d", min_stride)*/
            return LvResult::Invalid;
        }

        /*Check if buffer has enough space. */
        let new_size = calculate_draw_buf_size(w, h, header.cf, stride);
        if new_size > self.data_size {
            return LvResult::Invalid;
        }

        let offset = (header.cf.indexed_palette_size() * 4) as usize;
        let old_stride = header.stride as usize;
        let new_stride = stride as usize;
        let min_stride = min_stride as usize;

        let Some(data) = self.data_mut() else {
            return LvResult::Invalid;
        };

        if new_stride > old_stride {
            /*Copy from the last line to the first*/
            for y in (0..h as usize).rev() {
                let src = offset + old_stride * y;
                let dst = offset + new_stride * y;
                data.copy_within(src..src + min_stride, dst);
            }
        } else {
            /*Copy from the first line to the last*/
            for y in 0..h as usize {
                let src = offset + old_stride * y;
                let dst = offset + new_stride * y;
                data.copy_within(src..src + min_stride, dst);
            }
        }

        self.header.stride = stride as u16;

        LvResult::Ok
    }

    /// `lv_draw_buf_premultiply()`: premultiply the colour channels with the
    /// alpha (the palette for indexed formats). Supported formats: I1..I8,
    /// ARGB8888, RGB565A8 and ARGB8565. Fails if already premultiplied or not
    /// modifiable.
    pub fn premultiply(&mut self) -> LvResult {
        if self.has_flag(ImageFlags::PREMULTIPLIED) {
            return LvResult::Invalid;
        }
        if !self.has_flag(ImageFlags::MODIFIABLE) {
            return LvResult::Invalid;
        }

        let header = self.header;
        let Some(data) = self.data_mut() else {
            return LvResult::Invalid;
        };
        let res = convert_premultiply(&header, data);
        if res == LvResult::Ok {
            /* The flushed area (only the palette rows for indexed formats)
             * matters only for the no-op cache flush. */
            self.set_flag(ImageFlags::PREMULTIPLIED);
            /*lv_draw_buf_flush_cache(draw_buf, &area): no-op*/
        }

        res
    }

    /// `lv_draw_buf_set_palette()`: write palette entry `index` (BGRA at the
    /// start of the data). Ignored for non-indexed formats or read-only data.
    pub fn set_palette(&mut self, index: u8, color: Color32) {
        if !self.header.cf.is_indexed() {
            /*"Color format is not indexed"*/
            return;
        }
        let Some(data) = self.data_mut() else {
            return;
        };
        let p = index as usize * 4;
        if let Some(px) = data.get_mut(p..p + 4) {
            px.copy_from_slice(&[color.blue, color.green, color.red, color.alpha]);
        }
    }

    /// `lv_draw_buf_from_image()`: wrap the static pixels of `img`
    /// (keeping its flags).
    pub fn from_image(img: &ImageDsc) -> Result<DrawBuf, LvResult> {
        let mut buf = DrawBuf::init(
            img.header.w as u32,
            img.header.h as u32,
            img.header.cf,
            img.header.stride as u32,
            BufStorage::Static(img.data),
            img.data_size,
        )?;

        buf.header.flags = img.header.flags;
        Ok(buf)
    }

    /// `lv_draw_buf_to_image()`: an image descriptor sharing the header,
    /// `data_size` and `data` of the buffer.
    ///
    /// [`ImageDsc::data`] is `&'static`, so only [`BufStorage::Static`] data
    /// can be shared; for any other storage the descriptor carries the header
    /// and `data_size` but an empty `data` slice.
    pub fn to_image(&self) -> ImageDsc {
        let data: &'static [u8] = match &self.storage {
            BufStorage::Static(s) => {
                let s: &'static [u8] = s;
                let start = self.data_offset.min(s.len());
                let end = (self.data_offset + self.data_size as usize).min(s.len());
                &s[start..end]
            }
            _ => &[],
        };
        ImageDsc {
            header: self.header,
            data_size: self.data_size,
            data,
        }
    }

    /// `lv_draw_buf_has_flag()`: `true` if any bit of `flag` is set.
    #[inline]
    pub fn has_flag(&self, flag: ImageFlags) -> bool {
        self.header.flags.intersects(flag)
    }

    /// `lv_draw_buf_set_flag()`
    #[inline]
    pub fn set_flag(&mut self, flag: ImageFlags) {
        self.header.flags.insert(flag);
    }

    /// `lv_draw_buf_clear_flag()`
    #[inline]
    pub fn clear_flag(&mut self, flag: ImageFlags) {
        self.header.flags.remove(flag);
    }

    /// `data`: the pixel bytes (read), `data_size` bytes from `data_offset`
    /// (clamped to the storage).
    pub fn data(&self) -> &[u8] {
        let bytes: &[u8] = match &self.storage {
            BufStorage::Owned(w) => words_as_bytes(w),
            BufStorage::Static(s) => s,
            BufStorage::StaticMut(s) => s,
            BufStorage::Shared(s) => s,
        };
        let start = self.data_offset.min(bytes.len());
        let end = (self.data_offset + self.data_size as usize).min(bytes.len());
        &bytes[start..end]
    }

    /// `data`: the pixel bytes (write); `None` for read-only storage
    /// ([`BufStorage::Static`] and [`BufStorage::Shared`]).
    pub fn data_mut(&mut self) -> Option<&mut [u8]> {
        let bytes: &mut [u8] = match &mut self.storage {
            BufStorage::Owned(w) => words_as_bytes_mut(w),
            BufStorage::StaticMut(s) => s,
            BufStorage::Static(_) | BufStorage::Shared(_) => return None,
        };
        let start = self.data_offset.min(bytes.len());
        let end = (self.data_offset + self.data_size as usize).min(bytes.len());
        Some(&mut bytes[start..end])
    }
}

/// `lv_memzero(buf + start, len)`, clamped to the slice.
fn zero_range(data: &mut [u8], start: usize, len: usize) {
    let start = start.min(data.len());
    let end = start.saturating_add(len).min(data.len());
    data[start..end].fill(0);
}

/// `buf_copy()`: the default `buf_copy_cb` handler.
fn buf_copy(dest: &mut DrawBuf, dest_area: Option<&Area>, src: &DrawBuf, src_area: Option<&Area>) {
    /*Source and dest color format must be same. Color conversion is not supported yet.*/
    debug_assert_eq!(dest.header.cf, src.header.cf, "Color format mismatch");

    let line_width: i32 = match dest_area {
        None => dest.header.w as i32,
        Some(a) => a.width(),
    };

    let dest_cf = dest.header.cf;
    let dest_h = dest.header.h as i32;
    let dest_stride = dest.header.stride as usize;
    let src_stride = src.header.stride as usize;

    /* For indexed image, copy the palette if we are copying full image area*/
    #[allow(clippy::collapsible_if)] // keep the upstream nesting
    if dest_area.is_none() || src_area.is_none() {
        if dest_cf.is_indexed() {
            let n = (dest_cf.indexed_palette_size() * 4) as usize;
            let s = src.data();
            if let Some(d) = dest.data_mut() {
                let n = n.min(s.len()).min(d.len());
                d[..n].copy_from_slice(&s[..n]);
            }
        }
    }

    /*Check source and dest area have same width*/
    if (src_area.is_none() && line_width != src.header.w as i32)
        || src_area.is_some_and(|a| line_width != a.width())
    {
        debug_assert!(false, "Source and destination areas have different width");
        return;
    }

    let src_bufc = match src_area {
        Some(a) => src.goto_xy_checked(a.x1 as u32, a.y1 as u32),
        None => src.goto_xy_checked(0, 0),
    };

    let dest_bufc = match dest_area {
        Some(a) => dest.goto_xy_checked(a.x1 as u32, a.y1 as u32),
        None => dest.goto_xy_checked(0, 0),
    };

    let (Some(mut src_bufc), Some(mut dest_bufc)) = (src_bufc, dest_bufc) else {
        return;
    };

    let (mut start_y, end_y) = match dest_area {
        Some(a) => (a.y1, a.y2),
        None => (0, dest_h - 1),
    };

    let line_bytes = ((line_width as u32)
        .wrapping_mul(dest_cf.bpp() as u32)
        .wrapping_add(7)
        >> 3) as usize;

    let s = src.data();
    let Some(d) = dest.data_mut() else {
        return;
    };
    while start_y <= end_y {
        d[dest_bufc..dest_bufc + line_bytes].copy_from_slice(&s[src_bufc..src_bufc + line_bytes]);
        dest_bufc += dest_stride;
        src_bufc += src_stride;
        start_y += 1;
    }
}

/// `lv_image_dsc_t`: an image whose pixels live in static memory
/// (`LV_IMAGE_DECLARE` C arrays, `.bin` files embedded with `include_bytes!`).
/// Images created at runtime are [`DrawBuf`]s.
#[derive(Clone, Debug)]
pub struct ImageDsc {
    /// `header`
    pub header: ImageHeader,
    /// `data_size`
    pub data_size: u32,
    /// `data`
    pub data: &'static [u8],
}

impl ImageDsc {
    /// Build an image descriptor (usable in `static` initialisers).
    pub const fn new(header: ImageHeader, data: &'static [u8]) -> Self {
        Self {
            header,
            data_size: data.len() as u32,
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::misc::color::Color;
    use alloc::vec;

    fn px32(d: &[u8], i: usize) -> Color32 {
        Color32 {
            blue: d[i * 4],
            green: d[i * 4 + 1],
            red: d[i * 4 + 2],
            alpha: d[i * 4 + 3],
        }
    }

    #[test]
    fn flags_and_magic() {
        assert_eq!(IMAGE_HEADER_MAGIC, 0x19);
        assert_eq!(ImageFlags::MODIFIABLE.bits(), 0x20);
        assert_eq!(ImageFlags::USER8.bits(), 0x8000);
        let h = ImageHeader::new(ColorFormat::Argb8888, 10, 20, 40);
        assert_eq!(h.magic, IMAGE_HEADER_MAGIC);
        assert_eq!(h.w, 10);
    }

    #[test]
    fn header_bytes_roundtrip() {
        let mut h = ImageHeader::new(ColorFormat::Rgb565, 300, 2, 600);
        h.flags = ImageFlags::PREMULTIPLIED | ImageFlags::USER1;
        let b = h.to_bytes();
        assert_eq!(
            b,
            [
                0x19,
                ColorFormat::Rgb565 as u8,
                0x01,
                0x01,
                0x2C,
                0x01,
                2,
                0,
                0x58,
                0x02,
                0,
                0
            ]
        );
        assert_eq!(ImageHeader::from_bytes(&b), Some(h));
        assert_eq!(ImageHeader::from_bytes(&b[..11]), None);
    }

    #[test]
    fn stride_and_size() {
        assert_eq!(DrawBuf::width_to_stride(5, ColorFormat::I1), 1);
        assert_eq!(DrawBuf::width_to_stride(9, ColorFormat::I1), 2);
        assert_eq!(DrawBuf::width_to_stride(3, ColorFormat::I4), 2);
        assert_eq!(DrawBuf::width_to_stride(7, ColorFormat::Rgb565), 14);
        assert_eq!(DrawBuf::width_to_stride(3, ColorFormat::Rgb888), 9);
        assert_eq!(DrawBuf::width_to_stride(10, ColorFormat::Argb8888), 40);
        assert_eq!(DrawBuf::size_for(100, 100, ColorFormat::Rgb565), 200 * 100 + 4);
        assert_eq!(DrawBuf::size_for(2, 2, ColorFormat::I2), 2 + 4 + 16);
        assert_eq!(DrawBuf::align(5, ColorFormat::Rgb565), 8);
        assert_eq!(DrawBuf::align(8, ColorFormat::Rgb565), 8);
    }

    /// Port of `test_draw_buf.c: test_draw_buf_size_alignment`.
    #[test]
    fn size_alignment() {
        let cases = [
            (5, 1, ColorFormat::I1, 1),
            (1, 3, ColorFormat::A8, 1),
            (3, 1, ColorFormat::Rgb565, 6),
            (3, 1, ColorFormat::Rgb888, 9),
            (2, 1, ColorFormat::Rgb565A8, 4),
            (10, 10, ColorFormat::Argb8888, 40),
            (7, 5, ColorFormat::Rgb565, 0),
        ];
        for (w, h, cf, stride) in cases {
            let buf = DrawBuf::create(w, h, cf, stride).unwrap();
            let effective_stride = if stride != 0 {
                stride
            } else {
                DrawBuf::width_to_stride(w, cf)
            };
            let mut raw_size = effective_stride * h;
            if cf == ColorFormat::Rgb565A8 {
                raw_size += (effective_stride / 2) * h;
            } else if cf.is_indexed() {
                raw_size += cf.indexed_palette_size() * 4;
            }
            let expected = round_up(raw_size, DRAW_BUF_ALIGN as u32);
            assert_eq!(buf.data_size % DRAW_BUF_ALIGN as u32, 0);
            assert!(buf.data_size >= raw_size);
            assert_eq!(buf.data_size, expected);
            assert_eq!(buf.data().len(), expected as usize);
            assert_eq!(buf.header.flags, ImageFlags::MODIFIABLE | ImageFlags::ALLOCATED);
            assert_eq!(buf.header.stride as u32, effective_stride);
        }
    }

    /// Port of `test_draw_buf.c: test_draw_buf_xy_access`
    /// (`LV_DRAW_BUF_DEFINE_STATIC` + `LV_DRAW_BUF_INIT_STATIC`).
    #[test]
    fn xy_access() {
        let cf = ColorFormat::Rgb565;
        let size = DrawBuf::size_for(100, 100, cf);
        let mem: &'static mut [u8] = Box::leak(vec![0u8; size as usize].into_boxed_slice());
        let stride = DrawBuf::width_to_stride(100, cf);
        let mut buf = DrawBuf::init(100, 100, cf, stride, BufStorage::StaticMut(mem), size).unwrap();
        buf.set_flag(ImageFlags::MODIFIABLE);
        assert_eq!(buf.goto_xy_checked(50, 50), Some(50 * 200 + 100));
        assert_eq!(buf.goto_xy_checked(100, 0), None);
        assert!(buf.data_mut().is_some());
    }

    #[test]
    fn init_checks_size() {
        let mem: &'static [u8] = &[0u8; 16];
        assert!(DrawBuf::init(4, 2, ColorFormat::Argb8888, 0, BufStorage::Static(mem), 16).is_err());
        let b = DrawBuf::init(2, 2, ColorFormat::Argb8888, 0, BufStorage::Static(mem), 16).unwrap();
        assert_eq!(b.header.stride, 8);
        assert!(b.header.flags.is_empty());
        let mut b = b;
        assert!(b.data_mut().is_none());
        assert_eq!(b.adjust_stride(12), LvResult::Invalid);
    }

    #[test]
    fn goto_xy_skips_palette() {
        let b = DrawBuf::create(16, 4, ColorFormat::I4, 0).unwrap();
        assert_eq!(b.header.stride, 8);
        assert_eq!(b.goto_xy(0, 0), 64);
        assert_eq!(b.goto_xy(3, 2), 64 + 16 + 1);
        let b = DrawBuf::create(16, 4, ColorFormat::Rgb888, 0).unwrap();
        assert_eq!(b.goto_xy(3, 1), 48 + 9);
    }

    /// Port of `test_draw_buf.c: test_draw_buf_premultiply`.
    #[test]
    fn premultiply_formats() {
        /* ARGB8888 */
        let mut b = DrawBuf::create(2, 2, ColorFormat::Argb8888, 2 * 4).unwrap();
        for px in b.data_mut().unwrap()[..16].as_chunks_mut::<4>().0.iter_mut() {
            px.copy_from_slice(&[255, 255, 255, 128]);
        }
        assert_eq!(b.premultiply(), LvResult::Ok);
        assert!(b.has_flag(ImageFlags::PREMULTIPLIED));
        for i in 0..4 {
            assert_eq!(px32(b.data(), i), Color32::make(127, 127, 127, 128));
        }
        /* Already premultiplied */
        assert_eq!(b.premultiply(), LvResult::Invalid);

        /* XRGB8888 */
        let mut b = DrawBuf::create(2, 2, ColorFormat::Xrgb8888, 2 * 4).unwrap();
        b.data_mut().unwrap().fill(255);
        assert_eq!(b.premultiply(), LvResult::Invalid);
        assert!(!b.has_flag(ImageFlags::PREMULTIPLIED));

        /* RGB565A8 */
        let mut b = DrawBuf::create(2, 2, ColorFormat::Rgb565A8, 2 * 2).unwrap();
        {
            let d = b.data_mut().unwrap();
            let white = Color::white().to_u16().to_le_bytes();
            for i in 0..4 {
                d[i * 2..i * 2 + 2].copy_from_slice(&white);
                d[8 + i] = 128;
            }
        }
        assert_eq!(b.premultiply(), LvResult::Ok);
        assert!(b.has_flag(ImageFlags::PREMULTIPLIED));
        for i in 0..4 {
            let d = b.data();
            assert_eq!(u16::from_le_bytes([d[i * 2], d[i * 2 + 1]]), 0x7BEF);
        }

        /* ARGB8565 */
        let mut b = DrawBuf::create(2, 2, ColorFormat::Argb8565, 2 * 3).unwrap();
        {
            let d = b.data_mut().unwrap();
            let c = Color::white().to_u16();
            for i in 0..4 {
                d[i * 3] = (c & 0xFF) as u8;
                d[i * 3 + 1] = (c >> 8) as u8;
                d[i * 3 + 2] = 128;
            }
        }
        assert_eq!(b.premultiply(), LvResult::Ok);
        assert!(b.has_flag(ImageFlags::PREMULTIPLIED));
        let d = b.data();
        for i in 0..4 {
            assert_eq!(u16::from_le_bytes([d[i * 3], d[i * 3 + 1]]), 0x7BEF);
            assert_eq!(d[i * 3 + 2], 128);
        }

        /* Indexed */
        for cf in [ColorFormat::I1, ColorFormat::I2, ColorFormat::I4, ColorFormat::I8] {
            let mut b = DrawBuf::create(2, 2, cf, 0).unwrap();
            let n = cf.indexed_palette_size() as u8;
            for i in 0..n as u16 {
                b.set_palette(i as u8, Color32::make(255, 255, 255, 128));
            }
            assert_eq!(b.premultiply(), LvResult::Ok);
            assert!(b.has_flag(ImageFlags::PREMULTIPLIED));
            for i in 0..n as usize {
                assert_eq!(px32(b.data(), i), Color32::make(127, 127, 127, 128));
            }
        }

        /* Not modifiable */
        let mem: &'static mut [u8] = Box::leak(vec![0u8; 16].into_boxed_slice());
        let mut b = DrawBuf::init(2, 2, ColorFormat::Argb8888, 0, BufStorage::StaticMut(mem), 16).unwrap();
        assert_eq!(b.premultiply(), LvResult::Invalid);
    }

    fn fill_pattern(b: &mut DrawBuf) {
        let min_stride = ((b.header.w as u32 * b.header.cf.bpp() as u32 + 7) >> 3) as usize;
        let stride = b.header.stride as usize;
        let h = b.header.h as usize;
        let off = b.goto_xy(0, 0);
        let d = b.data_mut().unwrap();
        for (i, v) in d[..off].iter_mut().enumerate() {
            *v = (i * 7 + 3) as u8;
        }
        for y in 0..h {
            for x in 0..min_stride {
                d[off + y * stride + x] = (y * 31 + x * 5 + 1) as u8;
            }
        }
    }

    fn rows_equal(a: &DrawBuf, b: &DrawBuf) -> bool {
        let min_stride = ((a.header.w as u32 * a.header.cf.bpp() as u32 + 7) >> 3) as usize;
        let off = a.goto_xy(0, 0);
        if a.data()[..off] != b.data()[..off] {
            return false;
        }
        (0..a.header.h as u32).all(|y| {
            let pa = a.goto_xy(0, y);
            let pb = b.goto_xy(0, y);
            a.data()[pa..pa + min_stride] == b.data()[pb..pb + min_stride]
        })
    }

    /// Adapted from `test_draw_buf.c: test_draw_buf_stride_adjust`, with
    /// synthetic pixel data instead of decoded `.bin` images and row
    /// comparisons instead of screenshots.
    #[test]
    fn stride_adjust() {
        let cfs = [
            ColorFormat::I1,
            ColorFormat::I2,
            ColorFormat::I4,
            ColorFormat::I8,
            ColorFormat::A8,
            ColorFormat::Rgb565,
            ColorFormat::Rgb888,
            ColorFormat::Argb8888,
            ColorFormat::Xrgb8888,
        ];
        for cf in cfs {
            let (w, h) = (13u32, 7u32);
            let mut src = DrawBuf::create(w, h, cf, 0).unwrap();
            fill_pattern(&mut src);
            let mut decoded = src.dup().unwrap();
            assert!(rows_equal(&src, &decoded));

            let image_stride = decoded.header.stride as u32;
            let min_stride = (w * cf.bpp() as u32 + 7) >> 3;

            assert_eq!(decoded.adjust_stride(min_stride - 1), LvResult::Invalid);
            assert_eq!(decoded.adjust_stride(image_stride * 2), LvResult::Invalid);
            assert_eq!(decoded.adjust_stride(0), LvResult::Ok);

            let mut larger = DrawBuf::create(w, h, cf, min_stride + 100).unwrap();
            DrawBuf::copy(&mut larger, None, &decoded, None);
            assert!(rows_equal(&src, &larger));

            assert_eq!(larger.adjust_stride(min_stride), LvResult::Ok);
            assert_eq!(larger.header.stride as u32, min_stride);
            assert!(rows_equal(&src, &larger));

            assert_eq!(larger.adjust_stride(min_stride + 20), LvResult::Ok);
            assert_eq!(larger.header.stride as u32, min_stride + 20);
            assert!(rows_equal(&src, &larger));
        }
    }

    #[test]
    fn copy_areas() {
        let mut src = DrawBuf::create(8, 8, ColorFormat::Argb8888, 0).unwrap();
        fill_pattern(&mut src);
        let mut dest = DrawBuf::create(10, 10, ColorFormat::Argb8888, 0).unwrap();
        let sa = Area::new(2, 3, 4, 5);
        let da = Area::new(6, 1, 8, 3);
        DrawBuf::copy(&mut dest, Some(&da), &src, Some(&sa));
        for y in 0..3u32 {
            let s = src.goto_xy(2, 3 + y);
            let d = dest.goto_xy(6, 1 + y);
            assert_eq!(dest.data()[d..d + 12], src.data()[s..s + 12]);
            assert!(dest.data()[d + 12..d + 16].iter().all(|&v| v == 0));
        }
        assert!(dest.data()[..dest.goto_xy(0, 1)].iter().all(|&v| v == 0));

        /* Mismatched sizes: no-op */
        let mut dest2 = DrawBuf::create(10, 10, ColorFormat::Argb8888, 0).unwrap();
        DrawBuf::copy(&mut dest2, Some(&Area::new(0, 0, 3, 3)), &src, Some(&sa));
        assert!(dest2.data().iter().all(|&v| v == 0));
    }

    #[test]
    fn clear_area_and_full() {
        let mut b = DrawBuf::create(4, 4, ColorFormat::Rgb565, 10).unwrap();
        b.data_mut().unwrap().fill(0xAA);
        b.clear(Some(&Area::new(-5, 1, 1, 2)));
        for y in 0..4u32 {
            let p = b.goto_xy(0, y);
            let row = &b.data()[p..p + 10];
            if y == 1 || y == 2 {
                assert!(row[..4].iter().all(|&v| v == 0));
                assert!(row[4..].iter().all(|&v| v == 0xAA));
            } else {
                assert!(row.iter().all(|&v| v == 0xAA));
            }
        }
        /* Fully outside: no-op */
        b.clear(Some(&Area::new(10, 10, 20, 20)));
        b.clear(None);
        assert!(b.data()[..40].iter().all(|&v| v == 0));

        /* Indexed: the palette is kept */
        let mut b = DrawBuf::create(8, 2, ColorFormat::I1, 0).unwrap();
        b.data_mut().unwrap().fill(0xFF);
        b.clear_ex(None, None);
        assert!(b.data()[..8].iter().all(|&v| v == 0xFF));
        assert!(b.data()[8..10].iter().all(|&v| v == 0));
    }

    #[test]
    fn reshape_and_flags() {
        let mut b = DrawBuf::create(10, 10, ColorFormat::Argb8888, 0).unwrap();
        assert!(b.reshape(ColorFormat::Unknown, 20, 5, 0));
        assert_eq!(
            (b.header.cf, b.header.w, b.header.h, b.header.stride),
            (ColorFormat::Argb8888, 20, 5, 80)
        );
        assert!(!b.reshape(ColorFormat::Argb8888, 20, 6, 0));
        assert_eq!(b.header.h, 5);
        assert!(b.reshape(ColorFormat::Rgb565, 10, 10, 0));
        assert_eq!(b.header.stride, 20);

        b.set_flag(ImageFlags::USER1);
        let d = b.dup().unwrap();
        assert!(d.has_flag(ImageFlags::USER1 | ImageFlags::ALLOCATED));
        b.clear_flag(ImageFlags::USER1);
        assert!(!b.has_flag(ImageFlags::USER1));
    }

    #[test]
    fn image_roundtrip() {
        static PIXELS: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let mut header = ImageHeader::new(ColorFormat::Argb8888, 2, 2, 8);
        header.flags = ImageFlags::PREMULTIPLIED;
        let img = ImageDsc::new(header, &PIXELS);
        let b = DrawBuf::from_image(&img).unwrap();
        assert_eq!(b.header.flags, ImageFlags::PREMULTIPLIED);
        assert_eq!(b.data(), &PIXELS[..]);
        let back = b.to_image();
        assert_eq!(back.header, b.header);
        assert_eq!(back.data, &PIXELS[..]);

        let small = ImageDsc {
            header,
            data_size: 8,
            data: &PIXELS,
        };
        assert!(DrawBuf::from_image(&small).is_err());
    }

    #[test]
    fn set_palette_bgra() {
        let mut b = DrawBuf::create(4, 1, ColorFormat::I2, 0).unwrap();
        b.set_palette(3, Color32::make(1, 2, 3, 4));
        assert_eq!(&b.data()[12..16], &[3, 2, 1, 4]);
        let mut b = DrawBuf::create(4, 1, ColorFormat::Rgb565, 0).unwrap();
        b.set_palette(0, Color32::make(1, 2, 3, 4));
        assert!(b.data().iter().all(|&v| v == 0));
    }
}
