//! Ported from lvgl v9.6.0: `src/core/lv_obj_style.c`, `src/core/lv_obj_style_gen.c`
//! (types from `include/lvgl/core/lv_obj_style.h` and `src/core/lv_obj_style_private.h`,
//! inline helpers from `src/core/lv_obj_style_internal.h` and
//! `src/core/lv_obj_style_internal_gen.h`).
//!
//! Styles attached to objects, local styles, style transitions and the
//! property lookup (`get_prop_core`, state weighting, inheritance).
//!
//! Configuration (the `full.defconfig` parity build): `LV_OBJ_STYLE_CACHE = 1`,
//! `LV_USE_COLOR_FILTER = 0`, `LV_DRAW_TRANSFORM_USE_MATRIX = 0`; BiDi follows
//! the `bidi` cargo feature.
//!
//! `trans_t *` pointers become [`StyleTransId`] handles into
//! [`Ui::style_trans`]; `style_trans_ll` order is kept in
//! [`Ui::style_trans_order`] (head first, `lv_ll_ins_head` = insert at 0).
//! Style identity (`obj->styles[i].style == style`) is a private `StyleKey`:
//! the [`StyleId`] of a shared style, the address of a constant style, or the
//! address of the boxed local / transition style.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::{LayerType, Obj, ObjData, ObjId, SpecAttr};
use crate::draw::rect::RADIUS_CIRCLE;
use crate::misc::anim::{Anim, AnimExec, AnimId, AnimPath, AnimTarget};
use crate::misc::arena::{Arena, Key};
use crate::misc::check_arg_failed;
use crate::misc::color::{Color, Color32, OPA_COVER, OPA_MAX, OPA_MIN, OPA_TRANSP, Opa, opa_mix2};
use crate::misc::event::{EventCode, EventParam};
use crate::misc::style::{
    BUILTIN_PROP_FLAG_LOOKUP_TABLE, BlendMode, BorderSide, ConstStyle, Part, STYLE_PROP_FLAG_EXT_DRAW_UPDATE,
    STYLE_PROP_FLAG_INHERITABLE, STYLE_PROP_FLAG_LAYER_UPDATE, STYLE_PROP_FLAG_LAYOUT_UPDATE,
    STYLE_PROP_FLAG_TRANSFORM, State, Style, StyleProp, StyleRes, StyleSelector, StyleValue, get_prop_group,
    prop_has_flag_with_custom,
};
use crate::misc::text::{BaseDir, TextAlign};
use crate::misc::types::coord;

/// Handle of a shared style stored in [`Ui::styles`] (`lv_style_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct StyleId(pub Key);

/// Handle of a running style transition stored in [`Ui::style_trans`]
/// (`trans_t *` of `lv_obj_style.c`); the `var` of transition animations
/// (`AnimTarget::StyleTrans`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct StyleTransId(pub Key);

/// `const lv_style_t * style` of an `lv_obj_style_t`.
pub enum ObjStyleSrc {
    /// `is_local`: a style owned by the object.
    Local(Box<Style>),
    /// `is_trans`: a transition style owned by the object.
    Trans(Box<Style>),
    /// A shared style in [`Ui::styles`].
    Shared(StyleId),
    /// A constant style.
    Const(&'static ConstStyle),
}

impl core::fmt::Debug for ObjStyleSrc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ObjStyleSrc::Local(_) => f.write_str("Local"),
            ObjStyleSrc::Trans(_) => f.write_str("Trans"),
            ObjStyleSrc::Shared(id) => f.debug_tuple("Shared").field(id).finish(),
            ObjStyleSrc::Const(c) => write!(f, "Const({:p})", *c),
        }
    }
}

/// `lv_obj_style_t`
///
/// The C `is_local` / `is_trans` bits are implied by [`ObjStyle::src`]
/// (single source of truth), see [`ObjStyle::is_local`] / [`ObjStyle::is_trans`].
#[derive(Debug)]
pub struct ObjStyle {
    /// `style`
    pub src: ObjStyleSrc,
    /// `selector`
    pub selector: StyleSelector,
    /// `is_disabled`
    pub is_disabled: bool,
    /// `is_theme`
    pub is_theme: bool,
}

impl ObjStyle {
    /// `is_local`
    #[inline]
    pub fn is_local(&self) -> bool {
        matches!(self.src, ObjStyleSrc::Local(_))
    }

    /// `is_trans`
    #[inline]
    pub fn is_trans(&self) -> bool {
        matches!(self.src, ObjStyleSrc::Trans(_))
    }

    /// The owned (local or transition) style, mutably.
    #[inline]
    fn owned_mut(&mut self) -> Option<&mut Style> {
        match &mut self.src {
            ObjStyleSrc::Local(s) | ObjStyleSrc::Trans(s) => Some(s),
            _ => None,
        }
    }
}

/// `lv_style_state_cmp_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum StyleStateCmp {
    /// `LV_STYLE_STATE_CMP_SAME`: the style properties in the 2 states are identical.
    #[default]
    Same = 0,
    /// `LV_STYLE_STATE_CMP_DIFF_REDRAW`: differences are visible, redraw.
    DiffRedraw = 1,
    /// `LV_STYLE_STATE_CMP_DIFF_DRAW_PAD`: ext draw size changed too.
    DiffDrawPad = 2,
    /// `LV_STYLE_STATE_CMP_DIFF_LAYOUT`: layout related properties changed.
    DiffLayout = 3,
}

/// `lv_obj_style_transition_dsc_t`
pub struct ObjStyleTransitionDsc {
    /// `path_cb`
    pub path: AnimPath,
    /// `user_data`
    pub user_data: Option<Rc<dyn Any>>,
    /// `selector`
    pub selector: StyleSelector,
    /// `time`
    pub time: u16,
    /// `delay`
    pub delay: u16,
    /// `prop`
    pub prop: StyleProp,
}

/// `trans_t` of `lv_obj_style.c`: a running style transition (a node of
/// `style_trans_ll`, stored in [`Ui::style_trans`]).
pub struct StyleTrans {
    /// `obj`
    pub obj: Obj,
    /// `prop`
    pub prop: StyleProp,
    /// `selector`
    pub selector: StyleSelector,
    /// `start_value`
    pub start_value: StyleValue,
    /// `end_value`
    pub end_value: StyleValue,
}

/*********************
 *      DEFINES
 *********************/

/// `DRAW_AREA_FLAGS`: property flags that can change the area where the
/// widget is drawn. Before changing such a property the old area needs to be
/// invalidated, else the already drawn pixels are left on the screen.
const DRAW_AREA_FLAGS: u8 = STYLE_PROP_FLAG_TRANSFORM | STYLE_PROP_FLAG_EXT_DRAW_UPDATE;

/// `STYLE_PROP_SHIFTED(prop)`: the presence bit of `prop` in
/// `style_main_prop_is_set` / `style_other_prop_is_set`.
#[inline]
const fn style_prop_shifted(prop: StyleProp) -> u32 {
    1u32 << (prop.0 >> 3)
}

/// `LV_PART_ANY | LV_STATE_ANY`
const SELECTOR_ANY: StyleSelector = StyleSelector(Part::ANY.0 | State::ANY.bits() as u32);

/// `lv_obj_style_get_selector_state()`
#[inline]
pub const fn obj_style_get_selector_state(selector: StyleSelector) -> State {
    selector.state()
}

/// `lv_obj_style_get_selector_part()`
#[inline]
pub const fn obj_style_get_selector_part(selector: StyleSelector) -> Part {
    selector.part()
}

