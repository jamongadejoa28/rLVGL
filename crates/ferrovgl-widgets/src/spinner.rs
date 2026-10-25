//! Ported from LVGL v9.6.0: `src/widgets/spinner/lv_spinner.c`.
//!
//! Also covers `include/lvgl/widgets/lv_spinner.h` and
//! `src/widgets/spinner/lv_spinner_private.h`. The spinner derives from the
//! arc: its state embeds the arc state like `lv_spinner_t` embeds `lv_arc_t`,
//! and the arc API also accepts spinners. `LV_USE_OBJ_PROPERTY` is disabled in
//! the parity configuration, so the property table is not provided.

use alloc::boxed::Box;
use alloc::rc::Rc;

use ferrovgl_core::core::obj::{ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::misc::anim::{ANIM_REPEAT_INFINITE, Anim, AnimExec, AnimTarget};
use ferrovgl_core::misc::math::{BEZIER_VAL_MAX, BEZIER_VAL_SHIFT, bezier_val_float, cubic_bezier};
use ferrovgl_core::misc::types::ValuePrecise;
use ferrovgl_core::{Obj, ObjClass, Ui};

use crate::arc::{ARC_CLASS, Arc, ArcState};

/// `DEF_ARC_ANGLE`
const DEF_ARC_ANGLE: u32 = 200;
/// `DEF_TIME`
const DEF_TIME: u32 = 1000;

/// Identity of `arc_anim_angles` as an animation exec callback (compared by
/// `lv_anim_delete` like a C function pointer).
const ANIM_TAG_ARC_ANGLES: u32 = 0x4C53_504E;

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

/// `(lv_spinner_t *)obj` after `LV_CHECK_OBJ(obj, MY_CLASS, ...)`.
fn state(ui: &Ui, obj: Obj) -> Option<&SpinnerState> {
    ui.obj(obj)?.widget.get::<SpinnerState>()
}

/// Mutable [`state`].
fn state_mut(ui: &mut Ui, obj: Obj) -> Option<&mut SpinnerState> {
    ui.obj_mut(obj)?.widget.get_mut::<SpinnerState>()
}

/// A copyable spinner handle; widget state belongs to its [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    #[must_use]
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the spinner class.
    #[must_use]
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &SPINNER_CLASS)).then_some(Self(obj))
    }

    /// `lv_spinner_set_anim_params()`: set the animation time (ms) and the
    /// arc length (degrees, suited for 180..360).
    pub fn set_anim_params(self, ui: &mut Ui, t: u32, angle: u32) {
        set_anim_params(ui, self.0, t, angle);
    }

    /// `lv_spinner_set_anim_duration()`: set the animation time in ms.
    pub fn set_anim_duration(self, ui: &mut Ui, t: u32) {
        let Some(spinner) = state(ui, self.0) else { return };
        let angle = spinner.angle;

        set_anim_params(ui, self.0, t, angle);
    }

    /// `lv_spinner_set_arc_sweep()`: set the animation arc length in degrees.
    pub fn set_arc_sweep(self, ui: &mut Ui, angle: u32) {
        let Some(spinner) = state(ui, self.0) else { return };
        let duration = spinner.duration;

        set_anim_params(ui, self.0, duration, angle);
    }

    /// `lv_spinner_get_anim_duration()`
    #[must_use]
    pub fn anim_duration(self, ui: &Ui) -> u32 {
        state(ui, self.0).map_or(0, |spinner| spinner.duration)
    }

    /// `lv_spinner_get_arc_sweep()`
    #[must_use]
    pub fn arc_sweep(self, ui: &Ui) -> u32 {
        state(ui, self.0).map_or(0, |spinner| spinner.angle)
    }
}

impl From<Spinner> for Obj {
    fn from(spinner: Spinner) -> Self {
        spinner.obj()
    }
}

/// `lv_spinner_set_anim_params()`
fn set_anim_params(ui: &mut Ui, obj: Obj, t: u32, angle: u32) {
    let Some(spinner) = state_mut(ui, obj) else { return };

    spinner.duration = t;
    spinner.angle = angle;

    /*Delete the current animation*/
    ui.anim_delete(AnimTarget::Obj(obj), None);

    let mut a = Anim::new();
    a.set_var(AnimTarget::Obj(obj));
    a.set_exec_cb(anim_exec());
    a.set_repeat_count(ANIM_REPEAT_INFINITE);
    a.set_duration(t);
    a.set_values(0, BEZIER_VAL_MAX);
    ui.anim_start(&a);

    /*`lv_anim_start` runs the exec callback (early apply), so look the arc up
     *again; `LV_CHECK_OBJ` of `lv_arc_set_*` is the validity check.*/
    let Some(arc) = Arc::from_obj(ui, obj) else { return };
    arc.set_bg_angles(ui, 0 as ValuePrecise, 360 as ValuePrecise);
    arc.set_rotation(ui, 270);
}

/// `lv_spinner_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    ui.obj_set_clickable(obj, false);

    set_anim_params(ui, obj, DEF_TIME, DEF_ARC_ANGLE);
}

/// `arc_anim_angles` as an exec callback; the animated variable is the spinner.
fn anim_exec() -> AnimExec {
    AnimExec::Custom {
        tag: ANIM_TAG_ARC_ANGLES,
        f: Rc::new(|ui: &mut Ui, var: AnimTarget, v: i32| {
            if let AnimTarget::Obj(obj) = var {
                arc_anim_angles(ui, obj, v);
            }
        }),
    }
}

/// `arc_anim_angles()`
fn arc_anim_angles(ui: &mut Ui, obj: Obj, v: i32) {
    let Some(spinner) = state(ui, obj) else { return };
    let spinner_angle = spinner.angle;

    /*Start angle (bezier)*/
    let step = cubic_bezier(
        v,
        bezier_val_float(0.42),
        bezier_val_float(0.58),
        bezier_val_float(0.0),
        bezier_val_float(1.0),
    );
    let start = (step.wrapping_mul(360) >> BEZIER_VAL_SHIFT) as ValuePrecise;

    /*End angle (linear): `uint32_t + int32_t` is computed as `uint32_t`.*/
    let end = spinner_angle.wrapping_add((v.wrapping_mul(360) >> BEZIER_VAL_SHIFT) as u32) as ValuePrecise;

    let Some(arc) = Arc::from_obj(ui, obj) else { return };
    arc.set_angles(ui, start, end);
}
