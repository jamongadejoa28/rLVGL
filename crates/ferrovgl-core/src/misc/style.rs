//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_style.h`, `src/misc/lv_style.c`,
//! `src/misc/lv_style_private.h`, `include/lvgl/core/lv_obj_style.h` (part/state/selector).
//!
//! Style properties keep their upstream numbers so that the group caching
//! (`has_group`, `style_main_prop_is_set`) and the flag table behave exactly
//! like LVGL. `StyleProp` and `Part` are open newtypes (like the C `uint8_t`
//! / `uint32_t`) so custom properties and custom parts need no extra type.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::draw::draw_buf::{DrawBuf, ImageDsc};
use crate::draw::grad::GradDsc;
use crate::font::{Font, FontRef};
use crate::misc::anim::{Anim, AnimPath};
use crate::misc::check_arg_failed;
use crate::misc::color::{Color, ColorFilter, OPA_COVER};
use crate::misc::types::coord;

/// `LV_STYLE_SENTINEL_VALUE`
pub const STYLE_SENTINEL_VALUE: u32 = 0xAABB_CCDD;

/// `LV_STYLE_PROP_FLAG_NONE`
pub const STYLE_PROP_FLAG_NONE: u8 = 0;
/// `LV_STYLE_PROP_FLAG_INHERITABLE`: inherited from the parent.
pub const STYLE_PROP_FLAG_INHERITABLE: u8 = 1 << 0;
/// `LV_STYLE_PROP_FLAG_EXT_DRAW_UPDATE`: requires ext. draw size update when changed.
pub const STYLE_PROP_FLAG_EXT_DRAW_UPDATE: u8 = 1 << 1;
/// `LV_STYLE_PROP_FLAG_LAYOUT_UPDATE`: requires layout update when changed.
pub const STYLE_PROP_FLAG_LAYOUT_UPDATE: u8 = 1 << 2;
/// `LV_STYLE_PROP_FLAG_PARENT_LAYOUT_UPDATE`: requires layout update on the parent.
pub const STYLE_PROP_FLAG_PARENT_LAYOUT_UPDATE: u8 = 1 << 3;
/// `LV_STYLE_PROP_FLAG_LAYER_UPDATE`: affects layer handling.
pub const STYLE_PROP_FLAG_LAYER_UPDATE: u8 = 1 << 4;
/// `LV_STYLE_PROP_FLAG_TRANSFORM`: affects the object's transformation.
pub const STYLE_PROP_FLAG_TRANSFORM: u8 = 1 << 5;
/// `LV_STYLE_PROP_FLAG_ALL`
pub const STYLE_PROP_FLAG_ALL: u8 = 0x3F;

/// `LV_SCALE_NONE`: value for not zooming.
pub const SCALE_NONE: i32 = 256;

/// `LV_GRAD_LEFT`
pub const GRAD_LEFT: i32 = coord::pct(0);
/// `LV_GRAD_RIGHT`
pub const GRAD_RIGHT: i32 = coord::pct(100);
/// `LV_GRAD_TOP`
pub const GRAD_TOP: i32 = coord::pct(0);
/// `LV_GRAD_BOTTOM`
pub const GRAD_BOTTOM: i32 = coord::pct(100);
/// `LV_GRAD_CENTER`
pub const GRAD_CENTER: i32 = coord::pct(50);

/// `lv_blend_mode_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum BlendMode {
    /// `LV_BLEND_MODE_NORMAL`: simply mix according to the opacity value.
    #[default]
    Normal = 0,
    /// `LV_BLEND_MODE_ADDITIVE`: add the respective color channels.
    Additive = 1,
    /// `LV_BLEND_MODE_SUBTRACTIVE`: subtract the foreground from the background.
    Subtractive = 2,
    /// `LV_BLEND_MODE_MULTIPLY`: multiply the foreground and background.
    Multiply = 3,
    /// `LV_BLEND_MODE_DIFFERENCE`: absolute difference between foreground and background.
    Difference = 4,
}

impl BlendMode {
    /// Convert a raw style number back to a blend mode (`Normal` if unknown).
    #[must_use]
    pub const fn from_u8(v: u8) -> BlendMode {
        match v {
            1 => BlendMode::Additive,
            2 => BlendMode::Subtractive,
            3 => BlendMode::Multiply,
            4 => BlendMode::Difference,
            _ => BlendMode::Normal,
        }
    }
}

bitflags::bitflags! {
    /// `lv_border_side_t`
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct BorderSide: u8 {
        /// `LV_BORDER_SIDE_BOTTOM`
        const BOTTOM = 0x01;
        /// `LV_BORDER_SIDE_TOP`
        const TOP = 0x02;
        /// `LV_BORDER_SIDE_LEFT`
        const LEFT = 0x04;
        /// `LV_BORDER_SIDE_RIGHT`
        const RIGHT = 0x08;
        /// `LV_BORDER_SIDE_FULL`
        const FULL = 0x0F;
        /// `LV_BORDER_SIDE_INTERNAL`: for matrix-like objects (e.g. button matrix).
        const INTERNAL = 0x10;
    }
}

impl BorderSide {
    /// `LV_BORDER_SIDE_NONE`
    pub const NONE: BorderSide = BorderSide::empty();
}

/// `lv_blur_quality_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum BlurQuality {
    /// `LV_BLUR_QUALITY_AUTO`
    #[default]
    Auto = 0,
    /// `LV_BLUR_QUALITY_SPEED`
    Speed = 1,
    /// `LV_BLUR_QUALITY_PRECISION`
    Precision = 2,
}

impl BlurQuality {
    /// Convert a raw style number back to a quality (`Auto` if unknown).
    #[must_use]
    pub const fn from_u8(v: u8) -> BlurQuality {
        match v {
            1 => BlurQuality::Speed,
            2 => BlurQuality::Precision,
            _ => BlurQuality::Auto,
        }
    }
}

/// `lv_image_colorkey_t`: pixels inside `[low, high]` become transparent.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ImageColorkey {
    /// `low`
    pub low: Color,
    /// `high`
    pub high: Color,
}

/// `lv_style_prop_t`: a style property id.
///
/// Like the C `uint8_t` this is an *open* value: the built-in properties are
/// the associated constants below (numbers identical to `lv_style.h`, so the
/// group caching `has_group` / `style_main_prop_is_set` and the flag table
/// behave exactly like LVGL) and ids returned by
/// [`crate::core::Ui::style_register_prop`] (`>= NUM_BUILT_IN`) are plain
/// `StyleProp`s too. Constants can be used in `match` patterns.
///
/// `prop >> 2` is the group used by [`get_prop_group`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct StyleProp(pub u8);

