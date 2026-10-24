//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_obj_class.h`, `src/core/lv_obj_class_private.h`,
//! `src/core/lv_obj_class.c`.
//!
//! Object construction runs constructors base-first and destructors
//! derived-first, exactly like `lv_obj_class_create_obj()` / `lv_obj_destruct()`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::core::Ui;
use crate::core::obj::{Obj, ObjClass};

impl Ui {
    /// `lv_obj_class_create_obj()`: allocate the object, link it to `parent`
    /// (or create a screen) and run `lv_obj_class_init_obj()` is *not* done
    /// here, like in C.
    pub fn obj_class_create_obj(&mut self, class: &'static ObjClass, parent: Option<Obj>) -> Obj {
        todo!()
    }

    /// `lv_obj_class_init_obj()`: run the constructor chain, apply the theme,
    /// set default size and notify the parent.
    pub fn obj_class_init_obj(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_destruct()`: run the destructor chain.
    pub fn obj_destruct(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_is_editable()`
    pub fn obj_is_editable(&self, obj: Obj) -> bool {
        todo!()
    }

    /// `lv_obj_is_group_def()`
    pub fn obj_is_group_def(&self, obj: Obj) -> bool {
        todo!()
    }
}

/// `get_instance_size()` replacement: the widget state constructor of the
/// most derived class.
pub fn class_new_state(
    class: &'static ObjClass,
) -> fn() -> alloc::boxed::Box<dyn crate::core::obj::WidgetState> {
    class.new_state
}
