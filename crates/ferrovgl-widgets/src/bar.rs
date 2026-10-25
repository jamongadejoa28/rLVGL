//! Ported from LVGL v9.6.0: `src/widgets/bar/lv_bar.c`.
//!
//! Also covers `include/lvgl/widgets/lv_bar.h` and
//! `src/widgets/bar/lv_bar_private.h`.
//!
//! The animation variable of upstream (`&bar->cur_value_anim` /
//! `&bar->start_value_anim`) is not the bar object, so
//! `lv_anim_delete(obj, NULL)` (object deletion, `lv_screen_load_anim()`, user
//! code) never touches it. The port uses an [`AnimTarget::Tag`] derived from
//! the bar's slot index and the value (see `anim_var()`), and the exec /
//! completed callbacks capture the bar handle instead of decoding `a->var`.
//! `lv_bar_destructor()` deletes both animations explicitly, as upstream.
//!
//! The observer binding (`lv_bar_bind_value()`, `bar_value_observer_cb()`;
//! `LV_USE_OBSERVER=y` in the parity config) is ported here as
//! `bar_bind_value()`; its public entry point `Bar::bind_value()` is declared
//! in this crate's `observer.rs`.
//!
//! Not ported: the `LV_USE_OBJ_PROPERTY` table
//! (`lv_bar_properties[]`, `lv_bar_set_value_helper()`,
//! `lv_bar_set_start_value_helper()`) is compiled out by the parity config.

use alloc::boxed::Box;
use alloc::rc::Rc;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::core::ui::DPI_DEF;
use ferrovgl_core::draw::grad::GradDir;
use ferrovgl_core::draw::{BlurDsc, DrawImageDsc, MaskRectDsc, RectDsc};
use ferrovgl_core::misc::anim::{Anim, AnimEnable, AnimExec, AnimTarget};
use ferrovgl_core::misc::style::ImageSrc;
use ferrovgl_core::misc::text::BaseDir;
use ferrovgl_core::{Area, ColorFormat, Event, EventCode, EventParam, LvResult, Obj, ObjClass, Part, Ui};

#[cfg(feature = "observer")]
use ferrovgl_core::core::observer::{ObserverId, SubjectId, SubjectType};

/// `LV_BAR_SIZE_MIN`: hor. pad and ver. pad cannot make the indicator smaller than this [px].
const BAR_SIZE_MIN: i32 = 4;
/// `LV_BAR_ANIM_STATE_START`
const BAR_ANIM_STATE_START: i32 = 0;
/// `LV_BAR_ANIM_STATE_END`
const BAR_ANIM_STATE_END: i32 = 256;
/// `LV_BAR_ANIM_STATE_INV`: no animation is in progress.
const BAR_ANIM_STATE_INV: i32 = -1;
/// `LV_BAR_ANIM_STATE_NORM`: log2(`LV_BAR_ANIM_STATE_END`).
#[allow(dead_code)] // Defined but unused upstream too.
const BAR_ANIM_STATE_NORM: i32 = 8;

/// Exec tag of `lv_bar_anim` animating `cur_value_anim`.
const ANIM_TAG_CUR_VALUE: u32 = 0x4c42_4143;
/// Exec tag of `lv_bar_anim` animating `start_value_anim`.
const ANIM_TAG_START_VALUE: u32 = 0x4c42_4153;
/// High bit of the [`AnimTarget::Tag`] that stands for `&bar->*_value_anim`,
/// keeping it away from small application tags.
const ANIM_VAR_BASE: u32 = 0x8000_0000;

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

impl BarAnim {
    /// `LV_BAR_IS_ANIMATING(anim_struct)`
    const fn is_animating(&self) -> bool {
        self.anim_state != BAR_ANIM_STATE_INV
    }

    /// `LV_BAR_GET_ANIM_VALUE(orig_value, anim_struct)`
    const fn get_anim_value(&self, orig_value: i32) -> i32 {
        if self.is_animating() {
            self.anim_end
        } else {
            orig_value
        }
    }
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

/// Which value `lv_bar_set_value_with_anim()`'s `value_ptr` / `anim_info` point to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BarValue {
    /// `&bar->cur_value`, `&bar->cur_value_anim`
    Cur,
    /// `&bar->start_value`, `&bar->start_value_anim`
    Start,
}

impl BarState {
    fn value_mut(&mut self, which: BarValue) -> &mut i32 {
        match which {
            BarValue::Cur => &mut self.cur_value,
            BarValue::Start => &mut self.start_value,
        }
    }

