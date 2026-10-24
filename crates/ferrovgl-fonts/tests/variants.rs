//! Compressed and 1-bpp fonts decode and agree with their plain siblings.
//!
//! Needs the optional fonts: run with `--all-features` (CI does).

#![cfg(all(
    feature = "montserrat-28",
    feature = "montserrat-28-compressed",
    feature = "unscii-8"
))]

use ferrovgl_core::font::{Font, GlyphDsc};
use ferrovgl_fonts::{MONTSERRAT_28, MONTSERRAT_28_COMPRESSED, UNSCII_8};

fn render(font: &dyn Font, letter: char) -> (GlyphDsc, Vec<u8>) {
    let mut d = GlyphDsc::default();
    assert!(font.glyph_dsc(&mut d, letter as u32, 0));
    let w = d.box_w as usize;
    let h = d.box_h as usize;
    let mut out = vec![0u8; w * h];
    if w * h > 0 {
        assert!(font.glyph_bitmap_a8(&d, &mut out, w));
    }
    (d, out)
}

#[test]
fn compressed_matches_plain_montserrat_28() {
    assert!(MONTSERRAT_28_COMPRESSED.is_compressed());
    assert!(!MONTSERRAT_28_COMPRESSED.has_static_bitmap());
    for ch in ['A', 'g', '!', '@', 'W', '0', '\u{F00C}'] {
        let (dp, plain) = render(&MONTSERRAT_28, ch);
        let (dc, comp) = render(&MONTSERRAT_28_COMPRESSED, ch);
        assert_eq!(
            (dp.box_w, dp.box_h, dp.adv_w),
            (dc.box_w, dc.box_h, dc.adv_w),
            "{ch}"
        );
        assert_eq!(
            plain, comp,
            "bitmap of {ch:?} differs between plain and compressed"
        );
    }
}

#[test]
fn unscii_8_is_monochrome() {
    assert_eq!(UNSCII_8.bpp(), 1);
    let (d, bits) = render(&UNSCII_8, 'A');
    assert_eq!(d.adv_w, 8);
    assert!(bits.iter().all(|&v| v == 0 || v == 255));
    assert!(bits.contains(&255));
}
