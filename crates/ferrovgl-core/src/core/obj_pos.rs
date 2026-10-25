//! Ported from lvgl v9.6.0: src/core/lv_obj_pos.c
//!
//! Also `include/lvgl/core/lv_obj_pos.h`. Position, size, layout updates,
//! alignment, transforms and invalidation.
//!
//! The parity configuration (`tests/configs/full.defconfig`) leaves
//! `LV_DRAW_TRANSFORM_USE_MATRIX` off, so only the style transform branch of
//! `transform_point_array()` / `invalidate_area_core()` is ported and
//! `lv_obj_set_transform()` / `lv_obj_reset_transform()` /
//! `lv_obj_get_transform()` (no-ops in that configuration) are not.

use alloc::boxed::Box;

use crate::core::Ui;
use crate::core::obj::{LayerType, Obj, ObjFlags, ObjTreeWalkRes, SpecAttr};
use crate::display::DisplayId;
use crate::misc::area::{Align, Area, Point, transform_points};
use crate::misc::event::{EventCode, EventParam, HitTestInfo, LvResult};
use crate::misc::style::{Part, SCALE_NONE, StyleProp, StyleSelector, StyleValue};
use crate::misc::text::BaseDir;
use crate::misc::types::coord;

/// `LV_OBJ_LAYOUT_UPDATE_MAX_PASSES`
pub const OBJ_LAYOUT_UPDATE_MAX_PASSES: u32 = 100;

/// `lv_obj_point_transform_flag_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ObjPointTransformFlag {
    /// `LV_OBJ_POINT_TRANSFORM_FLAG_NONE`
    #[default]
    None = 0x00,
    /// `LV_OBJ_POINT_TRANSFORM_FLAG_RECURSIVE`
    Recursive = 0x01,
    /// `LV_OBJ_POINT_TRANSFORM_FLAG_INVERSE`
    Inverse = 0x02,
    /// `LV_OBJ_POINT_TRANSFORM_FLAG_INVERSE_RECURSIVE`
    InverseRecursive = 0x03,
}

impl ObjPointTransformFlag {
    /// `flags & RECURSIVE`
    #[inline]
    pub const fn recursive(self) -> bool {
        (self as u8) & 0x01 != 0
    }
    /// `flags & INVERSE`
    #[inline]
    pub const fn inverse(self) -> bool {
        (self as u8) & 0x02 != 0
    }
}

/// `LV_CLAMP(min, val, max)`: `LV_MAX(min, LV_MIN(val, max))` (never panics,
/// unlike `i32::clamp`, when `min > max`).
#[inline]
fn lv_clamp(min: i32, val: i32, max: i32) -> i32 {
    let v = if val < max { val } else { max };
    if min > v { min } else { v }
}

/// `lv_area_intersect(res, res, other)`: writes the intersection into `res`
/// even when it is empty, like the C function.
#[inline]
fn area_intersect_in_place(res: &mut Area, other: &Area) -> bool {
    res.x1 = res.x1.max(other.x1);
    res.y1 = res.y1.max(other.y1);
    res.x2 = res.x2.min(other.x2);
    res.y2 = res.y2.min(other.y2);
    !(res.x1 > res.x2 || res.y1 > res.y2)
}

/// Decode a numeric `LV_STYLE_ALIGN` value. `None` for values outside
/// `lv_align_t` (they take the `default:` branch of the C switches).
fn align_from_num(v: i32) -> Option<Align> {
    Some(match v {
        0 => Align::Default,
        1 => Align::TopLeft,
        2 => Align::TopMid,
        3 => Align::TopRight,
        4 => Align::BottomLeft,
        5 => Align::BottomMid,
        6 => Align::BottomRight,
        7 => Align::LeftMid,
        8 => Align::RightMid,
        9 => Align::Center,
        10 => Align::OutTopLeft,
        11 => Align::OutTopMid,
        12 => Align::OutTopRight,
        13 => Align::OutBottomLeft,
        14 => Align::OutBottomMid,
        15 => Align::OutBottomRight,
        16 => Align::OutLeftTop,
        17 => Align::OutLeftMid,
        18 => Align::OutLeftBottom,
        19 => Align::OutRightTop,
        20 => Align::OutRightMid,
        21 => Align::OutRightBottom,
        _ => return None,
    })
}

/// `size_in_effect_is_pct()`: tell whether the style which sets the final
/// size is a percentage.
fn size_in_effect_is_pct(
    unclamped: i32,
    min: i32,
    max: i32,
    size_style: i32,
    min_style: i32,
    max_style: i32,
) -> bool {
    // If the bounds are inverted the clamp always returns the min size
    if min > max {
        return coord::is_pct(min_style);
    }

    // The size is set by the bound which clamped it
    if unclamped < min {
        return coord::is_pct(min_style);
    }
    if unclamped > max {
        return coord::is_pct(max_style);
    }

    // Else the size is set by the size style. If it's equal to a bound, that
    // bound sets the same size, so use the style which is not a percentage.
    let mut is_pct = coord::is_pct(size_style);
    if is_pct && unclamped == min {
        is_pct = coord::is_pct(min_style);
    }
    if is_pct && unclamped == max {
        is_pct = coord::is_pct(max_style);
    }

    is_pct
}

/// `lv_clamp_width()`
pub fn clamp_width(width: i32, min_width: i32, max_width: i32, ref_width: i32) -> i32 {
    let mut min_width = min_width;
    let mut max_width = max_width;
    if coord::is_pct(min_width) {
        min_width = ref_width.wrapping_mul(coord::get_pct(min_width)) / 100;
    }
    if coord::is_pct(max_width) {
        max_width = ref_width.wrapping_mul(coord::get_pct(max_width)) / 100;
    }
    lv_clamp(min_width, width, max_width)
}

/// `lv_clamp_height()`
pub fn clamp_height(height: i32, min_height: i32, max_height: i32, ref_height: i32) -> i32 {
    let mut min_height = min_height;
    let mut max_height = max_height;
    if coord::is_pct(min_height) {
        min_height = ref_height.wrapping_mul(coord::get_pct(min_height)) / 100;
    }
    if coord::is_pct(max_height) {
        max_height = ref_height.wrapping_mul(coord::get_pct(max_height)) / 100;
    }
    lv_clamp(min_height, height, max_height)
}

/* Private helpers: direct field reads equivalent to the C bit-field / pointer
 * accesses (and to the trivial getters `lv_obj_get_parent()`,
 * `lv_obj_get_child_count()`, `lv_obj_is_<flag>()` for the low flags,
 * `lv_obj_get_layer_type()`), plus the `*_internal` style getters. */
impl Ui {
    /// `obj->parent`
    #[inline]
    fn pos_parent(&self, obj: Obj) -> Option<Obj> {
        self.obj(obj)?.parent.map(Obj)
    }

    /// `lv_obj_is_<flag>()` for flags stored in the object's bit fields.
    #[inline]
    fn pos_has_flag(&self, obj: Obj, f: ObjFlags) -> bool {
        self.obj(obj).is_some_and(|d| d.flags.contains(f))
    }

    /// `lv_obj_get_child_count()`
    #[inline]
    fn pos_child_count(&self, obj: Obj) -> u32 {
        self.obj(obj)
            .and_then(|d| d.spec.as_ref())
            .map_or(0, |s| s.children.len() as u32)
    }

    /// `obj->spec_attr->children[i]`
    #[inline]
    fn pos_child_at(&self, obj: Obj, i: u32) -> Option<Obj> {
        let spec = self.obj(obj)?.spec.as_ref()?;
        spec.children.get(i as usize).copied().map(Obj)
    }

    /// `obj->coords` (zero area for a stale handle).
    #[inline]
    fn pos_coords(&self, obj: Obj) -> Area {
        self.obj(obj).map_or(Area::default(), |d| d.coords)
    }

    /// `lv_obj_get_style_<prop>_internal(obj, LV_PART_MAIN)` for numeric props.
    #[inline]
    fn pos_style_num(&self, obj: Obj, prop: StyleProp) -> i32 {
        self.obj_get_style_prop(obj, Part::MAIN, prop).num()
    }

    /// `lv_obj_get_style_base_dir_internal(obj, LV_PART_MAIN) == LV_BASE_DIR_RTL`
    #[inline]
    fn pos_is_rtl(&self, obj: Obj) -> bool {
        self.pos_style_num(obj, StyleProp::BASE_DIR) == BaseDir::Rtl as i32
    }

    /// `lv_obj_set_style_<prop>(obj, v, 0)` for numeric props.
    #[inline]
    fn pos_set_style_num(&mut self, obj: Obj, prop: StyleProp, v: i32) {
        self.obj_set_local_style_prop(obj, prop, StyleValue::Num(v), StyleSelector::DEFAULT);
    }

    /// `is_transformed()`: the object or one of its parents has a transform layer.
    pub(crate) fn obj_pos_is_transformed(&self, obj: Obj) -> bool {
        let mut cur = Some(obj);
        while let Some(o) = cur {
            let Some(d) = self.obj(o) else { break };
            if d.spec
                .as_ref()
                .is_some_and(|s| s.layer_type == LayerType::Transform)
            {
                return true;
            }
            cur = d.parent.map(Obj);
        }
        false
    }
}

impl Ui {
    /// `lv_obj_set_pos()`
    pub fn obj_set_pos(&mut self, obj: Obj, x: i32, y: i32) {
        if !self.obj_is_valid(obj) {
            return;
        }

        self.obj_set_x(obj, x);
        self.obj_set_y(obj, y);
    }

    /// `lv_obj_set_x()`
    pub fn obj_set_x(&mut self, obj: Obj, x: i32) {
        if !self.obj_is_valid(obj) {
            return;
        }

        let res_x = self.obj_get_local_style_prop(obj, StyleProp::X, StyleSelector::DEFAULT);
        match res_x {
            Some(v_x) if v_x.num() == x => {}
            _ => self.pos_set_style_num(obj, StyleProp::X, x),
        }
    }

    /// `lv_obj_set_y()`
    pub fn obj_set_y(&mut self, obj: Obj, y: i32) {
        if !self.obj_is_valid(obj) {
            return;
        }

        let res_y = self.obj_get_local_style_prop(obj, StyleProp::Y, StyleSelector::DEFAULT);
        match res_y {
            Some(v_y) if v_y.num() == y => {}
            _ => self.pos_set_style_num(obj, StyleProp::Y, y),
        }
    }