    fn anim_mut(&mut self, which: BarValue) -> &mut BarAnim {
        match which {
            BarValue::Cur => &mut self.cur_value_anim,
            BarValue::Start => &mut self.start_value_anim,
        }
    }
}

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
        bar_set_value(ui, self.0, value, anim);
    }

    /// `lv_bar_set_start_value()`: set a new start value, optionally animated.
    pub fn set_start_value(self, ui: &mut Ui, start_value: i32, anim: AnimEnable) {
        let obj = self.0;
        let Some(bar) = state(ui, obj) else {
            return; /*LV_CHECK_OBJ*/
        };

        if bar.mode != BarMode::Range {
            return;
        }

        let mut start_value = start_value.clamp_c(bar.min_value, bar.max_value);
        /*Can't be greater than the right value*/
        start_value = if start_value > bar.cur_value {
            bar.cur_value
        } else {
            start_value
        };

        if bar.start_value == start_value {
            return;
        }

        set_value_with_anim(ui, obj, start_value, BarValue::Start, anim);
    }

    /// `lv_bar_set_range()`: set the minimum and maximum values. If `min` is
    /// greater than `max`, the drawing direction is reversed.
    pub fn set_range(self, ui: &mut Ui, min: i32, max: i32) {
        bar_set_range(ui, self.0, min, max);
    }

    /// `lv_bar_set_min_value()`
    pub fn set_min_value(self, ui: &mut Ui, min: i32) {
        if state(ui, self.0).is_none() {
            return; /*LV_CHECK_OBJ*/
        }
        let max = self.max_value(ui);
        self.set_range(ui, min, max);
    }

    /// `lv_bar_set_max_value()`
    pub fn set_max_value(self, ui: &mut Ui, max: i32) {
        if state(ui, self.0).is_none() {
            return; /*LV_CHECK_OBJ*/
        }
        let min = self.min_value(ui);
        self.set_range(ui, min, max);
    }

    /// `lv_bar_set_mode()`
    pub fn set_mode(self, ui: &mut Ui, mode: BarMode) {
        let Some(bar) = state_mut(ui, self.0) else {
            return; /*LV_CHECK_OBJ*/
        };

        bar.mode = mode;
        if bar.mode != BarMode::Range {
            bar.start_value = bar.min_value;
        }

        ui.obj_invalidate(self.0);
    }

    /// `lv_bar_set_orientation()`
    pub fn set_orientation(self, ui: &mut Ui, orientation: BarOrientation) {
        let Some(bar) = state_mut(ui, self.0) else {
            return; /*LV_CHECK_OBJ*/
        };

        bar.orientation = orientation;
        ui.obj_invalidate(self.0);
    }

    /// `lv_bar_get_value()`: the target value of a running animation.
    pub fn value(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |bar| bar.cur_value_anim.get_anim_value(bar.cur_value))
    }

    /// `lv_bar_get_start_value()`
    pub fn start_value(self, ui: &Ui) -> i32 {
        let Some(bar) = state(ui, self.0) else {
            return 0; /*LV_CHECK_OBJ*/
        };

        if bar.mode != BarMode::Range {
            return bar.min_value;
        }

        bar.start_value_anim.get_anim_value(bar.start_value)
    }

    /// `lv_bar_get_min_value()`
    pub fn min_value(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |bar| {
            if bar.val_reversed {
                bar.max_value
            } else {
                bar.min_value
            }
        })
    }

    /// `lv_bar_get_max_value()`
    pub fn max_value(self, ui: &Ui) -> i32 {
        state(ui, self.0).map_or(0, |bar| {
            if bar.val_reversed {
                bar.min_value
            } else {
                bar.max_value
            }
        })
    }

    /// `lv_bar_get_mode()`
    pub fn mode(self, ui: &Ui) -> BarMode {
        state(ui, self.0).map_or(BarMode::Normal, |bar| bar.mode)
    }

    /// `lv_bar_get_orientation()`
    pub fn orientation(self, ui: &Ui) -> BarOrientation {
        state(ui, self.0).map_or(BarOrientation::Auto, |bar| bar.orientation)
    }

    /// `lv_bar_is_symmetrical()`: whether the bar is in symmetrical mode.
    pub fn is_symmetrical(self, ui: &Ui) -> bool {
        state(ui, self.0).is_some_and(is_symmetrical)
    }

    /// `lv_bar_t::indic_area`: the indicator area computed by the last
    /// `draw_indic()` of `lv_bar.c` (upstream tests read the field directly).
    pub fn indic_area(self, ui: &Ui) -> Area {
        state(ui, self.0).map_or(Area::new(0, 0, 0, 0), |bar| bar.indic_area)
    }
}

impl From<Bar> for Obj {
    fn from(bar: Bar) -> Self {
        bar.obj()
    }
}

/// `LV_CLAMP(min, val, max)`: `LV_MAX(min, LV_MIN(val, max))`, which unlike
/// `i32::clamp` does not panic when `min > max` (then `min` wins).
trait ClampC {
    fn clamp_c(self, min: i32, max: i32) -> i32;
}

impl ClampC for i32 {
    fn clamp_c(self, min: i32, max: i32) -> i32 {
        let v = if self < max { self } else { max };
        if min > v { min } else { v }
    }
}

fn state(ui: &Ui, obj: Obj) -> Option<&BarState> {
    ui.obj(obj)?.widget.get::<BarState>()
}

fn state_mut(ui: &mut Ui, obj: Obj) -> Option<&mut BarState> {
    ui.obj_mut(obj)?.widget.get_mut::<BarState>()
}

/// The body of `lv_bar_is_symmetrical()`.
fn is_symmetrical(bar: &BarState) -> bool {
    bar.mode == BarMode::Symmetrical
        && bar.min_value < 0
        && bar.max_value > 0
        && bar.start_value == bar.min_value
}

