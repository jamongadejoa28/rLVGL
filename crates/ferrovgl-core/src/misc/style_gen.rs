//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_style_gen.h`, `src/misc/lv_style_gen.c`
//! (and the property table shared with `src/core/lv_obj_style_gen.c`).
//!
//! [`style_props!`](crate::style_props) is a "callback" table macro: `style_props!(my_macro)`
//! expands to `my_macro! { (ROW), (ROW), ... }` with one row per built-in
//! property, so that every typed accessor family (`lv_style_set_*`,
//! `lv_obj_set_style_*`, `lv_obj_get_style_*`) is generated from a single
//! source of truth. This file generates the `Style::set_*` setters and the
//! `ConstStyleEntry::*` const constructors (`LV_STYLE_CONST_*`).
//!
//! Row format:
//! `(CONST_NAME, snake_name, set_ident, obj_set_ident, obj_get_ident, Variant, kind, Type)`
//! where `CONST_NAME` is the `StyleProp::CONST_NAME` associated constant,
//! `Variant` the CamelCase spelling kept for generated enum-like tables, and `kind` is one of `num`, `cast`, `enum`, `flags`, `bool`, `color`,
//! `font`, `grad`, `image`, `transition`, `color_filter`, `tracks`, `anim`,
//! `colorkey`, and selects how `Type` is packed into a [`StyleValue`](crate::misc::style::StyleValue).

use crate::misc::style::{ConstStyleEntry, ConstStyleValue, StyleProp};

