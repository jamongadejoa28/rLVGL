//! Browser backend for ferrovgl: renders a display into a `<canvas>` through
//! `ImageData` and feeds pointer and keyboard events into input devices.

#![warn(missing_docs)]

/// Canvas element id the demo page looks for.
pub const DEFAULT_CANVAS_ID: &str = "ferrovgl";