/// `lv_bar_set_value()`
fn bar_set_value(ui: &mut Ui, obj: Obj, value: i32, anim: AnimEnable) {
    let Some(bar) = state(ui, obj) else {
        return; /*LV_CHECK_OBJ*/
    };

    if bar.cur_value == value {
        return;
    }

    let mut value = value.clamp_c(bar.min_value, bar.max_value);
    /*Can't be smaller than the left value*/
    value = if value < bar.start_value {
        bar.start_value
    } else {
        value
    };

    if bar.cur_value == value {
        return;
    }

    set_value_with_anim(ui, obj, value, BarValue::Cur, anim);
}

/// `lv_bar_set_range()`
fn bar_set_range(ui: &mut Ui, obj: Obj, min: i32, max: i32) {
    let Some(bar) = state_mut(ui, obj) else {
        return; /*LV_CHECK_OBJ*/
    };

    bar.val_reversed = min > max;

    let real_min = if bar.val_reversed { max } else { min };
    let real_max = if bar.val_reversed { min } else { max };
    if bar.min_value == real_min && bar.max_value == real_max {
        return;
    }

    bar.max_value = real_max;
    bar.min_value = real_min;

    /*lv_bar_get_mode(obj)*/
    if bar.mode != BarMode::Range {
        bar.start_value = real_min;
    }

    if bar.cur_value > real_max {
        bar.cur_value = real_max;
        let v = bar.cur_value;
        bar_set_value(ui, obj, v, false);
    }
    if let Some(bar) = state_mut(ui, obj)
        && bar.cur_value < real_min
    {
        bar.cur_value = real_min;
        let v = bar.cur_value;
        bar_set_value(ui, obj, v, false);
    }

    ui.obj_invalidate(obj);
}

/// `lv_bar_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    let Some(bar) = state_mut(ui, obj) else { return };
    bar.min_value = 0;
    bar.max_value = 100;
    bar.start_value = 0;
    bar.cur_value = 0;
    bar.indic_area.x1 = 0;
    bar.indic_area.x2 = 0;
    bar.indic_area.y1 = 0;
    bar.indic_area.y2 = 0;
    bar.mode = BarMode::Normal;
    bar.orientation = BarOrientation::Auto;
    bar.val_reversed = false;

    init_anim(obj, &mut bar.cur_value_anim);
    init_anim(obj, &mut bar.start_value_anim);

    ui.obj_set_checkable(obj, false);
    ui.obj_set_scrollable(obj, false);
    bar_set_value(ui, obj, 0, false);
}

/// `lv_bar_destructor()`
fn destructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    ui.anim_delete(anim_var(obj, BarValue::Cur), None);
    ui.anim_delete(anim_var(obj, BarValue::Start), None);
}

/// An end of `lv_bar_t::indic_area` that `draw_indic()` addresses through
/// `int32_t * axis1, * axis2`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
    X1,
    X2,
    Y1,
    Y2,
}

fn edge_get(area: &Area, edge: Edge) -> i32 {
    match edge {
        Edge::X1 => area.x1,
        Edge::X2 => area.x2,
        Edge::Y1 => area.y1,
        Edge::Y2 => area.y2,
    }
}

fn edge_set(area: &mut Area, edge: Edge, value: i32) {
    match edge {
        Edge::X1 => area.x1 = value,
        Edge::X2 => area.x2 = value,
        Edge::Y1 => area.y1 = value,
        Edge::Y2 => area.y2 = value,
    }
}

/// `(int32_t)(((int64_t)anim_length * (value - bar->min_value)) / range)`
fn value_to_x(anim_length: i32, value: i32, min_value: i32, range: i32) -> i32 {
    ((i64::from(anim_length) * i64::from(value.wrapping_sub(min_value))) / i64::from(range)) as i32
}

