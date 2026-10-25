//! Ported from LVGL v9.6.0: `src/widgets/switch/lv_switch.c`.
//!
//! Also covers `include/lvgl/widgets/lv_switch.h` and
//! `src/widgets/switch/lv_switch_private.h`.
//!
//! The knob animation is a regular core animation whose variable is the
//! switch object itself (`lv_anim_set_var(&a, sw)`), so `lv_anim_delete(sw,
//! NULL)` and object deletion stop it exactly as in C. The optional
//! object-property registry (`LV_USE_OBJ_PROPERTY`) is not exposed.

use alloc::boxed::Box;
use alloc::rc::Rc;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::core::ui::DPI_DEF;
use ferrovgl_core::draw::RectDsc;
use ferrovgl_core::misc::anim::{AnimExec, AnimTarget};
use ferrovgl_core::misc::text::BaseDir;
use ferrovgl_core::{Anim, AnimId, Event, EventCode, LvResult, Obj, ObjClass, Part, State, Ui};

/// `LV_SWITCH_KNOB_EXT_AREA_CORRECTION`: switch knob extra area correction factor.
pub const SWITCH_KNOB_EXT_AREA_CORRECTION: i32 = 2;
/// `LV_SWITCH_ANIM_STATE_START`: switch animation start value. (Not the real
/// value of the switch, it just indicates the animation's progress.)
const SWITCH_ANIM_STATE_START: i32 = 0;
/// `LV_SWITCH_ANIM_STATE_END`: switch animation end value.
const SWITCH_ANIM_STATE_END: i32 = 256;
/// `LV_SWITCH_ANIM_STATE_INV`: no animation is in progress.
const SWITCH_ANIM_STATE_INV: i32 = -1;

/// Identity of `lv_switch_anim_exec_cb` (compared like a C function pointer).
const ANIM_TAG_SWITCH: u32 = 0x4C53_5754;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    #[must_use]
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the switch class.
    #[must_use]
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &SWITCH_CLASS)).then_some(Self(obj))
    }

    /// `lv_switch_set_orientation()`
    pub fn set_orientation(self, ui: &mut Ui, orientation: SwitchOrientation) {
        let Some(sw) = state_mut(ui, self.0) else {
            return;
        };
        sw.orientation = orientation;
        ui.obj_invalidate(self.0);
    }

    /// `lv_switch_get_orientation()`
    #[must_use]
    pub fn orientation(self, ui: &Ui) -> SwitchOrientation {
        // C returns 0 (`LV_SWITCH_ORIENTATION_AUTO`) for a wrong object.
        state(ui, self.0).map_or(SwitchOrientation::Auto, |sw| sw.orientation)
    }

    /// `((lv_switch_t *)obj)->anim_state`: the knob animation's progress in
    /// `0..=256`, or `-1` (`LV_SWITCH_ANIM_STATE_INV`) when no animation runs.
    /// Upstream `test_switch.c` reads this private `lv_switch_t` field directly.
    ///
    /// Hidden: a test hook for the ported upstream tests, which read this
    /// private field; it may change in any release.
    #[doc(hidden)]
    #[must_use]
    pub fn anim_state(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(SWITCH_ANIM_STATE_INV, |sw| sw.anim_state)
    }
}

impl From<Switch> for Obj {
    fn from(switch: Switch) -> Self {
        switch.obj()
    }
}

fn state(ui: &Ui, obj: Obj) -> Option<&SwitchState> {
    ui.obj(obj)?.widget.get::<SwitchState>()
}

fn state_mut(ui: &mut Ui, obj: Obj) -> Option<&mut SwitchState> {
    ui.obj_mut(obj)?.widget.get_mut::<SwitchState>()
}

/// `LV_SWITCH_IS_ANIMATING(sw)`
fn is_animating(sw: &SwitchState) -> bool {
    sw.anim_state != SWITCH_ANIM_STATE_INV
}

/// `lv_switch_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    if let Some(sw) = state_mut(ui, obj) {
        sw.anim_state = SWITCH_ANIM_STATE_INV;
        sw.orientation = SwitchOrientation::Auto;
    }

    ui.obj_set_scrollable(obj, false);
    ui.obj_set_checkable(obj, true);
    ui.obj_set_scroll_on_focus(obj, true);
}

