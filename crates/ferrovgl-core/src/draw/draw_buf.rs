//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_buf.h`, `src/draw/lv_draw_buf_private.h`,
//! `include/lvgl/draw/lv_image_dsc.h`, `src/draw/lv_draw_buf.c`.
//!
//! This is one of the two modules allowed to contain `unsafe` (for the
//! `u64` <-> byte views of aligned owned buffers). The skeleton has none yet;
//! when added, the module gets `#![allow(unsafe_code)]` and every block a
//! `// SAFETY:` comment.
//!
//! `lv_draw_buf_handlers_t` is omitted: there is one allocator, so
//! [`DrawBuf::create`] stands in for `lv_draw_buf_create_ex(handlers, ...)`
//! (including `font_draw_buf_handlers` for glyph scratch buffers),
//! [`DrawBuf::dup`] for `dup_ex`, [`DrawBuf::align`] for `align_ex` and
//! [`DrawBuf::width_to_stride`] for `width_to_stride_ex`. The cache hooks
//! `lv_draw_buf_invalidate_cache` / `lv_draw_buf_flush_cache` are no-ops
//! here (no `LV_DRAW_BUF_INVALIDATE_CACHE` hook) and `lv_draw_buf_destroy`
//! is `Drop`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::boxed::Box;
use alloc::rc::Rc;

use crate::draw::LayerId;
use crate::misc::area::Area;
use crate::misc::color::{Color32, ColorFormat};
use crate::misc::event::LvResult;

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

    /// Parse the packed little-endian header of a `.bin` image / C array.
    pub fn from_bytes(bytes: &[u8]) -> Option<ImageHeader> {
        todo!()
    }

    /// Serialise to the packed little-endian layout.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        todo!()
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
    /// `lv_draw_buf_width_to_stride()`
    pub fn width_to_stride(w: u32, color_format: ColorFormat) -> u32 {
        todo!()
    }

    /// `LV_DRAW_BUF_SIZE(w, h, cf)`
    pub fn size_for(w: u32, h: u32, cf: ColorFormat) -> u32 {
        todo!()
    }

    /// `lv_draw_buf_create()` (and `lv_draw_buf_create_ex()` for any
    /// handler set, e.g. `font_draw_buf_handlers`): allocate a buffer
    /// (`stride` 0 = auto).
    pub fn create(w: u32, h: u32, cf: ColorFormat, stride: u32) -> Option<DrawBuf> {
        todo!()
    }

    /// `lv_draw_buf_init()`: wrap existing memory.
    pub fn init(
        w: u32,
        h: u32,
        cf: ColorFormat,
        stride: u32,
        storage: BufStorage,
        data_size: u32,
    ) -> Result<DrawBuf, LvResult> {
        todo!()
    }

    /// `lv_draw_buf_dup()`
    pub fn dup(&self) -> Option<DrawBuf> {
        todo!()
    }

    /// `lv_draw_buf_reshape()`: reinterpret the buffer with a new geometry
    /// if it fits. Returns `false` (and leaves the buffer unchanged) otherwise.
    pub fn reshape(&mut self, cf: ColorFormat, w: u32, h: u32, stride: u32) -> bool {
        todo!()
    }

    /// `lv_draw_buf_clear()`: zero `a` (or the whole buffer).
    pub fn clear(&mut self, a: Option<&Area>) {
        todo!()
    }

    /// `lv_draw_buf_clear_ex()` (`lv_draw_buf_private.h`): clear `a` of a
    /// layer buffer (`lv_refr.c`, `lv_draw_sw_mask_rect.c`). Without
    /// `buf_clear_cb` handlers this is [`DrawBuf::clear`]; `layer` is kept
    /// for the signature.
    pub fn clear_ex(&mut self, a: Option<&Area>, layer: Option<LayerId>) {
        todo!()
    }

    /// `lv_draw_buf_copy()`
    pub fn copy(dest: &mut DrawBuf, dest_area: Option<&Area>, src: &DrawBuf, src_area: Option<&Area>) {
        todo!()
    }

    /// `lv_draw_buf_goto_xy()`: byte offset of pixel `(x, y)` in [`DrawBuf::data`].
    pub fn goto_xy(&self, x: u32, y: u32) -> usize {
        todo!()
    }

    /// `lv_draw_buf_is_position_valid()`
    pub fn is_position_valid(&self, x: u32, y: u32) -> bool {
        todo!()
    }

    /// `lv_draw_buf_adjust_stride()`
    pub fn adjust_stride(&mut self, stride: u32) -> LvResult {
        todo!()
    }

    /// `lv_draw_buf_premultiply()`
    pub fn premultiply(&mut self) -> LvResult {
        todo!()
    }

    /// `lv_draw_buf_has_flag()`
    #[inline]
    pub fn has_flag(&self, flag: ImageFlags) -> bool {
        self.header.flags.contains(flag)
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

    /// `lv_draw_buf_from_image()`
    pub fn from_image(img: &ImageDsc) -> Result<DrawBuf, LvResult> {
        todo!()
    }

    /// `lv_draw_buf_to_image()`
    pub fn to_image(&self) -> ImageDsc {
        todo!()
    }

    /// `lv_draw_buf_set_palette()`
    pub fn set_palette(&mut self, index: u8, color: Color32) {
        todo!()
    }

    /// `lv_draw_buf_align()`: round a byte offset up to [`DRAW_BUF_ALIGN`].
    pub fn align(offset: usize, color_format: ColorFormat) -> usize {
        todo!()
    }

    /// `data`: the pixel bytes (read).
    pub fn data(&self) -> &[u8] {
        todo!()
    }

    /// `data`: the pixel bytes (write); `None` for read-only storage.
    pub fn data_mut(&mut self) -> Option<&mut [u8]> {
        todo!()
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

    #[test]
    fn flags_and_magic() {
        assert_eq!(IMAGE_HEADER_MAGIC, 0x19);
        assert_eq!(ImageFlags::MODIFIABLE.bits(), 0x20);
        assert_eq!(ImageFlags::USER8.bits(), 0x8000);
        let h = ImageHeader::new(ColorFormat::Argb8888, 10, 20, 40);
        assert_eq!(h.magic, IMAGE_HEADER_MAGIC);
        assert_eq!(h.w, 10);
    }
}
