//! Reader for the `.lvfnt` table format and port of LVGL's built-in font
//! engine.
//!
//! Ported from lvgl v9.6.0: `src/font/fmt_txt/lv_font_fmt_txt.c`,
//! `include/lvgl/font/lv_font_fmt_txt.h`.
//!
//! The `.lvfnt` blob is the data of a `lv_font_fmt_txt_dsc_t` serialised as
//! little-endian tables (see the `font-conv` tool for the writer). The reader
//! never copies the tables: every lookup indexes the byte slice, so a font in
//! `.rodata` costs no RAM, like the C arrays it was converted from.
//!
//! ```text
//! 0   magic "LVFNT\0", version u16
//! 8   line_height i32, base_line i32, cap_height i32, x_height i32
//! 24  subpx u8, underline_position i8, underline_thickness i8, bpp u8,
//!     bitmap_format u8, stride u8, kern_kind u8, pad u8
//! 32  kern_scale u16, pad u16, glyph_count u32, cmap_count u32
//! 44  glyph_dsc_off, cmaps_off, kern_off, bitmap_off, bitmap_len (u32 each)
//! 64  tables
//! ```

use alloc::vec;
use alloc::vec::Vec;

use super::{Font, GlyphDsc, GlyphFormat, Subpx};
use crate::misc::math::round_up;

const MAGIC: &[u8; 6] = b"LVFNT\0";
const VERSION: u16 = 1;
const HEADER_LEN: usize = 64;
const GLYPH_LEN: usize = 16;
const CMAP_LEN: usize = 20;

const KERN_NONE: u8 = 0;
const KERN_PAIRS: u8 = 1;
const KERN_CLASSES: u8 = 2;

/// `lv_font_fmt_txt_cmap_type_t`
const CMAP_FORMAT0_FULL: u8 = 0;
const CMAP_SPARSE_FULL: u8 = 1;
const CMAP_FORMAT0_TINY: u8 = 2;
const CMAP_SPARSE_TINY: u8 = 3;

/// `lv_font_fmt_txt_bitmap_format_t`
const BITMAP_PLAIN: u8 = 0;
const BITMAP_COMPRESSED: u8 = 1;

static OPA4_TABLE: [u8; 16] = [
    0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255,
];
static OPA3_TABLE: [u8; 8] = [0, 36, 73, 109, 146, 182, 218, 255];
static OPA2_TABLE: [u8; 4] = [0, 85, 170, 255];

const fn rd_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
const fn rd_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
const fn rd_i32(b: &[u8], at: usize) -> i32 {
    rd_u32(b, at) as i32
}

/// A bitmap font stored as `.lvfnt` tables (the Rust form of
/// `lv_font_t` + `lv_font_fmt_txt_dsc_t`).
#[derive(Clone, Copy)]
pub struct FmtTxtFont<'a> {
    bytes: &'a [u8],
    line_height: i32,
    base_line: i32,
    cap_height: i32,
    x_height: i32,
    subpx: Subpx,
    underline_position: i8,
    underline_thickness: i8,
    bpp: u8,
    bitmap_format: u8,
    stride: u8,
    kern_kind: u8,
    kern_scale: u16,
    glyph_count: u32,
    cmap_count: u32,
    glyph_dsc_off: usize,
    cmaps_off: usize,
    kern_off: usize,
    bitmap_off: usize,
    bitmap_len: usize,
    fallback: Option<&'a (dyn Font + Sync)>,
}

impl core::fmt::Debug for FmtTxtFont<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FmtTxtFont")
            .field("line_height", &self.line_height)
            .field("base_line", &self.base_line)
            .field("bpp", &self.bpp)
            .field("compressed", &self.is_compressed())
            .field("glyph_count", &self.glyph_count)
            .field("cmap_count", &self.cmap_count)
            .field("bytes", &self.bytes.len())
            .field("has_fallback", &self.fallback.is_some())
            .finish()
    }
}

/// Errors reported by [`FmtTxtFont::from_bytes`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FontError {
    /// The blob does not start with the `.lvfnt` magic.
    BadMagic,
    /// Unsupported format version.
    BadVersion(u16),
    /// A table lies outside the blob.
    Truncated,
}

