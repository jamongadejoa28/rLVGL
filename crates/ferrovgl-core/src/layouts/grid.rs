//! Ported from lvgl v9.6.0: `include/lvgl/layouts/lv_grid.h`, `src/layouts/grid/lv_grid.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::misc::style::TracksRef;
use crate::misc::types::coord;

/// `LV_GRID_CONTENT`
pub const GRID_CONTENT: i32 = coord::MAX - 101;
/// `LV_GRID_TEMPLATE_LAST`
pub const GRID_TEMPLATE_LAST: i32 = coord::MAX;

/// `LV_GRID_FR(x)` / `lv_grid_fr()`
#[inline]
pub const fn grid_fr(x: u8) -> i32 {
    coord::MAX - 100 + x as i32
}

/// `lv_grid_align_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GridAlign {
    /// `LV_GRID_ALIGN_START`
    #[default]
    Start = 0,
    /// `LV_GRID_ALIGN_CENTER`
    Center = 1,
    /// `LV_GRID_ALIGN_END`
    End = 2,
    /// `LV_GRID_ALIGN_STRETCH`
    Stretch = 3,
    /// `LV_GRID_ALIGN_SPACE_EVENLY`
    SpaceEvenly = 4,
    /// `LV_GRID_ALIGN_SPACE_AROUND`
    SpaceAround = 5,
    /// `LV_GRID_ALIGN_SPACE_BETWEEN`
    SpaceBetween = 6,
}

impl GridAlign {
    /// Convert a raw style number (`Start` if unknown).
    pub const fn from_u8(v: u8) -> GridAlign {
        match v {
            1 => GridAlign::Center,
            2 => GridAlign::End,
            3 => GridAlign::Stretch,
            4 => GridAlign::SpaceEvenly,
            5 => GridAlign::SpaceAround,
            6 => GridAlign::SpaceBetween,
            _ => GridAlign::Start,
        }
    }
}

/// `grid_update()`: the layout callback registered as `LV_LAYOUT_GRID`.
pub fn grid_update(ui: &mut Ui, obj: Obj) {
    todo!()
}

/// `grid_get_min_size()`
pub fn grid_get_min_size(ui: &mut Ui, obj: Obj, req_size: &mut i32, width: bool) -> bool {
    todo!()
}

impl Ui {
    /// `lv_obj_set_grid_dsc_array()`
    pub fn obj_set_grid_dsc_array(&mut self, obj: Obj, col_dsc: TracksRef, row_dsc: TracksRef) {
        todo!()
    }

    /// `lv_obj_set_grid_align()`
    pub fn obj_set_grid_align(&mut self, obj: Obj, column_align: GridAlign, row_align: GridAlign) {
        todo!()
    }

    /// `lv_obj_set_grid_cell()`
    pub fn obj_set_grid_cell(
        &mut self,
        obj: Obj,
        column_align: GridAlign,
        col_pos: i32,
        col_span: i32,
        row_align: GridAlign,
        row_pos: i32,
        row_span: i32,
    ) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_consts() {
        assert_eq!(grid_fr(1), coord::MAX - 99);
        assert!(GRID_CONTENT < grid_fr(0));
        assert_eq!(GRID_TEMPLATE_LAST, coord::MAX);
        assert_eq!(GridAlign::SpaceBetween as u8, 6);
    }
}
