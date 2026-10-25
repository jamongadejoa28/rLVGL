//! Ported from lvgl v9.6.0: `src/core/lv_obj_id_builtin.c`
//! (`LV_USE_OBJ_ID && LV_USE_OBJ_ID_BUILTIN`).
//!
//! Also covers the `LV_USE_OBJ_ID` prototypes of `include/lvgl/core/lv_obj.h`
//! and the `objid_array` / `objid_count` members of `lv_global_t`
//! (`src/core/lv_global.h`), which live in [`Ui::objid_array`].
//!
//! The feature `obj-id` stands for `LV_USE_OBJ_ID`, `LV_USE_OBJ_ID_BUILTIN`
//! and `LV_OBJ_ID_AUTO_ASSIGN` together (the `lv_conf_template.h` defaults
//! once `LV_USE_OBJ_ID` is on, and the parity configuration
//! `tests/configs/full.defconfig`).
//!
//! The object id is the `void * id` member of `lv_obj_t`, stored as
//! [`ObjData::id`](crate::core::obj::ObjData::id): a `usize`
//! (`lv_uintptr_t`) where 0 is `NULL`. It is unrelated to the arena handle
//! [`ObjId`](crate::core::obj::ObjId). The builtin generator numbers the
//! objects of each class from 1 in creation order; [`Ui::obj_set_id`] can
//! store any other value (C stores an arbitrary pointer).

use alloc::format;
use alloc::string::String;

use crate::core::Ui;
use crate::core::obj::{Obj, ObjClass};
use crate::misc::check_arg_failed;

/// `class_info_t`: the number of objects created so far of one class.
#[derive(Clone, Copy, Debug)]
pub struct ClassInfo {
    /// `class_p`
    pub class_p: &'static ObjClass,
    /// `obj_count`
    pub obj_count: u32,
}

impl Ui {
    /// `lv_obj_assign_id()`: give `obj` the next id of `class_p` (1 for the
    /// first object of a class). Called by `lv_obj_constructor()` with the
    /// class being created (`LV_OBJ_ID_AUTO_ASSIGN`).
    pub fn obj_assign_id(&mut self, class_p: &'static ObjClass, obj: Obj) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_assign_id: obj != NULL");
            return;
        }
        /*`LV_CHECK_ARG(class_p != NULL)`: a `&'static ObjClass` is never NULL*/

        if let Some(d) = self.obj_mut(obj) {
            d.id = 0;
        }

        let mut i = 0;
        while i < self.objid_array.len() {
            if core::ptr::eq(class_p, self.objid_array[i].class_p) {
                break;
            }
            i += 1;
        }

        /*Resize array*/
        if i == self.objid_array.len() {
            /*`lv_realloc()` can't fail (no allocation failure in Rust)*/
            self.objid_array.push(ClassInfo {
                class_p,
                obj_count: 0,
            });
        }

        let info = &mut self.objid_array[i];
        info.obj_count = info.obj_count.wrapping_add(1);
        let id = info.obj_count;

        if let Some(d) = self.obj_mut(obj) {
            d.id = id as usize;
        }
    }

    /// `lv_obj_set_id()`: replace the id of `obj` (`0` drops it, like
    /// `NULL`).
    pub fn obj_set_id(&mut self, obj: Obj, id: usize) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_set_id: obj != NULL");
            return;
        }
        if self.obj(obj).is_some_and(|d| d.id != 0) {
            self.obj_free_id(obj);
        }
        if let Some(d) = self.obj_mut(obj) {
            d.id = id;
        }
    }

    /// `lv_obj_free_id()`: forget the id of `obj` (the builtin id owns no
    /// resources). Called by `lv_obj_destructor()` (`LV_OBJ_ID_AUTO_ASSIGN`).
    pub fn obj_free_id(&mut self, obj: Obj) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("lv_obj_free_id: obj != NULL");
            return;
        }
        if let Some(d) = self.obj_mut(obj) {
            d.id = 0;
        }
    }

    /// `lv_obj_stringify_id()`: the class name followed by the id as an
    /// unsigned 32 bit number (e.g. `"lv_label3"`), cut like
    /// `lv_snprintf(buf, len, ...)` to at most `len - 1` bytes.
    ///
    /// `None` is the C `NULL` return of a failed argument check (invalid
    /// object or `len == 0`).
    pub fn obj_stringify_id(&self, obj: Obj, len: u32) -> Option<String> {
        let Some(d) = self.obj(obj) else {
            check_arg_failed("lv_obj_stringify_id: obj != NULL");
            return None;
        };
        /*`LV_CHECK_ARG(obj->class_p != NULL)` and `LV_CHECK_ARG(buf != NULL)`:
         *a class reference is never NULL and the buffer is the returned string*/
        if len == 0 {
            check_arg_failed("lv_obj_stringify_id: len > 0");
            return None;
        }

        /*`if(name == NULL) name = "nameless";`: a `&'static str` name is never NULL*/
        let name = d.class.name;

        let mut buf = format!("{}{}", name, d.id as u32);
        let max = len as usize - 1;
        if buf.len() > max {
            /*`lv_snprintf()` cuts bytes; back off to a char boundary so the
             *result stays a `str` (class names are ASCII in practice)*/
            let mut cut = max;
            while !buf.is_char_boundary(cut) {
                cut -= 1;
            }
            buf.truncate(cut);
        }
        Some(buf)
    }

    /// `lv_objid_builtin_destroy()`: free the per-class counters
    /// (`lv_deinit()`).
    pub fn objid_builtin_destroy(&mut self) {
        self.objid_array = alloc::vec::Vec::new();
    }
}

