//! Ported from LVGL v9.6.0: `src/widgets/arc/lv_arc.c`.
//!
//! Also covers `include/lvgl/widgets/lv_arc.h` and
//! `src/widgets/arc/lv_arc_private.h`. `LV_USE_OBJ_PROPERTY` is disabled in
//! the parity configuration, so the property table (`lv_arc_properties[]`)
//! is not provided.
//!
//! Observer binding (`lv_arc_bind_value()`, `arc_value_changed_event_cb()`,
//! `arc_value_observer_cb()`; `LV_USE_OBSERVER=y` in the parity config) is
//! NOT ported by this module: `Arc::bind_value()` is declared in this crate's
//! `observer.rs` and stays a `todo!()` skeleton until the core subject/observer
//! API (`ferrovgl_core::core::observer`) is ported. When it is, port it 1:1:
//! return `None` unless the subject is `INT` or `FLOAT`; add a
//! `VALUE_CHANGED` event callback that stores `lv_arc_get_value()` with
//! `lv_subject_set_int()` (or `lv_subject_set_float((float)value)` for a
//! `FLOAT` subject); then `lv_subject_add_observer_obj()` an observer that
//! calls `lv_arc_set_value(target, subject->value.num)` (or
//! `(int32_t)subject->value.float_v`, truncating). None of `test_arc.c` uses
//! it.
//!
//! `lv_value_precise_t` is `f32` with `LV_USE_FLOAT` (the parity config) and
//! `i32` otherwise; [`Precise`] keeps the C conversions and arithmetic of both
//! configurations in one code path.

use alloc::boxed::Box;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::core::ui::DPI_DEF;
use ferrovgl_core::draw::arc::arc_get_area;
use ferrovgl_core::draw::rect::RADIUS_CIRCLE;
use ferrovgl_core::draw::{ArcDsc, RectDsc};
use ferrovgl_core::group::{KEY_DOWN, KEY_LEFT, KEY_RIGHT, KEY_UP};
use ferrovgl_core::misc::color::OPA_MIN;
use ferrovgl_core::misc::math::{TRIGO_SHIFT, atan2, map, trigo_sin};
use ferrovgl_core::misc::types::ValuePrecise;
use ferrovgl_core::{
    Align, Area, Event, EventCode, EventParam, IndevType, LvResult, Obj, ObjClass, Part, Point,
    StyleSelector, Ui,
};

/// `VALUE_UNSET`: `INT16_MIN`.
const VALUE_UNSET: i32 = i16::MIN as i32;
/// `CLICK_OUTSIDE_BG_ANGLES`
const CLICK_OUTSIDE_BG_ANGLES: bool = false;
/// `CLICK_INSIDE_BG_ANGLES`
const CLICK_INSIDE_BG_ANGLES: bool = true;
/// `CLICK_CLOSER_TO_MAX_END`
const CLICK_CLOSER_TO_MAX_END: bool = false;
/// `CLICK_CLOSER_TO_MIN_END`
const CLICK_CLOSER_TO_MIN_END: bool = true;

/// The C conversions and operators of `lv_value_precise_t` in both
/// `LV_USE_FLOAT` configurations.
trait Precise: Copy {
    /// `(lv_value_precise_t)v` of an `int32_t`.
    fn from_i32(v: i32) -> Self;
    /// `(lv_value_precise_t)v` of a `uint32_t`.
    fn from_u32(v: u32) -> Self;
    /// `(int32_t)self`
    fn to_i32(self) -> i32;
    /// `LV_ABS(self)`
    fn lv_abs(self) -> Self;
    /// `self / rhs`
    fn div(self, rhs: Self) -> Self;
}

impl Precise for f32 {
    fn from_i32(v: i32) -> Self {
        v as f32
    }

    fn from_u32(v: u32) -> Self {
        v as f32
    }

    /// Truncates toward zero like C; out-of-range values (undefined in C)
    /// saturate.
    fn to_i32(self) -> i32 {
        self as i32
    }

    fn lv_abs(self) -> Self {
        if self > 0.0 { self } else { -self }
    }

    /// IEEE division: a zero divisor gives an infinity or NaN like C.
    fn div(self, rhs: Self) -> Self {
        self / rhs
    }
}

impl Precise for i32 {
    fn from_i32(v: i32) -> Self {
        v
    }

    fn from_u32(v: u32) -> Self {
        v as i32
    }

    fn to_i32(self) -> i32 {
        self
    }

    fn lv_abs(self) -> Self {
        if self > 0 { self } else { self.wrapping_neg() }
    }

    /// C division truncates toward zero; a zero divisor is undefined behavior
    /// upstream and gives 0 here instead of panicking.
    fn div(self, rhs: Self) -> Self {
        if rhs == 0 { 0 } else { self.wrapping_div(rhs) }
    }
}

/// `(lv_value_precise_t)v`
fn vp(v: i32) -> ValuePrecise {
    ValuePrecise::from_i32(v)
}

/// `lv_trigo_sin(angle)` with the implicit `int32_t` to `int16_t` conversion
/// of its parameter.
fn sin(angle: i32) -> i32 {
    trigo_sin(angle as i16)
}

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

fn state(ui: &Ui, obj: Obj) -> Option<&ArcState> {
    ui.obj(obj)?.widget.get::<ArcState>()
}

