//! Ported from LVGL v9.6.0: `src/themes/default/lv_theme_default.c`.
//!
//! Coverage: screens, base objects (cards), buttons, and scrollbars. Labels and
//! images inherit screen text styles without own styles, as upstream; other widgets are not
//! implemented yet. Style state is owned per display by `Ui`, instead of C's
//! global default-theme pointer. Resolution changes rebuild the same shared
//! styles, preserving references held by existing objects.
//! Theme resources live until `Ui::deinit` or an explicit [`deinit`] call after
//! their displays have been deleted/detached. They remain available throughout
//! display/object deletion callbacks and can be shared by another display.

use alloc::rc::Rc;
use core::any::Any;

use ferrovgl_core::core::obj::OBJ_CLASS;
use ferrovgl_core::core::obj_style::StyleId;
use ferrovgl_core::display::dpx_calc;
use ferrovgl_core::misc::anim::AnimPath;
use ferrovgl_core::misc::color::OPA_50;
use ferrovgl_core::misc::event::{EventCallback, EventTarget};
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::style::{SharedRef, StyleTransitionDsc};
use ferrovgl_core::theme::ThemeId;
use ferrovgl_core::{
    Color, DisplayId, EventCode, FontRef, Obj, Part, State, Style, StyleProp, StyleSelector, Ui,
};

