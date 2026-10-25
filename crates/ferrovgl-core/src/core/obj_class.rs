//! Ported from lvgl v9.6.0: `src/core/lv_obj_class.c`
//!
//! Also covers `include/lvgl/core/lv_obj_class.h` and `src/core/lv_obj_class_private.h`.
//!
//! Object construction runs constructors base-first and destructors
//! derived-first, exactly like `lv_obj_class_create_obj()` / `lv_obj_destruct()`.

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::core::Ui;
use crate::core::obj::{
    Obj, ObjClass, ObjClassEditable, ObjClassGroupDef, ObjData, ObjFlags, ObjId, WidgetState,
};
use crate::misc::area::Area;
use crate::misc::check_arg_failed;
use crate::misc::event::{EventCode, EventParam, LvResult};
use crate::misc::style::{Part, State, StyleProp};

impl Ui {
    /// `lv_obj_class_create_obj()`: allocate the object, link it to `parent`
    /// (or create a screen on the default display). `lv_obj_class_init_obj()`
    /// is *not* called here, like in C.
    ///
    /// Where C returns `NULL` (no default display, invalid parent) this
    /// returns a stale handle: every `Ui` method treats it like `NULL`
    /// (`obj_is_valid()` is `false`).
    pub fn obj_class_create_obj(&mut self, class: &'static ObjClass, parent: Option<Obj>) -> Obj {
        // uint32_t s = get_instance_size(class_p); lv_malloc_zeroed(s)
        let data = ObjData {
            class,
            parent: parent.map(Obj::id),
            spec: None,
            styles: Vec::new(),
            style_main_prop_is_set: 0,
            style_other_prop_is_set: 0,
            user_data: None,
            coords: Area::new(0, 0, 0, 0),
            flags: ObjFlags::empty(),
            radio_button: false,
            state: State::empty(),
            layout_inv: false,
            readjust_scroll_after_layout: false,
            scr_layout_inv: false,
            skip_trans: false,
            h_layout: false,
            w_layout: false,
            h_ignore_size: false,
            w_ignore_size: false,
            is_deleting: false,
            rendered: false,
            has_blur: false,
            widget: get_instance_size(class)(),
        };
        let obj = Obj(ObjId(self.objs.insert(data)));

        match parent {
            // Create a screen
            None => {
                let Some(disp) = self.display_get_default() else {
                    #[cfg(feature = "log")]
                    log::warn!("No display created yet. No place to assign the new screen");
                    return self.obj_class_free_new(obj);
                };

                let hor_res = self.display_get_horizontal_resolution(Some(disp));
                let ver_res = self.display_get_vertical_resolution(Some(disp));
                let Some(d) = self.displays.get_mut(disp.0) else {
                    return self.obj_class_free_new(obj);
                };
                d.screens.push(obj.id());

                // Set coordinates to full screen size
                if let Some(o) = self.obj_mut(obj) {
                    o.coords.x1 = 0;
                    o.coords.y1 = 0;
                    o.coords.x2 = hor_res.wrapping_sub(1);
                    o.coords.y2 = ver_res.wrapping_sub(1);
                }
            }
            // Create a normal object
            Some(parent) => {
                if !self.obj_is_valid(parent) {
                    check_arg_failed("lv_obj_class_create_obj: invalid parent");
                    return self.obj_class_free_new(obj);
                }

                if !self.obj_tree_allocate_spec_attr(parent) {
                    return self.obj_class_free_new(obj);
                }
                if self.obj_add_child(parent, obj) != LvResult::Ok {
                    return self.obj_class_free_new(obj);
                }
            }
        }

        obj
    }

    /// `lv_free(obj); return NULL;` of `lv_obj_class_create_obj()`: drop the
    /// half-created object and hand back its (now stale) handle.
    fn obj_class_free_new(&mut self, obj: Obj) -> Obj {
        self.objs.remove(obj.id().0);
        obj
    }