impl core::fmt::Display for FontError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FontError::BadMagic => f.write_str("not a .lvfnt font blob"),
            FontError::BadVersion(v) => write!(f, "unsupported .lvfnt version {v}"),
            FontError::Truncated => f.write_str("truncated .lvfnt font blob"),
        }
    }
}

impl core::error::Error for FontError {}

impl<'a> FmtTxtFont<'a> {
    /// Parse a `.lvfnt` blob in a `const` context. Panics on an invalid blob,
    /// which turns into a compile error when used to initialise a `static`.
    #[must_use]
    pub const fn from_bytes_const(bytes: &'a [u8]) -> Self {
        match Self::from_bytes(bytes) {
            Ok(f) => f,
            Err(FontError::BadMagic) => panic!("not a .lvfnt font blob"),
            Err(FontError::BadVersion(_)) => panic!("unsupported .lvfnt version"),
            Err(FontError::Truncated) => panic!("truncated .lvfnt font blob"),
        }
    }

    /// Parse a `.lvfnt` blob.
    pub const fn from_bytes(bytes: &'a [u8]) -> Result<Self, FontError> {
        if bytes.len() < HEADER_LEN {
            return Err(FontError::Truncated);
        }
        let mut i = 0;
        while i < MAGIC.len() {
            if bytes[i] != MAGIC[i] {
                return Err(FontError::BadMagic);
            }
            i += 1;
        }
        let version = rd_u16(bytes, 6);
        if version != VERSION {
            return Err(FontError::BadVersion(version));
        }
        let glyph_count = rd_u32(bytes, 36);
        let cmap_count = rd_u32(bytes, 40);
        let glyph_dsc_off = rd_u32(bytes, 44) as usize;
        let cmaps_off = rd_u32(bytes, 48) as usize;
        let kern_off = rd_u32(bytes, 52) as usize;
        let bitmap_off = rd_u32(bytes, 56) as usize;
        let bitmap_len = rd_u32(bytes, 60) as usize;
        if glyph_dsc_off + glyph_count as usize * GLYPH_LEN > bytes.len()
            || cmaps_off + cmap_count as usize * CMAP_LEN > bytes.len()
            || kern_off > bytes.len()
            || bitmap_off + bitmap_len > bytes.len()
        {
            return Err(FontError::Truncated);
        }
        let subpx = match bytes[24] {
            1 => Subpx::Hor,
            2 => Subpx::Ver,
            3 => Subpx::Both,
            _ => Subpx::None,
        };
        Ok(Self {
            bytes,
            line_height: rd_i32(bytes, 8),
            base_line: rd_i32(bytes, 12),
            cap_height: rd_i32(bytes, 16),
            x_height: rd_i32(bytes, 20),
            subpx,
            underline_position: bytes[25] as i8,
            underline_thickness: bytes[26] as i8,
            bpp: bytes[27],
            bitmap_format: bytes[28],
            stride: bytes[29],
            kern_kind: bytes[30],
            kern_scale: rd_u16(bytes, 32),
            glyph_count,
            cmap_count,
            glyph_dsc_off,
            cmaps_off,
            kern_off,
            bitmap_off,
            bitmap_len,
            fallback: None,
        })
    }

    /// Set the fallback font (`lv_font_t.fallback`).
    ///
    /// The fallback must be `Sync` so that the font can live in a `static`.
    #[must_use]
    pub const fn with_fallback(mut self, fallback: &'a (dyn Font + Sync)) -> Self {
        self.fallback = Some(fallback);
        self
    }

    /// Bits per pixel of the glyph bitmaps (1, 2, 3, 4 or 8).
    #[inline]
    #[must_use]
    pub const fn bpp(&self) -> u8 {
        self.bpp
    }

    /// Number of glyph descriptors, including the reserved index 0.
    #[inline]
    #[must_use]
    pub const fn glyph_count(&self) -> u32 {
        self.glyph_count
    }

