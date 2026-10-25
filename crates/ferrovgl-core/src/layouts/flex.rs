//! Ported from lvgl v9.6.0: `include/lvgl/layouts/lv_flex.h`, `src/layouts/flex/lv_flex.c`.

use alloc::vec::Vec;

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::layouts::LAYOUT_FLEX;
use crate::misc::check_arg_failed;
use crate::misc::event::{EventCode, EventParam};
use crate::misc::style::{Part, StyleProp, StyleSelector};
use crate::misc::text::BaseDir;
use crate::misc::types::coord;

/// `LV_FLEX_COLUMN`
pub const FLEX_COLUMN: u8 = 1 << 0;
/// `LV_FLEX_WRAP`
pub const FLEX_WRAP: u8 = 1 << 2;
/// `LV_FLEX_REVERSE`
pub const FLEX_REVERSE: u8 = 1 << 3;

/// `lv_flex_align_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FlexAlign {
    /// `LV_FLEX_ALIGN_START`
    #[default]
    Start = 0,
    /// `LV_FLEX_ALIGN_END`
    End = 1,
    /// `LV_FLEX_ALIGN_CENTER`
    Center = 2,
    /// `LV_FLEX_ALIGN_SPACE_EVENLY`
    SpaceEvenly = 3,
    /// `LV_FLEX_ALIGN_SPACE_AROUND`
    SpaceAround = 4,
    /// `LV_FLEX_ALIGN_SPACE_BETWEEN`
    SpaceBetween = 5,
}

impl FlexAlign {
    /// Convert a raw style number (`Start` if unknown).
    pub const fn from_u8(v: u8) -> FlexAlign {
        match v {
            1 => FlexAlign::End,
            2 => FlexAlign::Center,
            3 => FlexAlign::SpaceEvenly,
            4 => FlexAlign::SpaceAround,
            5 => FlexAlign::SpaceBetween,
            _ => FlexAlign::Start,
        }
    }
}

/// `lv_flex_flow_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FlexFlow {
    /// `LV_FLEX_FLOW_ROW`
    #[default]
    Row = 0x00,
    /// `LV_FLEX_FLOW_COLUMN`
    Column = 0x01,
    /// `LV_FLEX_FLOW_ROW_WRAP`
    RowWrap = 0x04,
    /// `LV_FLEX_FLOW_ROW_REVERSE`
    RowReverse = 0x08,
    /// `LV_FLEX_FLOW_ROW_WRAP_REVERSE`
    RowWrapReverse = 0x0C,
    /// `LV_FLEX_FLOW_COLUMN_WRAP`
    ColumnWrap = 0x05,
    /// `LV_FLEX_FLOW_COLUMN_REVERSE`
    ColumnReverse = 0x09,
    /// `LV_FLEX_FLOW_COLUMN_WRAP_REVERSE`
    ColumnWrapReverse = 0x0D,
}

impl FlexFlow {
    /// Convert a raw style number (`Row` if unknown).
    pub const fn from_u8(v: u8) -> FlexFlow {
        match v {
            0x01 => FlexFlow::Column,
            0x04 => FlexFlow::RowWrap,
            0x08 => FlexFlow::RowReverse,
            0x0C => FlexFlow::RowWrapReverse,
            0x05 => FlexFlow::ColumnWrap,
            0x09 => FlexFlow::ColumnReverse,
            0x0D => FlexFlow::ColumnWrapReverse,
            _ => FlexFlow::Row,
        }
    }

    /// `flow & LV_FLEX_COLUMN`
    #[inline]
    pub const fn is_column(self) -> bool {
        (self as u8) & FLEX_COLUMN != 0
    }
    /// `flow & LV_FLEX_WRAP`
    #[inline]
    pub const fn is_wrap(self) -> bool {
        (self as u8) & FLEX_WRAP != 0
    }
    /// `flow & LV_FLEX_REVERSE`
    #[inline]
    pub const fn is_reverse(self) -> bool {
        (self as u8) & FLEX_REVERSE != 0
    }
}

