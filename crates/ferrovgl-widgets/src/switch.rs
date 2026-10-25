//! Ported from LVGL v9.6.0: `src/widgets/switch/lv_switch.c`.
//!
//! Also covers `include/lvgl/widgets/lv_switch.h` and
//! `src/widgets/switch/lv_switch_private.h`.
//!
//! SKELETON: the class, state and public API are declared; the bodies are
//! `todo!()` until the widget is ported.

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

use alloc::boxed::Box;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::core::ui::DPI_DEF;
use ferrovgl_core::{Event, Obj, ObjClass, Ui};

/// `LV_SWITCH_KNOB_EXT_AREA_CORRECTION`: switch knob extra area correction factor.
pub const SWITCH_KNOB_EXT_AREA_CORRECTION: i32 = 2;
/// `LV_SWITCH_ANIM_STATE_START`
const SWITCH_ANIM_STATE_START: i32 = 0;
/// `LV_SWITCH_ANIM_STATE_END`
const SWITCH_ANIM_STATE_END: i32 = 256;
/// `LV_SWITCH_ANIM_STATE_INV`: no animation is in progress.
const SWITCH_ANIM_STATE_INV: i32 = -1;

/// `lv_switch_orientation_t`, with the upstream discriminants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SwitchOrientation {
    /// `LV_SWITCH_ORIENTATION_AUTO`
    #[default]
    Auto = 0,
    /// `LV_SWITCH_ORIENTATION_HORIZONTAL`
    Horizontal = 1,
    /// `LV_SWITCH_ORIENTATION_VERTICAL`
    Vertical = 2,
}

/// `lv_switch_t` without its `lv_obj_t` header. Zeroed like
/// `lv_malloc_zeroed`; the constructor sets the initial values.
#[derive(Clone, Debug, Default)]
pub(crate) struct SwitchState {
    /// `anim_state`
    pub(crate) anim_state: i32,
    /// `orientation : 3`: orientation of the switch.
    pub(crate) orientation: SwitchOrientation,
}

ferrovgl_core::impl_widget_state!(SwitchState);

/// `lv_switch_class`.
pub static SWITCH_CLASS: ObjClass = ObjClass {
    base: Some(&OBJ_CLASS),
    name: "lv_switch",
    new_state: || Box::new(SwitchState::default()),
    constructor: Some(constructor),
    destructor: Some(destructor),
    event_cb: Some(switch_event),
    width_def: (4 * DPI_DEF) / 10,
    height_def: (4 * DPI_DEF) / 17,
    editable: ObjClassEditable::Inherit,
    group_def: ObjClassGroupDef::True,
    theme_inheritable: false,
};

/// A copyable switch handle; widget state belongs to its [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Switch(Obj);

impl Switch {
    /// `lv_switch_create()`. `None` creates a switch screen on the default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&SWITCH_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }

    /// The base object for positioning, flags and styles.
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the switch class.
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &SWITCH_CLASS)).then_some(Self(obj))
    }

    /// `lv_switch_set_orientation()`
    pub fn set_orientation(self, ui: &mut Ui, orientation: SwitchOrientation) {
        todo!()
    }

    /// `lv_switch_get_orientation()`
    pub fn orientation(self, ui: &Ui) -> SwitchOrientation {
        todo!()
    }
}

impl From<Switch> for Obj {
    fn from(switch: Switch) -> Self {
        switch.obj()
    }
}

/// `lv_switch_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_switch_destructor()`
fn destructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_switch_event()`
fn switch_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    todo!()
}