/// `lv_obj_id_compare()`: 0 if the two ids are equal, 1 otherwise.
pub fn obj_id_compare(id1: usize, id2: usize) -> i32 {
    if id1 == id2 { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UiConfig;
    use crate::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
    use alloc::boxed::Box;

    /// A class deriving from `lv_obj` without a constructor of its own.
    static TEST_LABEL_CLASS: ObjClass = ObjClass {
        base: Some(&OBJ_CLASS),
        name: "lv_label",
        new_state: || Box::new(()),
        constructor: None,
        destructor: None,
        event_cb: None,
        width_def: 0,
        height_def: 0,
        editable: ObjClassEditable::Inherit,
        group_def: ObjClassGroupDef::Inherit,
        theme_inheritable: false,
    };

    fn create(ui: &mut Ui, class: &'static ObjClass, parent: Option<Obj>) -> Obj {
        let obj = ui.obj_class_create_obj(class, parent);
        ui.obj_class_init_obj(obj);
        obj
    }

    fn ui_with_screen() -> (Ui, Obj) {
        let mut ui = Ui::new(UiConfig::default());
        let disp = ui.display_create(320, 240);
        let scr = ui.display_get_screen_active(Some(disp)).unwrap();
        (ui, scr)
    }

    // Ported from tests/src/test_cases/widgets/test_objid.c
    // (`test_obj_id_should_match_class_name`; `lv_image_create()` is in the
    // widgets crate, so a second core class stands in for it).
    #[test]
    fn test_obj_id_should_match_class_name() {
        let (mut ui, _scr) = ui_with_screen();
        let obj = create(&mut ui, &OBJ_CLASS, None);
        let buf = ui.obj_stringify_id(obj, 128).unwrap();
        assert!(buf.starts_with("lv_obj"));

        let label = create(&mut ui, &TEST_LABEL_CLASS, None);
        let buf = ui.obj_stringify_id(label, 128).unwrap();
        assert!(buf.starts_with("lv_label"));
    }

    // Ported from test_objid.c `test_obj_id_should_grow_by_one`.
    #[test]
    fn test_obj_id_should_grow_by_one() {
        let (mut ui, _scr) = ui_with_screen();
        let obj1 = create(&mut ui, &TEST_LABEL_CLASS, None);
        let id1 = ui.obj_get_id(obj1) as u32;
        let obj2 = create(&mut ui, &TEST_LABEL_CLASS, None);
        let id2 = ui.obj_get_id(obj2) as u32;
        assert_eq!(id1 + 1, id2);
    }

    // Ported from test_objid.c `test_obj_id_get_child`.
    #[test]
    #[allow(deprecated)]
    fn test_obj_id_get_child() {
        let (mut ui, scr) = ui_with_screen();
        let parent = create(&mut ui, &OBJ_CLASS, Some(scr));
        let child = create(&mut ui, &TEST_LABEL_CLASS, Some(parent));
        let grandchild = create(&mut ui, &TEST_LABEL_CLASS, Some(child));

        ui.obj_set_id(child, 42);
        ui.obj_set_id(grandchild, 43);

        assert_eq!(Some(child), ui.obj_find_by_id(None, 42));
        assert_eq!(Some(grandchild), ui.obj_find_by_id(None, 43));
    }

    /// Not upstream: the counters are per class, start at 1 and the id is
    /// dropped by the destructor; `lv_objid_builtin_destroy()` restarts them.
    #[test]
    fn ids_are_counted_per_class_and_freed() {
        let (mut ui, scr) = ui_with_screen();
        // The display's screen and layers are `lv_obj`s numbered first.
        let scr_id = ui.obj_get_id(scr);
        assert_ne!(scr_id, 0);
        let a = create(&mut ui, &TEST_LABEL_CLASS, Some(scr));
        let b = create(&mut ui, &OBJ_CLASS, Some(scr));
        let c = create(&mut ui, &TEST_LABEL_CLASS, Some(scr));
        assert_eq!(ui.obj_get_id(a), 1);
        assert!(ui.obj_get_id(b) > scr_id);
        assert_eq!(ui.obj_get_id(c), 2);
        assert_eq!(ui.obj_stringify_id(c, 128).as_deref(), Some("lv_label2"));
        // `lv_snprintf()` keeps `len - 1` bytes.
        assert_eq!(ui.obj_stringify_id(c, 4).as_deref(), Some("lv_"));
        assert_eq!(ui.objid_array.len(), 2);

        // `lv_obj_destructor()` frees the id before the object goes away.
        ui.obj_destruct(c);
        assert_eq!(ui.obj_get_id(c), 0);

        ui.obj_set_id(a, 0);
        assert_eq!(ui.obj_get_id(a), 0);
        assert_eq!(obj_id_compare(5, 5), 0);
        assert_eq!(obj_id_compare(5, 6), 1);

        ui.objid_builtin_destroy();
        assert!(ui.objid_array.is_empty());
        let d = create(&mut ui, &TEST_LABEL_CLASS, Some(scr));
        assert_eq!(ui.obj_get_id(d), 1);
    }
}
