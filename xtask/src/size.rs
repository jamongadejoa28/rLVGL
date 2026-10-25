//! `cargo xtask size-report`: builds the no_std size probe in
//! `tools/nostd-size` for Cortex-M targets and reports the `.text` size.

use std::path::Path;

use crate::Result;

/// Builds the size probe and prints (or checks) its section sizes.
pub fn size_report(_root: &Path, _args: &[&str]) -> Result<()> {
    Err("size-report is not implemented yet".into())
}