/// Read-only view of the style behind an [`ObjStyleSrc`]
/// (`const lv_style_t *`, either a normal or a constant style).
#[derive(Copy, Clone)]
enum StyleView<'a> {
    /// A normal style (`lv_style_is_const_internal() == false`).
    Dyn(&'a Style),
    /// A constant style.
    Const(&'static ConstStyle),
}

impl StyleView<'_> {
    /// `style->has_group` (`0xFFFFFFFF` for constant styles).
    #[inline]
    fn has_group(self) -> u32 {
        match self {
            StyleView::Dyn(s) => s.has_group,
            StyleView::Const(_) => ConstStyle::HAS_GROUP,
        }
    }

    /// `lv_style_get_prop_internal()`
    #[inline]
    fn get_prop(self, prop: StyleProp) -> Option<StyleValue> {
        match self {
            StyleView::Dyn(s) => s.get_prop(prop).cloned(),
            StyleView::Const(c) => c.get_prop(prop),
        }
    }

    /// Walk the stored properties (a constant style ends at
    /// `LV_STYLE_PROP_INV`) until `f` returns `true`.
    fn any_prop(self, mut f: impl FnMut(StyleProp) -> bool) -> bool {
        match self {
            StyleView::Dyn(s) => s.props.iter().any(|e| f(e.prop)),
            StyleView::Const(c) => {
                for e in c.props {
                    if e.prop == StyleProp::INV {
                        break;
                    }
                    if f(e.prop) {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// OR of `STYLE_PROP_SHIFTED(prop)` of every stored property.
    fn prop_bits(self) -> u32 {
        let mut bits = 0u32;
        self.any_prop(|p| {
            bits |= style_prop_shifted(p);
            false
        });
        bits
    }
}

/// The style behind `src` (`None` for a deleted shared style).
#[inline]
fn src_view<'a>(styles: &'a Arena<Style>, src: &'a ObjStyleSrc) -> Option<StyleView<'a>> {
    match src {
        ObjStyleSrc::Local(s) | ObjStyleSrc::Trans(s) => Some(StyleView::Dyn(s)),
        ObjStyleSrc::Shared(id) => styles.get(id.0).map(StyleView::Dyn),
        ObjStyleSrc::Const(c) => Some(StyleView::Const(c)),
    }
}

/// Identity of a `const lv_style_t *` (pointer comparison in C).
#[derive(Copy, Clone, Debug)]
enum StyleKey {
    /// A shared style.
    Shared(StyleId),
    /// A constant style, compared by address.
    Const(&'static ConstStyle),
    /// A local / transition style, compared by the address of its box.
    Owned(*const Style),
}

impl StyleKey {
    /// `obj->styles[i].style == style`
    fn matches(self, src: &ObjStyleSrc) -> bool {
        match (self, src) {
            (StyleKey::Shared(a), ObjStyleSrc::Shared(b)) => a == *b,
            (StyleKey::Const(a), ObjStyleSrc::Const(b)) => core::ptr::eq(a, *b),
            (StyleKey::Owned(p), ObjStyleSrc::Local(b) | ObjStyleSrc::Trans(b)) => core::ptr::eq(p, &**b),
            _ => false,
        }
    }
}

/// `v1.ptr == v2.ptr && v1.num == v2.num && lv_color_eq(v1.color, v2.color)`:
/// bitwise equality of the C union. An unset value (`.ptr = NULL`) equals
/// `.num = 0` and a black colour, like the zeroed union does.
fn style_value_eq(a: &StyleValue, b: &StyleValue) -> bool {
    match (a, b) {
        (StyleValue::Null, StyleValue::Num(n)) | (StyleValue::Num(n), StyleValue::Null) => *n == 0,
        (StyleValue::Null, StyleValue::Color(c)) | (StyleValue::Color(c), StyleValue::Null) => {
            c.to_int() == 0
        }
        (StyleValue::Num(n), StyleValue::Color(c)) | (StyleValue::Color(c), StyleValue::Num(n)) => {
            *n == c.to_int() as i32
        }
        _ => a == b,
    }
}

/// `lv_bidi_calculate_align_internal()` (`LV_USE_BIDI == 0`)
#[cfg(not(feature = "bidi"))]
fn bidi_calculate_align(align: &mut TextAlign, base_dir: &mut BaseDir, txt: &str) {
    let _ = (txt, base_dir);
    if *align == TextAlign::Auto {
        *align = TextAlign::Left;
    }
}

/// `lv_bidi_calculate_align_internal()` (`LV_USE_BIDI == 1`). The BiDi
/// algorithm is not ported yet: `lv_bidi_detect_base_dir()` is stubbed to
/// `LV_BIDI_BASE_DIR_DEF` (LTR), like `draw/label.rs`.
#[cfg(feature = "bidi")]
fn bidi_calculate_align(align: &mut TextAlign, base_dir: &mut BaseDir, txt: &str) {
    let _ = txt;
    if *base_dir == BaseDir::Auto {
        *base_dir = BaseDir::Ltr;
    }

    if *align == TextAlign::Auto {
        if *base_dir == BaseDir::Rtl {
            *align = TextAlign::Right;
        } else {
            *align = TextAlign::Left;
        }
    }
}

/// The `(enum_t)v.num` casts of `lv_obj_style_internal_gen.h` for the enum
/// typed properties (unknown numbers map to the enum's default).
trait FromStyleNum {
    fn from_style_num(n: i32) -> Self;
}

impl FromStyleNum for crate::misc::area::Align {
    fn from_style_num(n: i32) -> Self {
        use crate::misc::area::Align as A;
        match n {
            1 => A::TopLeft,
            2 => A::TopMid,
            3 => A::TopRight,
            4 => A::BottomLeft,
            5 => A::BottomMid,
            6 => A::BottomRight,
            7 => A::LeftMid,
            8 => A::RightMid,
            9 => A::Center,
            10 => A::OutTopLeft,
            11 => A::OutTopMid,
            12 => A::OutTopRight,
            13 => A::OutBottomLeft,
            14 => A::OutBottomMid,
            15 => A::OutBottomRight,
            16 => A::OutLeftTop,
            17 => A::OutLeftMid,
            18 => A::OutLeftBottom,
            19 => A::OutRightTop,
            20 => A::OutRightMid,
            21 => A::OutRightBottom,
            _ => A::Default,
        }
    }
}

impl FromStyleNum for crate::draw::grad::GradDir {
    fn from_style_num(n: i32) -> Self {
        Self::from_u8(n as u8)
    }
}

impl FromStyleNum for TextAlign {
    fn from_style_num(n: i32) -> Self {
        match n {
            1 => TextAlign::Left,
            2 => TextAlign::Center,
            3 => TextAlign::Right,
            _ => TextAlign::Auto,
        }
    }
}

impl FromStyleNum for crate::misc::text::LeadingTrim {
    fn from_style_num(n: i32) -> Self {
        use crate::misc::text::LeadingTrim as L;
        match n {
            1 => L::CapitalBaseline,
            2 => L::LowerBaseline,
            3 => L::Capital,
            4 => L::Lower,
            _ => L::None,
        }
    }
}

impl FromStyleNum for crate::misc::style::BlurQuality {
    fn from_style_num(n: i32) -> Self {
        Self::from_u8(n as u8)
    }
}

impl FromStyleNum for BlendMode {
    fn from_style_num(n: i32) -> Self {
        Self::from_u8(n as u8)
    }
}

impl FromStyleNum for BaseDir {
    fn from_style_num(n: i32) -> Self {
        match n {
            0x01 => BaseDir::Rtl,
            0x02 => BaseDir::Auto,
            0x20 => BaseDir::Neutral,
            0x21 => BaseDir::Weak,
            _ => BaseDir::Ltr,
        }
    }
}

impl FromStyleNum for crate::layouts::flex::FlexFlow {
    fn from_style_num(n: i32) -> Self {
        Self::from_u8(n as u8)
    }
}

impl FromStyleNum for crate::layouts::flex::FlexAlign {
    fn from_style_num(n: i32) -> Self {
        Self::from_u8(n as u8)
    }
}

impl FromStyleNum for crate::layouts::grid::GridAlign {
    fn from_style_num(n: i32) -> Self {
        Self::from_u8(n as u8)
    }
}

/// Return type of a generated `lv_obj_get_style_*()` for a table `kind`.
macro_rules! obj_style_ret {
    (num, $t:ty) => { i32 };
    (cast, $t:ty) => { $t };
    (enum, $t:ty) => { $t };
    (flags, $t:ty) => { $t };
    (bool, $t:ty) => { bool };
    (color, $t:ty) => { $t };
    (font, $t:ty) => { Option<$t> };
    (grad, $t:ty) => { Option<$t> };
    (image, $t:ty) => { $t };
    (transition, $t:ty) => { Option<$t> };
    (color_filter, $t:ty) => { Option<$t> };
    (tracks, $t:ty) => { Option<$t> };
    (anim, $t:ty) => { Option<$t> };
    (colorkey, $t:ty) => { Option<$t> };
}

/// The `return (type)v.num;` / `v.color` / `v.ptr` of a generated getter.
macro_rules! obj_style_unpack {
    (num, $t:ty, $v:expr) => {
        $v.num()
    };
    (cast, $t:ty, $v:expr) => {
        $v.num() as $t
    };
    (enum, $t:ty, $v:expr) => {
        <$t as FromStyleNum>::from_style_num($v.num())
    };
    (flags, $t:ty, $v:expr) => {
        <$t>::from_bits_retain($v.num() as _)
    };
    (bool, $t:ty, $v:expr) => {
        $v.num() != 0
    };
    (color, $t:ty, $v:expr) => {
        $v.color()
    };
    (font, $t:ty, $v:expr) => {
        match $v {
            StyleValue::Font(f) => Some(f),
            _ => None,
        }
    };
    (grad, $t:ty, $v:expr) => {
        match $v {
            StyleValue::Grad(g) => Some(g),
            _ => None,
        }
    };
    (image, $t:ty, $v:expr) => {
        match $v {
            StyleValue::Image(i) => i,
            _ => crate::misc::style::ImageSrc::None,
        }
    };
    (transition, $t:ty, $v:expr) => {
        match $v {
            StyleValue::Transition(t) => Some(t),
            _ => None,
        }
    };
    (color_filter, $t:ty, $v:expr) => {
        match $v {
            StyleValue::ColorFilter(f) => Some(f),
            _ => None,
        }
    };
    (tracks, $t:ty, $v:expr) => {
        match $v {
            StyleValue::Tracks(t) => Some(t),
            _ => None,
        }
    };
    (anim, $t:ty, $v:expr) => {
        match $v {
            StyleValue::Anim(a) => Some(a),
            _ => None,
        }
    };
    (colorkey, $t:ty, $v:expr) => {
        match $v {
            StyleValue::Colorkey(k) => Some(k),
            _ => None,
        }
    };
}

/// Generate the `lv_obj_set_style_<prop>()` / `lv_obj_get_style_<prop>()`
/// pairs of `lv_obj_style_gen.c` (the getters inline
/// `lv_obj_get_style_<prop>_internal()` of `lv_obj_style_internal_gen.h`).
macro_rules! gen_obj_style_accessors {
    ($( ($c:ident, $oset:ident, $oget:ident, $kind:ident, $t:ty) ),* $(,)?) => {
        impl Ui {
            $(
                #[doc = concat!("`lv_", stringify!($oset), "()`: set the local `LV_STYLE_", stringify!($c), "` property.")]
                pub fn $oset(&mut self, obj: Obj, value: $t, selector: StyleSelector) {
                    if !self.obj_is_valid(obj) {
                        return; /*LV_CHECK_OBJ*/
                    }
                    let v = $crate::style_value_pack!($kind, value);
                    self.obj_set_local_style_prop(obj, StyleProp::$c, v, selector);
                }

                #[doc = concat!("`lv_", stringify!($oget), "()`: the resolved `LV_STYLE_", stringify!($c), "` value.")]
                pub fn $oget(&self, obj: Obj, part: Part) -> obj_style_ret!($kind, $t) {
                    let v = self.obj_get_style_prop(obj, part, StyleProp::$c);
                    obj_style_unpack!($kind, $t, v)
                }
            )*
        }

        /// Every property with a generated accessor, in table order.
        #[cfg(test)]
        const OBJ_STYLE_ACCESSOR_PROPS: &[StyleProp] = &[ $( StyleProp::$c, )* ];
    };
}

/// Short aliases (`ui.set_style_<prop>()` / `ui.get_style_<prop>()`) named
/// after the `obj_set_ident` / `obj_get_ident` columns of
/// [`style_props!`](crate::style_props).
macro_rules! gen_obj_style_aliases {
    ($( ($c:ident, $s:ident, $set:ident, $oset:ident, $oget:ident, $var:ident, $kind:ident, $t:ty) ),* $(,)?) => {
        impl Ui {
            $(
                #[doc = concat!("`lv_obj_", stringify!($oset), "()` (alias of `obj_", stringify!($oset), "`).")]
                #[inline]
                pub fn $oset(&mut self, obj: Obj, value: $t, selector: StyleSelector) {
                    if !self.obj_is_valid(obj) {
                        return; /*LV_CHECK_OBJ*/
                    }
                    let v = $crate::style_value_pack!($kind, value);
                    self.obj_set_local_style_prop(obj, StyleProp::$c, v, selector);
                }

                #[doc = concat!("`lv_obj_", stringify!($oget), "()` (alias of `obj_", stringify!($oget), "`).")]
                #[inline]
                pub fn $oget(&self, obj: Obj, part: Part) -> obj_style_ret!($kind, $t) {
                    let v = self.obj_get_style_prop(obj, part, StyleProp::$c);
                    obj_style_unpack!($kind, $t, v)
                }
            )*
        }
    };
}

/// The `lv_obj_get_style_<prop>_filtered()` getters of the colour properties:
/// `lv_obj_style_apply_color_filter_internal(obj, part, lv_obj_get_style_prop(obj, part, PROP)).color`.
macro_rules! gen_obj_style_filtered_getters {
    ($( ($c:ident, $name:ident) ),* $(,)?) => {
        impl Ui {
            $(
                #[doc = concat!("`lv_", stringify!($name), "()`")]
                pub fn $name(&self, obj: Obj, part: Part) -> Color {
                    let v = self.obj_style_apply_color_filter(obj, part, self.obj_get_style_prop(obj, part, StyleProp::$c));
                    v.color()
                }
            )*
        }
    };
}

gen_obj_style_accessors! {
    (WIDTH, obj_set_style_width, obj_get_style_width, num, i32),
    (MIN_WIDTH, obj_set_style_min_width, obj_get_style_min_width, num, i32),
    (MAX_WIDTH, obj_set_style_max_width, obj_get_style_max_width, num, i32),
    (HEIGHT, obj_set_style_height, obj_get_style_height, num, i32),
    (MIN_HEIGHT, obj_set_style_min_height, obj_get_style_min_height, num, i32),
    (MAX_HEIGHT, obj_set_style_max_height, obj_get_style_max_height, num, i32),
    (LENGTH, obj_set_style_length, obj_get_style_length, num, i32),
    (X, obj_set_style_x, obj_get_style_x, num, i32),
    (Y, obj_set_style_y, obj_get_style_y, num, i32),
    (ALIGN, obj_set_style_align, obj_get_style_align, enum, crate::misc::area::Align),
    (TRANSFORM_WIDTH, obj_set_style_transform_width, obj_get_style_transform_width, num, i32),
    (TRANSFORM_HEIGHT, obj_set_style_transform_height, obj_get_style_transform_height, num, i32),
    (TRANSLATE_X, obj_set_style_translate_x, obj_get_style_translate_x, num, i32),
    (TRANSLATE_Y, obj_set_style_translate_y, obj_get_style_translate_y, num, i32),
    (TRANSLATE_RADIAL, obj_set_style_translate_radial, obj_get_style_translate_radial, num, i32),
    (TRANSFORM_SCALE_X, obj_set_style_transform_scale_x, obj_get_style_transform_scale_x, num, i32),
    (TRANSFORM_SCALE_Y, obj_set_style_transform_scale_y, obj_get_style_transform_scale_y, num, i32),
    (TRANSFORM_ROTATION, obj_set_style_transform_rotation, obj_get_style_transform_rotation, num, i32),
    (TRANSFORM_PIVOT_X, obj_set_style_transform_pivot_x, obj_get_style_transform_pivot_x, num, i32),
    (TRANSFORM_PIVOT_Y, obj_set_style_transform_pivot_y, obj_get_style_transform_pivot_y, num, i32),
    (TRANSFORM_SKEW_X, obj_set_style_transform_skew_x, obj_get_style_transform_skew_x, num, i32),
    (TRANSFORM_SKEW_Y, obj_set_style_transform_skew_y, obj_get_style_transform_skew_y, num, i32),
    (PAD_TOP, obj_set_style_pad_top, obj_get_style_pad_top, num, i32),
    (PAD_BOTTOM, obj_set_style_pad_bottom, obj_get_style_pad_bottom, num, i32),
    (PAD_LEFT, obj_set_style_pad_left, obj_get_style_pad_left, num, i32),
    (PAD_RIGHT, obj_set_style_pad_right, obj_get_style_pad_right, num, i32),
    (PAD_ROW, obj_set_style_pad_row, obj_get_style_pad_row, num, i32),
    (PAD_COLUMN, obj_set_style_pad_column, obj_get_style_pad_column, num, i32),
    (PAD_RADIAL, obj_set_style_pad_radial, obj_get_style_pad_radial, num, i32),
    (MARGIN_TOP, obj_set_style_margin_top, obj_get_style_margin_top, num, i32),
    (MARGIN_BOTTOM, obj_set_style_margin_bottom, obj_get_style_margin_bottom, num, i32),
    (MARGIN_LEFT, obj_set_style_margin_left, obj_get_style_margin_left, num, i32),
    (MARGIN_RIGHT, obj_set_style_margin_right, obj_get_style_margin_right, num, i32),
    (BG_COLOR, obj_set_style_bg_color, obj_get_style_bg_color, color, Color),
    (BG_OPA, obj_set_style_bg_opa, obj_get_style_bg_opa, cast, Opa),
    (BG_GRAD_COLOR, obj_set_style_bg_grad_color, obj_get_style_bg_grad_color, color, Color),
    (BG_GRAD_DIR, obj_set_style_bg_grad_dir, obj_get_style_bg_grad_dir, enum, crate::draw::grad::GradDir),
    (BG_MAIN_STOP, obj_set_style_bg_main_stop, obj_get_style_bg_main_stop, num, i32),
    (BG_GRAD_STOP, obj_set_style_bg_grad_stop, obj_get_style_bg_grad_stop, num, i32),
    (BG_MAIN_OPA, obj_set_style_bg_main_opa, obj_get_style_bg_main_opa, cast, Opa),
    (BG_GRAD_OPA, obj_set_style_bg_grad_opa, obj_get_style_bg_grad_opa, cast, Opa),
    (BG_GRAD, obj_set_style_bg_grad, obj_get_style_bg_grad, grad, crate::misc::style::GradRef),
    (BG_IMAGE_SRC, obj_set_style_bg_image_src, obj_get_style_bg_image_src, image, crate::misc::style::ImageSrc),
    (BG_IMAGE_OPA, obj_set_style_bg_image_opa, obj_get_style_bg_image_opa, cast, Opa),
    (BG_IMAGE_RECOLOR, obj_set_style_bg_image_recolor, obj_get_style_bg_image_recolor, color, Color),
    (BG_IMAGE_RECOLOR_OPA, obj_set_style_bg_image_recolor_opa, obj_get_style_bg_image_recolor_opa, cast, Opa),
    (BG_IMAGE_TILED, obj_set_style_bg_image_tiled, obj_get_style_bg_image_tiled, bool, bool),
    (BORDER_COLOR, obj_set_style_border_color, obj_get_style_border_color, color, Color),
    (BORDER_OPA, obj_set_style_border_opa, obj_get_style_border_opa, cast, Opa),
    (BORDER_WIDTH, obj_set_style_border_width, obj_get_style_border_width, num, i32),
    (BORDER_SIDE, obj_set_style_border_side, obj_get_style_border_side, flags, BorderSide),
    (BORDER_POST, obj_set_style_border_post, obj_get_style_border_post, bool, bool),
    (OUTLINE_WIDTH, obj_set_style_outline_width, obj_get_style_outline_width, num, i32),
    (OUTLINE_COLOR, obj_set_style_outline_color, obj_get_style_outline_color, color, Color),
    (OUTLINE_OPA, obj_set_style_outline_opa, obj_get_style_outline_opa, cast, Opa),
    (OUTLINE_PAD, obj_set_style_outline_pad, obj_get_style_outline_pad, num, i32),
    (SHADOW_WIDTH, obj_set_style_shadow_width, obj_get_style_shadow_width, num, i32),
    (SHADOW_OFFSET_X, obj_set_style_shadow_offset_x, obj_get_style_shadow_offset_x, num, i32),
    (SHADOW_OFFSET_Y, obj_set_style_shadow_offset_y, obj_get_style_shadow_offset_y, num, i32),
    (SHADOW_SPREAD, obj_set_style_shadow_spread, obj_get_style_shadow_spread, num, i32),
    (SHADOW_COLOR, obj_set_style_shadow_color, obj_get_style_shadow_color, color, Color),
    (SHADOW_OPA, obj_set_style_shadow_opa, obj_get_style_shadow_opa, cast, Opa),
    (IMAGE_OPA, obj_set_style_image_opa, obj_get_style_image_opa, cast, Opa),
    (IMAGE_RECOLOR, obj_set_style_image_recolor, obj_get_style_image_recolor, color, Color),
    (IMAGE_RECOLOR_OPA, obj_set_style_image_recolor_opa, obj_get_style_image_recolor_opa, cast, Opa),
    (IMAGE_COLORKEY, obj_set_style_image_colorkey, obj_get_style_image_colorkey, colorkey, crate::misc::style::ImageColorkey),
    (LINE_WIDTH, obj_set_style_line_width, obj_get_style_line_width, num, i32),
    (LINE_DASH_WIDTH, obj_set_style_line_dash_width, obj_get_style_line_dash_width, num, i32),
    (LINE_DASH_GAP, obj_set_style_line_dash_gap, obj_get_style_line_dash_gap, num, i32),
    (LINE_ROUNDED, obj_set_style_line_rounded, obj_get_style_line_rounded, bool, bool),
    (LINE_COLOR, obj_set_style_line_color, obj_get_style_line_color, color, Color),
    (LINE_OPA, obj_set_style_line_opa, obj_get_style_line_opa, cast, Opa),
    (ARC_WIDTH, obj_set_style_arc_width, obj_get_style_arc_width, num, i32),
    (ARC_ROUNDED, obj_set_style_arc_rounded, obj_get_style_arc_rounded, bool, bool),
    (ARC_COLOR, obj_set_style_arc_color, obj_get_style_arc_color, color, Color),
    (ARC_OPA, obj_set_style_arc_opa, obj_get_style_arc_opa, cast, Opa),
    (ARC_IMAGE_SRC, obj_set_style_arc_image_src, obj_get_style_arc_image_src, image, crate::misc::style::ImageSrc),
    (TEXT_COLOR, obj_set_style_text_color, obj_get_style_text_color, color, Color),
    (TEXT_OPA, obj_set_style_text_opa, obj_get_style_text_opa, cast, Opa),
    (TEXT_FONT, obj_set_style_text_font, obj_get_style_text_font, font, crate::font::FontRef),
    (TEXT_LETTER_SPACE, obj_set_style_text_letter_space, obj_get_style_text_letter_space, num, i32),
    (TEXT_LINE_SPACE, obj_set_style_text_line_space, obj_get_style_text_line_space, num, i32),
    (TEXT_DECOR, obj_set_style_text_decor, obj_get_style_text_decor, flags, crate::misc::text::TextDecor),
    (TEXT_ALIGN, obj_set_style_text_align, obj_get_style_text_align, enum, TextAlign),
    (TEXT_OUTLINE_STROKE_COLOR, obj_set_style_text_outline_stroke_color, obj_get_style_text_outline_stroke_color, color, Color),
    (TEXT_OUTLINE_STROKE_WIDTH, obj_set_style_text_outline_stroke_width, obj_get_style_text_outline_stroke_width, num, i32),
    (TEXT_OUTLINE_STROKE_OPA, obj_set_style_text_outline_stroke_opa, obj_get_style_text_outline_stroke_opa, cast, Opa),
    (TEXT_LEADING_TRIM, obj_set_style_text_leading_trim, obj_get_style_text_leading_trim, enum, crate::misc::text::LeadingTrim),
    (BLUR_RADIUS, obj_set_style_blur_radius, obj_get_style_blur_radius, num, i32),
    (BLUR_BACKDROP, obj_set_style_blur_backdrop, obj_get_style_blur_backdrop, bool, bool),
    (BLUR_QUALITY, obj_set_style_blur_quality, obj_get_style_blur_quality, enum, crate::misc::style::BlurQuality),
    (DROP_SHADOW_RADIUS, obj_set_style_drop_shadow_radius, obj_get_style_drop_shadow_radius, num, i32),
    (DROP_SHADOW_OFFSET_X, obj_set_style_drop_shadow_offset_x, obj_get_style_drop_shadow_offset_x, num, i32),
    (DROP_SHADOW_OFFSET_Y, obj_set_style_drop_shadow_offset_y, obj_get_style_drop_shadow_offset_y, num, i32),
    (DROP_SHADOW_COLOR, obj_set_style_drop_shadow_color, obj_get_style_drop_shadow_color, color, Color),
    (DROP_SHADOW_OPA, obj_set_style_drop_shadow_opa, obj_get_style_drop_shadow_opa, cast, Opa),
    (DROP_SHADOW_QUALITY, obj_set_style_drop_shadow_quality, obj_get_style_drop_shadow_quality, enum, crate::misc::style::BlurQuality),
    (RADIUS, obj_set_style_radius, obj_get_style_radius, num, i32),
    (RADIAL_OFFSET, obj_set_style_radial_offset, obj_get_style_radial_offset, num, i32),
    (CLIP_CORNER, obj_set_style_clip_corner, obj_get_style_clip_corner, bool, bool),
    (OPA, obj_set_style_opa, obj_get_style_opa, cast, Opa),
    (OPA_LAYERED, obj_set_style_opa_layered, obj_get_style_opa_layered, cast, Opa),
    (COLOR_FILTER_DSC, obj_set_style_color_filter_dsc, obj_get_style_color_filter_dsc, color_filter, crate::misc::color::ColorFilter),
    (COLOR_FILTER_OPA, obj_set_style_color_filter_opa, obj_get_style_color_filter_opa, cast, Opa),
    (RECOLOR, obj_set_style_recolor, obj_get_style_recolor, color, Color),
    (RECOLOR_OPA, obj_set_style_recolor_opa, obj_get_style_recolor_opa, cast, Opa),
    (ANIM, obj_set_style_anim, obj_get_style_anim, anim, crate::misc::style::AnimRef),
    (ANIM_DURATION, obj_set_style_anim_duration, obj_get_style_anim_duration, cast, u32),
    (TRANSITION, obj_set_style_transition, obj_get_style_transition, transition, crate::misc::style::TransitionRef),
    (BLEND_MODE, obj_set_style_blend_mode, obj_get_style_blend_mode, enum, BlendMode),
    (LAYOUT, obj_set_style_layout, obj_get_style_layout, cast, u16),
    (BASE_DIR, obj_set_style_base_dir, obj_get_style_base_dir, enum, BaseDir),
    (BITMAP_MASK_SRC, obj_set_style_bitmap_mask_src, obj_get_style_bitmap_mask_src, image, crate::misc::style::ImageSrc),
    (ROTARY_SENSITIVITY, obj_set_style_rotary_sensitivity, obj_get_style_rotary_sensitivity, cast, u32),
    (FLEX_FLOW, obj_set_style_flex_flow, obj_get_style_flex_flow, enum, crate::layouts::flex::FlexFlow),
    (FLEX_MAIN_PLACE, obj_set_style_flex_main_place, obj_get_style_flex_main_place, enum, crate::layouts::flex::FlexAlign),
    (FLEX_CROSS_PLACE, obj_set_style_flex_cross_place, obj_get_style_flex_cross_place, enum, crate::layouts::flex::FlexAlign),
    (FLEX_TRACK_PLACE, obj_set_style_flex_track_place, obj_get_style_flex_track_place, enum, crate::layouts::flex::FlexAlign),
    (FLEX_GROW, obj_set_style_flex_grow, obj_get_style_flex_grow, cast, u8),
    (GRID_COLUMN_DSC_ARRAY, obj_set_style_grid_column_dsc_array, obj_get_style_grid_column_dsc_array, tracks, crate::misc::style::TracksRef),
    (GRID_COLUMN_ALIGN, obj_set_style_grid_column_align, obj_get_style_grid_column_align, enum, crate::layouts::grid::GridAlign),
    (GRID_ROW_DSC_ARRAY, obj_set_style_grid_row_dsc_array, obj_get_style_grid_row_dsc_array, tracks, crate::misc::style::TracksRef),
    (GRID_ROW_ALIGN, obj_set_style_grid_row_align, obj_get_style_grid_row_align, enum, crate::layouts::grid::GridAlign),
    (GRID_CELL_COLUMN_POS, obj_set_style_grid_cell_column_pos, obj_get_style_grid_cell_column_pos, num, i32),
    (GRID_CELL_X_ALIGN, obj_set_style_grid_cell_x_align, obj_get_style_grid_cell_x_align, enum, crate::layouts::grid::GridAlign),
    (GRID_CELL_COLUMN_SPAN, obj_set_style_grid_cell_column_span, obj_get_style_grid_cell_column_span, num, i32),
    (GRID_CELL_ROW_POS, obj_set_style_grid_cell_row_pos, obj_get_style_grid_cell_row_pos, num, i32),
    (GRID_CELL_Y_ALIGN, obj_set_style_grid_cell_y_align, obj_get_style_grid_cell_y_align, enum, crate::layouts::grid::GridAlign),
    (GRID_CELL_ROW_SPAN, obj_set_style_grid_cell_row_span, obj_get_style_grid_cell_row_span, num, i32),
}

crate::style_props!(gen_obj_style_aliases);

gen_obj_style_filtered_getters! {
    (BG_COLOR, obj_get_style_bg_color_filtered),
    (BG_GRAD_COLOR, obj_get_style_bg_grad_color_filtered),
    (BG_IMAGE_RECOLOR, obj_get_style_bg_image_recolor_filtered),
    (BORDER_COLOR, obj_get_style_border_color_filtered),
    (OUTLINE_COLOR, obj_get_style_outline_color_filtered),
    (SHADOW_COLOR, obj_get_style_shadow_color_filtered),
    (IMAGE_RECOLOR, obj_get_style_image_recolor_filtered),
    (LINE_COLOR, obj_get_style_line_color_filtered),
    (ARC_COLOR, obj_get_style_arc_color_filtered),
    (TEXT_COLOR, obj_get_style_text_color_filtered),
    (TEXT_OUTLINE_STROKE_COLOR, obj_get_style_text_outline_stroke_color_filtered),
    (DROP_SHADOW_COLOR, obj_get_style_drop_shadow_color_filtered),
}

/// Properties whose presence in a differing style means a layout change
/// (`lv_obj_style_state_compare()`, in upstream order).
const STATE_CMP_LAYOUT_PROPS: [StyleProp; 16] = [
    StyleProp::PAD_TOP,
    StyleProp::PAD_BOTTOM,
    StyleProp::PAD_LEFT,
    StyleProp::PAD_RIGHT,
    StyleProp::PAD_COLUMN,
    StyleProp::PAD_ROW,
    StyleProp::LAYOUT,
    StyleProp::TRANSLATE_X,
    StyleProp::TRANSLATE_Y,
    StyleProp::WIDTH,
    StyleProp::HEIGHT,
    StyleProp::MIN_WIDTH,
    StyleProp::MAX_WIDTH,
    StyleProp::MIN_HEIGHT,
    StyleProp::MAX_HEIGHT,
    StyleProp::BORDER_WIDTH,
];

/// Properties whose presence in a differing style changes the ext. draw size
/// (`lv_obj_style_state_compare()`, in upstream order).
const STATE_CMP_DRAW_PAD_PROPS: [StyleProp; 14] = [
    StyleProp::TRANSFORM_WIDTH,
    StyleProp::TRANSFORM_HEIGHT,
    StyleProp::TRANSFORM_ROTATION,
    StyleProp::TRANSFORM_SCALE_X,
    StyleProp::TRANSFORM_SCALE_Y,
    StyleProp::OUTLINE_OPA,
    StyleProp::OUTLINE_PAD,
    StyleProp::OUTLINE_WIDTH,
    StyleProp::SHADOW_WIDTH,
    StyleProp::SHADOW_OPA,
    StyleProp::SHADOW_OFFSET_X,
    StyleProp::SHADOW_OFFSET_Y,
    StyleProp::SHADOW_SPREAD,
    StyleProp::LINE_WIDTH,
];

/**********************
 *   GLOBAL FUNCTIONS
 **********************/

impl Ui {
    /// `lv_obj_style_init()`: `lv_ll_init(style_trans_ll_p)`; the list is
    /// already empty in a fresh [`Ui`].
    pub fn obj_style_init(&mut self) {
        self.style_trans.clear();
        self.style_trans_order.clear();
    }

    /// `lv_obj_style_deinit()`: drop the transition list and the custom
    /// property flag table.
    pub fn obj_style_deinit(&mut self) {
        self.style_trans.clear();
        self.style_trans_order.clear();
        self.style_custom_prop_flags = Vec::new();
    }

    /// `lv_style_init()` for a shared style: store it in the context.
    pub fn style_create(&mut self, style: Style) -> StyleId {
        StyleId(self.styles.insert(style))
    }

    /// Delete a shared style (also `lv_obj_remove_style` from every object).
    pub fn style_delete(&mut self, style: StyleId) {
        let users: Vec<Obj> = self
            .objs
            .iter()
            .filter(|(_, od)| {
                od.styles
                    .iter()
                    .any(|os| matches!(os.src, ObjStyleSrc::Shared(id) if id == style))
            })
            .map(|(k, _)| Obj(ObjId(k)))
            .collect();
        for obj in users {
            if self.obj_is_valid(obj) {
                self.remove_style_core(obj, Some(StyleKey::Shared(style)), SELECTOR_ANY, false);
            }
        }
        self.styles.remove(style.0);
    }

    /// Borrow a shared style.
    #[inline]
    pub fn style(&self, style: StyleId) -> Option<&Style> {
        self.styles.get(style.0)
    }

    /// Mutably borrow a shared style. Call
    /// [`obj_report_style_change`](Self::obj_report_style_change) afterwards.
    #[inline]
    pub fn style_mut(&mut self, style: StyleId) -> Option<&mut Style> {
        self.styles.get_mut(style.0)
    }

    /// `lv_obj_add_style()`
    pub fn obj_add_style(&mut self, obj: Obj, style: StyleId, selector: StyleSelector) {
        self.add_style_core(obj, ObjStyleSrc::Shared(style), StyleKey::Shared(style), selector);
    }

    /// `lv_obj_add_style()` with a constant style.
    pub fn obj_add_const_style(&mut self, obj: Obj, style: &'static ConstStyle, selector: StyleSelector) {
        self.add_style_core(obj, ObjStyleSrc::Const(style), StyleKey::Const(style), selector);
    }

    /// `lv_obj_replace_style()`
    pub fn obj_replace_style(
        &mut self,
        obj: Obj,
        old_style: StyleId,
        new_style: StyleId,
        selector: StyleSelector,
    ) -> bool {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return false;
        }
        let (Some(old_view), Some(new_view)) = (self.styles.get(old_style.0), self.styles.get(new_style.0))
        else {
            check_arg_failed("style != NULL");
            return false;
        };
        if old_style == new_style {
            check_arg_failed("old_style != new_style");
            return false;
        }

        let state = selector.state();
        let part = selector.part();

        /*If the draw size changes invalidate the old area first*/
        if (part == Part::MAIN || part == Part::ANY)
            && (self.style_has_flag(StyleView::Dyn(old_view), DRAW_AREA_FLAGS)
                || self.style_has_flag(StyleView::Dyn(new_view), DRAW_AREA_FLAGS))
        {
            let _ = self.obj_invalidate(obj);
        }

        /*Similar to lv_obj_add_style, delete transition*/
        let trans_removed = self.remove_trans_styles(obj, selector.0, StyleProp::ANY, None);

        let mut replaced = false;
        let Some(od) = self.obj_mut(obj) else {
            return false;
        };
        for os in od.styles.iter_mut() {
            let state_act = os.selector.state();
            let part_act = os.selector.part();

            /*Skip local styles and transitions*/
            if os.is_local() || os.is_trans() {
                continue;
            }

            /*Skip non-matching styles*/
            if (state != State::ANY && state_act != state)
                || (part != Part::ANY && part_act != part)
                || !StyleKey::Shared(old_style).matches(&os.src)
            {
                continue;
            }

            *os = ObjStyle {
                src: ObjStyleSrc::Shared(new_style),
                selector,
                is_disabled: false,
                is_theme: false,
            };

            replaced = true;
            /*Don't break and continue replacing other occurrences*/
        }
        if replaced || trans_removed {
            self.full_cache_refresh(obj, part);
            self.obj_refresh_style(obj, part, StyleProp::ANY);
        }
        replaced
    }

    /// `lv_obj_remove_style()`: `None` style removes every style matching `selector`.
    pub fn obj_remove_style(&mut self, obj: Obj, style: Option<StyleId>, selector: StyleSelector) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return;
        }

        self.remove_style_core(obj, style.map(StyleKey::Shared), selector, false);
    }

    /// `lv_obj_remove_style()` with a constant style.
    pub fn obj_remove_const_style(&mut self, obj: Obj, style: &'static ConstStyle, selector: StyleSelector) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return;
        }

        self.remove_style_core(obj, Some(StyleKey::Const(style)), selector, false);
    }

    /// `lv_obj_remove_theme()`
    pub fn obj_remove_theme(&mut self, obj: Obj, selector: StyleSelector) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return;
        }

        self.remove_style_core(obj, None, selector, true);
    }

    /// `lv_obj_remove_style_all()`
    pub fn obj_remove_style_all(&mut self, obj: Obj) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return;
        }

        self.remove_style_core(obj, None, SELECTOR_ANY, false);
    }

    /// `lv_obj_report_style_change()`: `None` = all styles changed.
    pub fn obj_report_style_change(&mut self, style: Option<StyleId>) {
        if !self.style_refresh {
            return;
        }
        let mut d_idx = 0;
        while let Some(&d) = self.display_order.get(d_idx) {
            let mut i = 0;
            while let Some(scr) = self.displays.get(d.0).and_then(|dd| dd.screens.get(i).copied()) {
                self.report_style_change_core(style, Obj(scr));
                i += 1;
            }
            d_idx += 1;
        }
    }

    /// `lv_obj_refresh_style()`
    pub fn obj_refresh_style(&mut self, obj: Obj, part: Part, prop: StyleProp) {
        if !self.obj_is_valid(obj) {
            return; /*LV_CHECK_OBJ*/
        }

        if !self.style_refresh {
            return;
        }

        let _ = self.obj_invalidate(obj);

        let is_layout_refr = self.style_prop_has_flag(prop, STYLE_PROP_FLAG_LAYOUT_UPDATE);
        let is_ext_draw = self.style_prop_has_flag(prop, STYLE_PROP_FLAG_EXT_DRAW_UPDATE);
        let is_inheritable = self.style_prop_has_flag(prop, STYLE_PROP_FLAG_INHERITABLE);
        let is_layer_refr = self.style_prop_has_flag(prop, STYLE_PROP_FLAG_LAYER_UPDATE);

        if is_layout_refr
            && (part == Part::ANY
                || part == Part::MAIN
                || self.obj_get_style_height(obj, Part::MAIN) == coord::SIZE_CONTENT
                || self.obj_get_style_width(obj, Part::MAIN) == coord::SIZE_CONTENT)
        {
            let _ = self.obj_send_event(obj, EventCode::STYLE_CHANGED, EventParam::None);
            if !self.obj_is_valid(obj) {
                return;
            }
            self.obj_mark_layout_as_dirty(obj);
        }
        if (part == Part::ANY || part == Part::MAIN) && (prop == StyleProp::ANY || is_layout_refr) {
            let parent = self.obj(obj).and_then(|od| od.parent).map(Obj);
            if let Some(parent) = parent {
                self.obj_mark_layout_as_dirty(parent);
            }
        }

        /*Cache the layer type*/
        if (part == Part::ANY || part == Part::MAIN) && is_layer_refr {
            self.obj_update_layer_type(obj);
        }

        /*Cache whether the widget has blur or drop shadow*/
        if prop == StyleProp::ANY || prop == StyleProp::BLUR_RADIUS || prop == StyleProp::DROP_SHADOW_OPA {
            self.obj_update_blur_status(obj);
        }

        if prop == StyleProp::ANY || is_ext_draw {
            self.obj_refresh_ext_draw_size(obj);
        }
        if !self.obj_is_valid(obj) {
            return;
        }
        let _ = self.obj_invalidate(obj);

        if (prop == StyleProp::ANY || (is_inheritable && (is_ext_draw || is_layout_refr)))
            && part != Part::SCROLLBAR
        {
            self.refresh_children_style(obj);
        }
    }

    /// `lv_obj_set_style_enabled()`
    pub fn obj_set_style_enabled(&mut self, obj: Obj, style: StyleId, selector: StyleSelector, en: bool) {
        let Some(od) = self.obj(obj) else {
            check_arg_failed("obj != NULL");
            return;
        };

        let key = StyleKey::Shared(style);
        let Some(idx) = find_style(od, key, selector) else {
            return;
        };

        if en != od.styles[idx].is_disabled {
            return; /*Already in the right state*/
        }

        let part = selector.part();

        /*If the draw size changes invalidate the old area first*/
        if (part == Part::MAIN || part == Part::ANY) && self.key_has_flag(obj, key, DRAW_AREA_FLAGS) {
            let _ = self.obj_invalidate(obj);
        }

        let Some(os) = self.obj_mut(obj).and_then(|od| od.styles.get_mut(idx)) else {
            return;
        };
        os.is_disabled = !en;
        self.full_cache_refresh(obj, part);
        self.obj_refresh_style(obj, part, StyleProp::ANY);
    }

    /// `lv_obj_get_style_enabled()`
    pub fn obj_get_style_enabled(&self, obj: Obj, style: StyleId, selector: StyleSelector) -> bool {
        let Some(od) = self.obj(obj) else {
            check_arg_failed("obj != NULL");
            return false;
        };

        let Some(idx) = find_style(od, StyleKey::Shared(style), selector) else {
            return false;
        };

        !od.styles[idx].is_disabled
    }

    /// `lv_obj_style_set_disabled()` (deprecated upstream: use
    /// [`obj_set_style_enabled`](Self::obj_set_style_enabled) with inverted logic).
    pub fn obj_style_set_disabled(&mut self, obj: Obj, style: StyleId, selector: StyleSelector, dis: bool) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return;
        }

        self.obj_set_style_enabled(obj, style, selector, !dis);
    }

    /// `lv_obj_style_get_disabled()` (deprecated upstream: use
    /// [`obj_get_style_enabled`](Self::obj_get_style_enabled) with inverted logic).
    pub fn obj_style_get_disabled(&self, obj: Obj, style: StyleId, selector: StyleSelector) -> bool {
        let Some(od) = self.obj(obj) else {
            check_arg_failed("obj != NULL");
            return false;
        };

        let Some(idx) = find_style(od, StyleKey::Shared(style), selector) else {
            return false;
        };

        od.styles[idx].is_disabled
    }

    /// `lv_obj_enable_style_refresh()`
    pub fn obj_enable_style_refresh(&mut self, en: bool) {
        self.style_refresh = en;
    }

    /// `lv_obj_get_style_prop()`: the resolved value (local, shared, theme,
    /// inherited or default).
    pub fn obj_get_style_prop(&self, obj: Obj, part: Part, prop: StyleProp) -> StyleValue {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return self.style_prop_get_default(prop);
        }
        self.obj_get_style_prop_internal(obj, part, prop).0
    }

    /// `lv_obj_get_style_prop_internal()`: like `obj_get_style_prop` but
    /// also reports whether the value was found.
    pub fn obj_get_style_prop_internal(
        &self,
        obj: Obj,
        part: Part,
        prop: StyleProp,
    ) -> (StyleValue, StyleRes) {
        let Some(od) = self.obj(obj) else {
            return (self.style_prop_get_default(prop), StyleRes::NotFound);
        };
        let selector = StyleSelector(part.0 | od.state.bits() as u32);

        match self.get_selector_style_prop(od, selector, prop) {
            Some(v) => (v, StyleRes::Found),
            None => (self.style_prop_get_default(prop), StyleRes::NotFound),
        }
    }

    /// `lv_obj_style_apply_color_filter()` / `lv_obj_style_apply_color_filter_internal()`.
    ///
    /// `LV_USE_COLOR_FILTER` is 0 in the parity configuration, so the value
    /// is returned unchanged.
    pub fn obj_style_apply_color_filter(&self, obj: Obj, part: Part, v: StyleValue) -> StyleValue {
        let _ = (obj, part);
        v
    }

    /// `lv_obj_has_style_prop()`
    pub fn obj_has_style_prop(&self, obj: Obj, selector: StyleSelector, prop: StyleProp) -> bool {
        let Some(od) = self.obj(obj) else {
            check_arg_failed("obj != NULL");
            return false;
        };

        self.get_selector_style_prop(od, selector, prop).is_some()
    }

    /// `lv_obj_set_local_style_prop()`
    pub fn obj_set_local_style_prop(
        &mut self,
        obj: Obj,
        prop: StyleProp,
        value: StyleValue,
        selector: StyleSelector,
    ) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return;
        }

        /*If the draw size changes invalidate the old area first*/
        if selector.part() == Part::MAIN && self.style_prop_has_flag(prop, DRAW_AREA_FLAGS) {
            let _ = self.obj_invalidate(obj);
        }

        /*Stop running transitions with this property */
        let trans_removed = self.remove_trans_styles(obj, selector.part().0, prop, None);

        let Some(idx) = self.get_local_style(obj, selector) else {
            return;
        };
        let Some(style) = self.obj_mut(obj).and_then(|od| od.styles[idx].owned_mut()) else {
            return;
        };
        style.set_prop(prop, value);

        if trans_removed {
            self.full_cache_refresh(obj, Part::ANY);
        } else if let Some(od) = self.obj_mut(obj) {
            let prop_shifted = style_prop_shifted(prop);
            if selector.part() == Part::MAIN {
                od.style_main_prop_is_set |= prop_shifted;
            } else {
                od.style_other_prop_is_set |= prop_shifted;
            }
        }

        self.obj_refresh_style(obj, Part(selector.0), prop);
    }

    /// `lv_obj_get_local_style_prop()`
    pub fn obj_get_local_style_prop(
        &self,
        obj: Obj,
        prop: StyleProp,
        selector: StyleSelector,
    ) -> Option<StyleValue> {
        let Some(od) = self.obj(obj) else {
            check_arg_failed("obj != NULL");
            return None;
        };

        for os in &od.styles {
            if os.is_local() && os.selector == selector {
                return src_view(&self.styles, &os.src)?.get_prop(prop);
            }
        }

        None
    }

    /// `lv_obj_remove_local_style_prop()`
    pub fn obj_remove_local_style_prop(
        &mut self,
        obj: Obj,
        prop: StyleProp,
        selector: StyleSelector,
    ) -> bool {
        let Some(od) = self.obj_mut(obj) else {
            return false; /*LV_CHECK_OBJ*/
        };

        /*Find the style*/
        let Some(i) = od
            .styles
            .iter()
            .position(|os| os.is_local() && os.selector == selector)
        else {
            /*The style is not found*/
            return false;
        };

        let res = od.styles[i].owned_mut().is_some_and(|s| s.remove_prop(prop));
        if res {
            self.full_cache_refresh(obj, selector.part());
            self.obj_refresh_style(obj, Part(selector.0), prop);
        }

        res
    }

    /// `lv_obj_style_create_transition()`
    pub fn obj_style_create_transition(
        &mut self,
        obj: Obj,
        part: Part,
        prev_state: State,
        new_state: State,
        tr_dsc: &ObjStyleTransitionDsc,
    ) {
        let Some(od) = self.obj_mut(obj) else {
            check_arg_failed("obj != NULL");
            return;
        };

        /*Get the previous and current values*/
        od.skip_trans = true;
        od.state = prev_state;
        let v1 = self.obj_get_style_prop(obj, part, tr_dsc.prop);
        let Some(od) = self.obj_mut(obj) else { return };
        od.state = new_state;
        let v2 = self.obj_get_style_prop(obj, part, tr_dsc.prop);
        let Some(od) = self.obj_mut(obj) else { return };
        od.skip_trans = false;

        if style_value_eq(&v1, &v2) {
            return;
        }
        od.state = prev_state;
        let mut v1 = self.obj_get_style_prop(obj, part, tr_dsc.prop);
        let mut v2 = v2;
        let Some(od) = self.obj_mut(obj) else { return };
        od.state = new_state;

        let Some(idx) = self.get_trans_style(obj, StyleSelector(part.0)) else {
            return;
        };
        /*Be sure `trans_style` has a valid value*/
        if let Some(style) = self.obj_mut(obj).and_then(|od| od.styles[idx].owned_mut()) {
            style.set_prop(tr_dsc.prop, v1.clone());
        }
        self.obj_refresh_style(obj, Part(tr_dsc.selector.0), tr_dsc.prop);
        let Some(od) = self.obj(obj) else { return };

        if tr_dsc.prop == StyleProp::RADIUS && (v1.num() == RADIUS_CIRCLE || v2.num() == RADIUS_CIRCLE) {
            /*lv_obj_get_width() / lv_obj_get_height(): the size of `obj->coords`*/
            let whalf = od.coords.width() / 2;
            let hhalf = od.coords.height() / 2;
            if v1.num() == RADIUS_CIRCLE {
                v1 = StyleValue::Num((whalf + 1).min(hhalf + 1));
            }
            if v2.num() == RADIUS_CIRCLE {
                v2 = StyleValue::Num((whalf + 1).min(hhalf + 1));
            }
        }

        let selector = StyleSelector(part.0);
        let tr = StyleTransId(self.style_trans.insert(StyleTrans {
            obj,
            prop: tr_dsc.prop,
            selector,
            start_value: v1,
            end_value: v2,
        }));
        self.style_trans_order.insert(0, tr);

        let mut a = Anim::new();
        a.set_var(AnimTarget::StyleTrans(tr))
            .set_exec_cb(AnimExec::StyleTrans {
                prop: tr_dsc.prop,
                selector,
            })
            .set_start_cb(Some(Rc::new(|ui: &mut Ui, a: AnimId| {
                ui.obj_style_trans_anim_start_cb(a)
            })))
            .set_completed_cb(Some(Rc::new(|ui: &mut Ui, a: AnimId| {
                ui.obj_style_trans_anim_completed_cb(a)
            })))
            .set_values(0x00, 0xFF)
            .set_duration(tr_dsc.time as u32)
            .set_delay(tr_dsc.delay as u32)
            .set_path_cb(tr_dsc.path)
            .set_early_apply(false)
            .set_user_data(tr_dsc.user_data.clone());
        self.anim_start(&a);
    }

    /// `lv_obj_style_state_compare()`: returns the comparison and the
    /// changed part.
    pub fn obj_style_state_compare(&self, obj: Obj, state1: State, state2: State) -> (StyleStateCmp, Part) {
        let Some(od) = self.obj(obj) else {
            check_arg_failed("obj != NULL");
            return (StyleStateCmp::Same, Part::ANY);
        };

        let mut res = StyleStateCmp::Same;

        /*Accumulate which part the differing styles belong to: the lone differing
         *part, or LV_PART_ANY once styles from more than one part differ. Reported
         *via changed_part so the caller can scope the refresh to a single part.
         *diff_part_found is needed as a separate flag because a style's part can
         *itself be LV_PART_ANY, so LV_PART_ANY can't double as "none seen yet".*/
        let mut diff_part = Part::ANY;
        let mut diff_part_found = false;

        /*Are there any new styles for the new state?*/
        for os in &od.styles {
            if os.is_trans() {
                continue;
            }

            let state_act = os.selector.state().bits();
            /*The style is valid for a state but not the other*/
            let valid1 = state_act & !state1.bits() == 0;
            let valid2 = state_act & !state2.bits() == 0;
            if valid1 != valid2 {
                /*Record this differing style's part: keep it if it's the only one
                 *seen so far, otherwise collapse to LV_PART_ANY (more than one part).*/
                let part_act = os.selector.part();
                if !diff_part_found {
                    diff_part = part_act;
                    diff_part_found = true;
                } else if part_act != diff_part {
                    diff_part = Part::ANY;
                }

                let view = src_view(&self.styles, &os.src);
                let has = |p: StyleProp| view.is_some_and(|v| v.get_prop(p).is_some());
                /*If there is layout difference on the main part, return immediately. There is no more serious difference*/
                let layout_diff = STATE_CMP_LAYOUT_PROPS.iter().any(|&p| has(p));

                if layout_diff {
                    /*Returning early, so the full set of changed parts is unknown:
                     *report LV_PART_ANY rather than a possibly-incomplete part.*/
                    return (StyleStateCmp::DiffLayout, Part::ANY);
                }

                /*Check for draw pad changes*/
                if STATE_CMP_DRAW_PAD_PROPS.iter().any(|&p| has(p)) {
                    res = StyleStateCmp::DiffDrawPad;
                } else if res == StyleStateCmp::Same {
                    res = StyleStateCmp::DiffRedraw;
                }
            }
        }

        (res, diff_part)
    }

    /// `lv_obj_fade_in()`
    pub fn obj_fade_in(&mut self, obj: Obj, time: u32, delay: u32) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return;
        }

        let mut a = Anim::new();
        a.set_var(AnimTarget::Obj(obj))
            .set_values(0, OPA_COVER as i32)
            .set_exec_cb(AnimExec::ObjOpa(StyleSelector(0)))
            .set_completed_cb(Some(Rc::new(fade_in_anim_completed)))
            .set_duration(time)
            .set_delay(delay);
        self.anim_start(&a);
    }

    /// `lv_obj_fade_out()`
    pub fn obj_fade_out(&mut self, obj: Obj, time: u32, delay: u32) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return;
        }

        let mut a = Anim::new();
        a.set_var(AnimTarget::Obj(obj))
            .set_values(self.obj_get_style_opa(obj, Part::MAIN) as i32, OPA_TRANSP as i32)
            .set_exec_cb(AnimExec::ObjOpa(StyleSelector(0)))
            .set_duration(time)
            .set_delay(delay);
        self.anim_start(&a);
    }

    /// `lv_obj_calculate_style_text_align()`
    pub fn obj_calculate_style_text_align(&self, obj: Obj, part: Part, txt: &str) -> TextAlign {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return TextAlign::Auto;
        }
        let mut align = self.obj_get_style_text_align(obj, part);
        let mut base_dir = self.obj_get_style_base_dir(obj, part);
        bidi_calculate_align(&mut align, &mut base_dir, txt);
        align
    }

    /// `lv_obj_get_style_opa_recursive()`
    pub fn obj_get_style_opa_recursive(&self, obj: Obj, part: Part) -> Opa {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return OPA_TRANSP;
        }

        let mut opa_obj = self.obj_get_style_opa(obj, part);
        if opa_obj <= OPA_MIN {
            return OPA_TRANSP;
        }

        let mut opa_final = OPA_COVER;
        if opa_obj < OPA_MAX {
            opa_final = opa_mix2(opa_final, opa_obj);
        }

        let mut part = part;
        let mut cur = Some(obj);
        if part != Part::MAIN {
            part = Part::MAIN;
        } else {
            cur = self.style_parent(obj);
        }

        while let Some(o) = cur {
            opa_obj = self.obj_get_style_opa(o, part);
            if opa_obj <= OPA_MIN {
                return OPA_TRANSP;
            }
            if opa_obj < OPA_MAX {
                opa_final = opa_mix2(opa_final, opa_obj);
            }

            cur = self.style_parent(o);
        }

        if opa_final <= OPA_MIN {
            return OPA_TRANSP;
        }

        if opa_final >= OPA_MAX {
            return OPA_COVER;
        }

        opa_final
    }

    /// `lv_obj_update_layer_type()`
    pub fn obj_update_layer_type(&mut self, obj: Obj) {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return;
        }

        let layer_type = self.calculate_layer_type(obj);
        let Some(od) = self.obj_mut(obj) else { return };
        if let Some(spec) = od.spec.as_mut() {
            spec.layer_type = layer_type;
        } else if layer_type != LayerType::None {
            /*lv_obj_allocate_spec_attr()*/
            let mut spec = Box::new(SpecAttr::new());
            spec.layer_type = layer_type;
            od.spec = Some(spec);
        }
    }

    /// `lv_obj_update_blur_status()`
    pub fn obj_update_blur_status(&mut self, obj: Obj) {
        let Some(od) = self.obj(obj) else { return };

        /*Deletion releases the count itself (see obj_delete_core())*/
        if od.is_deleting {
            return;
        }

        let has_blur = self.calculate_has_blur(od);
        if has_blur == od.has_blur {
            return;
        }

        if let Some(od) = self.obj_mut(obj) {
            od.has_blur = has_blur;
        }
        if has_blur {
            self.blur_obj_cnt += 1;
        } else {
            debug_assert!(self.blur_obj_cnt > 0);
            self.blur_obj_cnt = self.blur_obj_cnt.wrapping_sub(1);
        }
    }

    /// `lv_obj_style_apply_recolor()`
    pub fn obj_style_apply_recolor(&self, obj: Obj, part: Part, color: Color32) -> Color32 {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return Color32::default();
        }

        let mut color = color;
        let opa = self.obj_get_style_recolor_opa(obj, part);
        if opa > OPA_TRANSP {
            let recolor = self.obj_get_style_recolor(obj, part);
            color = Color32::over32(color, recolor.to_32(opa));
        }

        color
    }

    /// `lv_obj_get_style_recolor_recursive()`
    pub fn obj_get_style_recolor_recursive(&self, obj: Obj, part: Part) -> Color32 {
        if !self.obj_is_valid(obj) {
            check_arg_failed("obj != NULL");
            return Color32::default();
        }

        let color = self.obj_get_style_recolor(obj, part);
        let opa = self.obj_get_style_recolor_opa(obj, part);

        let mut result = color.to_32(opa);

        let mut part = part;
        let mut cur = Some(obj);
        if part != Part::MAIN {
            part = Part::MAIN;
        } else {
            cur = self.style_parent(obj);
        }

        while let Some(o) = cur {
            result = self.obj_style_apply_recolor(o, part, result);
            cur = self.style_parent(o);
        }

        result
    }

    /// `lv_obj_set_style_pad_all()`
    pub fn obj_set_style_pad_all(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        if !self.obj_is_valid(obj) {
            return; /*LV_CHECK_OBJ*/
        }
        self.obj_set_style_pad_left(obj, value, selector);
        self.obj_set_style_pad_right(obj, value, selector);
        self.obj_set_style_pad_top(obj, value, selector);
        self.obj_set_style_pad_bottom(obj, value, selector);
    }

    /// `lv_obj_set_style_pad_hor()`
    pub fn obj_set_style_pad_hor(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        if !self.obj_is_valid(obj) {
            return; /*LV_CHECK_OBJ*/
        }
        self.obj_set_style_pad_left(obj, value, selector);
        self.obj_set_style_pad_right(obj, value, selector);
    }

    /// `lv_obj_set_style_pad_ver()`
    pub fn obj_set_style_pad_ver(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        if !self.obj_is_valid(obj) {
            return; /*LV_CHECK_OBJ*/
        }
        self.obj_set_style_pad_top(obj, value, selector);
        self.obj_set_style_pad_bottom(obj, value, selector);
    }

    /// `lv_obj_set_style_margin_all()`
    pub fn obj_set_style_margin_all(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        if !self.obj_is_valid(obj) {
            return; /*LV_CHECK_OBJ*/
        }
        self.obj_set_style_margin_left(obj, value, selector);
        self.obj_set_style_margin_right(obj, value, selector);
        self.obj_set_style_margin_top(obj, value, selector);
        self.obj_set_style_margin_bottom(obj, value, selector);
    }

    /// `lv_obj_set_style_margin_hor()`
    pub fn obj_set_style_margin_hor(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        if !self.obj_is_valid(obj) {
            return; /*LV_CHECK_OBJ*/
        }
        self.obj_set_style_margin_left(obj, value, selector);
        self.obj_set_style_margin_right(obj, value, selector);
    }

    /// `lv_obj_set_style_margin_ver()`
    pub fn obj_set_style_margin_ver(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        if !self.obj_is_valid(obj) {
            return; /*LV_CHECK_OBJ*/
        }
        self.obj_set_style_margin_top(obj, value, selector);
        self.obj_set_style_margin_bottom(obj, value, selector);
    }

    /// `lv_obj_set_style_pad_gap()`
    pub fn obj_set_style_pad_gap(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        if !self.obj_is_valid(obj) {
            return; /*LV_CHECK_OBJ*/
        }
        self.obj_set_style_pad_row(obj, value, selector);
        self.obj_set_style_pad_column(obj, value, selector);
    }

    /// `lv_obj_set_style_size()`
    pub fn obj_set_style_size(&mut self, obj: Obj, width: i32, height: i32, selector: StyleSelector) {
        if !self.obj_is_valid(obj) {
            return; /*LV_CHECK_OBJ*/
        }
        self.obj_set_style_width(obj, width, selector);
        self.obj_set_style_height(obj, height, selector);
    }

    /// `lv_obj_set_style_transform_scale()`
    pub fn obj_set_style_transform_scale(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        if !self.obj_is_valid(obj) {
            return; /*LV_CHECK_OBJ*/
        }
        self.obj_set_style_transform_scale_x(obj, value, selector);
        self.obj_set_style_transform_scale_y(obj, value, selector);
    }

    /// `lv_obj_get_style_space_left()` / `lv_obj_get_style_space_left_internal()`
    pub fn obj_get_style_space_left(&self, obj: Obj, part: Part) -> i32 {
        self.style_space(obj, part, StyleProp::PAD_LEFT, BorderSide::LEFT)
    }

    /// `lv_obj_get_style_space_right()` / `lv_obj_get_style_space_right_internal()`
    pub fn obj_get_style_space_right(&self, obj: Obj, part: Part) -> i32 {
        self.style_space(obj, part, StyleProp::PAD_RIGHT, BorderSide::RIGHT)
    }

    /// `lv_obj_get_style_space_top()` / `lv_obj_get_style_space_top_internal()`
    pub fn obj_get_style_space_top(&self, obj: Obj, part: Part) -> i32 {
        self.style_space(obj, part, StyleProp::PAD_TOP, BorderSide::TOP)
    }

    /// `lv_obj_get_style_space_bottom()` / `lv_obj_get_style_space_bottom_internal()`
    pub fn obj_get_style_space_bottom(&self, obj: Obj, part: Part) -> i32 {
        self.style_space(obj, part, StyleProp::PAD_BOTTOM, BorderSide::BOTTOM)
    }

    /// `lv_obj_get_style_transform_scale_x_safe()`
    pub fn obj_get_style_transform_scale_x_safe(&self, obj: Obj, part: Part) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0; /*LV_CHECK_OBJ*/
        }
        let scale = self.obj_get_style_transform_scale_x(obj, part);
        if scale > 0 { scale } else { 1 }
    }

    /// `lv_obj_get_style_transform_scale_y_safe()`
    pub fn obj_get_style_transform_scale_y_safe(&self, obj: Obj, part: Part) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0; /*LV_CHECK_OBJ*/
        }
        let scale = self.obj_get_style_transform_scale_y(obj, part);
        if scale > 0 { scale } else { 1 }
    }
}