/// Table of all built-in style properties, see the module docs.
#[macro_export]
macro_rules! style_props {
    ($cb:ident) => {
        $cb! {
            (WIDTH, width, set_width, set_style_width, get_style_width, Width, num, i32),
            (MIN_WIDTH, min_width, set_min_width, set_style_min_width, get_style_min_width, MinWidth, num, i32),
            (MAX_WIDTH, max_width, set_max_width, set_style_max_width, get_style_max_width, MaxWidth, num, i32),
            (HEIGHT, height, set_height, set_style_height, get_style_height, Height, num, i32),
            (MIN_HEIGHT, min_height, set_min_height, set_style_min_height, get_style_min_height, MinHeight, num, i32),
            (MAX_HEIGHT, max_height, set_max_height, set_style_max_height, get_style_max_height, MaxHeight, num, i32),
            (LENGTH, length, set_length, set_style_length, get_style_length, Length, num, i32),
            (X, x, set_x, set_style_x, get_style_x, X, num, i32),
            (Y, y, set_y, set_style_y, get_style_y, Y, num, i32),
            (ALIGN, align, set_align, set_style_align, get_style_align, Align, enum, $crate::misc::area::Align),
            (TRANSFORM_WIDTH, transform_width, set_transform_width, set_style_transform_width, get_style_transform_width, TransformWidth, num, i32),
            (TRANSFORM_HEIGHT, transform_height, set_transform_height, set_style_transform_height, get_style_transform_height, TransformHeight, num, i32),
            (TRANSLATE_X, translate_x, set_translate_x, set_style_translate_x, get_style_translate_x, TranslateX, num, i32),
            (TRANSLATE_Y, translate_y, set_translate_y, set_style_translate_y, get_style_translate_y, TranslateY, num, i32),
            (TRANSLATE_RADIAL, translate_radial, set_translate_radial, set_style_translate_radial, get_style_translate_radial, TranslateRadial, num, i32),
            (TRANSFORM_SCALE_X, transform_scale_x, set_transform_scale_x, set_style_transform_scale_x, get_style_transform_scale_x, TransformScaleX, num, i32),
            (TRANSFORM_SCALE_Y, transform_scale_y, set_transform_scale_y, set_style_transform_scale_y, get_style_transform_scale_y, TransformScaleY, num, i32),
            (TRANSFORM_ROTATION, transform_rotation, set_transform_rotation, set_style_transform_rotation, get_style_transform_rotation, TransformRotation, num, i32),
            (TRANSFORM_PIVOT_X, transform_pivot_x, set_transform_pivot_x, set_style_transform_pivot_x, get_style_transform_pivot_x, TransformPivotX, num, i32),
            (TRANSFORM_PIVOT_Y, transform_pivot_y, set_transform_pivot_y, set_style_transform_pivot_y, get_style_transform_pivot_y, TransformPivotY, num, i32),
            (TRANSFORM_SKEW_X, transform_skew_x, set_transform_skew_x, set_style_transform_skew_x, get_style_transform_skew_x, TransformSkewX, num, i32),
            (TRANSFORM_SKEW_Y, transform_skew_y, set_transform_skew_y, set_style_transform_skew_y, get_style_transform_skew_y, TransformSkewY, num, i32),
            (PAD_TOP, pad_top, set_pad_top, set_style_pad_top, get_style_pad_top, PadTop, num, i32),
            (PAD_BOTTOM, pad_bottom, set_pad_bottom, set_style_pad_bottom, get_style_pad_bottom, PadBottom, num, i32),
            (PAD_LEFT, pad_left, set_pad_left, set_style_pad_left, get_style_pad_left, PadLeft, num, i32),
            (PAD_RIGHT, pad_right, set_pad_right, set_style_pad_right, get_style_pad_right, PadRight, num, i32),
            (PAD_ROW, pad_row, set_pad_row, set_style_pad_row, get_style_pad_row, PadRow, num, i32),
            (PAD_COLUMN, pad_column, set_pad_column, set_style_pad_column, get_style_pad_column, PadColumn, num, i32),
            (PAD_RADIAL, pad_radial, set_pad_radial, set_style_pad_radial, get_style_pad_radial, PadRadial, num, i32),
            (MARGIN_TOP, margin_top, set_margin_top, set_style_margin_top, get_style_margin_top, MarginTop, num, i32),
            (MARGIN_BOTTOM, margin_bottom, set_margin_bottom, set_style_margin_bottom, get_style_margin_bottom, MarginBottom, num, i32),
            (MARGIN_LEFT, margin_left, set_margin_left, set_style_margin_left, get_style_margin_left, MarginLeft, num, i32),
            (MARGIN_RIGHT, margin_right, set_margin_right, set_style_margin_right, get_style_margin_right, MarginRight, num, i32),
            (BG_COLOR, bg_color, set_bg_color, set_style_bg_color, get_style_bg_color, BgColor, color, $crate::misc::color::Color),
            (BG_OPA, bg_opa, set_bg_opa, set_style_bg_opa, get_style_bg_opa, BgOpa, cast, $crate::misc::color::Opa),
            (BG_GRAD_COLOR, bg_grad_color, set_bg_grad_color, set_style_bg_grad_color, get_style_bg_grad_color, BgGradColor, color, $crate::misc::color::Color),
            (BG_GRAD_DIR, bg_grad_dir, set_bg_grad_dir, set_style_bg_grad_dir, get_style_bg_grad_dir, BgGradDir, enum, $crate::draw::grad::GradDir),
            (BG_MAIN_STOP, bg_main_stop, set_bg_main_stop, set_style_bg_main_stop, get_style_bg_main_stop, BgMainStop, num, i32),
            (BG_GRAD_STOP, bg_grad_stop, set_bg_grad_stop, set_style_bg_grad_stop, get_style_bg_grad_stop, BgGradStop, num, i32),
            (BG_MAIN_OPA, bg_main_opa, set_bg_main_opa, set_style_bg_main_opa, get_style_bg_main_opa, BgMainOpa, cast, $crate::misc::color::Opa),
            (BG_GRAD_OPA, bg_grad_opa, set_bg_grad_opa, set_style_bg_grad_opa, get_style_bg_grad_opa, BgGradOpa, cast, $crate::misc::color::Opa),
            (BG_GRAD, bg_grad, set_bg_grad, set_style_bg_grad, get_style_bg_grad, BgGrad, grad, $crate::misc::style::GradRef),
            (BG_IMAGE_SRC, bg_image_src, set_bg_image_src, set_style_bg_image_src, get_style_bg_image_src, BgImageSrc, image, $crate::misc::style::ImageSrc),
            (BG_IMAGE_OPA, bg_image_opa, set_bg_image_opa, set_style_bg_image_opa, get_style_bg_image_opa, BgImageOpa, cast, $crate::misc::color::Opa),
            (BG_IMAGE_RECOLOR, bg_image_recolor, set_bg_image_recolor, set_style_bg_image_recolor, get_style_bg_image_recolor, BgImageRecolor, color, $crate::misc::color::Color),
            (BG_IMAGE_RECOLOR_OPA, bg_image_recolor_opa, set_bg_image_recolor_opa, set_style_bg_image_recolor_opa, get_style_bg_image_recolor_opa, BgImageRecolorOpa, cast, $crate::misc::color::Opa),
            (BG_IMAGE_TILED, bg_image_tiled, set_bg_image_tiled, set_style_bg_image_tiled, get_style_bg_image_tiled, BgImageTiled, bool, bool),
            (BORDER_COLOR, border_color, set_border_color, set_style_border_color, get_style_border_color, BorderColor, color, $crate::misc::color::Color),
            (BORDER_OPA, border_opa, set_border_opa, set_style_border_opa, get_style_border_opa, BorderOpa, cast, $crate::misc::color::Opa),
            (BORDER_WIDTH, border_width, set_border_width, set_style_border_width, get_style_border_width, BorderWidth, num, i32),
            (BORDER_SIDE, border_side, set_border_side, set_style_border_side, get_style_border_side, BorderSide, flags, $crate::misc::style::BorderSide),
            (BORDER_POST, border_post, set_border_post, set_style_border_post, get_style_border_post, BorderPost, bool, bool),
            (OUTLINE_WIDTH, outline_width, set_outline_width, set_style_outline_width, get_style_outline_width, OutlineWidth, num, i32),
            (OUTLINE_COLOR, outline_color, set_outline_color, set_style_outline_color, get_style_outline_color, OutlineColor, color, $crate::misc::color::Color),
            (OUTLINE_OPA, outline_opa, set_outline_opa, set_style_outline_opa, get_style_outline_opa, OutlineOpa, cast, $crate::misc::color::Opa),
            (OUTLINE_PAD, outline_pad, set_outline_pad, set_style_outline_pad, get_style_outline_pad, OutlinePad, num, i32),
            (SHADOW_WIDTH, shadow_width, set_shadow_width, set_style_shadow_width, get_style_shadow_width, ShadowWidth, num, i32),
            (SHADOW_OFFSET_X, shadow_offset_x, set_shadow_offset_x, set_style_shadow_offset_x, get_style_shadow_offset_x, ShadowOffsetX, num, i32),
            (SHADOW_OFFSET_Y, shadow_offset_y, set_shadow_offset_y, set_style_shadow_offset_y, get_style_shadow_offset_y, ShadowOffsetY, num, i32),
            (SHADOW_SPREAD, shadow_spread, set_shadow_spread, set_style_shadow_spread, get_style_shadow_spread, ShadowSpread, num, i32),
            (SHADOW_COLOR, shadow_color, set_shadow_color, set_style_shadow_color, get_style_shadow_color, ShadowColor, color, $crate::misc::color::Color),
            (SHADOW_OPA, shadow_opa, set_shadow_opa, set_style_shadow_opa, get_style_shadow_opa, ShadowOpa, cast, $crate::misc::color::Opa),
            (IMAGE_OPA, image_opa, set_image_opa, set_style_image_opa, get_style_image_opa, ImageOpa, cast, $crate::misc::color::Opa),
            (IMAGE_RECOLOR, image_recolor, set_image_recolor, set_style_image_recolor, get_style_image_recolor, ImageRecolor, color, $crate::misc::color::Color),
            (IMAGE_RECOLOR_OPA, image_recolor_opa, set_image_recolor_opa, set_style_image_recolor_opa, get_style_image_recolor_opa, ImageRecolorOpa, cast, $crate::misc::color::Opa),
            (IMAGE_COLORKEY, image_colorkey, set_image_colorkey, set_style_image_colorkey, get_style_image_colorkey, ImageColorkey, colorkey, $crate::misc::style::ImageColorkey),
            (LINE_WIDTH, line_width, set_line_width, set_style_line_width, get_style_line_width, LineWidth, num, i32),
            (LINE_DASH_WIDTH, line_dash_width, set_line_dash_width, set_style_line_dash_width, get_style_line_dash_width, LineDashWidth, num, i32),
            (LINE_DASH_GAP, line_dash_gap, set_line_dash_gap, set_style_line_dash_gap, get_style_line_dash_gap, LineDashGap, num, i32),
            (LINE_ROUNDED, line_rounded, set_line_rounded, set_style_line_rounded, get_style_line_rounded, LineRounded, bool, bool),
            (LINE_COLOR, line_color, set_line_color, set_style_line_color, get_style_line_color, LineColor, color, $crate::misc::color::Color),
            (LINE_OPA, line_opa, set_line_opa, set_style_line_opa, get_style_line_opa, LineOpa, cast, $crate::misc::color::Opa),
            (ARC_WIDTH, arc_width, set_arc_width, set_style_arc_width, get_style_arc_width, ArcWidth, num, i32),
            (ARC_ROUNDED, arc_rounded, set_arc_rounded, set_style_arc_rounded, get_style_arc_rounded, ArcRounded, bool, bool),
            (ARC_COLOR, arc_color, set_arc_color, set_style_arc_color, get_style_arc_color, ArcColor, color, $crate::misc::color::Color),
            (ARC_OPA, arc_opa, set_arc_opa, set_style_arc_opa, get_style_arc_opa, ArcOpa, cast, $crate::misc::color::Opa),
            (ARC_IMAGE_SRC, arc_image_src, set_arc_image_src, set_style_arc_image_src, get_style_arc_image_src, ArcImageSrc, image, $crate::misc::style::ImageSrc),
            (TEXT_COLOR, text_color, set_text_color, set_style_text_color, get_style_text_color, TextColor, color, $crate::misc::color::Color),
            (TEXT_OPA, text_opa, set_text_opa, set_style_text_opa, get_style_text_opa, TextOpa, cast, $crate::misc::color::Opa),
            (TEXT_FONT, text_font, set_text_font, set_style_text_font, get_style_text_font, TextFont, font, $crate::font::FontRef),
            (TEXT_LETTER_SPACE, text_letter_space, set_text_letter_space, set_style_text_letter_space, get_style_text_letter_space, TextLetterSpace, num, i32),
            (TEXT_LINE_SPACE, text_line_space, set_text_line_space, set_style_text_line_space, get_style_text_line_space, TextLineSpace, num, i32),
            (TEXT_DECOR, text_decor, set_text_decor, set_style_text_decor, get_style_text_decor, TextDecor, flags, $crate::misc::text::TextDecor),
            (TEXT_ALIGN, text_align, set_text_align, set_style_text_align, get_style_text_align, TextAlign, enum, $crate::misc::text::TextAlign),
            (TEXT_OUTLINE_STROKE_COLOR, text_outline_stroke_color, set_text_outline_stroke_color, set_style_text_outline_stroke_color, get_style_text_outline_stroke_color, TextOutlineStrokeColor, color, $crate::misc::color::Color),
            (TEXT_OUTLINE_STROKE_WIDTH, text_outline_stroke_width, set_text_outline_stroke_width, set_style_text_outline_stroke_width, get_style_text_outline_stroke_width, TextOutlineStrokeWidth, num, i32),
            (TEXT_OUTLINE_STROKE_OPA, text_outline_stroke_opa, set_text_outline_stroke_opa, set_style_text_outline_stroke_opa, get_style_text_outline_stroke_opa, TextOutlineStrokeOpa, cast, $crate::misc::color::Opa),
            (TEXT_LEADING_TRIM, text_leading_trim, set_text_leading_trim, set_style_text_leading_trim, get_style_text_leading_trim, TextLeadingTrim, enum, $crate::misc::text::LeadingTrim),
            (BLUR_RADIUS, blur_radius, set_blur_radius, set_style_blur_radius, get_style_blur_radius, BlurRadius, num, i32),
            (BLUR_BACKDROP, blur_backdrop, set_blur_backdrop, set_style_blur_backdrop, get_style_blur_backdrop, BlurBackdrop, bool, bool),
            (BLUR_QUALITY, blur_quality, set_blur_quality, set_style_blur_quality, get_style_blur_quality, BlurQuality, enum, $crate::misc::style::BlurQuality),
            (DROP_SHADOW_RADIUS, drop_shadow_radius, set_drop_shadow_radius, set_style_drop_shadow_radius, get_style_drop_shadow_radius, DropShadowRadius, num, i32),
            (DROP_SHADOW_OFFSET_X, drop_shadow_offset_x, set_drop_shadow_offset_x, set_style_drop_shadow_offset_x, get_style_drop_shadow_offset_x, DropShadowOffsetX, num, i32),
            (DROP_SHADOW_OFFSET_Y, drop_shadow_offset_y, set_drop_shadow_offset_y, set_style_drop_shadow_offset_y, get_style_drop_shadow_offset_y, DropShadowOffsetY, num, i32),
            (DROP_SHADOW_COLOR, drop_shadow_color, set_drop_shadow_color, set_style_drop_shadow_color, get_style_drop_shadow_color, DropShadowColor, color, $crate::misc::color::Color),
            (DROP_SHADOW_OPA, drop_shadow_opa, set_drop_shadow_opa, set_style_drop_shadow_opa, get_style_drop_shadow_opa, DropShadowOpa, cast, $crate::misc::color::Opa),
            (DROP_SHADOW_QUALITY, drop_shadow_quality, set_drop_shadow_quality, set_style_drop_shadow_quality, get_style_drop_shadow_quality, DropShadowQuality, enum, $crate::misc::style::BlurQuality),
            (RADIUS, radius, set_radius, set_style_radius, get_style_radius, Radius, num, i32),
            (RADIAL_OFFSET, radial_offset, set_radial_offset, set_style_radial_offset, get_style_radial_offset, RadialOffset, num, i32),
            (CLIP_CORNER, clip_corner, set_clip_corner, set_style_clip_corner, get_style_clip_corner, ClipCorner, bool, bool),
            (OPA, opa, set_opa, set_style_opa, get_style_opa, Opa, cast, $crate::misc::color::Opa),
            (OPA_LAYERED, opa_layered, set_opa_layered, set_style_opa_layered, get_style_opa_layered, OpaLayered, cast, $crate::misc::color::Opa),
            (COLOR_FILTER_DSC, color_filter_dsc, set_color_filter_dsc, set_style_color_filter_dsc, get_style_color_filter_dsc, ColorFilterDsc, color_filter, $crate::misc::color::ColorFilter),
            (COLOR_FILTER_OPA, color_filter_opa, set_color_filter_opa, set_style_color_filter_opa, get_style_color_filter_opa, ColorFilterOpa, cast, $crate::misc::color::Opa),
            (RECOLOR, recolor, set_recolor, set_style_recolor, get_style_recolor, Recolor, color, $crate::misc::color::Color),
            (RECOLOR_OPA, recolor_opa, set_recolor_opa, set_style_recolor_opa, get_style_recolor_opa, RecolorOpa, cast, $crate::misc::color::Opa),
            (ANIM, anim, set_anim, set_style_anim, get_style_anim, Anim, anim, $crate::misc::style::AnimRef),
            (ANIM_DURATION, anim_duration, set_anim_duration, set_style_anim_duration, get_style_anim_duration, AnimDuration, cast, u32),
            (TRANSITION, transition, set_transition, set_style_transition, get_style_transition, Transition, transition, $crate::misc::style::TransitionRef),
            (BLEND_MODE, blend_mode, set_blend_mode, set_style_blend_mode, get_style_blend_mode, BlendMode, enum, $crate::misc::style::BlendMode),
            (LAYOUT, layout, set_layout, set_style_layout, get_style_layout, Layout, cast, u16),
            (BASE_DIR, base_dir, set_base_dir, set_style_base_dir, get_style_base_dir, BaseDir, enum, $crate::misc::text::BaseDir),
            (BITMAP_MASK_SRC, bitmap_mask_src, set_bitmap_mask_src, set_style_bitmap_mask_src, get_style_bitmap_mask_src, BitmapMaskSrc, image, $crate::misc::style::ImageSrc),
            (ROTARY_SENSITIVITY, rotary_sensitivity, set_rotary_sensitivity, set_style_rotary_sensitivity, get_style_rotary_sensitivity, RotarySensitivity, cast, u32),
            (FLEX_FLOW, flex_flow, set_flex_flow, set_style_flex_flow, get_style_flex_flow, FlexFlow, enum, $crate::layouts::flex::FlexFlow),
            (FLEX_MAIN_PLACE, flex_main_place, set_flex_main_place, set_style_flex_main_place, get_style_flex_main_place, FlexMainPlace, enum, $crate::layouts::flex::FlexAlign),
            (FLEX_CROSS_PLACE, flex_cross_place, set_flex_cross_place, set_style_flex_cross_place, get_style_flex_cross_place, FlexCrossPlace, enum, $crate::layouts::flex::FlexAlign),
            (FLEX_TRACK_PLACE, flex_track_place, set_flex_track_place, set_style_flex_track_place, get_style_flex_track_place, FlexTrackPlace, enum, $crate::layouts::flex::FlexAlign),
            (FLEX_GROW, flex_grow, set_flex_grow, set_style_flex_grow, get_style_flex_grow, FlexGrow, cast, u8),
            (GRID_COLUMN_DSC_ARRAY, grid_column_dsc_array, set_grid_column_dsc_array, set_style_grid_column_dsc_array, get_style_grid_column_dsc_array, GridColumnDscArray, tracks, $crate::misc::style::TracksRef),
            (GRID_COLUMN_ALIGN, grid_column_align, set_grid_column_align, set_style_grid_column_align, get_style_grid_column_align, GridColumnAlign, enum, $crate::layouts::grid::GridAlign),
            (GRID_ROW_DSC_ARRAY, grid_row_dsc_array, set_grid_row_dsc_array, set_style_grid_row_dsc_array, get_style_grid_row_dsc_array, GridRowDscArray, tracks, $crate::misc::style::TracksRef),
            (GRID_ROW_ALIGN, grid_row_align, set_grid_row_align, set_style_grid_row_align, get_style_grid_row_align, GridRowAlign, enum, $crate::layouts::grid::GridAlign),
            (GRID_CELL_COLUMN_POS, grid_cell_column_pos, set_grid_cell_column_pos, set_style_grid_cell_column_pos, get_style_grid_cell_column_pos, GridCellColumnPos, num, i32),
            (GRID_CELL_X_ALIGN, grid_cell_x_align, set_grid_cell_x_align, set_style_grid_cell_x_align, get_style_grid_cell_x_align, GridCellXAlign, enum, $crate::layouts::grid::GridAlign),
            (GRID_CELL_COLUMN_SPAN, grid_cell_column_span, set_grid_cell_column_span, set_style_grid_cell_column_span, get_style_grid_cell_column_span, GridCellColumnSpan, num, i32),
            (GRID_CELL_ROW_POS, grid_cell_row_pos, set_grid_cell_row_pos, set_style_grid_cell_row_pos, get_style_grid_cell_row_pos, GridCellRowPos, num, i32),
            (GRID_CELL_Y_ALIGN, grid_cell_y_align, set_grid_cell_y_align, set_style_grid_cell_y_align, get_style_grid_cell_y_align, GridCellYAlign, enum, $crate::layouts::grid::GridAlign),
            (GRID_CELL_ROW_SPAN, grid_cell_row_span, set_grid_cell_row_span, set_style_grid_cell_row_span, get_style_grid_cell_row_span, GridCellRowSpan, num, i32),
        }
    };
}

