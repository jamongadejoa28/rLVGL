//! Ports of the LVGL v9.6.0 `lv_example_*` functions (`examples/` in the
//! upstream repository). Each example builds its UI on the active screen of
//! the default display, exactly like the C function, so the SDL and browser
//! demos and the headless smoke tests can all run the same code.

#![no_std]
#![warn(missing_docs)]

#[allow(unused_extern_crates)] // used once examples land
extern crate alloc;

use ferrovgl_core::Ui;

/// One runnable example.
#[derive(Clone, Copy)]
pub struct Example {
    /// Upstream function name, e.g. `lv_example_get_started_1`.
    pub name: &'static str,
    /// Builds the example on the default display's active screen.
    pub run: fn(&mut Ui),
}

/// Every ported example, in upstream `lv_examples.h` order.
pub const EXAMPLES: &[Example] = &[];

/// Looks up an example by its upstream name (with or without the `lv_example_` prefix).
pub fn find(name: &str) -> Option<&'static Example> {
    let short = name.strip_prefix("lv_example_").unwrap_or(name);
    EXAMPLES
        .iter()
        .find(|e| e.name.strip_prefix("lv_example_").unwrap_or(e.name) == short)
}
