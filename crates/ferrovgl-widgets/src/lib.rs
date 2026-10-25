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
    feature = "arc",
    feature = "line",
    feature = "led",
    feature = "spinner",
    feature = "buttonmatrix",
    feature = "textarea",
    feature = "theme-default",
    feature = "theme-simple"
))]
extern crate alloc;

#[cfg(feature = "arc")]
pub mod arc;
#[cfg(feature = "arc")]
pub use arc::{ARC_CLASS, Arc, ArcMode};

#[cfg(feature = "bar")]
pub mod bar;
#[cfg(feature = "bar")]
pub use bar::{BAR_CLASS, Bar, BarMode, BarOrientation};

#[cfg(feature = "button")]
pub mod button;
#[cfg(feature = "button")]
pub use button::{BUTTON_CLASS, Button};

#[cfg(feature = "buttonmatrix")]
pub mod buttonmatrix;
#[cfg(feature = "buttonmatrix")]
pub use buttonmatrix::{
    BUTTONMATRIX_BUTTON_NONE, BUTTONMATRIX_CLASS, Buttonmatrix, ButtonmatrixCtrl, ButtonmatrixMap,
};

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

#[cfg(feature = "led")]
pub mod led;
#[cfg(feature = "led")]
pub use led::{LED_BRIGHT_MAX, LED_BRIGHT_MIN, LED_CLASS, Led};

#[cfg(feature = "line")]
pub mod line;
#[cfg(feature = "line")]
pub use line::{LINE_CLASS, Line, LinePoints, LinePointsMut};

#[cfg(feature = "observer")]
pub mod observer;

#[cfg(feature = "slider")]
pub mod slider;
#[cfg(feature = "slider")]
pub use slider::{SLIDER_CLASS, Slider, SliderMode, SliderOrientation};

#[cfg(feature = "spinner")]
pub mod spinner;
#[cfg(feature = "spinner")]
pub use spinner::{SPINNER_CLASS, Spinner};

#[cfg(feature = "switch")]
pub mod switch;
#[cfg(feature = "switch")]
pub use switch::{SWITCH_CLASS, Switch, SwitchOrientation};

#[cfg(feature = "textarea")]
pub mod textarea;
#[cfg(feature = "textarea")]
pub use textarea::{PART_TEXTAREA_PLACEHOLDER, TEXTAREA_CLASS, TEXTAREA_CURSOR_LAST, Textarea};

pub mod theme;