fn state_mut(ui: &mut Ui, obj: Obj) -> Option<&mut ArcState> {
    ui.obj_mut(obj)?.widget.get_mut::<ArcState>()
}

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
        let Some(arc) = state(ui, self.0) else { return };
        let end = arc.indic_angle_end;

        self.set_angles(ui, start, end);
    }

    /// `lv_arc_set_end_angle()`
    pub fn set_end_angle(self, ui: &mut Ui, end: ValuePrecise) {
        let Some(arc) = state(ui, self.0) else { return };
        let start = arc.indic_angle_start;

        self.set_angles(ui, start, end);
    }

    /// `lv_arc_set_angles()`: set the start and end angles.
    pub fn set_angles(self, ui: &mut Ui, mut start: ValuePrecise, mut end: ValuePrecise) {
        let obj = self.0;
        if state(ui, obj).is_none() {
            return; /*LV_CHECK_OBJ*/
        }

        /* start=0, end=360 means full circle, so accept 360*/
        while start > vp(360) {
            start -= vp(360);
        }
        while end > vp(360) {
            end -= vp(360);
        }
        while start < vp(0) {
            start += vp(360);
        }
        while end < vp(0) {
            end += vp(360);
        }

        let visible = ui.obj_is_visible(obj);

        /*Snapshot for invalidation*/
        let Some(arc) = state(ui, obj) else { return };
        let old_start = arc.indic_angle_start;
        let old_end = arc.indic_angle_end;
        let mut old_knob_area = Area::default();
        let knob_visible = visible && get_knob_inv_area(ui, obj, &mut old_knob_area);

        /*Apply the change*/
        let Some(arc) = state_mut(ui, obj) else { return };
        arc.indic_angle_start = start;
        arc.indic_angle_end = end;

        /*Invalidation*/
        if visible {
            /*Area swept by the start angle*/
            let mut start_old_delta = end - old_start;
            let mut start_new_delta = end - start;
            if start_old_delta < vp(0) {
                start_old_delta += vp(360); /*`360 + start_old_delta`: addition commutes*/
            }
            if start_new_delta < vp(0) {
                start_new_delta += vp(360); /*`360 + start_new_delta`: addition commutes*/
            }
            if (start_new_delta - start_old_delta).lv_abs() > vp(180) {
                ui.obj_invalidate(obj);
            } else if start_new_delta < start_old_delta {
                inv_arc_area(ui, obj, old_start, start, Part::INDICATOR);
            } else if start_old_delta < start_new_delta {
                inv_arc_area(ui, obj, start, old_start, Part::INDICATOR);
            }

            /*Area swept by the end angle*/
            let mut end_old_delta = old_end - old_start;
            let mut end_new_delta = end - old_start;
            if end_old_delta < vp(0) {
                end_old_delta += vp(360); /*`360 + end_old_delta`: addition commutes*/
            }
            if end_new_delta < vp(0) {
                end_new_delta += vp(360); /*`360 + end_new_delta`: addition commutes*/
            }
            if (end_new_delta - end_old_delta).lv_abs() > vp(180) {
                ui.obj_invalidate(obj);
            } else if end_new_delta < end_old_delta {
                inv_arc_area(ui, obj, end, old_end, Part::INDICATOR);
            } else if end_old_delta < end_new_delta {
                inv_arc_area(ui, obj, old_end, end, Part::INDICATOR);
            }

            /*Knob*/
            if knob_visible {
                ui.obj_invalidate_area(obj, &old_knob_area);
                inv_knob_area(ui, obj);
            }
        }
    }

    /// `lv_arc_set_bg_start_angle()`
    pub fn set_bg_start_angle(self, ui: &mut Ui, start: ValuePrecise) {
        let Some(arc) = state(ui, self.0) else { return };
        let end = arc.bg_angle_end;

        self.set_bg_angles(ui, start, end);
    }

    /// `lv_arc_set_bg_end_angle()`
    pub fn set_bg_end_angle(self, ui: &mut Ui, end: ValuePrecise) {
        let Some(arc) = state(ui, self.0) else { return };
        let start = arc.bg_angle_start;

        self.set_bg_angles(ui, start, end);
    }

    /// `lv_arc_set_bg_angles()`: set the start and end angles of the
    /// background.
    pub fn set_bg_angles(self, ui: &mut Ui, mut start: ValuePrecise, mut end: ValuePrecise) {
        let obj = self.0;
        if state(ui, obj).is_none() {
            return; /*LV_CHECK_OBJ*/
        }

        /* start=0, end=360 means full circle, so accept 360*/
        while start > vp(360) {
            start -= vp(360);
        }
        while end > vp(360) {
            end -= vp(360);
        }
        while start < vp(0) {
            start += vp(360);
        }
        while end < vp(0) {
            end += vp(360);
        }

        /*Snapshot for invalidation*/
        let Some(arc) = state_mut(ui, obj) else { return };
        let old_start = arc.bg_angle_start;
        let old_end = arc.bg_angle_end;

        /*Apply the change*/
        arc.bg_angle_start = start;
        arc.bg_angle_end = end;
        value_update(ui, obj);

        /*Invalidation*/
        if ui.obj_is_visible(obj) {
            /*Area swept by the start angle*/
            let mut start_old_delta = end - old_start;
            let mut start_new_delta = end - start;
            if start_old_delta < vp(0) {
                start_old_delta += vp(360); /*`360 + start_old_delta`: addition commutes*/
            }
            if start_new_delta < vp(0) {
                start_new_delta += vp(360); /*`360 + start_new_delta`: addition commutes*/
            }
            if (start_new_delta - start_old_delta).lv_abs() > vp(180) {
                ui.obj_invalidate(obj);
            } else if start_new_delta < start_old_delta {
                inv_arc_area(ui, obj, old_start, start, Part::MAIN);
            } else if start_old_delta < start_new_delta {
                inv_arc_area(ui, obj, start, old_start, Part::MAIN);
            }

            /*Area swept by the end angle*/
            let mut end_old_delta = old_end - old_start;
            let mut end_new_delta = end - old_start;
            if end_old_delta < vp(0) {
                end_old_delta += vp(360); /*`360 + end_old_delta`: addition commutes*/
            }
            if end_new_delta < vp(0) {
                end_new_delta += vp(360); /*`360 + end_new_delta`: addition commutes*/
            }
            if (end_new_delta - end_old_delta).lv_abs() > vp(180) {
                ui.obj_invalidate(obj);
            } else if end_new_delta < end_old_delta {
                inv_arc_area(ui, obj, end, old_end, Part::MAIN);
            } else if end_old_delta < end_new_delta {
                inv_arc_area(ui, obj, old_end, end, Part::MAIN);
            }
        }
    }

    /// `lv_arc_set_rotation()`: rotate the whole arc.
    pub fn set_rotation(self, ui: &mut Ui, mut rotation: i32) {
        let Some(arc) = state_mut(ui, self.0) else { return };

        /* ensure the angle is in the range [0, 360) */
        while rotation < 0 {
            rotation += 360;
        }
        while rotation >= 360 {
            rotation -= 360;
        }
        arc.rotation = rotation;

        ui.obj_invalidate(self.0);
    }

    /// `lv_arc_set_mode()`: in which direction the indicator grows.
    pub fn set_mode(self, ui: &mut Ui, type_: ArcMode) {
        let Some(arc) = state_mut(ui, self.0) else { return };

        let val = arc.value;

        arc.type_ = type_;
        arc.value = -1; /* Force set_value handling*/

        let mut bg_end = arc.bg_angle_end;
        if arc.bg_angle_end < arc.bg_angle_start {
            bg_end = arc.bg_angle_end + vp(360);
        }
        let bg_angle_start = arc.bg_angle_start;
        let bg_angle_end = arc.bg_angle_end;

        match type_ {
            ArcMode::Symmetrical => {
                let bg_midpoint = (bg_angle_start + bg_end).div(vp(2));
                self.set_angles(ui, bg_midpoint, bg_midpoint);
            }
            ArcMode::Reverse => self.set_end_angle(ui, bg_angle_end),
            /* LV_ARC_TYPE_NORMAL*/
            ArcMode::Normal => self.set_start_angle(ui, bg_angle_start),
        }

        self.set_value(ui, val);
    }

    /// `lv_arc_set_value()`
    pub fn set_value(self, ui: &mut Ui, value: i32) {
        let Some(arc) = state_mut(ui, self.0) else { return };

        if arc.value == value {
            return;
        }

        let mut new_value = if value > arc.max_value {
            arc.max_value
        } else {
            value
        };
        new_value = if new_value < arc.min_value {
            arc.min_value
        } else {
            new_value
        };

        if arc.value == new_value {
            return;
        }
        arc.value = new_value;

        value_update(ui, self.0);
    }

    /// `lv_arc_set_range()`
    pub fn set_range(self, ui: &mut Ui, min: i32, max: i32) {
        let Some(arc) = state_mut(ui, self.0) else { return };

        if arc.min_value == min && arc.max_value == max {
            return;
        }

        arc.min_value = min;
        arc.max_value = max;

        if arc.value < min {
            arc.value = min;
        }
        if arc.value > max {
            arc.value = max;
        }

        value_update(ui, self.0); /*value has changed relative to the new range*/
    }

    /// `lv_arc_set_min_value()`
    pub fn set_min_value(self, ui: &mut Ui, min: i32) {
        if state(ui, self.0).is_none() {
            return; /*LV_CHECK_OBJ*/
        }

        let max = self.max_value(ui);
        self.set_range(ui, min, max);
    }

    /// `lv_arc_set_max_value()`
    pub fn set_max_value(self, ui: &mut Ui, max: i32) {
        if state(ui, self.0).is_none() {
            return; /*LV_CHECK_OBJ*/
        }

        let min = self.min_value(ui);
        self.set_range(ui, min, max);
    }

    /// `lv_arc_set_change_rate()`: limit how fast the arc reaches the pressed
    /// point (degrees/sec).
    pub fn set_change_rate(self, ui: &mut Ui, rate: u32) {
        let Some(arc) = state_mut(ui, self.0) else { return };

        arc.chg_rate = rate;
    }

    /// `lv_arc_set_knob_offset()`: knob offset from the main arc in degrees.
    pub fn set_knob_offset(self, ui: &mut Ui, offset: i32) {
        let Some(arc) = state_mut(ui, self.0) else { return };

        /*`int32_t` to `int16_t` field*/
        arc.knob_offset = offset as i16;
    }

    /// `lv_arc_get_angle_start()`
    pub fn angle_start(self, ui: &Ui) -> ValuePrecise {
        state(ui, self.0).map_or(vp(0), |arc| arc.indic_angle_start)
    }

    /// `lv_arc_get_angle_end()`
    pub fn angle_end(self, ui: &Ui) -> ValuePrecise {
        state(ui, self.0).map_or(vp(0), |arc| arc.indic_angle_end)
    }

    /// `lv_arc_get_bg_angle_start()`
    pub fn bg_angle_start(self, ui: &Ui) -> ValuePrecise {
        state(ui, self.0).map_or(vp(0), |arc| arc.bg_angle_start)
    }

    /// `lv_arc_get_bg_angle_end()`
    pub fn bg_angle_end(self, ui: &Ui) -> ValuePrecise {
        state(ui, self.0).map_or(vp(0), |arc| arc.bg_angle_end)
    }

    /// `lv_arc_get_value()`
    pub fn value(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |arc| arc.value)
    }

    /// `lv_arc_get_min_value()`
    pub fn min_value(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |arc| arc.min_value)
    }

    /// `lv_arc_get_max_value()`
    pub fn max_value(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |arc| arc.max_value)
    }

    /// `lv_arc_get_mode()`
    pub fn mode(self, ui: &Ui) -> ArcMode {
        state(ui, self.0).map_or(ArcMode::Normal, |arc| arc.type_)
    }

    /// `lv_arc_get_rotation()`
    pub fn rotation(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |arc| arc.rotation)
    }

    /// `lv_arc_get_knob_offset()`
    pub fn knob_offset(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |arc| i32::from(arc.knob_offset))
    }

    /// `lv_arc_get_change_rate()`
    pub fn change_rate(self, ui: &Ui) -> u32 {
        state(ui, self.0).map_or(0, |arc| arc.chg_rate)
    }

    /// `lv_arc_align_obj_to_angle()`: align `obj_to_align` to the current
    /// knob position; `r_offset` enlarges (> 0) or shrinks the radius.
    pub fn align_obj_to_angle(self, ui: &mut Ui, obj_to_align: Obj, r_offset: i32) {
        let obj = self.0;
        if state(ui, obj).is_none() || !ui.obj_is_valid(obj_to_align) {
            return; /*LV_CHECK_OBJ*/
        }

        ui.obj_update_layout(obj);

        let (_center, mut arc_r) = get_center(ui, obj);
        let indic_width = ui.obj_get_style_arc_width(obj, Part::INDICATOR);
        let indic_width_half = indic_width / 2;
        arc_r = arc_r.wrapping_sub(indic_width_half);
        arc_r = arc_r.wrapping_add(r_offset);

        let angle = get_angle(ui, obj).to_i32();
        let knob_x = arc_r.wrapping_mul(sin(angle.wrapping_add(90))) >> TRIGO_SHIFT;
        let knob_y = arc_r.wrapping_mul(sin(angle)) >> TRIGO_SHIFT;
        ui.obj_align_to(obj_to_align, obj, Align::Center, knob_x, knob_y);
    }

    /// `lv_arc_rotate_obj_to_angle()`: rotate `obj_to_rotate` to the current
    /// knob position.
    pub fn rotate_obj_to_angle(self, ui: &mut Ui, obj_to_rotate: Obj, r_offset: i32) {
        let obj = self.0;
        if state(ui, obj).is_none() || !ui.obj_is_valid(obj_to_rotate) {
            return; /*LV_CHECK_OBJ*/
        }

        ui.obj_update_layout(obj);

        let (center, mut arc_r) = get_center(ui, obj);
        let indic_width = ui.obj_get_style_arc_width(obj, Part::INDICATOR);
        let indic_width_half = indic_width / 2;
        arc_r = arc_r.wrapping_sub(indic_width_half);

        arc_r = arc_r.wrapping_add(r_offset);
        ui.obj_align_to(obj_to_rotate, obj, Align::Center, 0, arc_r.wrapping_neg());

        ui.obj_update_layout(obj);

        let angle = get_angle(ui, obj).to_i32();
        let coords = ui.obj_get_coords(obj_to_rotate);
        let pivot_x = coords.x1.wrapping_sub(center.x);
        let pivot_y = coords.y1.wrapping_sub(center.y);
        ui.obj_set_style_transform_pivot_x(obj_to_rotate, pivot_x.wrapping_neg(), StyleSelector::DEFAULT);
        ui.obj_set_style_transform_pivot_y(obj_to_rotate, pivot_y.wrapping_neg(), StyleSelector::DEFAULT);
        ui.obj_set_style_transform_rotation(
            obj_to_rotate,
            angle.wrapping_mul(10).wrapping_add(900),
            StyleSelector::DEFAULT,
        );
    }
}

