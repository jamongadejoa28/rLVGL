//! Ported from lvgl v9.6.0: `include/lvgl/themes/lv_theme.h`, `src/themes/lv_theme_private.h`,
//! `src/themes/lv_theme.c`.

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::any::Any;
use core::cell::{RefCell, RefMut};

use crate::core::Ui;
use crate::core::obj::Obj;
use crate::display::DisplayId;
use crate::font::FontRef;
use crate::misc::arena::Key;
use crate::misc::color::Color;
use crate::misc::palette::{self, Palette};

/// Handle of a theme (`lv_theme_t *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ThemeId(#[doc(hidden)] pub Key);

/// `lv_theme_apply_cb_t`
pub type ThemeApplyCallback = fn(&mut Ui, ThemeId, Obj);

/// `lv_theme_t`
#[derive(Debug, Clone)]
pub struct Theme {
    /// `apply_cb`
    pub apply: Option<ThemeApplyCallback>,
    /// `parent`
    pub parent: Option<ThemeId>,
    /// `user_data`: copies of a theme share the same payload, like C's
    /// shallow pointer copy. Borrow guards keep mutable access safe.
    pub user_data: Option<Rc<RefCell<Box<dyn Any>>>>,
    /// Implementation-owned theme state. Kept separate from application
    /// `user_data`; copies share the state, like an extended C theme object.
    pub extension: Option<Rc<dyn Any>>,
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
    #[must_use]
    pub fn new() -> Self {
        Self {
            apply: None,
            parent: None,
            user_data: None,
            extension: None,
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
    #[must_use]
    pub fn theme_create(&mut self) -> ThemeId {
        ThemeId(self.themes.insert(Theme::new()))
    }

    /// `lv_theme_delete()`
    pub fn theme_delete(&mut self, theme: ThemeId) {
        self.themes.remove(theme.0);
    }

    /// `lv_theme_copy()`
    pub fn theme_copy(&mut self, dst: ThemeId, src: ThemeId) {
        if let Some(source) = self.theme(src).cloned()
            && let Some(destination) = self.theme_mut(dst)
        {
            *destination = source;
        }
    }

    /// `lv_theme_get_from_obj()`
    #[must_use]
    pub fn theme_get_from_obj(&self, obj: Option<Obj>) -> Option<ThemeId> {
        let disp = match obj {
            Some(obj) => self.obj_get_display(obj),
            None => self.display_get_default(),
        }?;
        self.display(disp)?.theme
    }

    /// `lv_theme_apply()`: apply the display's theme (and its parents) to `obj`.
    pub fn theme_apply(&mut self, obj: Obj) {
        if self.obj(obj).is_none() {
            return;
        }
        let theme = self.theme_get_from_obj(Some(obj));
        self.obj_remove_style_all(obj);
        if let Some(theme) = theme {
            self.theme_apply_recursion(theme, obj);
        }
    }

    /// `lv_theme_set_parent()`
    pub fn theme_set_parent(&mut self, theme: ThemeId, parent: Option<ThemeId>) {
        if let Some(theme) = self.theme_mut(theme) {
            theme.parent = parent;
        }
    }

    /// `lv_theme_set_apply_cb()`
    pub fn theme_set_apply_cb(&mut self, theme: ThemeId, apply_cb: Option<ThemeApplyCallback>) {
        if let Some(theme) = self.theme_mut(theme) {
            theme.apply = apply_cb;
        }
    }

    /// `lv_theme_set_user_data()`
    pub fn theme_set_user_data(&mut self, theme: ThemeId, user_data: Option<Box<dyn Any>>) {
        if let Some(theme) = self.theme_mut(theme) {
            theme.user_data = user_data.map(|data| Rc::new(RefCell::new(data)));
        }
    }

    /// `lv_theme_get_font_small()`
    #[must_use]
    pub fn theme_get_font_small(&self, obj: Option<Obj>) -> Option<FontRef> {
        match self.theme_get_from_obj(obj).and_then(|id| self.theme(id)) {
            Some(theme) => theme.font_small.clone(),
            None => self.config().font_default.clone(),
        }
    }

    /// `lv_theme_get_font_normal()`
    #[must_use]
    pub fn theme_get_font_normal(&self, obj: Option<Obj>) -> Option<FontRef> {
        match self.theme_get_from_obj(obj).and_then(|id| self.theme(id)) {
            Some(theme) => theme.font_normal.clone(),
            None => self.config().font_default.clone(),
        }
    }

    /// `lv_theme_get_font_large()`
    #[must_use]
    pub fn theme_get_font_large(&self, obj: Option<Obj>) -> Option<FontRef> {
        match self.theme_get_from_obj(obj).and_then(|id| self.theme(id)) {
            Some(theme) => theme.font_large.clone(),
            None => self.config().font_default.clone(),
        }
    }

    /// `lv_theme_get_color_primary()`
    #[must_use]
    pub fn theme_get_color_primary(&self, obj: Option<Obj>) -> Color {
        self.theme_get_from_obj(obj)
            .and_then(|id| self.theme(id))
            .map_or_else(|| palette::main(Palette::BlueGrey), |theme| theme.color_primary)
    }

    /// `lv_theme_get_color_secondary()`
    #[must_use]
    pub fn theme_get_color_secondary(&self, obj: Option<Obj>) -> Color {
        self.theme_get_from_obj(obj)
            .and_then(|id| self.theme(id))
            .map_or_else(|| palette::main(Palette::Blue), |theme| theme.color_secondary)
    }

    /// `lv_theme_get_user_data()`: borrow the shared payload mutably.
    /// Returns `None` if a copy of the theme already holds a borrow.
    #[must_use]
    pub fn theme_get_user_data(&self, theme: ThemeId) -> Option<RefMut<'_, dyn Any>> {
        let data = self.theme(theme)?.user_data.as_ref()?;
        Some(RefMut::map(data.try_borrow_mut().ok()?, |data| data.as_mut()))
    }

    /// `apply_theme()`: parent themes run first, then all styles become
    /// theme styles after each non-null callback.
    fn theme_apply_parent_chain(&mut self, theme: ThemeId, obj: Obj) {
        let Some(parent) = self.theme(theme).map(|theme| theme.parent) else {
            return;
        };
        if let Some(parent) = parent {
            self.theme_apply_parent_chain(parent, obj);
        }
        if let Some(callback) = self.theme(theme).and_then(|theme| theme.apply) {
            callback(self, theme, obj);
            if let Some(obj) = self.obj_mut(obj) {
                for style in &mut obj.styles {
                    style.is_theme = true;
                }
            }
        }
    }

    /// `apply_theme_recursion()`: inheritable classes see the base class
    /// first; restore the original class before applying its theme.
    fn theme_apply_recursion(&mut self, theme: ThemeId, obj: Obj) {
        let Some(class) = self.obj(obj).map(|obj| obj.class) else {
            return;
        };
        if class.theme_inheritable
            && let Some(base) = class.base
        {
            self.obj_mut(obj).unwrap().class = base;
            self.theme_apply_recursion(theme, obj);
        }
        if let Some(data) = self.obj_mut(obj) {
            data.class = class;
            self.theme_apply_parent_chain(theme, obj);
        }
    }

    /// Borrow a theme.
    #[inline]
    #[must_use]
    pub fn theme(&self, theme: ThemeId) -> Option<&Theme> {
        self.themes.get(theme.0)
    }

    /// Mutably borrow a theme.
    #[inline]
    #[must_use]
    pub fn theme_mut(&mut self, theme: ThemeId) -> Option<&mut Theme> {
        self.themes.get_mut(theme.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::UiConfig;
    use crate::core::obj::{OBJ_CLASS, ObjClass, ObjClassEditable, ObjClassGroupDef};
    use crate::misc::style::{Part, StyleSelector};
    use alloc::vec;
    use alloc::vec::Vec;

    static DERIVED_CLASS: ObjClass = ObjClass {
        base: Some(&OBJ_CLASS),
        name: "theme-test",
        new_state: || Box::new(()),
        constructor: None,
        destructor: None,
        event_cb: None,
        width_def: 10,
        height_def: 10,
        editable: ObjClassEditable::Inherit,
        group_def: ObjClassGroupDef::Inherit,
        theme_inheritable: true,
    };

    fn record_theme(ui: &mut Ui, theme: ThemeId, obj: Obj) {
        let class = ui.obj_get_class(obj).name;
        ui.app_mut::<Vec<(ThemeId, &'static str)>>()
            .unwrap()
            .push((theme, class));
        ui.obj_set_style_bg_color(obj, Color::WHITE, StyleSelector::DEFAULT);
    }

    #[test]
    fn inherited_theme_order_restores_class_and_marks_styles() {
        let mut ui = Ui::new(UiConfig::default());
        let disp = ui.display_create(32, 24);
        let screen = ui.display_get_screen_active(Some(disp)).unwrap();
        let obj = ui.obj_class_create_obj(&DERIVED_CLASS, Some(screen));
        let parent = ui.theme_create();
        let child = ui.theme_create();
        ui.theme_set_parent(child, Some(parent));
        ui.theme_set_apply_cb(parent, Some(record_theme));
        ui.theme_set_apply_cb(child, Some(record_theme));
        ui.display_mut(disp).unwrap().theme = Some(child);
        ui.set_app(Vec::<(ThemeId, &'static str)>::new());
        ui.theme_apply(obj);
        assert_eq!(
            ui.app::<Vec<(ThemeId, &'static str)>>().unwrap(),
            &vec![
                (parent, OBJ_CLASS.name),
                (child, OBJ_CLASS.name),
                (parent, DERIVED_CLASS.name),
                (child, DERIVED_CLASS.name),
            ]
        );
        assert!(core::ptr::eq(ui.obj_get_class(obj), &DERIVED_CLASS));
        let styles = &ui.obj(obj).unwrap().styles;
        assert!(!styles.is_empty());
        assert!(styles.iter().all(|style| style.is_theme));
        assert_eq!(ui.obj_get_style_bg_color(obj, Part::MAIN), Color::WHITE);

        // Even a missing theme removes all styles, before the null check in C.
        ui.display_mut(disp).unwrap().theme = None;
        ui.theme_apply(obj);
        assert!(ui.obj(obj).unwrap().styles.is_empty());
        ui.obj_delete(obj);
        assert!(!ui.obj_is_valid(obj));
        ui.deinit();
        assert!(ui.displays.is_empty());
        assert!(ui.objs.is_empty());
    }

    #[test]
    fn theme_copy_shares_payload_but_reassignment_is_independent() {
        let mut ui = Ui::new(UiConfig::default());
        let source = ui.theme_create();
        let copy = ui.theme_create();
        ui.theme_set_user_data(source, Some(Box::new(7u32)));
        ui.theme_mut(source).unwrap().flags = 42;
        ui.theme_copy(copy, source);
        assert_eq!(ui.theme(copy).unwrap().flags, 42);
        *ui.theme_get_user_data(copy)
            .unwrap()
            .downcast_mut::<u32>()
            .unwrap() = 9;
        assert_eq!(
            ui.theme_get_user_data(source).unwrap().downcast_ref::<u32>(),
            Some(&9)
        );
        let borrow = ui.theme_get_user_data(source).unwrap();
        assert!(ui.theme_get_user_data(copy).is_none());
        drop(borrow);
        ui.theme_set_user_data(source, Some(Box::new(11u32)));
        assert_eq!(
            ui.theme_get_user_data(copy).unwrap().downcast_ref::<u32>(),
            Some(&9)
        );
        ui.theme_delete(source);
        assert!(ui.theme(source).is_none());
        assert_eq!(
            ui.theme_get_user_data(copy).unwrap().downcast_ref::<u32>(),
            Some(&9)
        );
    }

    #[test]
    fn fallback_palette_matches_upstream() {
        let ui = Ui::new(UiConfig::default());
        assert_eq!(ui.theme_get_color_primary(None), palette::main(Palette::BlueGrey));
        assert_eq!(ui.theme_get_color_secondary(None), palette::main(Palette::Blue));
        assert!(ui.theme_get_font_normal(None).is_none());
    }
}
