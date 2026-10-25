//! Ported from LVGL v9.6.0: `src/widgets/buttonmatrix/lv_buttonmatrix.c`.
//!
//! Also covers `include/lvgl/widgets/lv_buttonmatrix.h` and
//! `src/widgets/buttonmatrix/lv_buttonmatrix_private.h`. The map
//! (`const char * const map[]`, terminated by `""`) is a [`ButtonmatrixMap`]:
//! only the reference is kept, like the C pointer, and it is compared by
//! address. The end of the slice is read like the `NULL` entry that also stops
//! the C loops, so the trailing `""` may be omitted. Like `lv_strcmp()` and the
//! text drawing, every map entry ends at its first NUL byte.
//!
//! `LV_USE_OBJ_PROPERTY` is disabled in the parity configuration, so the
//! property table is not provided. Reads of `ctrl_bits`/`button_areas` that
//! would be out of bounds in C (undefined behaviour, e.g. a `ctrl_map` shorter
//! than the button count) read zero here instead of panicking.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::core::ui::DPI_DEF;
use ferrovgl_core::draw::RectDsc;
use ferrovgl_core::draw::label::LabelDsc;
use ferrovgl_core::misc::event::LvResult;
use ferrovgl_core::misc::style::{BorderSide, SharedRef, SharedStr};
use ferrovgl_core::misc::text::{self, BaseDir, TextAttributes, TextFlags};
use ferrovgl_core::{Area, Event, EventCode, EventParam, IndevType, Obj, ObjClass, Part, Point, State, Ui};

/// `LV_BUTTONMATRIX_BUTTON_NONE`: no button is selected.
pub const BUTTONMATRIX_BUTTON_NONE: u32 = 0xFFFF;

/// `BTN_EXTRA_CLICK_AREA_MAX` (private define in `lv_buttonmatrix.c`).
const BTN_EXTRA_CLICK_AREA_MAX: i32 = DPI_DEF / 10;
/// `LV_BUTTONMATRIX_WIDTH_MASK` (private define in `lv_buttonmatrix.c`).
const BUTTONMATRIX_WIDTH_MASK: u16 = 0x000F;

/// `lv_buttonmatrix_ctrl_t`: button control flags (disabled, hidden, ...).
/// The least significant 4 bits store the relative button width (1..15), so
/// this is a plain `u16` newtype with the upstream values rather than a set
/// of independent bits.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ButtonmatrixCtrl(pub u16);

impl ButtonmatrixCtrl {
    /// `LV_BUTTONMATRIX_CTRL_NONE`: default settings.
    pub const NONE: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0000);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_1`: width 1 relative to the other
    /// buttons in the same row.
    pub const WIDTH_1: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0001);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_2`
    pub const WIDTH_2: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0002);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_3`
    pub const WIDTH_3: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0003);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_4`
    pub const WIDTH_4: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0004);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_5`
    pub const WIDTH_5: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0005);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_6`
    pub const WIDTH_6: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0006);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_7`
    pub const WIDTH_7: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0007);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_8`
    pub const WIDTH_8: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0008);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_9`
    pub const WIDTH_9: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0009);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_10`
    pub const WIDTH_10: ButtonmatrixCtrl = ButtonmatrixCtrl(0x000A);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_11`
    pub const WIDTH_11: ButtonmatrixCtrl = ButtonmatrixCtrl(0x000B);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_12`
    pub const WIDTH_12: ButtonmatrixCtrl = ButtonmatrixCtrl(0x000C);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_13`
    pub const WIDTH_13: ButtonmatrixCtrl = ButtonmatrixCtrl(0x000D);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_14`
    pub const WIDTH_14: ButtonmatrixCtrl = ButtonmatrixCtrl(0x000E);
    /// `LV_BUTTONMATRIX_CTRL_WIDTH_15`
    pub const WIDTH_15: ButtonmatrixCtrl = ButtonmatrixCtrl(0x000F);
    /// `LV_BUTTONMATRIX_CTRL_HIDDEN`: hides the button; it keeps its
    /// space in the layout.
    pub const HIDDEN: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0010);
    /// `LV_BUTTONMATRIX_CTRL_NO_REPEAT`: no `LV_EVENT_LONG_PRESSED_REPEAT`
    /// while long-pressed.
    pub const NO_REPEAT: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0020);
    /// `LV_BUTTONMATRIX_CTRL_DISABLED`: like `LV_STATE_DISABLED`.
    pub const DISABLED: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0040);
    /// `LV_BUTTONMATRIX_CTRL_CHECKABLE`: toggle `LV_STATE_CHECKED` when
    /// clicked.
    pub const CHECKABLE: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0080);
    /// `LV_BUTTONMATRIX_CTRL_CHECKED`: the button uses the
    /// `LV_STATE_CHECKED` styles.
    pub const CHECKED: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0100);
    /// `LV_BUTTONMATRIX_CTRL_CLICK_TRIG`: send `LV_EVENT_VALUE_CHANGED`
    /// on CLICK instead of PRESS.
    pub const CLICK_TRIG: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0200);
    /// `LV_BUTTONMATRIX_CTRL_POPOVER`: show the text in a pop-over while
    /// pressed.
    pub const POPOVER: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0400);
    /// `LV_BUTTONMATRIX_CTRL_RECOLOR`: enable `#color` text recoloring.
    pub const RECOLOR: ButtonmatrixCtrl = ButtonmatrixCtrl(0x0800);
    /// `LV_BUTTONMATRIX_CTRL_RESERVED_1`
    pub const RESERVED_1: ButtonmatrixCtrl = ButtonmatrixCtrl(0x1000);
    /// `LV_BUTTONMATRIX_CTRL_RESERVED_2`
    pub const RESERVED_2: ButtonmatrixCtrl = ButtonmatrixCtrl(0x2000);
    /// `LV_BUTTONMATRIX_CTRL_CUSTOM_1`: free-to-use flag.
    pub const CUSTOM_1: ButtonmatrixCtrl = ButtonmatrixCtrl(0x4000);
    /// `LV_BUTTONMATRIX_CTRL_CUSTOM_2`: free-to-use flag.
    pub const CUSTOM_2: ButtonmatrixCtrl = ButtonmatrixCtrl(0x8000);

    /// The raw value.
    #[inline]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Whether every bit of `other` is set (`(self & other) == other`).
    #[inline]
    pub const fn contains(self, other: ButtonmatrixCtrl) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether any bit of `other` is set (`(self & other) != 0`).
    #[inline]
    pub const fn intersects(self, other: ButtonmatrixCtrl) -> bool {
        self.0 & other.0 != 0
    }
}

