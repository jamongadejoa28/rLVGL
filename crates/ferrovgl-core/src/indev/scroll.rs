//! Scroll selection, momentum, elasticity and snapping from `lv_indev_scroll.c`.
use super::*;
use crate::core::obj::ScrollSnap;
use crate::misc::style::{Part, State};
use crate::misc::types::coord;

const ELASTIC_SLOWNESS_FACTOR: i32 = 4;

impl Ui {
    /// `lv_indev_find_scroll_obj()`: find the deepest ancestor that can scroll
    /// in this direction, falling back to the last elastic candidate.
    #[must_use]
    pub fn indev_find_scroll_obj(&mut self, indev: IndevId) -> Option<Obj> {
        let d = self.indev_mut(indev)?;
        d.pointer.scroll_sum.x += d.pointer.vect.x;
        d.pointer.scroll_sum.y += d.pointer.vect.y;
        let sum = d.pointer.scroll_sum;
        let limit = i32::from(d.scroll_limit);
        let mut current = d.pointer.act_obj;
        let mut candidate = None;
        let mut direction = Dir::NONE;
        while let Some(obj) = current.filter(|&o| self.obj_is_valid(o)) {
            let mut local = sum;
            self.indev_transform_vector_inner(obj, &mut local, false, false);
            let horizontal = local.x.abs() > local.y.abs();
            let chain = if horizontal {
                ObjFlags::SCROLL_CHAIN_HOR
            } else {
                ObjFlags::SCROLL_CHAIN_VER
            };
            if !self.obj_has_flag(obj, ObjFlags::SCROLLABLE) {
                if !self.obj_has_flag(obj, chain) {
                    break;
                }
                current = self.obj_get_parent(obj);
                continue;
            }
            let allowed = self.obj_get_scroll_dir(obj);
            let mut left = horizontal && allowed.contains(Dir::LEFT);
            let mut right = horizontal && allowed.contains(Dir::RIGHT);
            let mut up = !horizontal && allowed.contains(Dir::TOP);
            let mut down = !horizontal && allowed.contains(Dir::BOTTOM);
            let (sl, sr) = if self.obj_get_scroll_snap_x(obj) == ScrollSnap::None {
                let left = self.obj_get_scroll_left(obj);
                if self.indev(indev).is_none() || !self.obj_is_valid(obj) {
                    return None;
                }
                let right = self.obj_get_scroll_right(obj);
                if self.indev(indev).is_none() || !self.obj_is_valid(obj) {
                    return None;
                }
                (left, right)
            } else {
                let (start, end) = self.indev_more_snap_points(obj, true);
                (if start { 1 } else { -1 }, if end { 1 } else { -1 })
            };
            let (st, sb) = if self.obj_get_scroll_snap_y(obj) == ScrollSnap::None {
                let top = self.obj_get_scroll_top(obj);
                let bottom = self.obj_get_scroll_bottom(obj);
                if self.indev(indev).is_none() || !self.obj_is_valid(obj) {
                    return None;
                }
                (top, bottom)
            } else {
                let (start, end) = self.indev_more_snap_points(obj, false);
                (if start { 1 } else { -1 }, if end { 1 } else { -1 })
            };
            if (st > 0 || sb > 0) && ((up && local.y >= limit) || (down && local.y <= -limit)) {
                candidate = Some(obj);
                direction = Dir::VER;
            }
            if (sl > 0 || sr > 0) && ((left && local.x >= limit) || (right && local.x <= -limit)) {
                candidate = Some(obj);
                direction = Dir::HOR;
            }
            if st <= 0 {
                up = false;
            }
            if sb <= 0 {
                down = false;
            }
            if sl <= 0 {
                left = false;
            }
            if sr <= 0 {
                right = false;
            }
            if (left && local.x >= limit)
                || (right && local.x <= -limit)
                || (up && local.y >= limit)
                || (down && local.y <= -limit)
            {
                break;
            }
            if !self.obj_has_flag(obj, chain) {
                break;
            }
            current = self.obj_get_parent(obj);
        }
        if candidate.is_some()
            && let Some(d) = self.indev_mut(indev)
        {
            d.pointer.scroll_dir = direction;
            d.pointer.scroll_obj = candidate;
            d.pointer.scroll_sum = Point::default();
        }
        candidate
    }