/// `lv_switch_destructor()`
fn destructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    ui.anim_delete(AnimTarget::Obj(obj), None);
}

/// `lv_switch_event()`
fn switch_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    /*Call the ancestor's event handler*/
    if ui.obj_event_base(&SWITCH_CLASS, event) != LvResult::Ok {
        return;
    }

    let code = event.code();
    let Some(obj) = event.current_target_obj() else {
        return;
    };

    if code == EventCode::REFR_EXT_DRAW_SIZE {
        let knob_left = ui.obj_get_style_pad_left(obj, Part::KNOB);
        let knob_right = ui.obj_get_style_pad_right(obj, Part::KNOB);
        let knob_top = ui.obj_get_style_pad_top(obj, Part::KNOB);
        let knob_bottom = ui.obj_get_style_pad_bottom(obj, Part::KNOB);

        /*The smaller size is the knob diameter*/
        let mut knob_size = knob_left.max(knob_right).max(knob_bottom).max(knob_top);
        knob_size += SWITCH_KNOB_EXT_AREA_CORRECTION;
        knob_size += ui.obj_calculate_ext_draw_size(obj, Part::KNOB);

        // `*s = LV_MAX(*s, ...)` twice.
        event.set_ext_draw_size(knob_size);
        event.set_ext_draw_size(ui.obj_calculate_ext_draw_size(obj, Part::INDICATOR));
    } else if code == EventCode::STATE_CHANGED {
        let prev_state = event.prev_state();
        let diff = prev_state ^ ui.obj_get_state(obj);

        if diff.intersects(State::CHECKED) {
            trigger_anim(ui, obj);
            if ui.obj_is_valid(obj) {
                ui.obj_invalidate(obj);
            }
        }
    } else if code == EventCode::DRAW_MAIN {
        draw_main(ui, obj, event);
    }
}

/// `draw_main()`
fn draw_main(ui: &mut Ui, obj: Obj, event: &Event<'_>) {
    let Some(layer) = event.layer() else {
        return;
    };

    /*Draw the indicator*/
    /*Exclude background's padding*/
    let indic_area = ui.obj_get_content_coords(obj);

    let mut draw_indic_dsc = RectDsc::new();
    draw_indic_dsc.base.layer = Some(layer);
    ui.obj_init_draw_rect_dsc(obj, Part::INDICATOR, &mut draw_indic_dsc);
    ui.draw_rect(layer, &draw_indic_dsc, &indic_area);

    // `LV_EVENT_DRAW_TASK_ADDED` callbacks may have deleted the switch.
    let Some(sw) = state(ui, obj) else {
        return;
    };
    let (anim_state, animating, orientation) = (sw.anim_state, is_animating(sw), sw.orientation);

    /*Draw the knob*/
    let coords = ui.obj_get_coords(obj);
    let mut knob_area = coords;

    let switch_w = coords.width();
    let switch_h = coords.height();

    let hor = match orientation {
        SwitchOrientation::Horizontal => true,
        SwitchOrientation::Vertical => false,
        SwitchOrientation::Auto => switch_w >= switch_h,
    };

    if hor {
        let mut anim_value_x;
        let knob_size = ui.obj_get_height(obj);
        let anim_length = coords.width() - knob_size;
        if animating {
            /* Use the animation's coordinate */
            anim_value_x = anim_length.wrapping_mul(anim_state) / SWITCH_ANIM_STATE_END;
        } else {
            /* Use LV_STATE_CHECKED to decide the coordinate */
            let chk = ui.obj_get_state(obj).intersects(State::CHECKED);
            anim_value_x = if chk { anim_length } else { 0 };
        }

        if ui.obj_get_style_base_dir(obj, Part::MAIN) == BaseDir::Rtl {
            anim_value_x = anim_length - anim_value_x;
        }
        knob_area.x1 += anim_value_x;
        knob_area.x2 = knob_area.x1 + if knob_size > 0 { knob_size - 1 } else { 0 };
    } else {
        let mut anim_value_y;
        let knob_size = ui.obj_get_width(obj);
        let anim_length = coords.height() - knob_size;
        if animating {
            /* Use the animation's coordinate */
            anim_value_y = anim_length.wrapping_mul(anim_state) / SWITCH_ANIM_STATE_END;
        } else {
            /* Use LV_STATE_CHECKED to decide the coordinate */
            let chk = ui.obj_get_state(obj).intersects(State::CHECKED);
            anim_value_y = if chk { anim_length } else { 0 };
        }

        if ui.obj_get_style_base_dir(obj, Part::MAIN) == BaseDir::Rtl {
            anim_value_y = anim_length - anim_value_y;
        }

        knob_area.y2 -= anim_value_y;
        knob_area.y1 = knob_area.y2 - if knob_size > 0 { knob_size - 1 } else { 0 };
    }

    let knob_left = ui.obj_get_style_pad_left(obj, Part::KNOB);
    let knob_right = ui.obj_get_style_pad_right(obj, Part::KNOB);
    let knob_top = ui.obj_get_style_pad_top(obj, Part::KNOB);
    let knob_bottom = ui.obj_get_style_pad_bottom(obj, Part::KNOB);

    /*Apply the paddings on the knob area*/
    knob_area.x1 -= knob_left;
    knob_area.x2 += knob_right;
    knob_area.y1 -= knob_top;
    knob_area.y2 += knob_bottom;

    let mut knob_rect_dsc = RectDsc::new();
    knob_rect_dsc.base.layer = Some(layer);
    ui.obj_init_draw_rect_dsc(obj, Part::KNOB, &mut knob_rect_dsc);

    ui.draw_rect(layer, &knob_rect_dsc, &knob_area);
}