impl core::ops::BitOr for ButtonmatrixCtrl {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for ButtonmatrixCtrl {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl core::ops::BitAnd for ButtonmatrixCtrl {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl core::ops::BitAndAssign for ButtonmatrixCtrl {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl core::ops::Not for ButtonmatrixCtrl {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

/// `const char * const * map`: the button texts, `"\n"` breaks a row and
/// `""` ends the map. Only the reference is stored.
pub type ButtonmatrixMap = SharedRef<[SharedStr]>;

/// `lv_buttonmatrix_button_draw_cb_t`: `(btnm, btn_id, draw_area,
/// clip_area) -> bool`.
pub type ButtonmatrixButtonDrawCb = Rc<dyn Fn(&mut Ui, Obj, u32, &Area, &Area) -> bool>;

/// `lv_buttonmatrix_t` without its `lv_obj_t` header. Zeroed like
/// `lv_malloc_zeroed`; the constructor sets the initial values.
#[derive(Clone, Debug, Default)]
pub(crate) struct ButtonmatrixState {
    /// `map_p` (`NULL` = `None`): the current map.
    pub(crate) map_p: Option<ButtonmatrixMap>,
    /// `button_areas`: the areas of the buttons.
    pub(crate) button_areas: Vec<Area>,
    /// `ctrl_bits`: the control bytes of the buttons.
    pub(crate) ctrl_bits: Vec<ButtonmatrixCtrl>,
    /// `btn_cnt`: number of buttons in `map_p`.
    pub(crate) btn_cnt: u32,
    /// `row_cnt`: number of rows in `map_p`.
    pub(crate) row_cnt: u32,
    /// `btn_id_sel`: index of the active button or
    /// [`BUTTONMATRIX_BUTTON_NONE`].
    pub(crate) btn_id_sel: u32,
    /// `one_check : 1`: single button toggled at once.
    pub(crate) one_check: bool,
    /// `auto_free_map : 1`: free the map when the widget is deleted.
    pub(crate) auto_free_map: bool,
}

ferrovgl_core::impl_widget_state!(ButtonmatrixState);

/// `lv_buttonmatrix_class`.
pub static BUTTONMATRIX_CLASS: ObjClass = ObjClass {
    base: Some(&OBJ_CLASS),
    name: "lv_buttonmatrix",
    new_state: || Box::new(ButtonmatrixState::default()),
    constructor: Some(constructor),
    destructor: Some(destructor),
    event_cb: Some(buttonmatrix_event),
    width_def: DPI_DEF * 2,
    height_def: DPI_DEF,
    editable: ObjClassEditable::True,
    group_def: ObjClassGroupDef::True,
    theme_inheritable: false,
};

/// `lv_buttonmatrix_def_map` (`LV_WIDGETS_HAS_DEFAULT_VALUE`).
const BUTTONMATRIX_DEF_MAP: &[SharedStr] = &[
    SharedRef::Static("Btn1"),
    SharedRef::Static("Btn2"),
    SharedRef::Static("Btn3"),
    SharedRef::Static("\n"),
    SharedRef::Static("Btn4"),
    SharedRef::Static("Btn5"),
    SharedRef::Static(""),
];

/// A copyable button matrix handle; widget state belongs to its [`Ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Buttonmatrix(Obj);

impl Buttonmatrix {
    /// `lv_buttonmatrix_create()`. `None` creates a button matrix screen on
    /// the default display.
    pub fn create(ui: &mut Ui, parent: impl Into<Option<Obj>>) -> Self {
        let obj = ui.obj_class_create_obj(&BUTTONMATRIX_CLASS, parent.into());
        if ui.obj_is_valid(obj) {
            ui.obj_class_init_obj(obj);
        }
        Self(obj)
    }

    /// The base object for positioning, flags and styles.
    pub const fn obj(self) -> Obj {
        self.0
    }

    /// Check that an existing object derives from the button matrix class.
    pub fn from_obj(ui: &Ui, obj: Obj) -> Option<Self> {
        check_obj(ui, obj).then_some(Self(obj))
    }

    /// `lv_buttonmatrix_set_map()`: set a new map; buttons are created and
    /// deleted according to it. The map is referenced, not copied.
    pub fn set_map(self, ui: &mut Ui, map: ButtonmatrixMap) {
        set_map(ui, self.0, map);
    }

    /// `lv_buttonmatrix_set_ctrl_map()`: copy one control value per button
    /// (new lines excluded).
    pub fn set_ctrl_map(self, ui: &mut Ui, ctrl_map: &[ButtonmatrixCtrl]) {
        // LV_CHECK_OBJ(obj, MY_CLASS, return);
        let Some(btnm) = state_mut(ui, self.0) else {
            return;
        };
        // lv_memcpy(btnm->ctrl_bits, ctrl_map, sizeof(lv_buttonmatrix_ctrl_t) * btnm->btn_cnt);
        // A shorter `ctrl_map` would be read out of bounds in C; only the
        // available values are copied here.
        let n = (btnm.btn_cnt as usize)
            .min(ctrl_map.len())
            .min(btnm.ctrl_bits.len());
        btnm.ctrl_bits[..n].copy_from_slice(&ctrl_map[..n]);

        update_map(ui, self.0);
    }

    /// `lv_buttonmatrix_set_selected_button()`: 0 based index, not counting
    /// new lines.
    pub fn set_selected_button(self, ui: &mut Ui, btn_id: u32) {
        set_selected_button(ui, self.0, btn_id);
    }

    /// `lv_buttonmatrix_set_button_ctrl()`: set OR-ed attributes of a button.
    pub fn set_button_ctrl(self, ui: &mut Ui, btn_id: u32, ctrl: ButtonmatrixCtrl) {
        set_button_ctrl(ui, self.0, btn_id, ctrl);
    }

    /// `lv_buttonmatrix_clear_button_ctrl()`: clear OR-ed attributes of a
    /// button.
    pub fn clear_button_ctrl(self, ui: &mut Ui, btn_id: u32, ctrl: ButtonmatrixCtrl) {
        clear_button_ctrl(ui, self.0, btn_id, ctrl);
    }

    /// `lv_buttonmatrix_set_button_ctrl_all()`
    pub fn set_button_ctrl_all(self, ui: &mut Ui, ctrl: ButtonmatrixCtrl) {
        // LV_CHECK_OBJ(obj, MY_CLASS, return);
        let mut i = 0;
        // `btnm->btn_cnt` is read again in every iteration.
        while i < state(ui, self.0).map_or(0, |btnm| btnm.btn_cnt) {
            set_button_ctrl(ui, self.0, i, ctrl);
            i += 1;
        }
    }

    /// `lv_buttonmatrix_clear_button_ctrl_all()`
    pub fn clear_button_ctrl_all(self, ui: &mut Ui, ctrl: ButtonmatrixCtrl) {
        clear_button_ctrl_all(ui, self.0, ctrl);
    }

    /// `lv_buttonmatrix_set_button_width()`: relative width (1..15) of a
    /// button; regenerates the matrix.
    pub fn set_button_width(self, ui: &mut Ui, btn_id: u32, width: u32) {
        // LV_CHECK_OBJ(obj, MY_CLASS, return);
        let Some(btnm) = state_mut(ui, self.0) else {
            return;
        };
        if btn_id >= btnm.btn_cnt {
            return;
        }
        if let Some(bits) = btnm.ctrl_bits.get_mut(btn_id as usize) {
            bits.0 &= !BUTTONMATRIX_WIDTH_MASK;
            // `LV_BUTTONMATRIX_WIDTH_MASK & width` fits the 16-bit control.
            bits.0 |= (u32::from(BUTTONMATRIX_WIDTH_MASK) & width) as u16;
        }

        update_map(ui, self.0);
    }

    /// `lv_buttonmatrix_set_one_checked()`: only one button may be checked
    /// at a time.
    pub fn set_one_checked(self, ui: &mut Ui, en: bool) {
        // LV_CHECK_OBJ(obj, MY_CLASS, return);
        let Some(btnm) = state_mut(ui, self.0) else {
            return;
        };
        btnm.one_check = en;

        /*If more than one button is toggled only the first one should be*/
        make_one_button_checked(ui, self.0, 0);
    }

    /// `lv_buttonmatrix_get_map()`
    pub fn map(self, ui: &Ui) -> Option<ButtonmatrixMap> {
        state(ui, self.0).and_then(|btnm| btnm.map_p.clone())
    }

    /// `lv_buttonmatrix_get_selected_button()`: index of the last
    /// "activated" button or [`BUTTONMATRIX_BUTTON_NONE`].
    pub fn selected_button(self, ui: &Ui) -> u32 {
        state(ui, self.0).map_or(0, |btnm| btnm.btn_id_sel)
    }

    /// `lv_buttonmatrix_get_button_text()`: `None` is `NULL`.
    pub fn button_text(self, ui: &Ui, btn_id: u32) -> Option<&str> {
        // LV_CHECK_OBJ(obj, MY_CLASS, return 0);
        let btnm = state(ui, self.0)?;

        if btn_id == BUTTONMATRIX_BUTTON_NONE {
            return None;
        }

        if btn_id >= btnm.btn_cnt {
            return None;
        }
        let map: &[SharedStr] = btnm.map_p.as_deref()?;

        let mut txt_i: usize = 0;
        let mut btn_i: u32 = 0;

        /*Search the text of btnm->btn_pr the buttons text in the map
         *Skip "\n"-s*/
        while btn_i != btn_id {
            btn_i += 1;
            txt_i += 1;
            if map_entry(map, txt_i) == Some("\n") {
                txt_i += 1;
            }
        }

        if btn_i == btnm.btn_cnt {
            return None;
        }

        map_entry(map, txt_i)
    }

    /// `lv_buttonmatrix_has_button_ctrl()`: whether all of `ctrl` are set on
    /// a button.
    pub fn has_button_ctrl(self, ui: &Ui, btn_id: u32, ctrl: ButtonmatrixCtrl) -> bool {
        has_button_ctrl(ui, self.0, btn_id, ctrl)
    }

    /// `lv_buttonmatrix_get_one_checked()`
    pub fn one_checked(self, ui: &Ui) -> bool {
        state(ui, self.0).is_some_and(|btnm| btnm.one_check)
    }