/// `flex_update()`: the layout callback registered as `LV_LAYOUT_FLEX`.
pub fn flex_update(ui: &mut Ui, obj: Obj) {
    if !ui.obj_is_valid(obj) {
        return;
    }
    let mut flex = Flex::get(ui, obj);
    let rtl = ui.obj_get_style_base_dir(obj, Part::MAIN) == BaseDir::Rtl;
    let track_gap = if flex.row {
        ui.obj_get_style_pad_row(obj, Part::MAIN)
    } else {
        ui.obj_get_style_pad_column(obj, Part::MAIN)
    };
    let item_gap = if flex.row {
        ui.obj_get_style_pad_column(obj, Part::MAIN)
    } else {
        ui.obj_get_style_pad_row(obj, Part::MAIN)
    };
    let max_main_size = if flex.row {
        ui.obj_get_content_width(obj)
    } else {
        ui.obj_get_content_height(obj)
    };
    let coords = ui.obj_get_coords(obj);
    let mut abs_y = coords.y1 + ui.obj_get_style_space_top(obj, Part::MAIN) - ui.obj_get_scroll_y(obj);
    let mut abs_x = coords.x1 + ui.obj_get_style_space_left(obj, Part::MAIN) - ui.obj_get_scroll_x(obj);
    let mut track_place = flex.track_place;
    let w_set = ui.obj_get_style_width(obj, Part::MAIN);
    let h_set = ui.obj_get_style_height(obj, Part::MAIN);
    let data = ui.obj(obj).expect("validated container");
    // Content-sized cross axes squeeze the track gaps regardless of alignment.
    if (flex.row && h_set == coord::SIZE_CONTENT && !data.h_layout)
        || (!flex.row && w_set == coord::SIZE_CONTENT && !data.w_layout)
    {
        track_place = FlexAlign::Start;
    }
    if rtl && !flex.row {
        track_place = match track_place {
            FlexAlign::Start => FlexAlign::End,
            FlexAlign::End => FlexAlign::Start,
            other => other,
        };
    }

    let mut total_track_cross_size = 0;
    let mut gap = 0;
    let mut track_count = 0;
    if track_place != FlexAlign::Start {
        let mut first = first_child_index(ui, obj, flex.rev);
        while in_child_range(ui, obj, first) {
            let Some((next, track)) =
                find_track_end(ui, obj, &mut flex, first, max_main_size, item_gap, false)
            else {
                return;
            };
            total_track_cross_size += track.cross_size + track_gap;
            track_count += 1;
            first = next;
        }
        if track_count != 0 {
            total_track_cross_size -= track_gap;
        }
        let max_cross_size = if flex.row {
            ui.obj_get_content_height(obj)
        } else {
            ui.obj_get_content_width(obj)
        };
        let cross_pos = if flex.row { &mut abs_y } else { &mut abs_x };
        place_content(
            track_place,
            max_cross_size,
            total_track_cross_size,
            track_count,
            cross_pos,
            &mut gap,
        );
    }

    let mut first = first_child_index(ui, obj, flex.rev);
    if rtl && !flex.row {
        abs_x += total_track_cross_size;
    }
    while in_child_range(ui, obj, first) {
        let Some((next, mut track)) =
            find_track_end(ui, obj, &mut flex, first, max_main_size, item_gap, true)
        else {
            return;
        };
        if rtl && !flex.row {
            abs_x -= track.cross_size;
        }
        children_repos(
            ui,
            obj,
            &flex,
            first,
            next,
            abs_x,
            abs_y,
            max_main_size,
            item_gap,
            &mut track,
        );
        first = next;
        if rtl && !flex.row {
            abs_x -= gap + track_gap;
        } else if flex.row {
            abs_y += track.cross_size + gap + track_gap;
        } else {
            abs_x += track.cross_size + gap + track_gap;
        }
    }
    if !ui.obj_is_valid(obj) {
        return;
    }
    if w_set == coord::SIZE_CONTENT || h_set == coord::SIZE_CONTENT {
        ui.obj_refr_size(obj);
    }
    if ui.obj_is_valid(obj) {
        ui.obj_send_event(obj, EventCode::LAYOUT_CHANGED, EventParam::None);
    }
}

/// `calc_min_size()`: handle the main axis, leaving the cross axis to the
/// ordinary content-size calculation. Growing children contribute their minima.
pub fn flex_get_min_size(ui: &mut Ui, obj: Obj, req_size: &mut i32, width: bool) -> bool {
    if ui.obj(obj).and_then(|data| data.spec.as_ref()).is_none() {
        return false;
    }
    let mut flex = Flex::get(ui, obj);
    if flex.row != width {
        return false;
    }
    flex.wrap = false;
    *req_size = 0;
    let (item_gap, space_start, space_end) = if flex.row {
        (
            ui.obj_get_style_pad_column(obj, Part::MAIN),
            ui.obj_get_style_space_left(obj, Part::MAIN),
            ui.obj_get_style_space_right(obj, Part::MAIN),
        )
    } else {
        (
            ui.obj_get_style_pad_row(obj, Part::MAIN),
            ui.obj_get_style_space_top(obj, Part::MAIN),
            ui.obj_get_style_space_bottom(obj, Part::MAIN),
        )
    };
    let mut first = first_child_index(ui, obj, flex.rev);
    while in_child_range(ui, obj, first) {
        let Some((next, track)) = find_track_end(ui, obj, &mut flex, first, 0, item_gap, false) else {
            return false;
        };
        *req_size = (*req_size).max(track.fixed_main_size + track.grow_min_size);
        first = next;
    }
    *req_size += space_start + space_end;
    true
}

impl Ui {
    /// `lv_obj_set_flex_flow()`
    pub fn obj_set_flex_flow(&mut self, obj: Obj, flow: FlexFlow) {
        if !check_obj(self, obj) {
            return;
        }
        self.obj_set_style_flex_flow(obj, flow, StyleSelector::DEFAULT);
        self.obj_set_style_layout(obj, LAYOUT_FLEX as u16, StyleSelector::DEFAULT);
    }

    /// `lv_obj_set_flex_align()`
    pub fn obj_set_flex_align(
        &mut self,
        obj: Obj,
        main_place: FlexAlign,
        cross_place: FlexAlign,
        track_cross_place: FlexAlign,
    ) {
        if !check_obj(self, obj) {
            return;
        }
        self.obj_set_style_flex_main_place(obj, main_place, StyleSelector::DEFAULT);
        self.obj_set_style_flex_cross_place(obj, cross_place, StyleSelector::DEFAULT);
        self.obj_set_style_flex_track_place(obj, track_cross_place, StyleSelector::DEFAULT);
        self.obj_set_style_layout(obj, LAYOUT_FLEX as u16, StyleSelector::DEFAULT);
    }

    /// `lv_obj_set_flex_grow()`
    pub fn obj_set_flex_grow(&mut self, obj: Obj, grow: u8) {
        if !check_obj(self, obj) {
            return;
        }
        self.obj_set_style_flex_grow(obj, grow, StyleSelector::DEFAULT);
        if let Some(parent) = self.obj_get_parent(obj) {
            self.obj_mark_layout_as_dirty(parent);
        }
    }
}

struct Flex {
    main_place: FlexAlign,
    cross_place: FlexAlign,
    track_place: FlexAlign,
    row: bool,
    wrap: bool,
    rev: bool,
}

impl Flex {
    fn get(ui: &Ui, obj: Obj) -> Self {
        let flow = ui.obj_get_style_flex_flow(obj, Part::MAIN);
        Self {
            main_place: ui.obj_get_style_flex_main_place(obj, Part::MAIN),
            cross_place: ui.obj_get_style_flex_cross_place(obj, Part::MAIN),
            track_place: ui.obj_get_style_flex_track_place(obj, Part::MAIN),
            row: !flow.is_column(),
            wrap: flow.is_wrap(),
            rev: flow.is_reverse(),
        }
    }
}

