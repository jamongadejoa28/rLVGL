//! Ported from LVGL v9.6.0: `src/themes/simple/lv_theme_simple.c`.
//!
//! `style_init()` builds the complete `my_theme_styles_t` set; the arc and
//! textarea styles follow their `#if LV_USE_<WIDGET>` crate features.
//! `theme_apply()` covers screens, base objects, buttons, button matrices,
//! bars, sliders, checkboxes, switches, arcs, spinners, text areas and LEDs.
//! Label, line and image classes intentionally receive no own styles, as in
//! C. Branches that need classes which are not ported yet (tabview, win,
//! msgbox, ...) are omitted.
//!
//! Each `Ui` owns one simple theme, like C's global `theme_def`
//! (`LV_GLOBAL_DEFAULT()->theme_simple`). The styles keep their IDs across
//! reinitialization; copied and shared themes
//! retain their resources until their users are detached. This theme uses
//! fixed pixel sizes, so resolution and DPI changes need no style rebuild or
//! resolution callback.

use alloc::rc::Rc;

use ferrovgl_core::core::obj::OBJ_CLASS;
use ferrovgl_core::core::obj_style::StyleId;
use ferrovgl_core::misc::color::{OPA_COVER, OPA_TRANSP};
use ferrovgl_core::misc::palette::{self, Palette};
#[cfg(feature = "textarea")]
use ferrovgl_core::misc::style::BorderSide;
use ferrovgl_core::theme::ThemeId;
use ferrovgl_core::{Color, DisplayId, Obj, Part, Style, StyleSelector, Ui};

/// `SCROLLBAR_WIDTH`
const SCROLLBAR_WIDTH: i32 = 2;

theme_styles! {
    /// `my_theme_styles_t`, in C declaration order.
    struct Styles {
        scr,
        transp,
        white,
        light,
        dark,
        dim,
        scrollbar,
        #[cfg(feature = "arc")]
        arc_line,
        #[cfg(feature = "arc")]
        arc_knob,
        #[cfg(feature = "textarea")]
        ta_cursor,
    }
}

#[derive(Clone)]
struct SimpleThemeData {
    /// The original theme (`theme_def`); a shallow `lv_theme_copy` keeps it.
    owner: ThemeId,
    styles: Styles<StyleId>,
}

fn data(ui: &Ui, theme: ThemeId) -> Option<&SimpleThemeData> {
    ui.theme(theme)?.extension.as_ref()?.downcast_ref()
}

/// `theme_def`: the single simple theme of this `Ui`
/// (`LV_GLOBAL_DEFAULT()->theme_simple`), or `None` before the first [`init`]
/// / after [`deinit`].
fn theme_def(ui: &Ui) -> Option<ThemeId> {
    ui.themes.iter().find_map(|(key, theme)| {
        let state = theme.extension.as_ref()?.downcast_ref::<SimpleThemeData>()?;
        (state.owner == ThemeId(key)).then_some(ThemeId(key))
    })
}

/// `lv_theme_simple_is_inited()`: whether this `Ui`'s simple theme exists.
#[must_use]
pub fn is_inited(ui: &Ui) -> bool {
    theme_def(ui).is_some()
}

/// `lv_theme_simple_get()`: this `Ui`'s simple theme, or `None` if it is not
/// initialized.
#[must_use]
pub fn get(ui: &Ui) -> Option<ThemeId> {
    if !is_inited(ui) {
        return None;
    }
    theme_def(ui)
}

/// `lv_theme_simple_init()`. There is one simple theme per `Ui`, like C's
/// global `theme_def`: every call re-initializes and returns it, with
/// `base.disp` set to `display` (which may be `None`) and the configured
/// default font in its three font fields. Install the returned theme with
/// `Ui::display_set_theme`, or use [`init_display`] as a display creation hook.
/// Returns `None` only for a deleted display.
pub fn init(ui: &mut Ui, display: impl Into<Option<DisplayId>>) -> Option<ThemeId> {
    let disp = display.into();
    if let Some(disp) = disp {
        ui.display(disp)?;
    }
    // if(!lv_theme_simple_is_inited()) theme_def = lv_malloc_zeroed(...)
    let theme = theme_def(ui).unwrap_or_else(|| ui.theme_create());
    let old = data(ui, theme).map(|data| data.styles);

    let font = ui.config().font_default.clone();
    let base = ui.theme_mut(theme)?;
    base.disp = disp;
    base.font_small = font.clone();
    base.font_normal = font.clone();
    base.font_large = font;
    base.apply = Some(theme_apply);

    // style_init(): style_init_reset() resets the existing shared styles in place.
    let styles = if let Some(old) = old {
        old.zip(style_init()).map(|(id, style)| {
            if let Some(target) = ui.style_mut(id) {
                *target = style;
                id
            } else {
                ui.style_create(style)
            }
        })
    } else {
        style_init().map(|style| ui.style_create(style))
    };
    ui.theme_mut(theme)?.extension = Some(Rc::new(SimpleThemeData { owner: theme, styles }));

    if disp.is_none() || ui.display_get_theme(disp) == Some(theme) {
        ui.obj_report_style_change(None);
    }

    // theme->inited = true;
    Some(theme)
}

