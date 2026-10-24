//! Checks the converted Montserrat 14 blob against values read directly from
//! upstream `src/font/lv_font_montserrat_14.c`.

use ferrovgl_core::font::{Font, GlyphDsc, GlyphFormat, get_glyph_dsc, get_glyph_width};
use ferrovgl_fonts::MONTSERRAT_14;

fn dsc(letter: char, next: char) -> GlyphDsc {
    let mut d = GlyphDsc::default();
    assert!(
        MONTSERRAT_14.glyph_dsc(&mut d, letter as u32, next as u32),
        "{letter:?} missing"
    );
    d
}

#[test]
fn metrics_match_upstream_header() {
    assert_eq!(MONTSERRAT_14.line_height(), 16);
    assert_eq!(MONTSERRAT_14.base_line(), 3);
    assert_eq!(MONTSERRAT_14.cap_height(), 10);
    assert_eq!(MONTSERRAT_14.x_height(), 7);
    assert_eq!(MONTSERRAT_14.underline_position(), -1);
    assert_eq!(MONTSERRAT_14.underline_thickness(), 1);
    assert_eq!(MONTSERRAT_14.bpp(), 4);
    assert_eq!(MONTSERRAT_14.glyph_count(), 158);
    assert!(MONTSERRAT_14.has_static_bitmap());
    assert_eq!(MONTSERRAT_14.bitmap_table().len(), 8832);
}

#[test]
fn glyph_descriptors_match_c_arrays() {
    // {.bitmap_index = 1094, .adv_w = 164, .box_w = 12, .box_h = 10, .ofs_x = -1, .ofs_y = 0}
    let a = dsc('A', '\0');
    assert_eq!(a.gid, 34);
    assert_eq!((a.adv_w, a.box_w, a.box_h, a.ofs_x, a.ofs_y), (10, 12, 10, -1, 0));
    assert_eq!(a.format, GlyphFormat::A4);
    assert_eq!(a.stride, 0);
    // {.bitmap_index = 2715, .adv_w = 155, .box_w = 9, .box_h = 11, .ofs_x = 0, .ofs_y = -3}
    let g = dsc('g', '\0');
    assert_eq!((g.adv_w, g.box_w, g.box_h, g.ofs_x, g.ofs_y), (10, 9, 11, 0, -3));
    // Space has no bitmap but an advance.
    let sp = dsc(' ', '\0');
    assert_eq!((sp.adv_w, sp.box_w, sp.box_h), (4, 0, 0));
    // Tab is a double-width space.
    let tab = dsc('\t', '\0');
    assert_eq!(tab.adv_w, 8);
}

#[test]
fn kerning_from_class_table() {
    // kern class values from upstream: VA = -8, To = -12, LT = -15, AA = +3
    assert_eq!(dsc('V', 'A').adv_w, 9);
    assert_eq!(dsc('V', '\0').adv_w, 10);
    assert_eq!(dsc('T', 'o').adv_w, 7);
    assert_eq!(dsc('T', '\0').adv_w, 8);
    assert_eq!(dsc('L', 'T').adv_w, 7);
    assert_eq!(dsc('A', 'A').adv_w, 10);
    assert_eq!(get_glyph_width(&MONTSERRAT_14, 'V' as u32, 'A' as u32), 9);
}

#[test]
fn sparse_cmap_and_missing_glyphs() {
    // U+2022 BULLET lives in the sparse unicode_list_1 (rcp 0x1f72) -> gid 97.
    let bullet = dsc('\u{2022}', '\0');
    assert_eq!(bullet.gid, 97);
    // U+00B0 DEGREE SIGN is the first entry of the sparse range -> gid 96.
    assert_eq!(dsc('\u{00B0}', '\0').gid, 96);
    // FontAwesome glyphs (private use area) are present, e.g. LV_SYMBOL_OK = U+F00C.
    assert!(dsc('\u{F00C}', '\0').box_w > 0);
    let mut d = GlyphDsc::default();
    assert!(
        !MONTSERRAT_14.glyph_dsc(&mut d, 'é' as u32, 0),
        "é is not in the built-in range"
    );
    assert!(!MONTSERRAT_14.glyph_dsc(&mut d, 0, 0));
    // Fallback resolution produces a placeholder for missing glyphs.
    let (p, resolved) = get_glyph_dsc(&MONTSERRAT_14, 'é' as u32, 0);
    assert!(resolved.is_none());
    assert!(p.is_placeholder);
    assert_eq!((p.box_w, p.adv_w, p.box_h), (8, 10, 16));
}

#[test]
fn decodes_exclamation_mark_bitmap() {
    // '!' is 3x10 at bitmap index 0: 0e a0 d9 0d 90 c8 0c 80 b7 0a 60 11 0b 80 d9
    let d = dsc('!', '\0');
    assert_eq!((d.box_w, d.box_h), (3, 10));
    let raw = MONTSERRAT_14.glyph_static_bitmap(&d).unwrap();
    assert_eq!(
        &raw[..15],
        &[
            0x0e, 0xa0, 0xd9, 0x0d, 0x90, 0xc8, 0x0c, 0x80, 0xb7, 0x0a, 0x60, 0x11, 0x0b, 0x80, 0xd9
        ]
    );
    let mut out = [0u8; 30];
    assert!(MONTSERRAT_14.glyph_bitmap_a8(&d, &mut out, 3));
    let t = |n: u8| n * 17;
    assert_eq!(&out[0..3], &[t(0x0), t(0xe), t(0xa)]);
    assert_eq!(&out[3..6], &[t(0x0), t(0xd), t(0x9)]);
    assert_eq!(&out[6..9], &[t(0x0), t(0xd), t(0x9)]);
    assert_eq!(&out[9..12], &[t(0x0), t(0xc), t(0x8)]);
    assert_eq!(&out[27..30], &[t(0x0), t(0xd), t(0x9)]);
    // Decoding with a wider stride leaves the padding untouched.
    let mut out = [0xAAu8; 40];
    assert!(MONTSERRAT_14.glyph_bitmap_a8(&d, &mut out, 4));
    assert_eq!(&out[0..4], &[t(0x0), t(0xe), t(0xa), 0xAA]);
}