impl From<Arc> for Obj {
    fn from(arc: Arc) -> Self {
        arc.obj()
    }
}

/// `lv_arc_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    let last_tick = ui.tick_get();
    let Some(arc) = state_mut(ui, obj) else { return };

    /*Initialize the allocated 'ext'*/
    arc.rotation = 0;
    arc.bg_angle_start = vp(135);
    arc.bg_angle_end = vp(45);
    arc.indic_angle_start = vp(135);
    arc.indic_angle_end = vp(270);
    arc.type_ = ArcMode::Normal;
    arc.value = VALUE_UNSET;
    arc.min_close = CLICK_CLOSER_TO_MIN_END;
    arc.min_value = 0;
    arc.max_value = 100;
    arc.dragging = false;
    arc.chg_rate = 720;
    arc.last_tick = last_tick;
    arc.last_angle = arc.indic_angle_end;
    arc.in_out = CLICK_OUTSIDE_BG_ANGLES;

    ui.obj_set_clickable(obj, true);
    ui.obj_set_scroll_chain(obj, false);
    ui.obj_set_scrollable(obj, false);
    ui.obj_set_ext_click_area(obj, DPI_DEF / 10);
}

/// `lv_arc_event()`
fn arc_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    /*Call the ancestor's event handler*/
    if ui.obj_event_base(&ARC_CLASS, event) != LvResult::Ok {
        return;
    }

    let code = event.code();
    let Some(obj) = event.current_target_obj() else {
        return;
    };
    if state(ui, obj).is_none() {
        return;
    }

    if code == EventCode::PRESSING {
        let Some(indev) = ui.indev_active() else { return };

        /*Handle only pointers here*/
        let indev_type = ui.indev_get_type(indev);
        if indev_type != IndevType::Pointer {
            return;
        }

        let mut p = ui.indev_get_point(indev);

        /*Make point relative to the arc's center*/
        let (center, mut r) = get_center(ui, obj);

        p.x = p.x.wrapping_sub(center.x);
        p.y = p.y.wrapping_sub(center.y);

        /*Enter dragging mode if pressed out of the knob*/
        if state(ui, obj).is_some_and(|arc| !arc.dragging) {
            let indic_width = ui.obj_get_style_arc_width(obj, Part::INDICATOR);
            r = r.wrapping_sub(indic_width);
            /*Add some more sensitive area if there is no advanced hit testing.
             * (Advanced hit testing is more precise)*/
            if ui.obj_is_adv_hittest(obj) {
                r = r.wrapping_sub(indic_width);
            } else {
                r = r.wrapping_sub((r / 4).max(indic_width));
            }
            if r < 1 {
                r = 1;
            }

            if p.x.wrapping_mul(p.x).wrapping_add(p.y.wrapping_mul(p.y)) > r.wrapping_mul(r) {
                let tick = ui.tick_get();
                let Some(arc) = state_mut(ui, obj) else { return };
                arc.dragging = true;
                arc.last_tick = tick; /*Capture timestamp at dragging start*/
            }
        }

        /*It must be in "dragging" mode to turn the arc*/
        if state(ui, obj).is_some_and(|arc| !arc.dragging) {
            return;
        }

        /*No angle can be determined if exactly the middle of the arc is being pressed*/
        if p.x == 0 && p.y == 0 {
            return;
        }

        let tolerance_deg = tolerance_deg(ui, r);
        let tick_now = ui.tick_get();
        let Some(arc) = state_mut(ui, obj) else { return };

        /*Calculate the angle of the pressed point*/
        let mut bg_end = arc.bg_angle_end;
        if arc.bg_angle_end < arc.bg_angle_start {
            bg_end = arc.bg_angle_end + vp(360);
        }

        let mut angle = vp(i32::from(atan2(p.y, p.x)));
        angle -= vp(arc.rotation);
        angle -= arc.bg_angle_start; /*Make the angle relative to the start angle*/

        /* ensure the angle is in the range [0, 360) */
        while angle < vp(0) {
            angle += vp(360);
        }
        while angle >= vp(360) {
            angle -= vp(360);
        }

        let min_close_prev = arc.min_close;

        let is_angle_within_bg_bounds = angle_within_bg_bounds(arc, angle, tolerance_deg);
        if !is_angle_within_bg_bounds {
            return;
        }

        let deg_range = bg_end - arc.bg_angle_start;
        let last_angle_rel = arc.last_angle - arc.bg_angle_start;
        let mut delta_angle = angle - last_angle_rel;

        /*Do not allow big jumps (jumps bigger than 280°).
         *It's mainly to avoid jumping to the opposite end if the "dead" range between min. and max. is crossed.
         *Check which end was closer on the last valid press (arc->min_close) and prefer that end*/
        if delta_angle.lv_abs() > vp(280) {
            if arc.min_close == CLICK_CLOSER_TO_MIN_END {
                angle = vp(0);
            } else {
                angle = deg_range;
            }
        }
        /* Check if click was outside the background arc start and end angles */
        else if CLICK_OUTSIDE_BG_ANGLES == arc.in_out {
            if arc.min_close == CLICK_CLOSER_TO_MIN_END {
                angle = -deg_range;
            } else {
                angle = deg_range;
            }
        }
        /* else: Keep the angle value */

        /* Prevent big jumps when the click goes from start to end angle in the invisible
         * part of the background arc without being released */
        if ((min_close_prev == CLICK_CLOSER_TO_MIN_END) && (arc.min_close == CLICK_CLOSER_TO_MAX_END))
            && ((CLICK_OUTSIDE_BG_ANGLES == arc.in_out) && (delta_angle.lv_abs() > vp(280)))
        {
            angle = vp(0);
            arc.min_close = min_close_prev;
        } else if ((min_close_prev == CLICK_CLOSER_TO_MAX_END) && (arc.min_close == CLICK_CLOSER_TO_MIN_END))
            && (CLICK_OUTSIDE_BG_ANGLES == arc.in_out)
            && (vp(360) - delta_angle.lv_abs() > vp(280))
        {
            angle = deg_range;
            arc.min_close = min_close_prev;
        }
        /* else: Keep the angle value */

        /*Calculate the slew rate limited angle based on change rate (degrees/sec)*/
        delta_angle = angle - last_angle_rel;

        let delta_tick = Ui::tick_diff(tick_now, arc.last_tick);
        /* delta_angle_max can never be signed. delta_tick is always signed, same for ch_rate */
        let delta_angle_max = ValuePrecise::from_u32(arc.chg_rate.wrapping_mul(delta_tick) / 1000);

        if delta_angle > delta_angle_max {
            delta_angle = delta_angle_max;
        } else if delta_angle < -delta_angle_max {
            delta_angle = -delta_angle_max;
        }
        /* else: Nothing to do */

        angle = last_angle_rel + delta_angle; /*Apply the limited angle change*/

        /*Rounding for symmetry*/
        let mut round =
            ((bg_end - arc.bg_angle_start) * vp(8)).div(vp(arc.max_value.wrapping_sub(arc.min_value)));
        round = (round + vp(4)).div(vp(16));
        angle += round;

        angle += arc.bg_angle_start; /*Make the angle absolute again*/

        /*Set the new value*/
        let old_value = arc.value;
        let mut new_value = map(
            angle.to_i32(),
            arc.bg_angle_start.to_i32(),
            bg_end.to_i32(),
            arc.min_value,
            arc.max_value,
        );
        if arc.type_ == ArcMode::Reverse {
            new_value = arc.max_value.wrapping_sub(new_value).wrapping_add(arc.min_value);
        }

        if new_value != Arc(obj).value(ui) {
            let tick = ui.tick_get();
            if let Some(arc) = state_mut(ui, obj) {
                arc.last_tick = tick; /*Cache timestamp for the next iteration*/
            }
            Arc(obj).set_value(ui, new_value); /*set_value caches the last_angle for the next iteration*/
            if new_value != old_value {
                let res = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
                if res != LvResult::Ok {
                    return;
                }
            }
        }

        /*Don't let the elapsed time become too big while sitting on an end point*/
        let tick = ui.tick_get();
        let Some(arc) = state_mut(ui, obj) else { return };
        if new_value == arc.min_value || new_value == arc.max_value {
            arc.last_tick = tick; /*Cache timestamp for the next iteration*/
        }
    } else if code == EventCode::RELEASED || code == EventCode::PRESS_LOST {
        let Some(arc) = state_mut(ui, obj) else { return };
        arc.dragging = false;

        /*Leave edit mode if released. (No need to wait for LONG_PRESS)*/
        let g = ui.obj_get_group(obj);
        let editing = g.is_some_and(|g| ui.group_get_editing(g));
        let indev = event.indev();
        let indev_type = indev.map_or(IndevType::None, |indev| ui.indev_get_type(indev));
        if indev_type == IndevType::Encoder
            && editing
            && let Some(g) = g
        {
            ui.group_set_editing(g, false);
        }
    } else if code == EventCode::KEY {
        let c = event.key();

        let arc = Arc(obj);
        let old_value = arc.value(ui);
        if c == KEY_RIGHT || c == KEY_UP {
            let v = arc.value(ui).wrapping_add(1);
            arc.set_value(ui, v);
        } else if c == KEY_LEFT || c == KEY_DOWN {
            let v = arc.value(ui).wrapping_sub(1);
            arc.set_value(ui, v);
        }

        if old_value != arc.value(ui) {
            /*The result only decides an early return at the end of the handler*/
            let _ = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
        }
    } else if code == EventCode::ROTARY {
        let r = event.rotary_diff();

        let arc = Arc(obj);
        let old_value = arc.value(ui);
        let v = arc.value(ui).wrapping_add(r);
        arc.set_value(ui, v);
        if old_value != arc.value(ui) {
            /*The result only decides an early return at the end of the handler*/
            let _ = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
        }
    } else if code == EventCode::HIT_TEST {
        let Some(point) = event.hit_test_info().map(|info| info.point) else {
            return;
        };

        let (p, mut r) = get_center(ui, obj);

        let mut ext_click_area = 0;
        if let Some(spec) = ui.obj(obj).and_then(|d| d.spec.as_ref()) {
            ext_click_area = spec.ext_click_pad;
        }

        let w = ui.obj_get_style_arc_width(obj, Part::MAIN);
        r = r.wrapping_sub(w.wrapping_add(ext_click_area));

        let mut a = Area::default();
        /*Invalid if clicked inside*/
        a.set(
            p.x.wrapping_sub(r),
            p.y.wrapping_sub(r),
            p.x.wrapping_add(r),
            p.y.wrapping_add(r),
        );
        if a.is_point_on(point, RADIUS_CIRCLE) {
            if let Some(info) = event.hit_test_info() {
                info.res = false;
            }
            return;
        }

        let tolerance_deg = tolerance_deg(ui, r);
        let Some(arc) = state_mut(ui, obj) else { return };

        /*Calculate the angle of the pressed point*/
        let mut angle = vp(i32::from(atan2(
            point.y.wrapping_sub(p.y),
            point.x.wrapping_sub(p.x),
        )));
        angle -= vp(arc.rotation);
        angle -= arc.bg_angle_start; /*Make the angle relative to the start angle*/

        /* ensure the angle is in the range [0, 360) */
        while angle < vp(0) {
            angle += vp(360);
        }
        while angle >= vp(360) {
            angle -= vp(360);
        }

        /* Check if the angle is outside the drawn background arc */
        let is_angle_within_bg_bounds = angle_within_bg_bounds(arc, angle, tolerance_deg);
        if !is_angle_within_bg_bounds {
            if let Some(info) = event.hit_test_info() {
                info.res = false;
            }
            return;
        }

        /*Valid if no clicked outside*/
        let extra = w.wrapping_add(ext_click_area.wrapping_mul(2));
        a.increase(extra, extra);
        let res = a.is_point_on(point, RADIUS_CIRCLE);
        if let Some(info) = event.hit_test_info() {
            info.res = res;
        }
    } else if code == EventCode::REFR_EXT_DRAW_SIZE {
        let bg_left = ui.obj_get_style_pad_left(obj, Part::MAIN);
        let bg_right = ui.obj_get_style_pad_right(obj, Part::MAIN);
        let bg_top = ui.obj_get_style_pad_top(obj, Part::MAIN);
        let bg_bottom = ui.obj_get_style_pad_bottom(obj, Part::MAIN);
        let bg_pad = bg_left.max(bg_right).max(bg_top.max(bg_bottom));

        let knob_left = ui.obj_get_style_pad_left(obj, Part::KNOB);
        let knob_right = ui.obj_get_style_pad_right(obj, Part::KNOB);
        let knob_top = ui.obj_get_style_pad_top(obj, Part::KNOB);
        let knob_bottom = ui.obj_get_style_pad_bottom(obj, Part::KNOB);
        let knob_pad = knob_left
            .max(knob_right)
            .max(knob_top.max(knob_bottom))
            .wrapping_add(2);

        let mut knob_extra_size = knob_pad.wrapping_sub(bg_pad);
        knob_extra_size = knob_extra_size.wrapping_add(knob_get_extra_size(ui, obj));

        let indic_ext = ui.obj_calculate_ext_draw_size(obj, Part::INDICATOR);
        if let EventParam::ExtDrawSize(s) = event.param() {
            **s = (**s).max(knob_extra_size);
            **s = (**s).max(indic_ext);
        }
    } else if code == EventCode::DRAW_MAIN {
        arc_draw(ui, obj, event);
    }
}