    /// `lv_buttonmatrix_t::btn_cnt`: the number of buttons (upstream tests
    /// read the field).
    pub fn btn_cnt(self, ui: &Ui) -> u32 {
        state(ui, self.0).map_or(0, |btnm| btnm.btn_cnt)
    }

    /// `lv_buttonmatrix_t::button_areas`: the button areas relative to the
    /// object (upstream tests read the field).
    pub fn button_areas(self, ui: &Ui) -> &[Area] {
        state(ui, self.0).map_or(&[][..], |btnm| btnm.button_areas.as_slice())
    }

    /// Assign `lv_buttonmatrix_t::btn_id_sel` without invalidating anything
    /// (upstream tests write the field).
    pub fn set_btn_id_sel(self, ui: &mut Ui, btn_id: u32) {
        if let Some(btnm) = state_mut(ui, self.0) {
            btnm.btn_id_sel = btn_id;
        }
    }
}

impl From<Buttonmatrix> for Obj {
    fn from(btnm: Buttonmatrix) -> Self {
        btnm.obj()
    }
}

/// `LV_CHECK_OBJ(obj, MY_CLASS, ...)`: a valid object of the button matrix
/// class.
fn check_obj(ui: &Ui, obj: Obj) -> bool {
    ui.obj_is_valid(obj) && ui.obj_has_class(obj, &BUTTONMATRIX_CLASS)
}

fn state(ui: &Ui, obj: Obj) -> Option<&ButtonmatrixState> {
    if !check_obj(ui, obj) {
        return None;
    }
    ui.obj(obj)?.widget.get::<ButtonmatrixState>()
}

fn state_mut(ui: &mut Ui, obj: Obj) -> Option<&mut ButtonmatrixState> {
    if !check_obj(ui, obj) {
        return None;
    }
    ui.obj_mut(obj)?.widget.get_mut::<ButtonmatrixState>()
}

/// `map[i]` read as a C string (up to its first NUL); `None` is past the end
/// of the slice, read like a `NULL` entry.
fn map_entry(map: &[SharedStr], i: usize) -> Option<&str> {
    map.get(i).map(|s| s.split('\0').next().unwrap_or(""))
}

/// `btnm->ctrl_bits[i]`
fn ctrl_at(btnm: &ButtonmatrixState, i: u32) -> ButtonmatrixCtrl {
    btnm.ctrl_bits.get(i as usize).copied().unwrap_or_default()
}

/// `btnm->button_areas[i]`
fn area_at(btnm: &ButtonmatrixState, i: u32) -> Area {
    btnm.button_areas.get(i as usize).copied().unwrap_or_default()
}

/// `btnm->btn_id_sel`
fn sel(ui: &Ui, obj: Obj) -> u32 {
    state(ui, obj).map_or(BUTTONMATRIX_BUTTON_NONE, |btnm| btnm.btn_id_sel)
}

/// `obj->state = state` without any style refresh.
fn set_state_raw(ui: &mut Ui, obj: Obj, state: State) {
    if let Some(od) = ui.obj_mut(obj) {
        od.state = state;
    }
}

/// `obj->skip_trans = skip`
fn set_skip_trans(ui: &mut Ui, obj: Obj, skip: bool) {
    if let Some(od) = ui.obj_mut(obj) {
        od.skip_trans = skip;
    }
}

/// `lv_buttonmatrix_set_map()`
fn set_map(ui: &mut Ui, obj: Obj, map: ButtonmatrixMap) {
    // LV_CHECK_OBJ(obj, MY_CLASS, return);
    let Some(btnm) = state(ui, obj) else {
        return;
    };

    /*Remember the on-screen position of the selected button. Changing the map changes which
     *button each index refers to, so the selection is restored by position below to avoid the
     *highlight jumping to an unrelated button.*/
    let had_sel = btnm.btn_id_sel != BUTTONMATRIX_BUTTON_NONE && btnm.btn_id_sel < btnm.btn_cnt;
    let mut sel_center = Point::new(0, 0);
    if had_sel {
        let obj_cords = ui.obj_get_coords(obj);
        let a = area_at(btnm, btnm.btn_id_sel);
        sel_center.x = obj_cords.x1 + a.x1 + (a.width() >> 1);
        sel_center.y = obj_cords.y1 + a.y1 + (a.height() >> 1);
    }

    let Some(btnm) = state_mut(ui, obj) else {
        return;
    };
    if btnm.auto_free_map {
        free_map(btnm);
    }
    btnm.auto_free_map = false;

    /*Analyze the map and create the required number of buttons*/
    allocate_button_areas_and_controls(btnm, &map);
    btnm.map_p = Some(map);

    update_map(ui, obj);

    /*Restore the selection on the button now occupying the previous selection's position*/
    if had_sel {
        let mut new_sel = get_button_from_point(ui, obj, sel_center);
        let Some(btnm) = state_mut(ui, obj) else {
            return;
        };
        if new_sel != BUTTONMATRIX_BUTTON_NONE
            && (button_is_hidden(ctrl_at(btnm, new_sel)) || button_is_inactive(ctrl_at(btnm, new_sel)))
        {
            new_sel = BUTTONMATRIX_BUTTON_NONE;
        }
        btnm.btn_id_sel = new_sel;
        invalidate_button_area(ui, obj, new_sel);
    }
}

/// `lv_buttonmatrix_set_selected_button()`
fn set_selected_button(ui: &mut Ui, obj: Obj, btn_id: u32) {
    // LV_CHECK_OBJ(obj, MY_CLASS, return);
    let Some(btnm) = state(ui, obj) else {
        return;
    };

    if btn_id >= btnm.btn_cnt && btn_id != BUTTONMATRIX_BUTTON_NONE {
        return;
    }

    let old = btnm.btn_id_sel;
    invalidate_button_area(ui, obj, old);
    let Some(btnm) = state_mut(ui, obj) else {
        return;
    };
    btnm.btn_id_sel = btn_id;
    invalidate_button_area(ui, obj, btn_id);
}

/// `lv_buttonmatrix_set_button_ctrl()`
fn set_button_ctrl(ui: &mut Ui, obj: Obj, btn_id: u32, ctrl: ButtonmatrixCtrl) {
    // LV_CHECK_OBJ(obj, MY_CLASS, return);
    let Some(btnm) = state(ui, obj) else {
        return;
    };

    if btn_id >= btnm.btn_cnt {
        return;
    }

    if btnm.one_check && ctrl.intersects(ButtonmatrixCtrl::CHECKED) {
        clear_button_ctrl_all(ui, obj, ButtonmatrixCtrl::CHECKED);
    }

    let Some(btnm) = state_mut(ui, obj) else {
        return;
    };
    if let Some(bits) = btnm.ctrl_bits.get_mut(btn_id as usize) {
        *bits |= ctrl;
    }
    invalidate_button_area(ui, obj, btn_id);

    if ctrl.intersects(ButtonmatrixCtrl::POPOVER) {
        ui.obj_refresh_ext_draw_size(obj);
    }
}

/// `lv_buttonmatrix_clear_button_ctrl()`
fn clear_button_ctrl(ui: &mut Ui, obj: Obj, btn_id: u32, ctrl: ButtonmatrixCtrl) {
    // LV_CHECK_OBJ(obj, MY_CLASS, return);
    let Some(btnm) = state_mut(ui, obj) else {
        return;
    };

    if btn_id >= btnm.btn_cnt {
        return;
    }

    if let Some(bits) = btnm.ctrl_bits.get_mut(btn_id as usize) {
        *bits &= !ctrl;
    }
    invalidate_button_area(ui, obj, btn_id);

    if ctrl.intersects(ButtonmatrixCtrl::POPOVER) {
        ui.obj_refresh_ext_draw_size(obj);
    }
}

/// `lv_buttonmatrix_clear_button_ctrl_all()`
fn clear_button_ctrl_all(ui: &mut Ui, obj: Obj, ctrl: ButtonmatrixCtrl) {
    // LV_CHECK_OBJ(obj, MY_CLASS, return);
    let mut i = 0;
    // `btnm->btn_cnt` is read again in every iteration.
    while i < state(ui, obj).map_or(0, |btnm| btnm.btn_cnt) {
        clear_button_ctrl(ui, obj, i, ctrl);
        i += 1;
    }
}

/// `lv_buttonmatrix_has_button_ctrl()`
fn has_button_ctrl(ui: &Ui, obj: Obj, btn_id: u32, ctrl: ButtonmatrixCtrl) -> bool {
    // LV_CHECK_OBJ(obj, MY_CLASS, return false);
    let Some(btnm) = state(ui, obj) else {
        return false;
    };
    if btn_id >= btnm.btn_cnt {
        return false;
    }

    ctrl_at(btnm, btn_id).contains(ctrl)
}

/// `lv_buttonmatrix_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    if let Some(btnm) = state_mut(ui, obj) {
        btnm.btn_cnt = 0;
        btnm.row_cnt = 0;
        btnm.btn_id_sel = BUTTONMATRIX_BUTTON_NONE;
        btnm.button_areas = Vec::new();
        btnm.ctrl_bits = Vec::new();
        btnm.map_p = None;
        btnm.one_check = false;
        btnm.auto_free_map = false;
    }