/// Pack a typed value into a [`StyleValue`](crate::misc::style::StyleValue) according to the table `kind`.
///
/// Usable in `const` contexts for every kind (moving an `Rc` through a const
/// fn is allowed; only creating one is not).
#[macro_export]
macro_rules! style_value_pack {
    (num, $v:expr) => {
        $crate::misc::style::StyleValue::Num($v)
    };
    (cast, $v:expr) => {
        $crate::misc::style::StyleValue::Num($v as i32)
    };
    (enum, $v:expr) => {
        $crate::misc::style::StyleValue::Num($v as i32)
    };
    (flags, $v:expr) => {
        $crate::misc::style::StyleValue::Num($v.bits() as i32)
    };
    (bool, $v:expr) => {
        $crate::misc::style::StyleValue::Num($v as i32)
    };
    (color, $v:expr) => {
        $crate::misc::style::StyleValue::Color($v)
    };
    (font, $v:expr) => {
        $crate::misc::style::StyleValue::Font($v)
    };
    (grad, $v:expr) => {
        $crate::misc::style::StyleValue::Grad($v)
    };
    (image, $v:expr) => {
        $crate::misc::style::StyleValue::Image($v)
    };
    (transition, $v:expr) => {
        $crate::misc::style::StyleValue::Transition($v)
    };
    (color_filter, $v:expr) => {
        $crate::misc::style::StyleValue::ColorFilter($v)
    };
    (tracks, $v:expr) => {
        $crate::misc::style::StyleValue::Tracks($v)
    };
    (anim, $v:expr) => {
        $crate::misc::style::StyleValue::Anim($v)
    };
    (colorkey, $v:expr) => {
        $crate::misc::style::StyleValue::Colorkey($v)
    };
}

