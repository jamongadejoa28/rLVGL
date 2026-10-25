//! ferrovgl: a pure-Rust reimplementation of LVGL v9.6.0.
//!
//! This umbrella crate re-exports the workspace crates behind feature flags.
//! Start with [`prelude`].
//!
//! ferrovgl is not affiliated with LVGL LLC. LVGL is a trademark of LVGL LLC.

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

pub use ferrovgl_core as core;
#[cfg(feature = "draw-sw")]
#[cfg_attr(docsrs, doc(cfg(feature = "draw-sw")))]
pub use ferrovgl_draw_sw as draw_sw;
#[cfg(feature = "embedded")]
#[cfg_attr(docsrs, doc(cfg(feature = "embedded")))]
pub use ferrovgl_embedded as embedded;
#[cfg(feature = "fonts")]
#[cfg_attr(docsrs, doc(cfg(feature = "fonts")))]
pub use ferrovgl_fonts as fonts;
#[cfg(feature = "sdl")]
#[cfg_attr(docsrs, doc(cfg(feature = "sdl")))]
pub use ferrovgl_sdl as sdl;
#[cfg(feature = "widgets")]
#[cfg_attr(docsrs, doc(cfg(feature = "widgets")))]
pub use ferrovgl_widgets as widgets;

/// Commonly used types.
pub mod prelude {
    pub use ferrovgl_core::{Align, Area, Color, Color32, ColorFormat, Dir, Opa, Point};
    #[cfg(feature = "widgets")]
    pub use ferrovgl_widgets::{
        Bar, BarMode, BarOrientation, Button, Checkbox, Image, ImageAlign, Label, LabelLongMode, Slider,
        SliderMode, SliderOrientation, Switch, SwitchOrientation,
    };
}
