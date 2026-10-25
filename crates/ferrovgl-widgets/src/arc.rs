//! Ported from LVGL v9.6.0: `src/widgets/arc/lv_arc.c`.
//!
//! Also covers `include/lvgl/widgets/lv_arc.h` and
//! `src/widgets/arc/lv_arc_private.h`. `LV_USE_OBJ_PROPERTY` is disabled in
//! the parity configuration, so the property table is not provided.
//!
//! SKELETON: the class, state and public API are declared; the bodies are
//! `todo!()` until the widget is ported. Observer binding
//! (`lv_arc_bind_value`) is declared in `observer.rs` of this crate.

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

use alloc::boxed::Box;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::misc::types::ValuePrecise;
use ferrovgl_core::{Event, Obj, ObjClass, Ui};

/// `lv_arc_mode_t`: in which direction the indicator grows, with the upstream
/// discriminants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ArcMode {
    /// `LV_ARC_MODE_NORMAL`: clockwise.
    #[default]
    Normal = 0,
    /// `LV_ARC_MODE_SYMMETRICAL`: left/right from the midpoint.
    Symmetrical = 1,
    /// `LV_ARC_MODE_REVERSE`: counterclockwise.
    Reverse = 2,
}

/// `lv_arc_t` without its `lv_obj_t` header. Zeroed like `lv_malloc_zeroed`;
/// the constructor sets the initial values.
#[derive(Clone, Debug, Default)]
pub(crate) struct ArcState {
    /// `rotation`
    pub(crate) rotation: i32,
    /// `indic_angle_start`
    pub(crate) indic_angle_start: ValuePrecise,
    /// `indic_angle_end`
    pub(crate) indic_angle_end: ValuePrecise,
    /// `bg_angle_start`
    pub(crate) bg_angle_start: ValuePrecise,
    /// `bg_angle_end`
    pub(crate) bg_angle_end: ValuePrecise,
    /// `value`: current value of the arc.
    pub(crate) value: i32,
    /// `min_value`: minimum value of the arc.
    pub(crate) min_value: i32,
    /// `max_value`: maximum value of the arc.
    pub(crate) max_value: i32,
    /// `dragging : 1`
    pub(crate) dragging: bool,
    /// `type : 2`: the arc mode.
    pub(crate) type_: ArcMode,
    /// `min_close : 1`: the last pressed angle was closer to the minimum end.
    pub(crate) min_close: bool,
    /// `in_out : 1`: the click was within the background arc angles.
    pub(crate) in_out: bool,
    /// `chg_rate`: drag angle rate of change of the arc (degrees/sec).
    pub(crate) chg_rate: u32,
    /// `last_tick`: last dragging event timestamp of the arc.
    pub(crate) last_tick: u32,
    /// `last_angle`: last dragging angle of the arc.
    pub(crate) last_angle: ValuePrecise,
    /// `knob_offset`: knob offset from the main arc.
    pub(crate) knob_offset: i16,
}

ferrovgl_core::impl_widget_state!(ArcState);

/// `lv_arc_class`.
pub static ARC_CLASS: ObjClass = ObjClass {
    base: Some(&OBJ_CLASS),
    name: "lv_arc",
    new_state: || Box::new(ArcState::default()),
    constructor: Some(constructor),
    destructor: None,
    event_cb: Some(arc_event),
    width_def: 0,
    height_def: 0,
    editable: ObjClassEditable::True,
    group_def: ObjClassGroupDef::Inherit,
    theme_inheritable: false,
};

/// A copyable arc handle; widget state belongs to its [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Arc(Obj);

impl Arc {
    /// `lv_arc_create()`. `None` creates an arc screen on the default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&ARC_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }

