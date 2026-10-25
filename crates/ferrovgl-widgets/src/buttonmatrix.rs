//! Ported from LVGL v9.6.0: `src/widgets/buttonmatrix/lv_buttonmatrix.c`.
//!
//! Also covers `include/lvgl/widgets/lv_buttonmatrix.h` and
//! `src/widgets/buttonmatrix/lv_buttonmatrix_private.h`. The map
//! (`const char * const map[]`, terminated by `""`) is a [`ButtonmatrixMap`]:
//! only the reference is kept, like the C pointer, and it is compared by
//! address. `LV_USE_OBJ_PROPERTY` is disabled in the parity configuration, so
//! the property table is not provided.
//!
//! SKELETON: the class, state and public API are declared; the bodies are
//! `todo!()` until the widget is ported.

// Skeleton only: remove together with the last `todo!()`.
#![allow(unused_variables, dead_code)]

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;

use ferrovgl_core::core::obj::{OBJ_CLASS, ObjClassEditable, ObjClassGroupDef};
use ferrovgl_core::core::ui::DPI_DEF;
use ferrovgl_core::misc::style::{SharedRef, SharedStr};
use ferrovgl_core::{Area, Event, Obj, ObjClass, Ui};

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
        (ui.obj_is_valid(obj) && ui.obj_has_class(obj, &BUTTONMATRIX_CLASS)).then_some(Self(obj))
    }

    /// `lv_buttonmatrix_set_map()`: set a new map; buttons are created and
    /// deleted according to it. The map is referenced, not copied.
    pub fn set_map(self, ui: &mut Ui, map: ButtonmatrixMap) {
        todo!()
    }

    /// `lv_buttonmatrix_set_ctrl_map()`: copy one control value per button
    /// (new lines excluded).
    pub fn set_ctrl_map(self, ui: &mut Ui, ctrl_map: &[ButtonmatrixCtrl]) {
        todo!()
    }

    /// `lv_buttonmatrix_set_selected_button()`: 0 based index, not counting
    /// new lines.
    pub fn set_selected_button(self, ui: &mut Ui, btn_id: u32) {
        todo!()
    }

    /// `lv_buttonmatrix_set_button_ctrl()`: set OR-ed attributes of a button.
    pub fn set_button_ctrl(self, ui: &mut Ui, btn_id: u32, ctrl: ButtonmatrixCtrl) {
        todo!()
    }

    /// `lv_buttonmatrix_clear_button_ctrl()`: clear OR-ed attributes of a
    /// button.
    pub fn clear_button_ctrl(self, ui: &mut Ui, btn_id: u32, ctrl: ButtonmatrixCtrl) {
        todo!()
    }

    /// `lv_buttonmatrix_set_button_ctrl_all()`
    pub fn set_button_ctrl_all(self, ui: &mut Ui, ctrl: ButtonmatrixCtrl) {
        todo!()
    }

    /// `lv_buttonmatrix_clear_button_ctrl_all()`
    pub fn clear_button_ctrl_all(self, ui: &mut Ui, ctrl: ButtonmatrixCtrl) {
        todo!()
    }

    /// `lv_buttonmatrix_set_button_width()`: relative width (1..15) of a
    /// button; regenerates the matrix.
    pub fn set_button_width(self, ui: &mut Ui, btn_id: u32, width: u32) {
        todo!()
    }

    /// `lv_buttonmatrix_set_one_checked()`: only one button may be checked
    /// at a time.
    pub fn set_one_checked(self, ui: &mut Ui, en: bool) {
        todo!()
    }

    /// `lv_buttonmatrix_get_map()`
    pub fn map(self, ui: &Ui) -> Option<ButtonmatrixMap> {
        todo!()
    }

    /// `lv_buttonmatrix_get_selected_button()`: index of the last
    /// "activated" button or [`BUTTONMATRIX_BUTTON_NONE`].
    pub fn selected_button(self, ui: &Ui) -> u32 {
        todo!()
    }

    /// `lv_buttonmatrix_get_button_text()`: `None` is `NULL`.
    pub fn button_text(self, ui: &Ui, btn_id: u32) -> Option<&str> {
        todo!()
    }

    /// `lv_buttonmatrix_has_button_ctrl()`: whether all of `ctrl` are set on
    /// a button.
    pub fn has_button_ctrl(self, ui: &Ui, btn_id: u32, ctrl: ButtonmatrixCtrl) -> bool {
        todo!()
    }

    /// `lv_buttonmatrix_get_one_checked()`
    pub fn one_checked(self, ui: &Ui) -> bool {
        todo!()
    }
}

impl From<Buttonmatrix> for Obj {
    fn from(btnm: Buttonmatrix) -> Self {
        btnm.obj()
    }
}

/// `lv_buttonmatrix_constructor()`
fn constructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_buttonmatrix_destructor()`
fn destructor(_class: &'static ObjClass, ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `lv_buttonmatrix_event()`
fn buttonmatrix_event(_class: &'static ObjClass, ui: &mut Ui, event: &mut Event<'_>) {
    todo!()
}