    /// `lv_obj_calc_dynamic_width()`
    ///
    /// Takes `&mut self` (the skeleton had `&self`): resolving
    /// `LV_SIZE_CONTENT` sends `LV_EVENT_GET_SELF_SIZE` and calls the
    /// layout's `get_min_size_cb`.
    pub fn obj_calc_dynamic_width(&mut self, obj: Obj, prop: StyleProp) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0;
        }
        if !(prop == StyleProp::WIDTH || prop == StyleProp::MIN_WIDTH || prop == StyleProp::MAX_WIDTH) {
            crate::misc::check_arg_failed("obj_calc_dynamic_width: prop is not a width property");
            return 0;
        }

        self.calc_dynamic_width(obj, prop, None)
    }

    /// `lv_obj_calc_dynamic_height()`
    ///
    /// Takes `&mut self` (the skeleton had `&self`), see
    /// [`obj_calc_dynamic_width`](Self::obj_calc_dynamic_width).
    pub fn obj_calc_dynamic_height(&mut self, obj: Obj, prop: StyleProp) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0;
        }
        if !(prop == StyleProp::HEIGHT || prop == StyleProp::MIN_HEIGHT || prop == StyleProp::MAX_HEIGHT) {
            crate::misc::check_arg_failed("obj_calc_dynamic_height: prop is not a height property");
            return 0;
        }

        self.calc_dynamic_height(obj, prop, None)
    }

    /// `lv_obj_refr_size()`: returns `true` if the size changed.
    pub fn obj_refr_size(&mut self, obj: Obj) -> bool {
        let Some(d) = self.obj(obj) else { return false };
        let (w_layout, h_layout) = (d.w_layout, d.h_layout);

        // If the width or height is set by a layout do not modify them
        if w_layout && h_layout {
            return false;
        }

        let Some(parent) = self.pos_parent(obj) else {
            return false;
        };

        let w = if w_layout {
            self.obj_get_width(obj)
        } else {
            let mut content_width = -1;
            let unclamped_w = self.calc_dynamic_width(obj, StyleProp::WIDTH, Some(&mut content_width));
            let minw = self.calc_dynamic_width(obj, StyleProp::MIN_WIDTH, Some(&mut content_width));
            let maxw = self.calc_dynamic_width(obj, StyleProp::MAX_WIDTH, Some(&mut content_width));
            let w = lv_clamp(minw, unclamped_w, maxw);

            // If the width in effect is a percentage of a parent that is itself
            // LV_SIZE_CONTENT and not managed by a layout, the two sizes would
            // depend on each other, so `w_ignore_size` excludes this object from
            // the parent's content width calculation.
            let w_pct = size_in_effect_is_pct(
                unclamped_w,
                minw,
                maxw,
                self.pos_style_num(obj, StyleProp::WIDTH),
                self.pos_style_num(obj, StyleProp::MIN_WIDTH),
                self.pos_style_num(obj, StyleProp::MAX_WIDTH),
            );
            let ignore = w_pct
                && !self.obj(parent).is_some_and(|p| p.w_layout)
                && self.pos_style_num(parent, StyleProp::WIDTH) == coord::SIZE_CONTENT;
            if let Some(d) = self.obj_mut(obj) {
                d.w_ignore_size = ignore;
            }
            w
        };

        let h = if h_layout {
            self.obj_get_height(obj)
        } else {
            let mut content_height = -1;
            let unclamped_h = self.calc_dynamic_height(obj, StyleProp::HEIGHT, Some(&mut content_height));
            let minh = self.calc_dynamic_height(obj, StyleProp::MIN_HEIGHT, Some(&mut content_height));
            let maxh = self.calc_dynamic_height(obj, StyleProp::MAX_HEIGHT, Some(&mut content_height));
            let h = lv_clamp(minh, unclamped_h, maxh);

            // See the width branch above.
            let h_pct = size_in_effect_is_pct(
                unclamped_h,
                minh,
                maxh,
                self.pos_style_num(obj, StyleProp::HEIGHT),
                self.pos_style_num(obj, StyleProp::MIN_HEIGHT),
                self.pos_style_num(obj, StyleProp::MAX_HEIGHT),
            );
            let ignore = h_pct
                && !self.obj(parent).is_some_and(|p| p.h_layout)
                && self.pos_style_num(parent, StyleProp::HEIGHT) == coord::SIZE_CONTENT;
            if let Some(d) = self.obj_mut(obj) {
                d.h_ignore_size = ignore;
            }
            h
        };

        // Do nothing if the size is not changed. It is very important else
        // recursive resizing can occur without size change
        if self.obj_get_width(obj) == w && self.obj_get_height(obj) == h {
            return false;
        }

        // Invalidate the original area
        self.obj_invalidate(obj);

        // Save the original coordinates
        let ori = self.obj_get_coords(obj);

        // Check if the object inside the parent or not
        let parent_fit_area = self.obj_get_content_coords(parent);

        // If the object is already out of the parent and its position is
        // changes surely the scrollbars also changes so invalidate them
        let on1 = ori.is_in(&parent_fit_area, 0);
        if !on1 {
            self.obj_scrollbar_invalidate(parent);
        }

        // Set the length and height. Be sure the content is not scrolled in an
        // invalid position on the new size
        if let Some(d) = self.obj_mut(obj) {
            d.coords.y2 = d.coords.y1 + h - 1;
        }
        let rtl = self.pos_is_rtl(obj);
        if let Some(d) = self.obj_mut(obj) {
            if rtl {
                d.coords.x1 = d.coords.x2 - w + 1;
            } else {
                d.coords.x2 = d.coords.x1 + w - 1;
            }
        }

        // Call the ancestor's event handler to the object with its new coordinates
        self.obj_send_event(obj, EventCode::SIZE_CHANGED, EventParam::Area(ori));
        if !self.obj_is_valid(obj) {
            return true;
        }

        // Call the ancestor's event handler to the parent too
        if self.obj_is_valid(parent) {
            self.obj_send_event(parent, EventCode::CHILD_CHANGED, EventParam::Obj(obj));
        }
        if !self.obj_is_valid(obj) {
            return true;
        }

        // Invalidate the new area
        self.obj_invalidate(obj);

        if let Some(d) = self.obj_mut(obj) {
            d.readjust_scroll_after_layout = true;
        }

        // If the object was out of the parent invalidate the new scrollbar area
        // too. If it wasn't out of the parent but out now, also invalidate the
        // scrollbars (`on1 || (!on1 && on2)`)
        let on2 = self.pos_coords(obj).is_in(&parent_fit_area, 0);
        if (on1 || on2) && self.obj_is_valid(parent) {
            self.obj_scrollbar_invalidate(parent);
        }

        self.obj_refresh_ext_draw_size(obj);

        true
    }

    /// `lv_obj_set_size()`
    pub fn obj_set_size(&mut self, obj: Obj, w: i32, h: i32) {
        if !self.obj_is_valid(obj) {
            return;
        }

        self.obj_set_width(obj, w);
        self.obj_set_height(obj, h);
    }

    /// `lv_obj_set_width()`
    pub fn obj_set_width(&mut self, obj: Obj, w: i32) {
        if !self.obj_is_valid(obj) {
            return;
        }
        let res_w = self.obj_get_local_style_prop(obj, StyleProp::WIDTH, StyleSelector::DEFAULT);
        match res_w {
            Some(v_w) if v_w.num() == w => {}
            _ => self.pos_set_style_num(obj, StyleProp::WIDTH, w),
        }
    }

    /// `lv_obj_set_height()`
    pub fn obj_set_height(&mut self, obj: Obj, h: i32) {
        if !self.obj_is_valid(obj) {
            return;
        }
        let res_h = self.obj_get_local_style_prop(obj, StyleProp::HEIGHT, StyleSelector::DEFAULT);
        match res_h {
            Some(v_h) if v_h.num() == h => {}
            _ => self.pos_set_style_num(obj, StyleProp::HEIGHT, h),
        }
    }

    /// `lv_obj_set_content_width()`
    pub fn obj_set_content_width(&mut self, obj: Obj, w: i32) {
        if !self.obj_is_valid(obj) {
            return;
        }

        let left = self.obj_get_style_space_left(obj, Part::MAIN);
        let right = self.obj_get_style_space_right(obj, Part::MAIN);
        self.obj_set_width(obj, w + left + right);
    }

    /// `lv_obj_set_content_height()`
    pub fn obj_set_content_height(&mut self, obj: Obj, h: i32) {
        if !self.obj_is_valid(obj) {
            return;
        }

        let top = self.obj_get_style_space_top(obj, Part::MAIN);
        let bottom = self.obj_get_style_space_bottom(obj, Part::MAIN);
        self.obj_set_height(obj, h + top + bottom);
    }

    /// `lv_obj_set_layout()`
    pub fn obj_set_layout(&mut self, obj: Obj, layout: u32) {
        if !self.obj_is_valid(obj) {
            return;
        }

        self.pos_set_style_num(obj, StyleProp::LAYOUT, layout as i32);

        self.obj_mark_layout_as_dirty(obj);
    }

    /// `lv_obj_is_layout_positioned()`
    pub fn obj_is_layout_positioned(&self, obj: Obj) -> bool {
        if !self.obj_is_valid(obj) {
            return false;
        }

        if self.pos_has_flag(obj, ObjFlags::HIDDEN)
            || self.pos_has_flag(obj, ObjFlags::IGNORE_LAYOUT)
            || self.pos_has_flag(obj, ObjFlags::FLOATING)
        {
            return false;
        }

        let Some(parent) = self.pos_parent(obj) else {
            return false;
        };

        let layout = self.pos_style_num(parent, StyleProp::LAYOUT) as u32;
        layout != 0
    }

    /// `lv_obj_mark_layout_as_dirty()`
    pub fn obj_mark_layout_as_dirty(&mut self, obj: Obj) {
        let Some(d) = self.obj_mut(obj) else { return };

        d.layout_inv = true;

        // Mark the screen as dirty too to mark that there is something to do on this screen
        let scr = self.obj_get_screen(obj);
        if let Some(s) = self.obj_mut(scr) {
            s.scr_layout_inv = true;
        }

        // Make the display refreshing
        if let Some(disp) = self.obj_get_display(scr) {
            self.display_send_event(disp, EventCode::REFR_REQUEST, EventParam::None);
        }
    }

    /// `lv_obj_update_layout()`
    pub fn obj_update_layout(&mut self, obj: Obj) {
        if !self.obj_is_valid(obj) {
            return;
        }

        if self.layout_update_mutex {
            // Already running, returning
            return;
        }
        self.layout_update_mutex = true;

        let scr = self.obj_get_screen(obj);
        // Repeat until there are no more layout invalidations
        let mut pass_cnt: u32 = 0;
        while self.obj(scr).is_some_and(|s| s.scr_layout_inv) {
            if pass_cnt >= OBJ_LAYOUT_UPDATE_MAX_PASSES {
                #[cfg(feature = "log")]
                log::error!(
                    "Layout of screen (class: '{}') didn't settle in {} passes, giving up. Some sizes \
                     probably depend on each other circularly.",
                    self.obj(scr).map_or("", |s| s.class.name),
                    OBJ_LAYOUT_UPDATE_MAX_PASSES
                );
                // Reached only if the assert handler returns
                if let Some(s) = self.obj_mut(scr) {
                    s.scr_layout_inv = false;
                }
                break;
            }
            pass_cnt += 1;
            if let Some(s) = self.obj_mut(scr) {
                s.scr_layout_inv = false;
            }
            self.layout_update_core(scr);
        }

        if self.obj_is_valid(scr)
            && let Some(disp) = self.obj_get_display(scr)
        {
            self.display_send_event(disp, EventCode::UPDATE_LAYOUT_COMPLETED, EventParam::None);
        }
        self.layout_update_mutex = false;
    }

    /// `lv_obj_set_align()`
    pub fn obj_set_align(&mut self, obj: Obj, align: Align) {
        if !self.obj_is_valid(obj) {
            return;
        }

        self.pos_set_style_num(obj, StyleProp::ALIGN, align as i32);
    }

    /// `lv_obj_align()`
    pub fn obj_align(&mut self, obj: Obj, align: Align, x_ofs: i32, y_ofs: i32) {
        if !self.obj_is_valid(obj) {
            return;
        }
        self.pos_set_style_num(obj, StyleProp::ALIGN, align as i32);
        self.obj_set_pos(obj, x_ofs, y_ofs);
    }

    /// `lv_obj_align_to()`
    ///
    /// `base` is not optional here (C accepts `NULL` = the parent); pass the
    /// parent explicitly.
    pub fn obj_align_to(&mut self, obj: Obj, base: Obj, align: Align, x_ofs: i32, y_ofs: i32) {
        if !self.obj_is_valid(obj) {
            return;
        }

        self.obj_update_layout(obj);
        if !self.obj_is_valid(obj) {
            return;
        }
        if !self.obj_is_valid(base) {
            #[cfg(feature = "log")]
            log::warn!("lv_obj_align_to: base is NULL");
            return;
        }

        let mut x: i32 = 0;
        let mut y: i32 = 0;

        let Some(parent) = self.pos_parent(obj) else {
            #[cfg(feature = "log")]
            log::warn!("lv_obj_align_to: parent is NULL");
            return;
        };

        let pleft = self.obj_get_style_space_left(parent, Part::MAIN);
        let ptop = self.obj_get_style_space_top(parent, Part::MAIN);

        let bleft = self.obj_get_style_space_left(base, Part::MAIN);
        let btop = self.obj_get_style_space_top(base, Part::MAIN);

        let mut align = align;
        if align == Align::Default {
            if self.pos_is_rtl(base) {
                align = Align::TopRight;
            } else {
                align = Align::TopLeft;
            }
        }

        match align {
            Align::Center => {
                x = self.obj_get_content_width(base) / 2 - self.obj_get_width(obj) / 2 + bleft;
                y = self.obj_get_content_height(base) / 2 - self.obj_get_height(obj) / 2 + btop;
            }
            Align::TopLeft => {
                x = bleft;
                y = btop;
            }
            Align::TopMid => {
                x = self.obj_get_content_width(base) / 2 - self.obj_get_width(obj) / 2 + bleft;
                y = btop;
            }
            Align::TopRight => {
                x = self.obj_get_content_width(base) - self.obj_get_width(obj) + bleft;
                y = btop;
            }
            Align::BottomLeft => {
                x = bleft;
                y = self.obj_get_content_height(base) - self.obj_get_height(obj) + btop;
            }
            Align::BottomMid => {
                x = self.obj_get_content_width(base) / 2 - self.obj_get_width(obj) / 2 + bleft;
                y = self.obj_get_content_height(base) - self.obj_get_height(obj) + btop;
            }
            Align::BottomRight => {
                x = self.obj_get_content_width(base) - self.obj_get_width(obj) + bleft;
                y = self.obj_get_content_height(base) - self.obj_get_height(obj) + btop;
            }
            Align::LeftMid => {
                x = bleft;
                y = self.obj_get_content_height(base) / 2 - self.obj_get_height(obj) / 2 + btop;
            }
            Align::RightMid => {
                x = self.obj_get_content_width(base) - self.obj_get_width(obj) + bleft;
                y = self.obj_get_content_height(base) / 2 - self.obj_get_height(obj) / 2 + btop;
            }
            Align::OutTopLeft => {
                x = 0;
                y = -self.obj_get_height(obj);
            }
            Align::OutTopMid => {
                x = self.obj_get_width(base) / 2 - self.obj_get_width(obj) / 2;
                y = -self.obj_get_height(obj);
            }
            Align::OutTopRight => {
                x = self.obj_get_width(base) - self.obj_get_width(obj);
                y = -self.obj_get_height(obj);
            }
            Align::OutBottomLeft => {
                x = 0;
                y = self.obj_get_height(base);
            }
            Align::OutBottomMid => {
                x = self.obj_get_width(base) / 2 - self.obj_get_width(obj) / 2;
                y = self.obj_get_height(base);
            }
            Align::OutBottomRight => {
                x = self.obj_get_width(base) - self.obj_get_width(obj);
                y = self.obj_get_height(base);
            }
            Align::OutLeftTop => {
                x = -self.obj_get_width(obj);
                y = 0;
            }
            Align::OutLeftMid => {
                x = -self.obj_get_width(obj);
                y = self.obj_get_height(base) / 2 - self.obj_get_height(obj) / 2;
            }
            Align::OutLeftBottom => {
                x = -self.obj_get_width(obj);
                y = self.obj_get_height(base) - self.obj_get_height(obj);
            }
            Align::OutRightTop => {
                x = self.obj_get_width(base);
                y = 0;
            }
            Align::OutRightMid => {
                x = self.obj_get_width(base);
                y = self.obj_get_height(base) / 2 - self.obj_get_height(obj) / 2;
            }
            Align::OutRightBottom => {
                x = self.obj_get_width(base);
                y = self.obj_get_height(base) - self.obj_get_height(obj);
            }
            Align::Default => {}
        }

        let mut x_ofs = x_ofs;
        let mut y_ofs = y_ofs;
        if coord::is_pct(x_ofs) {
            x_ofs = self.obj_get_width(base).wrapping_mul(coord::get_pct(x_ofs)) / 100;
        }
        if coord::is_pct(y_ofs) {
            y_ofs = self.obj_get_height(base).wrapping_mul(coord::get_pct(y_ofs)) / 100;
        }
        let base_coords = self.pos_coords(base);
        let parent_coords = self.pos_coords(parent);
        if self.pos_is_rtl(parent) {
            x += x_ofs + base_coords.x1 - parent_coords.x1 + self.obj_get_scroll_right(parent) - pleft;
        } else {
            x += x_ofs + base_coords.x1 - parent_coords.x1 + self.obj_get_scroll_left(parent) - pleft;
        }
        y += y_ofs + base_coords.y1 - parent_coords.y1 + self.obj_get_scroll_top(parent) - ptop;
        self.pos_set_style_num(obj, StyleProp::ALIGN, Align::TopLeft as i32);
        self.obj_set_pos(obj, x, y);
    }

    /// `lv_obj_center()`
    pub fn obj_center(&mut self, obj: Obj) {
        self.obj_align(obj, Align::Center, 0, 0);
    }

    /// `lv_obj_get_coords()`
    pub fn obj_get_coords(&self, obj: Obj) -> Area {
        self.pos_coords(obj)
    }

    /// `lv_obj_get_x()`
    pub fn obj_get_x(&self, obj: Obj) -> i32 {
        let Some(d) = self.obj(obj) else { return 0 };

        let rel_x;
        if let Some(parent) = d.parent.map(Obj) {
            let mut r = d.coords.x1 - self.pos_coords(parent).x1;
            r += self.obj_get_scroll_x(parent);
            r -= self.obj_get_style_space_left(parent, Part::MAIN);
            rel_x = r;
        } else {
            rel_x = d.coords.x1;
        }
        rel_x
    }

    /// `lv_obj_get_x2()`
    pub fn obj_get_x2(&self, obj: Obj) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0;
        }

        self.obj_get_x(obj) + self.obj_get_width(obj)
    }

    /// `lv_obj_get_y()`
    pub fn obj_get_y(&self, obj: Obj) -> i32 {
        let Some(d) = self.obj(obj) else { return 0 };

        let rel_y;
        if let Some(parent) = d.parent.map(Obj) {
            let mut r = d.coords.y1 - self.pos_coords(parent).y1;
            r += self.obj_get_scroll_y(parent);
            r -= self.obj_get_style_space_top(parent, Part::MAIN);
            rel_y = r;
        } else {
            rel_y = d.coords.y1;
        }
        rel_y
    }

    /// `lv_obj_get_y2()`
    pub fn obj_get_y2(&self, obj: Obj) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0;
        }

        self.obj_get_y(obj) + self.obj_get_height(obj)
    }

    /// `lv_obj_get_x_aligned()`
    pub fn obj_get_x_aligned(&self, obj: Obj) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0;
        }

        self.pos_style_num(obj, StyleProp::X)
    }

    /// `lv_obj_get_y_aligned()`
    pub fn obj_get_y_aligned(&self, obj: Obj) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0;
        }

        self.pos_style_num(obj, StyleProp::Y)
    }

    /// `lv_obj_get_width()`
    pub fn obj_get_width(&self, obj: Obj) -> i32 {
        self.obj(obj).map_or(0, |d| d.coords.width())
    }

    /// `lv_obj_get_height()`
    pub fn obj_get_height(&self, obj: Obj) -> i32 {
        self.obj(obj).map_or(0, |d| d.coords.height())
    }

    /// `lv_obj_get_content_width()`
    pub fn obj_get_content_width(&self, obj: Obj) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0;
        }

        let left = self.obj_get_style_space_left(obj, Part::MAIN);
        let right = self.obj_get_style_space_right(obj, Part::MAIN);

        self.obj_get_width(obj) - left - right
    }

    /// `lv_obj_get_content_height()`
    pub fn obj_get_content_height(&self, obj: Obj) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0;
        }

        let top = self.obj_get_style_space_top(obj, Part::MAIN);
        let bottom = self.obj_get_style_space_bottom(obj, Part::MAIN);

        self.obj_get_height(obj) - top - bottom
    }

    /// `lv_obj_get_content_coords()`
    pub fn obj_get_content_coords(&self, obj: Obj) -> Area {
        if !self.obj_is_valid(obj) {
            return Area::default();
        }

        let mut area = self.obj_get_coords(obj);
        area.x1 += self.obj_get_style_space_left(obj, Part::MAIN);
        area.x2 -= self.obj_get_style_space_right(obj, Part::MAIN);
        area.y1 += self.obj_get_style_space_top(obj, Part::MAIN);
        area.y2 -= self.obj_get_style_space_bottom(obj, Part::MAIN);
        area
    }

    /// `lv_obj_get_self_width()`
    pub fn obj_get_self_width(&mut self, obj: Obj) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0;
        }

        let mut p = Point::new(0, coord::MIN);
        self.obj_send_event(obj, EventCode::GET_SELF_SIZE, EventParam::SelfSize(&mut p));
        p.x
    }

    /// `lv_obj_get_self_height()`
    pub fn obj_get_self_height(&mut self, obj: Obj) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0;
        }

        let mut p = Point::new(coord::MIN, 0);
        self.obj_send_event(obj, EventCode::GET_SELF_SIZE, EventParam::SelfSize(&mut p));
        p.y
    }

    /// `lv_obj_get_style_clamped_width()`
    ///
    /// Takes `&mut self` (the skeleton had `&self`), see
    /// [`obj_calc_dynamic_width`](Self::obj_calc_dynamic_width).
    pub fn obj_get_style_clamped_width(&mut self, obj: Obj) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0;
        }

        let mut content_width = -1;
        let mut w = self.calc_dynamic_width(obj, StyleProp::WIDTH, Some(&mut content_width));
        let minw = self.calc_dynamic_width(obj, StyleProp::MIN_WIDTH, Some(&mut content_width));
        let maxw = self.calc_dynamic_width(obj, StyleProp::MAX_WIDTH, Some(&mut content_width));
        if w <= minw {
            w = self.pos_style_num(obj, StyleProp::MIN_WIDTH);
        } else if w >= maxw {
            w = self.pos_style_num(obj, StyleProp::MAX_WIDTH);
        } else {
            w = self.pos_style_num(obj, StyleProp::WIDTH);
        }
        w
    }

    /// `lv_obj_get_style_clamped_height()`
    ///
    /// Takes `&mut self` (the skeleton had `&self`), see
    /// [`obj_calc_dynamic_width`](Self::obj_calc_dynamic_width).
    pub fn obj_get_style_clamped_height(&mut self, obj: Obj) -> i32 {
        if !self.obj_is_valid(obj) {
            return 0;
        }

        let mut content_height = -1;
        let mut h = self.calc_dynamic_height(obj, StyleProp::HEIGHT, Some(&mut content_height));
        let minh = self.calc_dynamic_height(obj, StyleProp::MIN_HEIGHT, Some(&mut content_height));
        let maxh = self.calc_dynamic_height(obj, StyleProp::MAX_HEIGHT, Some(&mut content_height));
        if h <= minh {
            h = self.pos_style_num(obj, StyleProp::MIN_HEIGHT);
        } else if h >= maxh {
            h = self.pos_style_num(obj, StyleProp::MAX_HEIGHT);
        } else {
            h = self.pos_style_num(obj, StyleProp::HEIGHT);
        }
        h
    }

    /// `lv_obj_is_style_any_width_content()`
    pub fn obj_is_style_any_width_content(&self, obj: Obj) -> bool {
        if !self.obj_is_valid(obj) {
            return false;
        }

        let w = self.pos_style_num(obj, StyleProp::WIDTH);
        let minw = self.pos_style_num(obj, StyleProp::MIN_WIDTH);
        let maxw = self.pos_style_num(obj, StyleProp::MAX_WIDTH);
        w == coord::SIZE_CONTENT || minw == coord::SIZE_CONTENT || maxw == coord::SIZE_CONTENT
    }

    /// `lv_obj_is_style_any_height_content()`
    pub fn obj_is_style_any_height_content(&self, obj: Obj) -> bool {
        if !self.obj_is_valid(obj) {
            return false;
        }

        let h = self.pos_style_num(obj, StyleProp::HEIGHT);
        let minh = self.pos_style_num(obj, StyleProp::MIN_HEIGHT);
        let maxh = self.pos_style_num(obj, StyleProp::MAX_HEIGHT);
        h == coord::SIZE_CONTENT || minh == coord::SIZE_CONTENT || maxh == coord::SIZE_CONTENT
    }

    /// `lv_obj_is_width_min()`
    ///
    /// Takes `&mut self` (the skeleton had `&self`), see
    /// [`obj_calc_dynamic_width`](Self::obj_calc_dynamic_width).
    pub fn obj_is_width_min(&mut self, obj: Obj) -> bool {
        if !self.obj_is_valid(obj) {
            return false;
        }

        let minw = self.obj_calc_dynamic_width(obj, StyleProp::MIN_WIDTH);
        let w = self.obj_get_width(obj);
        w == minw
    }

    /// `lv_obj_is_height_min()`
    ///
    /// Takes `&mut self` (the skeleton had `&self`), see
    /// [`obj_calc_dynamic_width`](Self::obj_calc_dynamic_width).
    pub fn obj_is_height_min(&mut self, obj: Obj) -> bool {
        if !self.obj_is_valid(obj) {
            return false;
        }

        let minh = self.obj_calc_dynamic_height(obj, StyleProp::MIN_HEIGHT);
        let h = self.obj_get_height(obj);
        h == minh
    }

    /// `lv_obj_is_width_max()`
    ///
    /// Takes `&mut self` (the skeleton had `&self`), see
    /// [`obj_calc_dynamic_width`](Self::obj_calc_dynamic_width).
    pub fn obj_is_width_max(&mut self, obj: Obj) -> bool {
        if !self.obj_is_valid(obj) {
            return false;
        }

        let maxw = self.obj_calc_dynamic_width(obj, StyleProp::MAX_WIDTH);
        let w = self.obj_get_width(obj);
        w == maxw
    }

    /// `lv_obj_is_height_max()`
    ///
    /// Takes `&mut self` (the skeleton had `&self`), see
    /// [`obj_calc_dynamic_width`](Self::obj_calc_dynamic_width).
    pub fn obj_is_height_max(&mut self, obj: Obj) -> bool {
        if !self.obj_is_valid(obj) {
            return false;
        }

        let maxh = self.obj_calc_dynamic_height(obj, StyleProp::MAX_HEIGHT);
        let h = self.obj_get_height(obj);
        h == maxh
    }

    /// `lv_obj_refresh_self_size()`
    pub fn obj_refresh_self_size(&mut self, obj: Obj) -> bool {
        if !self.obj_is_valid(obj) {
            return false;
        }

        if !self.obj_is_style_any_width_content(obj) && !self.obj_is_style_any_height_content(obj) {
            return false;
        }

        // Refresh the parent's layout, because the children size is in some way
        // dependent on its contents we need to force a recalculation of the
        // parents layout
        if let Some(parent) = self.pos_parent(obj) {
            if let Some(p) = self.obj_mut(parent) {
                p.w_layout = false;
                p.h_layout = false;
            }
            self.obj_mark_layout_as_dirty(parent);
        }
        self.obj_mark_layout_as_dirty(obj);
        true
    }

    /// `lv_obj_refr_pos()`
    pub fn obj_refr_pos(&mut self, obj: Obj) {
        if !self.obj_is_valid(obj) {
            return;
        }

        if self.obj_is_layout_positioned(obj) {
            return;
        }

        let parent = self.pos_parent(obj);
        let mut x = self.pos_style_num(obj, StyleProp::X);
        let mut y = self.pos_style_num(obj, StyleProp::Y);

        let Some(parent) = parent else {
            self.obj_move_to(obj, x, y);
            return;
        };

        // Handle percentage value
        let pw = self.obj_get_content_width(parent);
        let ph = self.obj_get_content_height(parent);
        if coord::is_pct(x) {
            if self.pos_style_num(parent, StyleProp::WIDTH) == coord::SIZE_CONTENT {
                x = 0; // Avoid circular dependency
            } else {
                x = pw.wrapping_mul(coord::get_pct(x)) / 100;
            }
        }

        if coord::is_pct(y) {
            if self.pos_style_num(parent, StyleProp::HEIGHT) == coord::SIZE_CONTENT {
                y = 0; // Avoid circular dependency
            }
            // Upstream has no `else` here (with `y = 0` this yields 0 anyway).
            y = ph.wrapping_mul(coord::get_pct(y)) / 100;
        }

        // Handle percentage value of translate
        let mut tr_x = self.pos_style_num(obj, StyleProp::TRANSLATE_X);
        let mut tr_y = self.pos_style_num(obj, StyleProp::TRANSLATE_Y);
        let w = self.obj_get_width(obj);
        let h = self.obj_get_height(obj);
        if coord::is_pct(tr_x) {
            tr_x = w.wrapping_mul(coord::get_pct(tr_x)) / 100;
        }
        if coord::is_pct(tr_y) {
            tr_y = h.wrapping_mul(coord::get_pct(tr_y)) / 100;
        }

        // Use the translation
        x += tr_x;
        y += tr_y;

        let mut align = align_from_num(self.pos_style_num(obj, StyleProp::ALIGN));

        if align == Some(Align::Default) {
            align = Some(Align::TopLeft);
        }

        let rtl = self.pos_is_rtl(parent);

        if rtl {
            align = match align {
                Some(Align::TopLeft) => Some(Align::TopRight),
                Some(Align::TopRight) => Some(Align::TopLeft),
                Some(Align::LeftMid) => Some(Align::RightMid),
                Some(Align::RightMid) => Some(Align::LeftMid),
                Some(Align::BottomLeft) => Some(Align::BottomRight),
                Some(Align::BottomRight) => Some(Align::BottomLeft),
                other => other,
            };
        }

        match align {
            Some(Align::TopLeft) => {}
            Some(Align::TopMid) => {
                x = if rtl {
                    pw / 2 - w / 2 - x
                } else {
                    pw / 2 - w / 2 + x
                };
            }
            Some(Align::TopRight) => {
                x = if rtl { pw - w - x } else { pw - w + x };
            }
            Some(Align::LeftMid) => {
                y += ph / 2 - h / 2;
            }
            Some(Align::BottomLeft) => {
                y += ph - h;
            }
            Some(Align::BottomMid) => {
                x = if rtl {
                    pw / 2 - w / 2 - x
                } else {
                    pw / 2 - w / 2 + x
                };
                y += ph - h;
            }
            Some(Align::BottomRight) => {
                x = if rtl { pw - w - x } else { pw - w + x };
                y += ph - h;
            }
            Some(Align::RightMid) => {
                x = if rtl { pw - w - x } else { pw - w + x };
                y += ph / 2 - h / 2;
            }
            Some(Align::Center) => {
                x = if rtl {
                    pw / 2 - w / 2 - x
                } else {
                    pw / 2 - w / 2 + x
                };
                y += ph / 2 - h / 2;
            }
            _ => {}
        }

        self.obj_move_to(obj, x, y);
    }

    /// `lv_obj_move_to()`
    pub fn obj_move_to(&mut self, obj: Obj, x: i32, y: i32) {
        let Some(d) = self.obj(obj) else { return };

        // Convert x and y to absolute coordinates
        let parent = d.parent.map(Obj);
        let mut x = x;
        let mut y = y;

        if let Some(parent) = parent {
            let pc = self.pos_coords(parent);
            if self.pos_has_flag(obj, ObjFlags::FLOATING) {
                x += pc.x1;
                y += pc.y1;
            } else {
                x += pc.x1 - self.obj_get_scroll_x(parent);
                y += pc.y1 - self.obj_get_scroll_y(parent);
            }

            x += self.obj_get_style_space_left(parent, Part::MAIN);
            y += self.obj_get_style_space_top(parent, Part::MAIN);
        }

        // Calculate and set the movement
        let coords = self.pos_coords(obj);
        let diff = Point::new(x - coords.x1, y - coords.y1);

        // Do nothing if the position is not changed. It is very important else
        // recursive positioning can occur without position change
        if diff.x == 0 && diff.y == 0 {
            return;
        }

        // Invalidate the original area
        self.obj_invalidate(obj);

        // Save the original coordinates
        let ori = self.obj_get_coords(obj);

        // Check if the object inside the parent or not
        let mut parent_fit_area = Area::default();
        let mut on1 = false;
        if let Some(parent) = parent {
            parent_fit_area = self.obj_get_content_coords(parent);

            // If the object is already out of the parent and its position is
            // changes surely the scrollbars also changes so invalidate them
            on1 = ori.is_in(&parent_fit_area, 0);
            if !on1 {
                self.obj_scrollbar_invalidate(parent);
            }
        }

        if let Some(d) = self.obj_mut(obj) {
            d.coords.x1 += diff.x;
            d.coords.y1 += diff.y;
            d.coords.x2 += diff.x;
            d.coords.y2 += diff.y;
        }

        self.obj_move_children_by(obj, diff.x, diff.y, false);

        // Call the ancestor's event handler to the parent too
        if let Some(parent) = parent
            && self.obj_is_valid(parent)
        {
            self.obj_send_event(parent, EventCode::CHILD_CHANGED, EventParam::Obj(obj));
        }
        if !self.obj_is_valid(obj) {
            return;
        }

        // Invalidate the new area
        self.obj_invalidate(obj);

        // If the object was out of the parent invalidate the new scrollbar area
        // too. If it wasn't out of the parent but out now, also invalidate the
        // scrollbars (`on1 || (!on1 && on2)`)
        if let Some(parent) = parent {
            let on2 = self.pos_coords(obj).is_in(&parent_fit_area, 0);
            if (on1 || on2) && self.obj_is_valid(parent) {
                self.obj_scrollbar_invalidate(parent);
            }
        }
    }

    /// `lv_obj_move_children_by()`
    pub fn obj_move_children_by(&mut self, obj: Obj, x_diff: i32, y_diff: i32, ignore_floating: bool) {
        if !self.obj_is_valid(obj) {
            return;
        }

        let child_cnt = self.pos_child_count(obj);
        for i in 0..child_cnt {
            let Some(child) = self.pos_child_at(obj, i) else {
                break;
            };
            if ignore_floating && self.pos_has_flag(child, ObjFlags::FLOATING) {
                continue;
            }
            if let Some(c) = self.obj_mut(child) {
                c.coords.x1 += x_diff;
                c.coords.y1 += y_diff;
                c.coords.x2 += x_diff;
                c.coords.y2 += y_diff;
            }

            self.obj_move_children_by(child, x_diff, y_diff, false);
        }
    }

    /// `lv_obj_transform_point()`
    pub fn obj_transform_point(&self, obj: Obj, p: &mut Point, flags: ObjPointTransformFlag) {
        if !self.obj_is_valid(obj) {
            return;
        }

        self.obj_transform_point_array(obj, core::slice::from_mut(p), flags);
    }

    /// `lv_obj_transform_point_array()`
    pub fn obj_transform_point_array(&self, obj: Obj, points: &mut [Point], flags: ObjPointTransformFlag) {
        let Some(d) = self.obj(obj) else { return };

        let layer_type = d.spec.as_ref().map_or(LayerType::None, |s| s.layer_type);
        let do_tranf = layer_type == LayerType::Transform;
        let recursive = flags.recursive();
        let inverse = flags.inverse();
        let parent = d.parent.map(Obj);
        if inverse {
            if recursive && let Some(parent) = parent {
                self.obj_transform_point_array(parent, points, flags);
            }
            if do_tranf {
                self.transform_point_array(obj, points, inverse);
            }
        } else {
            if do_tranf {
                self.transform_point_array(obj, points, inverse);
            }
            if recursive && let Some(parent) = parent {
                self.obj_transform_point_array(parent, points, flags);
            }
        }
    }

    /// `lv_obj_get_transformed_area()`
    pub fn obj_get_transformed_area(&self, obj: Obj, area: &mut Area, flags: ObjPointTransformFlag) {
        if !self.obj_is_valid(obj) {
            return;
        }

        let mut p = [
            Point::new(area.x1, area.y1),
            Point::new(area.x1, area.y2 + 1),
            Point::new(area.x2 + 1, area.y1),
            Point::new(area.x2 + 1, area.y2 + 1),
        ];

        self.obj_transform_point_array(obj, &mut p, flags);

        area.x1 = p[0].x.min(p[1].x).min(p[2].x.min(p[3].x));
        area.x2 = p[0].x.max(p[1].x).max(p[2].x.max(p[3].x));
        area.y1 = p[0].y.min(p[1].y).min(p[2].y.min(p[3].y));
        area.y2 = p[0].y.max(p[1].y).max(p[2].y.max(p[3].y));
    }

    /// `blur_expand_walk_cb()` of `lv_obj_pos.c`.
    fn blur_expand_walk_cb(&mut self, obj: Obj, disp: DisplayId) -> ObjTreeWalkRes {
        let Some(d) = self.obj(obj) else {
            return ObjTreeWalkRes::Next;
        };
        if !d.has_blur {
            return ObjTreeWalkRes::Next;
        }

        // invalidate_area_core() expects untransformed coordinates and applies
        // the transform itself
        let ext_size = self.obj_get_ext_draw_size(obj);
        let mut obj_coords = self.pos_coords(obj);
        obj_coords.increase(ext_size, ext_size);

        // The invalidated areas are in screen coordinates so check the overlap
        // against the transformed area
        let mut scr_coords = obj_coords;
        if self.obj_pos_is_transformed(obj) {
            self.obj_get_transformed_area(obj, &mut scr_coords, ObjPointTransformFlag::Recursive);
        }

        let inv_p = self.displays.get(disp.0).map_or(0, |d| d.inv_p);
        for i in 0..inv_p as usize {
            let Some(dd) = self.displays.get(disp.0) else {
                break;
            };
            // The join pass runs after this and its state is reset each frame
            debug_assert!(!dd.inv_area_joined.get(i).copied().unwrap_or(false));
            let Some(inv) = dd.inv_areas.get(i).copied() else {
                break;
            };
            if inv.is_on(&scr_coords) {
                self.invalidate_area_core(obj, &mut obj_coords);

                // No need to check the children as the widget is already
                // invalidated which will redraw the children too
                return ObjTreeWalkRes::SkipChildren;
            }
        }

        ObjTreeWalkRes::Next
    }

    /// `lv_obj_invalidate_expand_blur()`: the body lives here because it uses
    /// the private helpers of `lv_obj_pos.c`; `refr.rs`'s
    /// `obj_invalidate_expand_blur` should delegate to it.
    pub fn obj_pos_invalidate_expand_blur(&mut self, disp: DisplayId) {
        let Some(d) = self.displays.get(disp.0) else {
            return;
        };
        if d.inv_p == 0 {
            return;
        }

        // There is nothing to expand if no widget has blur or drop shadow
        if self.blur_obj_cnt == 0 {
            return;
        }

        loop {
            let Some(d) = self.displays.get(disp.0) else {
                return;
            };
            let prev_inv_p = d.inv_p;

            let act_scr = d.act_scr.map(Obj);
            self.obj_tree_walk(act_scr, &mut |ui, o| ui.blur_expand_walk_cb(o, disp));
            let prev_scr = self.displays.get(disp.0).and_then(|d| d.prev_scr).map(Obj);
            if prev_scr.is_some() {
                self.obj_tree_walk(prev_scr, &mut |ui, o| ui.blur_expand_walk_cb(o, disp));
            }
            let sys_layer = self.displays.get(disp.0).and_then(|d| d.sys_layer).map(Obj);
            self.obj_tree_walk(sys_layer, &mut |ui, o| ui.blur_expand_walk_cb(o, disp));
            let top_layer = self.displays.get(disp.0).and_then(|d| d.top_layer).map(Obj);
            self.obj_tree_walk(top_layer, &mut |ui, o| ui.blur_expand_walk_cb(o, disp));
            let bottom_layer = self.displays.get(disp.0).and_then(|d| d.bottom_layer).map(Obj);
            self.obj_tree_walk(bottom_layer, &mut |ui, o| ui.blur_expand_walk_cb(o, disp));

            // Repeat while new areas keep being added. An overflow in
            // lv_inv_area() can instead shrink inv_p (it collapses to a single
            // whole-screen area) and the loop exits, which is correct because
            // the whole screen covers every blur object.
            let Some(d) = self.displays.get(disp.0) else {
                return;
            };
            if d.inv_p <= prev_inv_p {
                break;
            }
        }
    }

    /// `lv_obj_invalidate_area()`
    pub fn obj_invalidate_area(&mut self, obj: Obj, area: &Area) -> LvResult {
        let Some(d) = self.obj(obj) else {
            return LvResult::Invalid;
        };
        let has_blur = d.has_blur;

        let disp = self.obj_get_display(obj);
        if !self.display_is_invalidation_enabled(disp) {
            return LvResult::Invalid;
        }

        // If there are blurred or drop-shadow parts the whole widget needs to be
        // invalidated as these can't be calculated partially.
        if has_blur {
            self.obj_invalidate(obj)
        } else {
            self.obj_invalidate_area_internal(obj, area)
        }
    }

    /// `lv_obj_invalidate()`
    pub fn obj_invalidate(&mut self, obj: Obj) -> LvResult {
        if !self.obj_is_valid(obj) {
            return LvResult::Invalid;
        }

        let disp = self.obj_get_display(obj);
        if !self.display_is_invalidation_enabled(disp) {
            return LvResult::Invalid;
        }

        // Truncate the area to the object
        let ext_size = self.obj_get_ext_draw_size(obj);
        let mut obj_coords = self.pos_coords(obj);
        obj_coords.x1 -= ext_size;
        obj_coords.y1 -= ext_size;
        obj_coords.x2 += ext_size;
        obj_coords.y2 += ext_size;

        self.obj_invalidate_area_internal(obj, &obj_coords)
    }

    /// `lv_obj_area_is_visible()`: clips `area` to the visible part.
    pub fn obj_area_is_visible(&self, obj: Obj, area: &mut Area) -> bool {
        if !self.obj_is_valid(obj) {
            return false;
        }

        if self.pos_has_flag(obj, ObjFlags::HIDDEN) {
            return false;
        }

        // Invalidate the object only if it belongs to the current or previous
        // or one of the layers'
        let obj_scr = self.obj_get_screen(obj);
        let disp = self.obj_get_display(obj_scr);
        let scr = Some(obj_scr);
        if scr != self.display_get_screen_active(disp)
            && scr != self.display_get_screen_prev(disp)
            && scr != self.display_get_layer_bottom(disp)
            && scr != self.display_get_layer_top(disp)
            && scr != self.display_get_layer_sys(disp)
        {
            return false;
        }

        // Truncate the area to the object
        let ext_size = self.obj_get_ext_draw_size(obj);
        let mut obj_coords = self.pos_coords(obj);
        obj_coords.increase(ext_size, ext_size);

        // The area is not on the object
        if !area_intersect_in_place(area, &obj_coords) {
            return false;
        }

        if self.obj_pos_is_transformed(obj) {
            self.obj_get_transformed_area(obj, area, ObjPointTransformFlag::Recursive);
        }

        // Truncate recursively to the parents
        let mut parent = self.pos_parent(obj);
        while let Some(p) = parent {
            // If the parent is hidden then the child is hidden and won't be drawn
            if self.pos_has_flag(p, ObjFlags::HIDDEN) {
                return false;
            }

            // Truncate to the parent and if no common parts break
            let mut parent_coords = self.pos_coords(p);
            if self.pos_has_flag(p, ObjFlags::OVERFLOW_VISIBLE) {
                let parent_ext_size = self.obj_get_ext_draw_size(p);
                parent_coords.increase(parent_ext_size, parent_ext_size);
            }

            if self.obj_pos_is_transformed(p) {
                self.obj_get_transformed_area(p, &mut parent_coords, ObjPointTransformFlag::Recursive);
            }
            if !area_intersect_in_place(area, &parent_coords) {
                return false;
            }

            parent = self.pos_parent(p);
        }

        true
    }

    /// `lv_obj_is_visible()`
    pub fn obj_is_visible(&self, obj: Obj) -> bool {
        if !self.obj_is_valid(obj) {
            return false;
        }

        let ext_size = self.obj_get_ext_draw_size(obj);
        let mut obj_coords = self.pos_coords(obj);
        obj_coords.x1 -= ext_size;
        obj_coords.y1 -= ext_size;
        obj_coords.x2 += ext_size;
        obj_coords.y2 += ext_size;

        self.obj_area_is_visible(obj, &mut obj_coords)
    }

    /// `lv_obj_set_ext_click_area()`
    pub fn obj_set_ext_click_area(&mut self, obj: Obj, size: i32) {
        let Some(d) = self.obj_mut(obj) else { return };

        // lv_obj_allocate_spec_attr()
        let spec = d.spec.get_or_insert_with(|| Box::new(SpecAttr::new()));
        spec.ext_click_pad = size;
    }

    /// `lv_obj_get_click_area()`
    pub fn obj_get_click_area(&self, obj: Obj) -> Area {
        let Some(d) = self.obj(obj) else {
            return Area::default();
        };

        let mut area = d.coords;
        if let Some(spec) = d.spec.as_ref() {
            area.increase(spec.ext_click_pad, spec.ext_click_pad);
        }
        area
    }

    /// `lv_obj_hit_test()`
    pub fn obj_hit_test(&mut self, obj: Obj, point: Point) -> bool {
        if !self.obj_is_valid(obj) {
            return false;
        }

        if !self.pos_has_flag(obj, ObjFlags::CLICKABLE) {
            return false;
        }

        let a = self.obj_get_click_area(obj);
        let res = a.is_point_on(point, 0);
        if !res {
            return false;
        }

        if self.pos_has_flag(obj, ObjFlags::ADV_HITTEST) {
            let mut hit_info = HitTestInfo { point, res: true };
            self.obj_send_event(obj, EventCode::HIT_TEST, EventParam::HitTest(&mut hit_info));
            return hit_info.res;
        }

        res
    }
}