    /// `lv_indev_scroll_handler()`.
    pub fn indev_scroll_handler(&mut self, indev: IndevId) {
        let Some(d) = self.indev(indev) else {
            return;
        };
        if d.pointer.vect == Point::default() {
            return;
        }
        let scroll = if let Some(obj) = d.pointer.scroll_obj {
            obj
        } else {
            let Some(obj) = self.indev_find_scroll_obj(indev) else {
                return;
            };
            self.indev_init_scroll_limits(indev);
            if let Some(active) = self.indev(indev).and_then(|d| d.pointer.act_obj) {
                self.obj_remove_state(active, State::PRESSED);
            }
            if self.indev_reset_check(indev) {
                return;
            }
            self.obj_send_event(obj, EventCode::SCROLL_BEGIN, EventParam::None);
            if self.indev_reset_check(indev) || !self.obj_is_valid(obj) {
                return;
            }
            obj
        };
        if !self.obj_is_valid(scroll) {
            self.indev_reset(Some(indev), Some(scroll));
            return;
        }
        let Some(d) = self.indev(indev) else {
            return;
        };
        let mut vector = d.pointer.vect;
        let direction = d.pointer.scroll_dir;
        self.indev_transform_vector(scroll, &mut vector);
        self.indev_mut(indev).unwrap().pointer.vect = vector;
        let mut diff = Point::default();
        if direction == Dir::HOR {
            // v9.6 reads right first; its GET_SELF_SIZE callback can change
            // the left extent, so the getter order is observable.
            let right = self.obj_get_scroll_right(scroll);
            if self.indev_reset_check(indev) || !self.obj_is_valid(scroll) {
                return;
            }
            let left = self.obj_get_scroll_left(scroll);
            if self.indev_reset_check(indev) || !self.obj_is_valid(scroll) {
                return;
            }
            diff.x = self.indev_elastic_diff(scroll, vector.x, left, right, true);
        } else {
            let top = self.obj_get_scroll_top(scroll);
            let bottom = self.obj_get_scroll_bottom(scroll);
            if self.indev_reset_check(indev) || !self.obj_is_valid(scroll) {
                return;
            }
            diff.y = self.indev_elastic_diff(scroll, vector.y, top, bottom, false);
        }
        let allowed = self.obj_get_scroll_dir(scroll);
        if (!allowed.contains(Dir::LEFT) && diff.x > 0) || (!allowed.contains(Dir::RIGHT) && diff.x < 0) {
            diff.x = 0;
        }
        if (!allowed.contains(Dir::TOP) && diff.y > 0) || (!allowed.contains(Dir::BOTTOM) && diff.y < 0) {
            diff.y = 0;
        }
        self.indev_limit_diff(indev, &mut diff);
        self.obj_scroll_by_raw(scroll, diff.x, diff.y);
        if self.indev_reset_check(indev) {
            return;
        }
        let d = self.indev_mut(indev).unwrap();
        d.pointer.scroll_sum.x += diff.x;
        d.pointer.scroll_sum.y += diff.y;
    }