    /// `true` when the bitmaps are RLE compressed.
    #[inline]
    #[must_use]
    pub const fn is_compressed(&self) -> bool {
        self.bitmap_format != BITMAP_PLAIN
    }

    /// The whole packed bitmap table.
    #[inline]
    #[must_use]
    pub fn bitmap_table(&self) -> &'a [u8] {
        &self.bytes[self.bitmap_off..self.bitmap_off + self.bitmap_len]
    }

    #[inline]
    fn glyph(&self, gid: u32) -> RawGlyph {
        let at = self.glyph_dsc_off + gid as usize * GLYPH_LEN;
        let b = self.bytes;
        RawGlyph {
            bitmap_index: rd_u32(b, at),
            adv_w: rd_u32(b, at + 4),
            box_w: rd_u16(b, at + 8),
            box_h: rd_u16(b, at + 10),
            ofs_x: rd_u16(b, at + 12) as i16,
            ofs_y: rd_u16(b, at + 14) as i16,
        }
    }

    #[inline]
    fn cmap(&self, i: u32) -> RawCmap {
        let at = self.cmaps_off + i as usize * CMAP_LEN;
        let b = self.bytes;
        RawCmap {
            range_start: rd_u32(b, at),
            range_length: rd_u16(b, at + 4),
            glyph_id_start: rd_u16(b, at + 6),
            ty: b[at + 8],
            list_length: rd_u16(b, at + 10),
            unicode_list_off: rd_u32(b, at + 12) as usize,
            glyph_id_ofs_off: rd_u32(b, at + 16) as usize,
        }
    }

    #[inline]
    fn u16_at(&self, off: usize, index: usize) -> u16 {
        rd_u16(self.bytes, off + index * 2)
    }

    /// Binary search of `key` in a sorted `u16` table (`lv_utils_bsearch`).
    fn bsearch_u16(&self, off: usize, len: usize, key: u16) -> Option<usize> {
        let (mut lo, mut hi) = (0usize, len);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let v = self.u16_at(off, mid);
            if v == key {
                return Some(mid);
            } else if v < key {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        None
    }

    /// `get_glyph_dsc_id()`: glyph index for a code point, 0 if missing.
    #[must_use]
    pub fn glyph_id(&self, letter: u32) -> u32 {
        if letter == 0 {
            return 0;
        }
        for i in 0..self.cmap_count {
            let c = self.cmap(i);
            let rcp = letter.wrapping_sub(c.range_start);
            if rcp >= c.range_length as u32 {
                continue;
            }
            let glyph_id = match c.ty {
                CMAP_FORMAT0_TINY => c.glyph_id_start as u32 + rcp,
                CMAP_FORMAT0_FULL => {
                    let ofs = self.bytes[c.glyph_id_ofs_off + rcp as usize];
                    // The first character is always valid with offset 0; a 0 anywhere
                    // else marks a missing character.
                    if ofs == 0 && letter != c.range_start {
                        continue;
                    }
                    c.glyph_id_start as u32 + ofs as u32
                }
                CMAP_SPARSE_TINY => {
                    match self.bsearch_u16(c.unicode_list_off, c.list_length as usize, rcp as u16) {
                        Some(ofs) => c.glyph_id_start as u32 + ofs as u32,
                        None => 0,
                    }
                }
                CMAP_SPARSE_FULL => {
                    match self.bsearch_u16(c.unicode_list_off, c.list_length as usize, rcp as u16) {
                        Some(ofs) => c.glyph_id_start as u32 + self.u16_at(c.glyph_id_ofs_off, ofs) as u32,
                        None => 0,
                    }
                }
                _ => 0,
            };
            return glyph_id;
        }
        0
    }

    /// `get_kern_value()`
    fn kern_value(&self, gid_left: u32, gid_right: u32) -> i8 {
        let b = self.bytes;
        let k = self.kern_off;
        match self.kern_kind {
            KERN_CLASSES => {
                let right_class_cnt = b[k + 1] as usize;
                let left_off = rd_u32(b, k + 4) as usize;
                let left_len = rd_u32(b, k + 8) as usize;
                let right_off = rd_u32(b, k + 12) as usize;
                let right_len = rd_u32(b, k + 16) as usize;
                let values_off = rd_u32(b, k + 20) as usize;
                let values_len = rd_u32(b, k + 24) as usize;
                if gid_left as usize >= left_len || gid_right as usize >= right_len {
                    return 0;
                }
                let left_class = b[left_off + gid_left as usize] as usize;
                let right_class = b[right_off + gid_right as usize] as usize;
                if left_class > 0 && right_class > 0 {
                    let idx = (left_class - 1) * right_class_cnt + (right_class - 1);
                    if idx < values_len {
                        return b[values_off + idx] as i8;
                    }
                }
                0
            }
            KERN_PAIRS => {
                let pair_cnt = rd_u32(b, k) as usize;
                let ids_u16 = b[k + 4] == 1;
                let ids_off = rd_u32(b, k + 8) as usize;
                let values_off = rd_u32(b, k + 12) as usize;
                // Pairs are sorted by left id, then right id.
                let (mut lo, mut hi) = (0usize, pair_cnt);
                while lo < hi {
                    let mid = lo + (hi - lo) / 2;
                    let (l, r) = if ids_u16 {
                        (
                            rd_u16(b, ids_off + mid * 4) as u32,
                            rd_u16(b, ids_off + mid * 4 + 2) as u32,
                        )
                    } else {
                        (b[ids_off + mid * 2] as u32, b[ids_off + mid * 2 + 1] as u32)
                    };
                    let ord = if gid_left != l {
                        gid_left.cmp(&l)
                    } else {
                        gid_right.cmp(&r)
                    };
                    match ord {
                        core::cmp::Ordering::Equal => return b[values_off + mid] as i8,
                        core::cmp::Ordering::Greater => lo = mid + 1,
                        core::cmp::Ordering::Less => hi = mid,
                    }
                }
                0
            }
            _ => 0,
        }
    }

    /// Packed bitmap bytes of a glyph (`glyph_bitmap_acquire`).
    fn glyph_bytes(&self, g: &RawGlyph) -> &'a [u8] {
        &self.bitmap_table()[g.bitmap_index as usize..]
    }
}

