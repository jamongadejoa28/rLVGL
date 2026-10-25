//! Ported from LVGL v9.6.0: `src/widgets/slider/lv_slider.c`.
//!
//! Also covers `include/lvgl/widgets/lv_slider.h` and
//! `src/widgets/slider/lv_slider_private.h`. The slider derives from the bar:
//! its state embeds the bar state like `lv_slider_t` embeds `lv_bar_t`, and the
//! bar API also accepts sliders.
//!
//! `lv_slider_t::value_to_set` is a pointer into the bar state upstream; the
//! port stores which value it points to ([`SliderValueTarget`]).
//!
//! Not ported: observer binding (`lv_slider_bind_value()`,
//! `slider_value_changed_event_cb()`, `slider_value_observer_cb()`;
//! `LV_USE_OBSERVER=y` in the parity config) waits for core observer/subject
//! support. The `LV_USE_OBJ_PROPERTY` table (`lv_slider_properties[]`) is
//! compiled out by the parity config.

use alloc::boxed::Box;

use ferrovgl_core::core::obj::{ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::core::obj_pos::ObjPointTransformFlag;
use ferrovgl_core::draw::RectDsc;
use ferrovgl_core::group::{KEY_DOWN, KEY_LEFT, KEY_RIGHT, KEY_UP};
use ferrovgl_core::misc::anim::AnimEnable;
use ferrovgl_core::misc::text::BaseDir;
use ferrovgl_core::{
    Area, Event, EventCode, EventParam, IndevType, LvResult, Obj, ObjClass, Part, Point, Ui,
};

use crate::bar::{BAR_CLASS, Bar, BarMode, BarOrientation, BarState};

/// `LV_ANIM_ON`
const ANIM_ON: AnimEnable = true;

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

impl SliderMode {
    /// The `(lv_bar_mode_t)mode` cast of `lv_slider_set_mode()`.
    const fn to_bar(self) -> BarMode {
        match self {
            Self::Normal => BarMode::Normal,
            Self::Symmetrical => BarMode::Symmetrical,
            Self::Range => BarMode::Range,
        }
    }
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

impl SliderOrientation {
    /// The `(lv_bar_orientation_t)orientation` cast of `lv_slider_set_orientation()`.
    const fn to_bar(self) -> BarOrientation {
        match self {
            Self::Auto => BarOrientation::Auto,
            Self::Horizontal => BarOrientation::Horizontal,
            Self::Vertical => BarOrientation::Vertical,
        }
    }
}

/// The bar value that `lv_slider_t::value_to_set` points to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SliderValueTarget {
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

impl SliderState {
    /// `*slider->value_to_set` (the caller checked that it is not `NULL`).
    fn value_to_set_mut(&mut self, target: SliderValueTarget) -> &mut i32 {
        match target {
            SliderValueTarget::CurValue => &mut self.bar.cur_value,
            SliderValueTarget::StartValue => &mut self.bar.start_value,
        }
    }
}

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
        state(ui, self.0).is_some_and(|slider| slider.dragging)
    }

    /// `lv_slider_set_value()`: set the main knob's value, optionally animated.
    pub fn set_value(self, ui: &mut Ui, value: i32, anim: AnimEnable) {
        if let Some(bar) = as_bar(ui, self.0) {
            bar.set_value(ui, value, anim);
        }
    }

    /// `lv_slider_set_start_value()`: set the left knob's value, optionally animated.
    pub fn set_start_value(self, ui: &mut Ui, value: i32, anim: AnimEnable) {
        if let Some(bar) = as_bar(ui, self.0) {
            bar.set_start_value(ui, value, anim);
        }
    }

    /// `lv_slider_set_range()`
    pub fn set_range(self, ui: &mut Ui, min: i32, max: i32) {
        if let Some(bar) = as_bar(ui, self.0) {
            bar.set_range(ui, min, max);
        }
    }

    /// `lv_slider_set_min_value()`
    pub fn set_min_value(self, ui: &mut Ui, min: i32) {
        if let Some(bar) = as_bar(ui, self.0) {
            bar.set_min_value(ui, min);
        }
    }

    /// `lv_slider_set_max_value()`
    pub fn set_max_value(self, ui: &mut Ui, max: i32) {
        if let Some(bar) = as_bar(ui, self.0) {
            bar.set_max_value(ui, max);
        }
    }

    /// `lv_slider_set_mode()`
    pub fn set_mode(self, ui: &mut Ui, mode: SliderMode) {
        if let Some(bar) = as_bar(ui, self.0) {
            bar.set_mode(ui, mode.to_bar());
        }
    }

    /// `lv_slider_set_orientation()`
    pub fn set_orientation(self, ui: &mut Ui, orientation: SliderOrientation) {
        if let Some(bar) = as_bar(ui, self.0) {
            bar.set_orientation(ui, orientation.to_bar());
        }
    }

    /// `lv_slider_get_value()`: the main knob's value.
    pub fn value(self, ui: &Ui) -> i32 {
        as_bar(ui, self.0).map_or(0, |bar| bar.value(ui))
    }

    /// `lv_slider_get_left_value()`: the left knob's value.
    pub fn left_value(self, ui: &Ui) -> i32 {
        as_bar(ui, self.0).map_or(0, |bar| bar.start_value(ui))
    }

    /// `lv_slider_get_min_value()`
    pub fn min_value(self, ui: &Ui) -> i32 {
        as_bar(ui, self.0).map_or(0, |bar| bar.min_value(ui))
    }

    /// `lv_slider_get_max_value()`
    pub fn max_value(self, ui: &Ui) -> i32 {
        as_bar(ui, self.0).map_or(0, |bar| bar.max_value(ui))
    }

    /// `lv_slider_get_mode()`
    pub fn mode(self, ui: &Ui) -> SliderMode {
        slider_get_mode(ui, self.0)
    }

    /// `lv_slider_get_orientation()`
    pub fn orientation(self, ui: &Ui) -> SliderOrientation {
        let Some(bar) = as_bar(ui, self.0) else {
            return SliderOrientation::Auto; /*LV_CHECK_OBJ*/
        };

        match bar.orientation(ui) {
            BarOrientation::Horizontal => SliderOrientation::Horizontal,
            BarOrientation::Vertical => SliderOrientation::Vertical,
            BarOrientation::Auto => SliderOrientation::Auto,
        }
    }

    /// `lv_slider_is_symmetrical()`: whether the slider is in symmetrical mode.
    pub fn is_symmetrical(self, ui: &Ui) -> bool {
        as_bar(ui, self.0).is_some_and(|bar| bar.is_symmetrical(ui))
    }

    /// `lv_slider_t::left_knob_area`: the left knob's area computed by the
    /// last `draw_knob()` of `lv_slider.c` (upstream tests read the field).
    pub fn left_knob_area(self, ui: &Ui) -> Area {
        state(ui, self.0).map_or(Area::new(0, 0, 0, 0), |slider| slider.left_knob_area)
    }

    /// `lv_slider_t::right_knob_area`: the main knob's area computed by the
    /// last `draw_knob()` of `lv_slider.c` (upstream tests read the field).
    pub fn right_knob_area(self, ui: &Ui) -> Area {
        state(ui, self.0).map_or(Area::new(0, 0, 0, 0), |slider| slider.right_knob_area)
    }

    /// `lv_slider_t::value_to_set`: the value a drag adjusts (`NULL` = `None`;
    /// upstream tests read the field).
    pub fn value_to_set(self, ui: &Ui) -> Option<SliderValueTarget> {
        state(ui, self.0).and_then(|slider| slider.value_to_set)
    }

    /// `lv_slider_t::left_knob_focus`: whether an encoder or keypad adjusts
    /// the left knob (upstream tests read the field).
    pub fn left_knob_focus(self, ui: &Ui) -> bool {
        state(ui, self.0).is_some_and(|slider| slider.left_knob_focus)
    }

    /// Assign `lv_slider_t::left_knob_focus` (upstream tests write the field).
    pub fn set_left_knob_focus(self, ui: &mut Ui, focus: bool) {
        if let Some(slider) = state_mut(ui, self.0) {
            slider.left_knob_focus = focus;
        }
    }
}