impl StyleProp {
    /// `LV_STYLE_PROP_INV`
    pub const INV: StyleProp = StyleProp(0);
    /// `LV_STYLE_WIDTH`
    pub const WIDTH: StyleProp = StyleProp(1);
    /// `LV_STYLE_HEIGHT`
    pub const HEIGHT: StyleProp = StyleProp(2);
    /// `LV_STYLE_LENGTH`
    pub const LENGTH: StyleProp = StyleProp(3);
    /// `LV_STYLE_TRANSFORM_WIDTH`
    pub const TRANSFORM_WIDTH: StyleProp = StyleProp(4);
    /// `LV_STYLE_TRANSFORM_HEIGHT`
    pub const TRANSFORM_HEIGHT: StyleProp = StyleProp(5);
    /// `LV_STYLE_MIN_WIDTH`
    pub const MIN_WIDTH: StyleProp = StyleProp(8);
    /// `LV_STYLE_MAX_WIDTH`
    pub const MAX_WIDTH: StyleProp = StyleProp(9);
    /// `LV_STYLE_MIN_HEIGHT`
    pub const MIN_HEIGHT: StyleProp = StyleProp(10);
    /// `LV_STYLE_MAX_HEIGHT`
    pub const MAX_HEIGHT: StyleProp = StyleProp(11);
    /// `LV_STYLE_TRANSLATE_X`
    pub const TRANSLATE_X: StyleProp = StyleProp(12);
    /// `LV_STYLE_TRANSLATE_Y`
    pub const TRANSLATE_Y: StyleProp = StyleProp(13);
    /// `LV_STYLE_RADIAL_OFFSET`
    pub const RADIAL_OFFSET: StyleProp = StyleProp(14);
    /// `LV_STYLE_X`
    pub const X: StyleProp = StyleProp(16);
    /// `LV_STYLE_Y`
    pub const Y: StyleProp = StyleProp(17);
    /// `LV_STYLE_ALIGN`
    pub const ALIGN: StyleProp = StyleProp(18);
    /// `LV_STYLE_PAD_TOP`
    pub const PAD_TOP: StyleProp = StyleProp(24);
    /// `LV_STYLE_PAD_BOTTOM`
    pub const PAD_BOTTOM: StyleProp = StyleProp(25);
    /// `LV_STYLE_PAD_LEFT`
    pub const PAD_LEFT: StyleProp = StyleProp(26);
    /// `LV_STYLE_PAD_RIGHT`
    pub const PAD_RIGHT: StyleProp = StyleProp(27);
    /// `LV_STYLE_PAD_RADIAL`
    pub const PAD_RADIAL: StyleProp = StyleProp(28);
    /// `LV_STYLE_PAD_ROW`
    pub const PAD_ROW: StyleProp = StyleProp(29);
    /// `LV_STYLE_PAD_COLUMN`
    pub const PAD_COLUMN: StyleProp = StyleProp(30);
    /// `LV_STYLE_MARGIN_TOP`
    pub const MARGIN_TOP: StyleProp = StyleProp(32);
    /// `LV_STYLE_MARGIN_BOTTOM`
    pub const MARGIN_BOTTOM: StyleProp = StyleProp(33);
    /// `LV_STYLE_MARGIN_LEFT`
    pub const MARGIN_LEFT: StyleProp = StyleProp(34);
    /// `LV_STYLE_MARGIN_RIGHT`
    pub const MARGIN_RIGHT: StyleProp = StyleProp(35);
    /// `LV_STYLE_BG_GRAD`
    pub const BG_GRAD: StyleProp = StyleProp(40);
    /// `LV_STYLE_BG_GRAD_DIR`
    pub const BG_GRAD_DIR: StyleProp = StyleProp(41);
    /// `LV_STYLE_BG_MAIN_OPA`
    pub const BG_MAIN_OPA: StyleProp = StyleProp(42);
    /// `LV_STYLE_BG_GRAD_OPA`
    pub const BG_GRAD_OPA: StyleProp = StyleProp(43);
    /// `LV_STYLE_BG_GRAD_COLOR`
    pub const BG_GRAD_COLOR: StyleProp = StyleProp(44);
    /// `LV_STYLE_BG_MAIN_STOP`
    pub const BG_MAIN_STOP: StyleProp = StyleProp(45);
    /// `LV_STYLE_BG_GRAD_STOP`
    pub const BG_GRAD_STOP: StyleProp = StyleProp(46);
    /// `LV_STYLE_BG_IMAGE_SRC`
    pub const BG_IMAGE_SRC: StyleProp = StyleProp(48);
    /// `LV_STYLE_BG_IMAGE_OPA`
    pub const BG_IMAGE_OPA: StyleProp = StyleProp(49);
    /// `LV_STYLE_BG_IMAGE_RECOLOR_OPA`
    pub const BG_IMAGE_RECOLOR_OPA: StyleProp = StyleProp(50);
    /// `LV_STYLE_BG_IMAGE_TILED`
    pub const BG_IMAGE_TILED: StyleProp = StyleProp(51);
    /// `LV_STYLE_BG_IMAGE_RECOLOR`
    pub const BG_IMAGE_RECOLOR: StyleProp = StyleProp(52);
    /// `LV_STYLE_BORDER_WIDTH`
    pub const BORDER_WIDTH: StyleProp = StyleProp(56);
    /// `LV_STYLE_BORDER_COLOR`
    pub const BORDER_COLOR: StyleProp = StyleProp(57);
    /// `LV_STYLE_BORDER_OPA`
    pub const BORDER_OPA: StyleProp = StyleProp(58);
    /// `LV_STYLE_BORDER_POST`
    pub const BORDER_POST: StyleProp = StyleProp(59);
    /// `LV_STYLE_BORDER_SIDE`
    pub const BORDER_SIDE: StyleProp = StyleProp(60);
    /// `LV_STYLE_OUTLINE_WIDTH`
    pub const OUTLINE_WIDTH: StyleProp = StyleProp(64);
    /// `LV_STYLE_OUTLINE_COLOR`
    pub const OUTLINE_COLOR: StyleProp = StyleProp(65);
    /// `LV_STYLE_OUTLINE_OPA`
    pub const OUTLINE_OPA: StyleProp = StyleProp(66);
    /// `LV_STYLE_OUTLINE_PAD`
    pub const OUTLINE_PAD: StyleProp = StyleProp(67);
    /// `LV_STYLE_BG_OPA`
    pub const BG_OPA: StyleProp = StyleProp(72);
    /// `LV_STYLE_BG_COLOR`
    pub const BG_COLOR: StyleProp = StyleProp(73);
    /// `LV_STYLE_SHADOW_WIDTH`
    pub const SHADOW_WIDTH: StyleProp = StyleProp(74);
    /// `LV_STYLE_LINE_WIDTH`
    pub const LINE_WIDTH: StyleProp = StyleProp(75);
    /// `LV_STYLE_ARC_WIDTH`
    pub const ARC_WIDTH: StyleProp = StyleProp(76);
    /// `LV_STYLE_TEXT_FONT`
    pub const TEXT_FONT: StyleProp = StyleProp(77);
    /// `LV_STYLE_IMAGE_RECOLOR_OPA`
    pub const IMAGE_RECOLOR_OPA: StyleProp = StyleProp(78);
    /// `LV_STYLE_IMAGE_OPA`
    pub const IMAGE_OPA: StyleProp = StyleProp(80);
    /// `LV_STYLE_SHADOW_OPA`
    pub const SHADOW_OPA: StyleProp = StyleProp(81);
    /// `LV_STYLE_LINE_OPA`
    pub const LINE_OPA: StyleProp = StyleProp(82);
    /// `LV_STYLE_ARC_OPA`
    pub const ARC_OPA: StyleProp = StyleProp(83);
    /// `LV_STYLE_TEXT_OPA`
    pub const TEXT_OPA: StyleProp = StyleProp(84);
    /// `LV_STYLE_SHADOW_COLOR`
    pub const SHADOW_COLOR: StyleProp = StyleProp(88);
    /// `LV_STYLE_IMAGE_RECOLOR`
    pub const IMAGE_RECOLOR: StyleProp = StyleProp(89);
    /// `LV_STYLE_LINE_COLOR`
    pub const LINE_COLOR: StyleProp = StyleProp(90);
    /// `LV_STYLE_ARC_COLOR`
    pub const ARC_COLOR: StyleProp = StyleProp(91);
    /// `LV_STYLE_TEXT_COLOR`
    pub const TEXT_COLOR: StyleProp = StyleProp(92);
    /// `LV_STYLE_ARC_IMAGE_SRC`
    pub const ARC_IMAGE_SRC: StyleProp = StyleProp(96);
    /// `LV_STYLE_SHADOW_OFFSET_X`
    pub const SHADOW_OFFSET_X: StyleProp = StyleProp(97);
    /// `LV_STYLE_SHADOW_OFFSET_Y`
    pub const SHADOW_OFFSET_Y: StyleProp = StyleProp(98);
    /// `LV_STYLE_SHADOW_SPREAD`
    pub const SHADOW_SPREAD: StyleProp = StyleProp(99);
    /// `LV_STYLE_LINE_DASH_WIDTH`
    pub const LINE_DASH_WIDTH: StyleProp = StyleProp(100);
    /// `LV_STYLE_TEXT_ALIGN`
    pub const TEXT_ALIGN: StyleProp = StyleProp(101);
    /// `LV_STYLE_TEXT_LETTER_SPACE`
    pub const TEXT_LETTER_SPACE: StyleProp = StyleProp(102);
    /// `LV_STYLE_TEXT_LINE_SPACE`
    pub const TEXT_LINE_SPACE: StyleProp = StyleProp(103);
    /// `LV_STYLE_LINE_DASH_GAP`
    pub const LINE_DASH_GAP: StyleProp = StyleProp(104);
    /// `LV_STYLE_LINE_ROUNDED`
    pub const LINE_ROUNDED: StyleProp = StyleProp(105);
    /// `LV_STYLE_IMAGE_COLORKEY`
    pub const IMAGE_COLORKEY: StyleProp = StyleProp(106);
    /// `LV_STYLE_TEXT_OUTLINE_STROKE_WIDTH`
    pub const TEXT_OUTLINE_STROKE_WIDTH: StyleProp = StyleProp(107);
    /// `LV_STYLE_TEXT_OUTLINE_STROKE_OPA`
    pub const TEXT_OUTLINE_STROKE_OPA: StyleProp = StyleProp(108);
    /// `LV_STYLE_TEXT_OUTLINE_STROKE_COLOR`
    pub const TEXT_OUTLINE_STROKE_COLOR: StyleProp = StyleProp(109);
    /// `LV_STYLE_TEXT_DECOR`
    pub const TEXT_DECOR: StyleProp = StyleProp(110);
    /// `LV_STYLE_ARC_ROUNDED`
    pub const ARC_ROUNDED: StyleProp = StyleProp(111);
    /// `LV_STYLE_OPA`
    pub const OPA: StyleProp = StyleProp(112);
    /// `LV_STYLE_OPA_LAYERED`
    pub const OPA_LAYERED: StyleProp = StyleProp(113);
    /// `LV_STYLE_COLOR_FILTER_DSC`
    pub const COLOR_FILTER_DSC: StyleProp = StyleProp(114);
    /// `LV_STYLE_COLOR_FILTER_OPA`
    pub const COLOR_FILTER_OPA: StyleProp = StyleProp(115);
    /// `LV_STYLE_ANIM`
    pub const ANIM: StyleProp = StyleProp(116);
    /// `LV_STYLE_ANIM_DURATION`
    pub const ANIM_DURATION: StyleProp = StyleProp(117);
    /// `LV_STYLE_TRANSITION`
    pub const TRANSITION: StyleProp = StyleProp(118);
    /// `LV_STYLE_RADIUS`
    pub const RADIUS: StyleProp = StyleProp(120);
    /// `LV_STYLE_BITMAP_MASK_SRC`
    pub const BITMAP_MASK_SRC: StyleProp = StyleProp(121);
    /// `LV_STYLE_BLEND_MODE`
    pub const BLEND_MODE: StyleProp = StyleProp(122);
    /// `LV_STYLE_ROTARY_SENSITIVITY`
    pub const ROTARY_SENSITIVITY: StyleProp = StyleProp(123);
    /// `LV_STYLE_TRANSLATE_RADIAL`
    pub const TRANSLATE_RADIAL: StyleProp = StyleProp(124);
    /// `LV_STYLE_CLIP_CORNER`
    pub const CLIP_CORNER: StyleProp = StyleProp(128);
    /// `LV_STYLE_BASE_DIR`
    pub const BASE_DIR: StyleProp = StyleProp(129);
    /// `LV_STYLE_RECOLOR`
    pub const RECOLOR: StyleProp = StyleProp(130);
    /// `LV_STYLE_RECOLOR_OPA`
    pub const RECOLOR_OPA: StyleProp = StyleProp(131);
    /// `LV_STYLE_LAYOUT`
    pub const LAYOUT: StyleProp = StyleProp(132);
    /// `LV_STYLE_BLUR_RADIUS`
    pub const BLUR_RADIUS: StyleProp = StyleProp(136);
    /// `LV_STYLE_BLUR_BACKDROP`
    pub const BLUR_BACKDROP: StyleProp = StyleProp(137);
    /// `LV_STYLE_BLUR_QUALITY`
    pub const BLUR_QUALITY: StyleProp = StyleProp(138);
    /// `LV_STYLE_DROP_SHADOW_RADIUS`
    pub const DROP_SHADOW_RADIUS: StyleProp = StyleProp(144);
    /// `LV_STYLE_DROP_SHADOW_OFFSET_X`
    pub const DROP_SHADOW_OFFSET_X: StyleProp = StyleProp(145);
    /// `LV_STYLE_DROP_SHADOW_OFFSET_Y`
    pub const DROP_SHADOW_OFFSET_Y: StyleProp = StyleProp(146);
    /// `LV_STYLE_DROP_SHADOW_COLOR`
    pub const DROP_SHADOW_COLOR: StyleProp = StyleProp(147);
    /// `LV_STYLE_DROP_SHADOW_OPA`
    pub const DROP_SHADOW_OPA: StyleProp = StyleProp(148);
    /// `LV_STYLE_DROP_SHADOW_QUALITY`
    pub const DROP_SHADOW_QUALITY: StyleProp = StyleProp(149);
    /// `LV_STYLE_TRANSFORM_SCALE_X`
    pub const TRANSFORM_SCALE_X: StyleProp = StyleProp(152);
    /// `LV_STYLE_TRANSFORM_SCALE_Y`
    pub const TRANSFORM_SCALE_Y: StyleProp = StyleProp(153);
    /// `LV_STYLE_TRANSFORM_PIVOT_X`
    pub const TRANSFORM_PIVOT_X: StyleProp = StyleProp(154);
    /// `LV_STYLE_TRANSFORM_PIVOT_Y`
    pub const TRANSFORM_PIVOT_Y: StyleProp = StyleProp(155);
    /// `LV_STYLE_TRANSFORM_ROTATION`
    pub const TRANSFORM_ROTATION: StyleProp = StyleProp(156);
    /// `LV_STYLE_TRANSFORM_SKEW_X`
    pub const TRANSFORM_SKEW_X: StyleProp = StyleProp(157);
    /// `LV_STYLE_TRANSFORM_SKEW_Y`
    pub const TRANSFORM_SKEW_Y: StyleProp = StyleProp(158);
    /// `LV_STYLE_FLEX_FLOW`
    pub const FLEX_FLOW: StyleProp = StyleProp(160);
    /// `LV_STYLE_FLEX_MAIN_PLACE`
    pub const FLEX_MAIN_PLACE: StyleProp = StyleProp(161);
    /// `LV_STYLE_FLEX_CROSS_PLACE`
    pub const FLEX_CROSS_PLACE: StyleProp = StyleProp(162);
    /// `LV_STYLE_FLEX_TRACK_PLACE`
    pub const FLEX_TRACK_PLACE: StyleProp = StyleProp(163);
    /// `LV_STYLE_FLEX_GROW`
    pub const FLEX_GROW: StyleProp = StyleProp(164);
    /// `LV_STYLE_GRID_COLUMN_DSC_ARRAY`
    pub const GRID_COLUMN_DSC_ARRAY: StyleProp = StyleProp(165);
    /// `LV_STYLE_GRID_ROW_DSC_ARRAY`
    pub const GRID_ROW_DSC_ARRAY: StyleProp = StyleProp(166);
    /// `LV_STYLE_GRID_COLUMN_ALIGN`
    pub const GRID_COLUMN_ALIGN: StyleProp = StyleProp(168);
    /// `LV_STYLE_GRID_ROW_ALIGN`
    pub const GRID_ROW_ALIGN: StyleProp = StyleProp(169);
    /// `LV_STYLE_GRID_CELL_COLUMN_POS`
    pub const GRID_CELL_COLUMN_POS: StyleProp = StyleProp(170);
    /// `LV_STYLE_GRID_CELL_COLUMN_SPAN`
    pub const GRID_CELL_COLUMN_SPAN: StyleProp = StyleProp(171);
    /// `LV_STYLE_GRID_CELL_X_ALIGN`
    pub const GRID_CELL_X_ALIGN: StyleProp = StyleProp(172);
    /// `LV_STYLE_GRID_CELL_ROW_POS`
    pub const GRID_CELL_ROW_POS: StyleProp = StyleProp(173);
    /// `LV_STYLE_GRID_CELL_ROW_SPAN`
    pub const GRID_CELL_ROW_SPAN: StyleProp = StyleProp(174);
    /// `LV_STYLE_GRID_CELL_Y_ALIGN`
    pub const GRID_CELL_Y_ALIGN: StyleProp = StyleProp(175);
    /// `LV_STYLE_TEXT_LEADING_TRIM`
    pub const TEXT_LEADING_TRIM: StyleProp = StyleProp(176);
    /// `LV_STYLE_LAST_BUILT_IN_PROP`
    pub const LAST_BUILT_IN: StyleProp = StyleProp(177);
    /// `LV_STYLE_PROP_ANY` (also `LV_STYLE_PROP_CONST`, the const-style marker).
    pub const ANY: StyleProp = StyleProp(0xFF);
    /// `LV_STYLE_PROP_CONST`: magic value marking a const style (same as `ANY`).
    pub const CONST: StyleProp = StyleProp(0xFF);
    /// `LV_STYLE_NUM_BUILT_IN_PROPS` (= `LV_STYLE_LAST_BUILT_IN_PROP + 1`).
    pub const NUM_BUILT_IN: u8 = 178;