    // LV_WIDGETS_HAS_DEFAULT_VALUE
    set_map(ui, obj, SharedRef::Static(BUTTONMATRIX_DEF_MAP));
}

/// `lv_buttonmatrix_destructor()`
fn destructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    if let Some(btnm) = state_mut(ui, obj) {
        btnm.button_areas = Vec::new();
        btnm.ctrl_bits = Vec::new();
    }
}

/// `lv_buttonmatrix_event()`
fn buttonmatrix_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    /*Call the ancestor's event handler*/
    if ui.obj_event_base(&BUTTONMATRIX_CLASS, event) != LvResult::Ok {
        return;
    }

    let code = event.code;
    let Some(obj) = event.current_target_obj() else {
        return;
    };
    if state(ui, obj).is_none() {
        return;
    }

    if code == EventCode::REFR_EXT_DRAW_SIZE && state(ui, obj).is_some_and(has_popovers_in_top_row) {
        /*reserve one row worth of extra space to account for popovers in the top row*/
        let row_cnt = state(ui, obj).map_or(0, |btnm| btnm.row_cnt);
        // `lv_obj_get_content_height(obj) / btnm->row_cnt` is an unsigned
        // division (`int32_t / uint32_t`).
        let s = (ui.obj_get_content_height(obj) as u32)
            .checked_div(row_cnt)
            .map_or(0, |s| s as i32);
        event.set_ext_draw_size(s);
    }
    // Upstream has two branches with the same body.
    if code == EventCode::STYLE_CHANGED || code == EventCode::SIZE_CHANGED {
        update_map(ui, obj);
    } else if code == EventCode::PRESSED {
        let indev = event.indev();
        invalidate_button_area(ui, obj, sel(ui, obj));

        let indev_type = indev.map_or(IndevType::None, |indev| ui.indev_get_type(indev));
        if let Some(indev) = indev
            && matches!(indev_type, IndevType::Pointer | IndevType::Button)
        {
            /*Search the pressed area*/
            let p = ui.indev_get_point(indev);
            let btn_pr = get_button_from_point(ui, obj, p);
            let Some(btnm) = state_mut(ui, obj) else {
                return;
            };
            /*Handle the case where there is no button there*/
            btnm.btn_id_sel = BUTTONMATRIX_BUTTON_NONE;
            if btn_pr != BUTTONMATRIX_BUTTON_NONE {
                if !button_is_inactive(ctrl_at(btnm, btn_pr)) && !button_is_hidden(ctrl_at(btnm, btn_pr)) {
                    btnm.btn_id_sel = btn_pr;
                    invalidate_button_area(ui, obj, btn_pr); /*Invalidate the new area*/
                }
            } else {
                btnm.btn_id_sel = BUTTONMATRIX_BUTTON_NONE;
            }
        }

        let Some(btnm) = state(ui, obj) else {
            return;
        };
        if btnm.btn_id_sel != BUTTONMATRIX_BUTTON_NONE {
            let bits = ctrl_at(btnm, btnm.btn_id_sel);
            if !button_is_click_trig(bits)
                && !button_is_popover(bits)
                && !button_is_inactive(bits)
                && !button_is_hidden(bits)
            {
                let b: u32 = btnm.btn_id_sel;
                // `if(res != LV_RESULT_OK) return;` is the end of the handler.
                ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::Any(&b));
            }
        }
    } else if code == EventCode::PRESSING {
        if sel(ui, obj) != BUTTONMATRIX_BUTTON_NONE {
            let indev = event.indev();
            let indev_type = indev.map_or(IndevType::None, |indev| ui.indev_get_type(indev));
            if let Some(indev) = indev
                && matches!(indev_type, IndevType::Pointer | IndevType::Button)
            {
                /*If pointer device slid to a new button, discard the current button and don't press any buttons*/
                let p = ui.indev_get_point(indev);
                let btn_pr = get_button_from_point(ui, obj, p);
                let old = sel(ui, obj);
                if btn_pr != old {
                    invalidate_button_area(ui, obj, old); /*Invalidate the old area*/
                    if let Some(btnm) = state_mut(ui, obj) {
                        btnm.btn_id_sel = BUTTONMATRIX_BUTTON_NONE;
                    }
                }
            }
        }
    } else if code == EventCode::RELEASED {
        let Some(btnm) = state_mut(ui, obj) else {
            return;
        };
        if btnm.btn_id_sel != BUTTONMATRIX_BUTTON_NONE {
            let sel_id = btnm.btn_id_sel;
            let bits = ctrl_at(btnm, sel_id);
            /*Toggle the button if enabled*/
            if button_is_checkable(bits) && !button_is_inactive(bits) {
                if let Some(b) = btnm.ctrl_bits.get_mut(sel_id as usize) {
                    if button_get_checked(bits) && !btnm.one_check {
                        *b &= !ButtonmatrixCtrl::CHECKED;
                    } else {
                        *b |= ButtonmatrixCtrl::CHECKED;
                    }
                }
                if btnm.one_check {
                    make_one_button_checked(ui, obj, sel_id);
                }
            }

            let Some(btnm) = state(ui, obj) else {
                return;
            };
            let bits = ctrl_at(btnm, btnm.btn_id_sel);
            if (button_is_click_trig(bits) || button_is_popover(bits))
                && !button_is_inactive(bits)
                && !button_is_hidden(bits)
            {
                let b: u32 = btnm.btn_id_sel;
                let res = ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::Any(&b));
                if res != LvResult::Ok {
                    return;
                }
            }
        }