impl From<Slider> for Obj {
    fn from(slider: Slider) -> Self {
        slider.obj()
    }
}

fn state(ui: &Ui, obj: Obj) -> Option<&SliderState> {
    ui.obj(obj)?.widget.get::<SliderState>()
}

fn state_mut(ui: &mut Ui, obj: Obj) -> Option<&mut SliderState> {
    ui.obj_mut(obj)?.widget.get_mut::<SliderState>()
}

/// `LV_CHECK_OBJ(obj, MY_CLASS, ...)` followed by the `lv_bar_*()` call on the
/// same object.
fn as_bar(ui: &Ui, obj: Obj) -> Option<Bar> {
    state(ui, obj)?;
    Bar::from_obj(ui, obj)
}

/// `lv_slider_get_mode()`
fn slider_get_mode(ui: &Ui, obj: Obj) -> SliderMode {
    let Some(bar) = as_bar(ui, obj) else {
        return SliderMode::Normal; /*LV_CHECK_OBJ*/
    };

    match bar.mode(ui) {
        BarMode::Symmetrical => SliderMode::Symmetrical,
        BarMode::Range => SliderMode::Range,
        BarMode::Normal => SliderMode::Normal,
    }
}

/// `LV_ABS(x)`
const fn lv_abs(x: i32) -> i32 {
    if x > 0 { x } else { x.wrapping_neg() }
}