    /// `lv_indev_scroll_throw_handler()`.
    pub fn indev_scroll_throw_handler(&mut self, indev: IndevId) {
        let Some(d) = self.indev(indev) else {
            return;
        };
        let Some(obj) = d.pointer.scroll_obj.filter(|&o| self.obj_is_valid(o)) else {
            return;
        };
        let direction = d.pointer.scroll_dir;
        if direction == Dir::NONE {
            return;
        }
        let decay = i32::from(d.scroll_throw);
        if !self.obj_has_flag(obj, ObjFlags::SCROLL_MOMENTUM) {
            self.indev_mut(indev).unwrap().pointer.scroll_throw_vect = Point::default();
        }
        let snap_x = self.obj_get_scroll_snap_x(obj);
        let snap_y = self.obj_get_scroll_snap_y(obj);
        let horizontal = direction == Dir::HOR;
        let snap = if horizontal { snap_x } else { snap_y };
        let d = self.indev_mut(indev).unwrap();
        if horizontal {
            d.pointer.scroll_throw_vect.y = 0;
        } else {
            d.pointer.scroll_throw_vect.x = 0;
        }
        if snap == ScrollSnap::None {
            let value = if horizontal {
                d.pointer.scroll_throw_vect.x
            } else {
                d.pointer.scroll_throw_vect.y
            };
            let value = value * (100 - decay) / 100;
            let (start, end) = if horizontal {
                let left = self.obj_get_scroll_left(obj);
                if self.indev_reset_check(indev) || !self.obj_is_valid(obj) {
                    return;
                }
                let right = self.obj_get_scroll_right(obj);
                (left, right)
            } else {
                // Throwing reads bottom before top, unlike dragging.
                let bottom = self.obj_get_scroll_bottom(obj);
                if self.indev_reset_check(indev) || !self.obj_is_valid(obj) {
                    return;
                }
                let top = self.obj_get_scroll_top(obj);
                (top, bottom)
            };
            if self.indev_reset_check(indev) || !self.obj_is_valid(obj) {
                return;
            }
            let value = self.indev_elastic_diff(obj, value, start, end, horizontal);
            let d = self.indev_mut(indev).unwrap();
            if horizontal {
                d.pointer.scroll_throw_vect.x = value;
                self.obj_scroll_by_raw(obj, value, 0);
            } else {
                d.pointer.scroll_throw_vect.y = value;
                self.obj_scroll_by_raw(obj, 0, value);
            }
            if self.indev_reset_check(indev) {
                return;
            }
        } else {
            let predicted = self.indev_scroll_throw_predict(indev, direction);
            let d = self.indev_mut(indev).unwrap();
            if horizontal {
                d.pointer.scroll_throw_vect.x = 0;
            } else {
                d.pointer.scroll_throw_vect.y = 0;
            }
            let mut diff = if horizontal {
                Point::new(predicted, 0)
            } else {
                Point::new(0, predicted)
            };
            self.indev_limit_diff(indev, &mut diff);
            let value = if horizontal { diff.x } else { diff.y };
            let snap = self.indev_find_snap_point(obj, coord::MIN, coord::MAX, value, horizontal);
            // No snappable children is an invalid C sentinel; do not animate
            // by LV_COORD_MAX when a callback removed the final snap child.
            let value = value + if snap == coord::MAX { 0 } else { snap };
            if horizontal {
                self.obj_scroll_by(obj, value, 0, true);
            } else {
                self.obj_scroll_by(obj, 0, value, true);
            }
            if self.indev_reset_check(indev) {
                return;
            }
        }
        if self
            .indev(indev)
            .is_some_and(|d| d.pointer.scroll_throw_vect == Point::default())
        {
            for (horizontal, snap) in [(false, snap_y), (true, snap_x)] {
                if snap != ScrollSnap::None {
                    continue;
                }
                let (start, end) = if horizontal {
                    let left = self.obj_get_scroll_left(obj);
                    if self.indev_reset_check(indev) || !self.obj_is_valid(obj) {
                        return;
                    }
                    let right = self.obj_get_scroll_right(obj);
                    (left, right)
                } else {
                    let top = self.obj_get_scroll_top(obj);
                    let bottom = self.obj_get_scroll_bottom(obj);
                    (top, bottom)
                };
                if self.indev_reset_check(indev) || !self.obj_is_valid(obj) {
                    return;
                }
                if start > 0 || end > 0 {
                    let value = if start < 0 {
                        start
                    } else if end < 0 {
                        -end
                    } else {
                        0
                    };
                    if value != 0 {
                        if horizontal {
                            self.obj_scroll_by(obj, value, 0, true);
                        } else {
                            self.obj_scroll_by(obj, 0, value, true);
                        }
                        if self.indev_reset_check(indev) {
                            return;
                        }
                    }
                }
            }
            self.obj_send_event(obj, EventCode::SCROLL_END, EventParam::Indev(indev));
            if self.indev_reset_check(indev) {
                return;
            }
            let d = self.indev_mut(indev).unwrap();
            d.pointer.scroll_dir = Dir::NONE;
            d.pointer.scroll_obj = None;
        }
    }

    /// `lv_indev_scroll_throw_predict()`. A zero or nonconvergent slowdown
    /// returns zero instead of entering C's unbounded prediction loop.
    #[must_use]
    pub fn indev_scroll_throw_predict(&mut self, indev: IndevId, dir: Dir) -> i32 {
        let Some(d) = self.indev(indev) else {
            return 0;
        };
        let mut value = match dir {
            Dir::HOR => d.pointer.scroll_throw_vect_ori.x,
            Dir::VER => d.pointer.scroll_throw_vect_ori.y,
            _ => return 0,
        };
        let decay = i32::from(d.scroll_throw);
        if decay == 0 || decay >= 200 {
            return 0;
        }
        let mut sum: i32 = 0;
        while value != 0 {
            sum = sum.wrapping_add(value);
            value = value.wrapping_mul(100 - decay) / 100;
        }
        sum
    }

    /// `lv_indev_scroll_get_snap_dist()`.
    #[must_use]
    pub fn indev_scroll_get_snap_dist(&mut self, obj: Obj) -> Point {
        let Some(d) = self.obj(obj) else {
            return Point::default();
        };
        let coords = d.coords;
        Point::new(
            self.indev_find_snap_point(obj, coords.x1, coords.x2, 0, true),
            self.indev_find_snap_point(obj, coords.y1, coords.y2, 0, false),
        )
    }