    /// The base object for positioning, flags and styles.
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the arc class (spinners do).
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &ARC_CLASS)).then_some(Self(obj))
    }

    /// `lv_arc_set_start_angle()`: 0 deg: right, 90 bottom, etc.
    pub fn set_start_angle(self, ui: &mut Ui, start: ValuePrecise) {
        todo!()
    }

    /// `lv_arc_set_end_angle()`
    pub fn set_end_angle(self, ui: &mut Ui, end: ValuePrecise) {
        todo!()
    }

    /// `lv_arc_set_angles()`: set the start and end angles.
    pub fn set_angles(self, ui: &mut Ui, start: ValuePrecise, end: ValuePrecise) {
        todo!()
    }

    /// `lv_arc_set_bg_start_angle()`
    pub fn set_bg_start_angle(self, ui: &mut Ui, start: ValuePrecise) {
        todo!()
    }

    /// `lv_arc_set_bg_end_angle()`
    pub fn set_bg_end_angle(self, ui: &mut Ui, end: ValuePrecise) {
        todo!()
    }

    /// `lv_arc_set_bg_angles()`: set the start and end angles of the
    /// background.
    pub fn set_bg_angles(self, ui: &mut Ui, start: ValuePrecise, end: ValuePrecise) {
        todo!()
    }

    /// `lv_arc_set_rotation()`: rotate the whole arc.
    pub fn set_rotation(self, ui: &mut Ui, rotation: i32) {
        todo!()
    }

    /// `lv_arc_set_mode()`: in which direction the indicator grows.
    pub fn set_mode(self, ui: &mut Ui, type_: ArcMode) {
        todo!()
    }

    /// `lv_arc_set_value()`
    pub fn set_value(self, ui: &mut Ui, value: i32) {
        todo!()
    }

    /// `lv_arc_set_range()`
    pub fn set_range(self, ui: &mut Ui, min: i32, max: i32) {
        todo!()
    }

    /// `lv_arc_set_min_value()`
    pub fn set_min_value(self, ui: &mut Ui, min: i32) {
        todo!()
    }

    /// `lv_arc_set_max_value()`
    pub fn set_max_value(self, ui: &mut Ui, max: i32) {
        todo!()
    }

    /// `lv_arc_set_change_rate()`: limit how fast the arc reaches the pressed
    /// point (degrees/sec).
    pub fn set_change_rate(self, ui: &mut Ui, rate: u32) {
        todo!()
    }

    /// `lv_arc_set_knob_offset()`: knob offset from the main arc in degrees.
    pub fn set_knob_offset(self, ui: &mut Ui, offset: i32) {
        todo!()
    }

    /// `lv_arc_get_angle_start()`
    pub fn angle_start(self, ui: &Ui) -> ValuePrecise {
        todo!()
    }

    /// `lv_arc_get_angle_end()`
    pub fn angle_end(self, ui: &Ui) -> ValuePrecise {
        todo!()
    }

    /// `lv_arc_get_bg_angle_start()`
    pub fn bg_angle_start(self, ui: &Ui) -> ValuePrecise {
        todo!()
    }

    /// `lv_arc_get_bg_angle_end()`
    pub fn bg_angle_end(self, ui: &Ui) -> ValuePrecise {
        todo!()
    }

    /// `lv_arc_get_value()`
    pub fn value(self, ui: &Ui) -> i32 {
        todo!()
    }

    /// `lv_arc_get_min_value()`
    pub fn min_value(self, ui: &Ui) -> i32 {
        todo!()
    }

    /// `lv_arc_get_max_value()`
    pub fn max_value(self, ui: &Ui) -> i32 {
        todo!()
    }

    /// `lv_arc_get_mode()`
    pub fn mode(self, ui: &Ui) -> ArcMode {
        todo!()
    }

    /// `lv_arc_get_rotation()`
    pub fn rotation(self, ui: &Ui) -> i32 {
        todo!()
    }

    /// `lv_arc_get_knob_offset()`
    pub fn knob_offset(self, ui: &Ui) -> i32 {
        todo!()
    }

    /// `lv_arc_get_change_rate()`
    pub fn change_rate(self, ui: &Ui) -> u32 {
        todo!()
    }

    /// `lv_arc_align_obj_to_angle()`: align `obj_to_align` to the current
    /// knob position; `r_offset` enlarges (> 0) or shrinks the radius.
    pub fn align_obj_to_angle(self, ui: &mut Ui, obj_to_align: Obj, r_offset: i32) {
        todo!()
    }

    /// `lv_arc_rotate_obj_to_angle()`: rotate `obj_to_rotate` to the current
    /// knob position.
    pub fn rotate_obj_to_angle(self, ui: &mut Ui, obj_to_rotate: Obj, r_offset: i32) {
        todo!()
    }
}

impl From<Arc> for Obj {
    fn from(arc: Arc) -> Self {
        arc.obj()
    }
}

/// `lv_arc_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_arc_event()`
fn arc_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    todo!()
}