/// `draw_indic()`
fn draw_indic(ui: &mut Ui, obj: Obj, event: &Event<'_>) {
    let Some(layer) = event.layer() else { return };
    let Some(bar) = state(ui, obj).cloned() else {
        return;
    };

    let mut bar_coords = ui.obj_get_coords(obj);
    let obj_coords = bar_coords;

    let transf_w = ui.obj_get_style_transform_width(obj, Part::MAIN);
    let transf_h = ui.obj_get_style_transform_height(obj, Part::MAIN);
    bar_coords.increase(transf_w, transf_h);
    let barw = bar_coords.width();
    let barh = bar_coords.height();
    let mut range = bar.max_value.wrapping_sub(bar.min_value);

    /*Prevent division by 0*/
    if range == 0 {
        range = 1;
    }

    let hor = match bar.orientation {
        BarOrientation::Horizontal => true,
        BarOrientation::Vertical => false,
        BarOrientation::Auto => barw >= barh,
    };

    let sym = is_symmetrical(&bar);

    /*Calculate the indicator area*/
    let bg_left = ui.obj_get_style_pad_left(obj, Part::MAIN);
    let bg_right = ui.obj_get_style_pad_right(obj, Part::MAIN);
    let bg_top = ui.obj_get_style_pad_top(obj, Part::MAIN);
    let bg_bottom = ui.obj_get_style_pad_bottom(obj, Part::MAIN);

    /*Respect padding and minimum width/height too*/
    let mut ia = bar_coords;
    ia.x1 = ia.x1.wrapping_add(bg_left);
    ia.x2 = ia.x2.wrapping_sub(bg_right);
    ia.y1 = ia.y1.wrapping_add(bg_top);
    ia.y2 = ia.y2.wrapping_sub(bg_bottom);

    if hor && ia.height() < BAR_SIZE_MIN {
        ia.y1 = obj_coords
            .y1
            .wrapping_add(barh / 2)
            .wrapping_sub(BAR_SIZE_MIN / 2);
        ia.y2 = ia.y1.wrapping_add(BAR_SIZE_MIN);
    } else if !hor && ia.width() < BAR_SIZE_MIN {
        ia.x1 = obj_coords
            .x1
            .wrapping_add(barw / 2)
            .wrapping_sub(BAR_SIZE_MIN / 2);
        ia.x2 = ia.x1.wrapping_add(BAR_SIZE_MIN);
    }
    let indic_max_w = ia.width();
    let indic_max_h = ia.height();

    /*Calculate the indicator length*/
    let anim_length = if hor { indic_max_w } else { indic_max_h };

    let (mut axis1, mut axis2) = if hor {
        (Edge::X1, Edge::X2)
    } else {
        (Edge::Y1, Edge::Y2)
    };
    let indic_length_calc = |area: &Area| if hor { area.width() } else { area.height() };

    let mut anim_start_value_x;
    if bar.start_value_anim.is_animating() {
        let anim_start_value_start_x =
            value_to_x(anim_length, bar.start_value_anim.anim_start, bar.min_value, range);
        let anim_start_value_end_x =
            value_to_x(anim_length, bar.start_value_anim.anim_end, bar.min_value, range);

        anim_start_value_x = anim_start_value_end_x
            .wrapping_sub(anim_start_value_start_x)
            .wrapping_mul(bar.start_value_anim.anim_state)
            / BAR_ANIM_STATE_END;

        anim_start_value_x = anim_start_value_x.wrapping_add(anim_start_value_start_x);
    } else {
        anim_start_value_x = value_to_x(anim_length, bar.start_value, bar.min_value, range);
    }

    let mut anim_cur_value_x = if bar.cur_value_anim.is_animating() {
        let anim_cur_value_start_x =
            value_to_x(anim_length, bar.cur_value_anim.anim_start, bar.min_value, range);
        let anim_cur_value_end_x = value_to_x(anim_length, bar.cur_value_anim.anim_end, bar.min_value, range);

        anim_cur_value_start_x.wrapping_add(
            anim_cur_value_end_x
                .wrapping_sub(anim_cur_value_start_x)
                .wrapping_mul(bar.cur_value_anim.anim_state)
                / BAR_ANIM_STATE_END,
        )
    } else {
        value_to_x(anim_length, bar.cur_value, bar.min_value, range)
    };

    /*The drawing direction of the bar can be reversed only when one of the two conditions(value inversion
     *or horizontal direction base dir is LV_BASE_DIR_RTL) is met.*/
    let base_dir = ui.obj_get_style_base_dir(obj, Part::MAIN);
    let hor_need_reversed = hor && base_dir == BaseDir::Rtl;
    let reversed = bar.val_reversed ^ hor_need_reversed;

    /*An area with width 0 is {x1 = 0 x2 = -1} so subtracting 1 from `anim_cur_value_x` causes...
     *    anim_start_value_x = 0   anim_cur_value_x = 0   to be {x1 = 0 x2 = -1  } which is width 0
     *    anim_start_value_x = 0   anim_cur_value_x = 300 to be {x1 = 0 x2 =  299} which is width 300*/
    anim_cur_value_x = anim_cur_value_x.wrapping_sub(1);

    if reversed {
        /*Swap axes*/
        core::mem::swap(&mut axis1, &mut axis2);
        anim_cur_value_x = anim_cur_value_x.wrapping_neg();
        anim_start_value_x = anim_start_value_x.wrapping_neg();
    }

    /*Set the indicator length*/
    if hor {
        let a1 = edge_get(&ia, axis1);
        edge_set(&mut ia, axis2, a1.wrapping_add(anim_cur_value_x));
        edge_set(&mut ia, axis1, a1.wrapping_add(anim_start_value_x));
    } else {
        let a2 = edge_get(&ia, axis2);
        edge_set(&mut ia, axis1, a2.wrapping_sub(anim_cur_value_x));
        edge_set(&mut ia, axis2, a2.wrapping_sub(anim_start_value_x));
    }

    if sym {
        let shift = bar.min_value.wrapping_neg().wrapping_mul(anim_length) / range;

        if hor {
            let left = if reversed { axis2 } else { axis1 };
            let right = if reversed { axis1 } else { axis2 };
            let zero = if reversed {
                edge_get(&ia, axis1).wrapping_sub(shift).wrapping_add(1)
            } else {
                edge_get(&ia, axis1).wrapping_add(shift)
            };

            if edge_get(&ia, axis2) > zero {
                let v = edge_get(&ia, axis2);
                edge_set(&mut ia, right, v);
                edge_set(&mut ia, left, zero);
            } else {
                let v = edge_get(&ia, axis2);
                edge_set(&mut ia, left, v);
                edge_set(&mut ia, right, zero);
            }
        } else {
            let top = if reversed { axis2 } else { axis1 };
            let bottom = if reversed { axis1 } else { axis2 };
            let zero = if reversed {
                edge_get(&ia, axis2).wrapping_add(shift)
            } else {
                edge_get(&ia, axis2).wrapping_sub(shift).wrapping_add(1)
            };

            if edge_get(&ia, axis1) > zero {
                let v = edge_get(&ia, axis1);
                edge_set(&mut ia, bottom, v);
                edge_set(&mut ia, top, zero);
            } else {
                let v = edge_get(&ia, axis1);
                edge_set(&mut ia, top, v);
                edge_set(&mut ia, bottom, zero);
            }
        }
    }

    if let Some(state) = state_mut(ui, obj) {
        state.indic_area = ia;
    }

    /*Do not draw a zero length indicator but at least call the draw task event*/
    if !sym && indic_length_calc(&ia) <= 1 {
        ui.obj_send_event(obj, EventCode::DRAW_TASK_ADDED, EventParam::None);
        return;
    }

    let indic_area = ia;

    let mut draw_rect_dsc = RectDsc::new();
    draw_rect_dsc.base.layer = Some(layer);
    ui.obj_init_draw_rect_dsc(obj, Part::INDICATOR, &mut draw_rect_dsc);

    let mut bg_radius = ui.obj_get_style_radius(obj, Part::MAIN);
    let mut short_side = barw.min(barh);
    if bg_radius > short_side >> 1 {
        bg_radius = short_side >> 1;
    }

    let backdrop_blur = ui.obj_get_style_blur_backdrop(obj, Part::INDICATOR);
    let mut draw_blur_dsc = BlurDsc::new();
    draw_blur_dsc.corner_radius = draw_rect_dsc.radius;
    ui.obj_init_draw_blur_dsc(obj, Part::INDICATOR, &mut draw_blur_dsc);
    if backdrop_blur {
        ui.draw_blur(layer, &draw_blur_dsc, &indic_area);
    }

    let mut indic_radius = draw_rect_dsc.radius;
    short_side = ia.width().min(ia.height());
    if indic_radius > short_side >> 1 {
        indic_radius = short_side >> 1;
    }

    /*Cases:
     * Simple:
     *   - indicator area is the same or smaller then the bg
     *   - indicator has the same or larger radius than the bg
     *   - what to do? just draw the indicator
     * Radius issue:
     *   - indicator area is the same or smaller then bg
     *   - indicator has smaller radius than the bg and the indicator overflows on the corners
     *   - what to do? draw the indicator on a layer and clip to bg radius
     * Larger indicator:
     *   - indicator area is the larger then the bg
     *   - radius doesn't matter
     *   - shadow doesn't matter
     *   - what to do? just draw the indicator
     * Shadow:
     *   - indicator area is the same or smaller then the bg
     *   - indicator has the same or larger radius than the bg (shadow needs to be drawn on strange clipped shape)
     *   - what to do? don't draw the shadow if the indicator is too small has strange shape
     * Gradient:
     *   - the indicator has a gradient
     *   - what to do? draw it on a bg sized layer clip the indicator are from the gradient
     */

    let mut mask_needed = false;
    if (hor && draw_rect_dsc.bg_grad.dir == GradDir::Hor)
        || (!hor && draw_rect_dsc.bg_grad.dir == GradDir::Ver)
    {
        mask_needed = true;
    }

    if !draw_rect_dsc.bg_image_src.is_none() {
        mask_needed = true;
    }

    /*The indicator is fully drawn if it's larger than the bg.
     *The three `radius_issue = false` branches of the C if/else-if chain,
     *evaluated in the same order.*/
    let radius_issue = !((bg_left < 0 || bg_right < 0 || bg_top < 0 || bg_bottom < 0)
        || indic_radius >= bg_radius
        || indic_area.is_in(&bar_coords, bg_radius));

    if radius_issue || mask_needed {
        if !radius_issue {
            /*Draw only the shadow*/
            let mut draw_rect_tmp_dsc = draw_rect_dsc.clone();
            draw_rect_tmp_dsc.border_opa = 0;
            draw_rect_tmp_dsc.outline_opa = 0;
            draw_rect_tmp_dsc.bg_opa = 0;
            draw_rect_tmp_dsc.bg_image_opa = 0;
            ui.draw_rect(layer, &draw_rect_tmp_dsc, &indic_area);
        } else {
            draw_rect_dsc.border_opa = 0;
            draw_rect_dsc.outline_opa = 0;
        }
        draw_rect_dsc.shadow_opa = 0;

        /*If clipped for any reason cannot the border, outline, and shadow
         *as they would be clipped and looked ugly*/
        let mut draw_tmp_dsc = draw_rect_dsc.clone();
        draw_tmp_dsc.border_opa = 0;
        draw_tmp_dsc.outline_opa = 0;
        draw_tmp_dsc.shadow_opa = 0;
        let mut indic_draw_area = indic_area;
        if mask_needed {
            if hor {
                indic_draw_area.x1 = bar_coords.x1.wrapping_add(bg_left);
                indic_draw_area.x2 = bar_coords.x2.wrapping_sub(bg_right);
            } else {
                indic_draw_area.y1 = bar_coords.y1.wrapping_add(bg_top);
                indic_draw_area.y2 = bar_coords.y2.wrapping_sub(bg_bottom);
            }
            draw_tmp_dsc.radius = 0;
        }

        let layer_indic = ui.draw_layer_create(Some(layer), ColorFormat::Argb8888, &indic_draw_area);

        ui.draw_rect(layer_indic, &draw_tmp_dsc, &indic_draw_area);

        let mut mask_dsc = MaskRectDsc::new();
        if radius_issue {
            mask_dsc.area = bar_coords;
            mask_dsc.radius = bg_radius;
            ui.draw_mask_rect(layer_indic, &mask_dsc);
        }

        if mask_needed {
            mask_dsc.area = indic_area;
            mask_dsc.radius = indic_radius;
            ui.draw_mask_rect(layer_indic, &mask_dsc);
        }

        let mut layer_draw_dsc = DrawImageDsc::new();
        layer_draw_dsc.src = ImageSrc::Layer(layer_indic);
        ui.draw_layer(layer, &layer_draw_dsc, &indic_draw_area);

        /*Add the border, outline, and shadow only to the indicator area.
         *They might have disabled if there is a radius_issue*/
        let mut draw_tmp_dsc = draw_rect_dsc.clone();
        draw_tmp_dsc.bg_opa = 0;
        draw_tmp_dsc.bg_image_opa = 0;
        ui.draw_rect(layer, &draw_tmp_dsc, &indic_area);
    } else {
        ui.draw_rect(layer, &draw_rect_dsc, &indic_area);
    }

    if !backdrop_blur {
        ui.draw_blur(layer, &draw_blur_dsc, &indic_area);
    }
}