const MODE_DARK: u32 = 1;
const TRANSITION_TIME: u32 = 80;
const TRANSITION_PROPS: &[StyleProp] = &[
    StyleProp::BG_OPA,
    StyleProp::BG_COLOR,
    StyleProp::TRANSFORM_WIDTH,
    StyleProp::TRANSFORM_HEIGHT,
    StyleProp::TRANSLATE_Y,
    StyleProp::TRANSLATE_X,
    StyleProp::TRANSFORM_ROTATION,
    StyleProp::TRANSFORM_SCALE_X,
    StyleProp::TRANSFORM_SCALE_Y,
    StyleProp::RECOLOR_OPA,
    StyleProp::RECOLOR,
    StyleProp::INV,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum DisplaySize {
    Small,
    Medium,
    Large,
}

#[derive(Clone)]
struct DefaultThemeData {
    owner: ThemeId,
    display: DisplayId,
    dpi: i32,
    size: DisplaySize,
    // The first four entries remain scr, card, scrollbar, scrollbar_scrolled.
    styles: [StyleId; STYLE_COUNT],
    resolution_cb: EventCallback,
    event_data: Rc<dyn Any>,
}

const STYLE_COUNT: usize = 13;

fn data(ui: &Ui, theme: ThemeId) -> Option<&DefaultThemeData> {
    ui.theme(theme)?.extension.as_ref()?.downcast_ref()
}

/// Initialize the light default theme with blue/red accent colours and the
/// configured default font. Suitable for `Ui::display_create_ex`'s hook.
/// Returns `None` if the display is invalid or no default font is configured.
pub fn init_display(ui: &mut Ui, display: DisplayId) -> Option<ThemeId> {
    let font = ui.config().font_default.clone()?;
    init(
        ui,
        display,
        palette::main(Palette::Blue),
        palette::main(Palette::Red),
        false,
        font,
    )
}

/// `lv_theme_default_init()`, for the widget subset documented above.
/// Repeated calls reuse this display's theme and shared style IDs. The caller
/// installs the returned theme with `Ui::display_set_theme` unless using the
/// display-creation hook.
pub fn init(
    ui: &mut Ui,
    display: DisplayId,
    primary: Color,
    secondary: Color,
    dark: bool,
    font: FontRef,
) -> Option<ThemeId> {
    ui.display(display)?;
    let dpi = ui.display_get_dpi(Some(display));
    let greater_res = ui
        .display_get_horizontal_resolution(Some(display))
        .max(ui.display_get_vertical_resolution(Some(display)));
    let size = if greater_res <= 320 {
        DisplaySize::Small
    } else if greater_res < 720 {
        DisplaySize::Medium
    } else {
        DisplaySize::Large
    };
    let existing = ui.themes.iter().find_map(|(key, theme)| {
        let state = theme.extension.as_ref()?.downcast_ref::<DefaultThemeData>()?;
        (state.display == display && state.owner == ThemeId(key)).then_some(ThemeId(key))
    });
    let theme = existing.unwrap_or_else(|| ui.theme_create());
    let old = data(ui, theme).cloned();
    if let Some(old) = &old {
        let base = ui.theme(theme)?;
        if old.dpi == dpi
            && old.size == size
            && base.color_primary == primary
            && base.color_secondary == secondary
            && base.flags == u32::from(dark)
            && base.font_small.as_ref() == Some(&font)
        {
            return Some(theme);
        }
        ui.display_remove_event_cb_with_user_data(display, &old.resolution_cb, &old.event_data);
    }

    let built = build_styles(dpi, size, dark, font.clone(), primary, secondary);
    let styles = if let Some(old) = &old {
        let mut ids = old.styles;
        for (index, style) in built.into_iter().enumerate() {
            if let Some(target) = ui.style_mut(ids[index]) {
                *target = style;
            } else {
                ids[index] = ui.style_create(style);
            }
        }
        ids
    } else {
        built.map(|style| ui.style_create(style))
    };
    let resolution_cb: EventCallback =
        old.as_ref()
            .map(|old| old.resolution_cb.clone())
            .unwrap_or_else(|| {
                Rc::new(|ui, event| {
                    let EventTarget::Display(display) = event.target() else {
                        return;
                    };
                    let Some(theme) = event
                        .user_data()
                        .and_then(|data| data.downcast_ref::<ThemeId>())
                        .copied()
                    else {
                        return;
                    };
                    let Some(base) = ui.theme(theme).cloned() else {
                        return;
                    };
                    if let Some(font) = base.font_normal {
                        init(
                            ui,
                            display,
                            base.color_primary,
                            base.color_secondary,
                            base.flags & MODE_DARK != 0,
                            font,
                        );
                    }
                })
            });
    let event_data = old.map(|old| old.event_data).unwrap_or_else(|| Rc::new(theme));
    let base = ui.theme_mut(theme)?;
    base.disp = Some(display);
    base.color_primary = primary;
    base.color_secondary = secondary;
    base.font_small = Some(font.clone());
    base.font_normal = Some(font.clone());
    base.font_large = Some(font);
    base.apply = Some(apply);
    base.flags = u32::from(dark);
    base.extension = Some(Rc::new(DefaultThemeData {
        owner: theme,
        display,
        dpi,
        size,
        styles,
        resolution_cb: resolution_cb.clone(),
        event_data: event_data.clone(),
    }));
    if ui.display_get_theme(Some(display)) == Some(theme) {
        ui.obj_report_style_change(None);
    }
    if ui.display(display).is_some() {
        ui.display_add_event_cb(
            display,
            resolution_cb,
            EventCode::RESOLUTION_CHANGED,
            Some(event_data),
        );
    }
    Some(theme)
}

/// Release an unused default theme and its styles (`lv_theme_default_deinit`).
///
/// Returns `false` if this is not the original theme or if a live display,
/// child theme or shallow theme copy still references it. Delete/detach those
/// users first. This keeps display/object deletion callbacks able to read their
/// styles; deleting a display does not implicitly invalidate a shared theme.
/// `Ui::deinit` releases all themes/styles without needing this call.
pub fn deinit(ui: &mut Ui, theme: ThemeId) -> bool {
    let Some(state) = data(ui, theme).filter(|state| state.owner == theme).cloned() else {
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
                        .and_then(|data| data.downcast_ref::<DefaultThemeData>())
                        .is_some_and(|data| data.owner == theme))
        })
    {
        return false;
    }
    if ui.display(state.display).is_some() {
        ui.display_remove_event_cb_with_user_data(state.display, &state.resolution_cb, &state.event_data);
    }
    ui.theme_delete(theme);
    for style in state.styles {
        ui.style_delete(style);
    }
    true
}