    /// Raw property number.
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    /// Wrap a raw property number (the `(lv_style_prop_t)` cast).
    #[inline]
    #[must_use]
    pub const fn from_u8(v: u8) -> StyleProp {
        StyleProp(v)
    }

    /// `true` for `LV_STYLE_WIDTH ..= LV_STYLE_LAST_BUILT_IN_PROP`.
    #[inline]
    #[must_use]
    pub const fn is_built_in(self) -> bool {
        self.0 != 0 && self.0 < Self::NUM_BUILT_IN
    }

    /// `true` for an id handed out by `lv_style_register_prop()`.
    #[inline]
    #[must_use]
    pub const fn is_custom(self) -> bool {
        self.0 >= Self::NUM_BUILT_IN && self.0 != Self::ANY.0
    }
}

/// `LV_STYLE_NUM_BUILT_IN_PROPS`
pub const NUM_BUILT_IN_PROPS: usize = StyleProp::NUM_BUILT_IN as usize;

/// `lv_style_const_prop_id_inv`
pub const CONST_PROP_ID_INV: StyleProp = StyleProp::INV;

/// `lv_style_get_prop_group()`: group of a property. If a property from a
/// group is set in a style the `(1 << group)` bit of `has_group` is set.
/// Returns 0..=31; 31 marks all custom properties.
#[inline]
#[must_use]
pub const fn get_prop_group(prop: u8) -> u32 {
    let group = (prop as u32) >> 2;
    if group > 30 { 31 } else { group }
}

/// Flags of a built-in property as in `lv_style_builtin_prop_flag_lookup_table`
/// (`src/misc/lv_style.c`). Values outside the table yield 0; callers go
/// through [`lookup_flags`] for the `ANY` / `INV` / custom handling.
#[must_use]
pub const fn builtin_prop_flags(prop: StyleProp) -> u8 {
    use StyleProp as P;
    const L: u8 = STYLE_PROP_FLAG_LAYOUT_UPDATE;
    const E: u8 = STYLE_PROP_FLAG_EXT_DRAW_UPDATE;
    const I: u8 = STYLE_PROP_FLAG_INHERITABLE;
    const PL: u8 = STYLE_PROP_FLAG_PARENT_LAYOUT_UPDATE;
    const LY: u8 = STYLE_PROP_FLAG_LAYER_UPDATE;
    const T: u8 = STYLE_PROP_FLAG_TRANSFORM;
    match prop {
        P::WIDTH | P::MIN_WIDTH | P::MAX_WIDTH | P::HEIGHT | P::MIN_HEIGHT | P::MAX_HEIGHT => L,
        P::LENGTH => E,
        P::X | P::Y | P::ALIGN => L,
        P::TRANSFORM_WIDTH | P::TRANSFORM_HEIGHT => E | T,
        P::TRANSLATE_X | P::TRANSLATE_Y => L | PL,
        P::TRANSFORM_SCALE_X
        | P::TRANSFORM_SCALE_Y
        | P::TRANSFORM_SKEW_X
        | P::TRANSFORM_SKEW_Y
        | P::TRANSFORM_ROTATION => E | LY | T,
        P::PAD_TOP | P::PAD_BOTTOM | P::PAD_LEFT | P::PAD_RIGHT | P::PAD_ROW | P::PAD_COLUMN => E | L,
        P::MARGIN_TOP | P::MARGIN_BOTTOM | P::MARGIN_LEFT | P::MARGIN_RIGHT => E | L,
        P::BG_IMAGE_SRC => E,
        P::BORDER_WIDTH => L,
        P::OUTLINE_WIDTH | P::OUTLINE_OPA | P::OUTLINE_PAD => E,
        P::SHADOW_WIDTH | P::SHADOW_OFFSET_X | P::SHADOW_OFFSET_Y | P::SHADOW_SPREAD | P::SHADOW_OPA => E,
        P::LINE_WIDTH => E,
        P::ARC_WIDTH => E,
        P::TEXT_COLOR | P::TEXT_OPA | P::TEXT_DECOR => I,
        P::TEXT_FONT | P::TEXT_LETTER_SPACE | P::TEXT_LINE_SPACE | P::TEXT_ALIGN => I | L,
        P::OPA_LAYERED => LY,
        P::COLOR_FILTER_DSC | P::COLOR_FILTER_OPA => I,
        P::BLEND_MODE => LY,
        P::LAYOUT => L,
        P::BASE_DIR => I | L,
        P::BITMAP_MASK_SRC => LY,
        P::DROP_SHADOW_RADIUS | P::DROP_SHADOW_OFFSET_X | P::DROP_SHADOW_OFFSET_Y | P::DROP_SHADOW_OPA => E,
        #[cfg(feature = "flex")] /*LV_USE_FLEX*/
        P::FLEX_FLOW | P::FLEX_MAIN_PLACE | P::FLEX_CROSS_PLACE | P::FLEX_TRACK_PLACE | P::FLEX_GROW => L,
        #[cfg(feature = "grid")] /*LV_USE_GRID*/
        P::GRID_COLUMN_DSC_ARRAY
        | P::GRID_ROW_DSC_ARRAY
        | P::GRID_COLUMN_ALIGN
        | P::GRID_ROW_ALIGN
        | P::GRID_CELL_ROW_SPAN
        | P::GRID_CELL_ROW_POS
        | P::GRID_CELL_COLUMN_SPAN
        | P::GRID_CELL_COLUMN_POS
        | P::GRID_CELL_X_ALIGN
        | P::GRID_CELL_Y_ALIGN => L,
        P::TEXT_LEADING_TRIM => I | L,
        _ => STYLE_PROP_FLAG_NONE,
    }
}

/// `lv_style_builtin_prop_flag_lookup_table`, indexed by property number.
pub const BUILTIN_PROP_FLAG_LOOKUP_TABLE: [u8; NUM_BUILT_IN_PROPS] = {
    let mut table = [0u8; NUM_BUILT_IN_PROPS];
    let mut i = 0;
    while i < NUM_BUILT_IN_PROPS {
        table[i] = builtin_prop_flags(StyleProp(i as u8));
        i += 1;
    }
    table
};

/// `lv_style_prop_lookup_flags()` for built-in properties
/// (`src/misc/lv_style.c`): `LV_STYLE_PROP_ANY` has every flag,
/// `LV_STYLE_PROP_INV` none, then the built-in table.
///
/// Custom properties are looked up through
/// [`crate::core::Ui::style_prop_lookup_flags`], which has the same early
/// returns and then consults the custom table.
#[inline]
#[must_use]
pub const fn lookup_flags(prop: StyleProp) -> u8 {
    if prop.0 == StyleProp::ANY.0 {
        return STYLE_PROP_FLAG_ALL; /*Any prop can have any flags*/
    }
    if prop.0 == StyleProp::INV.0 {
        return 0;
    }
    if prop.0 < StyleProp::NUM_BUILT_IN {
        return builtin_prop_flags(prop);
    }
    0
}

/// `lv_style_prop_has_flag()`
#[inline]
#[must_use]
pub const fn prop_has_flag(prop: StyleProp, flag: u8) -> bool {
    lookup_flags(prop) & flag != 0
}

/// `lv_style_register_prop()`: register a custom property with `flag` and
/// return its id (`StyleProp::INV` on allocation failure in C, which cannot
/// happen here).
///
/// There is no global state, so the two `lv_global_t` members are passed in
/// explicitly: `custom_table` is `style_custom_prop_flag_lookup_table` (its
/// length is `style_custom_table_size`; an empty table is the C `NULL`) and
/// `last_custom_prop_id` is `style_last_custom_prop_id`. They live in the
/// context as `crate::core::Ui::style_custom_prop_flags` and
/// `crate::core::Ui::style_last_custom_prop_id`, and
/// `Ui::style_register_prop` is a thin wrapper around this function.
///
/// The table grows in 32-entry steps exactly like the C `lv_realloc` path
/// (`test_custom_prop_ids` checks that) and the returned id is the
/// `(lv_style_prop_t)` (8-bit) truncation of the counter, as upstream.
#[must_use]
pub fn register_prop(custom_table: &mut Vec<u8>, last_custom_prop_id: &mut u32, flag: u8) -> StyleProp {
    if custom_table.is_empty() {
        /*lv_style_custom_prop_flag_lookup_table == NULL: size 0, restart the ids*/
        *last_custom_prop_id = StyleProp::LAST_BUILT_IN.0 as u32;
    }

    /*
     * Allocate the lookup table if it's not yet available.
     */
    let mut required_size = (*last_custom_prop_id + 1 - StyleProp::LAST_BUILT_IN.0 as u32) as usize;
    if custom_table.len() < required_size {
        /* Round required_size up to the nearest 32-byte value */
        required_size = (required_size + 31) & !31;
        custom_table.resize(required_size, 0);
    }
    *last_custom_prop_id += 1;
    /* This should never happen - we should bail out above */
    custom_table[(*last_custom_prop_id - StyleProp::NUM_BUILT_IN as u32) as usize] = flag;
    StyleProp(*last_custom_prop_id as u8)
}

/// `lv_style_get_num_custom_props()`: number of custom properties registered
/// so far, derived from the `style_last_custom_prop_id` counter (see
/// [`register_prop`]).
///
/// The `uint32_t` difference is narrowed to `lv_style_prop_t` (8 bits) like
/// the C return type, so it agrees with the ids [`register_prop`] returns
/// once more than 255 custom properties exist. The narrowed value is handed
/// back in a `u32` to keep `Ui::style_get_num_custom_props` unchanged.
#[inline]
#[must_use]
pub const fn get_num_custom_props(last_custom_prop_id: u32) -> u32 {
    last_custom_prop_id.wrapping_sub(StyleProp::LAST_BUILT_IN.0 as u32) as u8 as u32
}

