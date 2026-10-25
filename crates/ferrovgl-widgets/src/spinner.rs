//! Ported from LVGL v9.6.0: `src/widgets/spinner/lv_spinner.c`.
//!
//! Also covers `include/lvgl/widgets/lv_spinner.h` and
//! `src/widgets/spinner/lv_spinner_private.h`. The spinner derives from the
//! arc: its state embeds the arc state like `lv_spinner_t` embeds `lv_arc_t`,
//! and the arc API also accepts spinners. `LV_USE_OBJ_PROPERTY` is disabled in
//! the parity configuration, so the property table is not provided.
//!
//! SKELETON: the class, state and public API are declared; the bodies are
//! `todo!()` until the widget is ported.

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

use alloc::boxed::Box;

use ferrovgl_core::core::obj::{ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::{Obj, ObjClass, Ui};

use crate::arc::{ARC_CLASS, ArcState};

/// `lv_spinner_t` without its `lv_obj_t` header. Zeroed like
/// `lv_malloc_zeroed`; the constructor sets the initial values.
#[derive(Clone, Debug, Default)]
pub(crate) struct SpinnerState {
    /// `arc`: the ancestor's state first.
    pub(crate) arc: ArcState,
    /// `duration`: animation duration in ms.
    pub(crate) duration: u32,
    /// `angle`: animation angle in degrees.
    pub(crate) angle: u32,
}

ferrovgl_core::impl_widget_state!(SpinnerState, base = arc);

/// `lv_spinner_class`. Sizes, editability and group default are inherited
/// from the arc.
pub static SPINNER_CLASS: ObjClass = ObjClass {
    base: Some(&ARC_CLASS),
    name: "lv_spinner",
    new_state: || Box::new(SpinnerState::default()),
    constructor: Some(constructor),
    destructor: None,
    event_cb: None,
    width_def: 0,
    height_def: 0,
    editable: ObjClassEditable::Inherit,
    group_def: ObjClassGroupDef::Inherit,
    theme_inheritable: false,
};

/// A copyable spinner handle; widget state belongs to its [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Spinner(Obj);

impl Spinner {
    /// `lv_spinner_create()`. `None` creates a spinner screen on the default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&SPINNER_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }

    /// The base object for positioning, flags and styles.
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the spinner class.
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &SPINNER_CLASS)).then_some(Self(obj))
    }

    /// `lv_spinner_set_anim_params()`: set the animation time (ms) and the
    /// arc length (degrees, suited for 180..360).
    pub fn set_anim_params(self, ui: &mut Ui, t: u32, angle: u32) {
        todo!()
    }

    /// `lv_spinner_set_anim_duration()`: set the animation time in ms.
    pub fn set_anim_duration(self, ui: &mut Ui, t: u32) {
        todo!()
    }

    /// `lv_spinner_set_arc_sweep()`: set the animation arc length in degrees.
    pub fn set_arc_sweep(self, ui: &mut Ui, angle: u32) {
        todo!()
    }

    /// `lv_spinner_get_anim_duration()`
    pub fn anim_duration(self, ui: &Ui) -> u32 {
        todo!()
    }

    /// `lv_spinner_get_arc_sweep()`
    pub fn arc_sweep(self, ui: &Ui) -> u32 {
        todo!()
    }
}

impl From<Spinner> for Obj {
    fn from(spinner: Spinner) -> Self {
        spinner.obj()
    }
}

/// `lv_spinner_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}
