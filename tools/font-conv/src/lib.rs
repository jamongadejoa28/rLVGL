//! Parser for LVGL's generated `lv_font_*.c` files (output of `lv_font_conv
//! --format lvgl`) and encoder for the `.lvfnt` table format read by
//! `ferrovgl_core::font::fmt_txt`.
//!
//! The `.lvfnt` layout (all integers little-endian) is documented in
//! `ferrovgl_core::font::fmt_txt`; [`encode`] is the single source of truth
//! for the writer side and the reader has matching constants.

use std::collections::HashMap;

/// Built-in fonts converted by `cargo xtask fonts` (file stem after `lv_font_`).
pub const BUILTIN_FONTS: &[&str] = &[
    "montserrat_8",
    "montserrat_10",
    "montserrat_12",
    "montserrat_14",
    "montserrat_16",
    "montserrat_18",
    "montserrat_20",
    "montserrat_22",
    "montserrat_24",
    "montserrat_26",
    "montserrat_28",
    "montserrat_30",
    "montserrat_32",
    "montserrat_34",
    "montserrat_36",
    "montserrat_38",
    "montserrat_40",
    "montserrat_42",
    "montserrat_44",
    "montserrat_46",
    "montserrat_48",
    "montserrat_28_compressed",
    "unscii_8",
    "unscii_16",
    "dejavu_16_persian_hebrew",
];

/// Magic bytes at the start of a `.lvfnt` blob.
pub const MAGIC: &[u8; 6] = b"LVFNT\0";
/// Format version written by this encoder.
pub const VERSION: u16 = 1;

/// A parsed font.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Font {
    /// `lv_font_t.line_height`
    pub line_height: i32,
    /// `lv_font_t.base_line`
    pub base_line: i32,
    /// `lv_font_t.cap_height` (0 when the file predates it)
    pub cap_height: i32,
    /// `lv_font_t.x_height`
    pub x_height: i32,
    /// `lv_font_t.subpx`
    pub subpx: u8,
    /// `lv_font_t.underline_position`
    pub underline_position: i8,
    /// `lv_font_t.underline_thickness`
    pub underline_thickness: i8,
    /// `font_dsc.bpp`
    pub bpp: u8,
    /// `font_dsc.bitmap_format` (0 plain, 1 compressed, 2 compressed without prefilter)
    pub bitmap_format: u8,
    /// `font_dsc.kern_scale` (12.4)
    pub kern_scale: u16,
    /// `font_dsc.stride`
    pub stride: u8,
    /// Glyph descriptors, index 0 reserved.
    pub glyphs: Vec<Glyph>,
    /// Character maps in lookup order.
    pub cmaps: Vec<Cmap>,
    /// Kerning data.
    pub kern: Kern,
    /// Packed glyph bitmaps.
    pub bitmap: Vec<u8>,
}

/// One glyph descriptor (`lv_font_fmt_txt_glyph_dsc_t`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    /// Start of the glyph in the bitmap array.
    pub bitmap_index: u32,
    /// Advance width in 8.4 fixed point.
    pub adv_w: u32,
    /// Bounding box width.
    pub box_w: u16,
    /// Bounding box height.
    pub box_h: u16,
    /// Horizontal offset of the bounding box.
    pub ofs_x: i16,
    /// Vertical offset of the bounding box, measured from the baseline.
    pub ofs_y: i16,
}

/// `lv_font_fmt_txt_cmap_type_t`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CmapType {
    /// `LV_FONT_FMT_TXT_CMAP_FORMAT0_FULL`
    Format0Full = 0,
    /// `LV_FONT_FMT_TXT_CMAP_SPARSE_FULL`
    SparseFull = 1,
    /// `LV_FONT_FMT_TXT_CMAP_FORMAT0_TINY`
    Format0Tiny = 2,
    /// `LV_FONT_FMT_TXT_CMAP_SPARSE_TINY`
    SparseTiny = 3,
}