#[derive(Copy, Clone)]
struct RawGlyph {
    bitmap_index: u32,
    adv_w: u32,
    box_w: u16,
    box_h: u16,
    ofs_x: i16,
    ofs_y: i16,
}

#[derive(Copy, Clone)]
struct RawCmap {
    range_start: u32,
    range_length: u16,
    glyph_id_start: u16,
    ty: u8,
    list_length: u16,
    unicode_list_off: usize,
    glyph_id_ofs_off: usize,
}

impl Font for FmtTxtFont<'_> {
    fn line_height(&self) -> i32 {
        self.line_height
    }
    fn base_line(&self) -> i32 {
        self.base_line
    }
    fn cap_height(&self) -> i32 {
        self.cap_height
    }
    fn x_height(&self) -> i32 {
        self.x_height
    }
    fn subpx(&self) -> Subpx {
        self.subpx
    }
    fn underline_position(&self) -> i8 {
        self.underline_position
    }
    fn underline_thickness(&self) -> i8 {
        self.underline_thickness
    }
    fn has_static_bitmap(&self) -> bool {
        // Upstream: "Bitmaps are stored as const so they are always static if not compressed".
        !self.is_compressed()
    }
    fn fallback(&self) -> Option<&dyn Font> {
        self.fallback.map(|f| f as &dyn Font)
    }

    /// `lv_font_get_glyph_dsc_fmt_txt()`
    fn glyph_dsc(&self, out: &mut GlyphDsc, letter: u32, letter_next: u32) -> bool {
        let is_tab = letter == '\t' as u32;
        let letter = if is_tab { ' ' as u32 } else { letter };
        let gid = self.glyph_id(letter);
        if gid == 0 {
            return false;
        }
        let mut kvalue: i8 = 0;
        if self.kern_kind != KERN_NONE {
            let gid_next = self.glyph_id(letter_next);
            if gid_next != 0 {
                kvalue = self.kern_value(gid, gid_next);
            }
        }
        let g = self.glyph(gid);
        let kv = (kvalue as i32 * self.kern_scale as i32) >> 4;
        let mut adv_w = g.adv_w;
        if is_tab {
            adv_w *= 2;
        }
        let adv_w = (adv_w as i32 + kv).clamp(0, i32::MAX) as u32;
        let adv_w = (adv_w + (1 << 3)) >> 4;
        out.adv_w = adv_w as u16;
        out.box_h = g.box_h;
        out.box_w = g.box_w;
        out.ofs_x = g.ofs_x;
        out.ofs_y = g.ofs_y;
        out.stride = if self.stride == 0 {
            0
        } else {
            let bit_count = out.box_w as u32 * self.bpp as u32;
            let width_in_bytes = (bit_count + 7) >> 3;
            round_up(width_in_bytes, self.stride as u32) as u16
        };
        out.format = GlyphFormat::from_bpp(self.bpp);
        out.is_placeholder = false;
        out.gid = gid;
        if is_tab {
            out.box_w *= 2;
        }
        true
    }

    /// `lv_font_get_bitmap_fmt_txt()` for the "decode to A8" path.
    fn glyph_bitmap_a8(&self, g_dsc: &GlyphDsc, out: &mut [u8], stride_out: usize) -> bool {
        let gid = g_dsc.gid;
        if gid == 0 || gid >= self.glyph_count {
            return false;
        }
        let g = self.glyph(gid);
        let (box_w, box_h) = (g.box_w as usize, g.box_h as usize);
        if box_w * box_h == 0 {
            return false;
        }
        if out.len() < stride_out * (box_h - 1) + box_w {
            crate::misc::check_arg_failed("glyph_bitmap_a8: output buffer too small");
            return false;
        }
        let bitmap_in = self.glyph_bytes(&g);
        let stride_in = g_dsc.stride as usize;
        if self.bitmap_format == BITMAP_PLAIN {
            decode_plain(bitmap_in, self.bpp, stride_in, box_w, box_h, out, stride_out).is_some()
        } else {
            let prefilter = self.bitmap_format == BITMAP_COMPRESSED;
            decompress(bitmap_in, out, stride_out, box_w, box_h, self.bpp, prefilter)
        }
    }

    fn glyph_static_bitmap(&self, g_dsc: &GlyphDsc) -> Option<&[u8]> {
        if self.is_compressed() || g_dsc.gid == 0 || g_dsc.gid >= self.glyph_count {
            return None;
        }
        let g = self.glyph(g_dsc.gid);
        Some(self.glyph_bytes(&g))
    }
}

