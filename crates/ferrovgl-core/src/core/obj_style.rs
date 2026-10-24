//! Ported from lvgl v9.6.0: `include/lvgl/core/lv_obj_style.h`, `src/core/lv_obj_style_private.h`,
//! `src/core/lv_obj_style.c`.
//!
//! Styles attached to objects, local styles, style transitions and the
//! property lookup (`get_prop_core`, state weighting, inheritance).

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::misc::anim::AnimPath;
use crate::misc::arena::Key;
use crate::misc::color::{Color32, Opa};
use crate::misc::style::{ConstStyle, Part, State, Style, StyleProp, StyleRes, StyleSelector, StyleValue};
use crate::misc::text::TextAlign;

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

impl Ui {
    /// `lv_style_init()` for a shared style: store it in the context.
    pub fn style_create(&mut self, style: Style) -> StyleId {
        todo!()
    }

    /// Delete a shared style (also `lv_obj_remove_style` from every object).
    pub fn style_delete(&mut self, style: StyleId) {
        todo!()
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
        todo!()
    }

    /// `lv_obj_add_style()` with a constant style.
    pub fn obj_add_const_style(&mut self, obj: Obj, style: &'static ConstStyle, selector: StyleSelector) {
        todo!()
    }

    /// `lv_obj_replace_style()`
    pub fn obj_replace_style(
        &mut self,
        obj: Obj,
        old_style: StyleId,
        new_style: StyleId,
        selector: StyleSelector,
    ) -> bool {
        todo!()
    }

    /// `lv_obj_remove_style()`: `None` style removes every style matching `selector`.
    pub fn obj_remove_style(&mut self, obj: Obj, style: Option<StyleId>, selector: StyleSelector) {
        todo!()
    }

    /// `lv_obj_remove_theme()`
    pub fn obj_remove_theme(&mut self, obj: Obj, selector: StyleSelector) {
        todo!()
    }