    pub(super) fn indev_transform_vector(&self, obj: Obj, vector: &mut Point) {
        self.indev_transform_vector_inner(obj, vector, true, false);
    }

    pub(super) fn indev_transform_release_vector(&self, obj: Obj, vector: &mut Point) {
        // lv_indev.c's release path derives Y from the just-updated X scale,
        // unlike scroll.c's drag/search paths. Preserve this upstream quirk.
        self.indev_transform_vector_inner(obj, vector, true, true);
    }

    fn indev_transform_vector_inner(&self, obj: Obj, vector: &mut Point, short_scale: bool, release: bool) {
        let mut angle: i16 = 0;
        let (mut x, mut y): (i32, i32) = (256, 256);
        let mut parent = Some(obj);
        while let Some(obj) = parent {
            angle = angle.wrapping_add(self.obj_get_style_transform_rotation(obj, Part::MAIN) as i16);
            x = x.wrapping_mul(self.obj_get_style_transform_scale_x_safe(obj, Part::MAIN)) >> 8;
            if short_scale {
                x = i32::from(x as i16);
            }
            let y_base = if release { x } else { y };
            y = y_base.wrapping_mul(self.obj_get_style_transform_scale_y_safe(obj, Part::MAIN)) >> 8;
            if short_scale {
                y = i32::from(y as i16);
            }
            parent = self.obj_get_parent(obj);
        }
        if x == 0 {
            x = 1;
        }
        if y == 0 {
            y = 1;
        }
        if angle != 0 || x != 256 || y != 256 {
            angle = angle.wrapping_neg();
            x = 65536 / x;
            y = 65536 / y;
            if short_scale {
                x = i32::from(x as i16);
                y = i32::from(y as i16);
            }
            vector.transform(i32::from(angle), x, y, Point::default(), false);
        }
    }

    fn indev_init_scroll_limits(&mut self, indev: IndevId) {
        let Some(obj) = self.indev(indev).and_then(|d| d.pointer.scroll_obj) else {
            return;
        };
        let mut area = Area::new(coord::MIN, coord::MIN, coord::MAX, coord::MAX);
        if self.obj_has_flag(obj, ObjFlags::SCROLL_ONE) {
            let coords = self.obj_get_coords(obj);
            for horizontal in [false, true] {
                let snap = if horizontal {
                    self.obj_get_scroll_snap_x(obj)
                } else {
                    self.obj_get_scroll_snap_y(obj)
                };
                let (begin, end) = if horizontal {
                    (coords.x1, coords.x2)
                } else {
                    (coords.y1, coords.y2)
                };
                let (min, max) = match snap {
                    ScrollSnap::None => continue,
                    ScrollSnap::Start => {
                        if horizontal {
                            (begin, begin)
                        } else {
                            (begin + 1, begin - 1)
                        }
                    }
                    ScrollSnap::End => (end, end),
                    ScrollSnap::Center => {
                        let mid = begin + (end - begin + 1) / 2;
                        (mid + 1, mid - 1)
                    }
                };
                let low = self.indev_find_snap_point(obj, min, coord::MAX, 0, horizontal);
                let high = self.indev_find_snap_point(obj, coord::MIN, max, 0, horizontal);
                if horizontal {
                    area.x1 = low;
                    area.x2 = high;
                } else {
                    area.y1 = low;
                    area.y2 = high;
                }
            }
        }
        if area.x1 == coord::MAX || area.x1 == 0 {
            area.x1 = coord::MIN;
        }
        if area.y1 == coord::MAX || area.y1 == 0 {
            area.y1 = coord::MIN;
        }
        if area.x2 == 0 {
            area.x2 = coord::MAX;
        }
        if area.y2 == 0 {
            area.y2 = coord::MAX;
        }
        if let Some(d) = self.indev_mut(indev) {
            d.pointer.scroll_area = area;
        }
    }