/// One character map (`lv_font_fmt_txt_cmap_t`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmap {
    /// First code point of the range.
    pub range_start: u32,
    /// Number of code points in the range.
    pub range_length: u16,
    /// Glyph id of the first code point.
    pub glyph_id_start: u16,
    /// Map type.
    pub ty: CmapType,
    /// Sorted relative code points (sparse maps).
    pub unicode_list: Vec<u16>,
    /// Glyph id offsets (`u8` for format0 full, `u16` for sparse full).
    pub glyph_id_ofs_list: Vec<u16>,
}

/// Kerning data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Kern {
    /// No kerning.
    #[default]
    None,
    /// `lv_font_fmt_txt_kern_pair_t`
    Pairs {
        /// `(left, right)` glyph id pairs, sorted.
        pairs: Vec<(u16, u16)>,
        /// Kern value per pair.
        values: Vec<i8>,
        /// `true` when upstream stored the ids as `uint16_t`.
        ids_u16: bool,
    },
    /// `lv_font_fmt_txt_kern_classes_t`
    Classes {
        /// Left class per glyph id.
        left_mapping: Vec<u8>,
        /// Right class per glyph id.
        right_mapping: Vec<u8>,
        /// `left_class_cnt * right_class_cnt` values.
        values: Vec<i8>,
        /// Number of left classes.
        left_class_cnt: u8,
        /// Number of right classes.
        right_class_cnt: u8,
    },
}

fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            let end = text[i + 2..]
                .find("*/")
                .map(|e| i + 2 + e + 2)
                .unwrap_or(bytes.len());
            out.push(' ');
            i = end;
        } else if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            let end = text[i..].find('\n').map(|e| i + e).unwrap_or(bytes.len());
            i = end;
        } else {
            // Copy one UTF-8 character.
            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Resolve the `#if` blocks of a generated font file for LVGL v9.6.0 and drop
/// all preprocessor lines. Generated fonts only use version checks
/// (`LVGL_VERSION_MAJOR == 8`, `LV_VERSION_CHECK(9, 6, 0)`, ...) and include
/// guards, so a tiny evaluator is enough.
fn preprocess(text: &str) -> String {
    fn condition_true(cond: &str) -> bool {
        let c = cond.trim();
        if c.contains("== 8") || c.contains("== 6") || c.contains("== 7") {
            return false;
        }
        true
    }
    // Stack of (currently emitting, parent emitting).
    let mut stack: Vec<(bool, bool)> = Vec::new();
    let mut emitting = true;
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let t = line.trim_start();
        if let Some(directive) = t.strip_prefix('#') {
            let directive = directive.trim_start();
            if let Some(cond) = directive.strip_prefix("if ") {
                stack.push((emitting, emitting));
                emitting = emitting && condition_true(cond);
            } else if directive.starts_with("ifdef") || directive.starts_with("ifndef") {
                stack.push((emitting, emitting));
            } else if directive.starts_with("else") {
                if let Some(&(_, parent)) = stack.last() {
                    emitting = parent && !emitting;
                }
            } else if directive.starts_with("endif")
                && let Some((prev, _)) = stack.pop()
            {
                emitting = prev;
            }
            continue; // never emit preprocessor lines
        }
        if emitting {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn parse_int(tok: &str) -> Result<i64, String> {
    let t = tok.trim().trim_end_matches(['u', 'U', 'l', 'L']);
    let (neg, t) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t),
    };
    let v = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        i64::from_str_radix(h, 16)
    } else {
        t.parse::<i64>()
    }
    .map_err(|e| format!("bad integer {tok:?}: {e}"))?;
    Ok(if neg { -v } else { v })
}