/// `lv_style_prop_lookup_flags()` including custom properties: the same
/// `ANY` / `INV` early returns as [`lookup_flags`], then the built-in table,
/// then `custom_table` (the `style_custom_prop_flag_lookup_table` slice kept
/// in the `Ui`; an empty slice is the C `NULL` table). Unknown ids yield 0.
#[must_use]
pub const fn lookup_flags_with_custom(prop: StyleProp, custom_table: &[u8]) -> u8 {
    if prop.0 == StyleProp::ANY.0 {
        return STYLE_PROP_FLAG_ALL; /*Any prop can have any flags*/
    }
    if prop.0 == StyleProp::INV.0 {
        return 0;
    }

    if prop.0 < StyleProp::NUM_BUILT_IN {
        return builtin_prop_flags(prop);
    }
    let prop = (prop.0 - StyleProp::NUM_BUILT_IN) as usize;
    if prop < custom_table.len() {
        return custom_table[prop];
    }
    0
}

/// `lv_style_prop_has_flag()` including custom properties, see
/// [`lookup_flags_with_custom`].
#[inline]
#[must_use]
pub const fn prop_has_flag_with_custom(prop: StyleProp, flag: u8, custom_table: &[u8]) -> bool {
    lookup_flags_with_custom(prop, custom_table) & flag != 0
}

/// A `'static` or reference-counted pointer to shared, immutable data.
///
/// Mirrors the `const void * ptr` member of `lv_style_value_t`: equality is
/// pointer identity, never a deep comparison.
pub enum SharedRef<T: ?Sized + 'static> {
    /// Data in static memory.
    Static(&'static T),
    /// Reference-counted data.
    Shared(Rc<T>),
}

impl<T: ?Sized> SharedRef<T> {
    /// Address of the data.
    #[inline]
    #[must_use]
    pub fn addr(&self) -> usize {
        match self {
            SharedRef::Static(p) => (*p as *const T).cast::<u8>() as usize,
            SharedRef::Shared(p) => Rc::as_ptr(p).cast::<u8>() as usize,
        }
    }
}

impl<T: ?Sized> Clone for SharedRef<T> {
    fn clone(&self) -> Self {
        match self {
            SharedRef::Static(p) => SharedRef::Static(p),
            SharedRef::Shared(p) => SharedRef::Shared(p.clone()),
        }
    }
}

impl<T: ?Sized> core::ops::Deref for SharedRef<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        match self {
            SharedRef::Static(p) => p,
            SharedRef::Shared(p) => p,
        }
    }
}

impl<T: ?Sized> PartialEq for SharedRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.addr() == other.addr()
    }
}

impl<T: ?Sized> Eq for SharedRef<T> {}

impl<T: ?Sized> core::fmt::Debug for SharedRef<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "SharedRef({:#x})", self.addr())
    }
}

/// `const lv_grad_dsc_t *` in a style.
pub type GradRef = SharedRef<GradDsc>;
/// `const lv_style_transition_dsc_t *` in a style.
pub type TransitionRef = SharedRef<StyleTransitionDsc>;
/// `const int32_t *` grid track descriptor (terminated by `LV_GRID_TEMPLATE_LAST`).
pub type TracksRef = SharedRef<[i32]>;
/// `const lv_anim_t *` template in a style (`LV_STYLE_ANIM`).
pub type AnimRef = SharedRef<Anim>;
/// A `'static` or reference-counted string (file path, symbol text).
pub type SharedStr = SharedRef<str>;

/// An image source (`const void * src`): `lv_image_src_get_type()` decides
/// between a variable, a file path and a symbol; here the variant does.
///
/// Equality is pointer identity for descriptors (like comparing `src`
/// pointers in C) and string equality for paths and symbols.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub enum ImageSrc {
    /// No image (`NULL`).
    #[default]
    None,
    /// `LV_IMAGE_SRC_VARIABLE`: an image descriptor in static memory.
    Static(&'static ImageDsc),
    /// `LV_IMAGE_SRC_VARIABLE`: a reference-counted image descriptor.
    Shared(Rc<ImageDsc>),
    /// `LV_IMAGE_SRC_VARIABLE`: a draw buffer (runtime created image, canvas).
    DrawBuf(Rc<DrawBuf>),
    /// `LV_IMAGE_SRC_FILE`: a path on a registered file system (`"A:img.bin"`).
    Path(SharedStr),
    /// `LV_IMAGE_SRC_SYMBOL`: a font symbol / text drawn as an image.
    Symbol(SharedStr),
    /// A draw layer used as the source of a `LV_DRAW_TASK_TYPE_LAYER` task.
    Layer(crate::draw::LayerId),
}

impl PartialEq for ImageSrc {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ImageSrc::None, ImageSrc::None) => true,
            (ImageSrc::Static(a), ImageSrc::Static(b)) => core::ptr::eq(*a, *b),
            (ImageSrc::Shared(a), ImageSrc::Shared(b)) => Rc::ptr_eq(a, b),
            (ImageSrc::Static(a), ImageSrc::Shared(b)) | (ImageSrc::Shared(b), ImageSrc::Static(a)) => {
                core::ptr::eq(*a, Rc::as_ptr(b))
            }
            (ImageSrc::DrawBuf(a), ImageSrc::DrawBuf(b)) => Rc::ptr_eq(a, b),
            (ImageSrc::Path(a), ImageSrc::Path(b)) => a == b,
            (ImageSrc::Symbol(a), ImageSrc::Symbol(b)) => a == b,
            (ImageSrc::Layer(a), ImageSrc::Layer(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for ImageSrc {}

impl ImageSrc {
    /// `lv_image_src_get_type()`
    #[must_use]
    pub fn src_type(&self) -> ImageSrcType {
        match self {
            ImageSrc::None => ImageSrcType::Unknown,
            ImageSrc::Static(_) | ImageSrc::Shared(_) | ImageSrc::DrawBuf(_) | ImageSrc::Layer(_) => {
                ImageSrcType::Variable
            }
            ImageSrc::Path(_) => ImageSrcType::File,
            ImageSrc::Symbol(_) => ImageSrcType::Symbol,
        }
    }

    /// `true` for [`ImageSrc::None`].
    #[inline]
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self, ImageSrc::None)
    }
}

/// `lv_image_src_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ImageSrcType {
    /// `LV_IMAGE_SRC_VARIABLE`: binary / C variable.
    Variable = 0,
    /// `LV_IMAGE_SRC_FILE`: file in a file system.
    File = 1,
    /// `LV_IMAGE_SRC_SYMBOL`: symbol (see `lv_symbol_def.h`).
    Symbol = 2,
    /// `LV_IMAGE_SRC_UNKNOWN`
    #[default]
    Unknown = 3,
}

/// `lv_style_value_t`: the typed value of a style property.
///
/// `Num` keeps the `LV_PCT()` / `LV_SIZE_CONTENT` coordinate encoding;
/// enums, opacities and booleans are stored as `Num` exactly like the C
/// union. Pointer kinds compare by identity.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum StyleValue {
    /// `.ptr = NULL`: an unset pointer value.
    #[default]
    Null,
    /// `.num`: integer, opacity, enum or boolean.
    Num(i32),
    /// `.color`
    Color(Color),
    /// `.ptr` pointing at an `lv_font_t`.
    Font(FontRef),
    /// `.ptr` pointing at an `lv_grad_dsc_t`.
    Grad(GradRef),
    /// `.ptr` image source (`LV_STYLE_BG_IMAGE_SRC`, `LV_STYLE_ARC_IMAGE_SRC`, `LV_STYLE_BITMAP_MASK_SRC`).
    Image(ImageSrc),
    /// `.ptr` pointing at an `lv_style_transition_dsc_t`.
    Transition(TransitionRef),
    /// `.ptr` pointing at an `lv_color_filter_dsc_t`.
    ColorFilter(ColorFilter),
    /// `.ptr` pointing at a grid track array.
    Tracks(TracksRef),
    /// `.ptr` pointing at an `lv_anim_t` template.
    Anim(AnimRef),
    /// `.ptr` pointing at an `lv_image_colorkey_t` (stored by value).
    Colorkey(ImageColorkey),
    /// A string value (for custom properties).
    Str(Rc<str>),
}

impl StyleValue {
    /// `.num` of the `lv_style_value_t` union.
    ///
    /// A `.color` value reads back as its three bytes in `lv_color_t` memory
    /// order (`blue | green << 8 | red << 16`, see `lv_color_to_int()`), as
    /// the C union does. `NULL` and the pointer kinds, whose addresses have
    /// no Rust equivalent, read as 0.
    #[inline]
    #[must_use]
    pub const fn num(&self) -> i32 {
        match self {
            StyleValue::Num(n) => *n,
            StyleValue::Color(c) => c.to_int() as i32,
            _ => 0,
        }
    }

    /// `.color` of the `lv_style_value_t` union.
    ///
    /// A `.num` value reads back as its low three bytes in `lv_color_t`
    /// memory order (blue, green, red), as the C union does. `NULL` and the
    /// pointer kinds read as black.
    #[inline]
    #[must_use]
    pub const fn color(&self) -> Color {
        match self {
            StyleValue::Color(c) => *c,
            StyleValue::Num(n) => Color {
                blue: (*n & 0xFF) as u8,
                green: ((*n >> 8) & 0xFF) as u8,
                red: ((*n >> 16) & 0xFF) as u8,
            },
            _ => Color::BLACK,
        }
    }

    /// `true` for the C `.ptr == NULL` case.
    #[inline]
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, StyleValue::Null)
    }
}

/// `lv_style_const_prop_t` / one entry of a style: a property and its value.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleEntry {
    /// The property.
    pub prop: StyleProp,
    /// Its value.
    pub value: StyleValue,
}

impl StyleEntry {
    /// Build an entry (`{ .prop = prop, .value = value }`).
    #[must_use]
    pub const fn new(prop: StyleProp, value: StyleValue) -> Self {
        Self { prop, value }
    }
}

/// `lv_style_t`: a collection of properties and values.
///
/// `has_group` has bit `lv_style_get_prop_group(prop)` set for every stored
/// property so that lookups can skip the style early.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    /// `values_and_props`
    pub props: Vec<StyleEntry>,
    /// `has_group`
    pub has_group: u32,
}

