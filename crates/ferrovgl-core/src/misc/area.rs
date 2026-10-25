//! Points, areas and alignment.
//!
//! Ported from lvgl v9.6.0: `src/misc/lv_area.c`, `include/lvgl/core/lv_area.h`.
//!
//! An [`Area`] is inclusive on both ends (`x2` is the last pixel column), so
//! its width is `x2 - x1 + 1`, exactly like `lv_area_t`.

use super::math;
use super::types::ValuePrecise;

/// `lv_point_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    /// X coordinate.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
}

impl Point {
    /// `lv_point_set()`
    #[inline]
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// `lv_point_to_precise()`
    #[inline]
    #[must_use]
    pub fn to_precise(self) -> PointPrecise {
        PointPrecise {
            x: self.x as ValuePrecise,
            y: self.y as ValuePrecise,
        }
    }

    /// `lv_point_transform()`: rotate by `angle` (0.1 degree units) and scale
    /// (`256` = 1.0) around `pivot`.
    pub fn transform(&mut self, angle: i32, scale_x: i32, scale_y: i32, pivot: Point, zoom_first: bool) {
        transform_points(
            core::slice::from_mut(self),
            angle,
            scale_x,
            scale_y,
            pivot,
            zoom_first,
        );
    }
}

/// `lv_point_precise_t`
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct PointPrecise {
    /// X coordinate.
    pub x: ValuePrecise,
    /// Y coordinate.
    pub y: ValuePrecise,
}

impl PointPrecise {
    /// `lv_point_precise_set()`
    #[inline]
    #[must_use]
    pub const fn new(x: ValuePrecise, y: ValuePrecise) -> Self {
        Self { x, y }
    }

    /// `lv_point_from_precise()`: truncate towards zero like a C cast.
    #[inline]
    #[allow(clippy::unnecessary_cast)] // `ValuePrecise` is `i32` without the `float` feature
    #[must_use]
    pub fn to_point(self) -> Point {
        Point {
            x: self.x as i32,
            y: self.y as i32,
        }
    }
}

/// `lv_area_t`: an inclusive rectangle.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Area {
    /// Left column.
    pub x1: i32,
    /// Top row.
    pub y1: i32,
    /// Right column (inclusive).
    pub x2: i32,
    /// Bottom row (inclusive).
    pub y2: i32,
}

/// `lv_align_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum Align {
    /// `LV_ALIGN_DEFAULT`
    #[default]
    Default = 0,
    /// `LV_ALIGN_TOP_LEFT`
    TopLeft,
    /// `LV_ALIGN_TOP_MID`
    TopMid,
    /// `LV_ALIGN_TOP_RIGHT`
    TopRight,
    /// `LV_ALIGN_BOTTOM_LEFT`
    BottomLeft,
    /// `LV_ALIGN_BOTTOM_MID`
    BottomMid,
    /// `LV_ALIGN_BOTTOM_RIGHT`
    BottomRight,
    /// `LV_ALIGN_LEFT_MID`
    LeftMid,
    /// `LV_ALIGN_RIGHT_MID`
    RightMid,
    /// `LV_ALIGN_CENTER`
    Center,
    /// `LV_ALIGN_OUT_TOP_LEFT`
    OutTopLeft,
    /// `LV_ALIGN_OUT_TOP_MID`
    OutTopMid,
    /// `LV_ALIGN_OUT_TOP_RIGHT`
    OutTopRight,
    /// `LV_ALIGN_OUT_BOTTOM_LEFT`
    OutBottomLeft,
    /// `LV_ALIGN_OUT_BOTTOM_MID`
    OutBottomMid,
    /// `LV_ALIGN_OUT_BOTTOM_RIGHT`
    OutBottomRight,
    /// `LV_ALIGN_OUT_LEFT_TOP`
    OutLeftTop,
    /// `LV_ALIGN_OUT_LEFT_MID`
    OutLeftMid,
    /// `LV_ALIGN_OUT_LEFT_BOTTOM`
    OutLeftBottom,
    /// `LV_ALIGN_OUT_RIGHT_TOP`
    OutRightTop,
    /// `LV_ALIGN_OUT_RIGHT_MID`
    OutRightMid,
    /// `LV_ALIGN_OUT_RIGHT_BOTTOM`
    OutRightBottom,
}

