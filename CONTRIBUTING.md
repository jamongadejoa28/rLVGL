# Contributing to ferrovgl

Thank you for helping. A few rules keep the project coherent.

## Ground rules

1. **Port, do not improve.** Renderer, mask, gradient, shadow, text-breaking,
   layout and style-lookup code is ported line-for-line from LVGL v9.6.0,
   including its rounding. Pixel parity with upstream reference images is the
   project's headline metric. Behavioural "improvements" belong in a separate
   discussion first.
2. **Only Rust ownership is redesigned.** Keep LVGL's data model, names and
   numeric enum values (`StyleProp`, `EventCode`, `ColorFormat`, parts, states)
   so the LVGL documentation keeps applying.
3. **`unsafe` is forbidden** outside `draw_buf::alloc` and
   `draw_sw::blend::words`. The workspace denies `unsafe_code`.
4. **Every widget PR ships a parity test** that reproduces the matching
   upstream `tests/src/test_cases/test_<widget>.c` scenario and compares
   against `tests/ref_imgs/...`.
5. **Core crates stay `#![no_std]`** (`alloc` allowed). CI builds them for
   `thumbv7em-none-eabihf`.

## Workflow

- Commits follow [Conventional Commits](https://www.conventionalcommits.org)
  (`feat(core): ...`, `fix(draw-sw): ...`). release-plz derives versions and
  changelogs from them.
- Sign off your commits (`git commit -s`, Developer Certificate of Origin).
- Set up the project-local development environment with `uv sync --locked`.
  Run development commands through `uv run --locked`; Python tooling belongs
  in the `dev` dependency group (`uv add --dev`). Keep `.venv/` untracked and
  preserve both `uv.lock` and the native Rust `Cargo.lock`.
- Before pushing: `uv run --locked cargo fmt --all --check`,
  `uv run --locked cargo clippy --locked --workspace --all-targets
  --all-features -- -D warnings`,
  `uv run --locked cargo test --locked --workspace --all-features`,
  `uv run --locked cargo xtask nostd-check`.
- Reference images come from upstream: `uv run --locked cargo xtask refimgs sync` creates a
  sparse checkout of LVGL v9.6.0 in `tests/upstream/` (git-ignored).

## Porting a file

Add a header comment to the Rust file:

```rust
//! Ported from lvgl v9.6.0: src/draw/sw/lv_draw_sw_fill.c
```

Keep function order close to upstream so diffs against future LVGL releases
remain reviewable.

## Build cache and disk use

The workspace defaults to `debug = 0` for development, tests and release builds.
Development and tests retain incremental compilation and Cargo's normal parallel
code generation; external dependencies keep the existing `opt-level = 2` setting.
Debug assertions and overflow checks are unchanged. This removes DWARF from each
feature/target variant without discarding the incremental cache on every build.
See [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html).

Use the same profile and feature set for repeated work. Focus a workspace test by
name with `uv run --locked cargo test --locked --workspace --all-features FILTER`
when practical; feature/profile/RUSTFLAGS changes create separate cache variants.
Avoid routine `cargo clean` and `CARGO_INCREMENTAL=0`: both sacrifice useful
recompilation state. The initial migration removed old build directories once,
while preserving `target/parity`, preview images, and documentation.

Source-level debugging is an explicit opt-in, with separate artifacts:

```sh
uv run --locked cargo build --locked --profile dev-debug -p ferrovgl-sdl --example interactive
```

This profile restores full debug information under `target/dev-debug`. The default
profile lacks source/variable debug information; function symbols are not stripped.
Delete only that optional profile's generated directory when it is no longer needed.

Fuzzing uses an independent locked workspace and `target/fuzz`, retaining
incremental builds with 16 codegen units and line-table debug information.
The core development cache remains in `target/debug`; see [fuzz/README.md](fuzz/README.md)
for the local native-tool installation and bounded campaigns. The tool installer
uses `target/tool-build`, which may be removed after installation without
affecting ordinary or fuzz recompilation; keep `target/tools/bin/cargo-fuzz`.
