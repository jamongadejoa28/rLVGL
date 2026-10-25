//! `examples/assets`: the image assets small enough to port verbatim.
//!
//! Only `img_example_lvgl_logo` (9 KiB) is ported; the other upstream assets
//! (`img_transform_avatar_15`, `img_bar_*`, `img_arc_*`, `animimg00*`, ...)
//! are hundreds of KiB of C arrays and the examples using them are not ported.

mod img_example_lvgl_logo;

pub use img_example_lvgl_logo::IMG_EXAMPLE_LVGL_LOGO;