    /// `lv_obj_class_init_obj()`: run the constructor chain, apply the theme,
    /// set default size and notify the parent.
    pub fn obj_class_init_obj(&mut self, obj: Obj) {
        let Some(class) = self.obj(obj).map(|o| o.class) else {
            check_arg_failed("lv_obj_class_init_obj: obj != NULL");
            return;
        };

        self.obj_mark_layout_as_dirty(obj);
        self.obj_enable_style_refresh(false);

        self.theme_apply(obj);
        self.obj_class_construct(class, obj);

        self.obj_enable_style_refresh(true);
        self.obj_refresh_style(obj, Part::ANY, StyleProp::ANY);

        self.obj_refresh_self_size(obj);

        let def_group = self.group_get_default();
        if let Some(def_group) = def_group
            && self.obj_is_group_def(obj)
        {
            self.group_add_obj(def_group, obj);
        }

        let parent = self.obj_get_parent(obj);
        if let Some(parent) = parent {
            // Call the ancestor's event handler to the parent to notify it about the new child.
            // Also triggers layout update
            self.obj_send_event(parent, EventCode::CHILD_CHANGED, EventParam::Obj(obj));
            self.obj_send_event(parent, EventCode::CHILD_CREATED, EventParam::Obj(obj));

            // Invalidate the area if not screen created
            self.obj_invalidate(obj);
        }
    }

    /// `lv_obj_destruct()`: run the destructor chain, derived class first.
    /// Every destructor receives the class currently being destructed
    /// (`obj->class_p`), which is lowered to the base class before the base
    /// destructor runs.
    pub fn obj_destruct(&mut self, obj: Obj) {
        let Some(class) = self.obj(obj).map(|o| o.class) else {
            check_arg_failed("lv_obj_destruct: obj != NULL");
            return;
        };

        // LV_USE_EXT_DATA: `ObjData` has no `ext_data` yet (shared type request).

        if let Some(destructor) = class.destructor {
            destructor(class, self, obj);
        }

        if let Some(base) = class.base {
            // Don't let the descendant methods run during destructing the ancestor type
            let Some(o) = self.obj_mut(obj) else { return };
            o.class = base;

            // Call the base class's destructor too
            self.obj_destruct(obj);
        }
    }

    /// `lv_obj_is_editable()`: the first `editable` value that is not
    /// `INHERIT`, walking from the object's class to its bases.
    pub fn obj_is_editable(&self, obj: Obj) -> bool {
        let Some(o) = self.obj(obj) else {
            check_arg_failed("lv_obj_is_editable: obj != NULL");
            return false;
        };

        let mut class_p = Some(o.class);

        // Find a base in which editable is set
        while let Some(c) = class_p
            && c.editable == ObjClassEditable::Inherit
        {
            class_p = c.base;
        }

        let Some(c) = class_p else { return false };

        c.editable == ObjClassEditable::True
    }

    /// `lv_obj_is_group_def()`: the first `group_def` value that is not
    /// `INHERIT`, walking from the object's class to its bases.
    pub fn obj_is_group_def(&self, obj: Obj) -> bool {
        let Some(o) = self.obj(obj) else {
            check_arg_failed("lv_obj_is_group_def: obj != NULL");
            return false;
        };

        let mut class_p = Some(o.class);

        // Find a base in which group_def is set
        while let Some(c) = class_p
            && c.group_def == ObjClassGroupDef::Inherit
        {
            class_p = c.base;
        }

        let Some(c) = class_p else { return false };

        c.group_def == ObjClassGroupDef::True
    }

    /// `lv_obj_construct()` (static): construct the base classes first,
    /// with `obj->class_p` temporarily lowered to the class being
    /// constructed, then call this level's constructor with the final
    /// class `class`.
    fn obj_class_construct(&mut self, class: &'static ObjClass, obj: Obj) {
        let Some(cur) = self.obj(obj).map(|o| o.class) else {
            return;
        };

        // LV_USE_EXT_DATA: `ext_data` would be cleared here (not in `ObjData`).

        if let Some(base) = cur.base {
            let original_class_p = cur;

            // Don't let the descendant methods run during constructing the ancestor type
            if let Some(o) = self.obj_mut(obj) {
                o.class = base;
            }

            // Construct the base first
            self.obj_class_construct(class, obj);

            // Restore the original class
            match self.obj_mut(obj) {
                Some(o) => o.class = original_class_p,
                None => return,
            }
        }

        let Some(cur) = self.obj(obj).map(|o| o.class) else {
            return;
        };
        if let Some(constructor) = cur.constructor {
            constructor(class, self, obj);
        }
    }
}