/**********************
 *   STATIC FUNCTIONS
 **********************/

/// `find_style()`: the index of the entry with `style` and `selector`.
fn find_style(od: &ObjData, style: StyleKey, selector: StyleSelector) -> Option<usize> {
    let found = od
        .styles
        .iter()
        .position(|os| style.matches(&os.src) && os.selector == selector);
    if found.is_none() {
        #[cfg(feature = "log")]
        log::warn!(
            "{:?} style was not found on the widget with {:06x} selector",
            style,
            selector.0
        );
    }
    found
}

/// `fade_in_anim_completed()`
fn fade_in_anim_completed(ui: &mut Ui, a: AnimId) {
    if let Some(AnimTarget::Obj(obj)) = ui.anim(a).map(|a| a.var) {
        ui.obj_remove_local_style_prop(obj, StyleProp::OPA, StyleSelector(0));
    }
}

impl Ui {
    /// `lv_style_prop_has_flag()` including custom properties.
    #[inline]
    fn style_prop_has_flag(&self, prop: StyleProp, flag: u8) -> bool {
        prop_has_flag_with_custom(prop, flag, &self.style_custom_prop_flags)
    }

    /// `lv_style_prop_get_default()` with the context's `LV_FONT_DEFAULT`.
    #[inline]
    fn style_prop_get_default(&self, prop: StyleProp) -> StyleValue {
        Style::prop_get_default(prop, self.config().font_default.as_ref())
    }