/// `LV_CLAMP(min, val, max)`: `LV_MAX(min, LV_MIN(val, max))`, which unlike
/// `i32::clamp` does not panic when `min > max` (then `min` wins).
const fn lv_clamp(min: i32, val: i32, max: i32) -> i32 {
    let v = if val < max { val } else { max };
    if min > v { min } else { v }
}

/// `LV_SLIDER_KNOB_COORD(is_reversed, area)`
const fn knob_coord(is_reversed: bool, area: &Area) -> i32 {
    if is_reversed { area.x1 } else { area.x2 }
}

/// `LV_SLIDER_KNOB_COORD_VERTICAL(is_reversed, area)`
const fn knob_coord_vertical(is_reversed: bool, area: &Area) -> i32 {
    if is_reversed { area.y2 } else { area.y1 }
}

/// `LV_BASE_DIR_RTL == lv_obj_get_style_base_dir_internal(obj, LV_PART_MAIN)`
fn is_rtl(ui: &Ui, obj: Obj) -> bool {
    ui.obj_get_style_base_dir(obj, Part::MAIN) == BaseDir::Rtl
}

/// `lv_slider_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    let Some(slider) = state_mut(ui, obj) else { return };

    /*Initialize the allocated 'slider'*/
    slider.value_to_set = None;
    slider.dragging = false;
    slider.left_knob_focus = false;

    ui.obj_set_scroll_chain_hor(obj, false);
    ui.obj_set_scrollable(obj, false);
    ui.obj_set_scroll_on_focus(obj, true);
    let ext = ui.dpx(8);
    ui.obj_set_ext_click_area(obj, ext);
}