/// `get_instance_size()` (static): every class sets `new_state`, so the
/// most derived class's constructor is used directly (no base walk needed).
fn get_instance_size(class: &'static ObjClass) -> fn() -> Box<dyn WidgetState> {
    class.new_state
}

/// `get_instance_size()` replacement: the widget state constructor of the
/// most derived class.
pub fn class_new_state(class: &'static ObjClass) -> fn() -> Box<dyn WidgetState> {
    get_instance_size(class)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::core::UiConfig;
    use crate::core::obj::OBJ_CLASS;
    use alloc::string::String;
    use alloc::vec;

    fn log(ui: &mut Ui, s: String) {
        ui.app_mut::<Vec<String>>().unwrap().push(s);
    }

    fn ctor_a(class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
        let cur = ui.obj(obj).unwrap().class.name;
        log(ui, alloc::format!("ctor A final={} cur={}", class.name, cur));
    }
    fn ctor_b(class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
        let cur = ui.obj(obj).unwrap().class.name;
        log(ui, alloc::format!("ctor B final={} cur={}", class.name, cur));
    }
    fn ctor_c(class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
        let cur = ui.obj(obj).unwrap().class.name;
        log(ui, alloc::format!("ctor C final={} cur={}", class.name, cur));
    }
    fn dtor_a(class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
        let cur = ui.obj(obj).unwrap().class.name;
        log(ui, alloc::format!("dtor A class={} cur={}", class.name, cur));
    }
    fn dtor_c(class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
        let cur = ui.obj(obj).unwrap().class.name;
        log(ui, alloc::format!("dtor C class={} cur={}", class.name, cur));
    }

    pub(crate) static TEST_A: ObjClass = ObjClass {
        base: None,
        name: "a",
        new_state: || Box::new(()),
        constructor: Some(ctor_a),
        destructor: Some(dtor_a),
        event_cb: None,
        width_def: 0,
        height_def: 0,
        editable: ObjClassEditable::False,
        group_def: ObjClassGroupDef::True,
        theme_inheritable: false,
    };
    pub(crate) static TEST_B: ObjClass = ObjClass {
        base: Some(&TEST_A),
        name: "b",
        new_state: || Box::new(()),
        constructor: Some(ctor_b),
        destructor: None,
        event_cb: None,
        width_def: 0,
        height_def: 0,
        editable: ObjClassEditable::Inherit,
        group_def: ObjClassGroupDef::Inherit,
        theme_inheritable: false,
    };
    pub(crate) static TEST_C: ObjClass = ObjClass {
        base: Some(&TEST_B),
        name: "c",
        new_state: || Box::new(()),
        constructor: Some(ctor_c),
        destructor: Some(dtor_c),
        event_cb: None,
        width_def: 0,
        height_def: 0,
        editable: ObjClassEditable::True,
        group_def: ObjClassGroupDef::Inherit,
        theme_inheritable: false,
    };
    static TEST_ALL_INHERIT: ObjClass = ObjClass {
        base: None,
        name: "inherit",
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

    /// Insert a bare object without any of the services that are still
    /// `todo!()`: with a parent this is `lv_obj_class_create_obj()`, without
    /// one a detached root (a "screen" not registered on any display).
    pub(crate) fn raw_obj(ui: &mut Ui, class: &'static ObjClass, parent: Option<Obj>) -> Obj {
        if let Some(p) = parent {
            return ui.obj_class_create_obj(class, Some(p));
        }
        let data = ObjData {
            class,
            parent: None,
            spec: None,
            styles: Vec::new(),
            style_main_prop_is_set: 0,
            style_other_prop_is_set: 0,
            user_data: None,
            coords: Area::new(0, 0, 0, 0),
            flags: ObjFlags::empty(),
            radio_button: false,
            state: State::empty(),
            layout_inv: false,
            readjust_scroll_after_layout: false,
            scr_layout_inv: false,
            skip_trans: false,
            h_layout: false,
            w_layout: false,
            h_ignore_size: false,
            w_ignore_size: false,
            is_deleting: false,
            rendered: false,
            has_blur: false,
            widget: (class.new_state)(),
        };
        Obj(ObjId(ui.objs.insert(data)))
    }

    #[test]
    fn create_obj_links_to_parent() {
        let mut ui = Ui::new(UiConfig::default());
        let scr = raw_obj(&mut ui, &OBJ_CLASS, None);
        let a = ui.obj_class_create_obj(&TEST_A, Some(scr));
        let b = ui.obj_class_create_obj(&TEST_B, Some(scr));
        assert_eq!(ui.obj(a).unwrap().parent, Some(scr.id()));
        assert_eq!(ui.obj_children(scr), &[a.id(), b.id()]);
        assert!(ui.obj(a).unwrap().spec.is_none());
        assert!(core::ptr::eq(ui.obj(b).unwrap().class, &TEST_B));
        assert_eq!(ui.obj(a).unwrap().coords, Area::new(0, 0, 0, 0));
        assert!(!ui.obj(a).unwrap().is_deleting);
    }

    #[test]
    #[cfg_attr(feature = "check-arg-panic", should_panic(expected = "argument check failed"))]
    fn create_obj_on_stale_parent_returns_stale_handle() {
        let mut ui = Ui::new(UiConfig::default());
        let p = raw_obj(&mut ui, &OBJ_CLASS, None);
        ui.objs.remove(p.id().0);
        let o = ui.obj_class_create_obj(&TEST_A, Some(p));
        assert!(!ui.obj_is_valid(o));
        assert!(ui.objs.is_empty());
    }

    #[test]
    fn constructors_run_base_first_with_final_class() {
        let mut ui = Ui::new(UiConfig::default());
        ui.set_app::<Vec<String>>(Vec::new());
        let o = raw_obj(&mut ui, &TEST_C, None);
        ui.obj_class_construct(&TEST_C, o);
        assert_eq!(
            ui.app::<Vec<String>>().unwrap(),
            &vec![
                String::from("ctor A final=c cur=a"),
                String::from("ctor B final=c cur=b"),
                String::from("ctor C final=c cur=c"),
            ]
        );
        // The original class is restored.
        assert!(core::ptr::eq(ui.obj(o).unwrap().class, &TEST_C));
    }

    #[test]
    fn destructors_run_derived_first_with_current_class() {
        let mut ui = Ui::new(UiConfig::default());
        ui.set_app::<Vec<String>>(Vec::new());
        let o = raw_obj(&mut ui, &TEST_C, None);
        ui.obj_destruct(o);
        assert_eq!(
            ui.app::<Vec<String>>().unwrap(),
            &vec![
                String::from("dtor C class=c cur=c"),
                String::from("dtor A class=a cur=a")
            ]
        );
        // Like C, `class_p` is left at the root class.
        assert!(core::ptr::eq(ui.obj(o).unwrap().class, &TEST_A));
    }

    #[test]
    fn editable_and_group_def_walk_the_bases() {
        let mut ui = Ui::new(UiConfig::default());
        let a = raw_obj(&mut ui, &TEST_A, None);
        let b = raw_obj(&mut ui, &TEST_B, None);
        let c = raw_obj(&mut ui, &TEST_C, None);
        let i = raw_obj(&mut ui, &TEST_ALL_INHERIT, None);
        let base = raw_obj(&mut ui, &OBJ_CLASS, None);
        assert!(!ui.obj_is_editable(a));
        assert!(!ui.obj_is_editable(b)); // inherits FALSE from A
        assert!(ui.obj_is_editable(c));
        assert!(!ui.obj_is_editable(i)); // no base sets it
        assert!(!ui.obj_is_editable(base));
        assert!(ui.obj_is_group_def(a));
        assert!(ui.obj_is_group_def(b));
        assert!(ui.obj_is_group_def(c)); // C -> B -> A (TRUE)
        assert!(!ui.obj_is_group_def(i));
        assert!(!ui.obj_is_group_def(base));
    }

    #[test]
    fn class_new_state_is_the_derived_state() {
        let s = class_new_state(&TEST_C)();
        assert!(s.get::<()>().is_some());
    }
}