/* Static functions of lv_obj_pos.c */
impl Ui {
    /// `obj_invalidate_area_internal()`
    fn obj_invalidate_area_internal(&mut self, obj: Obj, area: &Area) -> LvResult {
        let mut area_tmp = *area;

        self.invalidate_area_core(obj, &mut area_tmp)
    }

    /// `calc_content_width()`
    fn calc_content_width(&mut self, obj: Obj) -> i32 {
        let scroll_x_tmp = self.obj_get_scroll_x(obj);
        if let Some(spec) = self.obj_mut(obj).and_then(|d| d.spec.as_mut()) {
            spec.scroll.x = 0;
        }

        let space_right = self.obj_get_style_space_right(obj, Part::MAIN);
        let space_left = self.obj_get_style_space_left(obj, Part::MAIN);

        let self_w = self.obj_get_self_width(obj) + space_left + space_right;

        let mut child_res = coord::MIN;

        if !self.layout_get_min_size(obj, &mut child_res, true) {
            let child_cnt = self.pos_child_count(obj);
            let obj_coords = self.pos_coords(obj);
            // With RTL find the left most coordinate
            if self.pos_is_rtl(obj) {
                for i in 0..child_cnt {
                    let mut child_res_tmp = coord::MIN;
                    let Some(child) = self.pos_child_at(obj, i) else {
                        break;
                    };
                    let Some(cd) = self.obj(child) else { continue };
                    if cd.flags.contains(ObjFlags::HIDDEN) || cd.flags.contains(ObjFlags::FLOATING) {
                        continue;
                    }

                    if cd.w_ignore_size {
                        continue;
                    }
                    let child_coords = cd.coords;

                    if !self.obj_is_layout_positioned(child) {
                        let align = align_from_num(self.pos_style_num(child, StyleProp::ALIGN));
                        match align {
                            Some(Align::Default | Align::TopRight | Align::BottomRight | Align::RightMid) => {
                                // Normal right aligns. Other are ignored due to
                                // possible circular dependencies
                                child_res_tmp = obj_coords.x2 - child_coords.x1 + 1;
                            }
                            _ => {
                                // Consider other cases only if x=0 and use the
                                // width of the object. With x!=0 circular
                                // dependency could occur.
                                if self.pos_style_num(child, StyleProp::X) == 0 {
                                    child_res_tmp = child_coords.width() + space_right;
                                    child_res_tmp += self.pos_style_num(child, StyleProp::MARGIN_LEFT);
                                }
                            }
                        }
                    } else {
                        child_res_tmp = obj_coords.x2 - child_coords.x1 + 1;
                    }
                    child_res =
                        child_res.max(child_res_tmp + self.pos_style_num(child, StyleProp::MARGIN_LEFT));
                }
                if child_res != coord::MIN {
                    child_res += space_left;
                }
            }
            // Else find the right most coordinate
            else {
                for i in 0..child_cnt {
                    let mut child_res_tmp = coord::MIN;
                    let Some(child) = self.pos_child_at(obj, i) else {
                        break;
                    };
                    let Some(cd) = self.obj(child) else { continue };
                    if cd.flags.contains(ObjFlags::HIDDEN) || cd.flags.contains(ObjFlags::FLOATING) {
                        continue;
                    }

                    if cd.w_ignore_size {
                        continue;
                    }
                    let child_coords = cd.coords;

                    if !self.obj_is_layout_positioned(child) {
                        let align = align_from_num(self.pos_style_num(child, StyleProp::ALIGN));
                        match align {
                            Some(Align::Default | Align::TopLeft | Align::BottomLeft | Align::LeftMid) => {
                                // Normal left aligns.
                                child_res_tmp = child_coords.x2 - obj_coords.x1 + 1;
                            }
                            _ => {
                                // Consider other cases only if x=0 and use the
                                // width of the object. With x!=0 circular
                                // dependency could occur.
                                if self.pos_style_num(child, StyleProp::X) == 0 {
                                    child_res_tmp = child_coords.width() + space_left;
                                    child_res_tmp += self.pos_style_num(child, StyleProp::MARGIN_RIGHT);
                                }
                            }
                        }
                    } else {
                        child_res_tmp = child_coords.x2 - obj_coords.x1 + 1;
                    }

                    child_res =
                        child_res.max(child_res_tmp + self.pos_style_num(child, StyleProp::MARGIN_RIGHT));
                }

                if child_res != coord::MIN {
                    child_res += space_right;
                }
            }
        }

        if let Some(spec) = self.obj_mut(obj).and_then(|d| d.spec.as_mut()) {
            spec.scroll.x = -scroll_x_tmp;
        }

        if child_res == coord::MIN {
            return self_w;
        }

        child_res.max(self_w)
    }