/// `lv_slider_event()`
fn slider_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    /*Call the ancestor's event handler*/
    if ui.obj_event_base(&SLIDER_CLASS, event) != LvResult::Ok {
        return;
    }

    let code = event.code();
    let Some(obj) = event.current_target_obj() else {
        return;
    };
    if state(ui, obj).is_none() {
        return;
    }
    let ty = slider_get_mode(ui, obj);

    /*Advanced hit testing: react only on dragging the knob(s)*/
    if code == EventCode::HIT_TEST {
        let ext_click_area = ui
            .obj(obj)
            .and_then(|d| d.spec.as_ref())
            .map_or(0, |spec| spec.ext_click_pad);
        let Some(slider) = state(ui, obj) else { return };
        let right_knob_area = slider.right_knob_area;
        let left_knob_area = slider.left_knob_area;
        let Some(info) = event.hit_test_info() else {
            return;
        };

        /*Ordinary slider: was the knob area hit?*/
        let mut a = right_knob_area;
        a.increase(ext_click_area, ext_click_area);
        info.res = a.is_point_on(info.point, 0);

        /*There's still a chance that there is a hit if there is another knob*/
        if !info.res && ty == SliderMode::Range {
            a = left_knob_area;
            a.increase(ext_click_area, ext_click_area);
            info.res = a.is_point_on(info.point, 0);
        }
    } else if code == EventCode::PRESSED {
        /*Save the pressed coordinates*/
        let indev = event.indev();
        let mut pressed_point = Point::new(0, 0);
        if let Some(indev) = indev {
            pressed_point = ui.indev_get_point(indev);
        }
        ui.obj_transform_point(obj, &mut pressed_point, ObjPointTransformFlag::InverseRecursive);
        if let Some(slider) = state_mut(ui, obj) {
            slider.pressed_point = pressed_point;
        }
    } else if code == EventCode::PRESSING {
        update_knob_pos(ui, obj, true);
    } else if code == EventCode::RELEASED || code == EventCode::PRESS_LOST {
        update_knob_pos(ui, obj, false);
        /*`update_knob_pos()` may have run user callbacks*/
        let Some(slider) = state_mut(ui, obj) else { return };
        slider.dragging = false;
        slider.value_to_set = None;

        ui.obj_invalidate(obj);

        /*Leave edit mode if released. (No need to wait for LONG_PRESS)*/
        let g = ui.obj_get_group(obj);
        let editing = g.is_some_and(|g| ui.group_get_editing(g));
        let indev = event.indev();
        let indev_type = indev.map_or(IndevType::None, |indev| ui.indev_get_type(indev));
        if indev_type == IndevType::Encoder {
            if editing && let Some(g) = g {
                if slider_get_mode(ui, obj) == SliderMode::Range {
                    let Some(slider) = state_mut(ui, obj) else { return };
                    if !slider.left_knob_focus {
                        slider.left_knob_focus = true;
                    } else {
                        slider.left_knob_focus = false;
                        ui.group_set_editing(g, false);
                    }
                } else {
                    ui.group_set_editing(g, false);
                }
            }
        } else if indev_type == IndevType::Pointer {
            if is_slider_horizontal(ui, obj) {
                ui.obj_set_scroll_chain_ver(obj, true);
            } else {
                ui.obj_set_scroll_chain_hor(obj, true);
            }
        }
    } else if code == EventCode::FOCUSED {
        let indev = ui.indev_active();
        let indev_type = indev.map_or(IndevType::None, |indev| ui.indev_get_type(indev));
        if (indev_type == IndevType::Encoder || indev_type == IndevType::Keypad)
            && let Some(slider) = state_mut(ui, obj)
        {
            slider.left_knob_focus = false;
        }
    } else if code == EventCode::SIZE_CHANGED {
        if is_slider_horizontal(ui, obj) {
            ui.obj_set_scroll_chain_ver(obj, true);
            ui.obj_set_scroll_chain_hor(obj, false);
        } else {
            ui.obj_set_scroll_chain_hor(obj, true);
            ui.obj_set_scroll_chain_ver(obj, false);
        }
        ui.obj_refresh_ext_draw_size(obj);
    } else if code == EventCode::REFR_EXT_DRAW_SIZE {
        let knob_left = ui.obj_get_style_pad_left(obj, Part::KNOB);
        let knob_right = ui.obj_get_style_pad_right(obj, Part::KNOB);
        let knob_top = ui.obj_get_style_pad_top(obj, Part::KNOB);
        let knob_bottom = ui.obj_get_style_pad_bottom(obj, Part::KNOB);

        /*The smaller size is the knob diameter*/
        let trans_w = ui.obj_get_style_transform_width(obj, Part::KNOB);
        let trans_h = ui.obj_get_style_transform_height(obj, Part::KNOB);
        let mut knob_size = ui
            .obj_get_width(obj)
            .wrapping_add(trans_w.wrapping_mul(2))
            .min(ui.obj_get_height(obj).wrapping_add(trans_h.wrapping_mul(2)))
            >> 1;
        knob_size = knob_size.wrapping_add(knob_left.max(knob_right).max(knob_bottom.max(knob_top)));
        knob_size = knob_size.wrapping_add(2); /*For rounding error*/
        knob_size = knob_size.wrapping_add(ui.obj_calculate_ext_draw_size(obj, Part::KNOB));

        /*Indic. size is handled by bar*/
        if let EventParam::ExtDrawSize(s) = event.param() {
            **s = (**s).max(knob_size);
        }
    } else if code == EventCode::KEY {
        let c = event.key();
        let slider = Slider(obj);
        let left_knob_focus = slider.left_knob_focus(ui);

        if c == KEY_RIGHT || c == KEY_UP {
            if !left_knob_focus {
                let v = slider.value(ui).wrapping_add(1);
                slider.set_value(ui, v, ANIM_ON);
            } else {
                let v = slider.left_value(ui).wrapping_add(1);
                slider.set_start_value(ui, v, ANIM_ON);
            }
        } else if c == KEY_LEFT || c == KEY_DOWN {
            if !left_knob_focus {
                let v = slider.value(ui).wrapping_sub(1);
                slider.set_value(ui, v, ANIM_ON);
            } else {
                let v = slider.left_value(ui).wrapping_sub(1);
                slider.set_start_value(ui, v, ANIM_ON);
            }
        } else {
            return;
        }

        /*The result only decides an early return at the end of the handler*/
        let _ = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
    } else if code == EventCode::ROTARY {
        let r = event.rotary_diff();
        let slider = Slider(obj);

        if !slider.left_knob_focus(ui) {
            let v = slider.value(ui).wrapping_add(r);
            slider.set_value(ui, v, ANIM_ON);
        } else {
            let v = slider.left_value(ui).wrapping_add(1);
            slider.set_start_value(ui, v, ANIM_ON);
        }

        /*The result only decides an early return at the end of the handler*/
        let _ = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
    } else if code == EventCode::DRAW_MAIN {
        draw_knob(ui, obj, event);
    }
}

