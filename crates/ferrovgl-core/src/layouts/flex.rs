//! Ported from lvgl v9.6.0: `include/lvgl/layouts/lv_flex.h`, `src/layouts/flex/lv_flex.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use crate::core::Ui;
use crate::core::obj::Obj;

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
    todo!()
}

/// `flex_get_min_size()`
pub fn flex_get_min_size(ui: &mut Ui, obj: Obj, req_size: &mut i32, width: bool) -> bool {
    todo!()
}

impl Ui {
    /// `lv_obj_set_flex_flow()`
    pub fn obj_set_flex_flow(&mut self, obj: Obj, flow: FlexFlow) {
        todo!()
    }

    /// `lv_obj_set_flex_align()`
    pub fn obj_set_flex_align(
        &mut self,
        obj: Obj,
        main_place: FlexAlign,
        cross_place: FlexAlign,
        track_cross_place: FlexAlign,
    ) {
        todo!()
    }

    /// `lv_obj_set_flex_grow()`
    pub fn obj_set_flex_grow(&mut self, obj: Obj, grow: u8) {
        todo!()
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
}
