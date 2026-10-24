//! Software renderer for ferrovgl.
//!
//! Port of LVGL v9.6.0 `src/draw/sw`. Each colour format is behind a `cf-*`
//! feature so that unused blend paths are not compiled on small targets.
//!
//! The only `unsafe` code of this crate lives in `blend::words`.

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

/// Name reported by the software draw unit.
pub const DRAW_UNIT_NAME: &str = "SW";