    /// `obj->parent`
    #[inline]
    fn style_parent(&self, obj: Obj) -> Option<Obj> {
        self.obj(obj).and_then(|od| od.parent).map(Obj)
    }

    /// `obj->spec_attr->children[i]` (`None` past `lv_obj_get_child_count()`).
    #[inline]
    fn style_child(&self, obj: Obj, i: usize) -> Option<Obj> {
        self.obj(obj)
            .and_then(|od| od.spec.as_ref())
            .and_then(|spec| spec.children.get(i).copied())
            .map(Obj)
    }

    /// `lv_obj_get_child_count()`
    #[inline]
    fn style_child_count(&self, obj: Obj) -> usize {
        self.obj(obj)
            .and_then(|od| od.spec.as_ref())
            .map_or(0, |spec| spec.children.len())
    }

    /// `style_has_flag()`: any stored property of `style` has `flag`.
    fn style_has_flag(&self, style: StyleView<'_>, flag: u8) -> bool {
        style.any_prop(|p| self.style_prop_has_flag(p, flag))
    }

    /// The style a [`StyleKey`] refers to (searching `od` for owned styles).
    fn key_view<'a>(&'a self, od: &'a ObjData, key: StyleKey) -> Option<StyleView<'a>> {
        match key {
            StyleKey::Shared(id) => self.styles.get(id.0).map(StyleView::Dyn),
            StyleKey::Const(c) => Some(StyleView::Const(c)),
            StyleKey::Owned(_) => od
                .styles
                .iter()
                .find(|os| key.matches(&os.src))
                .and_then(|os| src_view(&self.styles, &os.src)),
        }
    }