    /// `calc_content_height()`
    fn calc_content_height(&mut self, obj: Obj) -> i32 {
        let scroll_y_tmp = self.obj_get_scroll_y(obj);
        if let Some(spec) = self.obj_mut(obj).and_then(|d| d.spec.as_mut()) {
            spec.scroll.y = 0;
        }

        let space_top = self.obj_get_style_space_top(obj, Part::MAIN);
        let space_bottom = self.obj_get_style_space_bottom(obj, Part::MAIN);

        let self_h = self.obj_get_self_height(obj) + space_top + space_bottom;

        let mut child_res = coord::MIN;

        if !self.layout_get_min_size(obj, &mut child_res, false) {
            let child_cnt = self.pos_child_count(obj);
            let obj_coords = self.pos_coords(obj);
            for i in 0..child_cnt {
                let mut child_res_tmp = coord::MIN;
                let Some(child) = self.pos_child_at(obj, i) else {
                    break;
                };
                let Some(cd) = self.obj(child) else { continue };
                if cd.flags.contains(ObjFlags::HIDDEN) || cd.flags.contains(ObjFlags::FLOATING) {
                    continue;
                }

                if cd.h_ignore_size {
                    continue;
                }
                let child_coords = cd.coords;

                if !self.obj_is_layout_positioned(child) {
                    let align = align_from_num(self.pos_style_num(child, StyleProp::ALIGN));
                    match align {
                        Some(Align::Default | Align::TopRight | Align::TopMid | Align::TopLeft) => {
                            // Normal top aligns.
                            child_res_tmp = child_coords.y2 - obj_coords.y1 + 1;
                        }
                        _ => {
                            // Consider other cases only if y=0 and use the height
                            // of the object. With y!=0 circular dependency could
                            // occur.
                            if self.pos_style_num(child, StyleProp::Y) == 0 {
                                child_res_tmp = child_coords.height() + space_top;
                                child_res_tmp += self.pos_style_num(child, StyleProp::MARGIN_TOP);
                            }
                        }
                    }
                } else {
                    child_res_tmp = child_coords.y2 - obj_coords.y1 + 1;
                }

                child_res =
                    child_res.max(child_res_tmp + self.pos_style_num(child, StyleProp::MARGIN_BOTTOM));
            }

            if child_res != coord::MIN {
                child_res += space_bottom;
            }
        }

        if let Some(spec) = self.obj_mut(obj).and_then(|d| d.spec.as_mut()) {
            spec.scroll.y = -scroll_y_tmp;
        }

        if child_res == coord::MIN {
            return self_h;
        }

        self_h.max(child_res)
    }