struct Grow {
    item: Obj,
    min_size: i32,
    max_size: i32,
    final_size: i32,
    value: i32,
    clamped: bool,
}

#[derive(Default)]
struct Track {
    cross_size: i32,
    main_size: i32,
    fixed_main_size: i32,
    grow_min_size: i32,
    item_count: i32,
    grow_count: u32,
    grow: Vec<Grow>,
}

fn check_obj(ui: &Ui, obj: Obj) -> bool {
    if ui.obj_is_valid(obj) {
        true
    } else {
        check_arg_failed("flex: object must be valid");
        false
    }
}

fn first_child_index(ui: &Ui, obj: Obj, reverse: bool) -> i32 {
    if reverse {
        ui.obj_get_child_count(obj) as i32 - 1
    } else {
        0
    }
}

fn in_child_range(ui: &Ui, obj: Obj, index: i32) -> bool {
    ui.obj_is_valid(obj) && index >= 0 && index < ui.obj_get_child_count(obj) as i32
}

fn child_at(ui: &Ui, obj: Obj, index: i32) -> Option<Obj> {
    if in_child_range(ui, obj, index) {
        ui.obj_get_child(obj, index)
    } else {
        None
    }
}

fn next_item(ui: &Ui, obj: Obj, reverse: bool, index: &mut i32) -> Option<Obj> {
    *index += if reverse { -1 } else { 1 };
    child_at(ui, obj, *index)
}

fn excluded(ui: &Ui, obj: Obj) -> bool {
    ui.obj_is_ignore_layout(obj) || ui.obj_is_hidden(obj) || ui.obj_is_floating(obj)
}

fn size_with_margin(ui: &Ui, obj: Obj, width: bool) -> i32 {
    if width {
        ui.obj_get_style_margin_left(obj, Part::MAIN)
            + ui.obj_get_width(obj)
            + ui.obj_get_style_margin_right(obj, Part::MAIN)
    } else {
        ui.obj_get_style_margin_top(obj, Part::MAIN)
            + ui.obj_get_height(obj)
            + ui.obj_get_style_margin_bottom(obj, Part::MAIN)
    }
}

/// `find_track_end()`: track traversal order and wrap decisions mirror C.
fn find_track_end(
    ui: &mut Ui,
    cont: Obj,
    flex: &mut Flex,
    start: i32,
    max_main_size: i32,
    item_gap: i32,
    calculate_grow: bool,
) -> Option<(i32, Track)> {
    let w_set = ui.obj_get_style_width(cont, Part::MAIN);
    let h_set = ui.obj_get_style_height(cont, Part::MAIN);
    let ignore_size_content = ui.obj_get_parent(cont).is_some_and(|parent| {
        ui.obj_get_style_layout(parent, Part::MAIN) as u32 == LAYOUT_FLEX
            && ui.obj_get_style_flex_grow(cont, Part::MAIN) > 0
    });
    if flex.wrap
        && ((flex.row && w_set == coord::SIZE_CONTENT) || (!flex.row && h_set == coord::SIZE_CONTENT))
        && !ignore_size_content
    {
        flex.wrap = false;
    }
    let mut track = Track::default();
    let mut index = start;
    let mut item = child_at(ui, cont, index);
    let mut first = true;
    while let Some(child) = item {
        if !excluded(ui, child) {
            if !first && ui.obj_is_flex_in_new_track(child) {
                break;
            }
            let grow = ui.obj_get_style_flex_grow(child, Part::MAIN);
            if grow != 0 {
                let min_size = if flex.row {
                    ui.obj_calc_dynamic_width(child, StyleProp::MIN_WIDTH)
                } else {
                    ui.obj_calc_dynamic_height(child, StyleProp::MIN_HEIGHT)
                };
                // Content minimum/maximum sizes dispatch GET_SELF_SIZE. A
                // callback can delete either object while measuring the track.
                if !ui.obj_is_valid(child) || !ui.obj_is_valid(cont) {
                    return None;
                }
                let req_size = min_size + if first { 0 } else { item_gap };
                if flex.wrap && track.fixed_main_size + track.grow_min_size + req_size > max_main_size {
                    break;
                }
                track.grow_min_size += min_size;
                if !first {
                    track.fixed_main_size += item_gap;
                }
                track.grow_count += 1;
                if calculate_grow {
                    let max_size = if flex.row {
                        ui.obj_calc_dynamic_width(child, StyleProp::MAX_WIDTH)
                    } else {
                        ui.obj_calc_dynamic_height(child, StyleProp::MAX_HEIGHT)
                    };
                    if !ui.obj_is_valid(child) || !ui.obj_is_valid(cont) {
                        return None;
                    }
                    track.grow.push(Grow {
                        item: child,
                        min_size,
                        max_size,
                        final_size: 0,
                        value: grow as i32,
                        clamped: false,
                    });
                }
            } else {
                let req_size = size_with_margin(ui, child, flex.row) + if first { 0 } else { item_gap };
                if flex.wrap && track.fixed_main_size + track.grow_min_size + req_size > max_main_size {
                    break;
                }
                track.fixed_main_size += req_size;
            }
            first = false;
            track.cross_size = track.cross_size.max(size_with_margin(ui, child, !flex.row));
            track.item_count += 1;
        }
        item = next_item(ui, cont, flex.rev, &mut index);
    }
    track.main_size = if track.grow_count != 0 {
        max_main_size
    } else {
        track.fixed_main_size
    };
    // Even an oversized first item must make progress. Keep C's separate
    // main-size/item-count fallback rather than including it in grow allocation.
    if let Some(child) = item
        && index == start
    {
        next_item(ui, cont, flex.rev, &mut index);
        track.cross_size = size_with_margin(ui, child, !flex.row);
        track.main_size = size_with_margin(ui, child, flex.row);
        track.item_count = 1;
    }
    Some((index, track))
}