/// `lv_bar_event()`
fn bar_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    /*Call the ancestor's event handler*/
    if ui.obj_event_base(&BAR_CLASS, event) != LvResult::Ok {
        return;
    }

    let code = event.code();
    let Some(obj) = event.current_target_obj() else {
        return;
    };

    if code == EventCode::REFR_EXT_DRAW_SIZE {
        let indic_size = ui.obj_calculate_ext_draw_size(obj, Part::INDICATOR);

        /*Calculate the indicator area*/
        let bg_left = ui.obj_get_style_pad_left(obj, Part::MAIN);
        let bg_right = ui.obj_get_style_pad_right(obj, Part::MAIN);
        let bg_top = ui.obj_get_style_pad_top(obj, Part::MAIN);
        let bg_bottom = ui.obj_get_style_pad_bottom(obj, Part::MAIN);

        let pad = bg_left.min(bg_right).min(bg_top.min(bg_bottom));

        /*Bg size is handled by lv_obj*/
        if let EventParam::ExtDrawSize(s) = event.param() {
            **s = (**s).max(indic_size);
            if pad < 0 {
                **s -= pad;
            }
        }
    } else if code == EventCode::PRESSED || code == EventCode::RELEASED {
        if let Some(bar) = state(ui, obj) {
            let area = bar.indic_area;
            ui.obj_invalidate_area(obj, &area);
        }
    } else if code == EventCode::DRAW_MAIN {
        draw_indic(ui, obj, event);
    }
}