        /*Invalidate to old pressed area*/
        invalidate_button_area(ui, obj, sel(ui, obj));
    } else if code == EventCode::LONG_PRESSED_REPEAT {
        let Some(btnm) = state(ui, obj) else {
            return;
        };
        if btnm.btn_id_sel != BUTTONMATRIX_BUTTON_NONE {
            let bits = ctrl_at(btnm, btnm.btn_id_sel);
            if !button_is_repeat_disabled(bits) && !button_is_inactive(bits) && !button_is_hidden(bits) {
                let b: u32 = btnm.btn_id_sel;
                // `if(res != LV_RESULT_OK) return;` is the end of the handler.
                ui.obj_send_event(obj, EventCode::VALUE_CHANGED, EventParam::Any(&b));
            }
        }
    } else if code == EventCode::PRESS_LOST {
        invalidate_button_area(ui, obj, sel(ui, obj));
        if let Some(btnm) = state_mut(ui, obj) {
            btnm.btn_id_sel = BUTTONMATRIX_BUTTON_NONE;
        }
    } else if code == EventCode::FOCUSED {
        if state(ui, obj).is_none_or(|btnm| btnm.btn_cnt == 0) {
            return;
        }

        let mut indev = event.indev();

        /*If not focused by an input device assume the last input device*/
        if indev.is_none() {
            indev = ui.indev_get_next(None);
        }
        let indev_type = indev.map_or(IndevType::None, |indev| ui.indev_get_type(indev));

        let g = ui.obj_get_group(obj);
        let editing = g.is_some_and(|g| ui.group_get_editing(g));
        let Some(btnm) = state_mut(ui, obj) else {
            return;
        };
        /*Focus the first button if there is not selected button*/
        if btnm.btn_id_sel == BUTTONMATRIX_BUTTON_NONE {
            if indev_type == IndevType::Keypad || (indev_type == IndevType::Encoder && editing) {
                let mut b: u32 = 0;
                if btnm.one_check {
                    while b < btnm.btn_cnt
                        && (button_is_hidden(ctrl_at(btnm, b))
                            || button_is_inactive(ctrl_at(btnm, b))
                            || !button_is_checked(ctrl_at(btnm, b)))
                    {
                        b += 1;
                    }
                } else {
                    while b < btnm.btn_cnt
                        && (button_is_hidden(ctrl_at(btnm, b)) || button_is_inactive(ctrl_at(btnm, b)))
                    {
                        b += 1;
                    }
                }

                btnm.btn_id_sel = b;
            } else {
                btnm.btn_id_sel = BUTTONMATRIX_BUTTON_NONE;
            }
        }
    } else if code == EventCode::DEFOCUSED || code == EventCode::LEAVE {
        // Upstream has only a commented out TODO here.
    } else if code == EventCode::KEY {
        invalidate_button_area(ui, obj, sel(ui, obj));

        let c = event.key();
        let col_gap = ui.obj_get_style_pad_column(obj, Part::MAIN);
        let Some(btnm) = state_mut(ui, obj) else {
            return;
        };
        let btn_cnt = btnm.btn_cnt;
        let mut sel_id = btnm.btn_id_sel;
        let hidden_or_inactive = |btnm: &ButtonmatrixState, i: u32| {
            button_is_hidden(ctrl_at(btnm, i)) || button_is_inactive(ctrl_at(btnm, i))
        };
        if c == ferrovgl_core::group::KEY_RIGHT {
            if sel_id == BUTTONMATRIX_BUTTON_NONE {
                sel_id = 0;
            } else {
                sel_id = sel_id.wrapping_add(1);
            }
            if sel_id >= btn_cnt {
                sel_id = 0;
            }

            let btn_id_start = sel_id;
            while hidden_or_inactive(btnm, sel_id) {
                sel_id = sel_id.wrapping_add(1);
                if sel_id >= btn_cnt {
                    sel_id = 0;
                }

                if sel_id == btn_id_start {
                    sel_id = BUTTONMATRIX_BUTTON_NONE;
                    break;
                }
            }
        } else if c == ferrovgl_core::group::KEY_LEFT {
            if sel_id == BUTTONMATRIX_BUTTON_NONE {
                sel_id = 0;
            }

            if sel_id == 0 {
                sel_id = btn_cnt.wrapping_sub(1);
            } else {
                // `else if(btnm->btn_id_sel > 0)`: always true here.
                sel_id -= 1;
            }

            let btn_id_start = sel_id;
            while hidden_or_inactive(btnm, sel_id) {
                if sel_id > 0 {
                    sel_id -= 1;
                } else {
                    sel_id = btn_cnt.wrapping_sub(1);
                }

                if sel_id == btn_id_start {
                    sel_id = BUTTONMATRIX_BUTTON_NONE;
                    break;
                }
            }
        } else if c == ferrovgl_core::group::KEY_DOWN {
            /*Find the area below the current*/
            if sel_id == BUTTONMATRIX_BUTTON_NONE {
                sel_id = 0;
                while hidden_or_inactive(btnm, sel_id) {
                    sel_id = sel_id.wrapping_add(1);
                    if sel_id >= btn_cnt {
                        sel_id = BUTTONMATRIX_BUTTON_NONE;
                        break;
                    }
                }
            } else {
                let sel_area = area_at(btnm, sel_id);
                let pr_center = sel_area.x1 + (sel_area.width() >> 1);

                let mut area_below = sel_id;
                while area_below < btn_cnt {
                    let a = area_at(btnm, area_below);
                    if a.y1 > sel_area.y1
                        && pr_center >= a.x1
                        && pr_center <= a.x2 + col_gap
                        && !button_is_inactive(ctrl_at(btnm, area_below))
                        && !button_is_hidden(ctrl_at(btnm, area_below))
                    {
                        break;
                    }
                    area_below += 1;
                }

                if area_below < btn_cnt {
                    sel_id = area_below;
                }
            }
        } else if c == ferrovgl_core::group::KEY_UP {
            /*Find the area below the current*/
            if sel_id == BUTTONMATRIX_BUTTON_NONE {
                sel_id = 0;
                while hidden_or_inactive(btnm, sel_id) {
                    sel_id = sel_id.wrapping_add(1);
                    if sel_id >= btn_cnt {
                        sel_id = BUTTONMATRIX_BUTTON_NONE;
                        break;
                    }
                }
            } else {
                let sel_area = area_at(btnm, sel_id);
                let pr_center = sel_area.x1 + (sel_area.width() >> 1);

                // `int16_t area_above = btnm->btn_id_sel` (truncating).
                let mut area_above = sel_id as i16;
                while area_above >= 0 {
                    let idx = area_above as u32;
                    let a = area_at(btnm, idx);
                    if a.y1 < sel_area.y1
                        && pr_center >= a.x1 - col_gap
                        && pr_center <= a.x2
                        && !button_is_inactive(ctrl_at(btnm, idx))
                        && !button_is_hidden(ctrl_at(btnm, idx))
                    {
                        break;
                    }
                    area_above -= 1;
                }
                if area_above >= 0 {
                    sel_id = area_above as u32;
                }
            }
        }
        btnm.btn_id_sel = sel_id;

        invalidate_button_area(ui, obj, sel_id);
    } else if code == EventCode::DRAW_MAIN {
        draw_main(ui, obj, event);
    } else if code == EventCode::DELETE
        && let Some(btnm) = state_mut(ui, obj)
        && btnm.auto_free_map
    {
        free_map(btnm);
    }
}