/// `children_repos()`: clamp grow sizes, distribute remaining space, then move.
fn children_repos(
    ui: &mut Ui,
    cont: Obj,
    flex: &Flex,
    mut first: i32,
    last: i32,
    abs_x: i32,
    abs_y: i32,
    max_main_size: i32,
    item_gap: i32,
    track: &mut Track,
) {
    let mut reiterate = true;
    while reiterate && track.grow_count != 0 {
        reiterate = false;
        let mut grow_sum = 0;
        let mut available = track.main_size - track.fixed_main_size;
        for grow in &track.grow {
            if grow.clamped {
                available -= grow.final_size;
            } else {
                grow_sum += grow.value;
            }
        }
        for grow in &mut track.grow {
            if !grow.clamped {
                debug_assert_ne!(grow_sum, 0);
                // The C product is uint32_t before conversion to int32_t.
                let size = available.wrapping_mul(grow.value).wrapping_add(grow_sum / 2) / grow_sum;
                let clamped = grow.min_size.max(size.min(grow.max_size));
                if size != clamped {
                    grow.clamped = true;
                    reiterate = true;
                }
                grow.final_size = clamped;
                grow_sum -= grow.value;
                available -= grow.final_size;
            }
        }
    }
    let rtl = ui.obj_get_style_base_dir(cont, Part::MAIN) == BaseDir::Rtl;
    let mut main_pos = 0;
    let mut place_gap = 0;
    place_content(
        flex.main_place,
        max_main_size,
        track.main_size,
        track.item_count,
        &mut main_pos,
        &mut place_gap,
    );
    if flex.row && rtl {
        main_pos = max_main_size - main_pos;
    }
    let mut item = child_at(ui, cont, first);
    while let Some(child) = item {
        if first == last {
            break;
        }
        if excluded(ui, child) {
            item = next_item(ui, cont, flex.rev, &mut first);
            continue;
        }
        let data = ui.obj(child).expect("live layout item");
        let (old_w_layout, old_h_layout) = (data.w_layout, data.h_layout);
        if ui.obj_get_style_flex_grow(child, Part::MAIN) != 0 {
            let size = track
                .grow
                .iter()
                .find(|grow| grow.item == child)
                .map_or(0, |grow| grow.final_size);
            let data = ui.obj_mut(child).expect("live layout item");
            data.w_layout = flex.row;
            data.h_layout = !flex.row;
            let old_coords = data.coords;
            if size
                != if flex.row {
                    old_coords.width()
                } else {
                    old_coords.height()
                }
            {
                ui.obj_invalidate(child);
                if let Some(data) = ui.obj_mut(child) {
                    if flex.row {
                        data.coords.set_width(size);
                    } else {
                        data.coords.set_height(size);
                    }
                }
                ui.obj_send_event(child, EventCode::SIZE_CHANGED, EventParam::Area(old_coords));
                if !ui.obj_is_valid(child) || !ui.obj_is_valid(cont) {
                    return;
                }
                if let Some(parent) = ui.obj_get_parent(child) {
                    ui.obj_send_event(parent, EventCode::CHILD_CHANGED, EventParam::Obj(child));
                }
                if !ui.obj_is_valid(child) || !ui.obj_is_valid(cont) {
                    return;
                }
                ui.obj_invalidate(child);
            }
        } else {
            let data = ui.obj_mut(child).expect("live layout item");
            data.w_layout = false;
            data.h_layout = false;
        }
        let data = ui.obj(child).expect("live layout item");
        if old_w_layout != data.w_layout || old_h_layout != data.h_layout {
            ui.obj_mark_layout_as_dirty(child);
        }
        let coords = ui.obj_get_coords(child);
        let cross_size = if flex.row { coords.height() } else { coords.width() };
        let main_size = if flex.row { coords.width() } else { coords.height() };
        let (main_margin_start, main_margin_end, cross_margin_start, cross_margin_end) = if flex.row {
            (
                ui.obj_get_style_margin_left(child, Part::MAIN),
                ui.obj_get_style_margin_right(child, Part::MAIN),
                ui.obj_get_style_margin_top(child, Part::MAIN),
                ui.obj_get_style_margin_bottom(child, Part::MAIN),
            )
        } else {
            (
                ui.obj_get_style_margin_top(child, Part::MAIN),
                ui.obj_get_style_margin_bottom(child, Part::MAIN),
                ui.obj_get_style_margin_left(child, Part::MAIN),
                ui.obj_get_style_margin_right(child, Part::MAIN),
            )
        };
        let cross_pos = match flex.cross_place {
            FlexAlign::Center => {
                (((track.cross_size + 1) & !1) - cross_size) / 2 + (cross_margin_start - cross_margin_end) / 2
            }
            FlexAlign::End => track.cross_size - cross_size - cross_margin_end,
            _ => cross_margin_start,
        };
        if flex.row && rtl {
            main_pos -= main_size;
        }
        let mut tx = ui.obj_get_style_translate_x(child, Part::MAIN);
        let mut ty = ui.obj_get_style_translate_y(child, Part::MAIN);
        if coord::is_pct(tx) {
            tx = coords.width().wrapping_mul(coord::get_pct(tx)) / 100;
        }
        if coord::is_pct(ty) {
            ty = coords.height().wrapping_mul(coord::get_pct(ty)) / 100;
        }
        let dx = abs_x - coords.x1
            + tx
            + if flex.row {
                main_pos + main_margin_start
            } else {
                cross_pos
            };
        let dy = abs_y - coords.y1
            + ty
            + if flex.row {
                cross_pos
            } else {
                main_pos + main_margin_start
            };
        if dx != 0 || dy != 0 {
            ui.obj_invalidate(child);
            if let Some(data) = ui.obj_mut(child) {
                data.coords.move_by(dx, dy);
            }
            ui.obj_invalidate(child);
            ui.obj_move_children_by(child, dx, dy, false);
        }
        if flex.row && rtl {
            main_pos -= item_gap + place_gap;
        } else {
            main_pos += main_size + item_gap + place_gap + main_margin_start + main_margin_end;
        }
        item = next_item(ui, cont, flex.rev, &mut first);
    }
}

