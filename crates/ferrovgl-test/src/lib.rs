//! Test harness mirroring LVGL's `src/debugging/test`: an 800x480 XRGB8888
//! headless display in direct render mode, synthetic pointer/keypad/encoder
//! devices and byte-exact screenshot comparison against upstream reference
//! images (`tests/upstream/lvgl/tests/ref_imgs`).

#![warn(missing_docs)]

/// Horizontal resolution of the upstream test display.
pub const DISPLAY_HOR_RES: i32 = 800;
/// Vertical resolution of the upstream test display.
pub const DISPLAY_VER_RES: i32 = 480;
/// DPI used by the upstream 32-bit test configuration.
pub const DISPLAY_DPI: i32 = 160;
/// Path of the upstream sparse checkout relative to the workspace root.
pub const UPSTREAM_DIR: &str = "tests/upstream/lvgl";
