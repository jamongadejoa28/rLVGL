# ferrovgl-fonts

Built-in bitmap fonts for ferrovgl, converted from the glyph tables shipped
with LVGL v9.6.0 (`src/font/lv_font_*.c`, commit
`80ca777e37a2b176770726a02e07a6fb79ef0b39`). The tables are stored as
`.lvfnt` blobs in `data/` and read in place with `include_bytes!`, so a font
costs flash but no RAM. `#![no_std]`, no allocation.

Part of [ferrovgl](https://github.com/jamongadejoa28/rLVGL) (independent project, not affiliated with
LVGL LLC). API documentation: <https://docs.rs/ferrovgl-fonts>.

Every font is behind its own feature; only enabled fonts are compiled into
the binary:

| feature | font |
|---|---|
| `montserrat-8` ... `montserrat-48` (even sizes) | Montserrat Medium, 4 bpp, ASCII + `°` + `•` + LVGL symbols |
| `montserrat-28-compressed` | the compressed Montserrat 28 variant |
| `unscii-8`, `unscii-16` | UNSCII |
| `dejavu-16-persian-hebrew` | DejaVu Sans 16 with Hebrew/Arabic/Persian ranges |
| `default-montserrat-14` (default) | enables Montserrat 14, the LVGL default font |

```rust,ignore
use ferrovgl_core::{FontRef, UiConfig};

let config = UiConfig {
    font_default: Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14)),
    ..UiConfig::default()
};
```

## Font licences

The font data is not covered by the ferrovgl code licence alone:

- Montserrat: SIL Open Font License 1.1, [`OFL.txt`](OFL.txt) (shipped in
  this package).
- Font Awesome 5 Free (the LVGL symbol glyphs): SIL Open Font License 1.1.
- DejaVu Sans: Bitstream Vera Fonts licence, DejaVu changes in the public
  domain (<https://dejavu-fonts.github.io/License.html>).
- UNSCII: public domain.

Provenance and the exact conversion options are recorded in
[`data/SOURCE.md`](data/SOURCE.md) (also shipped in this package).

## About ferrovgl

This crate is part of [ferrovgl](https://github.com/jamongadejoa28/rLVGL), a pure-Rust reimplementation of
LVGL v9.6.0. ferrovgl is an independent project and is **not** affiliated
with, endorsed by, or sponsored by LVGL LLC or LVGL Kft. "LVGL" is a trademark
of LVGL LLC, used here only to describe compatibility and provenance (see
[TRADEMARK.md](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/TRADEMARK.md)).

- Repository: <https://github.com/jamongadejoa28/rLVGL>
- Changelog: [CHANGELOG.md](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/CHANGELOG.md)
- Minimum supported Rust version: 1.97

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/LICENSE-APACHE)
or [MIT license](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/LICENSE-MIT) at your option. ferrovgl is derived from
LVGL (MIT, Copyright LVGL Kft); the upstream notice is reproduced in
[NOTICE](https://github.com/jamongadejoa28/rLVGL/blob/HEAD/NOTICE), which ships in this package.
