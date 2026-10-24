//! Ported from lvgl v9.6.0: `include/lvgl/layouts/lv_layout.h`, `src/layouts/lv_layout_private.h`,
//! `src/layouts/lv_layout.c`.
//!
//! The layout registry. `LAYOUT_FLEX` and `LAYOUT_GRID` are registered by
//! [`builtin_layouts`] when the matching feature is on.

#![allow(unused_variables)] // skeleton: bodies are todo!()

pub mod flex;
pub mod grid;

use alloc::vec::Vec;

use crate::core::Ui;
use crate::core::obj::Obj;

/// `LV_LAYOUT_NONE`
pub const LAYOUT_NONE: u32 = 0;
/// `LV_LAYOUT_FLEX`
pub const LAYOUT_FLEX: u32 = 1;
/// `LV_LAYOUT_GRID`
pub const LAYOUT_GRID: u32 = 2;
/// `LV_LAYOUT_LAST`: first id for user layouts.
pub const LAYOUT_LAST: u32 = 3;

/// `lv_layout_update_cb_t`
pub type LayoutUpdateCallback = fn(&mut Ui, Obj);
/// `lv_layout_get_min_size_cb_t`: `(ui, obj, req_size, width) -> handled`.
pub type LayoutGetMinSizeCallback = fn(&mut Ui, Obj, &mut i32, bool) -> bool;

/// `lv_layout_callbacks_t`
#[derive(Copy, Clone, Debug)]
pub struct LayoutCallbacks {
    /// `layout_update_cb`
    pub update: LayoutUpdateCallback,
    /// `get_min_size_cb`
    pub get_min_size: Option<LayoutGetMinSizeCallback>,
}

/// `lv_layout_dsc_t`
#[derive(Copy, Clone, Debug)]
pub struct LayoutDsc {
    /// `callbacks`
    pub callbacks: LayoutCallbacks,
    /// `user_data` (an opaque tag; layouts are `fn` pointers here).
    pub user_data: u32,
}

impl LayoutDsc {
    /// Build a descriptor with no `get_min_size_cb`.
    pub const fn new(update: LayoutUpdateCallback) -> Self {
        Self {
            callbacks: LayoutCallbacks {
                update,
                get_min_size: None,
            },
            user_data: 0,
        }
    }
}

/// `lv_layout_init()`: the registry with the built-in layouts at their
/// fixed ids (`LAYOUT_NONE` is a placeholder at index 0).
pub fn builtin_layouts() -> Vec<LayoutDsc> {
    alloc::vec![
        LayoutDsc::new(layout_none_update),
        LayoutDsc {
            callbacks: LayoutCallbacks {
                update: flex::flex_update,
                get_min_size: Some(flex::flex_get_min_size),
            },
            user_data: 0,
        },
        LayoutDsc {
            callbacks: LayoutCallbacks {
                update: grid::grid_update,
                get_min_size: Some(grid::grid_get_min_size),
            },
            user_data: 0,
        },
    ]
}

fn layout_none_update(ui: &mut Ui, obj: Obj) {}

impl Ui {
    /// `lv_layout_create()`: register a user layout and return its id.
    pub fn layout_create(&mut self, callbacks: LayoutCallbacks, user_data: u32) -> u32 {
        todo!()
    }

    /// `lv_layout_register()` (deprecated alias of `layout_create`).
    pub fn layout_register(&mut self, cb: LayoutUpdateCallback, user_data: u32) -> u32 {
        self.layout_create(
            LayoutCallbacks {
                update: cb,
                get_min_size: None,
            },
            user_data,
        )
    }

    /// `lv_layout_apply()`: run the layout of `obj`.
    pub fn layout_apply(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_layout_get_min_size()`
    pub fn layout_get_min_size(&mut self, obj: Obj, size: &mut i32, width: bool) -> bool {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_ids() {
        let l = builtin_layouts();
        assert_eq!(l.len(), 3);
        assert_eq!(LAYOUT_FLEX, 1);
        assert_eq!(LAYOUT_GRID, 2);
    }
}
