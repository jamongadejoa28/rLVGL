# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/jamongadejoa28/rLVGL/releases/tag/ferrovgl-draw-sw-v0.1.0) - 2026-10-25

### Added

- M1 integration, M2 input/layout/primitives, label/button/image widgets
- *(image)* port lv_draw_image.c, lv_draw_sw_img.c
- *(transform)* port lv_draw_sw_transform.c
- *(sw-unit)* port lv_draw_sw.c, lv_draw_sw_blend_to_rgb565_swapped.c
- *(sw-shadow-maskrect)* port lv_draw_sw_box_shadow.c, lv_draw_sw_mask_rect.c
- *(sw-fill-border-letter)* port lv_draw_sw_fill.c, lv_draw_sw_border.c, lv_draw_sw_letter.c
- *(blend-small)* port lv_draw_sw_blend_to_l8.c, lv_draw_sw_blend_to_al88.c, lv_draw_sw_blend_to_a8.c, lv_draw_sw_blend_to_i1.c
- *(blend-rgb888)* port lv_draw_sw_blend.c, lv_draw_sw_blend_to_rgb888.c
- *(blend-rgb565)* port lv_draw_sw_blend_to_rgb565.c
- *(blend-argb8888)* port lv_draw_sw_blend_to_argb8888.c, lv_draw_sw_blend_to_argb8888_premultiplied.c
- merge M1-A ports (style, text, timer/tick, grad, mask, draw-core)
- *(core)* M1 type skeleton from LVGL v9.6.0 headers
- M0 scaffolding, misc ports and font pipeline

### Fixed

- *(mask)* apply M1-A fidelity audit
- *(grad)* apply M1-A fidelity audit
- *(sw-unit)* apply fidelity review
- *(sw-shadow-maskrect)* apply fidelity review
- *(sw-fill-border-letter)* apply fidelity review

### Other

- bar/slider/switch/checkbox widgets and M1-A fidelity audit fixes
- M1-B ports (obj core, display, refr, lv_init, sw draw unit)
- M1-A rerun ports (event, draw-buf, blend x4, anim/palette, draw rect/label/mask)
