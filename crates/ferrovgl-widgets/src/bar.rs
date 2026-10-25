//! Ported from LVGL v9.6.0: `src/widgets/bar/lv_bar.c`.
//!
//! Also covers `include/lvgl/widgets/lv_bar.h` and
//! `src/widgets/bar/lv_bar_private.h`.
//!
//! SKELETON: the class, state and public API are declared; the bodies are
//! `todo!()` until the widget is ported. Observer binding
//! (`lv_bar_bind_value`) needs the core observer subsystem and is not exposed.

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

use alloc::boxed::Box;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::core::ui::DPI_DEF;
use ferrovgl_core::misc::anim::AnimEnable;
use ferrovgl_core::{Area, Event, Obj, ObjClass, Ui};

/// `LV_BAR_SIZE_MIN`: hor. pad and ver. pad cannot make the indicator smaller than this [px].
const BAR_SIZE_MIN: i32 = 4;
/// `LV_BAR_ANIM_STATE_START`
const BAR_ANIM_STATE_START: i32 = 0;
/// `LV_BAR_ANIM_STATE_END`
const BAR_ANIM_STATE_END: i32 = 256;
/// `LV_BAR_ANIM_STATE_INV`: no animation is in progress.
const BAR_ANIM_STATE_INV: i32 = -1;
/// `LV_BAR_ANIM_STATE_NORM`: log2(`LV_BAR_ANIM_STATE_END`).
const BAR_ANIM_STATE_NORM: i32 = 8;

/// `lv_bar_mode_t`, with the upstream discriminants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BarMode {
    /// `LV_BAR_MODE_NORMAL`
    #[default]
    Normal = 0,
    /// `LV_BAR_MODE_SYMMETRICAL`
    Symmetrical = 1,
    /// `LV_BAR_MODE_RANGE`
    Range = 2,
}

/// `lv_bar_orientation_t`, with the upstream discriminants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BarOrientation {
    /// `LV_BAR_ORIENTATION_AUTO`
    #[default]
    Auto = 0,
    /// `LV_BAR_ORIENTATION_HORIZONTAL`
    Horizontal = 1,
    /// `LV_BAR_ORIENTATION_VERTICAL`
    Vertical = 2,
}

/// `lv_bar_anim_t`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BarAnim {
    /// `bar`
    pub(crate) bar: Option<Obj>,
    /// `anim_start`
    pub(crate) anim_start: i32,
    /// `anim_end`
    pub(crate) anim_end: i32,
    /// `anim_state`
    pub(crate) anim_state: i32,
}

/// `lv_bar_t` without its `lv_obj_t` header. Zeroed like `lv_malloc_zeroed`;
/// the constructor sets the initial values.
#[derive(Clone, Debug, Default)]
pub(crate) struct BarState {
    /// `cur_value`: current value of the bar.
    pub(crate) cur_value: i32,
    /// `min_value`: minimum value of the bar.
    pub(crate) min_value: i32,
    /// `max_value`: maximum value of the bar.
    pub(crate) max_value: i32,
    /// `start_value`: start value of the bar.
    pub(crate) start_value: i32,
    /// `indic_area`: the indicator area. Might be used by derived types.
    pub(crate) indic_area: Area,
    /// `val_reversed`: whether the value has been reversed.
    pub(crate) val_reversed: bool,
    /// `cur_value_anim`
    pub(crate) cur_value_anim: BarAnim,
    /// `start_value_anim`
    pub(crate) start_value_anim: BarAnim,
    /// `mode : 3`: type of bar.
    pub(crate) mode: BarMode,
    /// `orientation : 3`: orientation of bar.
    pub(crate) orientation: BarOrientation,
}

ferrovgl_core::impl_widget_state!(BarState);

/// `lv_bar_class`.
pub static BAR_CLASS: ObjClass = ObjClass {
    base: Some(&OBJ_CLASS),
    name: "lv_bar",
    new_state: || Box::new(BarState::default()),
    constructor: Some(constructor),
    destructor: Some(destructor),
    event_cb: Some(bar_event),
    width_def: DPI_DEF * 2,
    height_def: DPI_DEF / 10,
    editable: ObjClassEditable::Inherit,
    group_def: ObjClassGroupDef::Inherit,
    theme_inheritable: false,
};

/// A copyable bar handle; widget state belongs to its [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bar(Obj);

impl Bar {
    /// `lv_bar_create()`. `None` creates a bar screen on the default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&BAR_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }

    /// The base object for positioning, flags and styles.
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the bar class (sliders do).
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &BAR_CLASS)).then_some(Self(obj))
    }

    /// `lv_bar_set_value()`: set a new value, optionally animated.
    pub fn set_value(self, ui: &mut Ui, value: i32, anim: AnimEnable) {
        todo!()
    }

    /// `lv_bar_set_start_value()`: set a new start value, optionally animated.
    pub fn set_start_value(self, ui: &mut Ui, start_value: i32, anim: AnimEnable) {
        todo!()
    }

    /// `lv_bar_set_range()`: set the minimum and maximum values. If `min` is
    /// greater than `max`, the drawing direction is reversed.
    pub fn set_range(self, ui: &mut Ui, min: i32, max: i32) {
        todo!()
    }

    /// `lv_bar_set_min_value()`
    pub fn set_min_value(self, ui: &mut Ui, min: i32) {
        todo!()
    }

    /// `lv_bar_set_max_value()`
    pub fn set_max_value(self, ui: &mut Ui, max: i32) {
        todo!()
    }

    /// `lv_bar_set_mode()`
    pub fn set_mode(self, ui: &mut Ui, mode: BarMode) {
        todo!()
    }

    /// `lv_bar_set_orientation()`
    pub fn set_orientation(self, ui: &mut Ui, orientation: BarOrientation) {
        todo!()
    }

    /// `lv_bar_get_value()`
    pub fn value(self, ui: &Ui) -> i32 {
        todo!()
    }

    /// `lv_bar_get_start_value()`
    pub fn start_value(self, ui: &Ui) -> i32 {
        todo!()
    }

    /// `lv_bar_get_min_value()`
    pub fn min_value(self, ui: &Ui) -> i32 {
        todo!()
    }

    /// `lv_bar_get_max_value()`
    pub fn max_value(self, ui: &Ui) -> i32 {
        todo!()
    }

    /// `lv_bar_get_mode()`
    pub fn mode(self, ui: &Ui) -> BarMode {
        todo!()
    }

    /// `lv_bar_get_orientation()`
    pub fn orientation(self, ui: &Ui) -> BarOrientation {
        todo!()
    }

    /// `lv_bar_is_symmetrical()`: whether the bar is in symmetrical mode.
    pub fn is_symmetrical(self, ui: &Ui) -> bool {
        todo!()
    }
}

impl From<Bar> for Obj {
    fn from(bar: Bar) -> Self {
        bar.obj()
    }
}

/// `lv_bar_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_bar_destructor()`
fn destructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_bar_event()`
fn bar_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    todo!()
}
