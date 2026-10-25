//! Ported from lvgl v9.6.0: src/core/lv_obj_scroll.c
//!
//! Also covers `include/lvgl/core/lv_obj_scroll.h` and
//! `src/core/lv_obj_scroll_private.h`.
//!
//! Scroll position, scroll bounds, animated scrolling, scroll-into-view,
//! snapping and the scrollbar geometry of objects.
//!
//! Notes on the Rust port:
//! - `LV_CHECK_OBJ(obj, &lv_obj_class, ...)` becomes a validity check of the
//!   handle (every object is an `lv_obj_class` instance, so the class check
//!   is always true).
//! - `scroll_x_anim` / `scroll_y_anim` are [`AnimExec::ObjScrollX`] /
//!   [`AnimExec::ObjScrollY`]; `misc/anim.rs` executes them exactly like the
//!   C static callbacks (`lv_obj_scroll_by_raw(obj, v + scroll_x, 0)`).
//! - `LV_EVENT_SCROLL_BEGIN` of an animated scroll passes the not yet started
//!   `lv_anim_t` template (a stack variable in C) so handlers can modify it.
//!   Here the template is staged in [`crate::misc::anim::AnimState::anims`]
//!   without being linked into the animation list for the duration of the
//!   event ([`EventParam::Anim`]); afterwards the (possibly modified)
//!   template is taken back and started, like C starts `&a`.
//! - `lv_obj_get_scroll_bottom/left/right()` and `lv_obj_get_scrollbar_area()`
//!   take `&mut self` because `lv_obj_get_self_width/height()` send
//!   `LV_EVENT_GET_SELF_SIZE`.
//! - A callback may delete the measured/scrolled object. Validity checks stop
//!   the current operation before its next access; scrollbar queries retain
//!   their empty output areas. This Rust safety adaptation preserves C's
//!   getter/event order and arithmetic whenever the objects remain alive.

use alloc::boxed::Box;
use alloc::rc::Rc;

use crate::core::Ui;
use crate::core::obj::{Obj, ScrollSnap, ScrollbarMode, SpecAttr};
use crate::misc::anim::{Anim, AnimCallback, AnimEnable, AnimExec, AnimId, AnimPath, AnimTarget};
use crate::misc::area::{Area, Dir, Point};
use crate::misc::color::OPA_MIN;
use crate::misc::event::{EventCode, EventParam, LvResult};
use crate::misc::style::{Part, StyleProp};
use crate::misc::text::BaseDir;
use crate::misc::types::coord::{MAX as COORD_MAX, MIN as COORD_MIN};

/// `SCROLL_ANIM_TIME_MIN` (ms)
pub const SCROLL_ANIM_TIME_MIN: u32 = 200;
/// `SCROLL_ANIM_TIME_MAX` (ms)
pub const SCROLL_ANIM_TIME_MAX: u32 = 400;
/// `SCROLLBAR_MIN_SIZE` is `LV_DPX(10)`: the `n` of that `LV_DPX`.
const SCROLLBAR_MIN_SIZE_DPX: i32 = 10;

impl Ui {
    /*=====================
     * Setter functions
     *====================*/

    /// `lv_obj_set_scrollbar_mode()`
    pub fn obj_set_scrollbar_mode(&mut self, obj: Obj, mode: ScrollbarMode) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        let Some(spec) = self.scroll_allocate_spec_attr(obj) else {
            return;
        };

