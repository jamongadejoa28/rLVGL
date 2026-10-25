//! `cargo xtask gen-mapping [--check]`: generates `docs/src/mapping.md`, the
//! LVGL C API -> ferrovgl table, from the upstream public headers and the
//! upstream function names cited in ferrovgl doc comments.

use std::path::Path;

use crate::Result;

/// Regenerates (or, with `check`, verifies) `docs/src/mapping.md`.
pub fn gen_mapping(_root: &Path, _check: bool) -> Result<()> {
    Err("gen-mapping is not implemented yet".into())
}