/// `draw_main()`
fn draw_main(ui: &mut Ui, obj: Obj, event: &Event<'_>) {
    let Some(btnm) = state(ui, obj) else {
        return;
    };
    if btnm.btn_cnt == 0 {
        return;
    }

    let Some(layer) = event.layer() else {
        return;
    };
    set_skip_trans(ui, obj, true);

    let area_obj = ui.obj_get_coords(obj);

    let mut txt_i: usize = 0;

    let state_ori = ui.obj(obj).map_or(State::DEFAULT, |od| od.state);
    set_state_raw(ui, obj, State::DEFAULT);
    set_skip_trans(ui, obj, true);
    let mut draw_rect_dsc_def = RectDsc::new();
    draw_rect_dsc_def.base.layer = Some(layer);
    let mut draw_label_dsc_def = LabelDsc::new();
    draw_label_dsc_def.base.layer = Some(layer);
    ui.obj_init_draw_rect_dsc(obj, Part::ITEMS, &mut draw_rect_dsc_def);
    ui.obj_init_draw_label_dsc(obj, Part::ITEMS, &mut draw_label_dsc_def);
    set_skip_trans(ui, obj, false);
    set_state_raw(ui, obj, state_ori);

    let ptop = ui.obj_get_style_pad_top(obj, Part::MAIN);
    let pbottom = ui.obj_get_style_pad_bottom(obj, Part::MAIN);
    let pleft = ui.obj_get_style_pad_left(obj, Part::MAIN);
    let pright = ui.obj_get_style_pad_right(obj, Part::MAIN);

    /*Clip margin for the early skip check in the draw loop below.
     *Buttons can render outside their base area due to outline and shadow.
     *Use the maximum of row and column gaps (with a dpi/10 floor) as a
     *conservative uniform margin on all sides.*/
    let mut row_gap = ui.obj_get_style_pad_row(obj, Part::MAIN);
    let mut col_gap = ui.obj_get_style_pad_column(obj, Part::MAIN);
    let dpi_margin = ui.display_get_dpi(ui.obj_get_display(obj)) / 10;
    row_gap = row_gap.max(dpi_margin);
    col_gap = col_gap.max(dpi_margin);
    let clip_margin = row_gap.max(col_gap);

    let mut btn_i: u32 = 0;
    loop {
        // `for(btn_i = 0; btn_i < btnm->btn_cnt; btn_i++, txt_i++)`: the
        // fields are read again after the draw calls that can run user
        // callbacks (`LV_EVENT_DRAW_TASK_ADDED`).
        if btn_i > 0 {
            txt_i += 1;
        }
        let Some(btnm) = state(ui, obj) else {
            return;
        };
        if btn_i >= btnm.btn_cnt {
            break;
        }
        let cur = btn_i;
        btn_i += 1;
        let map = btnm.map_p.clone();
        let map: &[SharedStr] = map.as_deref().unwrap_or(&[]);

        /*Search the next valid text in the map*/
        while map_entry(map, txt_i) == Some("\n") {
            txt_i += 1;
        }

        let ctrl_bits = ctrl_at(btnm, cur);

        /*Skip hidden buttons*/
        if button_is_hidden(ctrl_bits) {
            continue;
        }

        /*Get the state of the button*/
        let mut btn_state = State::DEFAULT;
        if button_get_checked(ctrl_bits) {
            btn_state |= State::CHECKED;
        }

        if button_is_inactive(ctrl_bits) {
            btn_state |= State::DISABLED;
        } else if cur == btnm.btn_id_sel {
            if state_ori.intersects(State::PRESSED) {
                btn_state |= State::PRESSED;
            }
            if state_ori.intersects(State::FOCUSED) {
                btn_state |= State::FOCUSED;
            }
            if state_ori.intersects(State::FOCUS_KEY) {
                btn_state |= State::FOCUS_KEY;
            }
            if state_ori.intersects(State::EDITED) {
                btn_state |= State::EDITED;
            }
        }

        /*Get the button's area*/
        let mut btn_area = area_at(btnm, cur);
        btn_area.x1 += area_obj.x1;
        btn_area.y1 += area_obj.y1;
        btn_area.x2 += area_obj.x1;
        btn_area.y2 += area_obj.y1;

        /*Skip buttons outside the clip area.  Extend the test area by
         *clip_margin (shadow/outline) and popover height when applicable.*/
        let mut clip_test = btn_area;
        clip_test.x1 -= clip_margin;
        clip_test.y1 -= clip_margin;
        clip_test.x2 += clip_margin;
        clip_test.y2 += clip_margin;
        if button_is_popover(ctrl_bits) {
            clip_test.y1 -= btn_area.height();
        }
        let Some(layer_clip) = ui.draw.layer(layer).map(|l| l.clip_area) else {
            return;
        };
        if !clip_test.is_on(&layer_clip) {
            continue;
        }

        /*Set up the draw descriptors*/
        let mut draw_rect_dsc_act;
        let mut draw_label_dsc_act;
        if btn_state == State::DEFAULT {
            draw_rect_dsc_act = draw_rect_dsc_def.clone();
            draw_label_dsc_act = draw_label_dsc_def.clone();
        }
        /*In other cases get the styles directly without caching them*/
        else {
            set_state_raw(ui, obj, btn_state);
            set_skip_trans(ui, obj, true);
            draw_rect_dsc_act = RectDsc::new();
            draw_rect_dsc_act.base.layer = Some(layer);
            draw_label_dsc_act = LabelDsc::new();
            draw_label_dsc_act.base.layer = Some(layer);
            ui.obj_init_draw_rect_dsc(obj, Part::ITEMS, &mut draw_rect_dsc_act);
            ui.obj_init_draw_label_dsc(obj, Part::ITEMS, &mut draw_label_dsc_act);
            set_state_raw(ui, obj, state_ori);
            set_skip_trans(ui, obj, false);
        }

        let recolor = button_is_recolor(ctrl_bits);
        if recolor {
            draw_label_dsc_act.flag |= TextFlags::RECOLOR;
        } else {
            draw_label_dsc_act.flag &= !TextFlags::RECOLOR;
        }

        draw_rect_dsc_act.base.id1 = cur;

        /*Remove borders on the edges if `LV_BORDER_SIDE_INTERNAL`*/
        if draw_rect_dsc_act.border_side.intersects(BorderSide::INTERNAL) {
            let coords = ui.obj_get_coords(obj);
            draw_rect_dsc_act.border_side = BorderSide::FULL;
            if btn_area.x1 == coords.x1 + pleft {
                draw_rect_dsc_act.border_side &= !BorderSide::LEFT;
            }
            if btn_area.x2 == coords.x2 - pright {
                draw_rect_dsc_act.border_side &= !BorderSide::RIGHT;
            }
            if btn_area.y1 == coords.y1 + ptop {
                draw_rect_dsc_act.border_side &= !BorderSide::TOP;
            }
            if btn_area.y2 == coords.y2 - pbottom {
                draw_rect_dsc_act.border_side &= !BorderSide::BOTTOM;
            }
        }

        let btn_height = btn_area.height();

        if btn_state.intersects(State::PRESSED) && ctrl_bits.intersects(ButtonmatrixCtrl::POPOVER) {
            /*Push up the upper boundary of the btn area to create the popover*/
            btn_area.y1 -= btn_height;
        }

        /*Draw the background*/
        ui.draw_rect(layer, &draw_rect_dsc_act, &btn_area);
        if !ui.obj_is_valid(obj) {
            return;
        }

        /*Calculate the size of the text*/
        let Some(font) = draw_label_dsc_act.font.clone() else {
            continue;
        };
        let letter_space = draw_label_dsc_act.letter_space;
        let line_space = draw_label_dsc_act.line_space;
        let txt: &str = map_entry(map, txt_i).unwrap_or("");

        #[cfg(feature = "arabic-persian-chars")]
        let txt_ap;
        #[cfg(feature = "arabic-persian-chars")]
        let txt: &str = {
            /*Get the size of the Arabic text and process it*/
            // `char txt_ap[256]`
            let len_ap = ferrovgl_core::misc::text_ap::ap_strlen(txt) + 1;
            if len_ap < 256 {
                txt_ap = ferrovgl_core::misc::text_ap::ap_proc(txt);
                txt_ap.as_str()
            } else {
                txt
            }
        };
        let attributes = TextAttributes {
            letter_space,
            line_space,
            text_flags: draw_label_dsc_act.flag,
            max_width: area_obj.width(),
        };

        let txt_size = text::get_size_attributes(txt, font.as_font(), &attributes);

        btn_area.x1 += (btn_area.width() - txt_size.x) / 2;
        btn_area.y1 += (btn_area.height() - txt_size.y) / 2;
        btn_area.x2 = btn_area.x1 + txt_size.x;
        btn_area.y2 = btn_area.y1 + txt_size.y;

        if btn_state.intersects(State::PRESSED) && ctrl_bits.intersects(ButtonmatrixCtrl::POPOVER) {
            /*Push up the button text into the popover*/
            btn_area.y1 -= btn_height / 2;
            btn_area.y2 -= btn_height / 2;
        }

        /*Draw the text*/
        draw_label_dsc_act.text = Rc::from(txt);
        draw_label_dsc_act.text_local = true;
        draw_label_dsc_act.base.id1 = cur;
        ui.draw_label(layer, &draw_label_dsc_act, &btn_area);
        if !ui.obj_is_valid(obj) {
            return;
        }
    }

    set_skip_trans(ui, obj, false);
}

/// `allocate_button_areas_and_controls()`: create the required number of
/// buttons and control bytes according to a map.
fn allocate_button_areas_and_controls(btnm: &mut ButtonmatrixState, map: &[SharedStr]) {
    btnm.row_cnt = 1;
    /*Count the buttons in the map*/
    let mut btn_cnt: u32 = 0;
    let mut i = 0;
    while let Some(entry) = map_entry(map, i)
        && !entry.is_empty()
    {
        if entry != "\n" {
            /*Do not count line breaks*/
            btn_cnt += 1;
        } else {
            btnm.row_cnt += 1;
        }
        i += 1;
    }

    /*Do not allocate memory for the same amount of buttons*/
    if btn_cnt == btnm.btn_cnt {
        return;
    }

    // `lv_malloc` leaves the areas uninitialized; `update_map()` sets every
    // one of them before use. The control bytes are zeroed.
    btnm.button_areas = vec![Area::default(); btn_cnt as usize];
    btnm.ctrl_bits = vec![ButtonmatrixCtrl::NONE; btn_cnt as usize];

    btnm.btn_cnt = btn_cnt;
}

/// `get_button_width()`: the width of a button in units (default is 1).
fn get_button_width(ctrl_bits: ButtonmatrixCtrl) -> u32 {
    let w = u32::from(ctrl_bits.0 & BUTTONMATRIX_WIDTH_MASK);
    if w != 0 { w } else { 1 }
}

/// `button_is_hidden()`
fn button_is_hidden(ctrl_bits: ButtonmatrixCtrl) -> bool {
    ctrl_bits.intersects(ButtonmatrixCtrl::HIDDEN)
}

/// `button_is_checked()`
fn button_is_checked(ctrl_bits: ButtonmatrixCtrl) -> bool {
    ctrl_bits.intersects(ButtonmatrixCtrl::CHECKED)
}

