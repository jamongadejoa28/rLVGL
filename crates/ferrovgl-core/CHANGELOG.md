# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/jamongadejoa28/rLVGL/releases/tag/ferrovgl-core-v0.1.0) - 2026-10-25

### Added

- *(miri-size)* Miri-clean unsafe modules, no_std size probe and cargo xtask size-report
- *(core-gaps)* obj id (LV_USE_OBJ_ID builtin), obj ext_data, lv_arc_bind_value, widgets float feature
- *(observer)* port lv_observer.c with upstream tests
- *(bidi)* port lv_bidi.c, lv_text_ap.c with upstream tests
- skeletons for arc/line/led/spinner/buttonmatrix/textarea, observer and bidi
- M1 integration, M2 input/layout/primitives, label/button/image widgets
- *(obj-style)* port lv_obj_style.c, lv_obj_style_gen.c
- *(image)* port lv_draw_image.c, lv_draw_sw_img.c
- *(display)* port lv_display.c
- *(refr)* port lv_refr.c
- *(obj)* port lv_obj.c
- *(obj-event-draw)* port lv_obj_event.c, lv_obj_draw.c
- *(obj-scroll)* port lv_obj_scroll.c
- *(obj-class-tree)* port lv_obj_class.c, lv_obj_tree.c
- *(obj-pos)* port lv_obj_pos.c
- *(ui-init)* port lv_init.c
- *(draw-rect-label)* port lv_draw_rect.c, lv_draw_label.c, lv_draw_mask.c
- *(anim-palette)* port lv_anim.c, lv_palette.c
- *(event)* port lv_event.c
- *(draw-buf)* port lv_draw_buf.c
- merge M1-A ports (style, text, timer/tick, grad, mask, draw-core)
- *(core)* M1 type skeleton from LVGL v9.6.0 headers
- M0 scaffolding, misc ports and font pipeline

### Fixed

- *(observer)* apply fidelity review
- *(draw-rect-label)* apply M1-A fidelity audit
- *(draw-core)* apply M1-A fidelity audit
- *(text)* apply M1-A fidelity audit
- *(style)* apply M1-A fidelity audit
- *(grad)* apply M1-A fidelity audit
- *(timer-tick)* apply M1-A fidelity audit
- *(anim-palette)* apply M1-A fidelity audit
- *(obj-style)* apply fidelity review
- *(image)* apply fidelity review
- *(display)* apply fidelity review
- *(refr)* apply fidelity review
- *(obj)* apply fidelity review
- *(obj-class-tree)* apply fidelity review
- *(obj-event-draw)* apply fidelity review
- *(obj-scroll)* apply fidelity review
- *(obj-pos)* apply fidelity review

### Other

- point repository metadata at github.com/jamongadejoa28/rLVGL
- browser backend, 132 lv_example ports, generated API mapping, Miri + size probe
- arc/spinner/line/led/buttonmatrix/textarea widgets, observer, BiDi/Arabic text
- bar/slider/switch/checkbox widgets and M1-A fidelity audit fixes
- M1-B ports (obj core, display, refr, lv_init, sw draw unit)
- M1-A rerun ports (event, draw-buf, blend x4, anim/palette, draw rect/label/mask)
