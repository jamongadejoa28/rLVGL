//! Byte slice <-> `u16` / `u32` word views for the blend kernels.
//!
//! This is the only module of the crate (and one of two in the workspace)
//! allowed to contain `unsafe`. The skeleton has none yet; when the views
//! are added the module gets `#![allow(unsafe_code)]` and every `unsafe`
//! block a `// SAFETY:` comment, and the functions are verified under Miri.
//! Everything else in the renderer uses `chunks_exact` and `from_le_bytes`.
