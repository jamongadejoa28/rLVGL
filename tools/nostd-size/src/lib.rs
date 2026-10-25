//! Host-testable parts of the no_std size probe: the global allocator.
//!
//! The probe binary (`src/main.rs`) only builds for bare-metal ARM; this
//! library builds everywhere, so `cargo test --lib` in `tools/nostd-size`
//! (run by `cargo xtask size-report`) checks the allocator on the host.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks, missing_docs)]

#[cfg(test)]
extern crate std;

pub mod heap;