/// `const lv_value_precise_t tolerance_deg = (360 * lv_dpx(20U)) / circumference;`
/// with `circumference = (uint32_t)((2U * r * 314U) / 100U)` (unsigned
/// arithmetic, so a negative `r` wraps as in C).
fn tolerance_deg(ui: &Ui, r: i32) -> ValuePrecise {
    /* Equivalent to: 2r * 3.14, avoiding floats */
    let circumference = 2u32.wrapping_mul(r as u32).wrapping_mul(314) / 100;
    let num = 360i32.wrapping_mul(ui.dpx(20)) as u32;
    /*A zero circumference (r == 0) divides by zero upstream (undefined
     *behavior); the port yields 0 instead of panicking.*/
    ValuePrecise::from_u32(num.checked_div(circumference).unwrap_or(0))
}

/// `lv_arc_draw()`
fn arc_draw(ui: &mut Ui, obj: Obj, event: &Event<'_>) {
    let Some(layer) = event.layer() else { return };
    let Some(arc) = state(ui, obj) else { return };
    let bg_angle_start = arc.bg_angle_start;
    let bg_angle_end = arc.bg_angle_end;
    let indic_angle_start = arc.indic_angle_start;
    let indic_angle_end = arc.indic_angle_end;
    let rotation = vp(arc.rotation);

    let (center, arc_r) = get_center(ui, obj);

    /*Draw the background arc*/
    if arc_r > 0 {
        let mut arc_dsc = ArcDsc::new();
        arc_dsc.base.layer = Some(layer);
        ui.obj_init_draw_arc_dsc(obj, Part::MAIN, &mut arc_dsc);
        arc_dsc.center = center;
        arc_dsc.start_angle = bg_angle_start + rotation;
        arc_dsc.end_angle = bg_angle_end + rotation;
        arc_dsc.radius = arc_r as u16;
        ui.draw_arc(layer, &arc_dsc);
    }

    /*Make the indicator arc smaller or larger according to its greatest padding value*/
    let indic_r = arc_r.wrapping_sub(get_indicator_max_pad(ui, obj));

    if indic_r > 0 {
        let mut arc_dsc = ArcDsc::new();
        arc_dsc.base.layer = Some(layer);
        ui.obj_init_draw_arc_dsc(obj, Part::INDICATOR, &mut arc_dsc);
        arc_dsc.center = center;
        arc_dsc.start_angle = indic_angle_start + rotation;
        arc_dsc.end_angle = indic_angle_end + rotation;

        arc_dsc.radius = indic_r as u16;

        ui.draw_arc(layer, &arc_dsc);
    }

    let mut knob_area = Area::default();
    get_knob_area(ui, obj, &center, arc_r, &mut knob_area);

    let mut knob_rect_dsc = RectDsc::new();
    knob_rect_dsc.base.layer = Some(layer);
    ui.obj_init_draw_rect_dsc(obj, Part::KNOB, &mut knob_rect_dsc);
    ui.draw_rect(layer, &knob_rect_dsc, &knob_area);
}