macro_rules! gen_style_setters {
    ($( ($c:ident, $s:ident, $set:ident, $oset:ident, $oget:ident, $var:ident, $kind:ident, $t:ty) ),* $(,)?) => {
        impl crate::misc::style::Style {
            $(
                #[doc = concat!("`lv_style_set_", stringify!($s), "()`")]
                #[inline]
                pub fn $set(&mut self, value: $t) {
                    self.set_prop(StyleProp::$c, $crate::style_value_pack!($kind, value));
                }
            )*
        }

        impl ConstStyleEntry {
            $( const_entry_fn!($kind, $c, $s, $var, $t); )*
        }

        /// Every built-in property, in table order.
        pub const BUILTIN_PROPS: &[StyleProp] = &[ $( StyleProp::$c, )* ];
    };
}

/// One `LV_STYLE_CONST_<PROP>(value)` constructor, per value kind.
macro_rules! const_entry_fn {
    (num, $c:ident, $s:ident, $var:ident, $t:ty) => {
        #[doc = concat!("`LV_STYLE_CONST_", stringify!($c), "(value)`")]
        #[must_use]
        pub const fn $s(value: $t) -> ConstStyleEntry {
            ConstStyleEntry::new(StyleProp::$c, ConstStyleValue::Num(value))
        }
    };
    (cast, $c:ident, $s:ident, $var:ident, $t:ty) => {
        #[doc = concat!("`LV_STYLE_CONST_", stringify!($c), "(value)`")]
        #[must_use]
        pub const fn $s(value: $t) -> ConstStyleEntry {
            ConstStyleEntry::new(StyleProp::$c, ConstStyleValue::Num(value as i32))
        }
    };
    (enum, $c:ident, $s:ident, $var:ident, $t:ty) => {
        #[doc = concat!("`LV_STYLE_CONST_", stringify!($c), "(value)`")]
        #[must_use]
        pub const fn $s(value: $t) -> ConstStyleEntry {
            ConstStyleEntry::new(StyleProp::$c, ConstStyleValue::Num(value as i32))
        }
    };
    (flags, $c:ident, $s:ident, $var:ident, $t:ty) => {
        #[doc = concat!("`LV_STYLE_CONST_", stringify!($c), "(value)`")]
        #[must_use]
        pub const fn $s(value: $t) -> ConstStyleEntry {
            ConstStyleEntry::new(StyleProp::$c, ConstStyleValue::Num(value.bits() as i32))
        }
    };
    (bool, $c:ident, $s:ident, $var:ident, $t:ty) => {
        #[doc = concat!("`LV_STYLE_CONST_", stringify!($c), "(value)`")]
        #[must_use]
        pub const fn $s(value: bool) -> ConstStyleEntry {
            ConstStyleEntry::new(StyleProp::$c, ConstStyleValue::Num(value as i32))
        }
    };
    (color, $c:ident, $s:ident, $var:ident, $t:ty) => {
        #[doc = concat!("`LV_STYLE_CONST_", stringify!($c), "(value)`")]
        #[must_use]
        pub const fn $s(value: $t) -> ConstStyleEntry {
            ConstStyleEntry::new(StyleProp::$c, ConstStyleValue::Color(value))
        }
    };
    (font, $c:ident, $s:ident, $var:ident, $t:ty) => {
        #[doc = concat!("`LV_STYLE_CONST_", stringify!($c), "(value)`")]
        #[must_use]
        pub const fn $s(value: &'static (dyn $crate::font::Font + Sync)) -> ConstStyleEntry {
            ConstStyleEntry::new(StyleProp::$c, ConstStyleValue::Font(value))
        }
    };
    (grad, $c:ident, $s:ident, $var:ident, $t:ty) => {
        #[doc = concat!("`LV_STYLE_CONST_", stringify!($c), "(value)`")]
        #[must_use]
        pub const fn $s(value: &'static $crate::draw::grad::GradDsc) -> ConstStyleEntry {
            ConstStyleEntry::new(StyleProp::$c, ConstStyleValue::Grad(value))
        }
    };
    (image, $c:ident, $s:ident, $var:ident, $t:ty) => {
        #[doc = concat!("`LV_STYLE_CONST_", stringify!($c), "(value)`")]
        #[must_use]
        pub const fn $s(value: $crate::misc::style::ConstImageSrc) -> ConstStyleEntry {
            ConstStyleEntry::new(StyleProp::$c, ConstStyleValue::Image(value))
        }
    };
    (color_filter, $c:ident, $s:ident, $var:ident, $t:ty) => {
        #[doc = concat!("`LV_STYLE_CONST_", stringify!($c), "(value)`")]
        #[must_use]
        pub const fn $s(value: $t) -> ConstStyleEntry {
            ConstStyleEntry::new(StyleProp::$c, ConstStyleValue::ColorFilter(value))
        }
    };
    (tracks, $c:ident, $s:ident, $var:ident, $t:ty) => {
        #[doc = concat!("`LV_STYLE_CONST_", stringify!($c), "(value)`")]
        #[must_use]
        pub const fn $s(value: &'static [i32]) -> ConstStyleEntry {
            ConstStyleEntry::new(StyleProp::$c, ConstStyleValue::Tracks(value))
        }
    };
    (colorkey, $c:ident, $s:ident, $var:ident, $t:ty) => {
        #[doc = concat!("`LV_STYLE_CONST_", stringify!($c), "(value)`")]
        #[must_use]
        pub const fn $s(value: $t) -> ConstStyleEntry {
            ConstStyleEntry::new(StyleProp::$c, ConstStyleValue::Colorkey(value))
        }
    };
    // Transitions and animation templates are reference counted and cannot
    // live in a `static` constant style.
    (transition, $c:ident, $s:ident, $var:ident, $t:ty) => {};
    (anim, $c:ident, $s:ident, $var:ident, $t:ty) => {};
}

