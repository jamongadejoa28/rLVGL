//! Core of ferrovgl, a pure-Rust reimplementation of LVGL v9.6.0.
//!
//! This crate is `#![no_std]` and only requires `alloc`. It contains the
//! object tree, the style system, events, layouts, the draw model (buffers,
//! layers, tasks, the `DrawUnit` trait), fonts, input devices, timers and
//! animations. Rendering itself lives in `ferrovgl-draw-sw`.
//!
//! The data model, names and numeric enum values follow LVGL v9.6.0 so that
//! the upstream documentation keeps applying. Only ownership is redesigned:
//! objects live in an arena inside the `Ui` context and are addressed through
//! `Copy` handles.
//!
//! ferrovgl is not affiliated with LVGL LLC. LVGL is a trademark of LVGL LLC.

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

extern crate alloc;

pub mod font;
pub mod misc;

pub use font::{Font, GlyphDsc, GlyphFormat};
pub use misc::area::{Align, Area, Dir, Point, PointPrecise};
pub use misc::color::{Color, Color16, Color32, ColorFormat, Opa};
pub use misc::types::{ValuePrecise, coord};

/// Version of LVGL whose semantics this crate reimplements.
pub const LVGL_VERSION: &str = "9.6.0";