/// `inv_arc_area()`
fn inv_arc_area(
    ui: &mut Ui,
    obj: Obj,
    mut start_angle: ValuePrecise,
    mut end_angle: ValuePrecise,
    part: Part,
) {
    let Some(arc) = state(ui, obj) else { return };

    if start_angle == end_angle {
        return;
    }

    start_angle += vp(arc.rotation);
    end_angle += vp(arc.rotation);

    while start_angle < vp(0) {
        start_angle += vp(360);
    }
    while start_angle >= vp(360) {
        start_angle -= vp(360);
    }
    while end_angle < vp(0) {
        end_angle += vp(360);
    }
    while end_angle >= vp(360) {
        end_angle -= vp(360);
    }

    let (c, mut r) = get_center(ui, obj);

    if part == Part::INDICATOR {
        r = r.wrapping_sub(get_indicator_max_pad(ui, obj));
    }

    let w = ui.obj_get_style_arc_width(obj, part);
    let rounded = ui.obj_get_style_arc_rounded(obj, part);

    /*Guard: if radius is zero or negative after padding, invalidate full object (incl. ext-draw area)*/
    if r <= 0 {
        ui.obj_invalidate(obj);
        return;
    }

    let mut inv_area = Area::default();
    /*`int32_t` radius to the `uint16_t` parameter*/
    arc_get_area(
        c.x,
        c.y,
        r as u16,
        start_angle,
        end_angle,
        w,
        rounded,
        &mut inv_area,
    );

    ui.obj_invalidate_area(obj, &inv_area);
}