    /// `layout_update_core()`
    fn layout_update_core(&mut self, obj: Obj) {
        let child_cnt = self.pos_child_count(obj);
        for i in 0..child_cnt {
            let Some(child) = self.pos_child_at(obj, i) else {
                break;
            };
            self.layout_update_core(child);
        }

        let Some(d) = self.obj_mut(obj) else { return };
        if d.layout_inv {
            d.layout_inv = false;
            self.obj_refr_size(obj);
            self.obj_refr_pos(obj);

            if child_cnt > 0 && self.obj_is_valid(obj) {
                self.layout_apply(obj);
            }
        }

        let Some(d) = self.obj_mut(obj) else { return };
        if d.readjust_scroll_after_layout {
            d.readjust_scroll_after_layout = false;
            self.obj_readjust_scroll(obj, false);
        }
    }

    /// `transform_point_array()` (the `LV_DRAW_TRANSFORM_USE_MATRIX == 0`
    /// branch, as in the parity configuration).
    fn transform_point_array(&self, obj: Obj, p: &mut [Point], inv: bool) {
        let mut angle = self.pos_style_num(obj, StyleProp::TRANSFORM_ROTATION);
        let mut scale_x = self.obj_get_style_transform_scale_x_safe(obj, Part::MAIN);
        let mut scale_y = self.obj_get_style_transform_scale_y_safe(obj, Part::MAIN);
        if scale_x == 0 {
            scale_x = 1;
        }
        if scale_y == 0 {
            scale_y = 1;
        }

        if angle == 0 && scale_x == SCALE_NONE && scale_y == SCALE_NONE {
            return;
        }

        let coords = self.pos_coords(obj);
        let mut pivot = Point::new(
            self.pos_style_num(obj, StyleProp::TRANSFORM_PIVOT_X),
            self.pos_style_num(obj, StyleProp::TRANSFORM_PIVOT_Y),
        );

        if coord::is_pct(pivot.x) {
            pivot.x = coord::get_pct(pivot.x).wrapping_mul(coords.width()) / 100;
        }
        if coord::is_pct(pivot.y) {
            pivot.y = coord::get_pct(pivot.y).wrapping_mul(coords.height()) / 100;
        }

        pivot.x += coords.x1;
        pivot.y += coords.y1;

        if inv {
            angle = -angle;
            scale_x = (256 * 256 + scale_x - 1) / scale_x;
            scale_y = (256 * 256 + scale_y - 1) / scale_y;
        }

        // lv_point_array_transform() with count == 0 is a no-op loop in C
        if p.is_empty() {
            return;
        }
        transform_points(p, angle, scale_x, scale_y, pivot, !inv);
    }

