//! Checks the converted DejaVu 16 Persian/Hebrew blob against values read
//! directly from upstream `src/font/lv_font_dejavu_16_persian_hebrew.c`.
//!
//! Needs the optional font: run with `--all-features` (CI does).

#![cfg(feature = "dejavu-16-persian-hebrew")]

use ferrovgl_core::font::{Font, GlyphDsc, GlyphFormat};
use ferrovgl_fonts::DEJAVU_16_PERSIAN_HEBREW;

fn dsc(letter: char) -> GlyphDsc {
    let mut d = GlyphDsc::default();
    assert!(
        DEJAVU_16_PERSIAN_HEBREW.glyph_dsc(&mut d, letter as u32, 0),
        "{letter:?} missing"
    );
    d
}

#[test]
fn metrics_match_upstream_header() {
    assert_eq!(DEJAVU_16_PERSIAN_HEBREW.line_height(), 24);
    assert_eq!(DEJAVU_16_PERSIAN_HEBREW.base_line(), 7);
    assert_eq!(DEJAVU_16_PERSIAN_HEBREW.cap_height(), 0);
    assert_eq!(DEJAVU_16_PERSIAN_HEBREW.x_height(), 0);
    assert_eq!(DEJAVU_16_PERSIAN_HEBREW.underline_position(), -1);
    assert_eq!(DEJAVU_16_PERSIAN_HEBREW.underline_thickness(), 1);
    assert_eq!(DEJAVU_16_PERSIAN_HEBREW.bpp(), 4);
    assert_eq!(DEJAVU_16_PERSIAN_HEBREW.glyph_count(), 597);
    assert!(DEJAVU_16_PERSIAN_HEBREW.has_static_bitmap());
    assert_eq!(DEJAVU_16_PERSIAN_HEBREW.bitmap_table().len(), 35278);
}

#[test]
fn glyph_descriptors_match_c_arrays() {
    // id 1: {.bitmap_index = 0, .adv_w = 81, .box_w = 0, .box_h = 0, ...} (space)
    let sp = dsc(' ');
    assert_eq!(sp.gid, 1);
    assert_eq!((sp.adv_w, sp.box_w, sp.box_h), (5, 0, 0));
    // U+05DE HEBREW LETTER MEM, cmap 1 (FORMAT0_TINY from 1488 at gid 96) -> id 110:
    // {.bitmap_index = 4815, .adv_w = 174, .box_w = 10, .box_h = 9, .ofs_x = 0, .ofs_y = 0}
    let mem = dsc('\u{05DE}');
    assert_eq!(mem.gid, 110);
    assert_eq!(
        (mem.adv_w, mem.box_w, mem.box_h, mem.ofs_x, mem.ofs_y),
        (11, 10, 9, 0, 0)
    );
    assert_eq!(mem.format, GlyphFormat::A4);
    // U+0621 ARABIC LETTER HAMZA, cmap 3 (from 1569 at gid 131) -> id 131:
    // {.bitmap_index = 5708, .adv_w = 120, .box_w = 6, .box_h = 8, .ofs_x = 1, .ofs_y = 1}
    let hamza = dsc('\u{0621}');
    assert_eq!(hamza.gid, 131);
    assert_eq!(
        (hamza.adv_w, hamza.box_w, hamza.box_h, hamza.ofs_x, hamza.ofs_y),
        (8, 6, 8, 1, 1)
    );
    // U+FE76 (Arabic presentation forms B), cmap 10 (from 65142 at gid 461) -> id 461:
    // {.bitmap_index = 27988, .adv_w = 75, .box_w = 5, .box_h = 3, .ofs_x = 0, .ofs_y = 9}
    let fatha = dsc('\u{FE76}');
    assert_eq!(fatha.gid, 461);
    assert_eq!(
        (fatha.adv_w, fatha.box_w, fatha.box_h, fatha.ofs_x, fatha.ofs_y),
        (5, 5, 3, 0, 9)
    );
    // U+FEFF, the last cmap -> id 596 (the last glyph).
    assert_eq!(dsc('\u{FEFF}').gid, 596);
    // Not in any range.
    let mut d = GlyphDsc::default();
    assert!(!DEJAVU_16_PERSIAN_HEBREW.glyph_dsc(&mut d, 'é' as u32, 0));
}