/// `get_knob_inv_area()`
fn get_knob_inv_area(ui: &Ui, obj: Obj, area: &mut Area) -> bool {
    if ui.obj_get_style_bg_opa(obj, Part::KNOB) <= OPA_MIN
        && ui.obj_get_style_bg_image_src(obj, Part::KNOB).is_none()
        && ui.obj_get_style_border_opa(obj, Part::KNOB) <= OPA_MIN
        && ui.obj_get_style_border_width(obj, Part::KNOB) <= 0
        && ui.obj_get_style_outline_opa(obj, Part::KNOB) <= OPA_MIN
        && ui.obj_get_style_outline_width(obj, Part::KNOB) <= 0
        && ui.obj_get_style_shadow_opa(obj, Part::KNOB) <= OPA_MIN
        && ui.obj_get_style_shadow_width(obj, Part::KNOB) <= 0
    {
        return false;
    }

    let (c, r) = get_center(ui, obj);

    get_knob_area(ui, obj, &c, r, area);

    let knob_extra_size = knob_get_extra_size(ui, obj);

    if knob_extra_size > 0 {
        area.increase(knob_extra_size, knob_extra_size);
    }

    true
}

/// `inv_knob_area()`
fn inv_knob_area(ui: &mut Ui, obj: Obj) {
    let mut a = Area::default();
    if get_knob_inv_area(ui, obj, &mut a) {
        ui.obj_invalidate_area(obj, &a);
    }
}

