//! Ported from LVGL v9.6.0: `tests/src/test_cases/widgets/test_objid.c`.
//!
//! The parity configuration has `LV_USE_OBJ_ID`, `LV_USE_OBJ_ID_BUILTIN` and
//! `LV_OBJ_ID_AUTO_ASSIGN` on (`tests/configs/full.defconfig`), which this
//! crate mirrors with the `ferrovgl-core/obj-id` feature. Every test runs
//! against the real `lv_image` / `lv_label` widgets on a fresh test display.

#![allow(deprecated)] // `LV_DEPRECATIONS_IGNORE_BEGIN`: `lv_obj_find_by_id()` is deprecated.

use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Image, Label};

#[test]
fn test_obj_id_should_match_class_name() {
    let mut display = TestDisplay::new().unwrap();
    let ui = &mut display.ui;
    let obj = ui.obj_create(None);
    let buf = ui.obj_stringify_id(obj, 128).unwrap();

    assert!(buf.starts_with("lv_obj"));

    let img = Image::create(ui, None);
    let buf = ui.obj_stringify_id(img.obj(), 128).unwrap();
    assert!(buf.starts_with("lv_image"));
}

#[test]
fn test_obj_id_should_grow_by_one() {
    let mut display = TestDisplay::new().unwrap();
    let ui = &mut display.ui;
    let obj1 = Label::create(ui, None);
    let id1 = ui.obj_get_id(obj1.obj()) as u32;
    let obj2 = Label::create(ui, None);
    let id2 = ui.obj_get_id(obj2.obj()) as u32;
    assert_eq!(id1 + 1, id2);
}

#[test]
fn test_obj_id_get_child() {
    let mut display = TestDisplay::new().unwrap();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let parent = ui.obj_create(Some(screen));
    let child = Label::create(ui, parent).obj();
    let grandchild = Label::create(ui, child).obj();

    ui.obj_set_id(child, 42);
    ui.obj_set_id(grandchild, 43);

    assert_eq!(Some(child), ui.obj_find_by_id(None, 42));
    assert_eq!(Some(grandchild), ui.obj_find_by_id(None, 43));
}