/// The `LV_USE_THEME_SIMPLE` block of `lv_display_create()`
/// (`lv_display.c:141..146`): [`get`] if the simple theme is inited, otherwise
/// [`init`] it for `display`. Suitable for `Ui::display_create_ex`'s hook.
pub fn init_display(ui: &mut Ui, display: DisplayId) -> Option<ThemeId> {
    if is_inited(ui) {
        return get(ui);
    }
    init(ui, display)
}

/// Release an unused original simple theme (`lv_theme_simple_deinit`).
/// Returns `false` while a live display, child theme or shallow theme copy
/// references its shared resources. `Ui::deinit` also releases all theme
/// resources.
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
    state.styles.map(|style| ui.style_delete(style));
    true
}

/// `style_init()`: every `lv_style_set_*` call, in the upstream order.
fn style_init() -> Styles<Style> {
    let color_scr = palette::lighten(Palette::Grey, 4);
    let color_white = Color::WHITE;
    let color_light = palette::lighten(Palette::Grey, 2);
    let color_dark = palette::main(Palette::Grey);
    let color_dim = palette::darken(Palette::Grey, 2);

    let mut s = Styles::from_fn(Style::new);

    s.scrollbar.set_bg_opa(OPA_COVER);
    s.scrollbar.set_bg_color(color_dark);
    s.scrollbar.set_width(SCROLLBAR_WIDTH);

    s.scr.set_bg_opa(OPA_COVER);
    s.scr.set_bg_color(color_scr);
    s.scr.set_text_color(color_dim);

    s.transp.set_bg_opa(OPA_TRANSP);

    for (style, color) in [
        (&mut s.white, color_white),
        (&mut s.light, color_light),
        (&mut s.dark, color_dark),
        (&mut s.dim, color_dim),
    ] {
        style.set_bg_opa(OPA_COVER);
        style.set_bg_color(color);
        style.set_line_width(1);
        style.set_line_color(color);
        style.set_arc_width(2);
        style.set_arc_color(color);
    }

    #[cfg(feature = "arc")]
    {
        s.arc_line.set_arc_width(6);
        s.arc_knob.set_pad_all(5);
    }

    #[cfg(feature = "textarea")]
    {
        s.ta_cursor.set_border_side(BorderSide::LEFT);
        s.ta_cursor.set_border_color(color_dim);
        s.ta_cursor.set_border_width(2);
        s.ta_cursor.set_bg_opa(OPA_TRANSP);
        s.ta_cursor.set_anim_duration(500);
    }

    s
}

/// Attach `styles` in order with `lv_obj_add_style()`, stopping if a
/// STYLE_CHANGED callback deleted the object.
fn add_styles(ui: &mut Ui, obj: Obj, styles: &[(StyleId, StyleSelector)]) {
    for &(style, selector) in styles {
        if !ui.obj_is_valid(obj) {
            return;
        }
        ui.obj_add_style(obj, style, selector);
    }
}