/// `get_center()`: the arc's center and radius.
fn get_center(ui: &Ui, obj: Obj) -> (Point, i32) {
    let left_bg = ui.obj_get_style_pad_left(obj, Part::MAIN);
    let right_bg = ui.obj_get_style_pad_right(obj, Part::MAIN);
    let top_bg = ui.obj_get_style_pad_top(obj, Part::MAIN);
    let bottom_bg = ui.obj_get_style_pad_bottom(obj, Part::MAIN);

    let r = (ui.obj_get_width(obj).wrapping_sub(left_bg).wrapping_sub(right_bg)).min(
        ui.obj_get_height(obj)
            .wrapping_sub(top_bg)
            .wrapping_sub(bottom_bg),
    ) / 2;

    let coords = ui.obj_get_coords(obj);
    let center = Point::new(
        coords.x1.wrapping_add(r).wrapping_add(left_bg),
        coords.y1.wrapping_add(r).wrapping_add(top_bg),
    );
    (center, r)
}

/// `get_angle()`: the knob's angle.
fn get_angle(ui: &Ui, obj: Obj) -> ValuePrecise {
    let Some(arc) = state(ui, obj) else {
        return vp(0);
    };
    let mut angle = vp(arc.rotation);
    if arc.type_ == ArcMode::Normal {
        angle += arc.indic_angle_end;
    } else if arc.type_ == ArcMode::Reverse {
        angle += arc.indic_angle_start;
    } else if arc.type_ == ArcMode::Symmetrical {
        let mut bg_end = arc.bg_angle_end;
        if arc.bg_angle_end < arc.bg_angle_start {
            bg_end = arc.bg_angle_end + vp(360);
        }
        let mut indic_end = arc.indic_angle_end;
        if arc.indic_angle_end < arc.indic_angle_start {
            indic_end = arc.indic_angle_end + vp(360);
        }

        let angle_midpoint = vp((arc.bg_angle_start + bg_end).to_i32() / 2);
        if arc.indic_angle_start < angle_midpoint {
            angle += arc.indic_angle_start;
        } else if indic_end > angle_midpoint {
            angle += arc.indic_angle_end;
        } else {
            angle += angle_midpoint;
        }
    }

    angle
}

/// `get_knob_area()`
fn get_knob_area(ui: &Ui, obj: Obj, center: &Point, mut r: i32, knob_area: &mut Area) {
    let indic_width = ui.obj_get_style_arc_width(obj, Part::INDICATOR);
    let indic_width_half = indic_width / 2;
    r = r.wrapping_sub(indic_width_half);

    r = r.wrapping_sub(get_indicator_max_pad(ui, obj));

    let angle = get_angle(ui, obj).to_i32();
    let knob_offset = Arc(obj).knob_offset(ui);
    let knob_x = r.wrapping_mul(sin(knob_offset.wrapping_add(angle).wrapping_add(90))) >> TRIGO_SHIFT;
    let knob_y = r.wrapping_mul(sin(knob_offset.wrapping_add(angle))) >> TRIGO_SHIFT;

    let left_knob = ui.obj_get_style_pad_left(obj, Part::KNOB);
    let right_knob = ui.obj_get_style_pad_right(obj, Part::KNOB);
    let top_knob = ui.obj_get_style_pad_top(obj, Part::KNOB);
    let bottom_knob = ui.obj_get_style_pad_bottom(obj, Part::KNOB);

    knob_area.x1 = center
        .x
        .wrapping_add(knob_x)
        .wrapping_sub(left_knob)
        .wrapping_sub(indic_width_half);
    knob_area.x2 = center
        .x
        .wrapping_add(knob_x)
        .wrapping_add(right_knob)
        .wrapping_add(indic_width_half);
    knob_area.y1 = center
        .y
        .wrapping_add(knob_y)
        .wrapping_sub(top_knob)
        .wrapping_sub(indic_width_half);
    knob_area.y2 = center
        .y
        .wrapping_add(knob_y)
        .wrapping_add(bottom_knob)
        .wrapping_add(indic_width_half);
}

/// `value_update()`: used internally to update arc angles after a value
/// change.
fn value_update(ui: &mut Ui, obj: Obj) {
    let Some(arc) = state(ui, obj) else { return };

    /*If the value is still not set to any value do not update*/
    if arc.value == VALUE_UNSET {
        return;
    }

    let mut bg_end = arc.bg_angle_end;
    if arc.bg_angle_end < arc.bg_angle_start {
        bg_end = arc.bg_angle_end + vp(360);
    }
    let value = arc.value;
    let min_value = arc.min_value;
    let max_value = arc.max_value;
    let bg_angle_start = arc.bg_angle_start;
    let bg_angle_end = arc.bg_angle_end;
    let arc_obj = Arc(obj);

    let angle;
    match arc.type_ {
        ArcMode::Symmetrical => {
            let bg_midpoint = (bg_angle_start + bg_end).div(vp(2));
            let range_midpoint = min_value.wrapping_add(max_value) / 2;

            if value < range_midpoint {
                angle = map(
                    value,
                    min_value,
                    range_midpoint,
                    bg_angle_start.to_i32(),
                    bg_midpoint.to_i32(),
                );
                arc_obj.set_angles(ui, vp(angle), bg_midpoint);
            } else {
                angle = map(
                    value,
                    range_midpoint,
                    max_value,
                    bg_midpoint.to_i32(),
                    bg_end.to_i32(),
                );
                arc_obj.set_angles(ui, bg_midpoint, vp(angle));
            }
        }
        ArcMode::Reverse => {
            angle = map(
                value,
                min_value,
                max_value,
                bg_end.to_i32(),
                bg_angle_start.to_i32(),
            );
            arc_obj.set_angles(ui, vp(angle), bg_angle_end);
        }
        ArcMode::Normal => {
            angle = map(
                value,
                min_value,
                max_value,
                bg_angle_start.to_i32(),
                bg_end.to_i32(),
            );
            arc_obj.set_angles(ui, bg_angle_start, vp(angle));
        }
    }
    if let Some(arc) = state_mut(ui, obj) {
        arc.last_angle = vp(angle); /*Cache angle for slew rate limiting*/
    }
}

