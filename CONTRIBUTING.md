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
- Before pushing: `cargo fmt --all`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, `cargo test --workspace`,
  `cargo xtask nostd-check`.
- Reference images come from upstream: `cargo xtask refimgs sync` creates a
  sparse checkout of LVGL v9.6.0 in `tests/upstream/` (git-ignored).

## Porting a file

Add a header comment to the Rust file:

```rust
//! Ported from lvgl v9.6.0: src/draw/sw/lv_draw_sw_fill.c
```

Keep function order close to upstream so diffs against future LVGL releases
remain reviewable.