/// `button_is_repeat_disabled()`
fn button_is_repeat_disabled(ctrl_bits: ButtonmatrixCtrl) -> bool {
    ctrl_bits.intersects(ButtonmatrixCtrl::NO_REPEAT)
}

/// `button_is_inactive()`
fn button_is_inactive(ctrl_bits: ButtonmatrixCtrl) -> bool {
    ctrl_bits.intersects(ButtonmatrixCtrl::DISABLED)
}

/// `button_is_click_trig()`
fn button_is_click_trig(ctrl_bits: ButtonmatrixCtrl) -> bool {
    ctrl_bits.intersects(ButtonmatrixCtrl::CLICK_TRIG)
}

/// `button_is_popover()`
fn button_is_popover(ctrl_bits: ButtonmatrixCtrl) -> bool {
    ctrl_bits.intersects(ButtonmatrixCtrl::POPOVER)
}

/// `button_is_checkable()`
fn button_is_checkable(ctrl_bits: ButtonmatrixCtrl) -> bool {
    ctrl_bits.intersects(ButtonmatrixCtrl::CHECKABLE)
}

/// `button_get_checked()`
fn button_get_checked(ctrl_bits: ButtonmatrixCtrl) -> bool {
    ctrl_bits.intersects(ButtonmatrixCtrl::CHECKED)
}

/// `button_is_recolor()`
fn button_is_recolor(ctrl_bits: ButtonmatrixCtrl) -> bool {
    ctrl_bits.intersects(ButtonmatrixCtrl::RECOLOR)
}

/// `get_button_from_point()`: the id of the button under a point with
/// absolute coordinates or [`BUTTONMATRIX_BUTTON_NONE`].
fn get_button_from_point(ui: &Ui, obj: Obj, p: Point) -> u32 {
    let Some(btnm) = state(ui, obj) else {
        return BUTTONMATRIX_BUTTON_NONE;
    };
    let obj_cords = ui.obj_get_coords(obj);

    let w = ui.obj_get_width(obj);
    let h = ui.obj_get_height(obj);
    let pleft = ui.obj_get_style_pad_left(obj, Part::MAIN);
    let mut pright = ui.obj_get_style_pad_right(obj, Part::MAIN);
    let mut ptop = ui.obj_get_style_pad_top(obj, Part::MAIN);
    let mut pbottom = ui.obj_get_style_pad_bottom(obj, Part::MAIN);
    let mut prow = ui.obj_get_style_pad_row(obj, Part::MAIN);
    let mut pcol = ui.obj_get_style_pad_column(obj, Part::MAIN);

    /*Get the half gap. Button look larger with this value. (+1 for rounding error)*/
    prow = (prow / 2) + 1 + (prow & 1);
    pcol = (pcol / 2) + 1 + (pcol & 1);

    prow = prow.min(BTN_EXTRA_CLICK_AREA_MAX);
    pcol = pcol.min(BTN_EXTRA_CLICK_AREA_MAX);
    pright = pright.min(BTN_EXTRA_CLICK_AREA_MAX);
    ptop = ptop.min(BTN_EXTRA_CLICK_AREA_MAX);
    pbottom = pbottom.min(BTN_EXTRA_CLICK_AREA_MAX);

    let mut i: u32 = 0;
    while i < btnm.btn_cnt {
        let mut btn_area = area_at(btnm, i);
        if btn_area.x1 <= pleft {
            btn_area.x1 += obj_cords.x1 - pleft.min(BTN_EXTRA_CLICK_AREA_MAX);
        } else {
            btn_area.x1 += obj_cords.x1 - pcol;
        }

        if btn_area.y1 <= ptop {
            btn_area.y1 += obj_cords.y1 - ptop.min(BTN_EXTRA_CLICK_AREA_MAX);
        } else {
            btn_area.y1 += obj_cords.y1 - prow;
        }

        if btn_area.x2 >= w - pright - 2 {
            btn_area.x2 += obj_cords.x1 + pright.min(BTN_EXTRA_CLICK_AREA_MAX); /*-2 for rounding error*/
        } else {
            btn_area.x2 += obj_cords.x1 + pcol;
        }

        if btn_area.y2 >= h - pbottom - 2 {
            btn_area.y2 += obj_cords.y1 + pbottom.min(BTN_EXTRA_CLICK_AREA_MAX); /*-2 for rounding error*/
        } else {
            btn_area.y2 += obj_cords.y1 + prow;
        }

        if btn_area.is_point_on(p, 0) {
            break;
        }
        i += 1;
    }

    if i == btnm.btn_cnt {
        i = BUTTONMATRIX_BUTTON_NONE;
    }

    i
}

/// `invalidate_button_area()`
fn invalidate_button_area(ui: &mut Ui, obj: Obj, btn_idx: u32) {
    if btn_idx == BUTTONMATRIX_BUTTON_NONE {
        return;
    }

    let Some(btnm) = state(ui, obj) else {
        return;
    };
    if btn_idx >= btnm.btn_cnt {
        return;
    }

    let mut btn_area = area_at(btnm, btn_idx);
    let obj_area = ui.obj_get_coords(obj);

    /*The buttons might have outline and shadow so make the invalidation larger with the gaps between the buttons.
     *It assumes that the outline or shadow is smaller than the gaps*/
    let mut row_gap = ui.obj_get_style_pad_row(obj, Part::MAIN);
    let mut col_gap = ui.obj_get_style_pad_column(obj, Part::MAIN);

    /*Be sure to have a minimal extra space if row/col_gap is small*/
    let dpi = ui.display_get_dpi(ui.obj_get_display(obj));
    row_gap = row_gap.max(dpi / 10);
    col_gap = col_gap.max(dpi / 10);

    /*Convert relative coordinates to absolute*/
    btn_area.x1 += obj_area.x1 - row_gap;
    btn_area.y1 += obj_area.y1 - col_gap;
    btn_area.x2 += obj_area.x1 + row_gap;
    btn_area.y2 += obj_area.y1 + col_gap;

    if btn_idx == btnm.btn_id_sel && ctrl_at(btnm, btn_idx).intersects(ButtonmatrixCtrl::POPOVER) {
        /*Push up the upper boundary of the btn area to also invalidate the popover*/
        btn_area.y1 -= btn_area.height();
    }

    ui.obj_invalidate_area(obj, &btn_area);
}

/// `make_one_button_checked()`: enforce a single checked button by clearing
/// the checked flag of the other buttons.
fn make_one_button_checked(ui: &mut Ui, obj: Obj, btn_idx: u32) {
    /*Save whether the button was toggled*/
    let was_toggled = has_button_ctrl(ui, obj, btn_idx, ButtonmatrixCtrl::CHECKED);

    clear_button_ctrl_all(ui, obj, ButtonmatrixCtrl::CHECKED);

    if was_toggled {
        set_button_ctrl(ui, obj, btn_idx, ButtonmatrixCtrl::CHECKED);
    }
}

/// `has_popovers_in_top_row()`: whether a button of the first row has
/// [`ButtonmatrixCtrl::POPOVER`].
fn has_popovers_in_top_row(btnm: &ButtonmatrixState) -> bool {
    if btnm.row_cnt == 0 {
        return false;
    }

    let Some(map_row) = btnm.map_p.as_deref() else {
        return false;
    };
    let mut btn_cnt: u32 = 0;

    while let Some(entry) = map_entry(map_row, btn_cnt as usize)
        && entry != "\n"
        && !entry.is_empty()
    {
        if button_is_popover(ctrl_at(btnm, btn_cnt)) {
            return true;
        }
        btn_cnt += 1;
    }

    false
}