/// `knob_get_extra_size()`
fn knob_get_extra_size(ui: &Ui, obj: Obj) -> i32 {
    let mut knob_shadow_size: i32 = 0;
    knob_shadow_size = knob_shadow_size.wrapping_add(ui.obj_get_style_shadow_width(obj, Part::KNOB));
    knob_shadow_size = knob_shadow_size.wrapping_add(ui.obj_get_style_shadow_spread(obj, Part::KNOB));
    knob_shadow_size =
        knob_shadow_size.wrapping_add(ui.obj_get_style_shadow_offset_x(obj, Part::KNOB).lv_abs());
    knob_shadow_size =
        knob_shadow_size.wrapping_add(ui.obj_get_style_shadow_offset_y(obj, Part::KNOB).lv_abs());

    let mut knob_outline_size: i32 = 0;
    knob_outline_size = knob_outline_size.wrapping_add(ui.obj_get_style_outline_width(obj, Part::KNOB));
    knob_outline_size = knob_outline_size.wrapping_add(ui.obj_get_style_outline_pad(obj, Part::KNOB));

    knob_shadow_size.max(knob_outline_size)
}

/// `lv_arc_angle_within_bg_bounds()`: check if `angle` (relative to
/// `bg_angle_start`, `0 <= angle <= 360`) is within the background arc,
/// allowing `tolerance_deg` extra room. Updates `min_close` and `in_out`.
fn angle_within_bg_bounds(arc: &mut ArcState, angle: ValuePrecise, tolerance_deg: ValuePrecise) -> bool {
    let mut bounds_angle = arc.bg_angle_end - arc.bg_angle_start;
    if arc.bg_angle_end == arc.bg_angle_start {
        return false; /*The arc has 0 deg span*/
    }

    /* ensure the angle is in the range [0, 360) */
    while bounds_angle < vp(0) {
        bounds_angle += vp(360);
    }
    while bounds_angle >= vp(360) {
        bounds_angle -= vp(360);
    }

    /*Full circle*/
    if bounds_angle == vp(0) {
        bounds_angle = vp(360);
    }

    /* Angle is in the bounds */
    if angle <= bounds_angle {
        if angle < bounds_angle.div(vp(2)) {
            arc.min_close = CLICK_CLOSER_TO_MIN_END;
        } else {
            arc.min_close = CLICK_CLOSER_TO_MAX_END;
        }
        arc.in_out = CLICK_INSIDE_BG_ANGLES;
        return true;
    }

    /* Distance between background start and end angles is less than tolerance,
     * consider the click inside the arc */
    if vp(360) - bounds_angle <= tolerance_deg {
        arc.min_close = CLICK_CLOSER_TO_MIN_END;
        arc.in_out = CLICK_INSIDE_BG_ANGLES;
        return true;
    }

    /* angle is within the tolerance of the min end */
    if vp(360) - angle <= tolerance_deg {
        arc.min_close = CLICK_CLOSER_TO_MIN_END;
        arc.in_out = CLICK_OUTSIDE_BG_ANGLES;
        return true;
    }

    /* angle is within the tolerance of the max end */
    if angle <= bounds_angle + tolerance_deg {
        arc.min_close = CLICK_CLOSER_TO_MAX_END;
        arc.in_out = CLICK_OUTSIDE_BG_ANGLES;
        return true;
    }

    false
}

/// `get_indicator_max_pad()`
fn get_indicator_max_pad(ui: &Ui, obj: Obj) -> i32 {
    let left_indic = ui.obj_get_style_pad_left(obj, Part::INDICATOR);
    let right_indic = ui.obj_get_style_pad_right(obj, Part::INDICATOR);
    let top_indic = ui.obj_get_style_pad_top(obj, Part::INDICATOR);
    let bottom_indic = ui.obj_get_style_pad_bottom(obj, Part::INDICATOR);
    left_indic.max(right_indic).max(top_indic.max(bottom_indic))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::UiConfig;

    fn setup() -> (Ui, Arc) {
        let mut ui = Ui::new(UiConfig::default());
        let display = ui.display_create(320, 240);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let arc = Arc::create(&mut ui, screen);
        (ui, arc)
    }

    #[test]
    fn class_defaults_match_c() {
        let (ui, arc) = setup();
        let obj = arc.obj();
        assert!(ui.obj_check_type(obj, &ARC_CLASS));
        assert!(ui.obj_is_clickable(obj));
        assert!(!ui.obj_is_scrollable(obj));
        assert!(!ui.obj_is_scroll_chain_hor(obj) && !ui.obj_is_scroll_chain_ver(obj));
        assert_eq!(
            ui.obj(obj).unwrap().spec.as_ref().unwrap().ext_click_pad,
            DPI_DEF / 10
        );
        assert_eq!(arc.value(&ui), VALUE_UNSET);
        assert_eq!((arc.min_value(&ui), arc.max_value(&ui)), (0, 100));
        assert_eq!(
            (arc.bg_angle_start(&ui), arc.bg_angle_end(&ui)),
            (vp(135), vp(45))
        );
        assert_eq!((arc.angle_start(&ui), arc.angle_end(&ui)), (vp(135), vp(270)));
        assert_eq!(arc.mode(&ui), ArcMode::Normal);
        assert_eq!(arc.change_rate(&ui), 720);
        assert_eq!(Arc::from_obj(&ui, obj), Some(arc));
    }

    #[test]
    fn setters_normalize_like_c() {
        let (mut ui, arc) = setup();
        arc.set_rotation(&mut ui, -30);
        assert_eq!(arc.rotation(&ui), 330);
        arc.set_rotation(&mut ui, 720);
        assert_eq!(arc.rotation(&ui), 0);
        arc.set_angles(&mut ui, vp(-90), vp(720));
        assert_eq!((arc.angle_start(&ui), arc.angle_end(&ui)), (vp(270), vp(360)));
        arc.set_knob_offset(&mut ui, 0x1_0005);
        assert_eq!(arc.knob_offset(&ui), 5);
        arc.set_mode(&mut ui, ArcMode::Reverse);
        assert_eq!(arc.value(&ui), 0);
        assert_eq!(arc.angle_start(&ui), vp(45));
    }

    #[test]
    fn non_arc_objects_are_rejected() {
        let (mut ui, arc) = setup();
        let parent = ui.obj_get_parent(arc.obj());
        let plain = Arc(ui.obj_create(parent));
        plain.set_value(&mut ui, 50);
        assert_eq!(plain.value(&ui), 0);
        assert!(Arc::from_obj(&ui, plain.obj()).is_none());
    }

    #[test]
    fn precise_helpers_match_c() {
        assert_eq!(<i32 as Precise>::lv_abs(i32::MIN), i32::MIN);
        assert_eq!(<i32 as Precise>::div(7, 0), 0);
        assert_eq!(<i32 as Precise>::div(-7, 2), -3);
        assert_eq!(<f32 as Precise>::to_i32(-1.6), -1);
        assert_eq!(<f32 as Precise>::from_u32(u32::MAX), 4_294_967_296.0);
    }
}