/// Image source usable in a `static` constant style.
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub enum ConstImageSrc {
    /// `NULL`
    None,
    /// An image descriptor in static memory.
    Static(&'static ImageDsc),
    /// A file path.
    Path(&'static str),
    /// A symbol / text.
    Symbol(&'static str),
}

impl ConstImageSrc {
    /// Convert to a runtime [`ImageSrc`] (no allocation).
    #[must_use]
    pub fn to_image_src(self) -> ImageSrc {
        match self {
            ConstImageSrc::None => ImageSrc::None,
            ConstImageSrc::Static(d) => ImageSrc::Static(d),
            ConstImageSrc::Path(p) => ImageSrc::Path(SharedRef::Static(p)),
            ConstImageSrc::Symbol(s) => ImageSrc::Symbol(SharedRef::Static(s)),
        }
    }
}

/// `lv_style_value_t` restricted to kinds that can live in a `static`
/// (`Sync`) constant style. Transitions and animation templates hold
/// reference-counted data and are therefore not const-constructible.
#[derive(Copy, Clone)]
#[non_exhaustive]
pub enum ConstStyleValue {
    /// `.ptr = NULL`
    Null,
    /// `.num`
    Num(i32),
    /// `.color`
    Color(Color),
    /// A static font.
    Font(&'static (dyn Font + Sync)),
    /// A static gradient descriptor.
    Grad(&'static GradDsc),
    /// A static image source.
    Image(ConstImageSrc),
    /// A colour filter.
    ColorFilter(ColorFilter),
    /// A static grid track array.
    Tracks(&'static [i32]),
    /// A colour key range.
    Colorkey(ImageColorkey),
}

impl core::fmt::Debug for ConstStyleValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ConstStyleValue::Null => f.write_str("Null"),
            ConstStyleValue::Num(n) => f.debug_tuple("Num").field(n).finish(),
            ConstStyleValue::Color(c) => f.debug_tuple("Color").field(c).finish(),
            ConstStyleValue::Font(p) => {
                write!(f, "Font({:p})", (*p as *const (dyn Font + Sync)).cast::<u8>())
            }
            ConstStyleValue::Grad(g) => f.debug_tuple("Grad").field(g).finish(),
            ConstStyleValue::Image(i) => f.debug_tuple("Image").field(i).finish(),
            ConstStyleValue::ColorFilter(c) => f.debug_tuple("ColorFilter").field(c).finish(),
            ConstStyleValue::Tracks(t) => f.debug_tuple("Tracks").field(t).finish(),
            ConstStyleValue::Colorkey(k) => f.debug_tuple("Colorkey").field(k).finish(),
        }
    }
}

impl ConstStyleValue {
    /// Convert to a runtime [`StyleValue`] (no allocation).
    #[must_use]
    pub fn to_value(self) -> StyleValue {
        match self {
            ConstStyleValue::Null => StyleValue::Null,
            ConstStyleValue::Num(n) => StyleValue::Num(n),
            ConstStyleValue::Color(c) => StyleValue::Color(c),
            ConstStyleValue::Font(f) => StyleValue::Font(FontRef::Static(f)),
            ConstStyleValue::Grad(g) => StyleValue::Grad(SharedRef::Static(g)),
            ConstStyleValue::Image(i) => StyleValue::Image(i.to_image_src()),
            ConstStyleValue::ColorFilter(f) => StyleValue::ColorFilter(f),
            ConstStyleValue::Tracks(t) => StyleValue::Tracks(SharedRef::Static(t)),
            ConstStyleValue::Colorkey(k) => StyleValue::Colorkey(k),
        }
    }
}

/// `lv_style_const_prop_t`: one entry of a constant style
/// (`LV_STYLE_CONST_<PROP>(value)`).
#[derive(Copy, Clone, Debug)]
pub struct ConstStyleEntry {
    /// The property.
    pub prop: StyleProp,
    /// Its value.
    pub value: ConstStyleValue,
}

impl ConstStyleEntry {
    /// Build an entry.
    #[must_use]
    pub const fn new(prop: StyleProp, value: ConstStyleValue) -> Self {
        Self { prop, value }
    }

    /// Convert to a runtime [`StyleEntry`].
    #[must_use]
    pub fn to_entry(&self) -> StyleEntry {
        StyleEntry {
            prop: self.prop,
            value: self.value.to_value(),
        }
    }
}

/// A constant style (`LV_STYLE_CONST_INIT`): properties in static memory.
#[derive(Copy, Clone, Debug, Default)]
pub struct ConstStyle {
    /// The property array (`LV_STYLE_CONST_PROPS_END` is not needed).
    pub props: &'static [ConstStyleEntry],
}

impl PartialEq for ConstStyle {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.props, other.props)
    }
}

impl ConstStyle {
    /// `LV_STYLE_CONST_INIT(name, props)`
    #[must_use]
    pub const fn new(props: &'static [ConstStyleEntry]) -> Self {
        Self { props }
    }

    /// `has_group` of a const style is `0xFFFFFFFF`.
    pub const HAS_GROUP: u32 = 0xFFFF_FFFF;

    /// `prop_cnt` of a const style: `LV_STYLE_CONST_INIT` always sets 255.
    pub const PROP_CNT: u8 = 255;

    /// Number of entries in [`ConstStyle::props`] (for iteration). This is
    /// not `prop_cnt`, which is always [`ConstStyle::PROP_CNT`].
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.props.len()
    }

    /// `lv_style_is_empty()`: `prop_cnt == 0`, so always `false` for a const
    /// style (`prop_cnt` is 255), even when the array holds no property.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        Self::PROP_CNT == 0
    }

    /// `lv_style_is_const()`: always `true`.
    #[inline]
    #[must_use]
    pub const fn is_const(&self) -> bool {
        true
    }

    /// `lv_style_get_prop()` / `lv_style_get_prop_internal()` on a constant
    /// style: a linear search from the first entry. An `LV_STYLE_PROP_INV`
    /// entry (`LV_STYLE_CONST_PROPS_END`) terminates the array like in C, so
    /// nothing after it is visible and `INV` itself is never found.
    #[must_use]
    pub fn get_prop(&self, prop: StyleProp) -> Option<StyleValue> {
        for entry in self.props {
            if entry.prop == StyleProp::INV {
                break;
            }
            if entry.prop == prop {
                return Some(entry.value.to_value());
            }
        }
        None
    }
}

/// `lv_style_res_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StyleRes {
    /// `LV_STYLE_RES_NOT_FOUND`
    #[default]
    NotFound = 0,
    /// `LV_STYLE_RES_FOUND`
    Found = 1,
}

/// `lv_style_transition_dsc_t`: descriptor for style transitions.
pub struct StyleTransitionDsc {
    /// `props`: the properties to animate.
    pub props: TracksLike,
    /// `path_xcb`: easing of the transition.
    pub path: AnimPath,
    /// `time`: duration in ms.
    pub time: u32,
    /// `delay`: delay before the transition in ms.
    pub delay: u32,
    /// `user_data`
    pub user_data: Option<Rc<dyn core::any::Any>>,
}

/// Property list of a transition: static or reference-counted.
pub type TracksLike = SharedRef<[StyleProp]>;

impl StyleTransitionDsc {
    /// `lv_style_transition_dsc_init()`
    #[must_use]
    pub fn new(props: TracksLike, path: AnimPath, time: u32, delay: u32) -> Self {
        Self {
            props,
            path,
            time,
            delay,
            user_data: None,
        }
    }

    /// `lv_style_transition_dsc_init()` for a static property list, usable in
    /// `static` initialisers.
    #[must_use]
    pub const fn new_static(props: &'static [StyleProp], path: AnimPath, time: u32, delay: u32) -> Self {
        Self {
            props: SharedRef::Static(props),
            path,
            time,
            delay,
            user_data: None,
        }
    }

    /// `lv_style_transition_dsc_init()` on an existing descriptor, with the
    /// full C parameter list: `path` `None` is `path_cb == NULL` and selects
    /// `lv_anim_path_linear`.
    pub fn init(
        &mut self,
        props: TracksLike,
        path: Option<AnimPath>,
        time: u32,
        delay: u32,
        user_data: Option<Rc<dyn core::any::Any>>,
    ) {
        /*lv_memzero(tr, sizeof(lv_style_transition_dsc_t)); then every member is assigned*/
        self.props = props;
        self.path = path.unwrap_or(AnimPath::Linear);
        self.time = time;
        self.delay = delay;
        self.user_data = user_data;
    }
}

impl core::fmt::Debug for StyleTransitionDsc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StyleTransitionDsc")
            .field("props", &&*self.props)
            .field("time", &self.time)
            .field("delay", &self.delay)
            .finish_non_exhaustive()
    }
}

bitflags::bitflags! {
    /// `lv_state_t`
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct State: u16 {
        /// `LV_STATE_ALT`
        const ALT = 1 << 0;
        /// `LV_STATE_CHECKED`
        const CHECKED = 1 << 2;
        /// `LV_STATE_FOCUSED`
        const FOCUSED = 1 << 3;
        /// `LV_STATE_FOCUS_KEY`
        const FOCUS_KEY = 1 << 4;
        /// `LV_STATE_EDITED`
        const EDITED = 1 << 5;
        /// `LV_STATE_HOVERED`
        const HOVERED = 1 << 6;
        /// `LV_STATE_PRESSED`
        const PRESSED = 1 << 7;
        /// `LV_STATE_SCROLLED`
        const SCROLLED = 1 << 8;
        /// `LV_STATE_DISABLED`
        const DISABLED = 1 << 9;
        /// `LV_STATE_USER_1`
        const USER_1 = 1 << 12;
        /// `LV_STATE_USER_2`
        const USER_2 = 1 << 13;
        /// `LV_STATE_USER_3`
        const USER_3 = 1 << 14;
        /// `LV_STATE_USER_4`
        const USER_4 = 1 << 15;
        /// `LV_STATE_ANY`
        const ANY = 0xFFFF;
    }
}

impl State {
    /// `LV_STATE_DEFAULT`
    pub const DEFAULT: State = State::empty();
}

/// `lv_part_t`: the part of an object a style applies to.
///
/// An open `uint32_t` like upstream: only the constants below are named and
/// custom widgets use [`Part::custom`] (`LV_PART_CUSTOM_FIRST + n`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Part(pub u32);

impl Part {
    /// `LV_PART_MAIN`: background-like rectangle.
    pub const MAIN: Part = Part(0x000000);
    /// `LV_PART_SCROLLBAR`
    pub const SCROLLBAR: Part = Part(0x010000);
    /// `LV_PART_INDICATOR`
    pub const INDICATOR: Part = Part(0x020000);
    /// `LV_PART_KNOB`
    pub const KNOB: Part = Part(0x030000);
    /// `LV_PART_SELECTED`
    pub const SELECTED: Part = Part(0x040000);
    /// `LV_PART_ITEMS`
    pub const ITEMS: Part = Part(0x050000);
    /// `LV_PART_CURSOR`
    pub const CURSOR: Part = Part(0x060000);
    /// `LV_PART_CUSTOM_FIRST`: extension point for custom widgets.
    pub const CUSTOM_FIRST: Part = Part(0x080000);
    /// `LV_PART_ANY`: special value, can be used in some functions to target all parts.
    pub const ANY: Part = Part(0x0F0000);

    /// `LV_PART_CUSTOM_FIRST + (n << 16)`: the `n`-th custom part.
    #[inline]
    #[must_use]
    pub const fn custom(n: u32) -> Part {
        Part(Self::CUSTOM_FIRST.0 + (n << 16))
    }

    /// Wrap a raw part value (`selector & 0xFF0000`).
    #[inline]
    #[must_use]
    pub const fn from_u32(v: u32) -> Part {
        Part(v & 0xFF0000)
    }

    /// Raw value.
    #[inline]
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// `lv_style_selector_t`: `part | state`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct StyleSelector(pub u32);

impl StyleSelector {
    /// `LV_PART_MAIN | LV_STATE_DEFAULT`
    pub const DEFAULT: StyleSelector = StyleSelector(0);

    /// Build a selector from a part and a state.
    #[inline]
    #[must_use]
    pub const fn new(part: Part, state: State) -> Self {
        StyleSelector(part.0 | state.bits() as u32)
    }

    /// `lv_obj_style_get_selector_part()`: `selector & 0xFF0000`, unchanged.
    #[inline]
    #[must_use]
    pub const fn part(self) -> Part {
        Part(self.0 & 0xFF0000)
    }

    /// `selector & 0xFF0000` without decoding.
    #[inline]
    #[must_use]
    pub const fn part_raw(self) -> u32 {
        self.0 & 0xFF0000
    }