/// `&bar->cur_value_anim` / `&bar->start_value_anim` as an animation
/// variable: unique among live bars (slot indices of live objects are unique
/// and a bar's animations die in `lv_bar_destructor()`), and never equal to
/// `AnimTarget::Obj(obj)`, so `lv_anim_delete(obj, NULL)` does not match it.
fn anim_var(obj: Obj, which: BarValue) -> AnimTarget {
    let which_bit = match which {
        BarValue::Cur => 0,
        BarValue::Start => 1,
    };
    AnimTarget::Tag(ANIM_VAR_BASE | (obj.0.0.index() << 1) | which_bit)
}

/// `lv_bar_anim` as an exec callback; the tag (C function identity) selects
/// `which`, the captured `obj` replaces `((lv_bar_anim_t *)a->var)->bar`.
fn anim_exec(obj: Obj, which: BarValue) -> AnimExec {
    AnimExec::Custom {
        tag: match which {
            BarValue::Cur => ANIM_TAG_CUR_VALUE,
            BarValue::Start => ANIM_TAG_START_VALUE,
        },
        f: Rc::new(move |ui, _var, value| bar_anim(ui, obj, which, value)),
    }
}

/// `lv_bar_anim()`
fn bar_anim(ui: &mut Ui, obj: Obj, which: BarValue, value: i32) {
    let Some(bar) = state_mut(ui, obj) else { return };
    let bar_anim = bar.anim_mut(which);
    bar_anim.anim_state = value;
    if let Some(target) = bar_anim.bar {
        ui.obj_invalidate(target);
    }
}