/// `theme_apply()`. The C `else if` chain checks exact classes, so each
/// ported branch returns after adding its styles.
// With some widget features disabled, an earlier branch's `return` is last.
#[allow(clippy::needless_return)]
fn theme_apply(ui: &mut Ui, theme: ThemeId, obj: Obj) {
    if !ui.obj_is_valid(obj) {
        return;
    }
    let Some(s) = data(ui, theme).map(|data| data.styles) else {
        return;
    };
    const DEFAULT: StyleSelector = StyleSelector::DEFAULT;

    if ui.obj_get_parent(obj).is_none() {
        add_styles(
            ui,
            obj,
            &[(s.scr, DEFAULT), (s.scrollbar, Part::SCROLLBAR.into())],
        );
        return;
    }

    if ui.obj_check_type(obj, &OBJ_CLASS) {
        // The tabview and win parent cases need classes that are not ported yet.
        add_styles(
            ui,
            obj,
            &[(s.white, DEFAULT), (s.scrollbar, Part::SCROLLBAR.into())],
        );
        return;
    }

    #[cfg(feature = "button")]
    if ui.obj_check_type(obj, &crate::button::BUTTON_CLASS) {
        add_styles(ui, obj, &[(s.dark, DEFAULT)]);
        return;
    }

    #[cfg(feature = "buttonmatrix")]
    if ui.obj_check_type(obj, &crate::buttonmatrix::BUTTONMATRIX_CLASS) {
        // The msgbox and tabview parent cases need classes that are not
        // ported yet.
        add_styles(ui, obj, &[(s.white, DEFAULT), (s.light, Part::ITEMS.into())]);
        return;
    }

    #[cfg(feature = "bar")]
    if ui.obj_check_type(obj, &crate::bar::BAR_CLASS) {
        add_styles(ui, obj, &[(s.light, DEFAULT), (s.dark, Part::INDICATOR.into())]);
        return;
    }

    #[cfg(feature = "slider")]
    if ui.obj_check_type(obj, &crate::slider::SLIDER_CLASS) {
        add_styles(
            ui,
            obj,
            &[
                (s.light, DEFAULT),
                (s.dark, Part::INDICATOR.into()),
                (s.dim, Part::KNOB.into()),
            ],
        );
        return;
    }

    #[cfg(feature = "checkbox")]
    if ui.obj_check_type(obj, &crate::checkbox::CHECKBOX_CLASS) {
        add_styles(
            ui,
            obj,
            &[
                (s.light, Part::INDICATOR.into()),
                (s.dark, Part::INDICATOR | ferrovgl_core::State::CHECKED),
            ],
        );
        return;
    }

    #[cfg(feature = "switch")]
    if ui.obj_check_type(obj, &crate::switch::SWITCH_CLASS) {
        add_styles(ui, obj, &[(s.light, DEFAULT), (s.dim, Part::KNOB.into())]);
        return;
    }

    #[cfg(feature = "arc")]
    if ui.obj_check_type(obj, &crate::arc::ARC_CLASS) {
        add_styles(
            ui,
            obj,
            &[
                (s.light, DEFAULT),
                (s.transp, DEFAULT),
                (s.arc_line, DEFAULT),
                (s.dark, Part::INDICATOR.into()),
                (s.arc_line, Part::INDICATOR.into()),
                (s.dim, Part::KNOB.into()),
                (s.arc_knob, Part::KNOB.into()),
            ],
        );
        return;
    }

    #[cfg(feature = "spinner")]
    if ui.obj_check_type(obj, &crate::spinner::SPINNER_CLASS) {
        add_styles(
            ui,
            obj,
            &[
                (s.light, DEFAULT),
                (s.transp, DEFAULT),
                (s.arc_line, DEFAULT),
                (s.dark, Part::INDICATOR.into()),
                (s.arc_line, Part::INDICATOR.into()),
            ],
        );
        return;
    }

    #[cfg(feature = "textarea")]
    if ui.obj_check_type(obj, &crate::textarea::TEXTAREA_CLASS) {
        add_styles(
            ui,
            obj,
            &[
                (s.white, DEFAULT),
                (s.scrollbar, Part::SCROLLBAR.into()),
                (s.ta_cursor, Part::CURSOR | ferrovgl_core::State::FOCUSED),
            ],
        );
        return;
    }

    #[cfg(feature = "led")]
    if ui.obj_check_type(obj, &crate::led::LED_CLASS) {
        add_styles(ui, obj, &[(s.light, DEFAULT)]);
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

    /// `lv_theme_simple_init(NULL)` (upstream `test_theme.c:236`): `base.disp`
    /// stays NULL and the style change is reported even though no display
    /// uses the theme yet.
    #[test]
    fn null_display_init_always_reports_style_change() {
        let mut ui = ui();
        let display = ui.display_create(800, 480);
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        // lv_obj_report_style_change(NULL) refreshes objects having any style.
        let style = ui.style_create(Style::new());
        ui.obj_add_style(screen, style, StyleSelector::DEFAULT);
        let changes = Rc::new(core::cell::Cell::new(0u32));
        let counter = changes.clone();
        ui.obj_add_event_cb(
            screen,
            Rc::new(move |_, _| counter.set(counter.get() + 1)),
            ferrovgl_core::EventCode::STYLE_CHANGED,
            None,
        );
        assert!(!is_inited(&ui));
        assert_eq!(get(&ui), None);
        let theme = init(&mut ui, display).unwrap();
        assert_eq!(changes.get(), 0);
        assert_eq!(ui.theme(theme).unwrap().disp, Some(display));
        assert!(is_inited(&ui));
        assert_eq!(init(&mut ui, None), Some(theme));
        assert!(changes.get() > 0);
        assert_eq!(ui.theme(theme).unwrap().disp, None);
        assert_eq!(get(&ui), Some(theme));
    }

    /// `lv_display.c:141..146`: later displays get the inited global theme.
    #[test]
    fn later_displays_share_the_inited_theme() {
        let mut ui = ui();
        let a = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let theme = ui.display_get_theme(Some(a)).unwrap();
        let ids = data(&ui, theme).unwrap().styles;
        let b = ui.display_create_ex(320, 240, Some(init_display)).unwrap();
        assert_eq!(ui.display_get_theme(Some(b)), Some(theme));
        assert_eq!(ui.theme(theme).unwrap().disp, Some(a));
        assert_eq!(init(&mut ui, b), Some(theme));
        assert_eq!(ui.theme(theme).unwrap().disp, Some(b));
        assert_eq!(data(&ui, theme).unwrap().styles, ids);
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
        assert_eq!(
            [ids.scr, ids.white, ids.dark, ids.scrollbar].map(|id| ui.style(id).unwrap().len()),
            [3, 6, 6, 3]
        );
        assert_eq!(
            [ids.transp, ids.light, ids.dim].map(|id| ui.style(id).unwrap().len()),
            [1, 6, 6]
        );
        #[cfg(feature = "arc")]
        assert_eq!(
            [ids.arc_line, ids.arc_knob].map(|id| ui.style(id).unwrap().len()),
            [1, 4]
        );
        #[cfg(feature = "textarea")]
        assert_eq!(ui.style(ids.ta_cursor).unwrap().len(), 5);
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
            ferrovgl_core::State::PRESSED
                | ferrovgl_core::State::CHECKED
                | ferrovgl_core::State::DISABLED
                | ferrovgl_core::State::FOCUS_KEY,
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

    /// Apply the theme to a widget allocated without its (not yet ported)
    /// constructor, with style refresh disabled as in `lv_obj_class_init_obj`.
    /// Returns the attached styles in `lv_obj_add_style()` call order.
    #[cfg(any(feature = "bar", feature = "slider", feature = "switch", feature = "checkbox"))]
    fn applied(
        class: &'static ferrovgl_core::ObjClass,
    ) -> (alloc::vec::Vec<(StyleId, StyleSelector)>, Styles<StyleId>) {
        use ferrovgl_core::core::obj_style::ObjStyleSrc;
        let mut ui = ui();
        let display = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let theme = ui.display_get_theme(Some(display)).unwrap();
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let obj = ui.obj_class_create_obj(class, Some(screen));
        ui.obj_enable_style_refresh(false);
        ui.theme_apply(obj);
        let entries = ui
            .obj(obj)
            .unwrap()
            .styles
            .iter()
            .rev()
            .map(|entry| match entry.src {
                ObjStyleSrc::Shared(id) if entry.is_theme => (id, entry.selector),
                _ => panic!("only shared theme styles are expected"),
            })
            .collect();
        (entries, data(&ui, theme).unwrap().styles)
    }

    /// `lv_theme_simple.c:290..335`
    #[test]
    #[cfg(any(feature = "bar", feature = "slider", feature = "switch", feature = "checkbox"))]
    fn widget_branches_match_upstream_order() {
        #[cfg(feature = "bar")]
        {
            let (entries, s) = applied(&crate::bar::BAR_CLASS);
            assert_eq!(
                entries,
                [
                    (s.light, StyleSelector::DEFAULT),
                    (s.dark, Part::INDICATOR.into())
                ]
            );
        }
        #[cfg(feature = "slider")]
        {
            let (entries, s) = applied(&crate::slider::SLIDER_CLASS);
            assert_eq!(
                entries,
                [
                    (s.light, StyleSelector::DEFAULT),
                    (s.dark, Part::INDICATOR.into()),
                    (s.dim, Part::KNOB.into()),
                ]
            );
        }
        #[cfg(feature = "checkbox")]
        {
            let (entries, s) = applied(&crate::checkbox::CHECKBOX_CLASS);
            assert_eq!(
                entries,
                [
                    (s.light, Part::INDICATOR.into()),
                    (s.dark, Part::INDICATOR | ferrovgl_core::State::CHECKED),
                ]
            );
        }
        #[cfg(feature = "switch")]
        {
            let (entries, s) = applied(&crate::switch::SWITCH_CLASS);
            assert_eq!(
                entries,
                [(s.light, StyleSelector::DEFAULT), (s.dim, Part::KNOB.into())]
            );
        }
    }
}