/// `draw_knob()`
fn draw_knob(ui: &mut Ui, obj: Obj, event: &Event<'_>) {
    let Some(layer) = event.layer() else { return };
    let Some(slider) = state(ui, obj) else { return };
    let bar = slider.bar.clone();

    let is_rtl = is_rtl(ui, obj);
    let is_horizontal = is_slider_horizontal(ui, obj);
    let is_reversed = bar.val_reversed ^ (is_rtl && is_horizontal);

    let mut knob_area = Area::default();
    let knob_size;
    let is_symmetrical = Slider(obj).is_symmetrical(ui);

    if is_horizontal {
        knob_size = ui.obj_get_height(obj);
        if is_symmetrical && bar.cur_value < 0 {
            knob_area.x1 = knob_coord(!is_reversed, &bar.indic_area);
        } else {
            knob_area.x1 = knob_coord(is_reversed, &bar.indic_area);
        }
    } else {
        knob_size = ui.obj_get_width(obj);
        if is_symmetrical && bar.cur_value < 0 {
            knob_area.y1 = knob_coord_vertical(!is_reversed, &bar.indic_area);
        } else {
            knob_area.y1 = knob_coord_vertical(is_reversed, &bar.indic_area);
        }
    }
    let mut knob_rect_dsc = RectDsc::new();
    knob_rect_dsc.base.layer = Some(layer);
    ui.obj_init_draw_rect_dsc(obj, Part::KNOB, &mut knob_rect_dsc);
    /* Update knob area with knob style */
    position_knob(ui, obj, &mut knob_area, knob_size, is_horizontal);
    /* Update right knob area with calculated knob area */
    let Some(slider) = state_mut(ui, obj) else { return };
    slider.right_knob_area = knob_area;
    let right_knob_area = slider.right_knob_area;

    if slider_get_mode(ui, obj) != SliderMode::Range {
        ui.draw_rect(layer, &knob_rect_dsc, &right_knob_area);
    } else {
        /*Save the draw part_draw_dsc. because it can be modified in the event*/
        let knob_rect_dsc_tmp = knob_rect_dsc.clone();
        /* Draw the right knob */
        ui.draw_rect(layer, &knob_rect_dsc, &right_knob_area);

        /*Calculate the second knob area*/
        if is_horizontal {
            /*use !is_reversed to get the other knob*/
            knob_area.x1 = knob_coord(!is_reversed, &bar.indic_area);
        } else {
            knob_area.y1 = knob_coord_vertical(!is_reversed, &bar.indic_area);
        }
        position_knob(ui, obj, &mut knob_area, knob_size, is_horizontal);
        let Some(slider) = state_mut(ui, obj) else { return };
        slider.left_knob_area = knob_area;
        let left_knob_area = slider.left_knob_area;

        knob_rect_dsc = knob_rect_dsc_tmp;

        ui.draw_rect(layer, &knob_rect_dsc, &left_knob_area);
    }
}

