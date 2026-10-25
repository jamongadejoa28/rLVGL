//! Ported from LVGL v9.6.0: `src/widgets/slider/lv_slider.c`.
//!
//! Also covers `include/lvgl/widgets/lv_slider.h` and
//! `src/widgets/slider/lv_slider_private.h`. The slider derives from the bar:
//! its state embeds the bar state like `lv_slider_t` embeds `lv_bar_t`, and the
//! bar API also accepts sliders.
//!
//! SKELETON: the class, state and public API are declared; the bodies are
//! `todo!()` until the widget is ported. Observer binding
//! (`lv_slider_bind_value`) needs the core observer subsystem and is not exposed.

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

use alloc::boxed::Box;

use ferrovgl_core::core::obj::{ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::misc::anim::AnimEnable;
use ferrovgl_core::{Area, Event, Obj, ObjClass, Point, Ui};

use crate::bar::{BAR_CLASS, BarMode, BarOrientation, BarState};

/// `lv_slider_mode_t`, with the upstream discriminants (equal to [`BarMode`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SliderMode {
    /// `LV_SLIDER_MODE_NORMAL` = `LV_BAR_MODE_NORMAL`
    #[default]
    Normal = BarMode::Normal as u8,
    /// `LV_SLIDER_MODE_SYMMETRICAL` = `LV_BAR_MODE_SYMMETRICAL`
    Symmetrical = BarMode::Symmetrical as u8,
    /// `LV_SLIDER_MODE_RANGE` = `LV_BAR_MODE_RANGE`
    Range = BarMode::Range as u8,
}

/// `lv_slider_orientation_t`, with the upstream discriminants (equal to
/// [`BarOrientation`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SliderOrientation {
    /// `LV_SLIDER_ORIENTATION_AUTO` = `LV_BAR_ORIENTATION_AUTO`
    #[default]
    Auto = BarOrientation::Auto as u8,
    /// `LV_SLIDER_ORIENTATION_HORIZONTAL` = `LV_BAR_ORIENTATION_HORIZONTAL`
    Horizontal = BarOrientation::Horizontal as u8,
    /// `LV_SLIDER_ORIENTATION_VERTICAL` = `LV_BAR_ORIENTATION_VERTICAL`
    Vertical = BarOrientation::Vertical as u8,
}

/// The bar value that `lv_slider_t::value_to_set` points to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SliderValueTarget {
    /// `&slider->bar.cur_value`
    CurValue,
    /// `&slider->bar.start_value`
    StartValue,
}

/// `lv_slider_t` without its `lv_obj_t` header. Zeroed like
/// `lv_malloc_zeroed`; the constructor sets the initial values.
#[derive(Clone, Debug, Default)]
pub(crate) struct SliderState {
    /// `bar`: the ancestor's state first.
    pub(crate) bar: BarState,
    /// `left_knob_area`
    pub(crate) left_knob_area: Area,
    /// `right_knob_area`
    pub(crate) right_knob_area: Area,
    /// `pressed_point`
    pub(crate) pressed_point: Point,
    /// `value_to_set`: which bar value to set (`NULL` = `None`).
    pub(crate) value_to_set: Option<SliderValueTarget>,
    /// `dragging : 1`: the slider is being dragged.
    pub(crate) dragging: bool,
    /// `left_knob_focus : 1`: with an encoder the left knob can be adjusted.
    pub(crate) left_knob_focus: bool,
}

ferrovgl_core::impl_widget_state!(SliderState, base = bar);

/// `lv_slider_class`. Width and height defaults (0) are inherited from the bar.
pub static SLIDER_CLASS: ObjClass = ObjClass {
    base: Some(&BAR_CLASS),
    name: "lv_slider",
    new_state: || Box::new(SliderState::default()),
    constructor: Some(constructor),
    destructor: None,
    event_cb: Some(slider_event),
    width_def: 0,
    height_def: 0,
    editable: ObjClassEditable::True,
    group_def: ObjClassGroupDef::True,
    theme_inheritable: false,
};

/// A copyable slider handle; widget state belongs to its [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slider(Obj);

impl Slider {
    /// `lv_slider_create()`. `None` creates a slider screen on the default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&SLIDER_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }

    /// The base object for positioning, flags and styles.
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the slider class.
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &SLIDER_CLASS)).then_some(Self(obj))
    }

    /// `lv_slider_is_dragged()`: whether a drag is in progress.
    pub fn is_dragged(self, ui: &Ui) -> bool {
        todo!()
    }

    /// `lv_slider_set_value()`: set the main knob's value, optionally animated.
    pub fn set_value(self, ui: &mut Ui, value: i32, anim: AnimEnable) {
        todo!()
    }

    /// `lv_slider_set_start_value()`: set the left knob's value, optionally animated.
    pub fn set_start_value(self, ui: &mut Ui, value: i32, anim: AnimEnable) {
        todo!()
    }

    /// `lv_slider_set_range()`
    pub fn set_range(self, ui: &mut Ui, min: i32, max: i32) {
        todo!()
    }

    /// `lv_slider_set_min_value()`
    pub fn set_min_value(self, ui: &mut Ui, min: i32) {
        todo!()
    }

    /// `lv_slider_set_max_value()`
    pub fn set_max_value(self, ui: &mut Ui, max: i32) {
        todo!()
    }

    /// `lv_slider_set_mode()`
    pub fn set_mode(self, ui: &mut Ui, mode: SliderMode) {
        todo!()
    }

    /// `lv_slider_set_orientation()`
    pub fn set_orientation(self, ui: &mut Ui, orientation: SliderOrientation) {
        todo!()
    }

    /// `lv_slider_get_value()`: the main knob's value.
    pub fn value(self, ui: &Ui) -> i32 {
        todo!()
    }

    /// `lv_slider_get_left_value()`: the left knob's value.
    pub fn left_value(self, ui: &Ui) -> i32 {
        todo!()
    }

    /// `lv_slider_get_min_value()`
    pub fn min_value(self, ui: &Ui) -> i32 {
        todo!()
    }

    /// `lv_slider_get_max_value()`
    pub fn max_value(self, ui: &Ui) -> i32 {
        todo!()
    }

    /// `lv_slider_get_mode()`
    pub fn mode(self, ui: &Ui) -> SliderMode {
        todo!()
    }

    /// `lv_slider_get_orientation()`
    pub fn orientation(self, ui: &Ui) -> SliderOrientation {
        todo!()
    }

    /// `lv_slider_is_symmetrical()`: whether the slider is in symmetrical mode.
    pub fn is_symmetrical(self, ui: &Ui) -> bool {
        todo!()
    }
}

impl From<Slider> for Obj {
    fn from(slider: Slider) -> Self {
        slider.obj()
    }
}

/// `lv_slider_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_slider_event()`
fn slider_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    todo!()
}
