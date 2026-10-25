//! LVGL v9.6.0 `src/themes/simple/lv_theme_simple.c` for screens, base objects
//! and buttons. Label and image classes intentionally receive no own styles,
//! as in C. Other widget branches are not implemented.
//!
//! Theme state is owned per display, replacing C's global singleton. The four
//! styles used by these classes keep their IDs across reinitialization; copied
//! and shared themes retain their resources until their users are detached.
//! This theme uses fixed pixel sizes, so resolution and DPI changes need no
//! style rebuild or resolution callback.

use alloc::rc::Rc;

use ferrovgl_core::core::obj::OBJ_CLASS;
use ferrovgl_core::core::obj_style::StyleId;
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::theme::ThemeId;
use ferrovgl_core::{Color, DisplayId, Obj, Part, State, Style, StyleSelector, Ui};

#[derive(Clone)]
struct SimpleThemeData {
    owner: ThemeId,
    display: DisplayId,
    // scr, white, dark, scrollbar. The other C styles serve unported widgets.
    styles: [StyleId; 4],
}

fn data(ui: &Ui, theme: ThemeId) -> Option<&SimpleThemeData> {
    ui.theme(theme)?.extension.as_ref()?.downcast_ref()
}

/// `lv_theme_simple_init()`. Reuse this display's simple theme, using the
/// configured default font in its three font fields. Install the returned
/// theme with `Ui::display_set_theme`, or use this as a display creation hook.
pub fn init(ui: &mut Ui, display: DisplayId) -> Option<ThemeId> {
    ui.display(display)?;
    let existing = ui.themes.iter().find_map(|(key, theme)| {
        let state = theme.extension.as_ref()?.downcast_ref::<SimpleThemeData>()?;
        (state.display == display && state.owner == ThemeId(key)).then_some(ThemeId(key))
    });
    let theme = existing.unwrap_or_else(|| ui.theme_create());
    let styles = if let Some(old) = data(ui, theme) {
        let mut ids = old.styles;
        for (index, style) in build_styles().into_iter().enumerate() {
            if let Some(target) = ui.style_mut(ids[index]) {
                *target = style;
            } else {
                ids[index] = ui.style_create(style);
            }
        }
        ids
    } else {
        build_styles().map(|style| ui.style_create(style))
    };
    let font = ui.config().font_default.clone();
    let base = ui.theme_mut(theme)?;
    base.disp = Some(display);
    base.font_small = font.clone();
    base.font_normal = font.clone();
    base.font_large = font;
    base.apply = Some(apply);
    base.extension = Some(Rc::new(SimpleThemeData {
        owner: theme,
        display,
        styles,
    }));
    if ui.display_get_theme(Some(display)) == Some(theme) {
        ui.obj_report_style_change(None);
    }
    Some(theme)
}

/// Alias suitable for the same display-creation hook as `default::init_display`.
pub fn init_display(ui: &mut Ui, display: DisplayId) -> Option<ThemeId> {
    init(ui, display)
}

/// Release an unused original simple theme. Returns `false` while a live
/// display, child theme or shallow theme copy references its shared resources.
/// `Ui::deinit` also releases all theme resources.
pub fn deinit(ui: &mut Ui, theme: ThemeId) -> bool {
    let Some(state) = data(ui, theme).filter(|data| data.owner == theme).cloned() else {
        return false;
    };
    if ui
        .displays
        .iter()
        .any(|(_, display)| display.theme == Some(theme))
        || ui.themes.iter().any(|(key, candidate)| {
            ThemeId(key) != theme
                && (candidate.parent == Some(theme)
                    || candidate
                        .extension
                        .as_ref()
                        .and_then(|extension| extension.downcast_ref::<SimpleThemeData>())
                        .is_some_and(|data| data.owner == theme))
        })
    {
        return false;
    }
    ui.theme_delete(theme);
    for style in state.styles {
        ui.style_delete(style);
    }
    true
}

fn build_styles() -> [Style; 4] {
    let mut screen = Style::new();
    screen.set_bg_opa(255);
    screen.set_bg_color(palette::lighten(Palette::Grey, 4));
    screen.set_text_color(palette::darken(Palette::Grey, 2));
    let color_style = |color| {
        let mut style = Style::new();
        style.set_bg_opa(255);
        style.set_bg_color(color);
        style.set_line_width(1);
        style.set_line_color(color);
        style.set_arc_width(2);
        style.set_arc_color(color);
        style
    };
    let white = color_style(Color::WHITE);
    let dark = color_style(palette::main(Palette::Grey));
    let mut scrollbar = Style::new();
    scrollbar.set_bg_opa(255);
    scrollbar.set_bg_color(palette::main(Palette::Grey));
    scrollbar.set_width(2);
    [screen, white, dark, scrollbar]
}

