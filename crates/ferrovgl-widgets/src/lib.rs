//! Widgets and themes for ferrovgl.
//!
//! Each widget directory mirrors LVGL v9.6.0 `src/widgets/<name>`.

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

#[cfg(any(
    feature = "label",
    feature = "button",
    feature = "image",
    feature = "bar",
    feature = "slider",
    feature = "switch",
    feature = "checkbox",
    feature = "theme-default",
    feature = "theme-simple"
))]
extern crate alloc;

#[cfg(feature = "bar")]
pub mod bar;
#[cfg(feature = "bar")]
pub use bar::{BAR_CLASS, Bar, BarMode, BarOrientation};

#[cfg(feature = "button")]
pub mod button;
#[cfg(feature = "button")]
pub use button::{BUTTON_CLASS, Button};

#[cfg(feature = "checkbox")]
pub mod checkbox;
#[cfg(feature = "checkbox")]
pub use checkbox::{CHECKBOX_CLASS, Checkbox};

#[cfg(feature = "image")]
pub mod image;
#[cfg(feature = "image")]
pub use image::{IMAGE_CLASS, Image, ImageAlign};

#[cfg(feature = "label")]
pub mod label;
#[cfg(feature = "label")]
pub use label::{LABEL_CLASS, Label, LabelLongMode};

#[cfg(feature = "slider")]
pub mod slider;
#[cfg(feature = "slider")]
pub use slider::{SLIDER_CLASS, Slider, SliderMode, SliderOrientation};

#[cfg(feature = "switch")]
pub mod switch;
#[cfg(feature = "switch")]
pub use switch::{SWITCH_CLASS, Switch, SwitchOrientation};

pub mod theme;