/// All `static const <type> NAME[] = { ... };` arrays of plain integers.
fn collect_int_arrays(text: &str) -> Result<HashMap<String, Vec<i64>>, String> {
    let mut map = HashMap::new();
    let mut rest = text;
    while let Some(pos) = rest.find("[] = {") {
        // Walk back to the identifier before `[]`.
        let head = &rest[..pos];
        let name_start = head
            .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
            .map(|i| i + 1)
            .unwrap_or(0);
        let name = &head[name_start..];
        let body_start = pos + "[] = {".len();
        let body_end = rest[body_start..].find("};").ok_or("unterminated array")? + body_start;
        let body = &rest[body_start..body_end];
        // Only plain integer arrays: skip struct arrays (they contain '{' or '.').
        if !body.contains('{') && !body.contains('.') {
            let mut values = Vec::new();
            for tok in body.split(',') {
                let tok = tok.trim();
                if tok.is_empty() {
                    continue;
                }
                values.push(parse_int(tok)?);
            }
            map.insert(name.to_string(), values);
        }
        rest = &rest[body_end + 2..];
    }
    Ok(map)
}

fn field<'a>(block: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(".{name}");
    let mut search = block;
    while let Some(p) = search.find(&key) {
        let after = &search[p + key.len()..];
        let after_trim = after.trim_start();
        if let Some(rest) = after_trim.strip_prefix('=') {
            let end = rest.find([',', '}', '\n']).unwrap_or(rest.len());
            return Some(rest[..end].trim());
        }
        search = after;
    }
    None
}

fn field_int(block: &str, name: &str) -> Result<i64, String> {
    let v = field(block, name).ok_or_else(|| format!("missing field .{name}"))?;
    parse_int(v)
}