    /// `style_has_flag()` for the style behind `key`.
    fn key_has_flag(&self, obj: Obj, key: StyleKey, flag: u8) -> bool {
        self.obj(obj)
            .and_then(|od| self.key_view(od, key))
            .is_some_and(|v| self.style_has_flag(v, flag))
    }

    /// `lv_obj_get_style_space_*_internal()`
    fn style_space(&self, obj: Obj, part: Part, pad_prop: StyleProp, side: BorderSide) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0; /*LV_CHECK_OBJ*/
        }
        let padding = self.obj_get_style_prop(obj, part, pad_prop).num();
        let border_width = self.obj_get_style_border_width(obj, part);
        let border_side = self.obj_get_style_border_side(obj, part);
        if border_side.intersects(side) {
            padding + border_width
        } else {
            padding
        }
    }

    /// The body of `lv_obj_add_style()`.
    fn add_style_core(&mut self, obj: Obj, src: ObjStyleSrc, key: StyleKey, selector: StyleSelector) {
        let Some(od) = self.obj(obj) else {
            check_arg_failed("obj != NULL");
            return;
        };
        let Some(view) = self.key_view(od, key) else {
            check_arg_failed("style != NULL");
            return;
        };
        if od.styles.len() >= 63 {
            check_arg_failed("obj->style_cnt is restricted to 6 bits, so we can't store more than 63 styles");
            return;
        }

        let part = selector.part();

        /*If the draw size changes invalidate the old area first*/
        if part == Part::MAIN && self.style_has_flag(view, DRAW_AREA_FLAGS) {
            let _ = self.obj_invalidate(obj);
        }

        let trans_removed = self.remove_trans_styles(obj, selector.0, StyleProp::ANY, None);

        /*Try removing the style first to be sure it won't be added twice*/
        self.remove_style_core(obj, Some(key), selector, false);

        let Some(od) = self.obj_mut(obj) else { return };
        /*Go after the transition and local styles*/
        let i = od
            .styles
            .iter()
            .position(|os| !os.is_trans() && !os.is_local())
            .unwrap_or(od.styles.len());

        /*Now `i` is at the first normal style. Insert the new style before this*/
        od.styles.insert(
            i,
            ObjStyle {
                src,
                selector,
                is_disabled: false,
                is_theme: false,
            },
        );

        if trans_removed {
            self.full_cache_refresh(obj, Part::ANY);
        } else {
            /*LV_OBJ_STYLE_CACHE*/
            let bits = self
                .obj(obj)
                .and_then(|od| src_view(&self.styles, &od.styles[i].src))
                .map_or(0, StyleView::prop_bits);
            if let Some(od) = self.obj_mut(obj) {
                if part == Part::MAIN {
                    od.style_main_prop_is_set |= bits;
                } else {
                    od.style_other_prop_is_set |= bits;
                }
            }
        }

        self.obj_refresh_style(obj, Part(selector.0), StyleProp::ANY);
    }

    /// `get_local_style()`: the index of the local style of `selector`,
    /// creating it (as the last local style) if it doesn't exist yet.
    fn get_local_style(&mut self, obj: Obj, selector: StyleSelector) -> Option<usize> {
        let od = self.obj_mut(obj)?;
        if let Some(i) = od
            .styles
            .iter()
            .position(|os| os.is_local() && os.selector == selector)
        {
            return Some(i);
        }

        let mut i = od.styles.len();
        while i > 0 {
            /*Copy only normal styles (not local and transition).
             *The new local style will be added as the last local style*/
            let prev = &od.styles[i - 1];
            if prev.is_local() || prev.is_trans() {
                break;
            }
            i -= 1;
        }

        od.styles.insert(
            i,
            ObjStyle {
                src: ObjStyleSrc::Local(Box::new(Style::new())),
                selector,
                is_disabled: false,
                is_theme: false,
            },
        );
        Some(i)
    }

    /// `get_trans_style()`: the index of the transition style of `selector`,
    /// creating it at the front of the list if it doesn't exist yet.
    fn get_trans_style(&mut self, obj: Obj, selector: StyleSelector) -> Option<usize> {
        let od = self.obj_mut(obj)?;
        /*Already have a transition style for it*/
        if let Some(i) = od
            .styles
            .iter()
            .position(|os| os.is_trans() && os.selector == selector)
        {
            return Some(i);
        }

        od.styles.insert(
            0,
            ObjStyle {
                src: ObjStyleSrc::Trans(Box::new(Style::new())),
                selector,
                is_disabled: false,
                is_theme: false,
            },
        );
        Some(0)
    }

    /// `get_prop_core()`: transition styles first, then the best matching
    /// state (highest state value wins, first style wins ties).
    fn get_prop_core(&self, od: &ObjData, selector: StyleSelector, prop: StyleProp) -> Option<StyleValue> {
        let group = 1u32 << get_prop_group(prop.0);
        let part = selector.part();
        let skip_trans = od.skip_trans;
        let mut weight: i32 = -1;
        let mut value: Option<StyleValue> = None;
        let styles = &od.styles;
        let mut i = 0;
        /*Handle the transition styles first. They are at the beginning of the style list
         *as the have the highest precedence. It's a different for loop as in case of transitions
         *the state doesn't need to be checked. */
        while i < styles.len() {
            let obj_style = &styles[i];
            if !obj_style.is_trans() {
                break;
            }
            i += 1;
            if skip_trans {
                continue;
            }

            let part_act = obj_style.selector.part();

            if part_act != part {
                continue;
            }
            let Some(view) = src_view(&self.styles, &obj_style.src) else {
                continue;
            };
            if view.has_group() & group == 0 {
                continue;
            }
            if let Some(v) = view.get_prop(prop) {
                return Some(v);
            }
        }

        /*Check the normal local and global styles that are assigned to given state*/
        let state = selector.state().bits();
        let state_inv = !state;
        while i < styles.len() {
            let obj_style = &styles[i];
            i += 1;
            let Some(view) = src_view(&self.styles, &obj_style.src) else {
                continue;
            };
            if view.has_group() & group == 0 {
                continue;
            }
            if obj_style.is_disabled {
                continue;
            }
            let part_act = obj_style.selector.part();
            if part_act != part {
                continue;
            }

            /*Be sure the style not specifies other state than the requested.
             *E.g. For HOVER+PRESS object state, HOVER style only is OK, but HOVER+FOCUS style is not*/
            let state_act = obj_style.selector.state().bits();
            if state_act & state_inv != 0 {
                continue;
            }

            /*Check only better candidates*/
            if (state_act as i32) <= weight {
                continue;
            }

            if let Some(v) = view.get_prop(prop) {
                if state_act == state {
                    return Some(v);
                }
                weight = state_act as i32;
                value = Some(v);
            }
        }

        if weight >= 0 { value } else { None }
    }

    /// `report_style_change_core()`: refresh `obj` if it uses `style` (or
    /// any style for `None`), then its children recursively.
    fn report_style_change_core(&mut self, style: Option<StyleId>, obj: Obj) {
        let Some(od) = self.obj(obj) else { return };
        let hit = od
            .styles
            .iter()
            .find(|os| style.is_none_or(|s| StyleKey::Shared(s).matches(&os.src)))
            .map(|os| os.selector.part());
        if let Some(part) = hit {
            self.full_cache_refresh(obj, part);
            self.obj_refresh_style(obj, Part::ANY, StyleProp::ANY);
        }

        let child_cnt = self.style_child_count(obj);
        for i in 0..child_cnt {
            let Some(child) = self.style_child(obj, i) else {
                break;
            };
            self.report_style_change_core(style, child);
        }
    }

    /// `refresh_children_style()`: recursively refresh the style of the
    /// children (the NULL styles are inherited from the parent).
    fn refresh_children_style(&mut self, obj: Obj) {
        let child_cnt = self.style_child_count(obj);
        for i in 0..child_cnt {
            let Some(child) = self.style_child(obj, i) else {
                break;
            };
            let _ = self.obj_invalidate(child);
            let _ = self.obj_send_event(child, EventCode::STYLE_CHANGED, EventParam::None);
            if !self.obj_is_valid(child) {
                continue;
            }
            let _ = self.obj_invalidate(child);

            self.refresh_children_style(child); /*Check children too*/
        }
    }

    /// `remove_trans_styles()`: remove the transitions of `obj`'s `part`
    /// (`LV_PART_ANY` = all parts; compared with the raw value like C) and
    /// `prop` (`LV_STYLE_PROP_ANY` = all), walking from the tail and stopping
    /// at `tr_limit`. Returns `true` if at least one was removed.
    ///
    /// It doesn't update the cache, `full_cache_refresh` needs to be called manually.
    fn remove_trans_styles(
        &mut self,
        obj: Obj,
        part: u32,
        prop: StyleProp,
        tr_limit: Option<StyleTransId>,
    ) -> bool {
        let mut removed = false;
        let mut cur = self.style_trans_order.last().copied();
        while let Some(tr) = cur {
            if Some(tr) == tr_limit {
                break;
            }

            /*'tr' might be deleted, so get the next object while 'tr' is valid*/
            let tr_prev = self.style_trans_prev(tr);

            let info = self.style_trans.get(tr.0).map(|t| (t.obj, t.selector, t.prop));
            if let Some((t_obj, t_selector, t_prop)) = info
                && t_obj == obj
                && (part == t_selector.0 || part == Part::ANY.0)
                && (prop == t_prop || prop == StyleProp::ANY)
            {
                /*Dropping the transitioned value changes the drawn area right away,
                 *so invalidate the old area first*/
                if t_selector.part() == Part::MAIN && self.style_prop_has_flag(t_prop, DRAW_AREA_FLAGS) {
                    let _ = self.obj_invalidate(obj);
                }

                /*Remove any transitioned properties from the trans. style
                 *to allow changing it by normal styles*/
                if let Some(od) = self.obj_mut(obj) {
                    for os in od.styles.iter_mut() {
                        if os.is_trans()
                            && (part == Part::ANY.0 || os.selector.0 == part)
                            && let Some(s) = os.owned_mut()
                        {
                            s.remove_prop(t_prop);
                        }
                    }
                }

                /*Free the transition descriptor too*/
                self.anim_delete(AnimTarget::StyleTrans(tr), None);
                self.style_trans_order.retain(|&t| t != tr);
                self.style_trans.remove(tr.0);
                removed = true;
            }
            cur = tr_prev;
        }

        removed
    }

    /// `lv_ll_get_prev(style_trans_ll_p, tr)`
    fn style_trans_prev(&self, tr: StyleTransId) -> Option<StyleTransId> {
        let pos = self.style_trans_order.iter().position(|&t| t == tr)?;
        pos.checked_sub(1).map(|p| self.style_trans_order[p])
    }

    /// `trans_anim_cb()`: the exec callback of transition animations
    /// ([`AnimExec::StyleTrans`]). Writes the value of the transition at
    /// `v` (0..=255) into the object's transition style.
    ///
    /// Public (hidden) only so it is not dead code until `misc/anim.rs`
    /// switches its `AnimExec::StyleTrans` dispatch to it.
    #[doc(hidden)]
    pub fn obj_style_trans_anim_cb(&mut self, tr: StyleTransId, v: i32) {
        let Some(t) = self.style_trans.get(tr.0) else {
            return;
        };
        let obj = t.obj;
        let prop = t.prop;
        let selector = t.selector;

        /*Find the transition style of of the descriptor*/
        let Some(od) = self.obj(obj) else { return };
        let Some(i) = od
            .styles
            .iter()
            .position(|os| os.is_trans() && os.selector == selector)
        else {
            return;
        };

        let Some(t) = self.style_trans.get(tr.0) else {
            return;
        };
        let start_value = &t.start_value;
        let end_value = &t.end_value;
        let value_final = match prop {
            StyleProp::BORDER_SIDE | StyleProp::BORDER_POST | StyleProp::BLEND_MODE => {
                if v < 255 {
                    StyleValue::Num(start_value.num())
                } else {
                    StyleValue::Num(end_value.num())
                }
            }
            StyleProp::TRANSITION | StyleProp::TEXT_FONT => {
                if v < 255 {
                    start_value.clone()
                } else {
                    end_value.clone()
                }
            }
            StyleProp::COLOR_FILTER_DSC => {
                if start_value.is_null() {
                    end_value.clone()
                } else if end_value.is_null() || v < 128 {
                    start_value.clone()
                } else {
                    end_value.clone()
                }
            }
            StyleProp::RECOLOR
            | StyleProp::BG_COLOR
            | StyleProp::BG_GRAD_COLOR
            | StyleProp::BORDER_COLOR
            | StyleProp::TEXT_COLOR
            | StyleProp::SHADOW_COLOR
            | StyleProp::OUTLINE_COLOR
            | StyleProp::IMAGE_RECOLOR => {
                if v <= 0 {
                    StyleValue::Color(start_value.color())
                } else if v >= 255 {
                    StyleValue::Color(end_value.color())
                } else {
                    StyleValue::Color(Color::mix(end_value.color(), start_value.color(), v as u8))
                }
            }
            _ => {
                if v == 0 {
                    start_value.clone()
                } else if v == 255 {
                    end_value.clone()
                } else {
                    let s = start_value.num();
                    let e = end_value.num();
                    StyleValue::Num(s.wrapping_add(e.wrapping_sub(s).wrapping_mul(v) >> 8))
                }
            }
        };

        /*Don't do anything is the prop didn't changed*/
        let old_value = src_view(&self.styles, &od.styles[i].src).and_then(|s| s.get_prop(prop));
        if let Some(old_value) = old_value
            && style_value_eq(&value_final, &old_value)
        {
            return;
        }

        /*If the draw size changes invalidate the old area first*/
        let part = selector.part();
        if part == Part::MAIN && self.style_prop_has_flag(prop, DRAW_AREA_FLAGS) {
            let _ = self.obj_invalidate(obj);
        }
        let Some(od) = self.obj_mut(obj) else { return };
        let Some(style) = od.styles.get_mut(i).and_then(ObjStyle::owned_mut) else {
            return;
        };
        style.set_prop(prop, value_final);

        /*LV_OBJ_STYLE_CACHE*/
        let prop_shifted = style_prop_shifted(prop);
        if part == Part::MAIN {
            od.style_main_prop_is_set |= prop_shifted;
        } else {
            od.style_other_prop_is_set |= prop_shifted;
        }

        self.obj_refresh_style(obj, Part(selector.0), prop);
    }

    /// `trans_anim_start_cb()`
    pub(crate) fn obj_style_trans_anim_start_cb(&mut self, a: AnimId) {
        let Some(AnimTarget::StyleTrans(tr)) = self.anim(a).map(|a| a.var) else {
            return;
        };
        let Some(t) = self.style_trans.get(tr.0) else {
            return;
        };
        let obj = t.obj;
        let selector = t.selector;
        let prop_tmp = t.prop;
        if !self.obj_is_valid(obj) {
            return;
        }

        let part = selector.part();
        let start_value = self.obj_get_style_prop(obj, part, prop_tmp);

        /*Init prop to an invalid values to be sure `trans_del` won't delete this added `tr`*/
        if let Some(t) = self.style_trans.get_mut(tr.0) {
            t.start_value = start_value.clone();
            t.prop = StyleProp::INV;
        }

        /*Delete the related transitions if any*/
        let trans_removed = self.remove_trans_styles(obj, part.0, prop_tmp, Some(tr));

        if let Some(t) = self.style_trans.get_mut(tr.0) {
            t.prop = prop_tmp;
        }

        let Some(idx) = self.get_trans_style(obj, selector) else {
            return;
        };
        /*Be sure `trans_style` has a valid value*/
        if let Some(style) = self.obj_mut(obj).and_then(|od| od.styles[idx].owned_mut()) {
            style.set_prop(prop_tmp, start_value);
        }

        /*LV_OBJ_STYLE_CACHE*/
        if trans_removed {
            self.full_cache_refresh(obj, part);
        } else if let Some(od) = self.obj_mut(obj) {
            let prop_shifted = style_prop_shifted(prop_tmp);
            if part == Part::MAIN {
                od.style_main_prop_is_set |= prop_shifted;
            } else {
                od.style_other_prop_is_set |= prop_shifted;
            }
        }

        self.obj_refresh_style(obj, Part(selector.0), prop_tmp);
    }

    /// `trans_anim_completed_cb()`
    pub(crate) fn obj_style_trans_anim_completed_cb(&mut self, a: AnimId) {
        let Some(AnimTarget::StyleTrans(tr)) = self.anim(a).map(|a| a.var) else {
            return;
        };
        let Some(t) = self.style_trans.get(tr.0) else {
            return;
        };
        let obj = t.obj;
        let prop = t.prop;
        let selector = t.selector;

        /*Remove the transitioned property from trans. style
         *if there no more transitions for this property
         *It allows changing it by normal styles*/
        let running = self.style_trans_order.iter().any(|&tr_i| {
            tr_i != tr
                && self
                    .style_trans
                    .get(tr_i.0)
                    .is_some_and(|t| t.obj == obj && t.selector == selector && t.prop == prop)
        });

        if !running {
            let Some(od) = self.obj(obj) else { return };
            let Some(i) = od
                .styles
                .iter()
                .position(|os| os.is_trans() && os.selector == selector)
            else {
                return;
            };
            self.style_trans_order.retain(|&t| t != tr);
            self.style_trans.remove(tr.0);

            let Some(od) = self.obj_mut(obj) else { return };
            let obj_style = &mut od.styles[i];
            let obj_style_selector = obj_style.selector;
            let Some(style) = obj_style.owned_mut() else {
                return;
            };
            style.remove_prop(prop);

            if style.is_empty() {
                let key = StyleKey::Owned(&*style as *const Style);
                self.remove_style_core(obj, Some(key), obj_style_selector, false);
            }
        }
    }

    /// `calculate_layer_type()`
    fn calculate_layer_type(&self, obj: Obj) -> LayerType {
        if self.obj_get_style_transform_rotation(obj, Part::MAIN) != 0 {
            return LayerType::Transform;
        }
        if self.obj_get_style_transform_scale_x(obj, Part::MAIN) != 256 {
            return LayerType::Transform;
        }
        if self.obj_get_style_transform_scale_y(obj, Part::MAIN) != 256 {
            return LayerType::Transform;
        }
        if self.obj_get_style_transform_skew_x(obj, Part::MAIN) != 0 {
            return LayerType::Transform;
        }
        if self.obj_get_style_transform_skew_y(obj, Part::MAIN) != 0 {
            return LayerType::Transform;
        }
        if self.obj_get_style_opa_layered(obj, Part::MAIN) != OPA_COVER {
            return LayerType::Simple;
        }
        if !self.obj_get_style_bitmap_mask_src(obj, Part::MAIN).is_none() {
            return LayerType::Simple;
        }
        if self.obj_get_style_blend_mode(obj, Part::MAIN) != BlendMode::Normal {
            return LayerType::Simple;
        }
        LayerType::None
    }

    /// `calculate_has_blur()`: true if any style attached to the widget
    /// enables blur or a drop shadow in the widget's current state. The
    /// per-style `has_group` bitmask makes the common no-blur case a cheap scan.
    fn calculate_has_blur(&self, od: &ObjData) -> bool {
        let group_blur = 1u32 << get_prop_group(StyleProp::BLUR_RADIUS.0);
        let group_dropshadow = 1u32 << get_prop_group(StyleProp::DROP_SHADOW_OPA.0);
        let state = od.state.bits();
        let state_inv = !state;
        for obj_style in &od.styles {
            if obj_style.is_disabled {
                continue;
            }

            let state_style = obj_style.selector.state().bits();
            if state_style & state_inv != 0 {
                continue;
            }

            let Some(view) = src_view(&self.styles, &obj_style.src) else {
                continue;
            };
            if view.has_group() & group_blur != 0
                && let Some(v) = view.get_prop(StyleProp::BLUR_RADIUS)
                && v.num() > 0
            {
                return true;
            }
            if view.has_group() & group_dropshadow != 0
                && let Some(v) = view.get_prop(StyleProp::DROP_SHADOW_OPA)
                && v.num() > 0
            {
                return true;
            }
        }

        false
    }

    /// `full_cache_refresh()`: recompute `style_main_prop_is_set` and/or
    /// `style_other_prop_is_set` (`LV_OBJ_STYLE_CACHE`).
    fn full_cache_refresh(&mut self, obj: Obj, part: Part) {
        let Some(od) = self.obj(obj) else { return };
        let mut main = od.style_main_prop_is_set;
        let mut other = od.style_other_prop_is_set;
        if part == Part::MAIN || part == Part::ANY {
            main = 0;
            for os in &od.styles {
                if os.selector.part() != Part::MAIN {
                    continue;
                }
                if os.is_disabled {
                    continue;
                }
                if let Some(view) = src_view(&self.styles, &os.src) {
                    main |= view.prop_bits();
                }
            }
        }
        #[allow(clippy::nonminimal_bool)] // the upstream condition, kept verbatim
        if part != Part::MAIN || part == Part::ANY {
            other = 0;
            for os in &od.styles {
                if os.selector.part() == Part::MAIN {
                    continue;
                }
                if os.is_disabled {
                    continue;
                }
                if let Some(view) = src_view(&self.styles, &os.src) {
                    other |= view.prop_bits();
                }
            }
        }
        if let Some(od) = self.obj_mut(obj) {
            od.style_main_prop_is_set = main;
            od.style_other_prop_is_set = other;
        }
    }

    /// `get_selector_style_prop()`: the cached presence check, `get_prop_core`,
    /// then inheritance (the parent's MAIN part) or the class default size.
    fn get_selector_style_prop(
        &self,
        od: &ObjData,
        selector: StyleSelector,
        prop: StyleProp,
    ) -> Option<StyleValue> {
        let mut part = selector.part();

        /*The happy path*/
        let prop_shifted = style_prop_shifted(prop);
        let prop_is_set = if part == Part::MAIN {
            od.style_main_prop_is_set
        } else {
            od.style_other_prop_is_set
        };
        if prop_is_set & prop_shifted != 0
            && let Some(v) = self.get_prop_core(od, selector, prop)
        {
            return Some(v);
        }

        let inheritable = if prop.0 < StyleProp::NUM_BUILT_IN {
            BUILTIN_PROP_FLAG_LOOKUP_TABLE[prop.0 as usize] & STYLE_PROP_FLAG_INHERITABLE != 0
        } else {
            self.style_custom_prop_flags
                .get((prop.0 - StyleProp::NUM_BUILT_IN) as usize)
                .is_some_and(|f| f & STYLE_PROP_FLAG_INHERITABLE != 0)
        };

        if inheritable {
            /*If not found, check the `MAIN` style first, if already on the MAIN part go to the parent*/
            let mut cur: Option<&ObjData> = Some(od);
            if part != Part::MAIN {
                part = Part::MAIN;
            } else {
                cur = od.parent.and_then(|p| self.objs.get(p.0));
            }

            while let Some(o) = cur {
                if o.style_main_prop_is_set & prop_shifted != 0 {
                    let selector = StyleSelector(part.0 | o.state.bits() as u32);
                    if let Some(v) = self.get_prop_core(o, selector, prop) {
                        return Some(v);
                    }
                }
                /*Check the parent too.*/
                cur = o.parent.and_then(|p| self.objs.get(p.0));
            }
        } else if part == Part::MAIN && (prop == StyleProp::WIDTH || prop == StyleProp::HEIGHT) {
            /*Get the width and height from the class.
             * WIDTH and HEIGHT are not inherited so add them in the `else` to skip checking them for inherited properties */
            let mut cls = Some(od.class);
            while let Some(c) = cls {
                if prop == StyleProp::WIDTH {
                    if c.width_def != 0 {
                        return Some(StyleValue::Num(c.width_def));
                    }
                } else if c.height_def != 0 {
                    return Some(StyleValue::Num(c.height_def));
                }
                cls = c.base;
            }
        }

        None
    }

    /// `remove_style_core()`: remove the entries matching `style` (`None` =
    /// any), `selector` (`LV_PART_ANY` / `LV_STATE_ANY` wildcards) and, with
    /// `theme_only`, only the ones added by a theme.
    fn remove_style_core(
        &mut self,
        obj: Obj,
        style: Option<StyleKey>,
        selector: StyleSelector,
        theme_only: bool,
    ) {
        let state = selector.state();
        let part = selector.part();

        /*If the draw size changes invalidate the old area first.
         *`style == NULL` means "any style" (e.g. `lv_obj_remove_style_all()`) so it can't be checked for flags.*/
        if (part == Part::MAIN || part == Part::ANY)
            && style.is_none_or(|k| self.key_has_flag(obj, k, DRAW_AREA_FLAGS))
        {
            let _ = self.obj_invalidate(obj);
        }

        let Some(od) = self.obj(obj) else { return };
        let remove: Vec<bool> = od
            .styles
            .iter()
            .map(|os| {
                let state_act = os.selector.state();
                let part_act = os.selector.part();

                if theme_only && !os.is_theme {
                    false
                } else {
                    !((state != State::ANY && state_act != state)
                        || (part != Part::ANY && part_act != part)
                        || style.is_some_and(|k| !k.matches(&os.src)))
                }
            })
            .collect();

        if !remove.iter().any(|&r| r) {
            return;
        }

        /*These entries are being removed*/
        for (i, &r) in remove.iter().enumerate() {
            if !r {
                continue;
            }
            let is_trans = self
                .obj(obj)
                .and_then(|od| od.styles.get(i))
                .is_some_and(ObjStyle::is_trans);
            if is_trans {
                self.remove_trans_styles(obj, part.0, StyleProp::ANY, None);
            }
        }

        /*Keep the other entries in order; local and transition styles are freed*/
        let Some(od) = self.obj_mut(obj) else { return };
        let mut i = 0;
        od.styles.retain(|_| {
            let keep = !remove.get(i).copied().unwrap_or(false);
            i += 1;
            keep
        });

        self.full_cache_refresh(obj, part);
        self.obj_refresh_style(obj, part, StyleProp::ANY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::UiConfig;
    use crate::core::obj::{OBJ_CLASS, ObjFlags};
    use crate::misc::area::Area;
    use crate::misc::style::{ConstStyleEntry, ImageSrc};
    use alloc::vec;

    fn ui() -> Ui {
        let mut ui = Ui::new(UiConfig::default());
        // `lv_obj_refresh_style()` invalidates and sends events (other files);
        // the tests only exercise the style bookkeeping.
        ui.obj_enable_style_refresh(false);
        ui
    }

    /// A bare `lv_obj` inserted straight into the arena (`lv_obj_create` lives
    /// in another file).
    fn new_obj(ui: &mut Ui, parent: Option<Obj>) -> Obj {
        let data = ObjData {
            class: &OBJ_CLASS,
            parent: parent.map(|p| p.0),
            spec: None,
            styles: Vec::new(),
            style_main_prop_is_set: 0,
            style_other_prop_is_set: 0,
            user_data: None,
            coords: Area::new(0, 0, 99, 49),
            flags: ObjFlags::empty(),
            radio_button: false,
            state: State::DEFAULT,
            layout_inv: false,
            readjust_scroll_after_layout: false,
            scr_layout_inv: false,
            skip_trans: false,
            h_layout: false,
            w_layout: false,
            h_ignore_size: false,
            w_ignore_size: false,
            is_deleting: false,
            rendered: false,
            has_blur: false,
            widget: Box::new(()),
        };
        let obj = Obj(ObjId(ui.objs.insert(data)));
        if let Some(p) = parent {
            let pd = ui.obj_mut(p).unwrap();
            pd.spec
                .get_or_insert_with(|| Box::new(SpecAttr::new()))
                .children
                .push(obj.0);
        }
        obj
    }

    fn sel(part: Part, state: State) -> StyleSelector {
        StyleSelector::new(part, state)
    }

    #[test]
    fn obj_style_kind_follows_src() {
        let local = ObjStyle {
            src: ObjStyleSrc::Local(Box::new(Style::new())),
            selector: StyleSelector::DEFAULT,
            is_disabled: false,
            is_theme: false,
        };
        assert!(local.is_local());
        assert!(!local.is_trans());
        let trans = ObjStyle {
            src: ObjStyleSrc::Trans(Box::new(Style::new())),
            selector: StyleSelector::DEFAULT,
            is_disabled: false,
            is_theme: false,
        };
        assert!(trans.is_trans());
        assert!(!trans.is_local());
    }

    #[test]
    fn state_cmp_order() {
        assert!(StyleStateCmp::Same < StyleStateCmp::DiffRedraw);
        assert!(StyleStateCmp::DiffRedraw < StyleStateCmp::DiffDrawPad);
        assert!(StyleStateCmp::DiffDrawPad < StyleStateCmp::DiffLayout);
        assert_eq!(StyleStateCmp::DiffLayout as u8, 3);
    }

    #[test]
    fn accessor_table_matches_style_gen() {
        // lv_obj_style_gen.c has one setter/getter pair per built-in property,
        // in the same order as lv_style_gen.c.
        assert_eq!(OBJ_STYLE_ACCESSOR_PROPS, crate::misc::style_gen::BUILTIN_PROPS);
        assert_eq!(style_prop_shifted(StyleProp::WIDTH), 1);
        assert_eq!(
            style_prop_shifted(StyleProp::BG_COLOR),
            1 << (StyleProp::BG_COLOR.0 >> 3)
        );
        assert_eq!(SELECTOR_ANY.0, 0x0F_FFFF);
        assert_eq!(
            obj_style_get_selector_part(sel(Part::KNOB, State::PRESSED)),
            Part::KNOB
        );
        assert_eq!(
            obj_style_get_selector_state(sel(Part::KNOB, State::PRESSED)),
            State::PRESSED
        );
    }

    /// Port of `test_style.c: test_style_replacement`.
    #[test]
    fn style_replacement() {
        let mut ui = ui();
        let mut red = Style::new();
        red.set_bg_color(Color::hex(0xff0000));
        let mut blue = Style::new();
        blue.set_bg_color(Color::hex(0x0000ff));
        let style_red = ui.style_create(red);
        let style_blue = ui.style_create(blue);

        let obj = new_obj(&mut ui, None);
        ui.obj_add_style(obj, style_red, StyleSelector::DEFAULT);
        assert_eq!(ui.obj_get_style_bg_color(obj, Part::MAIN), Color::hex(0xff0000));

        /*Replace style successfully*/
        let replaced = ui.obj_replace_style(obj, style_red, style_blue, StyleSelector::DEFAULT);
        assert!(replaced);
        assert_eq!(ui.obj_get_style_bg_color(obj, Part::MAIN), Color::hex(0x0000ff));

        /*Failed replacement (already replaced)*/
        let replaced = ui.obj_replace_style(obj, style_red, style_blue, StyleSelector::DEFAULT);
        assert!(!replaced);
        assert_eq!(ui.get_style_bg_color(obj, Part::MAIN), Color::hex(0x0000ff));
    }

    /// Port of `test_style.c: test_const_style`.
    #[test]
    fn const_style() {
        static PROPS: [ConstStyleEntry; 2] = [ConstStyleEntry::width(51), ConstStyleEntry::height(50)];
        static CONST_STYLE: ConstStyle = ConstStyle::new(&PROPS);
        let mut ui = ui();
        let obj = new_obj(&mut ui, None);
        ui.obj_add_const_style(obj, &CONST_STYLE, StyleSelector::DEFAULT);
        assert_eq!(ui.obj_get_style_width(obj, Part::MAIN), 51);
        assert_eq!(ui.obj_get_style_height(obj, Part::MAIN), 50);
        let bits = style_prop_shifted(StyleProp::WIDTH) | style_prop_shifted(StyleProp::HEIGHT);
        assert_eq!(ui.obj(obj).unwrap().style_main_prop_is_set, bits);

        // Adding it again does not duplicate it.
        ui.obj_add_const_style(obj, &CONST_STYLE, StyleSelector::DEFAULT);
        assert_eq!(ui.obj(obj).unwrap().styles.len(), 1);
        ui.obj_remove_const_style(obj, &CONST_STYLE, StyleSelector::DEFAULT);
        assert!(ui.obj(obj).unwrap().styles.is_empty());
        // Back to the class default (`width_def`).
        assert_eq!(ui.obj_get_style_width(obj, Part::MAIN), OBJ_CLASS.width_def);
    }

    /// Port of `test_style.c: test_style_has_prop` (with properties that don't
    /// invalidate, as invalidation lives in another file).
    #[test]
    fn style_has_prop() {
        let mut ui = ui();
        let mut style = Style::new();
        style.set_outline_color(Color::WHITE);
        let style = ui.style_create(style);
        let obj = new_obj(&mut ui, None);

        assert!(!ui.obj_has_style_prop(obj, Part::MAIN.into(), StyleProp::OUTLINE_COLOR));
        assert!(!ui.obj_has_style_prop(obj, Part::MAIN.into(), StyleProp::BORDER_OPA));
        assert!(!ui.obj_has_style_prop(obj, Part::INDICATOR.into(), StyleProp::OUTLINE_COLOR));

        ui.obj_add_style(obj, style, StyleSelector::DEFAULT);
        ui.obj_set_style_border_opa(obj, 2, StyleSelector::DEFAULT);

        assert!(ui.obj_has_style_prop(obj, Part::MAIN.into(), StyleProp::OUTLINE_COLOR));
        assert!(ui.obj_has_style_prop(obj, Part::MAIN.into(), StyleProp::BORDER_OPA));
        assert!(!ui.obj_has_style_prop(obj, Part::INDICATOR.into(), StyleProp::OUTLINE_COLOR));
    }

    #[test]
    fn style_order_local_before_shared_and_trans_first() {
        let mut ui = ui();
        let s1 = ui.style_create(Style::new());
        let s2 = ui.style_create(Style::new());
        let obj = new_obj(&mut ui, None);
        ui.obj_add_style(obj, s1, StyleSelector::DEFAULT);
        ui.obj_add_style(obj, s2, StyleSelector::DEFAULT);
        ui.obj_set_style_bg_opa(obj, 10, StyleSelector::DEFAULT);
        ui.obj_set_style_bg_opa(obj, 20, sel(Part::MAIN, State::PRESSED));
        let idx = ui.get_trans_style(obj, StyleSelector::DEFAULT).unwrap();
        assert_eq!(idx, 0);
        let kinds: Vec<&str> = ui
            .obj(obj)
            .unwrap()
            .styles
            .iter()
            .map(|os| match os.src {
                ObjStyleSrc::Trans(_) => "trans",
                ObjStyleSrc::Local(_) => "local",
                ObjStyleSrc::Shared(id) if id == s1 => "s1",
                ObjStyleSrc::Shared(_) => "s2",
                ObjStyleSrc::Const(_) => "const",
            })
            .collect();
        // Shared styles are inserted before the other normal styles (latest first).
        assert_eq!(kinds, vec!["trans", "local", "local", "s2", "s1"]);
        assert_eq!(
            ui.obj_get_local_style_prop(obj, StyleProp::BG_OPA, sel(Part::MAIN, State::PRESSED)),
            Some(StyleValue::Num(20))
        );
        assert_eq!(
            ui.obj_get_local_style_prop(obj, StyleProp::BG_COLOR, StyleSelector::DEFAULT),
            None
        );
    }

    #[test]
    fn state_weight_and_first_wins() {
        let mut ui = ui();
        let mut pressed = Style::new();
        pressed.set_bg_opa(1);
        let mut pressed_focused = Style::new();
        pressed_focused.set_bg_opa(2);
        let mut checked = Style::new();
        checked.set_bg_opa(3);
        let mut checked2 = Style::new();
        checked2.set_bg_opa(4);
        let pressed = ui.style_create(pressed);
        let pressed_focused = ui.style_create(pressed_focused);
        let checked = ui.style_create(checked);
        let checked2 = ui.style_create(checked2);
        let obj = new_obj(&mut ui, None);
        ui.obj_add_style(obj, pressed, sel(Part::MAIN, State::PRESSED));
        ui.obj_add_style(
            obj,
            pressed_focused,
            sel(Part::MAIN, State::PRESSED | State::FOCUSED),
        );
        ui.obj_add_style(obj, checked, sel(Part::MAIN, State::CHECKED));
        ui.obj_add_style(obj, checked2, sel(Part::MAIN, State::CHECKED));

        let set_state = |ui: &mut Ui, s: State| ui.obj_mut(obj).unwrap().state = s;
        set_state(&mut ui, State::DEFAULT);
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::MAIN), 0);
        set_state(&mut ui, State::PRESSED);
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::MAIN), 1);
        // HOVER+PRESS state: the PRESS style is OK.
        set_state(&mut ui, State::PRESSED | State::HOVERED);
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::MAIN), 1);
        set_state(&mut ui, State::PRESSED | State::FOCUSED);
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::MAIN), 2);
        // CHECKED (4) has a lower weight than PRESSED (0x80).
        set_state(&mut ui, State::PRESSED | State::CHECKED);
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::MAIN), 1);
        // Equal weight: the first style in the list (the last added) wins.
        set_state(&mut ui, State::CHECKED);
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::MAIN), 4);

        // Disabled styles are skipped.
        ui.obj_set_style_enabled(obj, checked2, sel(Part::MAIN, State::CHECKED), false);
        assert!(!ui.obj_get_style_enabled(obj, checked2, sel(Part::MAIN, State::CHECKED)));
        assert!(ui.obj_style_get_disabled(obj, checked2, sel(Part::MAIN, State::CHECKED)));
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::MAIN), 3);
        ui.obj_style_set_disabled(obj, checked2, sel(Part::MAIN, State::CHECKED), false);
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::MAIN), 4);
    }

    #[test]
    fn inheritance_goes_to_parent_main() {
        let mut ui = ui();
        let parent = new_obj(&mut ui, None);
        let child = new_obj(&mut ui, Some(parent));
        ui.obj_set_style_text_color(parent, Color::hex(0x112233), StyleSelector::DEFAULT);
        ui.obj_set_style_bg_color(parent, Color::hex(0x445566), StyleSelector::DEFAULT);
        // Inheritable: found on the parent's MAIN part, from any part of the child.
        assert_eq!(
            ui.obj_get_style_text_color(child, Part::MAIN),
            Color::hex(0x112233)
        );
        assert_eq!(
            ui.obj_get_style_text_color(child, Part::KNOB),
            Color::hex(0x112233)
        );
        // Not inheritable: default.
        assert_eq!(ui.obj_get_style_bg_color(child, Part::MAIN), Color::WHITE);
        // A part first falls back to the object's own MAIN part.
        ui.obj_set_style_text_color(child, Color::hex(0x778899), StyleSelector::DEFAULT);
        assert_eq!(
            ui.obj_get_style_text_color(child, Part::KNOB),
            Color::hex(0x778899)
        );
        // The parent's state is used for the parent's lookup.
        ui.obj_set_style_text_opa(parent, 99, sel(Part::MAIN, State::CHECKED));
        assert_eq!(ui.obj_get_style_text_opa(child, Part::MAIN), OPA_COVER);
        ui.obj_mut(parent).unwrap().state = State::CHECKED;
        assert_eq!(ui.obj_get_style_text_opa(child, Part::MAIN), 99);
        // Width / height come from the class only for MAIN.
        assert_eq!(ui.obj_get_style_width(child, Part::MAIN), OBJ_CLASS.width_def);
        assert_eq!(ui.obj_get_style_height(child, Part::KNOB), 0);
        let (v, res) = ui.obj_get_style_prop_internal(child, Part::KNOB, StyleProp::BG_OPA);
        assert_eq!(res, StyleRes::NotFound);
        assert_eq!(v.num(), 0);
    }

    #[test]
    fn cache_bits_gate_the_lookup() {
        let mut ui = ui();
        let mut st = Style::new();
        st.set_bg_opa(50);
        let st = ui.style_create(st);
        let obj = new_obj(&mut ui, None);
        ui.obj_add_style(obj, st, sel(Part::KNOB, State::DEFAULT));
        let od = ui.obj(obj).unwrap();
        assert_eq!(od.style_main_prop_is_set, 0);
        assert_eq!(od.style_other_prop_is_set, style_prop_shifted(StyleProp::BG_OPA));
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::KNOB), 50);
        // Clearing the presence bit hides the property (the happy path is skipped).
        ui.obj_mut(obj).unwrap().style_other_prop_is_set = 0;
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::KNOB), 0);
        ui.full_cache_refresh(obj, Part::KNOB);
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::KNOB), 50);
        // Remove local prop + remove style.
        ui.obj_set_style_bg_opa(obj, 7, sel(Part::KNOB, State::DEFAULT));
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::KNOB), 7);
        assert!(ui.obj_remove_local_style_prop(obj, StyleProp::BG_OPA, sel(Part::KNOB, State::DEFAULT)));
        assert!(!ui.obj_remove_local_style_prop(obj, StyleProp::BG_OPA, sel(Part::KNOB, State::DEFAULT)));
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::KNOB), 50);
        ui.obj_remove_style(obj, Some(st), sel(Part::KNOB, State::DEFAULT));
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::KNOB), 0);
        // Only the empty local style is left.
        assert_eq!(ui.obj(obj).unwrap().styles.len(), 1);
        assert_eq!(ui.obj(obj).unwrap().style_other_prop_is_set, 0);
    }

    #[test]
    fn remove_style_by_selector_and_theme() {
        let mut ui = ui();
        let a = ui.style_create(Style::new());
        let b = ui.style_create(Style::new());
        let obj = new_obj(&mut ui, None);
        ui.obj_add_style(obj, a, sel(Part::KNOB, State::DEFAULT));
        ui.obj_add_style(obj, a, sel(Part::KNOB, State::PRESSED));
        ui.obj_add_style(obj, b, sel(Part::INDICATOR, State::DEFAULT));
        ui.obj_mut(obj).unwrap().styles[2].is_theme = true; // a @ KNOB|DEFAULT
        ui.obj_remove_style(obj, Some(a), sel(Part::KNOB, State::ANY));
        assert_eq!(ui.obj(obj).unwrap().styles.len(), 1);
        ui.obj_add_style(obj, a, sel(Part::KNOB, State::DEFAULT));
        ui.obj_mut(obj).unwrap().styles[0].is_theme = true;
        ui.obj_remove_theme(obj, sel(Part::INDICATOR, State::ANY));
        assert_eq!(ui.obj(obj).unwrap().styles.len(), 2);
        ui.obj_remove_theme(obj, sel(Part::KNOB, State::ANY));
        assert_eq!(ui.obj(obj).unwrap().styles.len(), 1);
        // Deleting a shared style detaches it.
        ui.style_delete(b);
        assert!(ui.obj(obj).unwrap().styles.is_empty());
        assert!(ui.style(b).is_none());
    }

    #[test]
    fn state_compare() {
        let mut ui = ui();
        let mut pad = Style::new();
        pad.set_pad_left(5);
        let mut shadow = Style::new();
        shadow.set_shadow_width(5);
        let mut color = Style::new();
        color.set_bg_color(Color::hex(0x123456));
        let pad = ui.style_create(pad);
        let shadow = ui.style_create(shadow);
        let color = ui.style_create(color);
        let obj = new_obj(&mut ui, None);

        ui.obj_add_style(obj, color, sel(Part::SCROLLBAR, State::PRESSED));
        assert_eq!(
            ui.obj_style_state_compare(obj, State::DEFAULT, State::PRESSED),
            (StyleStateCmp::DiffRedraw, Part::SCROLLBAR)
        );
        assert_eq!(
            ui.obj_style_state_compare(obj, State::DEFAULT, State::FOCUSED),
            (StyleStateCmp::Same, Part::ANY)
        );
        ui.obj_add_style(obj, shadow, sel(Part::KNOB, State::PRESSED));
        assert_eq!(
            ui.obj_style_state_compare(obj, State::DEFAULT, State::PRESSED),
            (StyleStateCmp::DiffDrawPad, Part::ANY)
        );
        ui.obj_add_style(obj, pad, sel(Part::KNOB, State::PRESSED));
        assert_eq!(
            ui.obj_style_state_compare(obj, State::PRESSED, State::DEFAULT),
            (StyleStateCmp::DiffLayout, Part::ANY)
        );
    }

    #[test]
    fn transition_lifecycle() {
        let mut ui = ui();
        let obj = new_obj(&mut ui, None);
        ui.obj_set_style_bg_color(obj, Color::hex(0x000000), StyleSelector::DEFAULT);
        ui.obj_set_style_bg_color(obj, Color::hex(0xff0000), sel(Part::MAIN, State::PRESSED));
        ui.obj_mut(obj).unwrap().state = State::PRESSED;

        let dsc = ObjStyleTransitionDsc {
            path: AnimPath::Linear,
            user_data: None,
            selector: StyleSelector::DEFAULT,
            time: 100,
            delay: 0,
            prop: StyleProp::BG_COLOR,
        };
        // Same value in both states: nothing happens.
        let same = ObjStyleTransitionDsc {
            prop: StyleProp::BG_OPA,
            ..dsc
        };
        ui.obj_style_create_transition(obj, Part::MAIN, State::DEFAULT, State::PRESSED, &same);
        assert!(ui.style_trans.is_empty());

        let dsc = ObjStyleTransitionDsc {
            path: AnimPath::Linear,
            user_data: None,
            selector: StyleSelector::DEFAULT,
            time: 100,
            delay: 0,
            prop: StyleProp::BG_COLOR,
        };
        ui.obj_style_create_transition(obj, Part::MAIN, State::DEFAULT, State::PRESSED, &dsc);
        assert_eq!(ui.style_trans_order.len(), 1);
        let tr = ui.style_trans_order[0];
        assert!(ui.obj(obj).unwrap().styles[0].is_trans());
        // The transition style holds the start value and wins over the state.
        assert_eq!(ui.obj_get_style_bg_color(obj, Part::MAIN), Color::hex(0x000000));
        assert_eq!(
            ui.style_trans[tr.0].end_value,
            StyleValue::Color(Color::hex(0xff0000))
        );

        let a = ui
            .anim_get(AnimTarget::StyleTrans(tr), None)
            .expect("transition animation");
        assert!(!ui.anim(a).unwrap().early_apply);
        ui.obj_style_trans_anim_start_cb(a);
        assert_eq!(ui.style_trans_order.len(), 1);

        ui.obj_style_trans_anim_cb(tr, 128);
        assert_eq!(
            ui.obj_get_style_bg_color(obj, Part::MAIN),
            Color::mix(Color::hex(0xff0000), Color::hex(0x000000), 128)
        );
        ui.obj_style_trans_anim_cb(tr, 255);
        assert_eq!(ui.obj_get_style_bg_color(obj, Part::MAIN), Color::hex(0xff0000));

        ui.obj_style_trans_anim_completed_cb(a);
        assert!(ui.style_trans.is_empty());
        // The emptied transition style is removed.
        assert!(ui.obj(obj).unwrap().styles.iter().all(|os| !os.is_trans()));
        assert_eq!(ui.obj_get_style_bg_color(obj, Part::MAIN), Color::hex(0xff0000));
    }

    #[test]
    fn local_prop_stops_transition() {
        let mut ui = ui();
        let obj = new_obj(&mut ui, None);
        ui.obj_set_style_bg_opa(obj, 0, StyleSelector::DEFAULT);
        ui.obj_set_style_bg_opa(obj, 200, sel(Part::MAIN, State::PRESSED));
        ui.obj_mut(obj).unwrap().state = State::PRESSED;
        let dsc = ObjStyleTransitionDsc {
            path: AnimPath::Linear,
            user_data: None,
            selector: StyleSelector::DEFAULT,
            time: 100,
            delay: 0,
            prop: StyleProp::BG_OPA,
        };
        ui.obj_style_create_transition(obj, Part::MAIN, State::DEFAULT, State::PRESSED, &dsc);
        let tr = ui.style_trans_order[0];
        ui.obj_style_trans_anim_cb(tr, 128);
        // 0 + ((200 - 0) * 128 >> 8)
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::MAIN), 100);
        ui.obj_set_style_bg_opa(obj, 150, sel(Part::MAIN, State::PRESSED));
        assert!(ui.style_trans.is_empty());
        assert!(ui.anim_get(AnimTarget::StyleTrans(tr), None).is_none());
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::MAIN), 150);
    }

    #[test]
    fn radius_circle_transition_uses_the_size() {
        let mut ui = ui();
        let obj = new_obj(&mut ui, None); // 100 x 50
        ui.obj_set_style_radius(obj, RADIUS_CIRCLE, sel(Part::MAIN, State::PRESSED));
        ui.obj_mut(obj).unwrap().state = State::PRESSED;
        let dsc = ObjStyleTransitionDsc {
            path: AnimPath::Linear,
            user_data: None,
            selector: StyleSelector::DEFAULT,
            time: 100,
            delay: 0,
            prop: StyleProp::RADIUS,
        };
        ui.obj_style_create_transition(obj, Part::MAIN, State::DEFAULT, State::PRESSED, &dsc);
        let tr = ui.style_trans_order[0];
        assert_eq!(ui.style_trans[tr.0].end_value, StyleValue::Num(26));
        assert!(style_value_eq(
            &ui.style_trans[tr.0].start_value,
            &StyleValue::Num(0)
        ));
    }

    #[test]
    fn opa_and_recolor_recursive() {
        let mut ui = ui();
        let parent = new_obj(&mut ui, None);
        let child = new_obj(&mut ui, Some(parent));
        assert_eq!(ui.obj_get_style_opa_recursive(child, Part::MAIN), OPA_COVER);
        ui.obj_set_style_opa(parent, 128, StyleSelector::DEFAULT);
        ui.obj_set_style_opa(child, 128, StyleSelector::DEFAULT);
        assert_eq!(
            ui.obj_get_style_opa_recursive(child, Part::MAIN),
            opa_mix2(opa_mix2(255, 128), 128)
        );
        // A non-MAIN part uses its own opa, then the MAIN part of the same object.
        ui.obj_set_style_opa(child, 200, sel(Part::KNOB, State::DEFAULT));
        assert_eq!(
            ui.obj_get_style_opa_recursive(child, Part::KNOB),
            opa_mix2(opa_mix2(opa_mix2(255, 200), 128), 128)
        );
        ui.obj_set_style_opa(parent, 1, StyleSelector::DEFAULT);
        assert_eq!(ui.obj_get_style_opa_recursive(child, Part::MAIN), OPA_TRANSP);

        ui.obj_set_style_recolor(parent, Color::hex(0xff0000), StyleSelector::DEFAULT);
        ui.obj_set_style_recolor_opa(parent, OPA_COVER, StyleSelector::DEFAULT);
        let c = ui.obj_get_style_recolor_recursive(child, Part::MAIN);
        // Child has no recolor (transparent), the parent's covering recolor wins.
        assert_eq!(
            c,
            Color32::over32(Color::BLACK.to_32(0), Color::hex(0xff0000).to_32(255))
        );
        assert_eq!(c.alpha, 255);
    }

    #[test]
    fn space_and_scale_helpers() {
        let mut ui = ui();
        let obj = new_obj(&mut ui, None);
        let mut st = Style::new();
        st.set_pad_left(3);
        st.set_pad_top(4);
        st.set_border_width(2);
        st.set_border_side(BorderSide::LEFT | BorderSide::BOTTOM);
        st.set_transform_scale_x(0);
        let st = ui.style_create(st);
        let mut cst = Style::new();
        cst.set_pad_left(3);
        ui.objs.get_mut(obj.0.0).unwrap().styles.push(ObjStyle {
            src: ObjStyleSrc::Shared(st),
            selector: StyleSelector::DEFAULT,
            is_disabled: false,
            is_theme: false,
        });
        ui.full_cache_refresh(obj, Part::ANY);
        assert_eq!(ui.obj_get_style_space_left(obj, Part::MAIN), 5);
        assert_eq!(ui.obj_get_style_space_top(obj, Part::MAIN), 4);
        assert_eq!(ui.obj_get_style_space_right(obj, Part::MAIN), 0);
        assert_eq!(ui.obj_get_style_space_bottom(obj, Part::MAIN), 2);
        assert_eq!(ui.obj_get_style_transform_scale_x_safe(obj, Part::MAIN), 1);
        assert_eq!(ui.obj_get_style_transform_scale_y_safe(obj, Part::MAIN), 256);
        assert_eq!(
            ui.obj_get_style_border_side(obj, Part::MAIN),
            BorderSide::LEFT | BorderSide::BOTTOM
        );
        assert_eq!(
            ui.obj_calculate_style_text_align(obj, Part::MAIN, "abc"),
            TextAlign::Left
        );
        let _ = cst;
    }

    #[test]
    fn layer_type_and_blur_status() {
        let mut ui = ui();
        let obj = new_obj(&mut ui, None);
        ui.obj_update_layer_type(obj);
        assert!(ui.obj(obj).unwrap().spec.is_none());
        ui.obj_set_style_opa_layered(obj, 100, StyleSelector::DEFAULT);
        ui.obj_update_layer_type(obj);
        assert_eq!(
            ui.obj(obj).unwrap().spec.as_ref().unwrap().layer_type,
            LayerType::Simple
        );
        ui.obj_set_style_opa_layered(obj, OPA_COVER, StyleSelector::DEFAULT);
        ui.obj_set_style_blend_mode(obj, BlendMode::Additive, StyleSelector::DEFAULT);
        ui.obj_update_layer_type(obj);
        assert_eq!(
            ui.obj(obj).unwrap().spec.as_ref().unwrap().layer_type,
            LayerType::Simple
        );
        ui.obj_set_style_blend_mode(obj, BlendMode::Normal, StyleSelector::DEFAULT);
        ui.obj_update_layer_type(obj);
        assert_eq!(
            ui.obj(obj).unwrap().spec.as_ref().unwrap().layer_type,
            LayerType::None
        );
        assert!(ui.obj_get_style_bitmap_mask_src(obj, Part::MAIN) == ImageSrc::None);

        let mut blur = Style::new();
        blur.set_blur_radius(4);
        let blur = ui.style_create(blur);
        ui.objs.get_mut(obj.0.0).unwrap().styles.push(ObjStyle {
            src: ObjStyleSrc::Shared(blur),
            selector: sel(Part::MAIN, State::PRESSED),
            is_disabled: false,
            is_theme: false,
        });
        ui.obj_update_blur_status(obj);
        assert!(!ui.obj(obj).unwrap().has_blur);
        ui.obj_mut(obj).unwrap().state = State::PRESSED;
        ui.obj_update_blur_status(obj);
        assert!(ui.obj(obj).unwrap().has_blur);
        assert_eq!(ui.blur_obj_cnt, 1);
        ui.obj_mut(obj).unwrap().state = State::DEFAULT;
        ui.obj_update_blur_status(obj);
        assert_eq!(ui.blur_obj_cnt, 0);
    }

    #[test]
    fn fade_in_creates_an_opa_animation() {
        let mut ui = ui();
        let obj = new_obj(&mut ui, None);
        ui.obj_fade_in(obj, 300, 0);
        let a = ui
            .anim_get(AnimTarget::Obj(obj), Some(&AnimExec::ObjOpa(StyleSelector(0))))
            .expect("fade animation");
        let an = ui.anim(a).unwrap();
        assert_eq!((an.start_value, an.end_value), (0, OPA_COVER as i32));
        // early_apply wrote the start value as a local property.
        assert_eq!(ui.obj_get_style_opa(obj, Part::MAIN), 0);
        let cb = ui.anim(a).unwrap().completed_cb.clone().unwrap();
        cb(&mut ui, a);
        assert_eq!(
            ui.obj_get_local_style_prop(obj, StyleProp::OPA, StyleSelector(0)),
            None
        );
        assert_eq!(ui.obj_get_style_opa(obj, Part::MAIN), OPA_COVER);
    }

    #[test]
    fn union_equality() {
        assert!(style_value_eq(&StyleValue::Null, &StyleValue::Num(0)));
        assert!(style_value_eq(
            &StyleValue::Null,
            &StyleValue::Color(Color::BLACK)
        ));
        assert!(!style_value_eq(&StyleValue::Null, &StyleValue::Num(1)));
        assert!(style_value_eq(
            &StyleValue::Num(0x123456),
            &StyleValue::Color(Color::hex(0x123456))
        ));
        assert!(style_value_eq(&StyleValue::Num(3), &StyleValue::Num(3)));
    }
}
