//! Ported from lvgl v9.6.0: `include/lvgl/themes/lv_theme.h`, `src/themes/lv_theme_private.h`,
//! `src/themes/lv_theme.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::boxed::Box;
use core::any::Any;

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::display::DisplayId;
use crate::font::FontRef;
use crate::misc::arena::Key;
use crate::misc::color::Color;

/// Handle of a theme (`lv_theme_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ThemeId(pub Key);

/// `lv_theme_apply_cb_t`
pub type ThemeApplyCallback = fn(&mut Ui, ThemeId, Obj);

/// `lv_theme_t`
pub struct Theme {
    /// `apply_cb`
    pub apply: Option<ThemeApplyCallback>,
    /// `parent`
    pub parent: Option<ThemeId>,
    /// `user_data`
    pub user_data: Option<Box<dyn Any>>,
    /// `disp`
    pub disp: Option<DisplayId>,
    /// `color_primary`
    pub color_primary: Color,
    /// `color_secondary`
    pub color_secondary: Color,
    /// `font_small`
    pub font_small: Option<FontRef>,
    /// `font_normal`
    pub font_normal: Option<FontRef>,
    /// `font_large`
    pub font_large: Option<FontRef>,
    /// `flags`
    pub flags: u32,
}

impl Theme {
    /// `lv_theme_create()` defaults (all zero / `None`).
    pub fn new() -> Self {
        Self {
            apply: None,
            parent: None,
            user_data: None,
            disp: None,
            color_primary: Color::BLACK,
            color_secondary: Color::BLACK,
            font_small: None,
            font_normal: None,
            font_large: None,
            flags: 0,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    /// `lv_theme_create()`
    pub fn theme_create(&mut self) -> ThemeId {
        todo!()
    }

    /// `lv_theme_delete()`
    pub fn theme_delete(&mut self, theme: ThemeId) {
        todo!()
    }

    /// `lv_theme_copy()`
    pub fn theme_copy(&mut self, dst: ThemeId, src: ThemeId) {
        todo!()
    }

    /// `lv_theme_get_from_obj()`
    pub fn theme_get_from_obj(&self, obj: Option<Obj>) -> Option<ThemeId> {
        todo!()
    }

    /// `lv_theme_apply()`: apply the display's theme (and its parents) to `obj`.
    pub fn theme_apply(&mut self, obj: Obj) {
        todo!()
    }

    /// `lv_theme_set_parent()`
    pub fn theme_set_parent(&mut self, theme: ThemeId, parent: Option<ThemeId>) {
        todo!()
    }

    /// `lv_theme_set_apply_cb()`
    pub fn theme_set_apply_cb(&mut self, theme: ThemeId, apply_cb: Option<ThemeApplyCallback>) {
        todo!()
    }

    /// `lv_theme_set_user_data()`
    pub fn theme_set_user_data(&mut self, theme: ThemeId, user_data: Option<Box<dyn Any>>) {
        todo!()
    }

    /// `lv_theme_get_font_small()`
    pub fn theme_get_font_small(&self, obj: Option<Obj>) -> Option<FontRef> {
        todo!()
    }

    /// `lv_theme_get_font_normal()`
    pub fn theme_get_font_normal(&self, obj: Option<Obj>) -> Option<FontRef> {
        todo!()
    }

    /// `lv_theme_get_font_large()`
    pub fn theme_get_font_large(&self, obj: Option<Obj>) -> Option<FontRef> {
        todo!()
    }

    /// `lv_theme_get_color_primary()`
    pub fn theme_get_color_primary(&self, obj: Option<Obj>) -> Color {
        todo!()
    }

    /// `lv_theme_get_color_secondary()`
    pub fn theme_get_color_secondary(&self, obj: Option<Obj>) -> Color {
        todo!()
    }

    /// `lv_theme_get_user_data()`
    pub fn theme_get_user_data(&mut self, theme: ThemeId) -> Option<&mut dyn Any> {
        todo!()
    }

    /// Borrow a theme.
    #[inline]
    pub fn theme(&self, theme: ThemeId) -> Option<&Theme> {
        self.themes.get(theme.0)
    }

    /// Mutably borrow a theme.
    #[inline]
    pub fn theme_mut(&mut self, theme: ThemeId) -> Option<&mut Theme> {
        self.themes.get_mut(theme.0)
    }
}
