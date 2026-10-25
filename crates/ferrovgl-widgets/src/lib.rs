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
    feature = "theme-default",
    feature = "theme-simple"
))]
extern crate alloc;

#[cfg(feature = "button")]
pub mod button;
#[cfg(feature = "button")]
pub use button::{BUTTON_CLASS, Button};

#[cfg(feature = "image")]
pub mod image;
#[cfg(feature = "image")]
pub use image::{IMAGE_CLASS, Image, ImageAlign};

#[cfg(feature = "label")]
pub mod label;
#[cfg(feature = "label")]
pub use label::{LABEL_CLASS, Label, LabelLongMode};

pub mod theme;