/// Plain (uncompressed) decode of a packed `bpp` bitmap into A8 rows.
///
/// Mirrors the word-at-a-time loop of upstream: whole source bytes are read
/// and `shift` carries the bit position between rows because, without a
/// stride, rows are packed continuously.
fn decode_plain(
    bitmap_in: &[u8],
    bpp: u8,
    stride_in: usize,
    box_w: usize,
    box_h: usize,
    out: &mut [u8],
    stride_out: usize,
) -> Option<()> {
    if !matches!(bpp, 1 | 2 | 4 | 8) {
        return None;
    }
    let mut inp = 0usize; // index into bitmap_in
    let mut shift: i32 = 8;
    let mut cur: u8 = 0;
    let mut row_start = 0usize;
    for _y in 0..box_h {
        let mut used = 0usize;
        if stride_in != 0 {
            shift = 8; // with a stride every row starts on a byte boundary
        }
        let row = &mut out[row_start..row_start + box_w];
        match bpp {
            4 => {
                let mut x = 0usize;
                if shift == 4 {
                    // the row starts mid byte, finish the low nibble
                    row[x] = OPA4_TABLE[(cur & 0xF) as usize];
                    x += 1;
                    shift = 8;
                }
                while x + 1 < box_w {
                    cur = *bitmap_in.get(inp)?;
                    inp += 1;
                    used += 1;
                    row[x] = OPA4_TABLE[(cur >> 4) as usize];
                    row[x + 1] = OPA4_TABLE[(cur & 0xF) as usize];
                    x += 2;
                }
                if x < box_w {
                    // odd width, the low nibble belongs to the next row
                    cur = *bitmap_in.get(inp)?;
                    inp += 1;
                    used += 1;
                    row[x] = OPA4_TABLE[(cur >> 4) as usize];
                    shift = 4;
                }
            }
            8 => {
                let src = bitmap_in.get(inp..inp + box_w)?;
                row.copy_from_slice(src);
                inp += box_w;
                used = box_w;
            }
            _ => {
                // bpp 1 or 2
                let bpp_i = bpp as i32;
                let per = 8 / bpp_i;
                let mask = (1u32 << bpp) - 1;
                let mut x = 0usize;
                while x < box_w {
                    if shift >= 8 {
                        cur = *bitmap_in.get(inp)?;
                        inp += 1;
                        used += 1;
                        shift = 0;
                    }
                    let mut n = per - shift / bpp_i;
                    if n as usize > box_w - x {
                        n = (box_w - x) as i32;
                    }
                    while n > 0 {
                        let v = ((cur as u32) >> (8 - bpp_i - shift)) & mask;
                        row[x] = if bpp == 1 {
                            if v != 0 { 0xff } else { 0x00 }
                        } else {
                            OPA2_TABLE[v as usize]
                        };
                        x += 1;
                        shift += bpp_i;
                        n -= 1;
                    }
                }
                if shift >= 8 {
                    shift = 8;
                }
            }
        }
        if stride_in != 0 {
            inp += stride_in - used;
            shift = 8;
        }
        row_start += stride_out;
    }
    Some(())
}

