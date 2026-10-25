# Fuzzing

Three libFuzzer targets exercise the pure Rust core with AddressSanitizer,
coverage-guided mutations, debug assertions and overflow checks:

| Target | Inputs and assertions |
|---|---|
| `style_ops` | Set/remove/copy/merge/reset against an independent property map; shared styles, inheritance, PRESSED selector and invalidated lookup caches |
| `text_break` | Malformed raw UTF-8 decoding; valid UTF-8 wrapping with recolor/spacing flags; progress, bounds, character boundaries and insert/cut round trips |
| `tree_mutation` | Create/delete/reparent/swap/reorder/clean/async-delete sequences; reciprocal parent/child links, live handles, acyclicity and teardown |

Inputs are bounded to 1,024 bytes / 256 operations and at most 64 application
objects. Tree requests exclude cycles and ancestor swaps, which violate the C
API's caller preconditions. These targets are invariant checks, not a C parity
oracle. Passing a short run does not establish exhaustive safety or coverage.

Install the native tool locally from the workspace root. Nightly is selected
only for fuzz commands; the project's default toolchain remains stable. The
setup follows the [Rust Fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz/setup.html).

```sh
uv sync --locked
uv run --locked rustup toolchain install nightly --profile minimal
uv run --locked cargo install cargo-fuzz --version 0.13.2 --locked --root target/tools --target-dir target/tool-build
uv run --locked python tools/fuzz.py all --seconds 30
```

Run one target longer with `uv run --locked python tools/fuzz.py text_break
--seconds 600`. The weekly/manual GitHub workflow runs each target for ten
minutes and preserves its generated corpus and failure artifacts. Adding the
workflow locally does not mean a remote CI campaign has run.

`fuzz/Cargo.lock` pins the separate fuzz workspace, including libfuzzer-sys;
production dependencies and the root lockfile remain independent. cargo-fuzz
0.13.2 has no `--locked` passthrough, so the launcher first validates and fetches
with `cargo fetch --locked`, disables network access for the build/run, and
verifies that the lockfile did not change. Use `cargo update` explicitly when
updating fuzz dependencies, through `uv run --locked` as elsewhere.

`target/fuzz` isolates sanitizer artifacts from normal incremental builds.
The fuzz profile uses opt-level 1, incremental compilation and 16 codegen units.
cargo-fuzz retains only line-table debug information for useful crash traces;
normal dev/test builds still use debug=0. Tool installation artifacts are under
`target/tool-build`, and the small launcher binary under `target/tools`.
The nightly compiler itself is installed by rustup outside `target`.

Checked-in `seeds/` provide initial cases. Generated `corpus/` and `artifacts/`
are ignored, retained locally, and never overwrite seeds. A crash should be
minimized, fixed and turned into an ordinary regression test plus a new seed.
To replay a failure with the same instrumented binary after a build:

```sh
uv run --locked target/fuzz/x86_64-unknown-linux-gnu/release/text_break fuzz/artifacts/text_break/crash-HASH
```

The replay path is target-triple dependent. To format the separate workspace:
`uv run --locked cargo fmt --manifest-path fuzz/Cargo.toml --package ferrovgl-fuzz --check`.