        if spec.scrollbar_mode == mode {
            return;
        }
        spec.scrollbar_mode = mode;
        let _ = self.obj_invalidate(obj);
    }

    /// `lv_obj_set_scroll_dir()`
    pub fn obj_set_scroll_dir(&mut self, obj: Obj, dir: Dir) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        let Some(spec) = self.scroll_allocate_spec_attr(obj) else {
            return;
        };

        if dir != spec.scroll_dir {
            spec.scroll_dir = dir;
        }
    }

    /// `lv_obj_set_scroll_snap_x()`
    pub fn obj_set_scroll_snap_x(&mut self, obj: Obj, align: ScrollSnap) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        let Some(spec) = self.scroll_allocate_spec_attr(obj) else {
            return;
        };
        spec.scroll_snap_x = align;
    }

    /// `lv_obj_set_scroll_snap_y()`
    pub fn obj_set_scroll_snap_y(&mut self, obj: Obj, align: ScrollSnap) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        let Some(spec) = self.scroll_allocate_spec_attr(obj) else {
            return;
        };
        spec.scroll_snap_y = align;
    }

    /*=====================
     * Getter functions
     *====================*/

    /// `lv_obj_get_scrollbar_mode()`
    pub fn obj_get_scrollbar_mode(&self, obj: Obj) -> ScrollbarMode {
        if !self.scroll_check_obj(obj) {
            return ScrollbarMode::Off;
        }

        match self.scroll_spec(obj) {
            Some(spec) => spec.scrollbar_mode,
            None => ScrollbarMode::Auto,
        }
    }

    /// `lv_obj_get_scroll_dir()`
    pub fn obj_get_scroll_dir(&self, obj: Obj) -> Dir {
        if !self.scroll_check_obj(obj) {
            return Dir::NONE;
        }

        match self.scroll_spec(obj) {
            Some(spec) => spec.scroll_dir,
            None => Dir::ALL,
        }
    }

    /// `lv_obj_get_scroll_snap_x()`
    pub fn obj_get_scroll_snap_x(&self, obj: Obj) -> ScrollSnap {
        if !self.scroll_check_obj(obj) {
            return ScrollSnap::None;
        }

        match self.scroll_spec(obj) {
            Some(spec) => spec.scroll_snap_x,
            None => ScrollSnap::None,
        }
    }

    /// `lv_obj_get_scroll_snap_y()`
    pub fn obj_get_scroll_snap_y(&self, obj: Obj) -> ScrollSnap {
        if !self.scroll_check_obj(obj) {
            return ScrollSnap::None;
        }

        match self.scroll_spec(obj) {
            Some(spec) => spec.scroll_snap_y,
            None => ScrollSnap::None,
        }
    }

    /// `lv_obj_get_scroll_x()`
    pub fn obj_get_scroll_x(&self, obj: Obj) -> i32 {
        if !self.scroll_check_obj(obj) {
            return 0;
        }

        match self.scroll_spec(obj) {
            None => 0,
            Some(spec) => spec.scroll.x.wrapping_neg(),
        }
    }

    /// `lv_obj_get_scroll_y()`
    pub fn obj_get_scroll_y(&self, obj: Obj) -> i32 {
        if !self.scroll_check_obj(obj) {
            return 0;
        }

        match self.scroll_spec(obj) {
            None => 0,
            Some(spec) => spec.scroll.y.wrapping_neg(),
        }
    }

    /// `lv_obj_get_scroll_top()`
    pub fn obj_get_scroll_top(&self, obj: Obj) -> i32 {
        if !self.scroll_check_obj(obj) {
            return 0;
        }

        match self.scroll_spec(obj) {
            None => 0,
            Some(spec) => spec.scroll.y.wrapping_neg(),
        }
    }

    /// `lv_obj_get_scroll_bottom()`
    ///
    /// Takes `&mut self` because `lv_obj_get_self_height()` sends
    /// `LV_EVENT_GET_SELF_SIZE`.
    pub fn obj_get_scroll_bottom(&mut self, obj: Obj) -> i32 {
        if !self.scroll_check_obj(obj) {
            return 0;
        }

        let mut child_res = COORD_MIN;
        let child_cnt = self.scroll_child_count(obj);
        for i in 0..child_cnt {
            let Some(child) = self.scroll_child(obj, i) else {
                continue;
            };
            if self.obj_is_hidden(child) || self.obj_is_floating(child) {
                continue;
            }

            let y2 = self.scroll_coords(child).y2;
            let tmp_y = y2.wrapping_add(self.scroll_style_num(child, Part::MAIN, StyleProp::MARGIN_BOTTOM));
            child_res = child_res.max(tmp_y);
        }

        let space_top = self.obj_get_style_space_top(obj, Part::MAIN);
        let space_bottom = self.obj_get_style_space_bottom(obj, Part::MAIN);

        if child_res != COORD_MIN {
            let y2 = self.scroll_coords(obj).y2;
            child_res = child_res.wrapping_sub(y2.wrapping_sub(space_bottom));
        }

        let mut self_h = self.obj_get_self_height(obj);
        // GET_SELF_SIZE may delete this object (or an ancestor).
        if !self.obj_is_valid(obj) {
            return 0;
        }
        self_h = self_h.wrapping_sub(
            self.obj_get_height(obj)
                .wrapping_sub(space_top)
                .wrapping_sub(space_bottom),
        );
        self_h = self_h.wrapping_sub(self.obj_get_scroll_y(obj));
        child_res.max(self_h)
    }

    /// `lv_obj_get_scroll_left()`
    ///
    /// Takes `&mut self` because `lv_obj_get_self_width()` sends
    /// `LV_EVENT_GET_SELF_SIZE`.
    pub fn obj_get_scroll_left(&mut self, obj: Obj) -> i32 {
        if !self.scroll_check_obj(obj) {
            return 0;
        }

        // Normally can't scroll the object out on the left.
        // So simply use the current scroll position as "left size"
        if !self.scroll_is_rtl(obj, Part::MAIN) {
            return match self.scroll_spec(obj) {
                None => 0,
                Some(spec) => spec.scroll.x.wrapping_neg(),
            };
        }

        // With RTL base direction scrolling the left is normal so find the left most coordinate
        let space_right = self.obj_get_style_space_right(obj, Part::MAIN);
        let space_left = self.obj_get_style_space_left(obj, Part::MAIN);

        let mut child_res: i32;

        let mut x1 = COORD_MAX;
        let child_cnt = self.scroll_child_count(obj);
        for i in 0..child_cnt {
            let Some(child) = self.scroll_child(obj, i) else {
                continue;
            };
            if self.obj_is_hidden(child) || self.obj_is_floating(child) {
                continue;
            }

            let cx1 = self.scroll_coords(child).x1;
            let tmp_x = cx1.wrapping_sub(self.scroll_style_num(child, Part::MAIN, StyleProp::MARGIN_LEFT));
            x1 = x1.min(tmp_x);
        }

        if x1 != COORD_MAX {
            child_res = x1;
            let ox1 = self.scroll_coords(obj).x1;
            child_res = ox1.wrapping_add(space_left).wrapping_sub(child_res);
        } else {
            child_res = COORD_MIN;
        }

        let mut self_w = self.obj_get_self_width(obj);
        if !self.obj_is_valid(obj) {
            return 0;
        }
        self_w = self_w.wrapping_sub(
            self.obj_get_width(obj)
                .wrapping_sub(space_right)
                .wrapping_sub(space_left),
        );
        self_w = self_w.wrapping_add(self.obj_get_scroll_x(obj));

        child_res.max(self_w)
    }

    /// `lv_obj_get_scroll_right()`
    ///
    /// Takes `&mut self` because `lv_obj_get_self_width()` sends
    /// `LV_EVENT_GET_SELF_SIZE`.
    pub fn obj_get_scroll_right(&mut self, obj: Obj) -> i32 {
        if !self.scroll_check_obj(obj) {
            return 0;
        }

        // With RTL base dir can't scroll to the object out on the right.
        // So simply use the current scroll position as "right size"
        if self.scroll_is_rtl(obj, Part::MAIN) {
            return match self.scroll_spec(obj) {
                None => 0,
                Some(spec) => spec.scroll.x,
            };
        }

        // With other base direction (LTR) scrolling to the right is normal so find the right most coordinate
        let mut child_res = COORD_MIN;
        let child_cnt = self.scroll_child_count(obj);
        for i in 0..child_cnt {
            let Some(child) = self.scroll_child(obj, i) else {
                continue;
            };
            if self.obj_is_hidden(child) || self.obj_is_floating(child) {
                continue;
            }

            let cx2 = self.scroll_coords(child).x2;
            let tmp_x = cx2.wrapping_add(self.scroll_style_num(child, Part::MAIN, StyleProp::MARGIN_RIGHT));
            child_res = child_res.max(tmp_x);
        }

        let space_right = self.obj_get_style_space_right(obj, Part::MAIN);
        let space_left = self.obj_get_style_space_left(obj, Part::MAIN);

        if child_res != COORD_MIN {
            let x2 = self.scroll_coords(obj).x2;
            child_res = child_res.wrapping_sub(x2.wrapping_sub(space_right));
        }

        let mut self_w = self.obj_get_self_width(obj);
        if !self.obj_is_valid(obj) {
            return 0;
        }
        self_w = self_w.wrapping_sub(
            self.obj_get_width(obj)
                .wrapping_sub(space_right)
                .wrapping_sub(space_left),
        );
        self_w = self_w.wrapping_sub(self.obj_get_scroll_x(obj));
        child_res.max(self_w)
    }

    /// `lv_obj_get_scroll_end()`: the scroll position at the end of the
    /// running scroll animations (or the current one).
    pub fn obj_get_scroll_end(&self, obj: Obj) -> Point {
        if !self.scroll_check_obj(obj) {
            return Point::new(0, 0);
        }

        let mut end = Point::new(0, 0);
        let a = self.anim_get(AnimTarget::Obj(obj), Some(&AnimExec::ObjScrollX));
        end.x = match a.and_then(|a| self.anim(a)) {
            Some(a) => a.end_value.wrapping_neg(),
            None => self.obj_get_scroll_x(obj),
        };

        let a = self.anim_get(AnimTarget::Obj(obj), Some(&AnimExec::ObjScrollY));
        end.y = match a.and_then(|a| self.anim(a)) {
            Some(a) => a.end_value.wrapping_neg(),
            None => self.obj_get_scroll_y(obj),
        };
        end
    }

    /*=====================
     * Other functions
     *====================*/

    /// `lv_obj_scroll_by_bounded()`
    pub fn obj_scroll_by_bounded(&mut self, obj: Obj, dx: i32, dy: i32, anim_en: AnimEnable) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        if dx == 0 && dy == 0 {
            return;
        }

        // We need to know the final sizes for bound check
        self.obj_update_layout(obj);
        if !self.obj_is_valid(obj) {
            return;
        }

        // Don't let scroll more than naturally possible by the size of the content
        let x_current = self.obj_get_scroll_x(obj).wrapping_neg();
        let mut x_bounded = x_current.wrapping_add(dx);

        if !self.scroll_is_rtl(obj, Part::MAIN) {
            if x_bounded > 0 {
                x_bounded = 0;
            }
            if x_bounded < 0 {
                let left = self.obj_get_scroll_left(obj);
                if !self.obj_is_valid(obj) {
                    return;
                }
                let mut scroll_max = left.wrapping_add(self.obj_get_scroll_right(obj));
                if !self.obj_is_valid(obj) {
                    return;
                }
                if scroll_max < 0 {
                    scroll_max = 0;
                }

                if x_bounded < scroll_max.wrapping_neg() {
                    x_bounded = scroll_max.wrapping_neg();
                }
            }
        } else {
            if x_bounded < 0 {
                x_bounded = 0;
            }
            if x_bounded > 0 {
                let left = self.obj_get_scroll_left(obj);
                if !self.obj_is_valid(obj) {
                    return;
                }
                let mut scroll_max = left.wrapping_add(self.obj_get_scroll_right(obj));
                if !self.obj_is_valid(obj) {
                    return;
                }
                if scroll_max < 0 {
                    scroll_max = 0;
                }

                if x_bounded > scroll_max {
                    x_bounded = scroll_max;
                }
            }
        }

        // Don't let scroll more than naturally possible by the size of the content
        let y_current = self.obj_get_scroll_y(obj).wrapping_neg();
        let mut y_bounded = y_current.wrapping_add(dy);

        if y_bounded > 0 {
            y_bounded = 0;
        }
        if y_bounded < 0 {
            let mut scroll_max = self
                .obj_get_scroll_top(obj)
                .wrapping_add(self.obj_get_scroll_bottom(obj));
            if !self.obj_is_valid(obj) {
                return;
            }
            if scroll_max < 0 {
                scroll_max = 0;
            }
            if y_bounded < scroll_max.wrapping_neg() {
                y_bounded = scroll_max.wrapping_neg();
            }
        }

        let dx = x_bounded.wrapping_sub(x_current);
        let dy = y_bounded.wrapping_sub(y_current);
        if dx != 0 || dy != 0 {
            self.obj_scroll_by(obj, dx, dy, anim_en);
        }
    }

    /// `lv_obj_scroll_by()`
    pub fn obj_scroll_by(&mut self, obj: Obj, dx: i32, dy: i32, anim_en: AnimEnable) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        if dx == 0 && dy == 0 {
            return;
        }
        if anim_en {
            let d = self.obj_get_display(obj);
            let mut a = Anim::new();
            a.set_var(AnimTarget::Obj(obj));
            a.set_deleted_cb(Some(scroll_end_cb()));

            if dx != 0 {
                let t = Ui::anim_speed_clamped(
                    (self.display_get_horizontal_resolution(d) >> 1) as u32,
                    SCROLL_ANIM_TIME_MIN,
                    SCROLL_ANIM_TIME_MAX,
                );
                a.set_duration(t);
                let sx = self.obj_get_scroll_x(obj);
                a.set_values(sx.wrapping_neg(), sx.wrapping_neg().wrapping_add(dx));
                a.set_exec_cb(AnimExec::ObjScrollX);
                a.set_path_cb(AnimPath::EaseOut);

                let res = self.scroll_send_begin_with_anim(obj, &mut a);
                if res != LvResult::Ok {
                    return;
                }
                self.anim_start(&a);
                if !self.obj_is_valid(obj) {
                    return;
                }
            }

            if dy != 0 {
                let t = Ui::anim_speed_clamped(
                    (self.display_get_vertical_resolution(d) >> 1) as u32,
                    SCROLL_ANIM_TIME_MIN,
                    SCROLL_ANIM_TIME_MAX,
                );
                a.set_duration(t);
                let sy = self.obj_get_scroll_y(obj);
                a.set_values(sy.wrapping_neg(), sy.wrapping_neg().wrapping_add(dy));
                a.set_exec_cb(AnimExec::ObjScrollY);
                a.set_path_cb(AnimPath::EaseOut);

                let res = self.scroll_send_begin_with_anim(obj, &mut a);
                if res != LvResult::Ok {
                    return;
                }
                self.anim_start(&a);
            }
        } else {
            // Remove pending animations
            self.anim_delete(AnimTarget::Obj(obj), Some(&AnimExec::ObjScrollY));
            if !self.obj_is_valid(obj) {
                return;
            }
            self.anim_delete(AnimTarget::Obj(obj), Some(&AnimExec::ObjScrollX));
            if !self.obj_is_valid(obj) {
                return;
            }

            let res = self.obj_send_event(obj, EventCode::SCROLL_BEGIN, EventParam::None);
            if res != LvResult::Ok {
                return;
            }

            let res = self.obj_scroll_by_raw(obj, dx, dy);
            if res != LvResult::Ok {
                return;
            }

            // The result of the last event is not needed (upstream returns either way).
            let _ = self.obj_send_event(obj, EventCode::SCROLL_END, EventParam::None);
        }
    }

    /// `lv_obj_scroll_to()`
    pub fn obj_scroll_to(&mut self, obj: Obj, x: i32, y: i32, anim_en: AnimEnable) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        self.obj_scroll_to_x(obj, x, anim_en);
        if self.obj_is_valid(obj) {
            self.obj_scroll_to_y(obj, y, anim_en);
        }
    }

    /// `lv_obj_scroll_to_x()`
    pub fn obj_scroll_to_x(&mut self, obj: Obj, x: i32, anim_en: AnimEnable) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        self.anim_delete(AnimTarget::Obj(obj), Some(&AnimExec::ObjScrollX));
        if !self.obj_is_valid(obj) {
            return;
        }

        let scroll_x = self.obj_get_scroll_x(obj);
        let diff = x.wrapping_neg().wrapping_add(scroll_x);

        self.obj_scroll_by_bounded(obj, diff, 0, anim_en);
    }

    /// `lv_obj_scroll_to_y()`
    pub fn obj_scroll_to_y(&mut self, obj: Obj, y: i32, anim_en: AnimEnable) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        self.anim_delete(AnimTarget::Obj(obj), Some(&AnimExec::ObjScrollY));
        if !self.obj_is_valid(obj) {
            return;
        }

        let scroll_y = self.obj_get_scroll_y(obj);
        let diff = y.wrapping_neg().wrapping_add(scroll_y);

        self.obj_scroll_by_bounded(obj, 0, diff, anim_en);
    }

    /// `lv_obj_scroll_to_view()`: scroll the parent so that `obj` becomes
    /// visible.
    pub fn obj_scroll_to_view(&mut self, obj: Obj, anim_en: AnimEnable) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        // Be sure the screens layout is correct
        self.obj_update_layout(obj);
        if !self.obj_is_valid(obj) {
            return;
        }

        let mut p = Point::new(0, 0);
        self.scroll_area_into_view(obj, obj, &mut p, anim_en);
    }

    /// `lv_obj_scroll_to_view_recursive()`: scroll every ancestor so that
    /// `obj` becomes visible.
    pub fn obj_scroll_to_view_recursive(&mut self, obj: Obj, anim_en: AnimEnable) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        // Be sure the screens layout is correct
        self.obj_update_layout(obj);
        if !self.obj_is_valid(obj) {
            return;
        }

        let mut p = Point::new(0, 0);
        let mut child = obj;
        let mut parent = self.obj_get_parent(child);
        while let Some(par) = parent {
            self.scroll_area_into_view(obj, child, &mut p, anim_en);
            if !self.obj_is_valid(obj) || !self.obj_is_valid(par) {
                return;
            }
            child = par;
            parent = self.obj_get_parent(par);
        }
    }

    /// `lv_obj_scroll_by_raw()`: scroll without bound checks or animation.
    pub fn obj_scroll_by_raw(&mut self, obj: Obj, x: i32, y: i32) -> LvResult {
        if !self.scroll_check_obj(obj) {
            return LvResult::Invalid;
        }

        if x == 0 && y == 0 {
            return LvResult::Ok;
        }

        let Some(spec) = self.scroll_allocate_spec_attr(obj) else {
            return LvResult::Invalid;
        };

        spec.scroll.x = spec.scroll.x.wrapping_add(x);
        spec.scroll.y = spec.scroll.y.wrapping_add(y);

        self.obj_move_children_by(obj, x, y, true);
        let res = self.obj_send_event(obj, EventCode::SCROLL, EventParam::None);
        if res != LvResult::Ok {
            return res;
        }
        let _ = self.obj_invalidate(obj);
        LvResult::Ok
    }

    /// `lv_obj_is_scrolling()`: an input device or a scroll animation is
    /// scrolling `obj`.
    pub fn obj_is_scrolling(&self, obj: Obj) -> bool {
        if !self.scroll_check_obj(obj) {
            return false;
        }

        let mut indev = self.indev_get_next(None);
        while let Some(i) = indev {
            if self.indev_get_scroll_obj(i) == Some(obj) {
                return true;
            }
            indev = self.indev_get_next(Some(i));
        }

        if self
            .anim_get(AnimTarget::Obj(obj), Some(&AnimExec::ObjScrollX))
            .is_some()
            || self
                .anim_get(AnimTarget::Obj(obj), Some(&AnimExec::ObjScrollY))
                .is_some()
        {
            return true;
        }

        false
    }

    /// `lv_obj_stop_scroll_anim()`
    pub fn obj_stop_scroll_anim(&mut self, obj: Obj) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        self.anim_delete(AnimTarget::Obj(obj), Some(&AnimExec::ObjScrollY));
        self.anim_delete(AnimTarget::Obj(obj), Some(&AnimExec::ObjScrollX));
    }

    /// `lv_obj_update_snap()`: scroll to the snap position of the children.
    pub fn obj_update_snap(&mut self, obj: Obj, anim_en: AnimEnable) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        self.obj_update_layout(obj);
        if !self.obj_is_valid(obj) {
            return;
        }
        let mut p = self.indev_scroll_get_snap_dist(obj);
        if p.x == COORD_MAX || p.x == COORD_MIN {
            p.x = 0;
        }
        if p.y == COORD_MAX || p.y == COORD_MIN {
            p.y = 0;
        }
        self.obj_scroll_by(obj, p.x, p.y, anim_en);
    }

    /// `lv_obj_get_scrollbar_area()`: returns `(hor_area, ver_area)`.
    ///
    /// Takes `&mut self` because the scroll bound getters send
    /// `LV_EVENT_GET_SELF_SIZE`.
    pub fn obj_get_scrollbar_area(&mut self, obj: Obj) -> (Area, Area) {
        let mut hor_area = Area::new(0, 0, -1, -1);
        let mut ver_area = Area::new(0, 0, -1, -1);
        if !self.scroll_check_obj(obj) {
            return (hor_area, ver_area);
        }
        self.scrollbar_area_core(obj, &mut hor_area, &mut ver_area);
        (hor_area, ver_area)
    }

    /// `lv_obj_scrollbar_invalidate()`
    pub fn obj_scrollbar_invalidate(&mut self, obj: Obj) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        let (hor_area, ver_area) = self.obj_get_scrollbar_area(obj);

        // `lv_area_get_size()` is unsigned: `<= 0` is `== 0`
        if hor_area.size() == 0 && ver_area.size() == 0 {
            return;
        }

        if hor_area.size() > 0 {
            let _ = self.obj_invalidate_area(obj, &hor_area);
        }
        if ver_area.size() > 0 {
            let _ = self.obj_invalidate_area(obj, &ver_area);
        }
    }

    /// `lv_obj_readjust_scroll()`: make sure the content is not scrolled in
    /// (e.g. after the content shrank).
    pub fn obj_readjust_scroll(&mut self, obj: Obj, anim_en: AnimEnable) {
        if !self.scroll_check_obj(obj) {
            return;
        }

        // Be sure the bottom side is not remains scrolled in
        // With snapping the content can't be scrolled in
        if self.obj_get_scroll_snap_y(obj) == ScrollSnap::None {
            let st = self.obj_get_scroll_top(obj);
            let mut sb = self.obj_get_scroll_bottom(obj);
            if !self.obj_is_valid(obj) {
                return;
            }
            if sb < 0 && st > 0 {
                sb = st.min(sb.wrapping_neg());
                self.obj_scroll_by(obj, 0, sb, anim_en);
            }
        }

        if !self.obj_is_valid(obj) {
            return;
        }

        if self.obj_get_scroll_snap_x(obj) == ScrollSnap::None {
            let sl = self.obj_get_scroll_left(obj);
            if !self.obj_is_valid(obj) {
                return;
            }
            let sr = self.obj_get_scroll_right(obj);
            if !self.obj_is_valid(obj) {
                return;
            }
            if !self.scroll_is_rtl(obj, Part::MAIN) {
                // Be sure the left side is not remains scrolled in
                if sr < 0 && sl > 0 {
                    let sr = sl.min(sr.wrapping_neg());
                    self.obj_scroll_by(obj, sr, 0, anim_en);
                }
            } else {
                // Be sure the right side is not remains scrolled in
                // (upstream computes `sr = LV_MIN(sr, -sl)` here but then
                // scrolls by `sl`; the unused value is dropped, the scroll is kept.)
                if sl < 0 && sr > 0 {
                    self.obj_scroll_by(obj, sl, 0, anim_en);
                }
            }
        }
    }

    /**********************
     *   STATIC FUNCTIONS
     **********************/

    /// `LV_CHECK_OBJ(obj, &lv_obj_class, ...)`: the handle must refer to a
    /// live object (every object is an `lv_obj_class` instance).
    #[inline]
    fn scroll_check_obj(&self, obj: Obj) -> bool {
        if self.obj_is_valid(obj) {
            true
        } else {
            crate::misc::check_arg_failed("obj_scroll: invalid object");
            false
        }
    }

    /// `obj->spec_attr` (`None` for a stale handle as well).
    #[inline]
    fn scroll_spec(&self, obj: Obj) -> Option<&SpecAttr> {
        self.obj(obj)?.spec.as_deref()
    }

    /// `lv_obj_allocate_spec_attr()`: allocate `spec_attr` with its defaults
    /// (`scroll_dir = LV_DIR_ALL`, `scrollbar_mode = LV_SCROLLBAR_MODE_AUTO`)
    /// if it does not exist yet. `None` only for a stale handle.
    fn scroll_allocate_spec_attr(&mut self, obj: Obj) -> Option<&mut SpecAttr> {
        let od = self.obj_mut(obj)?;
        Some(od.spec.get_or_insert_with(|| Box::new(SpecAttr::new())))
    }

    /// `lv_obj_get_child_count()`: `spec_attr ? spec_attr->child_cnt : 0`.
    #[inline]
    fn scroll_child_count(&self, obj: Obj) -> usize {
        self.scroll_spec(obj).map_or(0, |s| s.children.len())
    }

    /// `obj->spec_attr->children[i]`
    #[inline]
    fn scroll_child(&self, obj: Obj, i: usize) -> Option<Obj> {
        self.scroll_spec(obj)?.children.get(i).copied().map(Obj)
    }

    /// `obj->coords`
    #[inline]
    fn scroll_coords(&self, obj: Obj) -> Area {
        self.obj(obj).map_or(Area::new(0, 0, -1, -1), |o| o.coords)
    }

    /// `(int32_t)lv_obj_get_style_<prop>_internal(obj, part)` of a numeric property.
    #[inline]
    fn scroll_style_num(&self, obj: Obj, part: Part, prop: StyleProp) -> i32 {
        self.obj_get_style_prop(obj, part, prop).num()
    }

    /// `lv_obj_get_style_base_dir_internal(obj, part) == LV_BASE_DIR_RTL`
    #[inline]
    fn scroll_is_rtl(&self, obj: Obj, part: Part) -> bool {
        self.scroll_style_num(obj, part, StyleProp::BASE_DIR) == BaseDir::Rtl as i32
    }

    /// `lv_obj_send_event(obj, LV_EVENT_SCROLL_BEGIN, &a)` with the not yet
    /// started template `a`: stage it (unlinked from the animation list) so
    /// handlers can read and modify it through [`EventParam::Anim`], then
    /// take the possibly modified template back.
    fn scroll_send_begin_with_anim(&mut self, obj: Obj, a: &mut Anim) -> LvResult {
        let id = AnimId(self.anims.anims.insert(a.clone()));
        let res = self.obj_send_event(obj, EventCode::SCROLL_BEGIN, EventParam::Anim(id));
        if let Some(staged) = self.anims.anims.remove(id.0) {
            *a = staged;
        }
        res
    }

    /// The body of `lv_obj_get_scrollbar_area()` after the argument checks;
    /// the areas are already set to `(0, 0, -1, -1)`.
    fn scrollbar_area_core(&mut self, obj: Obj, hor_area: &mut Area, ver_area: &mut Area) {
        if !self.obj_is_scrollable(obj) {
            return;
        }

        let sm = self.obj_get_scrollbar_mode(obj);
        if sm == ScrollbarMode::Off {
            return;
        }

        // If there is no indev scrolling this object but `mode==active` return
        let mut indev = self.indev_get_next(None);
        if sm == ScrollbarMode::Active {
            while let Some(i) = indev {
                if self.indev_get_scroll_obj(i) == Some(obj) {
                    break;
                }
                indev = self.indev_get_next(Some(i));
            }
            if indev.is_none() {
                return;
            }
        }

        let st = self.obj_get_scroll_top(obj);
        let sb = self.obj_get_scroll_bottom(obj);
        // Extent callbacks may delete the object before the next getter.
        if !self.obj_is_valid(obj) {
            return;
        }
        let sl = self.obj_get_scroll_left(obj);
        if !self.obj_is_valid(obj) {
            return;
        }
        let sr = self.obj_get_scroll_right(obj);
        if !self.obj_is_valid(obj) {
            return;
        }

        let dir = self.obj_get_scroll_dir(obj);

        let mut ver_draw = false;
        if dir.intersects(Dir::VER)
            && (sm == ScrollbarMode::On
                || (sm == ScrollbarMode::Auto && (st > 0 || sb > 0))
                || (sm == ScrollbarMode::Active
                    && indev.is_some_and(|i| self.indev_get_scroll_dir(i) == Dir::VER)))
        {
            ver_draw = true;
        }

        let mut hor_draw = false;
        if dir.intersects(Dir::HOR)
            && (sm == ScrollbarMode::On
                || (sm == ScrollbarMode::Auto && (sl > 0 || sr > 0))
                || (sm == ScrollbarMode::Active
                    && indev.is_some_and(|i| self.indev_get_scroll_dir(i) == Dir::HOR)))
        {
            hor_draw = true;
        }

        if !hor_draw && !ver_draw {
            return;
        }

        let rtl = self.scroll_is_rtl(obj, Part::SCROLLBAR);

        let top_space = self.scroll_style_num(obj, Part::SCROLLBAR, StyleProp::PAD_TOP);
        let bottom_space = self.scroll_style_num(obj, Part::SCROLLBAR, StyleProp::PAD_BOTTOM);
        let left_space = self.scroll_style_num(obj, Part::SCROLLBAR, StyleProp::PAD_LEFT);
        let right_space = self.scroll_style_num(obj, Part::SCROLLBAR, StyleProp::PAD_RIGHT);
        let thickness = self.scroll_style_num(obj, Part::SCROLLBAR, StyleProp::WIDTH);
        let length = self.scroll_style_num(obj, Part::SCROLLBAR, StyleProp::LENGTH);

        let obj_h = self.obj_get_height(obj);
        let obj_w = self.obj_get_width(obj);

        // Space required for the vertical and horizontal scrollbars
        let ver_reg_space = if ver_draw { thickness } else { 0 };
        let hor_req_space = if hor_draw { thickness } else { 0 };
        let mut rem: i32;

        // `(lv_opa_t)v.num`
        let bg_opa = self.scroll_style_num(obj, Part::SCROLLBAR, StyleProp::BG_OPA) as u8;
        let border_opa = self.scroll_style_num(obj, Part::SCROLLBAR, StyleProp::BORDER_OPA) as u8;
        if bg_opa <= OPA_MIN && border_opa <= OPA_MIN {
            return;
        }

        let coords = self.scroll_coords(obj);
        // SCROLLBAR_MIN_SIZE
        let min_size = self.dpx(SCROLLBAR_MIN_SIZE_DPX);

        // Draw vertical scrollbar if the mode is ON or can be scrolled in this direction
        let content_h = obj_h.wrapping_add(st).wrapping_add(sb);
        if ver_draw && content_h != 0 {
            ver_area.y1 = coords.y1;
            ver_area.y2 = coords.y2;
            if rtl {
                ver_area.x1 = coords.x1.wrapping_add(left_space);
                ver_area.x2 = ver_area.x1.wrapping_add(thickness).wrapping_sub(1);
            } else {
                ver_area.x2 = coords.x2.wrapping_sub(right_space);
                ver_area.x1 = ver_area.x2.wrapping_sub(thickness).wrapping_add(1);
            }

            let track_h = obj_h
                .wrapping_sub(top_space)
                .wrapping_sub(bottom_space)
                .wrapping_sub(hor_req_space);
            let mut sb_h = mul_div(track_h, obj_h, content_h);
            // Style-defined size, calculated size, or minimum size
            sb_h = (if length > 0 { length } else { sb_h }).max(min_size);
            // Limit scrollbar length to parent height
            sb_h = sb_h.min(obj_h);
            // Remaining size from the scrollbar track that is not the scrollbar itself
            rem = track_h.wrapping_sub(sb_h);
            // The size of the content which can be really scrolled
            let scroll_h = content_h.wrapping_sub(obj_h);
            if scroll_h <= 0 {
                ver_area.y1 = coords.y1.wrapping_add(top_space);
                ver_area.y2 = coords
                    .y2
                    .wrapping_sub(bottom_space)
                    .wrapping_sub(hor_req_space)
                    .wrapping_sub(1);
            } else {
                let mut sb_y = mul_div(rem, sb, scroll_h);
                sb_y = rem.wrapping_sub(sb_y);

                ver_area.y1 = coords.y1.wrapping_add(sb_y).wrapping_add(top_space);
                ver_area.y2 = ver_area.y1.wrapping_add(sb_h).wrapping_sub(1);
                if ver_area.y1 < coords.y1.wrapping_add(top_space) {
                    ver_area.y1 = coords.y1.wrapping_add(top_space);
                    if ver_area.y1.wrapping_add(min_size) > ver_area.y2 {
                        ver_area.y2 = ver_area.y1.wrapping_add(min_size);
                    }
                }
                if ver_area.y2 > coords.y2.wrapping_sub(hor_req_space).wrapping_sub(bottom_space) {
                    ver_area.y2 = coords.y2.wrapping_sub(hor_req_space).wrapping_sub(bottom_space);
                    if ver_area.y2.wrapping_sub(min_size) < ver_area.y1 {
                        ver_area.y1 = ver_area.y2.wrapping_sub(min_size);
                    }
                }
            }
        }

        // Draw horizontal scrollbar if the mode is ON or can be scrolled in this direction
        let content_w = obj_w.wrapping_add(sl).wrapping_add(sr);
        if hor_draw && content_w != 0 {
            hor_area.y2 = coords.y2.wrapping_sub(bottom_space);
            hor_area.y1 = hor_area.y2.wrapping_sub(thickness).wrapping_add(1);
            hor_area.x1 = coords.x1;
            hor_area.x2 = coords.x2;

            let track_w = obj_w
                .wrapping_sub(left_space)
                .wrapping_sub(right_space)
                .wrapping_sub(ver_reg_space);
            let mut sb_w = mul_div(track_w, obj_w, content_w);
            // Style-defined size, calculated size, or minimum size
            sb_w = (if length > 0 { length } else { sb_w }).max(min_size);
            // Limit scrollbar length to parent width
            sb_w = sb_w.min(obj_w);
            // Remaining size from the scrollbar track that is not the scrollbar itself
            rem = track_w.wrapping_sub(sb_w);
            // The size of the content which can be really scrolled
            let scroll_w = content_w.wrapping_sub(obj_w);
            if scroll_w <= 0 {
                if rtl {
                    hor_area.x1 = coords
                        .x1
                        .wrapping_add(left_space)
                        .wrapping_add(ver_reg_space)
                        .wrapping_sub(1);
                    hor_area.x2 = coords.x2.wrapping_sub(right_space);
                } else {
                    hor_area.x1 = coords.x1.wrapping_add(left_space);
                    hor_area.x2 = coords
                        .x2
                        .wrapping_sub(right_space)
                        .wrapping_sub(ver_reg_space)
                        .wrapping_sub(1);
                }
            } else {
                let mut sb_x = mul_div(rem, sr, scroll_w);
                sb_x = rem.wrapping_sub(sb_x);

                if rtl {
                    hor_area.x1 = coords
                        .x1
                        .wrapping_add(sb_x)
                        .wrapping_add(left_space)
                        .wrapping_add(ver_reg_space);
                    hor_area.x2 = hor_area.x1.wrapping_add(sb_w).wrapping_sub(1);
                    if hor_area.x1 < coords.x1.wrapping_add(left_space).wrapping_add(ver_reg_space) {
                        hor_area.x1 = coords.x1.wrapping_add(left_space).wrapping_add(ver_reg_space);
                        if hor_area.x1.wrapping_add(min_size) > hor_area.x2 {
                            hor_area.x2 = hor_area.x1.wrapping_add(min_size);
                        }
                    }
                    if hor_area.x2 > coords.x2.wrapping_sub(right_space) {
                        hor_area.x2 = coords.x2.wrapping_sub(right_space);
                        if hor_area.x2.wrapping_sub(min_size) < hor_area.x1 {
                            hor_area.x1 = hor_area.x2.wrapping_sub(min_size);
                        }
                    }
                } else {
                    hor_area.x1 = coords.x1.wrapping_add(sb_x).wrapping_add(left_space);
                    hor_area.x2 = hor_area.x1.wrapping_add(sb_w).wrapping_sub(1);
                    if hor_area.x1 < coords.x1.wrapping_add(left_space) {
                        hor_area.x1 = coords.x1.wrapping_add(left_space);
                        if hor_area.x1.wrapping_add(min_size) > hor_area.x2 {
                            hor_area.x2 = hor_area.x1.wrapping_add(min_size);
                        }
                    }
                    if hor_area.x2 > coords.x2.wrapping_sub(ver_reg_space).wrapping_sub(right_space) {
                        hor_area.x2 = coords.x2.wrapping_sub(ver_reg_space).wrapping_sub(right_space);
                        if hor_area.x2.wrapping_sub(min_size) < hor_area.x1 {
                            hor_area.x1 = hor_area.x2.wrapping_sub(min_size);
                        }
                    }
                }
            }
        }
    }

    /// `scroll_area_into_view()` (static): scroll the parent of `child` so
    /// that the area (`area_obj`'s coordinates, read live like the C
    /// `const lv_area_t *`) becomes visible, honouring snapping and the
    /// allowed scroll directions. `scroll_value` accumulates the animated
    /// scroll amounts for the recursive variant.
    fn scroll_area_into_view(
        &mut self,
        area_obj: Obj,
        child: Obj,
        scroll_value: &mut Point,
        anim_en: AnimEnable,
    ) {
        // `lv_obj_is_scrollable(NULL)`: its `LV_CHECK_OBJ` NULL check fails
        // (warn / assert) and it returns false
        let Some(parent) = self.obj_get_parent(child) else {
            crate::misc::check_arg_failed("obj_scroll: invalid object");
            return;
        };
        if !self.obj_is_scrollable(parent) {
            return;
        }

        let scroll_dir = self.obj_get_scroll_dir(parent);
        let mut snap_goal: i32;
        let mut act: i32;

        let mut y_scroll: i32 = 0;
        let snap_y = self.obj_get_scroll_snap_y(parent);
        // `area_tmp`: the child's coordinates when snapping, else the area
        let area_tmp_obj = if snap_y != ScrollSnap::None {
            child
        } else {
            area_obj
        };

        let stop = self.obj_get_style_space_top(parent, Part::MAIN);
        let sbottom = self.obj_get_style_space_bottom(parent, Part::MAIN);
        let pc = self.scroll_coords(parent);
        let at = self.scroll_coords(area_tmp_obj);
        let top_diff = pc
            .y1
            .wrapping_add(stop)
            .wrapping_sub(at.y1)
            .wrapping_sub(scroll_value.y);
        let bottom_diff = pc
            .y2
            .wrapping_sub(sbottom)
            .wrapping_sub(at.y2)
            .wrapping_sub(scroll_value.y)
            .wrapping_neg();
        let parent_h = self
            .obj_get_height(parent)
            .wrapping_sub(stop)
            .wrapping_sub(sbottom);
        if top_diff >= 0 && bottom_diff >= 0 {
            y_scroll = 0;
        } else if top_diff > 0 {
            y_scroll = top_diff;
            // Do not let scrolling in
            let st = self.obj_get_scroll_top(parent);
            if st.wrapping_sub(y_scroll) < 0 {
                y_scroll = 0;
            }
        } else if bottom_diff > 0 {
            y_scroll = bottom_diff.wrapping_neg();
            // Do not let scrolling in
            let sb = self.obj_get_scroll_bottom(parent);
            if !self.obj_is_valid(parent) || !self.obj_is_valid(child) || !self.obj_is_valid(area_obj) {
                return;
            }
            if sb.wrapping_add(y_scroll) < 0 {
                y_scroll = 0;
            }
        }

        let pc = self.scroll_coords(parent);
        let at = self.scroll_coords(area_tmp_obj);
        match snap_y {
            ScrollSnap::Start => {
                snap_goal = pc.y1.wrapping_add(stop);
                act = at.y1.wrapping_add(y_scroll);
                y_scroll = y_scroll.wrapping_add(snap_goal.wrapping_sub(act));
            }
            ScrollSnap::End => {
                snap_goal = pc.y2.wrapping_sub(sbottom);
                act = at.y2.wrapping_add(y_scroll);
                y_scroll = y_scroll.wrapping_add(snap_goal.wrapping_sub(act));
            }
            ScrollSnap::Center => {
                snap_goal = pc.y1.wrapping_add(stop).wrapping_add(parent_h / 2);
                act = (at.height() / 2).wrapping_add(at.y1).wrapping_add(y_scroll);
                y_scroll = y_scroll.wrapping_add(snap_goal.wrapping_sub(act));
            }
            ScrollSnap::None => {}
        }

        let mut x_scroll: i32 = 0;
        let snap_x = self.obj_get_scroll_snap_x(parent);
        let area_tmp_obj = if snap_x != ScrollSnap::None {
            child
        } else {
            area_obj
        };

        let sleft = self.obj_get_style_space_left(parent, Part::MAIN);
        let sright = self.obj_get_style_space_right(parent, Part::MAIN);
        let pc = self.scroll_coords(parent);
        let at = self.scroll_coords(area_tmp_obj);
        let left_diff = pc
            .x1
            .wrapping_add(sleft)
            .wrapping_sub(at.x1)
            .wrapping_sub(scroll_value.x);
        let right_diff = pc
            .x2
            .wrapping_sub(sright)
            .wrapping_sub(at.x2)
            .wrapping_sub(scroll_value.x)
            .wrapping_neg();
        if left_diff >= 0 && right_diff >= 0 {
            x_scroll = 0;
        } else if left_diff > 0 {
            x_scroll = left_diff;
            // Do not let scrolling in
            let sl = self.obj_get_scroll_left(parent);
            if !self.obj_is_valid(parent) || !self.obj_is_valid(child) || !self.obj_is_valid(area_obj) {
                return;
            }
            if sl.wrapping_sub(x_scroll) < 0 {
                x_scroll = 0;
            }
        } else if right_diff > 0 {
            x_scroll = right_diff.wrapping_neg();
            // Do not let scrolling in
            let sr = self.obj_get_scroll_right(parent);
            if !self.obj_is_valid(parent) || !self.obj_is_valid(child) || !self.obj_is_valid(area_obj) {
                return;
            }
            if sr.wrapping_add(x_scroll) < 0 {
                x_scroll = 0;
            }
        }

        let parent_w = self
            .obj_get_width(parent)
            .wrapping_sub(sleft)
            .wrapping_sub(sright);
        let pc = self.scroll_coords(parent);
        let at = self.scroll_coords(area_tmp_obj);
        match snap_x {
            ScrollSnap::Start => {
                snap_goal = pc.x1.wrapping_add(sleft);
                act = at.x1.wrapping_add(x_scroll);
                x_scroll = x_scroll.wrapping_add(snap_goal.wrapping_sub(act));
            }
            ScrollSnap::End => {
                snap_goal = pc.x2.wrapping_sub(sright);
                act = at.x2.wrapping_add(x_scroll);
                x_scroll = x_scroll.wrapping_add(snap_goal.wrapping_sub(act));
            }
            ScrollSnap::Center => {
                snap_goal = pc.x1.wrapping_add(sleft).wrapping_add(parent_w / 2);
                act = (at.width() / 2).wrapping_add(at.x1).wrapping_add(x_scroll);
                x_scroll = x_scroll.wrapping_add(snap_goal.wrapping_sub(act));
            }
            ScrollSnap::None => {}
        }

        // Remove any pending scroll animations.
        self.anim_delete(AnimTarget::Obj(parent), Some(&AnimExec::ObjScrollY));
        if !self.obj_is_valid(parent) || !self.obj_is_valid(child) || !self.obj_is_valid(area_obj) {
            return;
        }
        self.anim_delete(AnimTarget::Obj(parent), Some(&AnimExec::ObjScrollX));
        if !self.obj_is_valid(parent) || !self.obj_is_valid(child) || !self.obj_is_valid(area_obj) {
            return;
        }

        if !scroll_dir.contains(Dir::LEFT) && x_scroll < 0 {
            x_scroll = 0;
        }
        if !scroll_dir.contains(Dir::RIGHT) && x_scroll > 0 {
            x_scroll = 0;
        }
        if !scroll_dir.contains(Dir::TOP) && y_scroll < 0 {
            y_scroll = 0;
        }
        if !scroll_dir.contains(Dir::BOTTOM) && y_scroll > 0 {
            y_scroll = 0;
        }

        scroll_value.x = scroll_value.x.wrapping_add(if anim_en { x_scroll } else { 0 });
        scroll_value.y = scroll_value.y.wrapping_add(if anim_en { y_scroll } else { 0 });
        self.obj_scroll_by(parent, x_scroll, y_scroll, anim_en);
    }
}