bitflags::bitflags! {
    /// `lv_dir_t`
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Dir: u8 {
        /// `LV_DIR_LEFT`
        const LEFT = 1 << 0;
        /// `LV_DIR_RIGHT`
        const RIGHT = 1 << 1;
        /// `LV_DIR_TOP`
        const TOP = 1 << 2;
        /// `LV_DIR_BOTTOM`
        const BOTTOM = 1 << 3;
        /// `LV_DIR_HOR`
        const HOR = Self::LEFT.bits() | Self::RIGHT.bits();
        /// `LV_DIR_VER`
        const VER = Self::TOP.bits() | Self::BOTTOM.bits();
        /// `LV_DIR_ALL`
        const ALL = Self::HOR.bits() | Self::VER.bits();
    }
}

impl Dir {
    /// `LV_DIR_NONE`
    pub const NONE: Dir = Dir::empty();
}

impl Area {
    /// `lv_area_set()`
    #[inline]
    #[must_use]
    pub const fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// Area from a position and a size (`x2 = x + w - 1`).
    #[inline]
    #[must_use]
    pub const fn from_size(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            x1: x,
            y1: y,
            x2: x + w - 1,
            y2: y + h - 1,
        }
    }

    /// `lv_area_set()`
    #[inline]
    pub fn set(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) {
        *self = Self { x1, y1, x2, y2 };
    }

    /// `lv_area_get_width()`
    #[inline]
    #[must_use]
    pub const fn width(&self) -> i32 {
        self.x2 - self.x1 + 1
    }

    /// `lv_area_get_height()`
    #[inline]
    #[must_use]
    pub const fn height(&self) -> i32 {
        self.y2 - self.y1 + 1
    }

    /// `lv_area_set_width()`
    #[inline]
    pub fn set_width(&mut self, w: i32) {
        self.x2 = self.x1 + w - 1;
    }

    /// `lv_area_set_height()`
    #[inline]
    pub fn set_height(&mut self, h: i32) {
        self.y2 = self.y1 + h - 1;
    }

    /// `lv_area_set_pos()`: move the top-left corner keeping the size.
    pub fn set_pos(&mut self, x: i32, y: i32) {
        let w = self.width();
        let h = self.height();
        self.x1 = x;
        self.y1 = y;
        self.set_width(w);
        self.set_height(h);
    }

    /// `lv_area_get_size()`: number of pixels.
    #[inline]
    #[must_use]
    pub const fn size(&self) -> u32 {
        ((self.x2 - self.x1 + 1) as u32).wrapping_mul((self.y2 - self.y1 + 1) as u32)
    }

    /// `lv_area_increase()`: grow by `w_extra` on the left and right and
    /// `h_extra` on the top and bottom.
    #[inline]
    pub fn increase(&mut self, w_extra: i32, h_extra: i32) {
        self.x1 -= w_extra;
        self.x2 += w_extra;
        self.y1 -= h_extra;
        self.y2 += h_extra;
    }

    /// `lv_area_move()`
    #[inline]
    pub fn move_by(&mut self, x_ofs: i32, y_ofs: i32) {
        self.x1 += x_ofs;
        self.x2 += x_ofs;
        self.y1 += y_ofs;
        self.y2 += y_ofs;
    }

    /// `lv_area_intersect()`: the common part of two areas, or `None` if they
    /// do not overlap.
    #[must_use]
    pub fn intersect(a1: &Area, a2: &Area) -> Option<Area> {
        let res = Area {
            x1: a1.x1.max(a2.x1),
            y1: a1.y1.max(a2.y1),
            x2: a1.x2.min(a2.x2),
            y2: a1.y2.min(a2.y2),
        };
        if res.x1 > res.x2 || res.y1 > res.y2 {
            None
        } else {
            Some(res)
        }
    }

    /// `lv_area_join()`: the smallest area containing both.
    #[must_use]
    pub fn join(a1: &Area, a2: &Area) -> Area {
        Area {
            x1: a1.x1.min(a2.x1),
            y1: a1.y1.min(a2.y1),
            x2: a1.x2.max(a2.x2),
            y2: a1.y2.max(a2.y2),
        }
    }

    /// `lv_area_diff()`: the parts of `self` not covered by `remove`, as up to
    /// four areas. Returns `None` when the areas do not touch at all
    /// (upstream returns -1) and `Some(0)` when `self` is fully covered.
    #[must_use]
    pub fn diff(&self, remove: &Area, res: &mut [Area; 4]) -> Option<usize> {
        if !self.is_on(remove) {
            return None;
        }
        if self.is_in(remove, 0) {
            return Some(0);
        }
        let mut res_c = 0usize;
        let a1_w = self.width() - 1;
        let a1_h = self.height() - 1;

        // Top rectangle
        let th = remove.y1 - self.y1;
        if th > 0 {
            res[res_c] = Area {
                x1: self.x1,
                y1: self.y1,
                x2: self.x2,
                y2: self.y1 + th - 1,
            };
            res_c += 1;
        }
        // Bottom rectangle
        let bh = a1_h - (remove.y2 - self.y1);
        if bh > 0 && remove.y2 < self.y2 {
            res[res_c] = Area {
                x1: self.x1,
                y1: remove.y2 + 1,
                x2: self.x2,
                y2: remove.y2 + bh,
            };
            res_c += 1;
        }
        // Side height
        let y1 = if remove.y1 > self.y1 { remove.y1 } else { self.y1 };
        let y2 = if remove.y2 < self.y2 { remove.y2 } else { self.y2 };
        let sh = y2 - y1;
        // Left rectangle
        let lw = remove.x1 - self.x1;
        if lw > 0 && sh >= 0 {
            res[res_c] = Area {
                x1: self.x1,
                y1,
                x2: self.x1 + lw - 1,
                y2: y1 + sh,
            };
            res_c += 1;
        }
        // Right rectangle
        let rw = a1_w - (remove.x2 - self.x1);
        if rw > 0 && sh >= 0 {
            res[res_c] = Area {
                x1: remove.x2 + 1,
                y1,
                x2: remove.x2 + rw,
                y2: y1 + sh,
            };
            res_c += 1;
        }
        Some(res_c)
    }

    /// `lv_area_is_point_on()`: is `p` inside, taking a corner `radius` into account.
    #[must_use]
    pub fn is_point_on(&self, p: Point, radius: i32) -> bool {
        let is_on_rect = p.x >= self.x1 && p.x <= self.x2 && p.y >= self.y1 && p.y <= self.y2;
        if !is_on_rect {
            return false;
        }
        if radius <= 0 {
            return true;
        }
        let w = self.width() / 2;
        let h = self.height() / 2;
        let max_radius = w.min(h);
        let radius = radius.min(max_radius);

        // Top left
        let mut corner = Area {
            x1: self.x1,
            x2: self.x1 + radius,
            y1: self.y1,
            y2: self.y1 + radius,
        };
        if corner.is_point_on(p, 0) {
            corner.x2 += radius;
            corner.y2 += radius;
            return point_within_circle(&corner, p);
        }
        // Bottom left
        corner.y1 = self.y2 - radius;
        corner.y2 = self.y2;
        if corner.is_point_on(p, 0) {
            corner.x2 += radius;
            corner.y1 -= radius;
            return point_within_circle(&corner, p);
        }
        // Bottom right
        corner.x1 = self.x2 - radius;
        corner.x2 = self.x2;
        if corner.is_point_on(p, 0) {
            corner.x1 -= radius;
            corner.y1 -= radius;
            return point_within_circle(&corner, p);
        }
        // Top right
        corner.y1 = self.y1;
        corner.y2 = self.y1 + radius;
        if corner.is_point_on(p, 0) {
            corner.x1 -= radius;
            corner.y2 += radius;
            return point_within_circle(&corner, p);
        }
        true
    }

    /// `lv_area_is_on()`: do the two areas overlap or touch?
    #[inline]
    #[must_use]
    pub const fn is_on(&self, other: &Area) -> bool {
        self.x1 <= other.x2 && self.x2 >= other.x1 && self.y1 <= other.y2 && self.y2 >= other.y1
    }

    /// `lv_area_is_in()`: is `self` fully inside `holder` (whose corners may be rounded)?
    #[must_use]
    pub fn is_in(&self, holder: &Area, radius: i32) -> bool {
        let is_in =
            self.x1 >= holder.x1 && self.y1 >= holder.y1 && self.x2 <= holder.x2 && self.y2 <= holder.y2;
        if !is_in {
            return false;
        }
        if radius == 0 {
            return true;
        }
        holder.is_point_on(Point::new(self.x1, self.y1), radius)
            && holder.is_point_on(Point::new(self.x2, self.y1), radius)
            && holder.is_point_on(Point::new(self.x1, self.y2), radius)
            && holder.is_point_on(Point::new(self.x2, self.y2), radius)
    }

    /// `lv_area_is_out()`: is `self` fully outside `holder` (whose corners may be rounded)?
    #[must_use]
    pub fn is_out(&self, holder: &Area, radius: i32) -> bool {
        if self.x2 < holder.x1 || self.y2 < holder.y1 || self.x1 > holder.x2 || self.y1 > holder.y2 {
            return true;
        }
        if radius == 0 {
            return false;
        }
        !(holder.is_point_on(Point::new(self.x1, self.y1), radius)
            || holder.is_point_on(Point::new(self.x2, self.y1), radius)
            || holder.is_point_on(Point::new(self.x1, self.y2), radius)
            || holder.is_point_on(Point::new(self.x2, self.y2), radius))
    }

    /// `lv_area_is_equal()`
    #[inline]
    #[must_use]
    pub const fn is_equal(&self, other: &Area) -> bool {
        self.x1 == other.x1 && self.x2 == other.x2 && self.y1 == other.y1 && self.y2 == other.y2
    }

    /// `lv_area_align()`: position `to_align` relative to `base` and offset it.
    pub fn align(base: &Area, to_align: &mut Area, align: Align, ofs_x: i32, ofs_y: i32) {
        let bw = base.width();
        let bh = base.height();
        let w = to_align.width();
        let h = to_align.height();
        let (x, y) = match align {
            Align::Center => (bw / 2 - w / 2, bh / 2 - h / 2),
            Align::TopLeft => (0, 0),
            Align::TopMid => (bw / 2 - w / 2, 0),
            Align::TopRight => (bw - w, 0),
            Align::BottomLeft => (0, bh - h),
            Align::BottomMid => (bw / 2 - w / 2, bh - h),
            Align::BottomRight => (bw - w, bh - h),
            Align::LeftMid => (0, bh / 2 - h / 2),
            Align::RightMid => (bw - w, bh / 2 - h / 2),
            Align::OutTopLeft => (0, -h),
            Align::OutTopMid => (bw / 2 - w / 2, -h),
            Align::OutTopRight => (bw - w, -h),
            Align::OutBottomLeft => (0, bh),
            Align::OutBottomMid => (bw / 2 - w / 2, bh),
            Align::OutBottomRight => (bw - w, bh),
            Align::OutLeftTop => (-w, 0),
            Align::OutLeftMid => (-w, bh / 2 - h / 2),
            Align::OutLeftBottom => (-w, bh - h),
            Align::OutRightTop => (bw, 0),
            Align::OutRightMid => (bw, bh / 2 - h / 2),
            Align::OutRightBottom => (bw, bh - h),
            _ => (0, 0),
        };
        let x = x + base.x1;
        let y = y + base.y1;
        to_align.x1 = x + ofs_x;
        to_align.y1 = y + ofs_y;
        to_align.x2 = to_align.x1 + w - 1;
        to_align.y2 = to_align.y1 + h - 1;
    }
}