fn apply(ui: &mut Ui, theme: ThemeId, obj: Obj) {
    if !ui.obj_is_valid(obj) {
        return;
    }
    let Some([screen, white, _dark, scrollbar]) = data(ui, theme).map(|data| data.styles) else {
        return;
    };
    if ui.obj_get_parent(obj).is_none() {
        ui.obj_add_style(obj, screen, StyleSelector::DEFAULT);
        if ui.obj_is_valid(obj) {
            ui.obj_add_style(obj, scrollbar, Part::SCROLLBAR | State::DEFAULT);
        }
    } else if ui.obj_check_type(obj, &OBJ_CLASS) {
        ui.obj_add_style(obj, white, StyleSelector::DEFAULT);
        if ui.obj_is_valid(obj) {
            ui.obj_add_style(obj, scrollbar, Part::SCROLLBAR | State::DEFAULT);
        }
    } else {
        #[cfg(feature = "button")]
        if ui.obj_check_type(obj, &crate::button::BUTTON_CLASS) {
            ui.obj_add_style(obj, _dark, StyleSelector::DEFAULT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;
    use ferrovgl_core::{FontRef, UiConfig};

    fn ui() -> Ui {
        Ui::new(UiConfig {
            font_default: Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14)),
            ..UiConfig::default()
        })
    }

    #[test]
    fn reapplying_theme_survives_deletion_from_first_style_callback() {
        let mut ui = ui();
        let display = ui.display_create_ex(320, 240, Some(init_display)).unwrap();
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let base = ui.obj_create(Some(screen));
        // Cover both multi-style branches, including a separately created screen.
        let another_screen = ui.obj_create(None);
        for obj in [base, another_screen] {
            ui.obj_add_event_cb(
                obj,
                Rc::new(move |ui, _| {
                    if ui.obj(obj).is_some_and(|data| !data.styles.is_empty()) {
                        ui.obj_delete(obj);
                    }
                }),
                ferrovgl_core::EventCode::STYLE_CHANGED,
                None,
            );
            ui.theme_apply(obj);
            assert!(!ui.obj_is_valid(obj));
        }
    }

    #[test]
    fn base_styles_are_fixed_pixels_and_reinitialization_preserves_identity() {
        let mut ui = ui();
        let display = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let theme = ui.display_get_theme(Some(display)).unwrap();
        let ids = data(&ui, theme).unwrap().styles;
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let obj = ui.obj_create(Some(screen));
        assert_eq!(
            ui.obj_get_style_bg_color(screen, Part::MAIN),
            Color::hex(0xf5f5f5)
        );
        assert_eq!(ui.obj_get_style_text_color(obj, Part::MAIN), Color::hex(0x616161));
        assert_eq!(ui.obj_get_style_bg_color(obj, Part::MAIN), Color::WHITE);
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::MAIN), 255);
        assert_eq!(ui.obj_get_style_line_width(obj, Part::MAIN), 1);
        assert_eq!(ui.obj_get_style_arc_width(obj, Part::MAIN), 2);
        assert_eq!(ui.obj_get_style_width(obj, Part::SCROLLBAR), 2);
        assert_eq!(ui.obj_get_style_radius(obj, Part::MAIN), 0);
        assert_eq!(ui.obj_get_style_pad_left(obj, Part::MAIN), 0);
        assert_eq!(ui.obj_get_style_border_width(obj, Part::MAIN), 0);
        let events = ui.display_get_event_count(display);
        ui.theme_set_user_data(theme, Some(Box::new(42u32)));
        ui.display_set_dpi(display, 320);
        ui.display_set_resolution(display, 240, 320);
        assert_eq!(init(&mut ui, display), Some(theme));
        assert_eq!(data(&ui, theme).unwrap().styles, ids);
        assert_eq!(ui.display_get_event_count(display), events);
        assert_eq!(ui.obj_get_style_width(obj, Part::SCROLLBAR), 2);
        assert_eq!(
            ui.theme_get_user_data(theme).unwrap().downcast_ref::<u32>(),
            Some(&42)
        );
        assert_eq!(ids.map(|id| ui.style(id).unwrap().len()), [3, 6, 6, 3]);
    }

    #[test]
    fn copied_and_shared_themes_keep_resources_until_detached() {
        let mut ui = ui();
        let display = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let original = ui.display_get_theme(Some(display)).unwrap();
        let copy = ui.theme_create();
        ui.theme_copy(copy, original);
        assert_eq!(init(&mut ui, display), Some(original));
        assert!(!deinit(&mut ui, copy));
        let second = ui.display_create(320, 240);
        ui.display_set_theme(second, Some(copy));
        ui.display_delete(display);
        assert!(!deinit(&mut ui, original));
        ui.display_delete(second);
        assert!(!deinit(&mut ui, original));
        ui.theme_delete(copy);
        assert!(deinit(&mut ui, original));
        assert!(ui.styles.is_empty());
        assert!(ui.themes.is_empty());
    }

    #[test]
    #[cfg(all(feature = "button", feature = "label"))]
    fn button_and_label_use_only_their_upstream_styles() {
        let mut ui = ui();
        let display = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let button = crate::Button::create(&mut ui, screen);
        let label = crate::Label::create(&mut ui, button.obj());
        assert_eq!(ui.obj(button.obj()).unwrap().styles.len(), 1);
        assert!(ui.obj(label.obj()).unwrap().styles.is_empty());
        assert_eq!(
            ui.obj_get_style_bg_color(button.obj(), Part::MAIN),
            Color::hex(0x9e9e9e)
        );
        assert_eq!(
            ui.obj_get_style_text_color(label.obj(), Part::MAIN),
            Color::hex(0x616161)
        );
        ui.obj_add_state(
            button.obj(),
            State::PRESSED | State::CHECKED | State::DISABLED | State::FOCUS_KEY,
        );
        assert_eq!(
            ui.obj_get_style_bg_color(button.obj(), Part::MAIN),
            Color::hex(0x9e9e9e)
        );
        assert_eq!(ui.obj_get_style_recolor_opa(button.obj(), Part::MAIN), 0);
        assert_eq!(ui.obj_get_style_outline_width(button.obj(), Part::MAIN), 0);
    }

    #[test]
    #[cfg(feature = "image")]
    fn image_has_no_own_theme_styles() {
        let mut ui = ui();
        let display = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let image = crate::Image::create(&mut ui, screen);
        assert!(ui.obj(image.obj()).unwrap().styles.is_empty());
        assert_eq!(ui.obj_get_style_bg_opa(image.obj(), Part::MAIN), 0);
        assert_eq!(
            ui.obj_get_style_text_color(image.obj(), Part::MAIN),
            Color::hex(0x616161)
        );
    }
}
