//! Ported from LVGL v9.6.0: `src/widgets/checkbox/lv_checkbox.c`.
//!
//! Also covers `include/lvgl/widgets/lv_checkbox.h` and
//! `src/widgets/checkbox/lv_checkbox_private.h`.
//!
//! SKELETON: the class, state and public API are declared; the bodies are
//! `todo!()` until the widget is ported. Like the label widget, the
//! `lv_checkbox_set_text(obj, NULL)` form is [`Checkbox::refresh_text`].

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

use alloc::boxed::Box;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::misc::style::SharedStr;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Event, Obj, ObjClass, Ui};

/// The constructor's text with `LV_WIDGETS_HAS_DEFAULT_VALUE` enabled.
const CHECKBOX_DEFAULT_TEXT: &str = "Check box";

/// `lv_checkbox_t` without its `lv_obj_t` header. Zeroed like
/// `lv_malloc_zeroed`; the constructor sets the initial text.
#[derive(Clone, Debug, Default)]
pub(crate) struct CheckboxState {
    /// `txt` (`NULL` = `None`): static or owned text.
    pub(crate) txt: Option<SharedStr>,
    /// `static_txt : 1`
    pub(crate) static_txt: bool,
}

ferrovgl_core::impl_widget_state!(CheckboxState);

/// `lv_checkbox_class`.
pub static CHECKBOX_CLASS: ObjClass = ObjClass {
    base: Some(&OBJ_CLASS),
    name: "lv_checkbox",
    new_state: || Box::new(CheckboxState::default()),
    constructor: Some(constructor),
    destructor: Some(destructor),
    event_cb: Some(checkbox_event),
    width_def: coord::SIZE_CONTENT,
    height_def: coord::SIZE_CONTENT,
    editable: ObjClassEditable::Inherit,
    group_def: ObjClassGroupDef::True,
    theme_inheritable: false,
};

/// A copyable checkbox handle; widget state belongs to its [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Checkbox(Obj);

impl Checkbox {
    /// `lv_checkbox_create()`. `None` creates a checkbox screen on the default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&CHECKBOX_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }

    /// The base object for positioning, flags and styles.
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the checkbox class.
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &CHECKBOX_CLASS)).then_some(Self(obj))
    }

    /// `lv_checkbox_set_text()`: copy the text.
    pub fn set_text(self, ui: &mut Ui, text: &str) {
        todo!()
    }

    /// The `lv_checkbox_set_text(obj, NULL)` form: only refresh the widget.
    pub fn refresh_text(self, ui: &mut Ui) {
        todo!()
    }

    /// `lv_checkbox_set_text_static()`: use the text without copying it.
    pub fn set_text_static(self, ui: &mut Ui, text: &'static str) {
        todo!()
    }

    /// `lv_checkbox_get_text()`
    pub fn text(self, ui: &Ui) -> &str {
        todo!()
    }
}

impl From<Checkbox> for Obj {
    fn from(checkbox: Checkbox) -> Self {
        checkbox.obj()
    }
}

/// `lv_checkbox_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_checkbox_destructor()`
fn destructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_checkbox_event()`
fn checkbox_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    todo!()
}