fn point_within_circle(area: &Area, p: Point) -> bool {
    let r = (area.x2 - area.x1) / 2;
    let cx = area.x1 + r;
    let cy = area.y1 + r;
    let px = p.x - cx;
    let py = p.y - cy;
    let r_sqrd = (r * r) as u32;
    let dist = (px * px + py * py) as u32;
    dist <= r_sqrd
}

const TRANSFORM_TRIGO_SHIFT: u32 = 10;

/// `lv_point_array_transform()`: rotate by `angle` (0.1 degree units) and
/// scale (`256` = 1.0) every point around `pivot`.
pub fn transform_points(
    points: &mut [Point],
    angle: i32,
    scale_x: i32,
    scale_y: i32,
    pivot: Point,
    zoom_first: bool,
) {
    if points.is_empty() {
        crate::misc::check_arg_failed("transform_points: empty slice");
        return;
    }
    if angle == 0 && scale_x == 256 && scale_y == 256 {
        return;
    }
    for p in points.iter_mut() {
        p.x -= pivot.x;
        p.y -= pivot.y;
    }
    if angle == 0 {
        for p in points.iter_mut() {
            p.x = ((p.x * scale_x) >> 8) + pivot.x;
            p.y = ((p.y * scale_y) >> 8) + pivot.y;
        }
        return;
    }
    let mut angle_limited = angle;
    if angle_limited > 3600 {
        angle_limited -= 3600;
    }
    if angle_limited < 0 {
        angle_limited += 3600;
    }
    let angle_low = angle_limited / 10;
    let angle_high = angle_low + 1;
    let angle_rem = angle_limited - (angle_low * 10);

    let s1 = math::trigo_sin(angle_low as i16);
    let s2 = math::trigo_sin(angle_high as i16);
    let c1 = math::trigo_sin((angle_low + 90) as i16);
    let c2 = math::trigo_sin((angle_high + 90) as i16);

    let mut sinma = (s1 * (10 - angle_rem) + s2 * angle_rem) / 10;
    sinma >>= math::TRIGO_SHIFT - TRANSFORM_TRIGO_SHIFT;
    let mut cosma = (c1 * (10 - angle_rem) + c2 * angle_rem) / 10;
    cosma >>= math::TRIGO_SHIFT - TRANSFORM_TRIGO_SHIFT;

    for p in points.iter_mut() {
        let mut x = p.x;
        let mut y = p.y;
        if scale_x == 256 && scale_y == 256 {
            p.x = ((cosma * x - sinma * y) >> TRANSFORM_TRIGO_SHIFT) + pivot.x;
            p.y = ((sinma * x + cosma * y) >> TRANSFORM_TRIGO_SHIFT) + pivot.y;
        } else if zoom_first {
            x = x.wrapping_mul(scale_x);
            y = y.wrapping_mul(scale_y);
            p.x = ((cosma.wrapping_mul(x).wrapping_sub(sinma.wrapping_mul(y)))
                >> (TRANSFORM_TRIGO_SHIFT + 8))
                + pivot.x;
            p.y = ((sinma.wrapping_mul(x).wrapping_add(cosma.wrapping_mul(y)))
                >> (TRANSFORM_TRIGO_SHIFT + 8))
                + pivot.y;
        } else {
            p.x = ((cosma * x - sinma * y).wrapping_mul(scale_x) >> (TRANSFORM_TRIGO_SHIFT + 8)) + pivot.x;
            p.y = ((sinma * x + cosma * y).wrapping_mul(scale_y) >> (TRANSFORM_TRIGO_SHIFT + 8)) + pivot.y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Port of upstream `test_area.c: test_area_diff`.
    #[test]
    fn diff_splits_into_four() {
        let expected = [
            Area::new(0, 0, 100, 29),
            Area::new(0, 91, 100, 100),
            Area::new(0, 30, 39, 90),
            Area::new(81, 30, 100, 90),
        ];
        let outer = Area::new(0, 0, 100, 100);
        let mut remove = Area::new(40, 30, 80, 90);
        let mut actual = [Area::default(); 4];
        assert_eq!(outer.diff(&remove, &mut actual), Some(4));
        assert_eq!(actual, expected);

        // no left edge
        remove.set(0, 10, 10, 20);
        assert_eq!(outer.diff(&remove, &mut actual), Some(3));
        remove.set(1, 10, 10, 20);
        assert_eq!(outer.diff(&remove, &mut actual), Some(4));
        // no right edge
        remove.set(90, 10, 100, 20);
        assert_eq!(outer.diff(&remove, &mut actual), Some(3));
        remove.set(90, 10, 99, 20);
        assert_eq!(outer.diff(&remove, &mut actual), Some(4));
        // no top edge
        remove.set(10, 0, 20, 20);
        assert_eq!(outer.diff(&remove, &mut actual), Some(3));
        remove.set(10, 1, 20, 20);
        assert_eq!(outer.diff(&remove, &mut actual), Some(4));
        // no bottom edge
        remove.set(10, 90, 20, 100);
        assert_eq!(outer.diff(&remove, &mut actual), Some(3));
        remove.set(10, 90, 20, 99);
        assert_eq!(outer.diff(&remove, &mut actual), Some(4));
        // disjoint and fully covered
        remove.set(200, 200, 300, 300);
        assert_eq!(outer.diff(&remove, &mut actual), None);
        remove.set(-10, -10, 200, 200);
        assert_eq!(outer.diff(&remove, &mut actual), Some(0));
    }

    #[test]
    fn intersect_join_and_sizes() {
        let a = Area::new(0, 0, 9, 9);
        let b = Area::new(5, 5, 14, 14);
        assert_eq!(a.width(), 10);
        assert_eq!(a.size(), 100);
        assert_eq!(Area::intersect(&a, &b), Some(Area::new(5, 5, 9, 9)));
        assert_eq!(Area::intersect(&a, &Area::new(20, 20, 30, 30)), None);
        assert_eq!(Area::join(&a, &b), Area::new(0, 0, 14, 14));
        let mut c = a;
        c.increase(1, 2);
        assert_eq!(c, Area::new(-1, -2, 10, 11));
        c.move_by(1, 2);
        assert_eq!(c, Area::new(0, 0, 11, 13));
        c.set_pos(100, 100);
        assert_eq!(c, Area::new(100, 100, 111, 113));
    }

    #[test]
    fn rounded_corner_hit_test() {
        let a = Area::new(0, 0, 99, 99);
        assert!(a.is_point_on(Point::new(0, 0), 0));
        assert!(
            !a.is_point_on(Point::new(0, 0), 20),
            "corner pixel is outside the rounded corner"
        );
        assert!(a.is_point_on(Point::new(50, 0), 20));
        assert!(!a.is_point_on(Point::new(5, 5), 20));
        assert!(a.is_point_on(Point::new(15, 15), 20));
        assert!(!a.is_point_on(Point::new(100, 50), 0));
        assert!(Area::new(10, 10, 20, 20).is_in(&a, 0));
        assert!(!Area::new(0, 0, 20, 20).is_in(&a, 20));
        assert!(Area::new(200, 200, 210, 210).is_out(&a, 0));
        assert!(
            Area::new(0, 0, 3, 3).is_out(&a, 30),
            "tiny area tucked in the rounded corner is outside"
        );
    }

    #[test]
    fn align_center_and_out() {
        let base = Area::new(10, 10, 109, 59); // 100x50
        let mut t = Area::from_size(0, 0, 20, 10);
        Area::align(&base, &mut t, Align::Center, 0, 0);
        assert_eq!(t, Area::new(50, 30, 69, 39));
        Area::align(&base, &mut t, Align::OutBottomRight, 1, 2);
        assert_eq!(t, Area::new(91, 62, 110, 71));
        Area::align(&base, &mut t, Align::OutLeftMid, 0, 0);
        assert_eq!(t, Area::new(-10, 30, 9, 39));
    }

    #[test]
    fn transform_rotation_and_scale() {
        let pivot = Point::new(0, 0);
        let mut p = Point::new(100, 0);
        p.transform(900, 256, 256, pivot, false);
        assert_eq!((p.x, p.y), (0, 100));
        let mut p = Point::new(100, 0);
        p.transform(0, 512, 512, pivot, false);
        assert_eq!((p.x, p.y), (200, 0));
        let mut p = Point::new(100, 50);
        p.transform(1800, 256, 256, Point::new(50, 50), false);
        assert_eq!((p.x, p.y), (0, 50));
        let mut p = Point::new(100, 0);
        p.transform(450, 128, 128, pivot, true);
        assert!((p.x - 35).abs() <= 1 && (p.y - 35).abs() <= 1, "{p:?}");
    }
}