/// `update_map()`: set the size and position of the buttons.
fn update_map(ui: &mut Ui, obj: Obj) {
    if !check_obj(ui, obj) {
        return;
    }

    let base_dir = ui.obj_get_style_base_dir(obj, Part::MAIN);

    /*Set size and positions of the buttons*/
    let sleft = ui.obj_get_style_space_left(obj, Part::MAIN);
    let stop = ui.obj_get_style_space_top(obj, Part::MAIN);
    let prow = ui.obj_get_style_pad_row(obj, Part::MAIN);
    let pcol = ui.obj_get_style_pad_column(obj, Part::MAIN);

    let max_w = ui.obj_get_content_width(obj);
    let max_h = ui.obj_get_content_height(obj);

    let Some(btnm) = state_mut(ui, obj) else {
        return;
    };

    /*Calculate the position of each row*/
    // `prow * (btnm->row_cnt - 1)` is evaluated in `uint32_t`; the wrapping
    // `i32` arithmetic gives the same bits.
    let max_h_no_gap = max_h.wrapping_sub(prow.wrapping_mul((btnm.row_cnt as i32).wrapping_sub(1)));

    /*Count the units and the buttons in a line
     *(A button can be 1,2,3... unit wide)*/
    let mut btn_tot_i: u32 = 0; /*Act. index of button areas*/
    let map = btnm.map_p.clone();
    let map: &[SharedStr] = map.as_deref().unwrap_or(&[]);
    let mut map_row: usize = 0;

    /*Count the units and the buttons in a line*/
    let row_cnt = btnm.row_cnt as i32;
    for row in 0..row_cnt {
        let mut unit_cnt: u32 = 0; /*Number of units in a row*/
        let mut btn_cnt: u32 = 0; /*Number of buttons in a row*/
        /*Count the buttons and units in this row*/
        while let Some(entry) = map_entry(map, map_row + btn_cnt as usize)
            && entry != "\n"
            && !entry.is_empty()
        {
            unit_cnt += get_button_width(ctrl_at(btnm, btn_tot_i + btn_cnt));
            btn_cnt += 1;
        }

        /*Only deal with the non empty lines*/
        if btn_cnt == 0 {
            map_row += btn_cnt as usize + 1; /*Set the map to the next row*/
            continue;
        }

        let row_y1 = stop + (max_h_no_gap.wrapping_mul(row)) / row_cnt + row * prow;
        let row_y2 = stop + (max_h_no_gap.wrapping_mul(row + 1)) / row_cnt + row * prow - 1;

        /*Set the button size and positions*/
        // `max_w - (pcol * (btn_cnt - 1))` is evaluated in `uint32_t`.
        let mut max_w_no_gap = (max_w as u32).wrapping_sub((pcol as u32).wrapping_mul(btn_cnt - 1)) as i32;
        if max_w_no_gap < 0 {
            max_w_no_gap = 0;
        }

        let mut row_unit_cnt: u32 = 0; /*The current unit position in the row*/
        for btn in 0..btn_cnt {
            let btn_u = get_button_width(ctrl_at(btnm, btn_tot_i));

            // `int32_t * uint32_t` and the following additions are unsigned.
            let gap = btn.wrapping_mul(pcol as u32);
            let mut btn_x1 =
                ((max_w_no_gap as u32).wrapping_mul(row_unit_cnt) / unit_cnt).wrapping_add(gap) as i32;
            let mut btn_x2 = ((max_w_no_gap as u32).wrapping_mul(row_unit_cnt + btn_u) / unit_cnt)
                .wrapping_add(gap)
                .wrapping_sub(1) as i32;

            /*If RTL start from the right*/
            if base_dir == BaseDir::Rtl {
                core::mem::swap(&mut btn_x1, &mut btn_x2);

                btn_x1 = max_w - btn_x1;
                btn_x2 = max_w - btn_x2;
            }

            btn_x1 += sleft;
            btn_x2 += sleft;

            if let Some(a) = btnm.button_areas.get_mut(btn_tot_i as usize) {
                a.set(btn_x1, row_y1, btn_x2, row_y2);
            }

            row_unit_cnt += btn_u;
            btn_tot_i += 1;
        }

        map_row += btn_cnt as usize + 1; /*Set the map to the next line*/
    }

    /*Popovers in the top row will draw outside the widget and the extended draw size depends on
     *the row height which may have changed when setting the new map*/
    ui.obj_refresh_ext_draw_size(obj);

    ui.obj_invalidate(obj);
}

/// `free_map()`: drop the map (`auto_free_map`).
fn free_map(btnm: &mut ButtonmatrixState) {
    btnm.map_p = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrovgl_core::{FontRef, UiConfig};

    fn setup() -> (Ui, Obj) {
        let mut ui = Ui::new(UiConfig {
            font_default: Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14)),
            ..UiConfig::default()
        });
        let display = ui.display_create(320, 240);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        (ui, screen)
    }

    fn map(entries: &[&'static str]) -> ButtonmatrixMap {
        SharedRef::Shared(entries.iter().map(|s| SharedRef::Static(*s)).collect())
    }

    #[test]
    fn constructor_sets_default_map() {
        let (mut ui, screen) = setup();
        let btnm = Buttonmatrix::create(&mut ui, screen);
        assert_eq!(btnm.btn_cnt(&ui), 5);
        assert_eq!(state(&ui, btnm.obj()).unwrap().row_cnt, 2);
        assert_eq!(btnm.selected_button(&ui), BUTTONMATRIX_BUTTON_NONE);
        assert_eq!(btnm.button_text(&ui, 3), Some("Btn4"));
        assert_eq!(btnm.button_text(&ui, 5), None);
        assert_eq!(Buttonmatrix::from_obj(&ui, screen), None);
    }

    #[test]
    fn layout_splits_units_and_gaps() {
        let (mut ui, screen) = setup();
        let btnm = Buttonmatrix::create(&mut ui, screen);
        ui.obj_set_size(btnm.obj(), 100, 50);
        btnm.set_map(&mut ui, map(&["A", "B", "\n", "C", ""]));
        btnm.set_button_width(&mut ui, 0, 3);
        ui.obj_update_layout(btnm.obj());
        // No theme: no padding and no gaps.
        let areas = btnm.button_areas(&ui);
        assert_eq!(areas[0], Area::new(0, 0, 74, 24));
        assert_eq!(areas[1], Area::new(75, 0, 99, 24));
        assert_eq!(areas[2], Area::new(0, 25, 99, 49));
    }

    #[test]
    fn same_button_count_keeps_ctrl_bits_and_trailing_terminator_is_optional() {
        let (mut ui, screen) = setup();
        let btnm = Buttonmatrix::create(&mut ui, screen);
        btnm.set_map(&mut ui, map(&["A", "B"]));
        assert_eq!(btnm.btn_cnt(&ui), 2);
        btnm.set_button_ctrl(&mut ui, 1, ButtonmatrixCtrl::HIDDEN);
        btnm.set_map(&mut ui, map(&["C", "\n", "D", ""]));
        assert!(btnm.has_button_ctrl(&ui, 1, ButtonmatrixCtrl::HIDDEN));
        assert_eq!(btnm.button_text(&ui, 1), Some("D"));
    }

    #[test]
    fn keys_skip_hidden_and_wrap() {
        let (mut ui, screen) = setup();
        let btnm = Buttonmatrix::create(&mut ui, screen);
        btnm.set_map(&mut ui, map(&["A", "B", "C", ""]));
        btnm.set_button_ctrl(&mut ui, 1, ButtonmatrixCtrl::HIDDEN);
        btnm.set_selected_button(&mut ui, 0);
        let key = |ui: &mut Ui, k| {
            ui.obj_send_event(btnm.obj(), EventCode::KEY, EventParam::Key(k));
        };
        key(&mut ui, ferrovgl_core::group::KEY_RIGHT);
        assert_eq!(btnm.selected_button(&ui), 2);
        key(&mut ui, ferrovgl_core::group::KEY_RIGHT);
        assert_eq!(btnm.selected_button(&ui), 0);
        key(&mut ui, ferrovgl_core::group::KEY_LEFT);
        assert_eq!(btnm.selected_button(&ui), 2);
        btnm.set_button_ctrl_all(&mut ui, ButtonmatrixCtrl::DISABLED);
        key(&mut ui, ferrovgl_core::group::KEY_LEFT);
        assert_eq!(btnm.selected_button(&ui), BUTTONMATRIX_BUTTON_NONE);
    }

    #[test]
    fn one_checked_keeps_first() {
        let (mut ui, screen) = setup();
        let btnm = Buttonmatrix::create(&mut ui, screen);
        btnm.set_map(&mut ui, map(&["A", "B", "C", ""]));
        btnm.set_button_ctrl(&mut ui, 0, ButtonmatrixCtrl::CHECKED);
        btnm.set_button_ctrl(&mut ui, 2, ButtonmatrixCtrl::CHECKED);
        btnm.set_one_checked(&mut ui, true);
        assert!(btnm.has_button_ctrl(&ui, 0, ButtonmatrixCtrl::CHECKED));
        assert!(!btnm.has_button_ctrl(&ui, 2, ButtonmatrixCtrl::CHECKED));
        assert!(btnm.one_checked(&ui));
    }
}