/// `lv_bar_anim_completed()`
fn bar_anim_completed(ui: &mut Ui, obj: Obj, which: BarValue) {
    let Some(bar) = state_mut(ui, obj) else { return };

    let var = bar.anim_mut(which);
    var.anim_state = BAR_ANIM_STATE_INV;
    let anim_end = var.anim_end;
    let target = var.bar;
    *bar.value_mut(which) = anim_end;
    if let Some(target) = target {
        ui.obj_invalidate(target);
    }
}

/// `lv_bar_set_value_with_anim()`
fn set_value_with_anim(ui: &mut Ui, obj: Obj, new_value: i32, which: BarValue, en: AnimEnable) {
    let var = anim_var(obj, which);
    if !en {
        ui.anim_delete(var, None);
        let Some(bar) = state_mut(ui, obj) else { return };
        bar.anim_mut(which).anim_state = BAR_ANIM_STATE_INV;
        *bar.value_mut(which) = new_value;
        ui.obj_invalidate(obj);

        /*Stop the previous animation if it exists*/
        ui.anim_delete(var, None);
        /*Reset animation state*/
        if let Some(bar) = state_mut(ui, obj) {
            init_anim(obj, bar.anim_mut(which));
        }
    } else {
        let Some(bar) = state_mut(ui, obj) else { return };
        let value = *bar.value_mut(which);
        let anim_info = bar.anim_mut(which);
        /*No animation in progress -> simply set the values*/
        if anim_info.anim_state == BAR_ANIM_STATE_INV {
            anim_info.anim_start = value;
            anim_info.anim_end = new_value;
        }
        /*Animation in progress. Start from the animation end value*/
        else {
            anim_info.anim_start = anim_info.anim_end;
            anim_info.anim_end = new_value;
        }
        *bar.value_mut(which) = new_value;
        /*Stop the previous animation if it exists*/
        ui.anim_delete(var, None);

        let mut a = Anim::new();
        a.set_var(var);
        a.set_exec_cb(anim_exec(obj, which));
        a.set_values(BAR_ANIM_STATE_START, BAR_ANIM_STATE_END);
        a.set_completed_cb(Some(Rc::new(move |ui, _a| bar_anim_completed(ui, obj, which))));
        a.set_duration(ui.obj_get_style_anim_duration(obj, Part::MAIN));
        ui.anim_start(&a);
    }
}

/// `lv_bar_init_anim()`
fn init_anim(obj: Obj, bar_anim: &mut BarAnim) {
    bar_anim.bar = Some(obj);
    bar_anim.anim_start = 0;
    bar_anim.anim_end = 0;
    bar_anim.anim_state = BAR_ANIM_STATE_INV;
}

/// `lv_bar_bind_value()`: bind an integer or float subject to the bar's
/// value. Called by `Bar::bind_value()`.
#[cfg(feature = "observer")]
pub(crate) fn bar_bind_value(ui: &mut Ui, obj: Obj, subject: SubjectId) -> Option<ObserverId> {
    if !ui.obj_is_valid(obj) {
        return None; /*LV_CHECK_OBJ*/
    }
    let Some(type_) = ui.observer.subjects.get(subject.0).map(|s| s.type_) else {
        ferrovgl_core::misc::check_arg_failed("lv_bar_bind_value: subject != NULL");
        return None;
    };
    if !(type_ == SubjectType::Int || type_ == SubjectType::Float) {
        ferrovgl_core::misc::check_arg_failed("Incompatible subject type");
        return None;
    }

    ui.subject_add_observer_obj(subject, Rc::new(bar_value_observer_cb), Some(obj), None)
}

