//! Browser backend for ferrovgl: renders a display into a `<canvas>` through
//! `ImageData`/`putImageData` and feeds pointer, keyboard and wheel events
//! into LVGL input devices.
//!
//! The crate is split so that everything except the thin DOM glue runs on
//! the host under `cargo test`:
//!
//! - [`backend`]: the [`Ui`](ferrovgl_core::Ui), display, flush driver,
//!   input devices and frame step, drawing through a [`backend::Surface`].
//! - [`convert`]: flushed area (XRGB8888 and friends) to RGBA conversion,
//!   following the display's render mode.
//! - [`geometry`]: CSS sizing for `devicePixelRatio`, pointer coordinate
//!   mapping with `getBoundingClientRect()`, wheel steps.
//! - [`keys`]: `KeyboardEvent` to `LV_KEY_*` / UTF-8 key mapping.
//! - [`clock`]: `performance.now()` to `lv_tick_inc()` bookkeeping.
//! - [`gallery`]: the demo scenes (widget gallery and ported examples).
//! - [`web`]: the wasm-bindgen API ([`web::WebDisplay`]).

#![warn(missing_docs)]

pub mod backend;
pub mod clock;
pub mod convert;
pub mod gallery;
pub mod geometry;
pub mod input;
pub mod keys;
pub mod web;

pub use backend::{Backend, BackendConfig, Surface};
pub use input::InputEvent;
pub use web::WebDisplay;

/// Canvas element id the demo page looks for.
pub const DEFAULT_CANVAS_ID: &str = "ferrovgl";