fn build_styles(
    dpi: i32,
    size: DisplaySize,
    dark: bool,
    font: FontRef,
    primary: Color,
    secondary: Color,
) -> [Style; STYLE_COUNT] {
    let color_scr = if dark {
        Color::hex(0x15171a)
    } else {
        palette::lighten(Palette::Grey, 4)
    };
    let color_text = if dark {
        palette::lighten(Palette::Grey, 5)
    } else {
        palette::darken(Palette::Grey, 4)
    };
    let color_card = if dark { Color::hex(0x282b30) } else { Color::WHITE };
    let color_grey = if dark {
        Color::hex(0x2f3237)
    } else {
        palette::lighten(Palette::Grey, 2)
    };
    let pad_def = dpx_calc(
        dpi,
        match size {
            DisplaySize::Large => 24,
            DisplaySize::Medium => 20,
            DisplaySize::Small => 16,
        },
    );
    let pad_small = dpx_calc(
        dpi,
        match size {
            DisplaySize::Large => 14,
            DisplaySize::Medium => 12,
            DisplaySize::Small => 10,
        },
    );

    let mut scrollbar = Style::new();
    scrollbar.set_bg_color(if dark {
        palette::darken(Palette::Grey, 2)
    } else {
        palette::main(Palette::Grey)
    });
    scrollbar.set_radius(ferrovgl_core::draw::rect::RADIUS_CIRCLE);
    scrollbar.set_pad_all(dpx_calc(dpi, 7));
    scrollbar.set_width(dpx_calc(dpi, 5));
    scrollbar.set_bg_opa(102);
    let normal = SharedRef::Shared(Rc::new(StyleTransitionDsc::new_static(
        TRANSITION_PROPS,
        AnimPath::Linear,
        TRANSITION_TIME,
        0,
    )));
    scrollbar.set_transition(normal.clone());
    let mut scrolled = Style::new();
    scrolled.set_bg_opa(255);

    let mut screen = Style::new();
    screen.set_bg_opa(255);
    screen.set_bg_color(color_scr);
    screen.set_text_color(color_text);
    screen.set_text_font(font);
    screen.set_pad_row(pad_small);
    screen.set_pad_column(pad_small);
    screen.set_rotary_sensitivity((dpi / 4 * 256) as u32);

    let mut card = Style::new();
    card.set_radius(dpx_calc(dpi, if size == DisplaySize::Large { 12 } else { 8 }));
    card.set_bg_opa(255);
    card.set_bg_color(color_card);
    card.set_border_color(color_grey);
    card.set_border_width(dpx_calc(dpi, 2));
    card.set_border_post(true);
    card.set_text_color(color_text);
    card.set_pad_all(pad_def);
    card.set_pad_row(pad_small);
    card.set_pad_column(pad_small);
    card.set_line_color(palette::main(Palette::Grey));
    card.set_line_width(dpx_calc(dpi, 1));
    let mut button = Style::new();
    button.set_radius(dpx_calc(
        dpi,
        match size {
            DisplaySize::Large => 16,
            DisplaySize::Medium => 12,
            DisplaySize::Small => 8,
        },
    ));
    button.set_bg_opa(255);
    button.set_bg_color(color_grey);
    if !dark {
        button.set_shadow_color(palette::main(Palette::Grey));
        button.set_shadow_width(dpx_calc(dpi, 3));
        button.set_shadow_opa(OPA_50);
        button.set_shadow_offset_y(dpx_calc(dpi, 3));
    }
    button.set_text_color(color_text);
    button.set_pad_hor(pad_def);
    button.set_pad_ver(pad_small);
    button.set_pad_column(dpx_calc(dpi, 5));
    button.set_pad_row(dpx_calc(dpi, 5));
    let mut primary_style = Style::new();
    primary_style.set_bg_color(primary);
    primary_style.set_text_color(Color::WHITE);
    primary_style.set_bg_opa(255);
    let mut secondary_style = Style::new();
    secondary_style.set_bg_color(secondary);
    secondary_style.set_text_color(Color::WHITE);
    secondary_style.set_bg_opa(255);
    let mut delayed = Style::new();
    delayed.set_transition(SharedRef::Shared(Rc::new(StyleTransitionDsc::new_static(
        TRANSITION_PROPS,
        AnimPath::Linear,
        TRANSITION_TIME,
        70,
    ))));
    let mut normal_style = Style::new();
    normal_style.set_transition(normal);
    let mut pressed = Style::new();
    pressed.set_recolor(Color::BLACK);
    pressed.set_recolor_opa(35);
    let mut outline = Style::new();
    outline.set_outline_color(primary);
    outline.set_outline_width(dpx_calc(dpi, 3));
    outline.set_outline_pad(dpx_calc(dpi, 3));
    outline.set_outline_opa(OPA_50);
    // LV_THEME_DEFAULT_GROW=1 in the pinned reference configuration.
    let mut grow = Style::new();
    grow.set_transform_width(dpx_calc(dpi, 3));
    grow.set_transform_height(dpx_calc(dpi, 3));
    let mut disabled = Style::new();
    disabled.set_recolor(if dark {
        palette::darken(Palette::Grey, 2)
    } else {
        palette::lighten(Palette::Grey, 2)
    });
    disabled.set_recolor_opa(OPA_50);
    [
        screen,
        card,
        scrollbar,
        scrolled,
        button,
        primary_style,
        delayed,
        pressed,
        normal_style,
        outline,
        grow,
        secondary_style,
        disabled,
    ]
}