    /// `lv_obj_remove_style_all()`
    pub fn obj_remove_style_all(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_report_style_change()`: `None` = all styles changed.
    pub fn obj_report_style_change(&mut self, style: Option<StyleId>) {
        todo!()
    }

    /// `lv_obj_refresh_style()`
    pub fn obj_refresh_style(&mut self, obj: Obj, part: Part, prop: StyleProp) {
        todo!()
    }

    /// `lv_obj_set_style_enabled()`
    pub fn obj_set_style_enabled(&mut self, obj: Obj, style: StyleId, selector: StyleSelector, en: bool) {
        todo!()
    }

    /// `lv_obj_get_style_enabled()`
    pub fn obj_get_style_enabled(&self, obj: Obj, style: StyleId, selector: StyleSelector) -> bool {
        todo!()
    }

    /// `lv_obj_enable_style_refresh()`
    pub fn obj_enable_style_refresh(&mut self, en: bool) {
        self.style_refresh = en;
    }

    /// `lv_obj_get_style_prop()`: the resolved value (local, shared, theme,
    /// inherited or default).
    pub fn obj_get_style_prop(&self, obj: Obj, part: Part, prop: StyleProp) -> StyleValue {
        todo!()
    }

    /// `lv_obj_get_style_prop_internal()`: like `obj_get_style_prop` but
    /// also reports whether the value was found.
    pub fn obj_get_style_prop_internal(
        &self,
        obj: Obj,
        part: Part,
        prop: StyleProp,
    ) -> (StyleValue, StyleRes) {
        todo!()
    }

    /// `lv_obj_has_style_prop()`
    pub fn obj_has_style_prop(&self, obj: Obj, selector: StyleSelector, prop: StyleProp) -> bool {
        todo!()
    }

    /// `lv_obj_set_local_style_prop()`
    pub fn obj_set_local_style_prop(
        &mut self,
        obj: Obj,
        prop: StyleProp,
        value: StyleValue,
        selector: StyleSelector,
    ) {
        todo!()
    }

    /// `lv_obj_get_local_style_prop()`
    pub fn obj_get_local_style_prop(
        &self,
        obj: Obj,
        prop: StyleProp,
        selector: StyleSelector,
    ) -> Option<StyleValue> {
        todo!()
    }

    /// `lv_obj_remove_local_style_prop()`
    pub fn obj_remove_local_style_prop(
        &mut self,
        obj: Obj,
        prop: StyleProp,
        selector: StyleSelector,
    ) -> bool {
        todo!()
    }

    /// `lv_obj_style_apply_color_filter()`
    pub fn obj_style_apply_color_filter(&self, obj: Obj, part: Part, v: StyleValue) -> StyleValue {
        todo!()
    }

    /// `lv_obj_fade_in()`
    pub fn obj_fade_in(&mut self, obj: Obj, time: u32, delay: u32) {
        todo!()
    }

    /// `lv_obj_fade_out()`
    pub fn obj_fade_out(&mut self, obj: Obj, time: u32, delay: u32) {
        todo!()
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
        todo!()
    }

    /// `lv_obj_style_state_compare()`: returns the comparison and the
    /// changed part.
    pub fn obj_style_state_compare(&self, obj: Obj, state1: State, state2: State) -> (StyleStateCmp, Part) {
        todo!()
    }

    /// `lv_obj_update_layer_type()`
    pub fn obj_update_layer_type(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_update_blur_status()`
    pub fn obj_update_blur_status(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_obj_set_style_pad_all()`
    pub fn obj_set_style_pad_all(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        todo!()
    }

    /// `lv_obj_set_style_pad_hor()`
    pub fn obj_set_style_pad_hor(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        todo!()
    }

    /// `lv_obj_set_style_pad_ver()`
    pub fn obj_set_style_pad_ver(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        todo!()
    }

    /// `lv_obj_set_style_margin_all()`
    pub fn obj_set_style_margin_all(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        todo!()
    }

    /// `lv_obj_set_style_margin_hor()`
    pub fn obj_set_style_margin_hor(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        todo!()
    }

    /// `lv_obj_set_style_margin_ver()`
    pub fn obj_set_style_margin_ver(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        todo!()
    }

    /// `lv_obj_set_style_pad_gap()`
    pub fn obj_set_style_pad_gap(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        todo!()
    }

    /// `lv_obj_set_style_size()`
    pub fn obj_set_style_size(&mut self, obj: Obj, width: i32, height: i32, selector: StyleSelector) {
        todo!()
    }

    /// `lv_obj_set_style_transform_scale()`
    pub fn obj_set_style_transform_scale(&mut self, obj: Obj, value: i32, selector: StyleSelector) {
        todo!()
    }

    /// `lv_obj_get_style_space_left()`
    pub fn obj_get_style_space_left(&self, obj: Obj, part: Part) -> i32 {
        todo!()
    }

    /// `lv_obj_get_style_space_right()`
    pub fn obj_get_style_space_right(&self, obj: Obj, part: Part) -> i32 {
        todo!()
    }

    /// `lv_obj_get_style_space_top()`
    pub fn obj_get_style_space_top(&self, obj: Obj, part: Part) -> i32 {
        todo!()
    }

    /// `lv_obj_get_style_space_bottom()`
    pub fn obj_get_style_space_bottom(&self, obj: Obj, part: Part) -> i32 {
        todo!()
    }

    /// `lv_obj_calculate_style_text_align()`
    pub fn obj_calculate_style_text_align(&self, obj: Obj, part: Part, txt: &str) -> TextAlign {
        todo!()
    }

    /// `lv_obj_get_style_transform_scale_x_safe()`
    pub fn obj_get_style_transform_scale_x_safe(&self, obj: Obj, part: Part) -> i32 {
        todo!()
    }

    /// `lv_obj_get_style_transform_scale_y_safe()`
    pub fn obj_get_style_transform_scale_y_safe(&self, obj: Obj, part: Part) -> i32 {
        todo!()
    }

    /// `lv_obj_get_style_opa_recursive()`
    pub fn obj_get_style_opa_recursive(&self, obj: Obj, part: Part) -> Opa {
        todo!()
    }

    /// `lv_obj_style_apply_recolor()`
    pub fn obj_style_apply_recolor(&self, obj: Obj, part: Part, color: Color32) -> Color32 {
        todo!()
    }

    /// `lv_obj_get_style_recolor_recursive()`
    pub fn obj_get_style_recolor_recursive(&self, obj: Obj, part: Part) -> Color32 {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