/// `bar_value_observer_cb()`
#[cfg(feature = "observer")]
fn bar_value_observer_cb(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let Some(obj) = ui.observer_get_target_obj(observer) else {
        return;
    };
    let Some(s) = ui.observer.subjects.get(subject.0) else {
        return;
    };
    let (type_, num, float_v) = (s.type_, s.value.num(), s.value.float_v_as_i32());

    /*If the bar is not rendered yet show the new state immediately*/
    let anim_on: AnimEnable = ui.obj(obj).is_some_and(|o| o.rendered);
    if type_ == SubjectType::Int {
        bar_set_value(ui, obj, num, anim_on);
    } else {
        /*LV_USE_FLOAT*/
        bar_set_value(ui, obj, float_v, anim_on);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::UiConfig;

    fn setup() -> (Ui, Bar) {
        let mut ui = Ui::new(UiConfig::default());
        let display = ui.display_create(320, 240);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let bar = Bar::create(&mut ui, screen);
        (ui, bar)
    }

    #[test]
    fn class_defaults_match_c() {
        let (ui, bar) = setup();
        assert!(ui.obj_check_type(bar.obj(), &BAR_CLASS));
        assert!(!ui.obj_is_checkable(bar.obj()));
        assert!(!ui.obj_is_scrollable(bar.obj()));
        assert_eq!(bar.min_value(&ui), 0);
        assert_eq!(bar.max_value(&ui), 100);
        assert_eq!(bar.value(&ui), 0);
        assert_eq!(bar.mode(&ui), BarMode::Normal);
        assert_eq!(bar.orientation(&ui), BarOrientation::Auto);
        assert_eq!(Bar::from_obj(&ui, bar.obj()), Some(bar));
    }

    #[test]
    fn reversed_range_and_clamping() {
        let (mut ui, bar) = setup();
        bar.set_value(&mut ui, 50, false);
        bar.set_range(&mut ui, 100, 0);
        assert_eq!(bar.min_value(&ui), 100);
        assert_eq!(bar.max_value(&ui), 0);
        assert_eq!(bar.value(&ui), 50);
        bar.set_range(&mut ui, 0, 20);
        assert_eq!(bar.value(&ui), 20);
        bar.set_range(&mut ui, 30, 40);
        assert_eq!(bar.value(&ui), 30);
        bar.set_mode(&mut ui, BarMode::Symmetrical);
        bar.set_range(&mut ui, -10, 10);
        assert!(bar.is_symmetrical(&ui));
    }

    #[test]
    fn animated_value_reports_target_and_completes() {
        let (mut ui, bar) = setup();
        ui.obj_set_style_anim_duration(bar.obj(), 100, ferrovgl_core::StyleSelector::DEFAULT);
        bar.set_value(&mut ui, 80, true);
        assert_eq!(bar.value(&ui), 80);
        let animating = state(&ui, bar.obj()).unwrap().cur_value_anim;
        assert_eq!(animating.anim_start, 0);
        assert_eq!(animating.anim_end, 80);
        assert_eq!(animating.anim_state, 0);
        for _ in 0..200 {
            ui.tick_inc(1);
            ui.timer_handler();
        }
        let done = state(&ui, bar.obj()).unwrap();
        assert_eq!(done.cur_value_anim.anim_state, BAR_ANIM_STATE_INV);
        assert_eq!(done.cur_value, 80);
        // Deleting during an animation removes it (`lv_bar_destructor`).
        bar.set_value(&mut ui, 10, true);
        ui.obj_delete(bar.obj());
        assert_eq!(ui.anim_count_running(), 0);
    }

    /// `lv_screen_load_anim()` runs `lv_anim_delete(scr, NULL)`, which in C
    /// cannot match `&bar->cur_value_anim`: the value animation of a bar
    /// screen keeps running and completes.
    #[test]
    fn screen_load_keeps_value_animation() {
        let mut ui = Ui::new(UiConfig::default());
        ui.display_create(320, 240);
        let bar = Bar::create(&mut ui, None);
        ui.obj_set_style_anim_duration(bar.obj(), 500, ferrovgl_core::StyleSelector::DEFAULT);
        bar.set_value(&mut ui, 80, true);
        ui.screen_load_anim(bar.obj(), ferrovgl_core::ScreenLoadAnim::None, 0, 0, false);
        assert_eq!(state(&ui, bar.obj()).unwrap().cur_value_anim.anim_state, 0);
        for _ in 0..600 {
            ui.tick_inc(1);
            ui.timer_handler();
        }
        let done = state(&ui, bar.obj()).unwrap();
        assert_eq!(done.cur_value_anim.anim_state, BAR_ANIM_STATE_INV);
        assert_eq!(done.cur_value, 80);
    }

    /// Two bars' animations of the same value are independent variables.
    #[test]
    fn value_animations_are_per_bar_and_per_value() {
        let (mut ui, bar) = setup();
        let screen = ui.obj_get_parent(bar.obj());
        let other = Bar::create(&mut ui, screen);
        for b in [bar, other] {
            ui.obj_set_style_anim_duration(b.obj(), 100, ferrovgl_core::StyleSelector::DEFAULT);
            b.set_mode(&mut ui, BarMode::Range);
            b.set_value(&mut ui, 80, true);
            b.set_start_value(&mut ui, 20, true);
        }
        assert_eq!(ui.anim_count_running(), 4);
        bar.set_value(&mut ui, 50, false);
        assert_eq!(ui.anim_count_running(), 3);
        ui.anim_delete(AnimTarget::Obj(other.obj()), None);
        assert_eq!(ui.anim_count_running(), 3);
        ui.obj_delete(other.obj());
        assert_eq!(ui.anim_count_running(), 1);
        assert!(state(&ui, bar.obj()).unwrap().start_value_anim.is_animating());
    }

    /// `LV_CLAMP(min, val, max)` is `LV_MAX(min, LV_MIN(val, max))`.
    #[test]
    fn clamp_c_matches_lv_clamp() {
        assert_eq!(5.clamp_c(0, 10), 5);
        assert_eq!((-3).clamp_c(0, 10), 0);
        assert_eq!(42.clamp_c(0, 10), 10);
        // min > max: LV_CLAMP(10, 5, 0) = LV_MAX(10, LV_MIN(5, 0)) = 10.
        assert_eq!(5.clamp_c(10, 0), 10);
        assert_eq!(20.clamp_c(10, 0), 10);
    }
}
