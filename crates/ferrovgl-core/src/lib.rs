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

pub mod core;
pub mod display;
pub mod draw;
pub mod font;
pub mod group;
pub mod indev;
pub mod layouts;
pub mod misc;
pub mod theme;

pub use core::{AsyncJob, Obj, ObjClass, ObjData, ObjFlags, ObjId, Ui, UiConfig, WidgetState};
pub use display::{DisplayDriver, DisplayId, RenderMode, Rotation, ScreenLoadAnim};
pub use draw::{DrawCtx, DrawTask, DrawTaskRef, DrawTaskType, DrawUnit, Layer, LayerId};
pub use font::{Font, FontRef, GlyphDsc, GlyphFormat};
pub use group::GroupId;
pub use indev::{IndevDriver, IndevId, IndevReadData, IndevType};
pub use misc::anim::{Anim, AnimId, AnimPath};
pub use misc::area::{Align, Area, Dir, Point, PointPrecise};
pub use misc::color::{Color, Color16, Color32, ColorFormat, Opa};
pub use misc::event::{Event, EventCode, EventParam, LvResult};
pub use misc::style::{Part, State, Style, StyleProp, StyleSelector, StyleValue};
pub use misc::timer::TimerId;
pub use misc::types::{ValuePrecise, coord};
pub use theme::ThemeId;

/// Version of LVGL whose semantics this crate reimplements.
pub const LVGL_VERSION: &str = "9.6.0";