fn apply(ui: &mut Ui, theme: ThemeId, obj: Obj) {
    if !ui.obj_is_valid(obj) {
        return;
    }
    let Some(styles) = data(ui, theme).map(|data| data.styles) else {
        return;
    };
    if ui.obj_get_parent(obj).is_none() || ui.obj_check_type(obj, &OBJ_CLASS) {
        let style = if ui.obj_get_parent(obj).is_none() {
            styles[0]
        } else {
            styles[1]
        };
        for (style, selector) in [
            (style, StyleSelector::DEFAULT),
            (styles[2], Part::SCROLLBAR | State::DEFAULT),
            (styles[3], Part::SCROLLBAR | State::SCROLLED),
        ] {
            // A STYLE_CHANGED callback can delete the object while applying
            // a style. Keep the C order for surviving objects.
            if !ui.obj_is_valid(obj) {
                return;
            }
            ui.obj_add_style(obj, style, selector);
        }
        #[cfg(feature = "button")]
        return;
    }
    #[cfg(feature = "button")]
    if ui.obj_check_type(obj, &crate::button::BUTTON_CLASS) {
        const BUTTON: usize = 4;
        const PRIMARY: usize = 5;
        const TRANSITION_DELAYED: usize = 6;
        const PRESSED: usize = 7;
        const TRANSITION_NORMAL: usize = 8;
        const OUTLINE_PRIMARY: usize = 9;
        const GROW: usize = 10;
        const SECONDARY: usize = 11;
        const DISABLED: usize = 12;
        for (index, state) in [
            (BUTTON, State::DEFAULT),
            (PRIMARY, State::DEFAULT),
            (TRANSITION_DELAYED, State::DEFAULT),
            (PRESSED, State::PRESSED),
            (TRANSITION_NORMAL, State::PRESSED),
            (OUTLINE_PRIMARY, State::FOCUS_KEY),
            (GROW, State::PRESSED),
            (SECONDARY, State::CHECKED),
            (DISABLED, State::DISABLED),
        ] {
            if !ui.obj_is_valid(obj) {
                return;
            }
            ui.obj_add_style(obj, styles[index], Part::MAIN | state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;
    use ferrovgl_core::UiConfig;

    fn ui() -> Ui {
        Ui::new(UiConfig {
            dpi_def: 160,
            font_default: Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14)),
            ..UiConfig::default()
        })
    }

    #[test]
    fn reapplying_theme_stops_when_first_style_callback_deletes_the_object() {
        let mut ui = ui();
        let display = ui.display_create_ex(320, 240, Some(init_display)).unwrap();
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let base = ui.obj_create(Some(screen));
        #[cfg(feature = "button")]
        let objects = [base, crate::Button::create(&mut ui, screen).obj()];
        #[cfg(not(feature = "button"))]
        let objects = [base];
        for obj in objects {
            ui.obj_add_event_cb(
                obj,
                Rc::new(move |ui, _| {
                    // theme_apply removes the old styles first; delete only when
                    // the first replacement style has been attached.
                    if ui.obj(obj).is_some_and(|data| !data.styles.is_empty()) {
                        ui.obj_delete(obj);
                    }
                }),
                EventCode::STYLE_CHANGED,
                None,
            );
            ui.theme_apply(obj);
            assert!(!ui.obj_is_valid(obj));
        }
    }

    #[test]
    fn light_screen_and_card_match_upstream_large_display() {
        let mut ui = ui();
        let display = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        assert_eq!(
            ui.obj_get_style_bg_color(screen, Part::MAIN),
            Color::hex(0xf5f5f5)
        );
        assert_eq!(
            ui.obj_get_style_text_color(screen, Part::MAIN),
            Color::hex(0x212121)
        );
        assert_eq!(ui.obj_get_style_pad_left(screen, Part::MAIN), 0);
        assert_eq!(ui.obj_get_style_pad_row(screen, Part::MAIN), 14);
        assert_eq!(ui.obj_get_style_rotary_sensitivity(screen, Part::MAIN), 10240);
        assert_eq!(ui.obj_get_style_bg_opa(screen, Part::SCROLLBAR), 102);
        assert_eq!(ui.obj_get_style_width(screen, Part::SCROLLBAR), 5);
        let card = ui.obj_create(Some(screen));
        assert_eq!(ui.obj_get_style_radius(card, Part::MAIN), 12);
        assert_eq!(ui.obj_get_style_pad_left(card, Part::MAIN), 24);
        assert_eq!(ui.obj_get_style_border_width(card, Part::MAIN), 2);
        assert!(ui.obj_get_style_border_post(card, Part::MAIN));
        assert_eq!(ui.obj_get_style_bg_color(card, Part::MAIN), Color::WHITE);
        assert!(ui.obj(card).unwrap().styles.iter().all(|style| style.is_theme));
        ui.deinit();
        assert!(ui.styles.is_empty());
        assert!(ui.themes.is_empty());
    }

    /// `lv_theme_default.c:32..48, 242..285, 651..655`: both palettes,
    /// portrait/landscape size thresholds, and the integer DPX rounding.
    #[test]
    fn theme_palette_dpi_and_size_boundaries() {
        for (width, height, radius, pad, gap) in [
            (320, 240, 8, 16, 10),
            (240, 320, 8, 16, 10),
            (321, 240, 8, 20, 12),
            (240, 719, 8, 20, 12),
            (720, 480, 12, 24, 14),
            (480, 720, 12, 24, 14),
        ] {
            for dpi in [80, 161, 240] {
                for dark in [false, true] {
                    let mut ui = ui();
                    let display = ui.display_create_ex(width, height, Some(init_display)).unwrap();
                    ui.display_set_dpi(display, dpi);
                    let font = ui.config().font_default.clone().unwrap();
                    let theme = init(&mut ui, display, Color::BLACK, Color::WHITE, dark, font).unwrap();
                    let screen = ui.display_get_screen_active(Some(display)).unwrap();
                    let card = ui.obj_create(Some(screen));
                    let dpx = |n: i32| ((dpi * n + 80) / 160).max(1);
                    assert_eq!(ui.obj_get_style_radius(card, Part::MAIN), dpx(radius));
                    assert_eq!(ui.obj_get_style_pad_left(card, Part::MAIN), dpx(pad));
                    assert_eq!(ui.obj_get_style_pad_row(screen, Part::MAIN), dpx(gap));
                    assert_eq!(ui.obj_get_style_pad_column(card, Part::MAIN), dpx(gap));
                    assert_eq!(ui.obj_get_style_border_width(card, Part::MAIN), dpx(2));
                    assert_eq!(ui.obj_get_style_line_width(card, Part::MAIN), dpx(1));
                    assert_eq!(ui.obj_get_style_width(card, Part::SCROLLBAR), dpx(5));
                    assert_eq!(ui.obj_get_style_pad_bottom(card, Part::SCROLLBAR), dpx(7));
                    // Division precedes multiplication in C, including odd DPI.
                    assert_eq!(
                        ui.obj_get_style_rotary_sensitivity(screen, Part::MAIN),
                        (dpi / 4 * 256) as u32
                    );
                    assert_eq!(
                        ui.obj_get_style_bg_color(screen, Part::MAIN),
                        Color::hex(if dark { 0x15171a } else { 0xf5f5f5 })
                    );
                    assert_eq!(
                        ui.obj_get_style_bg_color(card, Part::MAIN),
                        Color::hex(if dark { 0x282b30 } else { 0xffffff })
                    );
                    assert_eq!(
                        ui.obj_get_style_text_color(card, Part::MAIN),
                        Color::hex(if dark { 0xfafafa } else { 0x212121 })
                    );
                    assert_eq!(
                        ui.obj_get_style_border_color(card, Part::MAIN),
                        Color::hex(if dark { 0x2f3237 } else { 0xe0e0e0 })
                    );
                    assert_eq!(
                        ui.obj_get_style_bg_color(card, Part::SCROLLBAR),
                        Color::hex(if dark { 0x616161 } else { 0x9e9e9e })
                    );
                    assert_eq!(
                        ui.obj_get_style_line_color(card, Part::MAIN),
                        Color::hex(0x9e9e9e)
                    );
                    let ids = data(&ui, theme).unwrap().styles;
                    assert_eq!(
                        ids[..4]
                            .iter()
                            .map(|id| ui.style(*id).unwrap().len())
                            .collect::<alloc::vec::Vec<_>>(),
                        [7, 15, 9, 1]
                    );
                }
            }
        }
    }

    /// Exact application order in `lv_theme_default.c:751..821`; later styles
    /// are prepended, and the state style must affect only the scrollbar part.
    #[test]
    fn theme_selector_order_and_scrollbar_transition() {
        use ferrovgl_core::core::obj_style::ObjStyleSrc;

        let mut ui = ui();
        let display = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let theme = ui.display_get_theme(Some(display)).unwrap();
        let [screen_style, card_style, scrollbar, scrolled]: [StyleId; 4] =
            data(&ui, theme).unwrap().styles[..4].try_into().unwrap();
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let card = ui.obj_create(Some(screen));
        for (obj, main) in [(screen, screen_style), (card, card_style)] {
            let styles = &ui.obj(obj).unwrap().styles;
            assert_eq!(styles.len(), 3);
            for (entry, (id, selector)) in styles.iter().zip([
                (scrolled, Part::SCROLLBAR | State::SCROLLED),
                (scrollbar, Part::SCROLLBAR | State::DEFAULT),
                (main, StyleSelector::DEFAULT),
            ]) {
                assert!(matches!(entry.src, ObjStyleSrc::Shared(actual) if actual == id));
                assert_eq!(entry.selector, selector);
                assert!(entry.is_theme);
            }
            let transition = ui.obj_get_style_transition(obj, Part::SCROLLBAR).unwrap();
            assert_eq!(transition.time, 80);
            assert_eq!(transition.delay, 0);
            assert_eq!(transition.props.as_ref(), TRANSITION_PROPS);
            ui.obj_add_state(obj, State::SCROLLED); // unrendered: no animation yet
            assert_eq!(ui.obj_get_style_bg_opa(obj, Part::SCROLLBAR), 255);
            assert_eq!(ui.obj_get_style_bg_opa(obj, Part::MAIN), 255);
            ui.obj_remove_state(obj, State::SCROLLED);
            assert_eq!(ui.obj_get_style_bg_opa(obj, Part::SCROLLBAR), 102);
        }
    }

    /// Button branches from `lv_theme_default.c:287..310, 342..354, 830..843`.
    /// Rebuild the same styles on dark-mode/DPI/size changes so old buttons lose
    /// light-only shadows and use the new palette without being recreated.
    #[test]
    #[cfg(feature = "button")]
    fn button_states_and_reinitialization_update_existing_objects() {
        let mut ui = ui();
        let display = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let theme = ui.display_get_theme(Some(display)).unwrap();
        let original_ids = data(&ui, theme).unwrap().styles;
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let button = crate::Button::create(&mut ui, screen).obj();
        assert_eq!(ui.obj(button).unwrap().styles.len(), 9);
        assert!(ui.obj(button).unwrap().styles.iter().all(|style| style.is_theme));
        assert_eq!(ui.obj_get_style_radius(button, Part::MAIN), 16);
        assert_eq!(ui.obj_get_style_pad_left(button, Part::MAIN), 24);
        assert_eq!(ui.obj_get_style_pad_top(button, Part::MAIN), 14);
        assert_eq!(ui.obj_get_style_pad_row(button, Part::MAIN), 5);
        assert_eq!(ui.obj_get_style_shadow_width(button, Part::MAIN), 3);
        assert_eq!(ui.obj_get_style_shadow_offset_y(button, Part::MAIN), 3);
        assert_eq!(ui.obj_get_style_shadow_opa(button, Part::MAIN), 127);
        assert_eq!(ui.obj_get_style_border_width(button, Part::MAIN), 0);
        assert_eq!(ui.obj_get_style_bg_opa(button, Part::SCROLLBAR), 0);
        assert_eq!(ui.obj_get_style_transition(button, Part::MAIN).unwrap().delay, 70);

        ui.obj_add_state(button, State::PRESSED | State::FOCUS_KEY);
        assert_eq!(ui.obj_get_style_transition(button, Part::MAIN).unwrap().delay, 0);
        assert_eq!(ui.obj_get_style_transform_width(button, Part::MAIN), 3);
        assert_eq!(ui.obj_get_style_transform_height(button, Part::MAIN), 3);
        assert_eq!(ui.obj_get_style_recolor_opa(button, Part::MAIN), 35);
        assert_eq!(ui.obj_get_style_outline_width(button, Part::MAIN), 3);
        assert_eq!(ui.obj_get_style_outline_pad(button, Part::MAIN), 3);
        assert_eq!(ui.obj_get_style_outline_opa(button, Part::MAIN), 127);
        ui.obj_add_state(button, State::CHECKED | State::DISABLED);
        assert_eq!(
            ui.obj_get_style_bg_color(button, Part::MAIN),
            palette::main(Palette::Red)
        );
        assert_eq!(ui.obj_get_style_recolor_opa(button, Part::MAIN), 127);
        assert_eq!(
            ui.obj_get_style_recolor(button, Part::MAIN),
            palette::lighten(Palette::Grey, 2)
        );

        let font = ui.config().font_default.clone().unwrap();
        init(
            &mut ui,
            display,
            Color::hex(0x123456),
            Color::hex(0x654321),
            true,
            font,
        );
        assert_eq!(data(&ui, theme).unwrap().styles, original_ids);
        assert_eq!(ui.obj_get_style_shadow_width(button, Part::MAIN), 0);
        assert_eq!(
            ui.obj_get_style_recolor(button, Part::MAIN),
            palette::darken(Palette::Grey, 2)
        );
        assert_eq!(
            ui.obj_get_style_outline_color(button, Part::MAIN),
            Color::hex(0x123456)
        );
        assert_eq!(
            ui.obj_get_style_bg_color(button, Part::MAIN),
            Color::hex(0x654321)
        );
        ui.obj_remove_state(
            button,
            State::PRESSED | State::FOCUS_KEY | State::CHECKED | State::DISABLED,
        );
        assert_eq!(
            ui.obj_get_style_bg_color(button, Part::MAIN),
            Color::hex(0x123456)
        );
        ui.display_set_resolution(display, 320, 240);
        assert_eq!(ui.obj_get_style_radius(button, Part::MAIN), 8);
        assert_eq!(ui.obj_get_style_pad_left(button, Part::MAIN), 16);
        assert_eq!(ui.obj_get_style_pad_top(button, Part::MAIN), 10);
        assert_eq!(data(&ui, theme).unwrap().styles, original_ids);
    }

    #[test]
    fn reinitialization_preserves_style_ids_user_data_and_one_callback() {
        let mut ui = ui();
        let display = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let theme = ui.display_get_theme(Some(display)).unwrap();
        let before = data(&ui, theme).unwrap().styles;
        let events = ui.display_get_event_count(display);
        ui.theme_set_user_data(theme, Some(Box::new(42u32)));
        assert_eq!(init_display(&mut ui, display), Some(theme));
        assert_eq!(data(&ui, theme).unwrap().styles, before);
        let font = ui.config().font_default.clone().unwrap();
        assert_eq!(
            init(&mut ui, display, Color::BLACK, Color::WHITE, true, font),
            Some(theme)
        );
        assert_eq!(data(&ui, theme).unwrap().styles, before);
        assert_eq!(ui.display_get_event_count(display), events);
        assert_eq!(
            ui.theme_get_user_data(theme).unwrap().downcast_ref::<u32>(),
            Some(&42)
        );
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        assert_eq!(
            ui.obj_get_style_bg_color(screen, Part::MAIN),
            Color::hex(0x15171a)
        );
        assert_eq!(
            ui.obj_get_style_text_color(screen, Part::MAIN),
            palette::lighten(Palette::Grey, 5)
        );
    }

    #[test]
    fn resolution_change_updates_existing_card_and_isolates_displays() {
        let mut ui = ui();
        let a = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let b = ui.display_create_ex(320, 240, Some(init_display)).unwrap();
        let ta = ui.display_get_theme(Some(a)).unwrap();
        let tb = ui.display_get_theme(Some(b)).unwrap();
        assert_ne!(ta, tb);
        let screen = ui.display_get_screen_active(Some(a)).unwrap();
        let card = ui.obj_create(Some(screen));
        let before = data(&ui, ta).unwrap().styles;
        ui.display_set_resolution(a, 320, 240);
        assert_eq!(data(&ui, ta).unwrap().styles, before);
        assert_eq!(ui.obj_get_style_radius(card, Part::MAIN), 8);
        assert_eq!(ui.obj_get_style_pad_left(card, Part::MAIN), 16);
        assert_eq!(data(&ui, tb).unwrap().dpi, 160);
    }

    #[test]
    fn copied_theme_cannot_replace_the_default_theme_identity() {
        let mut ui = ui();
        let copy = ui.theme_create();
        let display = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let original = ui.display_get_theme(Some(display)).unwrap();
        ui.theme_copy(copy, original);
        assert_eq!(init_display(&mut ui, display), Some(original));
    }

    #[test]
    fn unused_theme_release_bounds_repeated_display_allocations() {
        let mut ui = ui();
        for _ in 0..3 {
            let display = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
            let theme = ui.display_get_theme(Some(display)).unwrap();
            assert!(!deinit(&mut ui, theme));
            ui.display_delete(display);
            assert!(deinit(&mut ui, theme));
            assert!(ui.themes.is_empty());
            assert!(ui.styles.is_empty());
        }
    }

    #[test]
    fn shared_displays_and_theme_copies_keep_resources_alive() {
        let mut ui = ui();
        let a = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let original = ui.display_get_theme(Some(a)).unwrap();
        let b = ui.display_create(320, 240);
        ui.display_set_theme(b, Some(original));
        ui.display_delete(a);
        assert!(!deinit(&mut ui, original));
        assert_eq!(ui.display_get_theme(Some(b)), Some(original));
        assert_eq!(ui.theme_get_color_primary(None), palette::main(Palette::Blue));
        let copy = ui.theme_create();
        ui.theme_copy(copy, original);
        ui.display_delete(b);
        assert!(!deinit(&mut ui, original));
        ui.theme_delete(copy);
        let child = ui.theme_create();
        ui.theme_set_parent(child, Some(original));
        assert!(!deinit(&mut ui, original));
        ui.theme_delete(child);
        assert!(deinit(&mut ui, original));
        assert!(ui.styles.is_empty());
    }

    #[cfg(feature = "label")]
    #[test]
    fn labels_inherit_text_without_card_styles() {
        let mut ui = ui();
        let display = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let label = crate::Label::create(&mut ui, screen).obj();
        assert!(ui.obj(label).unwrap().styles.is_empty());
        assert_eq!(ui.obj_get_style_bg_opa(label, Part::MAIN), 0);
        assert_eq!(ui.obj_get_style_pad_left(label, Part::MAIN), 0);
        assert_eq!(
            ui.obj_get_style_text_color(label, Part::MAIN),
            Color::hex(0x212121)
        );
        assert_eq!(
            ui.obj_get_style_text_font(label, Part::MAIN),
            ui.config().font_default
        );
    }
}
