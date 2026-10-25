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

## Releases

The published crates are `ferrovgl`, `ferrovgl-core`, `ferrovgl-draw-sw`,
`ferrovgl-fonts`, `ferrovgl-widgets`, `ferrovgl-sdl`, `ferrovgl-wasm`,
`ferrovgl-embedded` (a placeholder until 0.2, published because the umbrella's
`embedded` feature depends on it) and `ferrovgl-examples` (a normal dependency
of `ferrovgl-wasm`). `ferrovgl-test`, `xtask` and `font-conv` are
`publish = false`. All published crates share the workspace version and are
released together (`version_group = "ferrovgl"` in `release-plz.toml`).

There is one changelog, [CHANGELOG.md](CHANGELOG.md) (Keep a Changelog). It is
attached to the umbrella crate, which includes the commits of every other
published crate; the member crates have no changelog of their own.

Publishing is manual (`.github/workflows/release.yml` only runs on
`workflow_dispatch`) and irreversible. Before a release, check the packages
locally:

```sh
uv run --locked cargo package --workspace --locked --list
uv run --locked cargo publish --workspace --dry-run --locked
```

`cargo publish --workspace` orders the crates by their dependencies:
`ferrovgl-core`, `ferrovgl-draw-sw`, `ferrovgl-embedded`, `ferrovgl-fonts`,
`ferrovgl-widgets`, `ferrovgl-examples`, `ferrovgl-sdl`, `ferrovgl-wasm`,
`ferrovgl`. Every package carries `LICENSE-APACHE`, `LICENSE-MIT`, `NOTICE`
(the LVGL MIT notice, the MIT notices of the third-party code LVGL ported and
a summary of the font licences) and `TRADEMARK.md` as symlinks to the
repository root. `ferrovgl-fonts` also carries `OFL.txt`, `LICENSE-DejaVu.txt`
and `data/SOURCE.md`, and declares
`(MIT OR Apache-2.0) AND OFL-1.1 AND Bitstream-Vera` because of its font data.
A dev-dependency on `ferrovgl-test` must be path-only (no `version`) so that
`cargo publish` strips it. A package does not ship test targets that need
`ferrovgl-test` (`ferrovgl-examples` leaves out `tests/`).

### 0.1.0

The 0.1.0 entry of `CHANGELOG.md` is written by hand and stays dated
"Unreleased" until the publish; set the release date in the commit that
publishes. Publish 0.1.0 with `release-plz release`, which publishes every
crate whose version is not on crates.io yet and creates the
`ferrovgl-v0.1.0` GitHub release from that changelog section. Do not run
`release-plz release-pr` before 0.1.0 is on crates.io: without a registry
baseline it would turn the whole history into changelog entries.

The release workflow (`.github/workflows/release.yml`) runs
`release-plz/action` without a `command`, so one dispatch runs
`release-plz release` and then `release-plz release-pr` straight away, while
the crates.io index may still be catching up on the crates just published.
Until the workflow sets `command: release` (with `release-pr` as a separate
job once 0.1.0 is the baseline), run `release-plz release` for 0.1.0 locally
with a crates.io token instead of dispatching the workflow, and dispatch the
workflow only after the 0.1.0 crates are visible on crates.io. Close a release
PR that `release-pr` opens against an incomplete baseline instead of merging
it.

### Semver checks after 0.1.0

There is no published baseline before 0.1.0, so no semver check applies to it.
From the first release-plz PR after 0.1.0 on, `semver_check = true` makes
release-plz run [cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks)
against the latest crates.io version of each crate and choose the version bump
from the result: a breaking change bumps the minor version while the crates
are `0.x` (0.1.x -> 0.2.0), anything else bumps the patch version. Because
the crates share a version group, a breaking change in one crate bumps all of
them. Mark breaking commits with `!` or a `BREAKING CHANGE:` footer so the
changelog says why.

To check a change locally before opening a PR (needs `cargo install
cargo-semver-checks --locked`; the baseline is downloaded from crates.io):

```sh
cargo semver-checks --workspace --exclude ferrovgl-test --exclude xtask --exclude font-conv
```

`cargo semver-checks` compares the default feature set; add
`--all-features` for the feature-gated API (the widgets, renderer colour
formats and fonts are all behind features).