    /// `lv_obj_style_get_selector_state()`
    #[inline]
    #[must_use]
    pub const fn state(self) -> State {
        State::from_bits_retain((self.0 & 0xFFFF) as u16)
    }
}

impl From<Part> for StyleSelector {
    fn from(p: Part) -> Self {
        StyleSelector(p.0)
    }
}

impl From<State> for StyleSelector {
    fn from(s: State) -> Self {
        StyleSelector(s.bits() as u32)
    }
}

impl core::ops::BitOr<State> for Part {
    type Output = StyleSelector;
    fn bitor(self, rhs: State) -> StyleSelector {
        StyleSelector::new(self, rhs)
    }
}

impl core::ops::BitOr<Part> for State {
    type Output = StyleSelector;
    fn bitor(self, rhs: Part) -> StyleSelector {
        StyleSelector::new(rhs, self)
    }
}

impl Style {
    /// `lv_style_init()`: an empty style.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            props: Vec::new(),
            has_group: 0,
        }
    }

    /// `lv_style_reset()`: remove every property. Frees the storage and
    /// clears `has_group` (`lv_free` + `lv_memzero`).
    pub fn reset(&mut self) {
        *self = Style::new();
    }

    /// `lv_style_copy()`: replace the content of `self` with a copy of `src`.
    pub fn copy_from(&mut self, src: &Style) {
        self.reset();

        self.merge_from(src);
    }

    /// `lv_style_merge()`: copy the properties of `src` into `self` without
    /// resetting it first. Properties already in `self` are overwritten in
    /// place, new ones are appended in `src` order (it is a `set_prop` loop).
    pub fn merge_from(&mut self, src: &Style) {
        /*Source is empty*/
        if src.props.is_empty() {
            return;
        }

        /* Merge the styles */
        for entry in &src.props {
            self.set_prop(entry.prop, entry.value.clone());
        }
    }

    /// `lv_style_merge()` with a constant source
    /// (the `lv_style_is_const_internal(src)` branch): an `LV_STYLE_PROP_INV`
    /// entry terminates the source array.
    pub fn merge_from_const(&mut self, src: &ConstStyle) {
        for entry in src.props {
            if entry.prop == StyleProp::INV {
                break;
            }
            self.set_prop(entry.prop, entry.value.to_value());
        }
    }

    /// `lv_style_copy()` with a constant source.
    pub fn copy_from_const(&mut self, src: &ConstStyle) {
        self.reset();

        self.merge_from_const(src);
    }

    /// `lv_style_is_const()`: always `false`; constant styles are [`ConstStyle`].
    #[inline]
    #[must_use]
    pub const fn is_const(&self) -> bool {
        false
    }

    /// `lv_style_remove_prop()`: returns `true` if the property was found and
    /// removed. The other properties keep their order (C rebuilds the array
    /// skipping the removed entry) and `has_group` is left as is, like
    /// upstream.
    pub fn remove_prop(&mut self, prop: StyleProp) -> bool {
        if self.props.is_empty() {
            return false;
        }

        match self.props.iter().position(|entry| entry.prop == prop) {
            Some(i) => {
                self.props.remove(i);
                true
            }
            None => false,
        }
    }

    /// `lv_style_set_prop()`: set or replace a property. The list is searched
    /// from the end; a hit is overwritten in place (position and `has_group`
    /// unchanged), otherwise the entry is appended and the property's group
    /// bit is set. `StyleProp::INV` is rejected (`LV_CHECK_ARG`).
    pub fn set_prop(&mut self, prop: StyleProp, value: StyleValue) {
        if prop == StyleProp::INV {
            check_arg_failed("prop != LV_STYLE_PROP_INV");
            return;
        }

        for entry in self.props.iter_mut().rev() {
            if entry.prop == prop {
                entry.value = value;
                return;
            }
        }

        /*Set the new property and value*/
        self.props.push(StyleEntry { prop, value });

        let group = get_prop_group(prop.0);
        self.has_group |= 1u32 << group;
    }

    /// `lv_style_get_prop()` / `lv_style_get_prop_internal()`: the entry with
    /// `prop`, searched from the start (`None` is `LV_STYLE_RES_NOT_FOUND`).
    #[must_use]
    pub fn get_prop(&self, prop: StyleProp) -> Option<&StyleValue> {
        self.props
            .iter()
            .find(|entry| entry.prop == prop)
            .map(|entry| &entry.value)
    }

    /// `lv_style_is_empty()`
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.props.is_empty()
    }

    /// `prop_cnt`
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.props.len()
    }

    /// `lv_style_prop_get_default()`. `default_font` stands in for
    /// `LV_FONT_DEFAULT` (it is not a compile-time constant in Rust).
    #[must_use]
    pub fn prop_get_default(prop: StyleProp, default_font: Option<&FontRef>) -> StyleValue {
        use StyleProp as P;
        match prop {
            P::TRANSFORM_SCALE_X | P::TRANSFORM_SCALE_Y => StyleValue::Num(SCALE_NONE),
            P::BG_COLOR => StyleValue::Color(Color::WHITE),
            P::BG_GRAD_COLOR
            | P::BORDER_COLOR
            | P::SHADOW_COLOR
            | P::OUTLINE_COLOR
            | P::ARC_COLOR
            | P::LINE_COLOR
            | P::TEXT_COLOR
            | P::DROP_SHADOW_COLOR
            | P::IMAGE_RECOLOR
            | P::RECOLOR => StyleValue::Color(Color::BLACK),
            P::OPA
            | P::OPA_LAYERED
            | P::BORDER_OPA
            | P::TEXT_OPA
            | P::IMAGE_OPA
            | P::BG_GRAD_OPA
            | P::BG_MAIN_OPA
            | P::BG_IMAGE_OPA
            | P::OUTLINE_OPA
            | P::LINE_OPA
            | P::ARC_OPA
            | P::SHADOW_OPA => StyleValue::Num(OPA_COVER as i32),
            P::BG_GRAD_STOP => StyleValue::Num(255),
            P::BORDER_SIDE => StyleValue::Num(BorderSide::FULL.bits() as i32),
            P::TEXT_FONT => match default_font {
                Some(f) => StyleValue::Font(f.clone()),
                None => StyleValue::Null,
            },
            P::MAX_WIDTH | P::MAX_HEIGHT => StyleValue::Num(coord::MAX),
            P::ROTARY_SENSITIVITY => StyleValue::Num(256),
            P::DROP_SHADOW_QUALITY => StyleValue::Num(BlurQuality::Precision as i32),
            #[cfg(feature = "grid")]
            P::GRID_CELL_ROW_SPAN | P::GRID_CELL_COLUMN_SPAN => StyleValue::Num(1),
            _ => StyleValue::Null,
        }
    }

    /// `lv_style_set_size()`
    pub fn set_size(&mut self, width: i32, height: i32) {
        self.set_width(width);
        self.set_height(height);
    }

    /// `lv_style_set_pad_all()`
    pub fn set_pad_all(&mut self, value: i32) {
        self.set_pad_left(value);
        self.set_pad_right(value);
        self.set_pad_top(value);
        self.set_pad_bottom(value);
    }

    /// `lv_style_set_pad_hor()`
    pub fn set_pad_hor(&mut self, value: i32) {
        self.set_pad_left(value);
        self.set_pad_right(value);
    }

    /// `lv_style_set_pad_ver()`
    pub fn set_pad_ver(&mut self, value: i32) {
        self.set_pad_top(value);
        self.set_pad_bottom(value);
    }

    /// `lv_style_set_pad_gap()`
    pub fn set_pad_gap(&mut self, value: i32) {
        self.set_pad_row(value);
        self.set_pad_column(value);
    }

    /// `lv_style_set_margin_hor()`
    pub fn set_margin_hor(&mut self, value: i32) {
        self.set_margin_left(value);
        self.set_margin_right(value);
    }

    /// `lv_style_set_margin_ver()`
    pub fn set_margin_ver(&mut self, value: i32) {
        self.set_margin_top(value);
        self.set_margin_bottom(value);
    }

    /// `lv_style_set_margin_all()`
    pub fn set_margin_all(&mut self, value: i32) {
        self.set_margin_left(value);
        self.set_margin_right(value);
        self.set_margin_top(value);
        self.set_margin_bottom(value);
    }

    /// `lv_style_set_transform_scale()`
    pub fn set_transform_scale(&mut self, value: i32) {
        self.set_transform_scale_x(value);
        self.set_transform_scale_y(value);
    }
}