style_props!(gen_style_setters);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::misc::color::Color;
    use crate::misc::style::StyleValue;

    #[test]
    fn table_covers_every_built_in_setter() {
        // 129 `lv_style_set_*` functions in lv_style_gen.h.
        assert_eq!(BUILTIN_PROPS.len(), 129);
        let mut sorted = alloc::vec::Vec::from(BUILTIN_PROPS);
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), BUILTIN_PROPS.len(), "duplicate property in table");
        for p in BUILTIN_PROPS {
            assert!(p.is_built_in());
        }
    }

    #[test]
    fn const_entries_pack_values_like_the_c_union() {
        static E1: ConstStyleEntry = ConstStyleEntry::bg_color(Color::hex(0x123456));
        static E2: ConstStyleEntry = ConstStyleEntry::bg_opa(128);
        static E3: ConstStyleEntry = ConstStyleEntry::border_side(crate::misc::style::BorderSide::LEFT);
        static E4: ConstStyleEntry = ConstStyleEntry::bg_image_tiled(true);
        assert_eq!(E1.prop, StyleProp::BG_COLOR);
        assert_eq!(E1.to_entry().value, StyleValue::Color(Color::hex(0x123456)));
        assert_eq!(E2.to_entry().value, StyleValue::Num(128));
        assert_eq!(E3.to_entry().value, StyleValue::Num(4));
        assert_eq!(E4.to_entry().value, StyleValue::Num(1));
        assert_eq!(
            ConstStyleEntry::text_align(crate::misc::text::TextAlign::Right)
                .to_entry()
                .value,
            StyleValue::Num(3)
        );
        // The generated setters pack exactly like the `lv_style_value_t v = {...}`
        // initialisers of lv_style_gen.c and go through `lv_style_set_prop()`.
        let mut st = crate::misc::style::Style::new();
        st.set_width(10);
        st.set_bg_opa(128);
        st.set_border_side(crate::misc::style::BorderSide::LEFT | crate::misc::style::BorderSide::RIGHT);
        st.set_bg_image_tiled(true);
        st.set_text_align(crate::misc::text::TextAlign::Right);
        st.set_bg_color(Color::hex(0x123456));
        st.set_anim_duration(500);
        st.set_layout(2);
        st.set_flex_grow(3);
        st.set_image_colorkey(crate::misc::style::ImageColorkey {
            low: Color::hex(0x000000),
            high: Color::hex(0x101010),
        });
        assert_eq!(st.get_prop(StyleProp::WIDTH), Some(&StyleValue::Num(10)));
        assert_eq!(st.get_prop(StyleProp::BG_OPA), Some(&StyleValue::Num(128)));
        assert_eq!(st.get_prop(StyleProp::BORDER_SIDE), Some(&StyleValue::Num(0x0C)));
        assert_eq!(st.get_prop(StyleProp::BG_IMAGE_TILED), Some(&StyleValue::Num(1)));
        assert_eq!(st.get_prop(StyleProp::TEXT_ALIGN), Some(&StyleValue::Num(3)));
        assert_eq!(
            st.get_prop(StyleProp::BG_COLOR),
            Some(&StyleValue::Color(Color::hex(0x123456)))
        );
        assert_eq!(st.get_prop(StyleProp::ANIM_DURATION), Some(&StyleValue::Num(500)));
        assert_eq!(st.get_prop(StyleProp::LAYOUT), Some(&StyleValue::Num(2)));
        assert_eq!(st.get_prop(StyleProp::FLEX_GROW), Some(&StyleValue::Num(3)));
        assert!(matches!(
            st.get_prop(StyleProp::IMAGE_COLORKEY),
            Some(StyleValue::Colorkey(k)) if k.high == Color::hex(0x101010)
        ));
        assert_eq!(st.len(), 10);
        // Calling a setter twice overwrites instead of appending.
        st.set_width(11);
        assert_eq!(st.len(), 10);
        assert_eq!(st.get_prop(StyleProp::WIDTH), Some(&StyleValue::Num(11)));
    }

    #[test]
    fn every_built_in_prop_has_a_setter_in_table_order() {
        // lv_style_gen.c order: the first and last few functions.
        assert_eq!(BUILTIN_PROPS[0], StyleProp::WIDTH);
        assert_eq!(BUILTIN_PROPS[1], StyleProp::MIN_WIDTH);
        assert_eq!(BUILTIN_PROPS[9], StyleProp::ALIGN);
        assert_eq!(BUILTIN_PROPS[128], StyleProp::GRID_CELL_ROW_SPAN);
        assert_eq!(BUILTIN_PROPS[127], StyleProp::GRID_CELL_Y_ALIGN);
        // Every property with a default or a flag in lv_style.c is in the table.
        for p in [
            StyleProp::TEXT_LEADING_TRIM,
            StyleProp::ROTARY_SENSITIVITY,
            StyleProp::DROP_SHADOW_QUALITY,
            StyleProp::BITMAP_MASK_SRC,
            StyleProp::TRANSLATE_RADIAL,
        ] {
            assert!(BUILTIN_PROPS.contains(&p), "{p:?} missing from the setter table");
        }
    }
}