    /// `invalidate_area_core()` (the `LV_DRAW_TRANSFORM_USE_MATRIX == 0` branch).
    fn invalidate_area_core(&mut self, obj: Obj, area_tmp: &mut Area) -> LvResult {
        if !self.obj_area_is_visible(obj, area_tmp) {
            return LvResult::Invalid;
        }
        let is_transform_layer = self
            .obj(obj)
            .and_then(|d| d.spec.as_ref())
            .is_some_and(|s| s.layer_type == LayerType::Transform);
        if is_transform_layer {
            // Make the area slightly larger to avoid rounding errors.
            // 5 is an empirical value
            area_tmp.increase(5, 5);
        }

        let disp = self.obj_get_display(obj);
        self.inv_area(disp, Some(area_tmp))
    }

    /// `calc_dynamic_width()`: the width in pixels for `prop` (`WIDTH`,
    /// `MIN_WIDTH` or `MAX_WIDTH`). `content_width` caches the content width
    /// between calls (negative = not computed yet).
    fn calc_dynamic_width(&mut self, obj: Obj, prop: StyleProp, content_width: Option<&mut i32>) -> i32 {
        debug_assert!(
            prop == StyleProp::WIDTH || prop == StyleProp::MIN_WIDTH || prop == StyleProp::MAX_WIDTH
        );

        let mut width = self.obj_get_style_prop(obj, Part::MAIN, prop).num();

        if width == coord::SIZE_CONTENT {
            match content_width {
                None => width = self.calc_content_width(obj),
                Some(cw) => {
                    if *cw < 0 {
                        *cw = self.calc_content_width(obj);
                    }
                    width = *cw;
                }
            }
        } else if coord::is_pct(width) {
            let parent = self.pos_parent(obj);
            let parent_w = parent.map_or(0, |p| self.obj_get_content_width(p));
            width = coord::get_pct(width).wrapping_mul(parent_w) / 100;
            width -= self.pos_style_num(obj, StyleProp::MARGIN_LEFT)
                + self.pos_style_num(obj, StyleProp::MARGIN_RIGHT);
        }
        width
    }