/// `position_knob()`
fn position_knob(ui: &Ui, obj: Obj, knob_area: &mut Area, knob_size: i32, hor: bool) {
    let coords = ui.obj_get_coords(obj);
    if hor {
        knob_area.x1 = knob_area.x1.wrapping_sub(knob_size >> 1);
        knob_area.x2 = knob_area.x1.wrapping_add(knob_size).wrapping_sub(1);
        knob_area.y1 = coords.y1;
        knob_area.y2 = coords.y2;
    } else {
        knob_area.y1 = knob_area.y1.wrapping_sub(knob_size >> 1);
        knob_area.y2 = knob_area.y1.wrapping_add(knob_size).wrapping_sub(1);
        knob_area.x1 = coords.x1;
        knob_area.x2 = coords.x2;
    }

    let knob_left = ui.obj_get_style_pad_left(obj, Part::KNOB);
    let knob_right = ui.obj_get_style_pad_right(obj, Part::KNOB);
    let knob_top = ui.obj_get_style_pad_top(obj, Part::KNOB);
    let knob_bottom = ui.obj_get_style_pad_bottom(obj, Part::KNOB);

    let transf_w = ui.obj_get_style_transform_width(obj, Part::KNOB);
    let transf_h = ui.obj_get_style_transform_height(obj, Part::KNOB);

    /*Apply the paddings on the knob area*/
    knob_area.x1 = knob_area.x1.wrapping_sub(knob_left.wrapping_add(transf_w));
    knob_area.x2 = knob_area.x2.wrapping_add(knob_right.wrapping_add(transf_w));
    knob_area.y1 = knob_area.y1.wrapping_sub(knob_top.wrapping_add(transf_h));
    knob_area.y2 = knob_area.y2.wrapping_add(knob_bottom.wrapping_add(transf_h));
}

/// `is_slider_horizontal()`
fn is_slider_horizontal(ui: &Ui, obj: Obj) -> bool {
    let orientation = state(ui, obj).map_or(BarOrientation::Auto, |slider| slider.bar.orientation);
    if orientation == BarOrientation::Auto {
        return ui.obj_get_width(obj) >= ui.obj_get_height(obj);
    }
    orientation == BarOrientation::Horizontal
}

/// The "use whichever one is closer" tail of `drag_start()`.
fn closer_knob(slider: &mut SliderState, dist_left: i32, dist_right: i32) {
    if dist_right < dist_left {
        slider.value_to_set = Some(SliderValueTarget::CurValue);
        slider.left_knob_focus = false;
    } else {
        slider.value_to_set = Some(SliderValueTarget::StartValue);
        slider.left_knob_focus = true;
    }
}

/// `drag_start()`
fn drag_start(ui: &mut Ui, obj: Obj) {
    let mode = slider_get_mode(ui, obj);
    let mut p = Point::new(0, 0);
    let Some(slider) = state_mut(ui, obj) else { return };
    slider.dragging = true;
    if mode == SliderMode::Normal || mode == SliderMode::Symmetrical {
        slider.value_to_set = Some(SliderValueTarget::CurValue);
    } else if mode == SliderMode::Range {
        if let Some(indev) = ui.indev_active() {
            p = ui.indev_get_point(indev);
        }
        ui.obj_transform_point(obj, &mut p, ObjPointTransformFlag::InverseRecursive);
        let is_rtl = is_rtl(ui, obj);
        let is_horizontal = is_slider_horizontal(ui, obj);
        let Some(slider) = state_mut(ui, obj) else { return };
        let is_reversed = slider.bar.val_reversed ^ (is_rtl && is_horizontal);
        let left = slider.left_knob_area;
        let right = slider.right_knob_area;
        if is_horizontal {
            if (!is_reversed && p.x > right.x2) || (is_reversed && p.x < right.x1) {
                slider.value_to_set = Some(SliderValueTarget::CurValue);
            } else if (!is_reversed && p.x < left.x1) || (is_reversed && p.x > left.x2) {
                slider.value_to_set = Some(SliderValueTarget::StartValue);
            } else {
                /*Calculate the distance from each knob*/
                let dist_left = lv_abs(
                    left.x1
                        .wrapping_add(left.x2.wrapping_sub(left.x1) / 2)
                        .wrapping_sub(p.x),
                );
                let dist_right = lv_abs(
                    right
                        .x1
                        .wrapping_add(right.x2.wrapping_sub(right.x1) / 2)
                        .wrapping_sub(p.x),
                );
                /*Use whichever one is closer*/
                closer_knob(slider, dist_left, dist_right);
            }
        } else if (!is_reversed && p.y < right.y1) || (is_reversed && p.y > right.y2) {
            slider.value_to_set = Some(SliderValueTarget::CurValue);
        } else if (!is_reversed && p.y > left.y2) || (is_reversed && p.y < left.y1) {
            slider.value_to_set = Some(SliderValueTarget::StartValue);
        } else {
            /*Calculate the distance from each knob*/
            let dist_left = lv_abs(
                left.y1
                    .wrapping_add(left.y2.wrapping_sub(left.y1) / 2)
                    .wrapping_sub(p.y),
            );
            let dist_right = lv_abs(
                right
                    .y1
                    .wrapping_add(right.y2.wrapping_sub(right.y1) / 2)
                    .wrapping_sub(p.y),
            );

            /*Use whichever one is closer*/
            closer_knob(slider, dist_left, dist_right);
        }
    }
}