/// `get_bits()`: read `len` bits at `bit_pos` (MSB first).
#[inline]
fn get_bits(input: &[u8], bit_pos: u32, len: u8) -> u8 {
    let bit_mask: u16 = match len {
        1 => 0x1,
        2 => 0x3,
        3 => 0x7,
        4 => 0xF,
        8 => 0xFF,
        _ => (1u16 << len) - 1,
    };
    let byte_pos = (bit_pos >> 3) as usize;
    let bit_pos = bit_pos & 0x7;
    let b0 = input.get(byte_pos).copied().unwrap_or(0) as u16;
    if bit_pos + len as u32 >= 8 {
        let b1 = input.get(byte_pos + 1).copied().unwrap_or(0) as u16;
        let in16 = (b0 << 8) + b1;
        ((in16 >> (16 - bit_pos - len as u32)) & bit_mask) as u8
    } else {
        ((b0 >> (8 - bit_pos - len as u32)) & bit_mask) as u8
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum RleState {
    Single,
    Repeated,
    Counter,
}

/// `lv_font_fmt_rle_t` plus `rle_init`/`rle_next`.
struct Rle<'a> {
    input: &'a [u8],
    bpp: u8,
    state: RleState,
    rdp: u32,
    prev_v: u8,
    count: u32,
}

impl<'a> Rle<'a> {
    fn new(input: &'a [u8], bpp: u8) -> Self {
        Self {
            input,
            bpp,
            state: RleState::Single,
            rdp: 0,
            prev_v: 0,
            count: 0,
        }
    }

    fn next(&mut self) -> u8 {
        match self.state {
            RleState::Single => {
                let ret = get_bits(self.input, self.rdp, self.bpp);
                if self.rdp != 0 && self.prev_v == ret {
                    self.count = 0;
                    self.state = RleState::Repeated;
                }
                self.prev_v = ret;
                self.rdp += self.bpp as u32;
                ret
            }
            RleState::Repeated => {
                let v = get_bits(self.input, self.rdp, 1);
                self.count += 1;
                self.rdp += 1;
                if v == 1 {
                    let mut ret = self.prev_v;
                    if self.count == 11 {
                        self.count = get_bits(self.input, self.rdp, 6) as u32;
                        self.rdp += 6;
                        if self.count != 0 {
                            self.state = RleState::Counter;
                        } else {
                            ret = get_bits(self.input, self.rdp, self.bpp);
                            self.prev_v = ret;
                            self.rdp += self.bpp as u32;
                            self.state = RleState::Single;
                        }
                    }
                    ret
                } else {
                    let ret = get_bits(self.input, self.rdp, self.bpp);
                    self.prev_v = ret;
                    self.rdp += self.bpp as u32;
                    self.state = RleState::Single;
                    ret
                }
            }
            RleState::Counter => {
                let mut ret = self.prev_v;
                self.count -= 1;
                if self.count == 0 {
                    ret = get_bits(self.input, self.rdp, self.bpp);
                    self.prev_v = ret;
                    self.rdp += self.bpp as u32;
                    self.state = RleState::Single;
                }
                ret
            }
        }
    }

    fn line(&mut self, out: &mut [u8]) {
        for v in out.iter_mut() {
            *v = self.next();
        }
    }
}

/// `decompress()`: RLE decode a glyph into A8 rows of `stride_out` bytes.
fn decompress(
    input: &[u8],
    out: &mut [u8],
    stride_out: usize,
    w: usize,
    h: usize,
    bpp: u8,
    prefilter: bool,
) -> bool {
    let opa_table: &[u8] = match bpp {
        2 => &OPA2_TABLE,
        3 => &OPA3_TABLE,
        4 => &OPA4_TABLE,
        _ => return false,
    };
    let mut rle = Rle::new(input, bpp);
    let mut line_buf1 = vec![0u8; w];
    let mut line_buf2 = if prefilter { vec![0u8; w] } else { Vec::new() };
    rle.line(&mut line_buf1);
    let mut row_start = 0usize;
    for x in 0..w {
        out[row_start + x] = opa_table[line_buf1[x] as usize];
    }
    row_start += stride_out;
    for _y in 1..h {
        if prefilter {
            rle.line(&mut line_buf2);
            for x in 0..w {
                line_buf1[x] ^= line_buf2[x];
                out[row_start + x] = opa_table[line_buf1[x] as usize];
            }
        } else {
            rle.line(&mut line_buf1);
            for x in 0..w {
                out[row_start + x] = opa_table[line_buf1[x] as usize];
            }
        }
        row_start += stride_out;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_bits_crosses_byte_boundary() {
        let data = [0b1011_0011u8, 0b1100_0000];
        assert_eq!(get_bits(&data, 0, 4), 0b1011);
        assert_eq!(get_bits(&data, 4, 4), 0b0011);
        assert_eq!(get_bits(&data, 6, 4), 0b1111);
        assert_eq!(get_bits(&data, 7, 1), 1);
        assert_eq!(get_bits(&data, 12, 4), 0, "reads past the end as zero");
    }

    #[test]
    fn plain_decode_4bpp_odd_width_carries_nibble() {
        // Two rows of width 3: nibbles 0,e,a | 0,d,9 packed as 0x0e 0xa0 0xd9.
        let input = [0x0e, 0xa0, 0xd9];
        let mut out = [0u8; 6];
        assert!(decode_plain(&input, 4, 0, 3, 2, &mut out, 3).is_some());
        assert_eq!(out, [0, 238, 170, 0, 221, 153]);
    }

    #[test]
    fn plain_decode_1bpp() {
        let input = [0b1010_0000, 0b0101_0000];
        let mut out = [0u8; 8];
        assert!(decode_plain(&input, 1, 1, 4, 2, &mut out, 4).is_some());
        assert_eq!(out, [255, 0, 255, 0, 0, 255, 0, 255]);
    }

    #[test]
    fn rejects_bad_blob() {
        assert_eq!(
            FmtTxtFont::from_bytes(&[0u8; 10]).unwrap_err(),
            FontError::Truncated
        );
        let mut b = [0u8; 64];
        assert_eq!(FmtTxtFont::from_bytes(&b).unwrap_err(), FontError::BadMagic);
        b[..6].copy_from_slice(b"LVFNT\0");
        b[6] = 9;
        assert_eq!(FmtTxtFont::from_bytes(&b).unwrap_err(), FontError::BadVersion(9));
    }
}