    fn indev_find_snap_point(&self, obj: Obj, min: i32, max: i32, offset: i32, horizontal: bool) -> i32 {
        let snap = if horizontal {
            self.obj_get_scroll_snap_x(obj)
        } else {
            self.obj_get_scroll_snap_y(obj)
        };
        if snap == ScrollSnap::None {
            return coord::MAX;
        }
        let coords = self.obj_get_coords(obj);
        let (start, end, pad_start, pad_end) = if horizontal {
            (
                coords.x1,
                coords.x2,
                self.obj_get_style_pad_left(obj, Part::MAIN),
                self.obj_get_style_pad_right(obj, Part::MAIN),
            )
        } else {
            (
                coords.y1,
                coords.y2,
                self.obj_get_style_pad_top(obj, Part::MAIN),
                self.obj_get_style_pad_bottom(obj, Part::MAIN),
            )
        };
        let mut distance = coord::MAX;
        for index in 0..self.obj_get_child_count(obj) {
            let Some(child) = self.obj_get_child(obj, index as i32) else {
                continue;
            };
            if self.obj_has_flag_any(child, ObjFlags::HIDDEN | ObjFlags::FLOATING)
                || !self.obj_has_flag(child, ObjFlags::SNAPPABLE)
            {
                continue;
            }
            let child = self.obj_get_coords(child);
            let (child_start, child_end) = if horizontal {
                (child.x1, child.x2)
            } else {
                (child.y1, child.y2)
            };
            let (child_point, parent_point) = match snap {
                ScrollSnap::Start => (child_start, start + pad_start),
                ScrollSnap::End => (child_end, end - pad_end),
                ScrollSnap::Center => (
                    child_start + (child_end - child_start + 1) / 2,
                    start + pad_start + (end - start + 1 - pad_start - pad_end) / 2,
                ),
                ScrollSnap::None => continue,
            };
            let child_point = child_point + offset;
            if child_point >= min && child_point <= max {
                let value = child_point - parent_point;
                if value.abs() < distance.abs() {
                    distance = value;
                }
            }
        }
        if distance == coord::MAX {
            distance
        } else {
            -distance
        }
    }

    fn indev_limit_diff(&self, indev: IndevId, diff: &mut Point) {
        let Some(d) = self.indev(indev) else {
            return;
        };
        let area = d.pointer.scroll_area;
        let sum = d.pointer.scroll_sum;
        if sum.x + diff.x < area.x1 {
            diff.x = area.x1 - sum.x;
        }
        if sum.x + diff.x > area.x2 {
            diff.x = area.x2 - sum.x;
        }
        if sum.y + diff.y < area.y1 {
            diff.y = area.y1 - sum.y;
        }
        if sum.y + diff.y > area.y2 {
            diff.y = area.y2 - sum.y;
        }
    }

    fn indev_elastic_diff(&self, obj: Obj, mut diff: i32, start: i32, end: i32, horizontal: bool) -> i32 {
        if diff == 0 {
            return 0;
        }
        if !self.obj_has_flag(obj, ObjFlags::SCROLL_ELASTIC) {
            let edge = if diff > 0 { start } else { end };
            if edge <= 0 {
                diff = 0;
            } else if edge - diff < 0 {
                diff = edge;
            }
            return diff;
        }
        let snap = if horizontal {
            self.obj_get_scroll_snap_x(obj)
        } else {
            self.obj_get_scroll_snap_y(obj)
        };
        let outside = if snap == ScrollSnap::None {
            start < 0 || end < 0
        } else {
            let (start, end) = self.indev_more_snap_points(obj, horizontal);
            !start || !end
        };
        if outside {
            if diff < 0 {
                diff -= ELASTIC_SLOWNESS_FACTOR / 2;
            }
            if diff > 0 {
                diff += ELASTIC_SLOWNESS_FACTOR / 2;
            }
            diff / ELASTIC_SLOWNESS_FACTOR
        } else {
            diff
        }
    }

    fn indev_more_snap_points(&self, obj: Obj, horizontal: bool) -> (bool, bool) {
        let snap = if horizontal {
            self.obj_get_scroll_snap_x(obj)
        } else {
            self.obj_get_scroll_snap_y(obj)
        };
        let coords = self.obj_get_coords(obj);
        let (start, end, pad_start, pad_end) = if horizontal {
            (
                coords.x1,
                coords.x2,
                self.obj_get_style_pad_left(obj, Part::MAIN),
                self.obj_get_style_pad_right(obj, Part::MAIN),
            )
        } else {
            (
                coords.y1,
                coords.y2,
                self.obj_get_style_pad_top(obj, Part::MAIN),
                self.obj_get_style_pad_bottom(obj, Part::MAIN),
            )
        };
        let point = match snap {
            ScrollSnap::Center => start + (end - start + 1 - pad_start - pad_end) / 2 + pad_start,
            ScrollSnap::Start => start + pad_start,
            ScrollSnap::End => end - pad_end,
            ScrollSnap::None => 0,
        };
        (
            self.indev_find_snap_point(obj, coord::MIN, point - 1, 0, horizontal) != coord::MAX,
            self.indev_find_snap_point(obj, point + 1, coord::MAX, 0, horizontal) != coord::MAX,
        )
    }
}