/// `mul_div()` (static): `((int64_t)v * factor) / divisor` truncated to `int32_t`.
#[inline]
fn mul_div(v: i32, factor: i32, divisor: i32) -> i32 {
    ((v as i64 * factor as i64) / divisor as i64) as i32
}

/// `scroll_end_cb()` (static): the `deleted_cb` of scroll animations.
fn scroll_end_cb() -> AnimCallback {
    Rc::new(|ui: &mut Ui, a: AnimId| {
        let Some(anim) = ui.anim(a) else { return };
        // Do not sent END event if there wasn't a BEGIN
        if anim.start_cb_called
            && let AnimTarget::Obj(obj) = anim.var
            && ui.obj_is_valid(obj)
        {
            let _ = ui.obj_send_event(obj, EventCode::SCROLL_END, EventParam::None);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::UiConfig;
    use crate::core::obj::{OBJ_CLASS, ObjData, ObjFlags, ObjId};
    use crate::misc::style::State;
    use alloc::vec::Vec;

    /// Insert a bare object without going through `lv_obj_create()` (other
    /// core files are still being ported).
    fn raw_obj(ui: &mut Ui, coords: Area, spec: Option<SpecAttr>) -> Obj {
        let data = ObjData {
            class: &OBJ_CLASS,
            parent: None,
            spec: spec.map(Box::new),
            styles: Vec::new(),
            style_main_prop_is_set: 0,
            style_other_prop_is_set: 0,
            user_data: None,
            coords,
            flags: ObjFlags::empty(),
            radio_button: false,
            state: State::default(),
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
        Obj(ObjId(ui.objs.insert(data)))
    }

    fn scrolled_spec(x: i32, y: i32) -> SpecAttr {
        let mut s = SpecAttr::new();
        s.scroll = Point::new(x, y);
        s
    }

    #[test]
    fn scroll_extent_queries_stop_if_self_size_deletes_the_object() {
        use crate::misc::style::StyleSelector;
        use alloc::rc::Rc;
        for query in 0..3 {
            let mut ui = Ui::new(UiConfig::default());
            let display = ui.display_create(160, 120);
            let screen = ui.display_get_screen_active(Some(display)).unwrap();
            let obj = ui.obj_create(Some(screen));
            if query == 2 {
                ui.obj_set_style_base_dir(obj, BaseDir::Rtl, StyleSelector::DEFAULT);
            }
            ui.obj_add_event_cb(
                obj,
                Rc::new(move |ui, _| ui.obj_delete(obj)),
                EventCode::GET_SELF_SIZE,
                None,
            );
            let extent = match query {
                0 => ui.obj_get_scroll_bottom(obj),
                1 => ui.obj_get_scroll_right(obj),
                _ => ui.obj_get_scroll_left(obj),
            };
            assert_eq!(extent, 0);
            assert!(!ui.obj_is_valid(obj));
        }
    }

    #[test]
    fn scrollbar_bounded_scroll_and_readjust_stop_after_self_size_deletion() {
        use crate::misc::style::StyleSelector;
        for operation in 0..5 {
            let mut ui = Ui::new(UiConfig::default());
            let display = ui.display_create(160, 120);
            let screen = ui.display_get_screen_active(Some(display)).unwrap();
            let obj = ui.obj_create(Some(screen));
            ui.obj_set_size(obj, 100, 80);
            if operation == 2 {
                ui.obj_set_style_base_dir(obj, BaseDir::Rtl, StyleSelector::DEFAULT);
            }
            ui.obj_update_layout(screen);
            ui.obj_add_event_cb(
                obj,
                Rc::new(move |ui, _| ui.obj_delete(obj)),
                EventCode::GET_SELF_SIZE,
                None,
            );
            match operation {
                0 => {
                    let (horizontal, vertical) = ui.obj_get_scrollbar_area(obj);
                    assert_eq!(horizontal, Area::new(0, 0, -1, -1));
                    assert_eq!(vertical, Area::new(0, 0, -1, -1));
                }
                1 => ui.obj_scroll_by_bounded(obj, -10, 0, false),
                2 => ui.obj_scroll_by_bounded(obj, 10, 0, false),
                3 => ui.obj_scroll_by_bounded(obj, 0, -10, false),
                _ => ui.obj_readjust_scroll(obj, false),
            }
            assert!(!ui.obj_is_valid(obj));
        }
    }

    #[test]
    fn scrolling_both_axes_stops_when_the_first_axis_callback_deletes_object() {
        let mut ui = Ui::new(UiConfig::default());
        let display = ui.display_create(160, 120);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let obj = ui.obj_create(Some(screen));
        ui.obj_set_size(obj, 100, 80);
        let content = ui.obj_create(Some(obj));
        ui.obj_set_size(content, 300, 300);
        ui.obj_update_layout(screen);
        ui.obj_add_event_cb(
            obj,
            Rc::new(move |ui, _| ui.obj_delete(obj)),
            EventCode::SCROLL,
            None,
        );
        ui.obj_scroll_to(obj, 20, 20, false);
        assert!(!ui.obj_is_valid(obj));
    }

    #[test]
    fn cancelling_scroll_animation_stops_when_scroll_end_deletes_object() {
        for operation in 0..3 {
            let mut ui = Ui::new(UiConfig::default());
            let display = ui.display_create(160, 120);
            let screen = ui.display_get_screen_active(Some(display)).unwrap();
            let obj = ui.obj_create(Some(screen));
            ui.obj_update_layout(screen);
            ui.obj_scroll_by(obj, 100, 100, true);
            ui.tick_inc(20);
            ui.anim_refr_now();
            ui.obj_add_event_cb(
                obj,
                Rc::new(move |ui, _| ui.obj_delete(obj)),
                EventCode::SCROLL_END,
                None,
            );
            match operation {
                0 => ui.obj_scroll_by(obj, 5, 5, false),
                1 => ui.obj_scroll_to_x(obj, 5, false),
                _ => ui.obj_scroll_to_y(obj, 5, false),
            }
            assert!(!ui.obj_is_valid(obj), "operation {operation}");
        }
    }

    #[test]
    fn scrolling_into_view_stops_if_a_callback_deletes_its_ancestor() {
        for event in [EventCode::GET_SELF_SIZE, EventCode::SCROLL] {
            for recursive in [false, true] {
                let mut ui = Ui::new(UiConfig::default());
                let display = ui.display_create(160, 120);
                let screen = ui.display_get_screen_active(Some(display)).unwrap();
                let parent = ui.obj_create(Some(screen));
                ui.obj_set_size(parent, 100, 80);
                let child = ui.obj_create(Some(parent));
                ui.obj_set_size(child, 10, 10);
                ui.obj_set_pos(child, 200, 200);
                ui.obj_update_layout(screen);
                ui.obj_add_event_cb(parent, Rc::new(move |ui, _| ui.obj_delete(parent)), event, None);
                if recursive {
                    ui.obj_scroll_to_view_recursive(child, false);
                } else {
                    ui.obj_scroll_to_view(child, false);
                }
                assert!(!ui.obj_is_valid(parent));
                assert!(!ui.obj_is_valid(child));
            }
        }
    }

    #[test]
    fn mul_div_uses_64bit_intermediate_and_truncates() {
        assert_eq!(mul_div(10, 3, 4), 7);
        assert_eq!(mul_div(-10, 3, 4), -7); // C truncates toward zero
        assert_eq!(mul_div(100_000, 100_000, 1_000), 10_000_000);
        assert_eq!(mul_div(0, 5, 3), 0);
    }

    #[test]
    fn getters_without_spec_attr_return_defaults() {
        let mut ui = Ui::new(UiConfig::default());
        let o = raw_obj(&mut ui, Area::new(0, 0, 99, 99), None);
        assert_eq!(ui.obj_get_scrollbar_mode(o), ScrollbarMode::Auto);
        assert_eq!(ui.obj_get_scroll_dir(o), Dir::ALL);
        assert_eq!(ui.obj_get_scroll_snap_x(o), ScrollSnap::None);
        assert_eq!(ui.obj_get_scroll_snap_y(o), ScrollSnap::None);
        assert_eq!(ui.obj_get_scroll_x(o), 0);
        assert_eq!(ui.obj_get_scroll_y(o), 0);
        assert_eq!(ui.obj_get_scroll_top(o), 0);
        assert_eq!(ui.obj_get_scroll_end(o), Point::new(0, 0));
        // Nothing was allocated by the getters.
        assert!(ui.obj(o).unwrap().spec.is_none());
    }

    #[test]
    fn scroll_getters_negate_the_stored_offset() {
        let mut ui = Ui::new(UiConfig::default());
        let o = raw_obj(&mut ui, Area::new(0, 0, 99, 99), Some(scrolled_spec(-30, -40)));
        assert_eq!(ui.obj_get_scroll_x(o), 30);
        assert_eq!(ui.obj_get_scroll_y(o), 40);
        assert_eq!(ui.obj_get_scroll_top(o), 40);
        assert_eq!(ui.obj_get_scroll_end(o), Point::new(30, 40));
    }

    #[test]
    fn setters_allocate_spec_attr_with_defaults() {
        let mut ui = Ui::new(UiConfig::default());
        let o = raw_obj(&mut ui, Area::new(0, 0, 99, 99), None);

        ui.obj_set_scroll_dir(o, Dir::HOR);
        assert!(ui.obj(o).unwrap().spec.is_some());
        assert_eq!(ui.obj_get_scroll_dir(o), Dir::HOR);
        // Allocation defaults of lv_obj_allocate_spec_attr
        assert_eq!(ui.obj_get_scrollbar_mode(o), ScrollbarMode::Auto);

        ui.obj_set_scroll_snap_x(o, ScrollSnap::Center);
        ui.obj_set_scroll_snap_y(o, ScrollSnap::End);
        assert_eq!(ui.obj_get_scroll_snap_x(o), ScrollSnap::Center);
        assert_eq!(ui.obj_get_scroll_snap_y(o), ScrollSnap::End);

        // Same mode: early return before lv_obj_invalidate
        ui.obj_set_scrollbar_mode(o, ScrollbarMode::Auto);
        assert_eq!(ui.obj_get_scrollbar_mode(o), ScrollbarMode::Auto);

        let o2 = raw_obj(&mut ui, Area::new(0, 0, 9, 9), None);
        ui.obj_set_scrollbar_mode(o2, ScrollbarMode::Auto);
        assert!(ui.obj(o2).unwrap().spec.is_some());
    }

    #[test]
    fn zero_scrolls_are_no_ops() {
        let mut ui = Ui::new(UiConfig::default());
        let o = raw_obj(&mut ui, Area::new(0, 0, 99, 99), None);
        assert_eq!(ui.obj_scroll_by_raw(o, 0, 0), LvResult::Ok);
        // No spec_attr allocated for an empty scroll
        assert!(ui.obj(o).unwrap().spec.is_none());
        ui.obj_scroll_by(o, 0, 0, true);
        ui.obj_scroll_by(o, 0, 0, false);
        ui.obj_scroll_by_bounded(o, 0, 0, true);
        assert_eq!(ui.anim_count_running(), 0);
    }

    #[test]
    #[cfg(not(feature = "check-arg-panic"))]
    fn stale_handles_are_rejected() {
        let mut ui = Ui::new(UiConfig::default());
        let o = raw_obj(&mut ui, Area::new(0, 0, 9, 9), Some(scrolled_spec(-5, -5)));
        ui.objs.remove(o.0.0);
        assert_eq!(ui.obj_get_scroll_x(o), 0);
        assert_eq!(ui.obj_get_scrollbar_mode(o), ScrollbarMode::Off);
        assert_eq!(ui.obj_get_scroll_dir(o), Dir::NONE);
        assert_eq!(ui.obj_scroll_by_raw(o, 1, 1), LvResult::Invalid);
        assert_eq!(
            ui.obj_get_scrollbar_area(o),
            (Area::new(0, 0, -1, -1), Area::new(0, 0, -1, -1))
        );
        ui.obj_set_scroll_dir(o, Dir::HOR);
        ui.obj_stop_scroll_anim(o);
    }

    #[test]
    #[cfg(feature = "check-arg-panic")]
    #[should_panic(expected = "argument check failed")]
    fn stale_handle_trips_the_argument_check() {
        let mut ui = Ui::new(UiConfig::default());
        let o = raw_obj(&mut ui, Area::new(0, 0, 9, 9), None);
        ui.objs.remove(o.0.0);
        let _ = ui.obj_get_scroll_x(o);
    }

    /// Start a scroll animation the way `lv_obj_scroll_by()` builds it (but
    /// without the event, which needs the event module).
    fn start_scroll_anim(ui: &mut Ui, o: Obj, exec: AnimExec, start: i32, end: i32) -> AnimId {
        let mut a = Anim::new();
        a.set_var(AnimTarget::Obj(o));
        a.set_deleted_cb(Some(scroll_end_cb()));
        a.set_duration(Ui::anim_speed_clamped(
            400,
            SCROLL_ANIM_TIME_MIN,
            SCROLL_ANIM_TIME_MAX,
        ));
        a.set_values(start, end);
        a.set_exec_cb(exec);
        a.set_path_cb(AnimPath::EaseOut);
        ui.anim_start(&a)
    }

    #[test]
    fn scroll_end_reports_the_animation_targets() {
        let mut ui = Ui::new(UiConfig::default());
        let o = raw_obj(&mut ui, Area::new(0, 0, 99, 99), Some(scrolled_spec(-10, -20)));
        // Like lv_obj_scroll_by: values (-sx, -sx + dx); early apply of the
        // start value is a zero raw scroll.
        start_scroll_anim(&mut ui, o, AnimExec::ObjScrollX, -10, -60);
        start_scroll_anim(&mut ui, o, AnimExec::ObjScrollY, -20, -25);
        assert_eq!(ui.obj_get_scroll_x(o), 10);
        assert_eq!(ui.obj_get_scroll_end(o), Point::new(60, 25));

        // lv_obj_scroll_to_x to the current position: deletes the x anim and
        // returns before any layout update (dx == dy == 0).
        ui.obj_scroll_to_x(o, 10, false);
        assert_eq!(ui.obj_get_scroll_end(o), Point::new(10, 25));
        assert!(
            ui.anim_get(AnimTarget::Obj(o), Some(&AnimExec::ObjScrollY))
                .is_some()
        );

        ui.obj_stop_scroll_anim(o);
        assert_eq!(ui.obj_get_scroll_end(o), Point::new(10, 20));
        assert_eq!(ui.anim_count_running(), 0);
    }

    #[test]
    fn scroll_anim_time_encoding() {
        // lv_anim_speed_clamped(hor_res >> 1, 200, 400) for an 800 px wide display
        let t = Ui::anim_speed_clamped(800 >> 1, SCROLL_ANIM_TIME_MIN, SCROLL_ANIM_TIME_MAX);
        // 400 px at 400 px/s = 1000 ms, clamped to 400 ms
        assert_eq!(Ui::anim_resolve_speed(t, 0, 400), 400);
        // 40 px -> 100 ms, clamped up to 200 ms
        assert_eq!(Ui::anim_resolve_speed(t, 0, 40), 200);
        assert_eq!(Ui::anim_resolve_speed(t, 0, 120), 300);
    }
}