fn struct_init<'a>(text: &'a str, decl_contains: &str) -> Option<&'a str> {
    // Find `<decl_contains> ... = {` and return the brace body.
    let p = text.find(decl_contains)?;
    let rest = &text[p..];
    let open = rest.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in rest[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&rest[open + 1..open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse a generated `lv_font_*.c` file.
pub fn parse_c_font(text: &str) -> Result<Font, String> {
    let text = preprocess(&strip_comments(text));
    let arrays = collect_int_arrays(&text)?;
    let int_array = |name: &str| -> Result<&Vec<i64>, String> {
        arrays.get(name).ok_or_else(|| format!("array {name} not found"))
    };

    let bitmap: Vec<u8> = int_array("glyph_bitmap")?
        .iter()
        .map(|&v| u8::try_from(v).map_err(|_| format!("bitmap byte {v} out of range")))
        .collect::<Result<_, _>>()?;

    // Glyph descriptors
    let gd_body = struct_init(&text, "glyph_dsc[] =").ok_or("glyph_dsc[] not found")?;
    let mut glyphs = Vec::new();
    for entry in gd_body.split('{').skip(1) {
        let entry = &entry[..entry.find('}').ok_or("bad glyph entry")?];
        glyphs.push(Glyph {
            bitmap_index: field_int(entry, "bitmap_index")? as u32,
            adv_w: field_int(entry, "adv_w")? as u32,
            box_w: field_int(entry, "box_w")? as u16,
            box_h: field_int(entry, "box_h")? as u16,
            ofs_x: field_int(entry, "ofs_x")? as i16,
            ofs_y: field_int(entry, "ofs_y")? as i16,
        });
    }

    // Character maps
    let cm_body = struct_init(&text, "cmaps[] =").ok_or("cmaps[] not found")?;
    let mut cmaps = Vec::new();
    for entry in cm_body.split('{').skip(1) {
        let entry = &entry[..entry.find('}').ok_or("bad cmap entry")?];
        let ty = match field(entry, "type").ok_or("cmap without type")? {
            "LV_FONT_FMT_TXT_CMAP_FORMAT0_FULL" => CmapType::Format0Full,
            "LV_FONT_FMT_TXT_CMAP_SPARSE_FULL" => CmapType::SparseFull,
            "LV_FONT_FMT_TXT_CMAP_FORMAT0_TINY" => CmapType::Format0Tiny,
            "LV_FONT_FMT_TXT_CMAP_SPARSE_TINY" => CmapType::SparseTiny,
            other => return Err(format!("unknown cmap type {other}")),
        };
        let list = |name: &str| -> Result<Vec<u16>, String> {
            match field(entry, name).unwrap_or("NULL") {
                "NULL" => Ok(Vec::new()),
                arr => int_array(arr)?.iter().map(|&v| Ok(v as u16)).collect(),
            }
        };
        cmaps.push(Cmap {
            range_start: field_int(entry, "range_start")? as u32,
            range_length: field_int(entry, "range_length")? as u16,
            glyph_id_start: field_int(entry, "glyph_id_start")? as u16,
            ty,
            unicode_list: list("unicode_list")?,
            glyph_id_ofs_list: list("glyph_id_ofs_list")?,
        });
    }

    // Font descriptor
    let fd = struct_init(&text, "font_dsc =").ok_or("font_dsc not found")?;
    let kern_classes_flag = field_int(fd, "kern_classes").unwrap_or(0);
    let kern = match field(fd, "kern_dsc").unwrap_or("NULL") {
        "NULL" => Kern::None,
        _ if kern_classes_flag == 1 => {
            let kc = struct_init(&text, "kern_classes =").ok_or("kern_classes struct not found")?;
            let name = |f: &str| field(kc, f).ok_or_else(|| format!("kern_classes.{f} missing"));
            let to_u8 = |v: &Vec<i64>| v.iter().map(|&x| x as u8).collect::<Vec<u8>>();
            let to_i8 = |v: &Vec<i64>| v.iter().map(|&x| x as i8).collect::<Vec<i8>>();
            Kern::Classes {
                values: to_i8(int_array(name("class_pair_values")?)?),
                left_mapping: to_u8(int_array(name("left_class_mapping")?)?),
                right_mapping: to_u8(int_array(name("right_class_mapping")?)?),
                left_class_cnt: field_int(kc, "left_class_cnt")? as u8,
                right_class_cnt: field_int(kc, "right_class_cnt")? as u8,
            }
        }
        _ => {
            let kp = struct_init(&text, "kern_pairs =").ok_or("kern_pairs struct not found")?;
            let ids = int_array(field(kp, "glyph_ids").ok_or("kern_pairs.glyph_ids missing")?)?;
            let values = int_array(field(kp, "values").ok_or("kern_pairs.values missing")?)?;
            let ids_u16 = field_int(kp, "glyph_ids_size")? == 1;
            let pairs = ids.chunks(2).map(|c| (c[0] as u16, c[1] as u16)).collect();
            Kern::Pairs {
                pairs,
                values: values.iter().map(|&v| v as i8).collect(),
                ids_u16,
            }
        }
    };

    // Public font struct
    let pf_pos = text
        .find("get_glyph_dsc = ")
        .ok_or("lv_font_t initializer not found")?;
    let pf = &text[pf_pos..];
    let pf_end = pf.find("};").ok_or("unterminated lv_font_t")?;
    let pf = &pf[..pf_end];
    let subpx = match field(pf, "subpx").unwrap_or("LV_FONT_SUBPX_NONE") {
        "LV_FONT_SUBPX_NONE" => 0,
        "LV_FONT_SUBPX_HOR" => 1,
        "LV_FONT_SUBPX_VER" => 2,
        "LV_FONT_SUBPX_BOTH" => 3,
        other => parse_int(other)? as u8,
    };

    Ok(Font {
        line_height: field_int(pf, "line_height")? as i32,
        base_line: field_int(pf, "base_line")? as i32,
        cap_height: field_int(pf, "cap_height").unwrap_or(0) as i32,
        x_height: field_int(pf, "x_height").unwrap_or(0) as i32,
        subpx,
        underline_position: field_int(pf, "underline_position").unwrap_or(0) as i8,
        underline_thickness: field_int(pf, "underline_thickness").unwrap_or(0) as i8,
        bpp: field_int(fd, "bpp")? as u8,
        bitmap_format: field_int(fd, "bitmap_format").unwrap_or(0) as u8,
        kern_scale: field_int(fd, "kern_scale").unwrap_or(0) as u16,
        stride: field_int(fd, "stride").unwrap_or(0) as u8,
        glyphs,
        cmaps,
        kern,
        bitmap,
    })
}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn i8(&mut self, v: i8) {
        self.0.push(v as u8);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i16(&mut self, v: i16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn patch_u32(&mut self, at: usize, v: u32) {
        self.0[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn align4(&mut self) {
        while !self.0.len().is_multiple_of(4) {
            self.0.push(0);
        }
    }
}

/// Size of the fixed header in bytes.
pub const HEADER_LEN: usize = 64;
/// Size of one glyph descriptor record.
pub const GLYPH_LEN: usize = 16;
/// Size of one cmap record.
pub const CMAP_LEN: usize = 20;

/// Encode a font into the `.lvfnt` blob format.
///
/// Layout:
/// ```text
/// 0   magic "LVFNT\0", version u16
/// 8   line_height i32, base_line i32, cap_height i32, x_height i32
/// 24  subpx u8, underline_position i8, underline_thickness i8, bpp u8,
///     bitmap_format u8, stride u8, kern_kind u8 (0 none/1 pairs/2 classes), pad u8
/// 32  kern_scale u16, pad u16, glyph_count u32, cmap_count u32
/// 44  glyph_dsc_off u32, cmaps_off u32, kern_off u32, bitmap_off u32, bitmap_len u32
/// 64  (end of header, HEADER_LEN)
/// glyph_dsc: glyph_count x { bitmap_index u32, adv_w u32, box_w u16, box_h u16, ofs_x i16, ofs_y i16 }
/// cmaps:     cmap_count  x { range_start u32, range_length u16, glyph_id_start u16, type u8, pad u8,
///                            list_length u16, unicode_list_off u32, glyph_id_ofs_off u32 }
///            followed by the referenced lists (u16 for unicode_list and sparse-full offsets,
///            u8 for format0-full offsets)
/// kern:      classes: left_cnt u8, right_cnt u8, pad u16, left_off u32, left_len u32,
///                     right_off u32, right_len u32, values_off u32, values_len u32, arrays
///            pairs:   pair_cnt u32, ids_u16 u8, pad[3], ids_off u32, values_off u32, arrays
/// bitmap:    raw bytes
/// ```
pub fn encode(font: &Font) -> Vec<u8> {
    let mut w = Writer(Vec::with_capacity(
        HEADER_LEN + font.bitmap.len() + font.glyphs.len() * GLYPH_LEN,
    ));
    w.0.extend_from_slice(MAGIC);
    w.u16(VERSION);
    w.i32(font.line_height);
    w.i32(font.base_line);
    w.i32(font.cap_height);
    w.i32(font.x_height);
    w.u8(font.subpx);
    w.i8(font.underline_position);
    w.i8(font.underline_thickness);
    w.u8(font.bpp);
    w.u8(font.bitmap_format);
    w.u8(font.stride);
    w.u8(match &font.kern {
        Kern::None => 0,
        Kern::Pairs { .. } => 1,
        Kern::Classes { .. } => 2,
    });
    w.u8(0);
    w.u16(font.kern_scale);
    w.u16(0);
    w.u32(font.glyphs.len() as u32);
    w.u32(font.cmaps.len() as u32);
    let offsets_at = w.0.len();
    for _ in 0..5 {
        w.u32(0);
    }
    assert_eq!(w.0.len(), HEADER_LEN);

    // Glyph descriptors
    let glyph_dsc_off = w.0.len() as u32;
    for g in &font.glyphs {
        w.u32(g.bitmap_index);
        w.u32(g.adv_w);
        w.u16(g.box_w);
        w.u16(g.box_h);
        w.i16(g.ofs_x);
        w.i16(g.ofs_y);
    }

    // Cmaps: table first, lists after.
    let cmaps_off = w.0.len() as u32;
    let table_start = w.0.len();
    for c in &font.cmaps {
        w.u32(c.range_start);
        w.u16(c.range_length);
        w.u16(c.glyph_id_start);
        w.u8(c.ty as u8);
        w.u8(0);
        w.u16(c.unicode_list.len().max(c.glyph_id_ofs_list.len()) as u16);
        w.u32(0); // unicode_list_off
        w.u32(0); // glyph_id_ofs_off
    }
    for (i, c) in font.cmaps.iter().enumerate() {
        let rec = table_start + i * CMAP_LEN;
        if !c.unicode_list.is_empty() {
            w.align4();
            w.patch_u32(rec + 12, w.0.len() as u32);
            for &u in &c.unicode_list {
                w.u16(u);
            }
        }
        if !c.glyph_id_ofs_list.is_empty() {
            w.align4();
            w.patch_u32(rec + 16, w.0.len() as u32);
            match c.ty {
                CmapType::Format0Full => {
                    for &o in &c.glyph_id_ofs_list {
                        w.u8(o as u8);
                    }
                }
                _ => {
                    for &o in &c.glyph_id_ofs_list {
                        w.u16(o);
                    }
                }
            }
        }
    }

    // Kerning
    w.align4();
    let kern_off = w.0.len() as u32;
    match &font.kern {
        Kern::None => {}
        Kern::Classes {
            left_mapping,
            right_mapping,
            values,
            left_class_cnt,
            right_class_cnt,
        } => {
            w.u8(*left_class_cnt);
            w.u8(*right_class_cnt);
            w.u16(0);
            let fix = w.0.len();
            for _ in 0..6 {
                w.u32(0);
            }
            let left_off = w.0.len() as u32;
            w.0.extend_from_slice(left_mapping);
            let right_off = w.0.len() as u32;
            w.0.extend_from_slice(right_mapping);
            let values_off = w.0.len() as u32;
            w.0.extend(values.iter().map(|&v| v as u8));
            for (i, v) in [
                left_off,
                left_mapping.len() as u32,
                right_off,
                right_mapping.len() as u32,
                values_off,
                values.len() as u32,
            ]
            .into_iter()
            .enumerate()
            {
                w.patch_u32(fix + i * 4, v);
            }
        }
        Kern::Pairs {
            pairs,
            values,
            ids_u16,
        } => {
            w.u32(pairs.len() as u32);
            w.u8(u8::from(*ids_u16));
            w.u8(0);
            w.u16(0);
            let fix = w.0.len();
            w.u32(0);
            w.u32(0);
            let ids_off = w.0.len() as u32;
            for &(l, r) in pairs {
                if *ids_u16 {
                    w.u16(l);
                    w.u16(r);
                } else {
                    w.u8(l as u8);
                    w.u8(r as u8);
                }
            }
            let values_off = w.0.len() as u32;
            w.0.extend(values.iter().map(|&v| v as u8));
            w.patch_u32(fix, ids_off);
            w.patch_u32(fix + 4, values_off);
        }
    }

    // Bitmap
    w.align4();
    let bitmap_off = w.0.len() as u32;
    w.0.extend_from_slice(&font.bitmap);

    for (i, v) in [
        glyph_dsc_off,
        cmaps_off,
        kern_off,
        bitmap_off,
        font.bitmap.len() as u32,
    ]
    .into_iter()
    .enumerate()
    {
        w.patch_u32(offsets_at + i * 4, v);
    }
    w.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
/* comment */
static LV_ATTRIBUTE_LARGE_CONST const uint8_t glyph_bitmap[] = {
    /* U+0021 "!" */
    0xff, 0x0f, 0x10, // trailing
    0x7
};
static const lv_font_fmt_txt_glyph_dsc_t glyph_dsc[] = {
    {.bitmap_index = 0, .adv_w = 0, .box_w = 0, .box_h = 0, .ofs_x = 0, .ofs_y = 0} /* id = 0 reserved */,
    {.bitmap_index = 0, .adv_w = 60, .box_w = 1, .box_h = 2, .ofs_x = -1, .ofs_y = -2}
};
static const uint16_t unicode_list_1[] = {
    0x00, 0x1f72
};
static const lv_font_fmt_txt_cmap_t cmaps[] = {
    {
        .range_start = 32, .range_length = 1, .glyph_id_start = 1,
        .unicode_list = NULL, .glyph_id_ofs_list = NULL, .list_length = 0, .type = LV_FONT_FMT_TXT_CMAP_FORMAT0_TINY
    },
    {
        .range_start = 176, .range_length = 63475, .glyph_id_start = 2,
        .unicode_list = unicode_list_1, .glyph_id_ofs_list = NULL, .list_length = 2, .type = LV_FONT_FMT_TXT_CMAP_SPARSE_TINY
    }
};
static const uint8_t kern_left_class_mapping[] = { 0, 1 };
static const uint8_t kern_right_class_mapping[] = { 0, 1 };
static const int8_t kern_class_values[] = { -3 };
static const lv_font_fmt_txt_kern_classes_t kern_classes = {
    .class_pair_values   = kern_class_values,
    .left_class_mapping  = kern_left_class_mapping,
    .right_class_mapping = kern_right_class_mapping,
    .left_class_cnt      = 1,
    .right_class_cnt     = 1,
};
#if LVGL_VERSION_MAJOR >= 8
static const lv_font_fmt_txt_dsc_t font_dsc = {
#else
static lv_font_fmt_txt_dsc_t font_dsc = {
#endif
    .glyph_bitmap = glyph_bitmap,
    .glyph_dsc = glyph_dsc,
    .cmaps = cmaps,
    .kern_dsc = &kern_classes,
    .kern_scale = 16,
    .cmap_num = 2,
    .bpp = 4,
    .kern_classes = 1,
    .bitmap_format = 0,
#if LVGL_VERSION_MAJOR == 8
    .cache = &cache
#endif
};
const lv_font_t lv_font_test = {
    .get_glyph_dsc = lv_font_get_glyph_dsc_fmt_txt,
    .get_glyph_bitmap = lv_font_get_bitmap_fmt_txt,
    .line_height = 16,
    .base_line = 3,
    .cap_height = 10,
    .x_height = 7,
    .subpx = LV_FONT_SUBPX_NONE,
    .underline_position = -1,
    .underline_thickness = 1,
    .dsc = &font_dsc,
    .fallback = NULL,
    .user_data = NULL,
};
"#;

    #[test]
    fn parses_sample() {
        let f = parse_c_font(SAMPLE).unwrap();
        assert_eq!(f.bitmap, [0xff, 0x0f, 0x10, 0x7]);
        assert_eq!(f.glyphs.len(), 2);
        assert_eq!(
            f.glyphs[1],
            Glyph {
                bitmap_index: 0,
                adv_w: 60,
                box_w: 1,
                box_h: 2,
                ofs_x: -1,
                ofs_y: -2
            }
        );
        assert_eq!(f.cmaps.len(), 2);
        assert_eq!(f.cmaps[1].ty, CmapType::SparseTiny);
        assert_eq!(f.cmaps[1].unicode_list, [0, 0x1f72]);
        assert_eq!(
            (f.line_height, f.base_line, f.cap_height, f.x_height),
            (16, 3, 10, 7)
        );
        assert_eq!(
            (f.underline_position, f.underline_thickness, f.bpp, f.kern_scale),
            (-1, 1, 4, 16)
        );
        match &f.kern {
            Kern::Classes {
                values,
                left_class_cnt,
                right_class_cnt,
                ..
            } => {
                assert_eq!(values, &[-3]);
                assert_eq!((*left_class_cnt, *right_class_cnt), (1, 1));
            }
            other => panic!("{other:?}"),
        }
        let blob = encode(&f);
        assert_eq!(&blob[..6], MAGIC);
        assert_eq!(blob.len() % 4, 0);
        let bitmap_off = u32::from_le_bytes(blob[56..60].try_into().unwrap()) as usize;
        assert_eq!(&blob[bitmap_off..bitmap_off + 4], &[0xff, 0x0f, 0x10, 0x7]);
    }
}
