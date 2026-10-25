//! Puts `link.x` on the linker search path and links the probe with it.

use std::path::PathBuf;
use std::{env, fs};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    fs::copy("link.x", out.join("link.x")).expect("copy link.x into OUT_DIR");
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    // rustc already passes this for executables; keep it explicit because the
    // measured `.text` must only contain what is reachable from `Reset`.
    println!("cargo:rustc-link-arg-bins=--gc-sections");
    println!("cargo:rerun-if-changed=link.x");
    println!("cargo:rerun-if-changed=build.rs");
}