    /// `calc_dynamic_height()`: the height in pixels for `prop` (`HEIGHT`,
    /// `MIN_HEIGHT` or `MAX_HEIGHT`). `content_height` caches the content
    /// height between calls (negative = not computed yet).
    fn calc_dynamic_height(&mut self, obj: Obj, prop: StyleProp, content_height: Option<&mut i32>) -> i32 {
        debug_assert!(
            prop == StyleProp::HEIGHT || prop == StyleProp::MIN_HEIGHT || prop == StyleProp::MAX_HEIGHT
        );

        let mut height = self.obj_get_style_prop(obj, Part::MAIN, prop).num();

        if height == coord::SIZE_CONTENT {
            match content_height {
                None => height = self.calc_content_height(obj),
                Some(ch) => {
                    if *ch < 0 {
                        *ch = self.calc_content_height(obj);
                    }
                    height = *ch;
                }
            }
        } else if coord::is_pct(height) {
            let parent = self.pos_parent(obj);
            let parent_h = parent.map_or(0, |p| self.obj_get_content_height(p));
            height = coord::get_pct(height).wrapping_mul(parent_h) / 100;
            height -= self.pos_style_num(obj, StyleProp::MARGIN_TOP)
                + self.pos_style_num(obj, StyleProp::MARGIN_BOTTOM);
        }
        height
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::obj::{OBJ_CLASS, ObjData, ObjId};
    use crate::core::ui::UiConfig;
    use crate::misc::style::State;
    use alloc::vec::Vec;

    fn new_obj(ui: &mut Ui, parent: Option<Obj>, coords: Area) -> Obj {
        let data = ObjData {
            class: &OBJ_CLASS,
            parent: parent.map(|p| p.0),
            spec: None,
            styles: Vec::new(),
            style_main_prop_is_set: 0,
            style_other_prop_is_set: 0,
            user_data: None,
            coords,
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
        let o = Obj(ObjId(ui.objs.insert(data)));
        if let Some(p) = parent {
            ui.obj_mut(p)
                .unwrap()
                .spec
                .get_or_insert_with(|| Box::new(SpecAttr::new()))
                .children
                .push(o.0);
        }
        o
    }

    fn ui() -> Ui {
        Ui::new(UiConfig::default())
    }

    #[test]
    fn clamp_width_height_resolve_pct_bounds() {
        // Plain px bounds.
        assert_eq!(clamp_width(50, 10, 100, 0), 50);
        assert_eq!(clamp_width(5, 10, 100, 0), 10);
        assert_eq!(clamp_width(500, 10, 100, 0), 100);
        // Percent bounds resolve against the reference width.
        assert_eq!(clamp_width(500, coord::pct(10), coord::pct(50), 200), 100);
        assert_eq!(clamp_width(5, coord::pct(10), coord::pct(50), 200), 20);
        assert_eq!(clamp_height(150, 0, coord::pct(25), 400), 100);
        assert_eq!(clamp_height(-3, coord::pct(1), 1000, 400), 4);
        // LV_CLAMP with inverted bounds returns the min (no panic).
        assert_eq!(clamp_width(50, 80, 20, 0), 80);
        assert_eq!(clamp_height(50, 80, 20, 0), 80);
    }

    #[test]
    fn size_in_effect_is_pct_follows_the_clamping_bound() {
        let p = coord::pct(50);
        // Inverted bounds: the min style decides.
        assert!(size_in_effect_is_pct(10, 30, 20, 0, p, 0));
        assert!(!size_in_effect_is_pct(10, 30, 20, p, 0, p));
        // Clamped by min / max.
        assert!(size_in_effect_is_pct(5, 10, 100, 0, p, 0));
        assert!(!size_in_effect_is_pct(5, 10, 100, p, 0, p));
        assert!(size_in_effect_is_pct(500, 10, 100, 0, 0, p));
        // Set by the size style.
        assert!(size_in_effect_is_pct(50, 10, 100, p, 0, 0));
        assert!(!size_in_effect_is_pct(50, 10, 100, 0, p, p));
        // Equal to a bound: the non-percentage style wins.
        assert!(!size_in_effect_is_pct(10, 10, 100, p, 0, p));
        assert!(!size_in_effect_is_pct(100, 10, 100, p, p, 0));
        assert!(size_in_effect_is_pct(100, 10, 100, p, p, p));
    }

    #[test]
    fn transform_flag_bits() {
        assert_eq!(ObjPointTransformFlag::None as u8, 0);
        assert_eq!(ObjPointTransformFlag::Recursive as u8, 1);
        assert_eq!(ObjPointTransformFlag::Inverse as u8, 2);
        assert_eq!(ObjPointTransformFlag::InverseRecursive as u8, 3);
        assert!(ObjPointTransformFlag::InverseRecursive.recursive());
        assert!(ObjPointTransformFlag::InverseRecursive.inverse());
        assert!(!ObjPointTransformFlag::Recursive.inverse());
        assert!(!ObjPointTransformFlag::Inverse.recursive());
        assert_eq!(OBJ_LAYOUT_UPDATE_MAX_PASSES, 100);
    }

    #[test]
    fn align_decoding_matches_lv_align_t() {
        assert_eq!(align_from_num(0), Some(Align::Default));
        assert_eq!(align_from_num(9), Some(Align::Center));
        assert_eq!(align_from_num(21), Some(Align::OutRightBottom));
        assert_eq!(align_from_num(22), None);
        assert_eq!(align_from_num(-1), None);
        for v in 0..=21 {
            assert_eq!(align_from_num(v).map(|a| a as i32), Some(v));
        }
    }

    #[test]
    fn coords_size_and_screen_position() {
        let mut ui = ui();
        let scr = new_obj(&mut ui, None, Area::new(3, 4, 102, 53));
        assert_eq!(ui.obj_get_coords(scr), Area::new(3, 4, 102, 53));
        assert_eq!(ui.obj_get_width(scr), 100);
        assert_eq!(ui.obj_get_height(scr), 50);
        // A screen has no parent: x/y are the absolute coordinates.
        assert_eq!(ui.obj_get_x(scr), 3);
        assert_eq!(ui.obj_get_y(scr), 4);
        assert_eq!(ui.obj_get_x2(scr), 103);
        assert_eq!(ui.obj_get_y2(scr), 54);
    }

    #[test]
    fn stale_handle_returns_neutral_values() {
        let mut ui = ui();
        let o = new_obj(&mut ui, None, Area::new(0, 0, 9, 9));
        ui.objs.remove(o.0.0);
        assert_eq!(ui.obj_get_width(o), 0);
        assert_eq!(ui.obj_get_height(o), 0);
        assert_eq!(ui.obj_get_x(o), 0);
        assert_eq!(ui.obj_get_y2(o), 0);
        assert_eq!(ui.obj_get_coords(o), Area::default());
        assert_eq!(ui.obj_get_click_area(o), Area::default());
        assert!(!ui.obj_hit_test(o, Point::new(1, 1)));
        assert!(!ui.obj_refr_size(o));
        assert!(!ui.obj_is_layout_positioned(o));
        assert_eq!(ui.obj_invalidate(o), LvResult::Invalid);
        ui.obj_set_ext_click_area(o, 5);
        ui.obj_move_children_by(o, 1, 1, false);
        ui.obj_mark_layout_as_dirty(o);
    }

    #[test]
    fn ext_click_area_grows_click_area() {
        let mut ui = ui();
        let o = new_obj(&mut ui, None, Area::new(10, 10, 19, 19));
        assert_eq!(ui.obj_get_click_area(o), Area::new(10, 10, 19, 19));
        assert!(ui.obj(o).unwrap().spec.is_none());
        ui.obj_set_ext_click_area(o, 4);
        // lv_obj_allocate_spec_attr() defaults are kept.
        let spec = ui.obj(o).unwrap().spec.as_ref().unwrap();
        assert_eq!(spec.ext_click_pad, 4);
        assert_eq!(spec.layer_type, LayerType::None);
        assert_eq!(ui.obj_get_click_area(o), Area::new(6, 6, 23, 23));
    }

    #[test]
    fn hit_test_uses_clickable_flag_and_click_area() {
        let mut ui = ui();
        let o = new_obj(&mut ui, None, Area::new(10, 10, 19, 19));
        // Not clickable.
        assert!(!ui.obj_hit_test(o, Point::new(12, 12)));
        ui.obj_mut(o).unwrap().flags.insert(ObjFlags::CLICKABLE);
        assert!(ui.obj_hit_test(o, Point::new(12, 12)));
        assert!(ui.obj_hit_test(o, Point::new(19, 19)));
        assert!(!ui.obj_hit_test(o, Point::new(20, 12)));
        ui.obj_set_ext_click_area(o, 2);
        assert!(ui.obj_hit_test(o, Point::new(21, 12)));
        assert!(!ui.obj_hit_test(o, Point::new(22, 12)));
    }

    #[test]
    fn move_children_by_is_recursive_and_skips_floating() {
        let mut ui = ui();
        let scr = new_obj(&mut ui, None, Area::new(0, 0, 99, 99));
        let a = new_obj(&mut ui, Some(scr), Area::new(10, 10, 19, 19));
        let a1 = new_obj(&mut ui, Some(a), Area::new(12, 12, 13, 13));
        let f = new_obj(&mut ui, Some(scr), Area::new(50, 50, 59, 59));
        let f1 = new_obj(&mut ui, Some(f), Area::new(51, 51, 52, 52));
        ui.obj_mut(f).unwrap().flags.insert(ObjFlags::FLOATING);

        ui.obj_move_children_by(scr, 5, -3, true);
        // The parent itself does not move.
        assert_eq!(ui.obj_get_coords(scr), Area::new(0, 0, 99, 99));
        assert_eq!(ui.obj_get_coords(a), Area::new(15, 7, 24, 16));
        assert_eq!(ui.obj_get_coords(a1), Area::new(17, 9, 18, 10));
        // Floating children (and their subtree) are skipped.
        assert_eq!(ui.obj_get_coords(f), Area::new(50, 50, 59, 59));
        assert_eq!(ui.obj_get_coords(f1), Area::new(51, 51, 52, 52));

        ui.obj_move_children_by(scr, 1, 1, false);
        assert_eq!(ui.obj_get_coords(f), Area::new(51, 51, 60, 60));
        assert_eq!(ui.obj_get_coords(f1), Area::new(52, 52, 53, 53));
        // `ignore_floating` only applies to the direct children: grandchildren
        // marked floating still move with their parent.
        ui.obj_mut(a1).unwrap().flags.insert(ObjFlags::FLOATING);
        ui.obj_move_children_by(scr, 1, 0, true);
        assert_eq!(ui.obj_get_coords(a1), Area::new(19, 10, 20, 11));
    }

    #[test]
    fn is_transformed_walks_parents() {
        let mut ui = ui();
        let scr = new_obj(&mut ui, None, Area::new(0, 0, 99, 99));
        let a = new_obj(&mut ui, Some(scr), Area::new(10, 10, 19, 19));
        let b = new_obj(&mut ui, Some(a), Area::new(12, 12, 13, 13));
        assert!(!ui.obj_pos_is_transformed(b));
        ui.obj_mut(a).unwrap().spec.as_mut().unwrap().layer_type = LayerType::Simple;
        assert!(!ui.obj_pos_is_transformed(b));
        ui.obj_mut(a).unwrap().spec.as_mut().unwrap().layer_type = LayerType::Transform;
        assert!(ui.obj_pos_is_transformed(b));
        assert!(ui.obj_pos_is_transformed(a));
        assert!(!ui.obj_pos_is_transformed(scr));
    }

    #[test]
    fn untransformed_points_are_unchanged() {
        let mut ui = ui();
        let scr = new_obj(&mut ui, None, Area::new(0, 0, 99, 99));
        let a = new_obj(&mut ui, Some(scr), Area::new(10, 10, 19, 19));
        let mut pts = [Point::new(1, 2), Point::new(-5, 70)];
        for flags in [
            ObjPointTransformFlag::None,
            ObjPointTransformFlag::Recursive,
            ObjPointTransformFlag::Inverse,
            ObjPointTransformFlag::InverseRecursive,
        ] {
            ui.obj_transform_point_array(a, &mut pts, flags);
            assert_eq!(pts, [Point::new(1, 2), Point::new(-5, 70)]);
        }
        let mut p = Point::new(7, 8);
        ui.obj_transform_point(a, &mut p, ObjPointTransformFlag::Recursive);
        assert_eq!(p, Point::new(7, 8));
        ui.obj_transform_point_array(a, &mut [], ObjPointTransformFlag::Recursive);
    }

    #[test]
    fn transformed_area_uses_exclusive_corners() {
        // lv_obj_get_transformed_area() transforms the exclusive corners
        // (x2 + 1, y2 + 1) and stores their max, so without a transform the
        // area grows by one pixel on the right and bottom, like upstream.
        let mut ui = ui();
        let a = new_obj(&mut ui, None, Area::new(10, 10, 19, 19));
        let mut area = Area::new(1, 2, 30, 40);
        ui.obj_get_transformed_area(a, &mut area, ObjPointTransformFlag::Recursive);
        assert_eq!(area, Area::new(1, 2, 31, 41));
    }

    #[test]
    fn area_intersect_in_place_writes_even_when_empty() {
        let mut a = Area::new(0, 0, 10, 10);
        assert!(area_intersect_in_place(&mut a, &Area::new(5, 5, 20, 20)));
        assert_eq!(a, Area::new(5, 5, 10, 10));
        let mut b = Area::new(0, 0, 10, 10);
        assert!(!area_intersect_in_place(&mut b, &Area::new(20, 20, 30, 30)));
        assert_eq!(b, Area::new(20, 20, 10, 10));
    }

    #[test]
    fn update_layout_is_reentrancy_guarded() {
        let mut ui = ui();
        let scr = new_obj(&mut ui, None, Area::new(0, 0, 99, 99));
        ui.obj_mut(scr).unwrap().scr_layout_inv = true;
        ui.layout_update_mutex = true;
        // Already running: returns immediately and leaves the flags alone.
        ui.obj_update_layout(scr);
        assert!(ui.layout_update_mutex);
        assert!(ui.obj(scr).unwrap().scr_layout_inv);
    }

    #[test]
    fn lv_clamp_matches_c_macro() {
        assert_eq!(lv_clamp(0, 5, 10), 5);
        assert_eq!(lv_clamp(0, -5, 10), 0);
        assert_eq!(lv_clamp(0, 15, 10), 10);
        assert_eq!(lv_clamp(10, 5, 0), 10);
    }
}