/// `lv_switch_anim_exec_cb()`
fn anim_exec() -> AnimExec {
    AnimExec::Custom {
        tag: ANIM_TAG_SWITCH,
        f: Rc::new(|ui: &mut Ui, var: AnimTarget, value: i32| {
            if let AnimTarget::Obj(obj) = var
                && let Some(sw) = state_mut(ui, obj)
            {
                sw.anim_state = value;
                ui.obj_invalidate(obj);
            }
        }),
    }
}

/// `lv_switch_anim_completed()`: resets the switch's animation state to "no
/// animation in progress".
fn anim_completed(ui: &mut Ui, a: AnimId) {
    let Some(AnimTarget::Obj(obj)) = ui.anim(a).map(|a| a.var) else {
        return;
    };
    if let Some(sw) = state_mut(ui, obj) {
        sw.anim_state = SWITCH_ANIM_STATE_INV;
        ui.obj_invalidate(obj);
    }
}

/// `lv_switch_trigger_anim()`: starts an animation for the switch knob if the
/// `anim_duration` style property is greater than 0.
fn trigger_anim(ui: &mut Ui, obj: Obj) {
    // LV_CHECK_OBJ(obj, MY_CLASS, return);
    let Some(anim_state) = state(ui, obj).map(|sw| sw.anim_state) else {
        return;
    };
    /*If the widget is not rendered yet show state changes immediately*/
    if !ui.obj(obj).is_some_and(|o| o.rendered) {
        return;
    }

    let anim_dur_full: u32 = ui.obj_get_style_anim_duration(obj, Part::MAIN);

    if anim_dur_full > 0 {
        let chk = ui.obj_get_state(obj).intersects(State::CHECKED);
        let (anim_start, anim_end) = if anim_state == SWITCH_ANIM_STATE_INV {
            /*No animation in progress -> simply set the values*/
            (
                if chk {
                    SWITCH_ANIM_STATE_START
                } else {
                    SWITCH_ANIM_STATE_END
                },
                if chk {
                    SWITCH_ANIM_STATE_END
                } else {
                    SWITCH_ANIM_STATE_START
                },
            )
        } else {
            /*Animation in progress. Start from the animation end value*/
            (
                anim_state,
                if chk {
                    SWITCH_ANIM_STATE_END
                } else {
                    SWITCH_ANIM_STATE_START
                },
            )
        };
        /*Calculate actual animation duration*/
        let anim_dur = anim_dur_full.wrapping_mul(anim_start.wrapping_sub(anim_end).unsigned_abs())
            / SWITCH_ANIM_STATE_END as u32;

        /*Stop the previous animation if it exists*/
        ui.anim_delete(AnimTarget::Obj(obj), None);

        let mut a = Anim::new();
        a.set_var(AnimTarget::Obj(obj));
        a.set_exec_cb(anim_exec());
        a.set_values(anim_start, anim_end);
        a.set_completed_cb(Some(Rc::new(anim_completed)));
        a.set_duration(anim_dur);
        ui.anim_start(&a);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::UiConfig;

    fn setup() -> (Ui, Obj) {
        let mut ui = Ui::new(UiConfig::default());
        let display = ui.display_create(320, 240);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        (ui, screen)
    }

    #[test]
    fn constructor_sets_flags_state_and_default_size() {
        let (mut ui, screen) = setup();
        let sw = Switch::create(&mut ui, screen);
        assert!(ui.obj_check_type(sw.obj(), &SWITCH_CLASS));
        assert!(!ui.obj_is_scrollable(sw.obj()));
        assert!(ui.obj_is_checkable(sw.obj()));
        assert!(ui.obj_is_scroll_on_focus(sw.obj()));
        assert_eq!(sw.anim_state(&ui), SWITCH_ANIM_STATE_INV);
        assert_eq!(sw.orientation(&ui), SwitchOrientation::Auto);
        assert_eq!(Switch::from_obj(&ui, sw.obj()), Some(sw));
        assert_eq!(Switch::from_obj(&ui, screen), None);
    }

    #[test]
    fn orientation_round_trips_and_wrong_class_is_ignored() {
        let (mut ui, screen) = setup();
        let sw = Switch::create(&mut ui, screen);
        sw.set_orientation(&mut ui, SwitchOrientation::Vertical);
        assert_eq!(sw.orientation(&ui), SwitchOrientation::Vertical);
        sw.set_orientation(&mut ui, SwitchOrientation::Horizontal);
        assert_eq!(sw.orientation(&ui), SwitchOrientation::Horizontal);
        let not_switch = Switch(screen);
        not_switch.set_orientation(&mut ui, SwitchOrientation::Vertical);
        assert_eq!(not_switch.orientation(&ui), SwitchOrientation::Auto);
        assert_eq!(SwitchOrientation::Auto as u8, 0);
        assert_eq!(SwitchOrientation::Horizontal as u8, 1);
        assert_eq!(SwitchOrientation::Vertical as u8, 2);
    }

    #[test]
    fn unrendered_switch_changes_state_without_animation() {
        let (mut ui, screen) = setup();
        let sw = Switch::create(&mut ui, screen);
        ui.obj_set_style_anim_duration(sw.obj(), 200, Part::MAIN.into());
        ui.obj_add_state(sw.obj(), State::CHECKED);
        assert_eq!(sw.anim_state(&ui), SWITCH_ANIM_STATE_INV);
        assert!(ui.anim_get(AnimTarget::Obj(sw.obj()), None).is_none());
    }

    #[test]
    fn rendered_switch_animates_and_delete_stops_the_animation() {
        let (mut ui, screen) = setup();
        let sw = Switch::create(&mut ui, screen);
        ui.obj_set_style_anim_duration(sw.obj(), 256, Part::MAIN.into());
        if let Some(o) = ui.obj_mut(sw.obj()) {
            o.rendered = true;
        }
        ui.obj_add_state(sw.obj(), State::CHECKED);
        // early_apply runs the exec callback with the start value at once.
        assert_eq!(sw.anim_state(&ui), SWITCH_ANIM_STATE_START);
        let id = ui
            .anim_get(AnimTarget::Obj(sw.obj()), None)
            .expect("knob animation");
        assert_eq!(ui.anim(id).map(|a| a.duration), Some(256));
        assert_eq!(ui.anim(id).map(|a| (a.start_value, a.end_value)), Some((0, 256)));
        // Reversing mid-flight starts from the current value with a shorter duration.
        if let Some(s) = state_mut(&mut ui, sw.obj()) {
            s.anim_state = 64;
        }
        ui.obj_remove_state(sw.obj(), State::CHECKED);
        let id = ui
            .anim_get(AnimTarget::Obj(sw.obj()), None)
            .expect("reverse animation");
        assert_eq!(ui.anim(id).map(|a| (a.start_value, a.end_value)), Some((64, 0)));
        assert_eq!(ui.anim(id).map(|a| a.duration), Some(64));
        ui.obj_delete(sw.obj());
        assert!(ui.anim_get(AnimTarget::Obj(sw.obj()), None).is_none());
    }
}
