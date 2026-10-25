//! Ported from lvgl v9.6.0: `include/lvgl/layouts/lv_layout.h`, `src/layouts/lv_layout_private.h`,
//! `src/layouts/lv_layout.c`.
//!
//! The layout registry. `LAYOUT_FLEX` and `LAYOUT_GRID` are registered by
//! `builtin_layouts()` when the matching feature is on.

pub mod flex;
pub mod grid;

use alloc::vec::Vec;

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::misc::style::Part;

/// `LV_LAYOUT_NONE`
pub const LAYOUT_NONE: u32 = 0;
/// `LV_LAYOUT_FLEX`
pub const LAYOUT_FLEX: u32 = 1;
/// `LV_LAYOUT_GRID`
pub const LAYOUT_GRID: u32 = 2;
/// `LV_LAYOUT_LAST`: first id for user layouts.
pub const LAYOUT_LAST: u32 = 3;

/// `lv_layout_update_cb_t`
pub type LayoutUpdateCallback = fn(&mut Ui, Obj, u32);
/// `lv_layout_get_min_size_cb_t`: `(ui, obj, req_size, width, user_data) -> handled`.
pub type LayoutGetMinSizeCallback = fn(&mut Ui, Obj, &mut i32, bool, u32) -> bool;

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
    #[must_use]
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
#[must_use]
pub(crate) fn builtin_layouts() -> Vec<LayoutDsc> {
    alloc::vec![
        LayoutDsc::new(layout_none_update),
        LayoutDsc {
            callbacks: LayoutCallbacks {
                update: |ui, obj, _| flex::flex_update(ui, obj),
                get_min_size: Some(|ui, obj, size, width, _| flex::flex_get_min_size(ui, obj, size, width)),
            },
            user_data: 0,
        },
        LayoutDsc {
            callbacks: LayoutCallbacks {
                update: |ui, obj, _| grid::grid_update(ui, obj),
                get_min_size: Some(|ui, obj, size, width, _| grid::grid_get_min_size(ui, obj, size, width)),
            },
            user_data: 0,
        },
    ]
}

fn layout_none_update(_ui: &mut Ui, _obj: Obj, _user_data: u32) {}

impl Ui {
    /// `lv_layout_create()`: register a user layout and return its id.
    #[must_use]
    pub fn layout_create(&mut self, callbacks: LayoutCallbacks, user_data: u32) -> u32 {
        let id = self.layouts.len() as u32;
        self.layouts.push(LayoutDsc { callbacks, user_data });
        id
    }

    /// `lv_layout_register()` (deprecated alias of `layout_create`).
    #[must_use]
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
        let id = self.obj_get_style_layout(obj, Part::MAIN) as usize;
        if id > 0
            && let Some(layout) = self.layouts.get(id).copied()
        {
            (layout.callbacks.update)(self, obj, layout.user_data);
        }
    }

    /// `lv_layout_get_min_size()`
    pub fn layout_get_min_size(&mut self, obj: Obj, size: &mut i32, width: bool) -> bool {
        let id = self.obj_get_style_layout(obj, Part::MAIN) as usize;
        if id > 0
            && let Some(layout) = self.layouts.get(id).copied()
            && let Some(callback) = layout.callbacks.get_min_size
        {
            return callback(self, obj, size, width, layout.user_data);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::UiConfig;
    use crate::misc::style::StyleSelector;

    #[test]
    fn builtin_ids() {
        let l = builtin_layouts();
        assert_eq!(l.len(), 3);
        assert_eq!(LAYOUT_FLEX, 1);
        assert_eq!(LAYOUT_GRID, 2);
    }

    #[test]
    fn layout_dispatches_user_data_and_minimum_size_without_holding_registry_borrow() {
        let mut ui = Ui::new(UiConfig::default());
        let disp = ui.display_create(32, 24);
        let screen = ui.display_get_screen_active(Some(disp)).unwrap();
        let obj = ui.obj_create(Some(screen));
        let layout = ui.layout_create(
            LayoutCallbacks {
                update: |ui, _, tag| {
                    ui.set_app(tag);
                    // Registration from the callback may reallocate the registry.
                    let _ = ui.layout_register(|_, _, _| {}, 0);
                },
                get_min_size: Some(|_, _, size, width, tag| {
                    *size = if width { tag as i32 } else { -(tag as i32) };
                    true
                }),
            },
            42,
        );
        assert_eq!(layout, LAYOUT_LAST);
        ui.obj_set_style_layout(obj, layout as u16, StyleSelector::DEFAULT);
        ui.layout_apply(obj);
        assert_eq!(ui.app::<u32>(), Some(&42));
        let mut size = 0;
        assert!(ui.layout_get_min_size(obj, &mut size, true));
        assert_eq!(size, 42);
        assert!(ui.layout_get_min_size(obj, &mut size, false));
        assert_eq!(size, -42);
        for id in [LAYOUT_NONE as u16, u16::MAX] {
            ui.obj_set_style_layout(obj, id, StyleSelector::DEFAULT);
            ui.layout_apply(obj);
            assert!(!ui.layout_get_min_size(obj, &mut size, true));
            assert_eq!(size, -42);
        }
    }
}