/// A boxed style, used for per-object local and transition styles.
pub type StyleBox = Box<Style>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prop_numbers_match_upstream() {
        assert_eq!(StyleProp::WIDTH.0, 1);
        assert_eq!(StyleProp::MIN_WIDTH.0, 8);
        assert_eq!(StyleProp::BG_OPA.0, 72);
        assert_eq!(StyleProp::BG_COLOR.0, 73);
        assert_eq!(StyleProp::TEXT_FONT.0, 77);
        assert_eq!(StyleProp::RADIUS.0, 120);
        assert_eq!(StyleProp::FLEX_FLOW.0, 160);
        assert_eq!(StyleProp::TEXT_LEADING_TRIM.0, 176);
        assert_eq!(StyleProp::LAST_BUILT_IN.0, 177);
        assert_eq!(StyleProp::NUM_BUILT_IN, 178);
        assert_eq!(StyleProp::ANY.0, 0xFF);
        assert_eq!(StyleProp::CONST, StyleProp::ANY);
        assert_eq!(StyleProp::from_u8(73), StyleProp::BG_COLOR);
        assert!(StyleProp::from_u8(178).is_custom());
        assert!(!StyleProp::from_u8(178).is_built_in());
        assert!(!StyleProp::ANY.is_custom());
        assert!(!StyleProp::INV.is_built_in());
        assert_eq!(StyleProp::default(), StyleProp::INV);
        // Constants are usable as match patterns.
        let p = StyleProp(73);
        assert!(matches!(p, StyleProp::BG_COLOR));
    }

    #[test]
    fn flag_table_matches_lv_style_c() {
        assert_eq!(lookup_flags(StyleProp::WIDTH), STYLE_PROP_FLAG_LAYOUT_UPDATE);
        assert_eq!(
            lookup_flags(StyleProp::TRANSFORM_SCALE_X),
            STYLE_PROP_FLAG_EXT_DRAW_UPDATE | STYLE_PROP_FLAG_LAYER_UPDATE | STYLE_PROP_FLAG_TRANSFORM
        );
        assert_eq!(
            lookup_flags(StyleProp::TRANSLATE_X),
            STYLE_PROP_FLAG_LAYOUT_UPDATE | STYLE_PROP_FLAG_PARENT_LAYOUT_UPDATE
        );
        assert_eq!(lookup_flags(StyleProp::TEXT_COLOR), STYLE_PROP_FLAG_INHERITABLE);
        assert_eq!(
            lookup_flags(StyleProp::TEXT_FONT),
            STYLE_PROP_FLAG_INHERITABLE | STYLE_PROP_FLAG_LAYOUT_UPDATE
        );
        assert_eq!(lookup_flags(StyleProp::BG_COLOR), 0);
        assert_eq!(lookup_flags(StyleProp::RADIUS), 0);
        assert_eq!(
            BUILTIN_PROP_FLAG_LOOKUP_TABLE[StyleProp::PAD_TOP.0 as usize],
            0x06
        );
        assert_eq!(BUILTIN_PROP_FLAG_LOOKUP_TABLE[0], 0);
        assert!(prop_has_flag(StyleProp::BASE_DIR, STYLE_PROP_FLAG_INHERITABLE));
        // lv_style_prop_lookup_flags() early returns.
        assert_eq!(lookup_flags(StyleProp::ANY), STYLE_PROP_FLAG_ALL);
        assert_eq!(lookup_flags(StyleProp::ANY), 0x3F);
        assert_eq!(lookup_flags(StyleProp::INV), 0);
        assert_eq!(lookup_flags(StyleProp(200)), 0);
        assert!(prop_has_flag(StyleProp::ANY, STYLE_PROP_FLAG_LAYER_UPDATE));
        // Gaps between built-in numbers are 0 like the C table.
        assert_eq!(lookup_flags(StyleProp(6)), 0);
        // LV_USE_FLEX / LV_USE_GRID gate their rows of the table.
        #[cfg(feature = "flex")]
        assert_eq!(lookup_flags(StyleProp::FLEX_GROW), STYLE_PROP_FLAG_LAYOUT_UPDATE);
        #[cfg(not(feature = "flex"))]
        assert_eq!(lookup_flags(StyleProp::FLEX_GROW), 0);
        #[cfg(feature = "grid")]
        assert_eq!(
            lookup_flags(StyleProp::GRID_CELL_Y_ALIGN),
            STYLE_PROP_FLAG_LAYOUT_UPDATE
        );
        #[cfg(not(feature = "grid"))]
        assert_eq!(lookup_flags(StyleProp::GRID_CELL_Y_ALIGN), 0);
    }

    #[test]
    fn prop_groups() {
        assert_eq!(get_prop_group(StyleProp::WIDTH.0), 0);
        assert_eq!(get_prop_group(StyleProp::BG_COLOR.0), 18);
        assert_eq!(get_prop_group(StyleProp::TEXT_LEADING_TRIM.0), 31);
        assert_eq!(get_prop_group(200), 31);
    }

    #[test]
    fn selector_round_trip() {
        let s = StyleSelector::new(Part::KNOB, State::PRESSED | State::CHECKED);
        assert_eq!(s.0, 0x030000 | 0x84);
        assert_eq!(s.part(), Part::KNOB);
        assert_eq!(s.state(), State::PRESSED | State::CHECKED);
        assert_eq!((Part::INDICATOR | State::FOCUSED).0, 0x020008);
        assert_eq!(State::ANY.bits(), 0xFFFF);
        assert_eq!(Part::ANY.0, 0x0F0000);
        assert_eq!(StyleSelector::DEFAULT.part(), Part::MAIN);
    }

    #[test]
    fn defaults_match_lv_style_prop_get_default() {
        assert_eq!(
            Style::prop_get_default(StyleProp::BG_COLOR, None),
            StyleValue::Color(Color::WHITE)
        );
        // LV_STYLE_BG_OPA is *not* in the COVER list of lv_style_prop_get_default().
        assert_eq!(Style::prop_get_default(StyleProp::BG_OPA, None), StyleValue::Null);
        assert_eq!(
            Style::prop_get_default(StyleProp::BORDER_OPA, None),
            StyleValue::Num(255)
        );
        assert_eq!(
            Style::prop_get_default(StyleProp::BORDER_SIDE, None),
            StyleValue::Num(0x0F)
        );
        assert_eq!(
            Style::prop_get_default(StyleProp::MAX_WIDTH, None),
            StyleValue::Num(coord::MAX)
        );
        assert_eq!(
            Style::prop_get_default(StyleProp::TRANSFORM_SCALE_X, None),
            StyleValue::Num(256)
        );
        assert_eq!(Style::prop_get_default(StyleProp::WIDTH, None), StyleValue::Null);
        assert_eq!(
            Style::prop_get_default(StyleProp::TEXT_FONT, None),
            StyleValue::Null
        );
    }

    #[test]
    fn shared_ref_compares_by_pointer() {
        static A: [i32; 2] = [1, 2];
        static B: [i32; 2] = [1, 2];
        let ra: TracksRef = SharedRef::Static(&A);
        let rb: TracksRef = SharedRef::Static(&B);
        assert_eq!(ra, ra.clone());
        assert_ne!(ra, rb);
        assert_eq!(&*ra, &[1, 2]);
    }

    /// `lv_style_value_t` is a union: `.color` of a `.num` value and `.num`
    /// of a `.color` value reinterpret the same bytes.
    #[test]
    fn style_value_union_reinterpretation() {
        let red = StyleValue::Num(0xFF0000);
        assert_eq!(red.color(), Color::hex(0xFF0000));
        assert_eq!(StyleValue::Num(0x7F_12_34_56).color(), Color::hex(0x123456));
        assert_eq!(StyleValue::Color(Color::hex(0x123456)).num(), 0x123456);
        assert_eq!(StyleValue::Null.num(), 0);
        assert_eq!(StyleValue::Null.color(), Color::BLACK);

        let mut st = Style::new();
        st.set_prop(StyleProp::BG_COLOR, StyleValue::Num(0xFF0000));
        assert_eq!(
            st.get_prop(StyleProp::BG_COLOR).map(|v| v.color()),
            Some(Color::hex(0xFF0000))
        );
    }

    /// `LV_STYLE_CONST_INIT` sets `prop_cnt = 255`, so `lv_style_is_empty()`
    /// is `false` even for `{ LV_STYLE_CONST_PROPS_END }`.
    #[test]
    fn const_style_is_never_empty() {
        static END_ONLY: [ConstStyleEntry; 1] =
            [ConstStyleEntry::new(StyleProp::INV, ConstStyleValue::Num(0))];
        static S_END: ConstStyle = ConstStyle::new(&END_ONLY);
        static S_NONE: ConstStyle = ConstStyle::new(&[]);
        assert!(!S_END.is_empty());
        assert!(!S_NONE.is_empty());
        assert_eq!(ConstStyle::PROP_CNT, 255);
    }

    /// `lv_style_get_num_custom_props()` returns `lv_style_prop_t` (8 bits).
    #[test]
    fn num_custom_props_narrows_to_prop_t() {
        let mut table = Vec::new();
        let mut last = StyleProp::LAST_BUILT_IN.0 as u32;
        let mut prop = StyleProp::INV;
        for _ in 0..300 {
            prop = register_prop(&mut table, &mut last, 0);
        }
        assert_eq!(last, StyleProp::LAST_BUILT_IN.0 as u32 + 300);
        assert_eq!(get_num_custom_props(last), 300 % 256);
        assert_eq!(prop.0, last as u8);
        assert_eq!(get_num_custom_props(StyleProp::LAST_BUILT_IN.0 as u32), 0);
    }

    #[test]
    fn const_style_is_const_constructible() {
        static PROPS: [ConstStyleEntry; 2] = [
            ConstStyleEntry::new(StyleProp::BG_COLOR, ConstStyleValue::Color(Color::hex(0xFF0000))),
            ConstStyleEntry::new(StyleProp::RADIUS, ConstStyleValue::Num(5)),
        ];
        static S: ConstStyle = ConstStyle::new(&PROPS);
        assert_eq!(S.len(), 2);
        assert!(!S.is_empty());
        assert_eq!(
            S.props[0].to_entry().value,
            StyleValue::Color(Color::hex(0xFF0000))
        );
        assert_eq!(S.props[1].to_entry().value, StyleValue::Num(5));
    }

    // tests/src/test_cases/test_style.c: test_custom_prop_ids (the global
    // table is a local Vec + counter here, see `register_prop`).
    #[test]
    fn custom_prop_ids() {
        let mut table: Vec<u8> = Vec::new();
        let mut last_id = StyleProp::LAST_BUILT_IN.0 as u32;
        let mut fake_flag: u8 = 0;
        let initial_custom_props = get_num_custom_props(last_id);
        let max_props_to_register = 64u32;
        for i in 0..max_props_to_register {
            let prop = register_prop(&mut table, &mut last_id, fake_flag);
            /* Should have a higher index than the last built-in prop */
            assert!(prop.0 > StyleProp::LAST_BUILT_IN.0);
            if i == 0 {
                /* Should be equal to the first expected index of a custom prop */
                assert_eq!(
                    StyleProp::NUM_BUILT_IN as u32 + initial_custom_props,
                    prop.0 as u32
                );
            }
            /*We should find our flags*/
            assert_eq!(fake_flag, lookup_flags_with_custom(prop, &table));
            assert!(prop.is_custom());
            fake_flag = if fake_flag == 0xff { 0 } else { fake_flag + 1 };
        }
        assert_eq!(
            initial_custom_props + max_props_to_register,
            get_num_custom_props(last_id)
        );
        /*
         * Check that the resizing algorithm works correctly, given that 64 props
         * were registered + whatever's built-in.
         */
        assert_eq!(table.len(), 64);
    }

    #[test]
    fn custom_flag_lookup_edge_cases() {
        let empty: [u8; 0] = [];
        assert_eq!(
            lookup_flags_with_custom(StyleProp::ANY, &empty),
            STYLE_PROP_FLAG_ALL
        );
        assert_eq!(lookup_flags_with_custom(StyleProp::INV, &empty), 0);
        assert_eq!(
            lookup_flags_with_custom(StyleProp::PAD_TOP, &empty),
            STYLE_PROP_FLAG_EXT_DRAW_UPDATE | STYLE_PROP_FLAG_LAYOUT_UPDATE
        );
        // A custom id without a table (C: table == NULL) has no flags.
        assert_eq!(lookup_flags_with_custom(StyleProp(178), &empty), 0);

        let mut table = Vec::new();
        let mut last = StyleProp::LAST_BUILT_IN.0 as u32;
        let p = register_prop(&mut table, &mut last, STYLE_PROP_FLAG_INHERITABLE);
        assert_eq!(p, StyleProp(178));
        assert_eq!(table.len(), 32);
        assert!(prop_has_flag_with_custom(p, STYLE_PROP_FLAG_INHERITABLE, &table));
        assert!(!prop_has_flag_with_custom(
            p,
            STYLE_PROP_FLAG_LAYOUT_UPDATE,
            &table
        ));
        // Inside the 32-entry allocation but unregistered: 0; outside: 0.
        assert_eq!(lookup_flags_with_custom(StyleProp(179), &table), 0);
        assert_eq!(lookup_flags_with_custom(StyleProp(254), &table), 0);
        // The 33rd registration grows the table to 64 (32-byte rounding).
        for _ in 0..31 {
            let _ = register_prop(&mut table, &mut last, 0);
        }
        assert_eq!(table.len(), 32);
        let p33 = register_prop(&mut table, &mut last, 9);
        assert_eq!(p33, StyleProp(178 + 32));
        assert_eq!(table.len(), 64);
        assert_eq!(lookup_flags_with_custom(p33, &table), 9);
        assert_eq!(get_num_custom_props(last), 33);
        // An emptied table restarts the ids like the C `NULL` check.
        table.clear();
        let p = register_prop(&mut table, &mut last, 7);
        assert_eq!(p, StyleProp(178));
        assert_eq!(get_num_custom_props(last), 1);
        assert_eq!(lookup_flags_with_custom(p, &table), 7);
    }

    // test_style.c: test_const_style (the lookup half; attaching a const
    // style to an object belongs to lv_obj_style.c).
    #[test]
    fn const_style_get_prop() {
        static CONST_STYLE_PROPS: [ConstStyleEntry; 3] = [
            ConstStyleEntry::width(51),
            ConstStyleEntry::height(50),
            /*LV_STYLE_CONST_PROPS_END*/
            ConstStyleEntry::new(StyleProp::INV, ConstStyleValue::Num(0)),
        ];
        static CONST_STYLE: ConstStyle = ConstStyle::new(&CONST_STYLE_PROPS);
        assert!(CONST_STYLE.is_const());
        assert_eq!(CONST_STYLE.get_prop(StyleProp::WIDTH), Some(StyleValue::Num(51)));
        assert_eq!(CONST_STYLE.get_prop(StyleProp::HEIGHT), Some(StyleValue::Num(50)));
        assert_eq!(CONST_STYLE.get_prop(StyleProp::X), None);
        assert_eq!(CONST_STYLE.get_prop(StyleProp::INV), None);

        // Entries after the INV terminator are invisible, for lookup and merge.
        static TERMINATED: [ConstStyleEntry; 3] = [
            ConstStyleEntry::width(1),
            ConstStyleEntry::new(StyleProp::INV, ConstStyleValue::Num(0)),
            ConstStyleEntry::height(2),
        ];
        let t = ConstStyle::new(&TERMINATED);
        assert_eq!(t.get_prop(StyleProp::WIDTH), Some(StyleValue::Num(1)));
        assert_eq!(t.get_prop(StyleProp::HEIGHT), None);

        let mut s = Style::new();
        s.merge_from_const(&t);
        assert_eq!(s.len(), 1);
        assert_eq!(s.get_prop(StyleProp::WIDTH), Some(&StyleValue::Num(1)));
        s.copy_from_const(&CONST_STYLE);
        assert_eq!(s.len(), 2);
        assert_eq!(s.get_prop(StyleProp::WIDTH), Some(&StyleValue::Num(51)));
        assert_eq!(s.get_prop(StyleProp::HEIGHT), Some(&StyleValue::Num(50)));
        assert!(!s.is_const());
    }

    // test_style.c: test_style_copy
    #[test]
    fn style_copy() {
        let copied_bg_color = Color::hex(0x0000ff);
        let copied_border_color = Color::hex(0x00ff00);
        let copied_outline_color = Color::hex(0xffff00);

        let mut style1 = Style::new();
        style1.set_bg_color(Color::hex(0xff0000)); // this should get overwritten
        style1.set_outline_color(copied_outline_color);

        let mut style2 = Style::new();
        style2.set_bg_color(copied_bg_color);
        style2.set_border_color(copied_border_color);

        let mut copied_style = Style::new();
        copied_style.copy_from(&style1);
        copied_style.copy_from(&style2); // This should reset `copied_style` then duplicate the properties of `style2`

        assert_eq!(
            copied_style.get_prop(StyleProp::BG_COLOR),
            Some(&StyleValue::Color(copied_bg_color))
        );
        assert_eq!(
            copied_style.get_prop(StyleProp::BORDER_COLOR),
            Some(&StyleValue::Color(copied_border_color))
        );
        assert_eq!(copied_style.get_prop(StyleProp::OUTLINE_COLOR), None);

        /* Changing the original style should not impact the style that copied it */
        style2.set_bg_color(Color::hex(0x00ff00));
        assert_eq!(
            copied_style.get_prop(StyleProp::BG_COLOR),
            Some(&StyleValue::Color(copied_bg_color))
        );

        style1.reset();
        style2.reset();
        copied_style.reset();
        assert!(copied_style.is_empty());
        assert_eq!(copied_style.len(), 0);
        assert_eq!(copied_style.has_group, 0);
    }

    // test_style.c: test_style_merge
    #[test]
    fn style_merge() {
        let merged_bg_color = Color::hex(0x0000ff);
        let merged_border_color = Color::hex(0x00ff00);
        let merged_outline_color = Color::hex(0xffff00);

        let mut style1 = Style::new();
        style1.set_bg_color(Color::hex(0xff0000)); // this should get overwritten
        style1.set_outline_color(merged_outline_color);

        let mut style2 = Style::new();
        style2.set_bg_color(merged_bg_color);
        style2.set_border_color(merged_border_color);

        let mut merged_style = Style::new();
        merged_style.merge_from(&style1);
        merged_style.merge_from(&style2);

        assert_eq!(
            merged_style.get_prop(StyleProp::BG_COLOR),
            Some(&StyleValue::Color(merged_bg_color))
        );
        assert_eq!(
            merged_style.get_prop(StyleProp::BORDER_COLOR),
            Some(&StyleValue::Color(merged_border_color))
        );
        assert_eq!(
            merged_style.get_prop(StyleProp::OUTLINE_COLOR),
            Some(&StyleValue::Color(merged_outline_color))
        );
        // Overwritten in place: bg_color keeps its first position.
        assert_eq!(merged_style.len(), 3);
        assert_eq!(merged_style.props[0].prop, StyleProp::BG_COLOR);
        assert_eq!(merged_style.props[1].prop, StyleProp::OUTLINE_COLOR);
        assert_eq!(merged_style.props[2].prop, StyleProp::BORDER_COLOR);

        /* Changing the original style should not impact the style that copied it */
        style2.set_bg_color(Color::hex(0x00ff00));
        assert_eq!(
            merged_style.get_prop(StyleProp::BG_COLOR),
            Some(&StyleValue::Color(merged_bg_color))
        );
    }

    #[test]
    fn merge_from_empty_source_is_a_no_op() {
        let mut dst = Style::new();
        dst.set_width(5);
        let src = Style::new();
        dst.merge_from(&src);
        assert_eq!(dst.len(), 1);
        dst.copy_from(&src);
        assert!(dst.is_empty());
        assert_eq!(dst.has_group, 0);
    }

    #[test]
    fn set_prop_replaces_in_place_and_tracks_groups() {
        let mut s = Style::new();
        assert_eq!(s.has_group, 0);
        s.set_width(10);
        s.set_bg_color(Color::WHITE);
        s.set_radius(3);
        assert_eq!(s.len(), 3);
        assert_eq!(
            s.has_group,
            (1 << get_prop_group(StyleProp::WIDTH.0))
                | (1 << get_prop_group(StyleProp::BG_COLOR.0))
                | (1 << get_prop_group(StyleProp::RADIUS.0))
        );
        assert_eq!(s.has_group, (1 << 0) | (1 << 18) | (1 << 30));
        // Overwriting keeps the position and the count.
        s.set_width(20);
        assert_eq!(s.len(), 3);
        assert_eq!(s.props[0].prop, StyleProp::WIDTH);
        assert_eq!(s.props[0].value, StyleValue::Num(20));
        assert_eq!(s.get_prop(StyleProp::WIDTH), Some(&StyleValue::Num(20)));
        // The value kind is not checked, like the C union.
        s.set_prop(StyleProp::WIDTH, StyleValue::Color(Color::BLACK));
        assert_eq!(s.props[0].value, StyleValue::Color(Color::BLACK));
        assert_eq!(s.get_prop(StyleProp::HEIGHT), None);
        // Custom properties land in group 31.
        s.set_prop(StyleProp(200), StyleValue::Num(1));
        assert_ne!(s.has_group & (1 << 31), 0);
        assert_eq!(s.get_prop(StyleProp(200)), Some(&StyleValue::Num(1)));
    }

    #[test]
    fn remove_prop_keeps_order() {
        let mut s = Style::new();
        assert!(!s.remove_prop(StyleProp::WIDTH)); // prop_cnt == 0
        s.set_width(1);
        s.set_height(2);
        s.set_x(3);
        assert!(!s.remove_prop(StyleProp::Y));
        assert_eq!(s.len(), 3);
        assert!(s.remove_prop(StyleProp::HEIGHT));
        assert_eq!(s.len(), 2);
        assert_eq!(s.props[0].prop, StyleProp::WIDTH);
        assert_eq!(s.props[1].prop, StyleProp::X);
        assert_eq!(s.get_prop(StyleProp::HEIGHT), None);
        assert!(!s.remove_prop(StyleProp::HEIGHT));
        // `has_group` is a superset cache; removal does not clear it (as in C).
        let g = s.has_group;
        assert!(s.remove_prop(StyleProp::WIDTH));
        assert!(s.remove_prop(StyleProp::X));
        assert!(s.is_empty());
        assert_eq!(s.has_group, g);
        // A re-added property is appended again.
        s.set_x(9);
        assert_eq!(s.props[0].prop, StyleProp::X);
        assert_eq!(s.len(), 1);
    }

    #[cfg(not(feature = "check-arg-panic"))]
    #[test]
    fn set_prop_inv_is_rejected() {
        let mut s = Style::new();
        s.set_prop(StyleProp::INV, StyleValue::Num(1));
        assert!(s.is_empty());
        assert_eq!(s.has_group, 0);
    }

    #[cfg(feature = "check-arg-panic")]
    #[test]
    #[should_panic(expected = "argument check failed")]
    fn set_prop_inv_asserts() {
        let mut s = Style::new();
        s.set_prop(StyleProp::INV, StyleValue::Num(1));
    }

    // lv_style.c: lv_style_set_size .. lv_style_set_transform_scale
    #[test]
    fn compound_setters_expand_like_lv_style_c() {
        let mut s = Style::new();
        s.set_size(10, 20);
        assert_eq!(s.get_prop(StyleProp::WIDTH), Some(&StyleValue::Num(10)));
        assert_eq!(s.get_prop(StyleProp::HEIGHT), Some(&StyleValue::Num(20)));

        s.set_pad_all(4);
        // Insertion order: left, right, top, bottom.
        assert_eq!(s.props[2].prop, StyleProp::PAD_LEFT);
        assert_eq!(s.props[3].prop, StyleProp::PAD_RIGHT);
        assert_eq!(s.props[4].prop, StyleProp::PAD_TOP);
        assert_eq!(s.props[5].prop, StyleProp::PAD_BOTTOM);
        s.set_pad_hor(1);
        s.set_pad_ver(2);
        assert_eq!(s.get_prop(StyleProp::PAD_LEFT), Some(&StyleValue::Num(1)));
        assert_eq!(s.get_prop(StyleProp::PAD_RIGHT), Some(&StyleValue::Num(1)));
        assert_eq!(s.get_prop(StyleProp::PAD_TOP), Some(&StyleValue::Num(2)));
        assert_eq!(s.get_prop(StyleProp::PAD_BOTTOM), Some(&StyleValue::Num(2)));
        s.set_pad_gap(3);
        assert_eq!(s.get_prop(StyleProp::PAD_ROW), Some(&StyleValue::Num(3)));
        assert_eq!(s.get_prop(StyleProp::PAD_COLUMN), Some(&StyleValue::Num(3)));

        s.set_margin_all(5);
        s.set_margin_hor(6);
        s.set_margin_ver(7);
        assert_eq!(s.get_prop(StyleProp::MARGIN_LEFT), Some(&StyleValue::Num(6)));
        assert_eq!(s.get_prop(StyleProp::MARGIN_RIGHT), Some(&StyleValue::Num(6)));
        assert_eq!(s.get_prop(StyleProp::MARGIN_TOP), Some(&StyleValue::Num(7)));
        assert_eq!(s.get_prop(StyleProp::MARGIN_BOTTOM), Some(&StyleValue::Num(7)));

        s.set_transform_scale(300);
        assert_eq!(
            s.get_prop(StyleProp::TRANSFORM_SCALE_X),
            Some(&StyleValue::Num(300))
        );
        assert_eq!(
            s.get_prop(StyleProp::TRANSFORM_SCALE_Y),
            Some(&StyleValue::Num(300))
        );
        assert_eq!(s.len(), 2 + 4 + 2 + 4 + 2);
    }

    #[test]
    fn transition_dsc_init_defaults_the_path() {
        static PROPS: [StyleProp; 2] = [StyleProp::BG_COLOR, StyleProp::WIDTH];
        let mut tr = StyleTransitionDsc::new_static(&PROPS, AnimPath::EaseIn, 10, 20);
        assert_eq!(tr.path, AnimPath::EaseIn);
        assert_eq!(tr.time, 10);
        tr.init(SharedRef::Static(&PROPS), None, 300, 5, None);
        assert_eq!(tr.path, AnimPath::Linear); /*path_cb == NULL -> lv_anim_path_linear*/
        assert_eq!(tr.time, 300);
        assert_eq!(tr.delay, 5);
        assert_eq!(&*tr.props, &PROPS[..]);
        assert!(tr.user_data.is_none());
        tr.init(
            SharedRef::Static(&PROPS),
            Some(AnimPath::Bounce),
            1,
            2,
            Some(Rc::new(5u8)),
        );
        assert_eq!(tr.path, AnimPath::Bounce);
        assert!(tr.user_data.is_some());
        // Stored transitions compare by identity, like `const lv_style_transition_dsc_t *`.
        let shared: TransitionRef = SharedRef::Shared(Rc::new(tr));
        let mut s = Style::new();
        s.set_transition(shared.clone());
        assert_eq!(
            s.get_prop(StyleProp::TRANSITION),
            Some(&StyleValue::Transition(shared))
        );
    }
}
