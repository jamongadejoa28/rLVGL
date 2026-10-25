//! Button class from LVGL v9.6.0 `src/widgets/button/lv_button.c`.
//!
//! A button adds no state or event handler: interaction, checkable state and
//! drawing are inherited from the base object. Its class opts into the default
//! group and sizes itself from its contents.

use alloc::boxed::Box;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Obj, ObjClass, Ui};

/// `lv_button_class`.
pub static BUTTON_CLASS: ObjClass = ObjClass {
    base: Some(&OBJ_CLASS),
    name: "lv_button",
    new_state: || Box::new(()),
    constructor: Some(constructor),
    destructor: None,
    event_cb: None,
    width_def: coord::SIZE_CONTENT,
    height_def: coord::SIZE_CONTENT,
    editable: ObjClassEditable::Inherit,
    group_def: ObjClassGroupDef::True,
    theme_inheritable: false,
};

/// A copyable button handle; its state belongs to the associated [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Button(Obj);

impl Button {
    /// `lv_button_create()`. `None` creates a button screen on the default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&BUTTON_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }

    /// The base object for positioning, flags and styles.
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the button class.
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &BUTTON_CLASS)).then_some(Self(obj))
    }
}

impl From<Button> for Obj {
    fn from(button: Button) -> Self {
        button.obj()
    }
}

fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    ui.obj_set_scrollable(obj, false);
    ui.obj_set_scroll_on_focus(obj, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::{Part, UiConfig};

    #[test]
    fn class_flags_group_and_stale_handle_match_c() {
        let mut ui = Ui::new(UiConfig::default());
        let display = ui.display_create(320, 240);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let group = ui.group_create();
        ui.group_set_default(Some(group));
        let button = Button::create(&mut ui, screen);
        assert!(ui.obj_check_type(button.obj(), &BUTTON_CLASS));
        assert!(!ui.obj_check_type(button.obj(), &OBJ_CLASS));
        assert!(!ui.obj_is_scrollable(button.obj()));
        assert!(ui.obj_is_scroll_on_focus(button.obj()));
        assert!(ui.obj_is_clickable(button.obj()));
        assert!(!ui.obj_is_checkable(button.obj()));
        assert_eq!(ui.group_get_focused(group), Some(button.obj()));
        assert_eq!(
            ui.obj_get_style_width(button.obj(), Part::MAIN),
            coord::SIZE_CONTENT
        );
        assert_eq!(
            ui.obj_get_style_height(button.obj(), Part::MAIN),
            coord::SIZE_CONTENT
        );
        assert_eq!(Button::from_obj(&ui, button.obj()), Some(button));
        assert_eq!(Button::from_obj(&ui, screen), None);
        ui.obj_delete(button.obj());
        assert_eq!(Button::from_obj(&ui, button.obj()), None);
        assert_eq!(ui.group_get_focused(group), None);
    }
}