/// `(int32_t)(((int64_t)new_value * range + indic_len / 2) / indic_len)`
fn scale_value(new_value: i32, range: i32, indic_len: i32) -> i32 {
    ((i64::from(new_value) * i64::from(range) + i64::from(indic_len / 2)) / i64::from(indic_len)) as i32
}

/// `update_knob_pos()`
fn update_knob_pos(ui: &mut Ui, obj: Obj, check_drag: bool) {
    let Some(indev) = ui.indev_active() else { return };
    if ui.indev_get_type(indev) != IndevType::Pointer {
        return;
    }
    let Some(dragging) = state(ui, obj).map(|slider| slider.dragging) else {
        return;
    };
    if ui.indev_get_scroll_obj(indev).is_some() && !dragging {
        return;
    }

    let mut p = ui.indev_get_point(indev);
    ui.obj_transform_point(obj, &mut p, ObjPointTransformFlag::InverseRecursive);

    let is_hor = is_slider_horizontal(ui, obj);

    if check_drag && !dragging {
        let Some(pressed_point) = state(ui, obj).map(|slider| slider.pressed_point) else {
            return;
        };
        let ofs = if is_hor {
            p.x.wrapping_sub(pressed_point.x)
        } else {
            p.y.wrapping_sub(pressed_point.y)
        };

        /*Stop processing when offset is below scroll_limit*/
        let scroll_limit = ui.indev(indev).map_or(0, |d| i32::from(d.scroll_limit));
        if lv_abs(ofs) < scroll_limit {
            return;
        }
    }

    if state(ui, obj).is_some_and(|slider| slider.value_to_set.is_none()) {
        /*Ready to start drag*/
        drag_start(ui, obj);
    }

    let Some(bar) = state(ui, obj).map(|slider| slider.bar.clone()) else {
        return;
    };
    let mut new_value;
    let range = bar.max_value.wrapping_sub(bar.min_value);
    let is_rtl = is_rtl(ui, obj);
    let is_horizontal = is_slider_horizontal(ui, obj);
    let is_reversed = bar.val_reversed ^ (is_rtl && is_horizontal);
    let coords = ui.obj_get_coords(obj);

    if is_hor {
        let bg_left = ui.obj_get_style_pad_left(obj, Part::MAIN);
        let bg_right = ui.obj_get_style_pad_right(obj, Part::MAIN);
        let w = ui.obj_get_width(obj);
        let indic_w = w.wrapping_sub(bg_left).wrapping_sub(bg_right);

        if is_reversed {
            /*Make the point relative to the indicator*/
            new_value = coords.x2.wrapping_sub(bg_right).wrapping_sub(p.x);
        } else {
            /*Make the point relative to the indicator*/
            new_value = p.x.wrapping_sub(coords.x1.wrapping_add(bg_left));
        }
        if indic_w != 0 {
            new_value = scale_value(new_value, range, indic_w);
            new_value = new_value.wrapping_add(bar.min_value);
        }
    } else {
        let bg_top = ui.obj_get_style_pad_top(obj, Part::MAIN);
        let bg_bottom = ui.obj_get_style_pad_bottom(obj, Part::MAIN);
        let h = ui.obj_get_height(obj);
        let indic_h = h.wrapping_sub(bg_bottom).wrapping_sub(bg_top);

        if is_reversed {
            /*Make the point relative to the indicator*/
            new_value = p.y.wrapping_sub(coords.y1.wrapping_add(bg_top));
        } else {
            /*Make the point relative to the indicator*/
            new_value = p.y.wrapping_sub(coords.y2.wrapping_add(bg_bottom));
            new_value = new_value.wrapping_neg();
        }
        /*Upstream divides by `indic_h` unchecked (undefined for 0); the port
         *skips only the division instead of panicking.*/
        if indic_h != 0 {
            new_value = scale_value(new_value, range, indic_h);
        }
        new_value = new_value.wrapping_add(bar.min_value);
    }

    let Some(slider) = state_mut(ui, obj) else { return };
    let Some(target) = slider.value_to_set else {
        return;
    };
    let mut real_max_value = slider.bar.max_value;
    let mut real_min_value = slider.bar.min_value;
    /*Figure out the min. and max. for this mode*/
    if target == SliderValueTarget::StartValue {
        real_max_value = slider.bar.cur_value;
    } else {
        real_min_value = slider.bar.start_value;
    }

    new_value = lv_clamp(real_min_value, new_value, real_max_value);
    let value = slider.value_to_set_mut(target);
    if *value != new_value {
        *value = new_value;
        if is_hor {
            ui.obj_set_scroll_chain_ver(obj, false);
        } else {
            ui.obj_set_scroll_chain_hor(obj, false);
        }

        ui.obj_invalidate(obj);
        /*The result only decides an early return at the end of the function*/
        let _ = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::UiConfig;

    fn setup() -> (Ui, Slider) {
        let mut ui = Ui::new(UiConfig::default());
        let display = ui.display_create(320, 240);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let slider = Slider::create(&mut ui, screen);
        (ui, slider)
    }

    #[test]
    fn class_defaults_match_c() {
        let (ui, slider) = setup();
        let obj = slider.obj();
        assert!(ui.obj_check_type(obj, &SLIDER_CLASS));
        assert!(ui.obj_has_class(obj, &BAR_CLASS));
        assert!(!ui.obj_is_scrollable(obj));
        assert!(ui.obj_is_scroll_on_focus(obj));
        assert_eq!(slider.value(&ui), 0);
        assert_eq!(slider.min_value(&ui), 0);
        assert_eq!(slider.max_value(&ui), 100);
        assert_eq!(slider.mode(&ui), SliderMode::Normal);
        assert_eq!(slider.orientation(&ui), SliderOrientation::Auto);
        assert!(!slider.is_dragged(&ui));
        assert_eq!(slider.value_to_set(&ui), None);
        assert_eq!(Slider::from_obj(&ui, obj), Some(slider));
        assert_eq!(
            ui.obj(obj).unwrap().spec.as_ref().unwrap().ext_click_pad,
            ui.dpx(8)
        );
    }

    #[test]
    fn mode_and_orientation_round_trip() {
        let (mut ui, slider) = setup();
        for mode in [SliderMode::Range, SliderMode::Symmetrical, SliderMode::Normal] {
            slider.set_mode(&mut ui, mode);
            assert_eq!(slider.mode(&ui), mode);
        }
        for orientation in [
            SliderOrientation::Vertical,
            SliderOrientation::Horizontal,
            SliderOrientation::Auto,
        ] {
            slider.set_orientation(&mut ui, orientation);
            assert_eq!(slider.orientation(&ui), orientation);
        }
        slider.set_mode(&mut ui, SliderMode::Symmetrical);
        slider.set_range(&mut ui, -10, 10);
        assert!(slider.is_symmetrical(&ui));
        slider.set_min_value(&mut ui, 5);
        slider.set_max_value(&mut ui, 50);
        assert_eq!((slider.min_value(&ui), slider.max_value(&ui)), (5, 50));
    }

    #[test]
    fn non_slider_objects_are_rejected() {
        let (mut ui, slider) = setup();
        let parent = ui.obj_get_parent(slider.obj());
        let bar = Bar::create(&mut ui, parent);
        let fake = Slider(bar.obj());
        fake.set_value(&mut ui, 40, false);
        assert_eq!(bar.value(&ui), 0);
        assert_eq!(fake.value(&ui), 0);
        assert!(Slider::from_obj(&ui, bar.obj()).is_none());
    }

    #[test]
    fn helpers_match_c_macros() {
        assert_eq!(lv_abs(-3), 3);
        assert_eq!(lv_abs(i32::MIN), i32::MIN);
        assert_eq!(lv_clamp(10, 5, 0), 10);
        assert_eq!(scale_value(300, 20_000_000, 600), 10_000_000);
    }
}