/// `place_content()`: retain C's signed truncation and the single-item cases.
fn place_content(
    mut place: FlexAlign,
    max_size: i32,
    content_size: i32,
    item_count: i32,
    start: &mut i32,
    gap: &mut i32,
) {
    if item_count <= 1 && matches!(place, FlexAlign::SpaceAround | FlexAlign::SpaceEvenly) {
        place = FlexAlign::Center;
    }
    match place {
        FlexAlign::Center => {
            *gap = 0;
            *start += (max_size - content_size) / 2;
        }
        FlexAlign::End => {
            *gap = 0;
            *start += max_size - content_size;
        }
        FlexAlign::SpaceBetween => {
            if item_count > 1 {
                *gap = (max_size - content_size) / (item_count - 1);
            }
        }
        FlexAlign::SpaceAround => {
            *gap += (max_size - content_size) / item_count;
            *start += *gap / 2;
        }
        FlexAlign::SpaceEvenly => {
            *gap = (max_size - content_size) / (item_count + 1);
            *start += *gap;
        }
        FlexAlign::Start => {
            *gap = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_bits() {
        assert_eq!(FlexFlow::RowWrapReverse as u8, FLEX_WRAP | FLEX_REVERSE);
        assert_eq!(
            FlexFlow::ColumnWrapReverse as u8,
            FLEX_COLUMN | FLEX_WRAP | FLEX_REVERSE
        );
        assert!(FlexFlow::ColumnWrap.is_column());
        assert!(FlexFlow::ColumnWrap.is_wrap());
        assert!(!FlexFlow::ColumnWrap.is_reverse());
        assert_eq!(FlexFlow::from_u8(0x09), FlexFlow::ColumnReverse);
        assert_eq!(FlexAlign::SpaceBetween as u8, 5);
    }

    use crate::core::UiConfig;
    use crate::misc::area::Area;

    fn container(width: i32, height: i32) -> (Ui, Obj) {
        let mut ui = Ui::new(UiConfig::default());
        let display = ui.display_create(300, 300);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let cont = ui.obj_create(Some(screen));
        ui.obj_set_pos(cont, 0, 0);
        ui.obj_set_size(cont, width, height);
        ui.obj_set_style_pad_all(cont, 0, StyleSelector::DEFAULT);
        ui.obj_set_style_border_width(cont, 0, StyleSelector::DEFAULT);
        ui.obj_set_style_pad_row(cont, 0, StyleSelector::DEFAULT);
        ui.obj_set_style_pad_column(cont, 0, StyleSelector::DEFAULT);
        (ui, cont)
    }

    fn item(ui: &mut Ui, parent: Obj, width: i32, height: i32) -> Obj {
        let child = ui.obj_create(Some(parent));
        ui.obj_set_size(child, width, height);
        child
    }

    fn positions(ui: &Ui, children: &[Obj]) -> Vec<(i32, i32)> {
        children
            .iter()
            .map(|&obj| (ui.obj_get_x(obj), ui.obj_get_y(obj)))
            .collect()
    }

    #[test]
    fn row_main_alignment_preserves_integer_rounding() {
        for (align, xs) in [
            (FlexAlign::Start, [0, 25, 60]),
            (FlexAlign::End, [25, 50, 85]),
            (FlexAlign::Center, [12, 37, 72]),
            (FlexAlign::SpaceBetween, [0, 37, 84]),
            (FlexAlign::SpaceAround, [4, 37, 80]),
            (FlexAlign::SpaceEvenly, [6, 37, 78]),
        ] {
            let (mut ui, cont) = container(100, 40);
            let children = [
                item(&mut ui, cont, 20, 10),
                item(&mut ui, cont, 30, 10),
                item(&mut ui, cont, 15, 10),
            ];
            ui.obj_set_flex_flow(cont, FlexFlow::Row);
            ui.obj_set_style_pad_column(cont, 5, StyleSelector::DEFAULT);
            ui.obj_set_flex_align(cont, align, FlexAlign::Start, FlexAlign::Start);
            ui.obj_update_layout(cont);
            assert_eq!(positions(&ui, &children), xs.map(|x| (x, 0)), "{align:?}");
        }
    }

    #[test]
    fn reverse_and_rtl_have_independent_effects() {
        for (flow, rtl, xs) in [
            (FlexFlow::Row, false, [0, 25, 60]),
            (FlexFlow::RowReverse, false, [55, 20, 0]),
            (FlexFlow::Row, true, [80, 45, 25]),
            (FlexFlow::RowReverse, true, [25, 50, 85]),
        ] {
            let (mut ui, cont) = container(100, 40);
            let children = [
                item(&mut ui, cont, 20, 10),
                item(&mut ui, cont, 30, 10),
                item(&mut ui, cont, 15, 10),
            ];
            ui.obj_set_flex_flow(cont, flow);
            ui.obj_set_style_pad_column(cont, 5, StyleSelector::DEFAULT);
            ui.obj_set_style_base_dir(
                cont,
                if rtl { BaseDir::Rtl } else { BaseDir::Ltr },
                StyleSelector::DEFAULT,
            );
            ui.obj_update_layout(cont);
            assert_eq!(
                positions(&ui, &children),
                xs.map(|x| (x, 0)),
                "{flow:?}, rtl={rtl}"
            );
        }
    }

    #[test]
    fn row_wrap_cross_alignment_and_track_alignment() {
        let (mut ui, cont) = container(70, 100);
        let children = [
            item(&mut ui, cont, 30, 10),
            item(&mut ui, cont, 30, 20),
            item(&mut ui, cont, 30, 15),
        ];
        ui.obj_set_flex_flow(cont, FlexFlow::RowWrap);
        ui.obj_set_style_pad_column(cont, 5, StyleSelector::DEFAULT);
        ui.obj_set_style_pad_row(cont, 3, StyleSelector::DEFAULT);
        for (cross, track, expected) in [
            (FlexAlign::Start, FlexAlign::Start, [(0, 0), (35, 0), (0, 23)]),
            (FlexAlign::Center, FlexAlign::Start, [(0, 5), (35, 0), (0, 23)]),
            (FlexAlign::End, FlexAlign::Start, [(0, 10), (35, 0), (0, 23)]),
            (FlexAlign::Start, FlexAlign::Center, [(0, 31), (35, 31), (0, 54)]),
            (
                FlexAlign::Start,
                FlexAlign::SpaceBetween,
                [(0, 0), (35, 0), (0, 85)],
            ),
        ] {
            ui.obj_set_flex_align(cont, FlexAlign::Start, cross, track);
            ui.obj_update_layout(cont);
            assert_eq!(positions(&ui, &children), expected, "{cross:?}, {track:?}");
        }
    }

    #[test]
    fn column_wrap_places_rtl_tracks_from_the_right() {
        let (mut ui, cont) = container(100, 70);
        let children = [
            item(&mut ui, cont, 10, 30),
            item(&mut ui, cont, 20, 30),
            item(&mut ui, cont, 15, 30),
        ];
        ui.obj_set_flex_flow(cont, FlexFlow::ColumnWrap);
        ui.obj_set_style_pad_row(cont, 5, StyleSelector::DEFAULT);
        ui.obj_set_style_pad_column(cont, 3, StyleSelector::DEFAULT);
        ui.obj_set_style_base_dir(cont, BaseDir::Rtl, StyleSelector::DEFAULT);
        ui.obj_update_layout(cont);
        assert_eq!(positions(&ui, &children), [(80, 0), (80, 35), (62, 0)]);
        ui.obj_set_flex_flow(cont, FlexFlow::ColumnWrapReverse);
        ui.obj_update_layout(cont);
        assert_eq!(positions(&ui, &children), [(67, 0), (80, 35), (80, 0)]);
    }

    #[test]
    fn grow_distributes_remainders_and_reiterates_after_clamping() {
        let (mut ui, cont) = container(101, 30);
        let a = item(&mut ui, cont, 1, 10);
        let b = item(&mut ui, cont, 1, 10);
        let fixed = item(&mut ui, cont, 20, 10);
        ui.obj_set_flex_flow(cont, FlexFlow::Row);
        ui.obj_set_style_pad_column(cont, 2, StyleSelector::DEFAULT);
        ui.obj_set_flex_grow(a, 1);
        ui.obj_set_flex_grow(b, 2);
        ui.obj_update_layout(cont);
        assert_eq!((ui.obj_get_width(a), ui.obj_get_width(b)), (26, 51));
        assert_eq!(positions(&ui, &[a, b, fixed]), [(0, 0), (28, 0), (81, 0)]);
        assert!(ui.obj(a).unwrap().w_layout);
        assert!(!ui.obj(a).unwrap().h_layout);

        ui.obj_set_style_max_width(a, 10, StyleSelector::DEFAULT);
        ui.obj_update_layout(cont);
        assert_eq!((ui.obj_get_width(a), ui.obj_get_width(b)), (10, 67));
        ui.obj_set_style_max_width(a, coord::MAX, StyleSelector::DEFAULT);
        ui.obj_set_style_min_width(a, 60, StyleSelector::DEFAULT);
        ui.obj_update_layout(cont);
        assert_eq!((ui.obj_get_width(a), ui.obj_get_width(b)), (60, 17));
        ui.obj_set_style_min_width(a, 0, StyleSelector::DEFAULT);
        ui.obj_set_style_max_width(b, 10, StyleSelector::DEFAULT);
        ui.obj_update_layout(cont);
        assert_eq!((ui.obj_get_width(a), ui.obj_get_width(b)), (67, 10));

        ui.obj_set_flex_grow(a, 0);
        ui.obj_update_layout(cont);
        assert!(!ui.obj(a).unwrap().w_layout);
        assert_eq!(ui.obj_get_width(a), 1);
    }

    #[test]
    fn column_grow_resolves_percentage_minimum_and_maximum() {
        let (mut ui, cont) = container(30, 100);
        let a = item(&mut ui, cont, 10, 1);
        let b = item(&mut ui, cont, 10, 1);
        ui.obj_set_flex_flow(cont, FlexFlow::Column);
        ui.obj_set_flex_grow(a, 1);
        ui.obj_set_flex_grow(b, 1);
        ui.obj_set_style_min_height(a, coord::pct(60), StyleSelector::DEFAULT);
        ui.obj_set_style_max_height(b, coord::pct(40), StyleSelector::DEFAULT);
        ui.obj_update_layout(cont);
        assert_eq!((ui.obj_get_height(a), ui.obj_get_height(b)), (60, 40));
        assert_eq!(positions(&ui, &[a, b]), [(0, 0), (0, 60)]);
        assert!(ui.obj(a).unwrap().h_layout);
        assert!(!ui.obj(a).unwrap().w_layout);
    }

    #[test]
    fn hidden_floating_and_ignored_children_do_not_consume_space() {
        let (mut ui, cont) = container(100, 40);
        let a = item(&mut ui, cont, 10, 10);
        let hidden = item(&mut ui, cont, 50, 10);
        let floating = item(&mut ui, cont, 50, 10);
        let ignored = item(&mut ui, cont, 50, 10);
        let b = item(&mut ui, cont, 20, 10);
        ui.obj_set_hidden(hidden, true);
        ui.obj_set_floating(floating, true);
        ui.obj_set_ignore_layout(ignored, true);
        ui.obj_set_pos(hidden, 40, 20);
        ui.obj_set_pos(floating, 50, 20);
        ui.obj_set_pos(ignored, 60, 20);
        ui.obj_set_flex_flow(cont, FlexFlow::RowWrap);
        ui.obj_update_layout(cont);
        assert_eq!(positions(&ui, &[a, b]), [(0, 0), (10, 0)]);
        assert_eq!(
            positions(&ui, &[hidden, floating, ignored]),
            [(40, 20), (50, 20), (60, 20)]
        );
        let mut minimum = -1;
        assert!(flex_get_min_size(&mut ui, cont, &mut minimum, true));
        assert_eq!(minimum, 30);
    }

    #[test]
    fn forced_track_break_and_oversized_first_item_make_progress() {
        let (mut ui, cont) = container(40, 100);
        let a = item(&mut ui, cont, 70, 10);
        let b = item(&mut ui, cont, 10, 15);
        let c = item(&mut ui, cont, 10, 20);
        ui.obj_set_flex_flow(cont, FlexFlow::RowWrap);
        ui.obj_set_style_pad_row(cont, 2, StyleSelector::DEFAULT);
        ui.obj_set_flex_in_new_track(c, true);
        ui.obj_update_layout(cont);
        assert_eq!(positions(&ui, &[a, b, c]), [(0, 0), (0, 12), (0, 29)]);
        ui.obj_set_flex_flow(cont, FlexFlow::Row);
        ui.obj_update_layout(cont);
        assert_eq!(positions(&ui, &[a, b, c]), [(0, 0), (70, 0), (0, 17)]);
    }

    #[test]
    fn content_size_disables_wrapping_and_squeezes_cross_alignment() {
        let (mut ui, cont) = container(coord::SIZE_CONTENT, coord::SIZE_CONTENT);
        let a = item(&mut ui, cont, 70, 10);
        let b = item(&mut ui, cont, 70, 20);
        ui.obj_set_flex_flow(cont, FlexFlow::RowWrap);
        ui.obj_set_style_pad_column(cont, 5, StyleSelector::DEFAULT);
        ui.obj_set_flex_align(cont, FlexAlign::Start, FlexAlign::Start, FlexAlign::SpaceEvenly);
        ui.obj_update_layout(cont);
        assert_eq!((ui.obj_get_width(cont), ui.obj_get_height(cont)), (145, 20));
        assert_eq!(positions(&ui, &[a, b]), [(0, 0), (75, 0)]);
    }

    #[test]
    fn grown_content_sized_container_can_wrap() {
        let (mut ui, outer) = container(100, 80);
        ui.obj_set_flex_flow(outer, FlexFlow::Row);
        let inner = item(&mut ui, outer, coord::SIZE_CONTENT, coord::SIZE_CONTENT);
        ui.obj_set_flex_flow(inner, FlexFlow::RowWrap);
        ui.obj_set_flex_grow(inner, 1);
        let a = item(&mut ui, inner, 60, 10);
        let b = item(&mut ui, inner, 60, 10);
        ui.obj_update_layout(outer);
        assert_eq!((ui.obj_get_width(inner), ui.obj_get_height(inner)), (100, 20));
        assert_eq!(positions(&ui, &[a, b]), [(0, 0), (0, 10)]);
    }

    #[test]
    fn margins_translate_and_descendant_movement_preserve_physical_positions() {
        let (mut ui, cont) = container(100, 40);
        let a = item(&mut ui, cont, 20, 10);
        let nested = item(&mut ui, a, 2, 2);
        ui.obj_set_pos(nested, 2, 3);
        let b = item(&mut ui, cont, 10, 8);
        ui.obj_set_style_margin_left(a, 3, StyleSelector::DEFAULT);
        ui.obj_set_style_margin_right(a, 5, StyleSelector::DEFAULT);
        ui.obj_set_style_margin_top(a, 2, StyleSelector::DEFAULT);
        ui.obj_set_style_margin_bottom(a, 4, StyleSelector::DEFAULT);
        ui.obj_set_style_translate_x(a, coord::pct(50), StyleSelector::DEFAULT);
        ui.obj_set_flex_flow(cont, FlexFlow::Row);
        ui.obj_set_style_pad_column(cont, 2, StyleSelector::DEFAULT);
        ui.obj_set_flex_align(cont, FlexAlign::Start, FlexAlign::Center, FlexAlign::Start);
        ui.obj_update_layout(cont);
        assert_eq!(positions(&ui, &[a, b]), [(13, 2), (30, 4)]);
        assert_eq!(ui.obj_get_coords(nested), Area::new(15, 5, 16, 6));
    }

    #[test]
    fn minimum_size_counts_padding_grow_minima_and_forced_tracks() {
        let (mut ui, cont) = container(100, 40);
        let a = item(&mut ui, cont, 28, 10);
        let b = item(&mut ui, cont, 1, 10);
        let c = item(&mut ui, cont, 10, 10);
        ui.obj_set_flex_flow(cont, FlexFlow::Row);
        ui.obj_set_flex_grow(b, 1);
        ui.obj_set_style_min_width(b, 12, StyleSelector::DEFAULT);
        ui.obj_set_style_pad_column(cont, 4, StyleSelector::DEFAULT);
        ui.obj_set_style_pad_left(cont, 2, StyleSelector::DEFAULT);
        ui.obj_set_style_pad_right(cont, 3, StyleSelector::DEFAULT);
        ui.obj_set_flex_in_new_track(c, true);
        ui.obj_update_layout(cont);
        let mut size = -1;
        assert!(flex_get_min_size(&mut ui, cont, &mut size, true));
        assert_eq!(size, 49);
        assert!(!flex_get_min_size(&mut ui, cont, &mut size, false));
        assert_eq!(size, 49);
        assert_eq!(ui.obj_get_width(a), 28);
    }

    #[test]
    fn grow_size_events_report_old_coords_before_parent_notification() {
        use alloc::rc::Rc;
        use core::cell::RefCell;
        let (mut ui, cont) = container(100, 40);
        let a = item(&mut ui, cont, 1, 10);
        ui.obj_set_flex_flow(cont, FlexFlow::Row);
        ui.obj_update_layout(cont);
        let log = Rc::new(RefCell::new(Vec::new()));
        let child_log = log.clone();
        ui.obj_add_event_cb(
            a,
            Rc::new(move |ui, event| {
                if let EventParam::Area(old) = event.param() {
                    child_log.borrow_mut().push((0, old.width(), ui.obj_get_width(a)));
                }
            }),
            EventCode::SIZE_CHANGED,
            None,
        );
        let parent_log = log.clone();
        ui.obj_add_event_cb(
            cont,
            Rc::new(move |ui, event| {
                if matches!(event.param(), EventParam::Obj(obj) if *obj == a) {
                    parent_log.borrow_mut().push((1, 0, ui.obj_get_width(a)));
                }
            }),
            EventCode::CHILD_CHANGED,
            None,
        );
        ui.obj_set_flex_grow(a, 1);
        ui.obj_update_layout(cont);
        assert_eq!(&*log.borrow(), &[(0, 1, 100), (1, 0, 100)]);
    }

    #[test]
    fn grow_callbacks_can_delete_the_item_or_container() {
        use alloc::rc::Rc;
        for delete_container in [false, true] {
            let (mut ui, cont) = container(100, 40);
            let child = item(&mut ui, cont, 1, 10);
            ui.obj_set_flex_flow(cont, FlexFlow::Row);
            ui.obj_update_layout(cont);
            let target = if delete_container { cont } else { child };
            let code = if delete_container {
                EventCode::CHILD_CHANGED
            } else {
                EventCode::SIZE_CHANGED
            };
            ui.obj_add_event_cb(target, Rc::new(move |ui, _| ui.obj_delete(target)), code, None);
            ui.obj_set_flex_grow(child, 1);
            flex_update(&mut ui, cont);
            assert!(!ui.obj_is_valid(child));
            assert_eq!(ui.obj_is_valid(cont), !delete_container);
        }
    }

    #[test]
    fn content_minimum_and_maximum_callbacks_can_delete_during_track_measurement() {
        use crate::misc::style::StyleValue;
        use alloc::rc::Rc;
        for prop in [
            StyleProp::MIN_WIDTH,
            StyleProp::MAX_WIDTH,
            StyleProp::MIN_HEIGHT,
            StyleProp::MAX_HEIGHT,
        ] {
            let width = matches!(prop, StyleProp::MIN_WIDTH | StyleProp::MAX_WIDTH);
            let minimum = matches!(prop, StyleProp::MIN_WIDTH | StyleProp::MIN_HEIGHT);
            for query_minimum in [false, true] {
                if query_minimum && !minimum {
                    continue;
                }
                for delete_container in [false, true] {
                    let (mut ui, cont) = container(100, 100);
                    let child = item(&mut ui, cont, 10, 10);
                    ui.obj_set_flex_flow(cont, if width { FlexFlow::Row } else { FlexFlow::Column });
                    ui.obj_set_flex_grow(child, 1);
                    ui.obj_update_layout(cont);
                    ui.obj_set_local_style_prop(
                        child,
                        prop,
                        StyleValue::Num(coord::SIZE_CONTENT),
                        StyleSelector::DEFAULT,
                    );
                    let target = if delete_container { cont } else { child };
                    ui.obj_add_event_cb(
                        child,
                        Rc::new(move |ui, _| ui.obj_delete(target)),
                        EventCode::GET_SELF_SIZE,
                        None,
                    );
                    if query_minimum {
                        assert!(!flex_get_min_size(&mut ui, cont, &mut 0, width));
                    } else {
                        flex_update(&mut ui, cont);
                    }
                    assert!(!ui.obj_is_valid(child));
                    assert_eq!(ui.obj_is_valid(cont), !delete_container);
                }
            }
        }
    }

    #[test]
    fn public_layout_update_handles_deletion_while_resolving_content_dimensions() {
        use crate::misc::style::StyleValue;
        use alloc::rc::Rc;
        for prop in [
            StyleProp::WIDTH,
            StyleProp::MIN_WIDTH,
            StyleProp::MAX_WIDTH,
            StyleProp::HEIGHT,
            StyleProp::MIN_HEIGHT,
            StyleProp::MAX_HEIGHT,
        ] {
            for delete_container in [false, true] {
                let (mut ui, cont) = container(100, 100);
                let child = item(&mut ui, cont, 10, 10);
                ui.obj_set_flex_flow(cont, FlexFlow::Row);
                ui.obj_update_layout(cont);
                ui.obj_set_local_style_prop(
                    child,
                    prop,
                    StyleValue::Num(coord::SIZE_CONTENT),
                    StyleSelector::DEFAULT,
                );
                let target = if delete_container { cont } else { child };
                ui.obj_add_event_cb(
                    child,
                    Rc::new(move |ui, _| ui.obj_delete(target)),
                    EventCode::GET_SELF_SIZE,
                    None,
                );
                ui.obj_update_layout(cont);
                assert!(!ui.obj_is_valid(child));
                assert_eq!(ui.obj_is_valid(cont), !delete_container);
            }
        }
    }

    #[test]
    fn content_sizing_handles_deletion_from_nested_flex_minimum_callback() {
        use alloc::rc::Rc;
        for width in [false, true] {
            let (mut ui, cont) = container(100, 100);
            let child = item(&mut ui, cont, 10, 10);
            ui.obj_set_flex_flow(cont, if width { FlexFlow::Row } else { FlexFlow::Column });
            ui.obj_set_flex_grow(child, 1);
            ui.obj_update_layout(cont);
            if width {
                ui.obj_set_width(cont, coord::SIZE_CONTENT);
                ui.obj_set_style_min_width(child, coord::SIZE_CONTENT, StyleSelector::DEFAULT);
            } else {
                ui.obj_set_height(cont, coord::SIZE_CONTENT);
                ui.obj_set_style_min_height(child, coord::SIZE_CONTENT, StyleSelector::DEFAULT);
            }
            ui.obj_add_event_cb(
                child,
                Rc::new(move |ui, _| ui.obj_delete(cont)),
                EventCode::GET_SELF_SIZE,
                None,
            );
            let size = if width {
                ui.obj_calc_dynamic_width(cont, StyleProp::WIDTH)
            } else {
                ui.obj_calc_dynamic_height(cont, StyleProp::HEIGHT)
            };
            assert_eq!(size, 0);
            assert!(!ui.obj_is_valid(cont));
            assert!(!ui.obj_is_valid(child));
        }
    }
}
