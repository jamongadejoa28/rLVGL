//! Built-in fonts for ferrovgl.
//!
//! The glyph tables are converted from the C arrays shipped with LVGL v9.6.0
//! (`src/font/lv_font_*.c`) into little-endian table blobs in `data/` by
//! `cargo xtask fonts`. They are read in place with `include_bytes!`, so a
//! font costs no RAM.
//!
//! Montserrat is licensed under the SIL Open Font License 1.1 (`OFL.txt`).

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

pub use ferrovgl_core::font::{FmtTxtFont, Font};

/// Upstream commit the font blobs were generated from.
pub const UPSTREAM_COMMIT: &str = "80ca777e37a2b176770726a02e07a6fb79ef0b39";

macro_rules! builtin_font {
    ($(#[$meta:meta])* $feature:literal, $name:ident, $file:literal) => {
        $(#[$meta])*
        #[cfg(feature = $feature)]
        #[cfg_attr(docsrs, doc(cfg(feature = $feature)))]
        pub static $name: FmtTxtFont<'static> =
            FmtTxtFont::from_bytes_const(include_bytes!(concat!("../data/", $file, ".lvfnt")));
    };
}

builtin_font!(/// `lv_font_montserrat_8`
    "montserrat-8", MONTSERRAT_8, "montserrat_8");
builtin_font!(/// `lv_font_montserrat_10`
    "montserrat-10", MONTSERRAT_10, "montserrat_10");
builtin_font!(/// `lv_font_montserrat_12`
    "montserrat-12", MONTSERRAT_12, "montserrat_12");
builtin_font!(/// `lv_font_montserrat_14`
    "montserrat-14", MONTSERRAT_14, "montserrat_14");
builtin_font!(/// `lv_font_montserrat_16`
    "montserrat-16", MONTSERRAT_16, "montserrat_16");
builtin_font!(/// `lv_font_montserrat_18`
    "montserrat-18", MONTSERRAT_18, "montserrat_18");
builtin_font!(/// `lv_font_montserrat_20`
    "montserrat-20", MONTSERRAT_20, "montserrat_20");
builtin_font!(/// `lv_font_montserrat_22`
    "montserrat-22", MONTSERRAT_22, "montserrat_22");
builtin_font!(/// `lv_font_montserrat_24`
    "montserrat-24", MONTSERRAT_24, "montserrat_24");
builtin_font!(/// `lv_font_montserrat_26`
    "montserrat-26", MONTSERRAT_26, "montserrat_26");
builtin_font!(/// `lv_font_montserrat_28`
    "montserrat-28", MONTSERRAT_28, "montserrat_28");
builtin_font!(/// `lv_font_montserrat_30`
    "montserrat-30", MONTSERRAT_30, "montserrat_30");
builtin_font!(/// `lv_font_montserrat_32`
    "montserrat-32", MONTSERRAT_32, "montserrat_32");
builtin_font!(/// `lv_font_montserrat_34`
    "montserrat-34", MONTSERRAT_34, "montserrat_34");
builtin_font!(/// `lv_font_montserrat_36`
    "montserrat-36", MONTSERRAT_36, "montserrat_36");
builtin_font!(/// `lv_font_montserrat_38`
    "montserrat-38", MONTSERRAT_38, "montserrat_38");
builtin_font!(/// `lv_font_montserrat_40`
    "montserrat-40", MONTSERRAT_40, "montserrat_40");
builtin_font!(/// `lv_font_montserrat_42`
    "montserrat-42", MONTSERRAT_42, "montserrat_42");
builtin_font!(/// `lv_font_montserrat_44`
    "montserrat-44", MONTSERRAT_44, "montserrat_44");
builtin_font!(/// `lv_font_montserrat_46`
    "montserrat-46", MONTSERRAT_46, "montserrat_46");
builtin_font!(/// `lv_font_montserrat_48`
    "montserrat-48", MONTSERRAT_48, "montserrat_48");
builtin_font!(/// `lv_font_montserrat_28_compressed` (RLE compressed bitmaps)
    "montserrat-28-compressed", MONTSERRAT_28_COMPRESSED, "montserrat_28_compressed");
builtin_font!(/// `lv_font_unscii_8` (1 bpp)
    "unscii-8", UNSCII_8, "unscii_8");
builtin_font!(/// `lv_font_unscii_16` (1 bpp)
    "unscii-16", UNSCII_16, "unscii_16");

/// `LV_FONT_DEFAULT`: the font used when none is set.
#[cfg(feature = "default-montserrat-14")]
pub static FONT_DEFAULT: &FmtTxtFont<'static> = &MONTSERRAT_14;
