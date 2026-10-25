//! Ported from LVGL v9.6.0: `src/themes/default/lv_theme_default.c`.
//!
//! `style_init()` builds the complete `my_theme_styles_t` set for the parity
//! configuration (`LV_THEME_DEFAULT_GROW=1`, `LV_THEME_DEFAULT_TRANSITION_TIME`
//! 80 ms). Styles of widgets that have a crate feature follow its
//! `#if LV_USE_<WIDGET>`; styles of widgets without a feature yet (chart,
//! dropdown, menu, table, calendar, msgbox, keyboard, list, tabview, scale) are
//! always built, as with those widgets enabled in the reference configuration.
//!
//! `theme_apply()` covers screens, base objects (cards), buttons, bars,
//! sliders, switches and checkboxes. Labels and images receive no own styles,
//! as upstream. Branches that need classes which are not ported yet (tabview,
//! win, calendar, menu, ...) are omitted. Each `Ui` owns one default theme,
//! like C's global `theme_def` (`LV_GLOBAL_DEFAULT()->theme_default`): every
//! display created with [`init_display`] after the first one shares it, and
//! [`init`] re-initializes that single instance for the display it is given.
//! Resolution changes rebuild the same shared styles, preserving references
//! held by existing objects. Theme resources live until `Ui::deinit` or an
//! explicit [`deinit`] call after their displays have been deleted/detached.
//! They remain available throughout display/object deletion callbacks.

use alloc::rc::Rc;
use core::any::Any;

use ferrovgl_core::core::obj::OBJ_CLASS;
use ferrovgl_core::core::obj_style::StyleId;
use ferrovgl_core::display::dpx_calc;
use ferrovgl_core::draw::rect::RADIUS_CIRCLE;
use ferrovgl_core::misc::anim::AnimPath;
use ferrovgl_core::misc::check_arg_failed;
use ferrovgl_core::misc::color::{OPA_10, OPA_20, OPA_40, OPA_50, OPA_COVER, OPA_TRANSP};
use ferrovgl_core::misc::event::{EventCallback, EventTarget};
use ferrovgl_core::misc::palette::{self, Palette};
#[cfg(feature = "checkbox")]
use ferrovgl_core::misc::style::ImageSrc;
use ferrovgl_core::misc::style::{BorderSide, SharedRef, StyleTransitionDsc};
use ferrovgl_core::misc::text::TextAlign;
use ferrovgl_core::theme::ThemeId;
use ferrovgl_core::{
    Color, DisplayId, EventCode, FontRef, Obj, Part, State, Style, StyleProp, StyleSelector, Ui,
};

/// `MODE_DARK`
const MODE_DARK: u32 = 1;
/// `TRANSITION_TIME` = `LV_THEME_DEFAULT_TRANSITION_TIME` (80 in the reference build).
const TRANSITION_TIME: u32 = 80;
/// `trans_props` of `style_init()` (without the C terminator `0`).
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
/// `LV_SYMBOL_OK` (`"\xEF\x80\x8C"`).
#[cfg(feature = "checkbox")]
const SYMBOL_OK: &str = "\u{F00C}";

/// `disp_size_t`, with the upstream discriminants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum DisplaySize {
    /// `DISP_SMALL`
    Small = 3,
    /// `DISP_MEDIUM`
    Medium = 2,
    /// `DISP_LARGE`
    Large = 1,
}

theme_styles! {
    /// `my_theme_styles_t`, in C declaration order.
    struct Styles {
        scr,
        scrollbar,
        scrollbar_scrolled,
        card,
        btn,

        // Utility
        bg_color_primary,
        bg_color_primary_muted,
        bg_color_secondary,
        bg_color_secondary_muted,
        bg_color_grey,
        bg_color_white,
        pressed,
        disabled,
        pad_zero,
        pad_tiny,
        pad_small,
        pad_normal,
        pad_gap,
        line_space_large,
        text_align_center,
        outline_primary,
        outline_secondary,
        circle,
        no_radius,
        clip_corner,
        rotary_scroll,
        // LV_THEME_DEFAULT_GROW
        grow,
        transition_delayed,
        transition_normal,
        anim,
        anim_fast,

        // Parts
        knob,

        #[cfg(feature = "arc")]
        arc_indic,
        #[cfg(feature = "arc")]
        arc_indic_primary,

        // LV_USE_CHART
        chart_series,
        chart_indic,
        chart_bg,

        // LV_USE_DROPDOWN
        dropdown_list,

        #[cfg(feature = "checkbox")]
        cb_marker,
        #[cfg(feature = "checkbox")]
        cb_marker_checked,

        #[cfg(feature = "switch")]
        switch_knob,

        #[cfg(feature = "line")]
        line,

        // LV_USE_TABLE
        table_cell,

        #[cfg(feature = "textarea")]
        ta_cursor,
        #[cfg(feature = "textarea")]
        ta_placeholder,

        // LV_USE_CALENDAR
        calendar_btnm_bg,
        calendar_btnm_day,
        calendar_header,

        // LV_USE_MENU
        menu_bg,
        menu_cont,
        menu_sidebar_cont,
        menu_main_cont,
        menu_page,
        menu_header_cont,
        menu_header_btn,
        menu_section,
        menu_pressed,
        menu_separator,

        // LV_USE_MSGBOX
        msgbox_backdrop_bg,

        // LV_USE_KEYBOARD
        keyboard_button_bg,

        // LV_USE_LIST
        list_bg,
        list_btn,
        list_item_grow,

        // LV_USE_TABVIEW
        tab_bg_focus,
        tab_btn,

        #[cfg(feature = "led")]
        led,

        // LV_USE_SCALE
        scale,
    }
}

#[derive(Clone)]
struct DefaultThemeData {
    /// The original theme (`theme_def`); a shallow `lv_theme_copy` keeps it.
    owner: ThemeId,
    /// `disp_dpi`
    dpi: i32,
    /// `disp_size`
    size: DisplaySize,
    styles: Styles<StyleId>,
    /// `resolution_change_event_cb`
    resolution_cb: EventCallback,
    /// The callback's user data (the `theme` pointer in C).
    event_data: Rc<dyn Any>,
}

fn data(ui: &Ui, theme: ThemeId) -> Option<&DefaultThemeData> {
    ui.theme(theme)?.extension.as_ref()?.downcast_ref()
}

/// `theme_def`: the single default theme of this `Ui`
/// (`LV_GLOBAL_DEFAULT()->theme_default`), or `None` before the first
/// [`init`] / after [`deinit`].
fn theme_def(ui: &Ui) -> Option<ThemeId> {
    ui.themes.iter().find_map(|(key, theme)| {
        let state = theme.extension.as_ref()?.downcast_ref::<DefaultThemeData>()?;
        (state.owner == ThemeId(key)).then_some(ThemeId(key))
    })
}

/// `lv_theme_default_is_inited()`: whether this `Ui`'s default theme exists.
pub fn is_inited(ui: &Ui) -> bool {
    theme_def(ui).is_some()
}

/// `lv_theme_default_get()`: this `Ui`'s default theme, or `None` if it is not
/// initialized.
pub fn get(ui: &Ui) -> Option<ThemeId> {
    if !is_inited(ui) {
        return None;
    }
    theme_def(ui)
}

/// The `LV_USE_THEME_DEFAULT` block of `lv_display_create()`
/// (`lv_display.c:133..139`): if the default theme is inited, return it
/// unchanged ([`get`]); otherwise [`init`] it for `display` with blue/red accent
/// colours, light mode (`LV_THEME_DEFAULT_DARK` 0) and `LV_FONT_DEFAULT`.
/// Suitable for `Ui::display_create_ex`'s hook. Returns `None` if the display
/// is invalid or no default font is configured.
pub fn init_display(ui: &mut Ui, display: DisplayId) -> Option<ThemeId> {
    if is_inited(ui) {
        return get(ui);
    }
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
///
/// There is one default theme per `Ui`, like C's global `theme_def`: every
/// call returns it and, unless nothing changed, rebuilds its shared styles in
/// place for `display` (`None` falls back to `lv_display_get_default()`). The
/// resolution callback is added to the display of this call only. The caller
/// installs the returned theme with `Ui::display_set_theme` unless using the
/// display-creation hook.
pub fn init(
    ui: &mut Ui,
    display: impl Into<Option<DisplayId>>,
    primary: Color,
    secondary: Color,
    dark: bool,
    font: FontRef,
) -> Option<ThemeId> {
    // LV_CHECK_ARG(font != NULL): a `FontRef` is never NULL.
    // if(!disp) disp = lv_display_get_default();
    let Some(disp) = display.into().or_else(|| ui.display_get_default()) else {
        check_arg_failed("lv_theme_default_init: disp != NULL");
        return None;
    };
    ui.display(disp)?;

    // if(!lv_theme_default_is_inited()) theme_def = lv_malloc_zeroed(...)
    let theme = theme_def(ui).unwrap_or_else(|| ui.theme_create());
    let old = data(ui, theme).cloned();

    let new_disp = disp;
    let new_dpi = ui.display_get_dpi(Some(new_disp));
    let hor_res = ui.display_get_horizontal_resolution(Some(new_disp));
    let ver_res = ui.display_get_vertical_resolution(Some(new_disp));
    let greater_res = hor_res.max(ver_res);
    let new_size = if greater_res <= 320 {
        DisplaySize::Small
    } else if greater_res < 720 {
        DisplaySize::Medium
    } else {
        DisplaySize::Large
    };

    /* check theme information whether will change or not*/
    if let Some(old) = &old {
        let base = ui.theme(theme)?;
        if old.dpi == new_dpi
            && old.size == new_size
            && base.color_primary == primary
            && base.color_secondary == secondary
            && base.flags == if dark { MODE_DARK } else { 0 }
            && base.font_small.as_ref() == Some(&font)
        {
            return Some(theme);
        }
    }

    let resolution_cb: EventCallback =
        old.as_ref()
            .map(|old| old.resolution_cb.clone())
            .unwrap_or_else(|| {
                // resolution_change_event_cb()
                Rc::new(|ui, event| {
                    let EventTarget::Display(disp) = event.target() else {
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
                            disp,
                            base.color_primary,
                            base.color_secondary,
                            base.flags & MODE_DARK != 0,
                            font,
                        );
                    }
                })
            });
    let event_data = old
        .as_ref()
        .map(|old| old.event_data.clone())
        .unwrap_or_else(|| Rc::new(theme));

    let base = ui.theme_mut(theme)?;
    base.disp = Some(new_disp);
    base.color_primary = primary;
    base.color_secondary = secondary;
    base.font_small = Some(font.clone());
    base.font_normal = Some(font.clone());
    base.font_large = Some(font.clone());
    base.apply = Some(theme_apply);
    base.flags = if dark { MODE_DARK } else { 0 };

    /*Remove the callback before triggering style refresh to prevent
     *resolution_change_event_cb from re-entering lv_theme_default_init
     *during lv_obj_report_style_change. Re-added below.*/
    ui.display_remove_event_cb_with_user_data(new_disp, &resolution_cb, &event_data);

    let built = style_init(&StyleParams {
        dpi: new_dpi,
        size: new_size,
        dark,
        font,
        primary,
        secondary,
        dpi_def: ui.config().dpi_def,
    });
    // style_init_reset(): reset the existing shared styles in place.
    let styles = if let Some(old) = &old {
        old.styles.zip(built).map(|(id, style)| {
            if let Some(target) = ui.style_mut(id) {
                *target = style;
                id
            } else {
                ui.style_create(style)
            }
        })
    } else {
        built.map(|style| ui.style_create(style))
    };
    // theme->inited = true;
    ui.theme_mut(theme)?.extension = Some(Rc::new(DefaultThemeData {
        owner: theme,
        dpi: new_dpi,
        size: new_size,
        styles,
        resolution_cb: resolution_cb.clone(),
        event_data: event_data.clone(),
    }));

    if ui.display_get_theme(Some(disp)) == Some(theme) {
        ui.obj_report_style_change(None);
    }

    if ui.display(new_disp).is_some() {
        ui.display_add_event_cb(
            new_disp,
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
/// The resolution callback is removed from every display that still has it.
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
    let mut disp = ui.display_get_next(None);
    while let Some(d) = disp {
        ui.display_remove_event_cb_with_user_data(d, &state.resolution_cb, &state.event_data);
        disp = ui.display_get_next(Some(d));
    }
    ui.theme_delete(theme);
    state.styles.map(|style| ui.style_delete(style));
    true
}

/// The `my_theme_t` fields read by `style_init()`.
struct StyleParams {
    /// `disp_dpi`
    dpi: i32,
    /// `disp_size`
    size: DisplaySize,
    /// `base.flags & MODE_DARK`
    dark: bool,
    /// `base.font_small` / `base.font_normal` (the same font in this theme).
    font: FontRef,
    /// `base.color_primary`
    primary: Color,
    /// `base.color_secondary`
    secondary: Color,
    /// `LV_DPI_DEF`
    dpi_def: i32,
}

/// `style_init()`: every `lv_style_set_*` call, in the upstream order.
fn style_init(theme: &StyleParams) -> Styles<Style> {
    let dpi = theme.dpi;
    let dpx = |n: i32| dpx_calc(dpi, n);
    let by_size = |large: i32, medium: i32, small: i32| match theme.size {
        DisplaySize::Large => large,
        DisplaySize::Medium => medium,
        DisplaySize::Small => small,
    };
    let dark = theme.dark;
    let radius_default = dpx(if theme.size == DisplaySize::Large { 12 } else { 8 });
    let border_width = dpx(2);
    let outline_width = dpx(3);
    let pad_def = dpx(by_size(24, 20, 16));
    let pad_small = dpx(by_size(14, 12, 10));
    let pad_tiny = dpx(by_size(8, 6, 2));

    let color_scr = if dark {
        Color::hex(0x15171A)
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

    let mut s = Styles::from_fn(Style::new);

    let trans_delayed = SharedRef::Shared(Rc::new(StyleTransitionDsc::new_static(
        TRANSITION_PROPS,
        AnimPath::Linear,
        TRANSITION_TIME,
        70,
    )));
    let trans_normal = SharedRef::Shared(Rc::new(StyleTransitionDsc::new_static(
        TRANSITION_PROPS,
        AnimPath::Linear,
        TRANSITION_TIME,
        0,
    )));
    // Go back to default state with delay
    s.transition_delayed.set_transition(trans_delayed);
    s.transition_normal.set_transition(trans_normal.clone());

    s.scrollbar.set_bg_color(if dark {
        palette::darken(Palette::Grey, 2)
    } else {
        palette::main(Palette::Grey)
    });
    s.scrollbar.set_radius(RADIUS_CIRCLE);
    s.scrollbar.set_pad_all(dpx(7));
    s.scrollbar.set_width(dpx(5));
    s.scrollbar.set_bg_opa(OPA_40);
    s.scrollbar.set_transition(trans_normal);

    s.scrollbar_scrolled.set_bg_opa(OPA_COVER);

    s.scr.set_bg_opa(OPA_COVER);
    s.scr.set_bg_color(color_scr);
    s.scr.set_text_color(color_text);
    s.scr.set_text_font(theme.font.clone());
    s.scr.set_pad_row(pad_small);
    s.scr.set_pad_column(pad_small);
    s.scr.set_rotary_sensitivity((dpi / 4 * 256) as u32);

    s.card.set_radius(radius_default);
    s.card.set_bg_opa(OPA_COVER);
    s.card.set_bg_color(color_card);
    s.card.set_border_color(color_grey);
    s.card.set_border_width(border_width);
    s.card.set_border_post(true);
    s.card.set_text_color(color_text);
    s.card.set_pad_all(pad_def);
    s.card.set_pad_row(pad_small);
    s.card.set_pad_column(pad_small);
    s.card.set_line_color(palette::main(Palette::Grey));
    s.card.set_line_width(dpx(1));

    s.outline_primary.set_outline_color(theme.primary);
    s.outline_primary.set_outline_width(outline_width);
    s.outline_primary.set_outline_pad(outline_width);
    s.outline_primary.set_outline_opa(OPA_50);

    s.outline_secondary.set_outline_color(theme.secondary);
    s.outline_secondary.set_outline_width(outline_width);
    s.outline_secondary.set_outline_opa(OPA_50);

    s.btn.set_radius(dpx(by_size(16, 12, 8)));
    s.btn.set_bg_opa(OPA_COVER);
    s.btn.set_bg_color(color_grey);
    if !dark {
        s.btn.set_shadow_color(palette::main(Palette::Grey));
        s.btn.set_shadow_width(dpx(3));
        s.btn.set_shadow_opa(OPA_50);
        s.btn.set_shadow_offset_y(dpx(3));
    }
    s.btn.set_text_color(color_text);
    s.btn.set_pad_hor(pad_def);
    s.btn.set_pad_ver(pad_small);
    s.btn.set_pad_column(dpx(5));
    s.btn.set_pad_row(dpx(5));

    s.pressed.set_recolor(Color::BLACK);
    s.pressed.set_recolor_opa(35);

    // C reads `theme_def->base.flags`: the same theme in this per-display port.
    s.disabled.set_recolor(if dark {
        palette::darken(Palette::Grey, 2)
    } else {
        palette::lighten(Palette::Grey, 2)
    });
    s.disabled.set_recolor_opa(OPA_50);

    s.clip_corner.set_clip_corner(true);
    s.clip_corner.set_border_post(true);

    s.pad_normal.set_pad_all(pad_def);
    s.pad_normal.set_pad_row(pad_def);
    s.pad_normal.set_pad_column(pad_def);

    s.pad_small.set_pad_all(pad_small);
    s.pad_small.set_pad_gap(pad_small);

    s.pad_gap.set_pad_row(dpx(10));
    s.pad_gap.set_pad_column(dpx(10));

    s.line_space_large.set_text_line_space(dpx(20));

    s.text_align_center.set_text_align(TextAlign::Center);

    s.pad_zero.set_pad_all(0);
    s.pad_zero.set_pad_row(0);
    s.pad_zero.set_pad_column(0);

    s.pad_tiny.set_pad_all(pad_tiny);
    s.pad_tiny.set_pad_row(pad_tiny);
    s.pad_tiny.set_pad_column(pad_tiny);

    s.bg_color_primary.set_bg_color(theme.primary);
    s.bg_color_primary.set_text_color(Color::WHITE);
    s.bg_color_primary.set_bg_opa(OPA_COVER);

    s.bg_color_primary_muted.set_bg_color(theme.primary);
    s.bg_color_primary_muted.set_text_color(theme.primary);
    s.bg_color_primary_muted.set_bg_opa(OPA_20);

    s.bg_color_secondary.set_bg_color(theme.secondary);
    s.bg_color_secondary.set_text_color(Color::WHITE);
    s.bg_color_secondary.set_bg_opa(OPA_COVER);

    s.bg_color_secondary_muted.set_bg_color(theme.secondary);
    s.bg_color_secondary_muted.set_text_color(theme.secondary);
    s.bg_color_secondary_muted.set_bg_opa(OPA_20);

    s.bg_color_grey.set_bg_color(color_grey);
    s.bg_color_grey.set_bg_opa(OPA_COVER);
    s.bg_color_grey.set_text_color(color_text);

    s.bg_color_white.set_bg_color(color_card);
    s.bg_color_white.set_bg_opa(OPA_COVER);
    s.bg_color_white.set_text_color(color_text);

    s.circle.set_radius(RADIUS_CIRCLE);

    s.no_radius.set_radius(0);

    s.rotary_scroll.set_rotary_sensitivity((dpi / 4 * 256) as u32);

    // LV_THEME_DEFAULT_GROW=1 in the pinned reference configuration.
    s.grow.set_transform_width(dpx(3));
    s.grow.set_transform_height(dpx(3));

    s.knob.set_bg_color(theme.primary);
    s.knob.set_bg_opa(OPA_COVER);
    s.knob.set_pad_all(dpx(6));
    s.knob.set_radius(RADIUS_CIRCLE);

    s.anim.set_anim_duration(200);

    s.anim_fast.set_anim_duration(120);

    #[cfg(feature = "arc")]
    {
        s.arc_indic.set_arc_color(color_grey);
        s.arc_indic.set_arc_width(dpx(15));
        s.arc_indic.set_arc_rounded(true);

        s.arc_indic_primary.set_arc_color(theme.primary);
    }

    // LV_USE_DROPDOWN
    s.dropdown_list.set_max_height(theme.dpi_def * 2);

    #[cfg(feature = "checkbox")]
    {
        s.cb_marker.set_pad_all(dpx(3));
        s.cb_marker.set_border_width(border_width);
        s.cb_marker.set_border_color(theme.primary);
        s.cb_marker.set_bg_color(color_card);
        s.cb_marker.set_bg_opa(OPA_COVER);
        s.cb_marker.set_radius(radius_default / 2);
        s.cb_marker.set_text_font(theme.font.clone());
        s.cb_marker.set_text_color(Color::WHITE);

        s.cb_marker_checked
            .set_bg_image_src(ImageSrc::Symbol(SharedRef::Static(SYMBOL_OK)));
    }

    #[cfg(feature = "switch")]
    {
        s.switch_knob.set_pad_all(-dpx(4));
        s.switch_knob.set_bg_color(Color::WHITE);
    }

    #[cfg(feature = "line")]
    {
        s.line.set_line_width(1);
        s.line.set_line_color(color_text);
    }

    // LV_USE_CHART
    s.chart_bg.set_border_post(false);
    s.chart_bg.set_pad_column(dpx(10));
    s.chart_bg.set_line_color(color_grey);

    s.chart_series.set_line_width(dpx(3));
    s.chart_series.set_radius(dpx(3));

    let chart_size = dpx(8);
    s.chart_series.set_size(chart_size, chart_size);
    s.chart_series.set_pad_column(dpx(2));

    s.chart_indic.set_radius(RADIUS_CIRCLE);
    s.chart_indic.set_size(chart_size, chart_size);
    s.chart_indic.set_bg_color(theme.primary);
    s.chart_indic.set_bg_opa(OPA_COVER);

    // LV_USE_MENU
    s.menu_bg.set_pad_all(0);
    s.menu_bg.set_pad_gap(0);
    s.menu_bg.set_radius(0);
    s.menu_bg.set_clip_corner(true);
    s.menu_bg.set_border_side(BorderSide::NONE);

    s.menu_section.set_radius(radius_default);
    s.menu_section.set_clip_corner(true);
    s.menu_section.set_bg_opa(OPA_COVER);
    s.menu_section.set_bg_color(color_card);
    s.menu_section.set_text_color(color_text);

    s.menu_cont.set_pad_hor(pad_small);
    s.menu_cont.set_pad_ver(pad_small);
    s.menu_cont.set_pad_gap(pad_small);
    s.menu_cont.set_border_width(dpx(1));
    s.menu_cont.set_border_opa(OPA_10);
    s.menu_cont.set_border_color(color_text);
    s.menu_cont.set_border_side(BorderSide::NONE);

    s.menu_sidebar_cont.set_pad_all(0);
    s.menu_sidebar_cont.set_pad_gap(0);
    s.menu_sidebar_cont.set_border_width(dpx(1));
    s.menu_sidebar_cont.set_border_opa(OPA_10);
    s.menu_sidebar_cont.set_border_color(color_text);
    s.menu_sidebar_cont.set_border_side(BorderSide::RIGHT);

    s.menu_main_cont.set_pad_all(0);
    s.menu_main_cont.set_pad_gap(0);

    s.menu_header_cont.set_pad_hor(pad_small);
    s.menu_header_cont.set_pad_ver(pad_tiny);
    s.menu_header_cont.set_pad_gap(pad_small);

    s.menu_header_btn.set_pad_hor(pad_tiny);
    s.menu_header_btn.set_pad_ver(pad_tiny);
    s.menu_header_btn.set_shadow_opa(OPA_TRANSP);
    s.menu_header_btn.set_bg_opa(OPA_TRANSP);
    s.menu_header_btn.set_text_color(color_text);

    s.menu_page.set_pad_hor(0);
    s.menu_page.set_pad_gap(0);

    s.menu_pressed.set_bg_opa(OPA_20);
    s.menu_pressed.set_bg_color(palette::main(Palette::Grey));

    s.menu_separator.set_bg_opa(OPA_TRANSP);
    s.menu_separator.set_pad_ver(pad_tiny);

    // LV_USE_TABLE
    s.table_cell.set_border_width(dpx(1));
    s.table_cell.set_border_color(color_grey);
    s.table_cell.set_border_side(BorderSide::TOP | BorderSide::BOTTOM);

    #[cfg(feature = "textarea")]
    {
        s.ta_cursor.set_border_color(color_text);
        s.ta_cursor.set_border_width(dpx(2));
        s.ta_cursor.set_pad_left(-dpx(1));
        s.ta_cursor.set_border_side(BorderSide::LEFT);
        s.ta_cursor.set_anim_duration(400);

        s.ta_placeholder.set_text_color(if dark {
            palette::darken(Palette::Grey, 2)
        } else {
            palette::lighten(Palette::Grey, 1)
        });
    }

    // LV_USE_CALENDAR
    s.calendar_btnm_bg.set_pad_all(pad_small);
    s.calendar_btnm_bg.set_pad_gap(pad_small / 2);

    s.calendar_btnm_day.set_border_width(dpx(1));
    s.calendar_btnm_day.set_border_color(color_grey);
    s.calendar_btnm_day.set_bg_color(color_card);
    s.calendar_btnm_day.set_bg_opa(OPA_20);

    s.calendar_header.set_pad_hor(pad_small);
    s.calendar_header.set_pad_top(pad_small);
    s.calendar_header.set_pad_bottom(pad_tiny);
    s.calendar_header.set_pad_gap(pad_small);

    // LV_USE_MSGBOX
    s.msgbox_backdrop_bg.set_bg_color(palette::main(Palette::Grey));
    s.msgbox_backdrop_bg.set_bg_opa(OPA_50);

    // LV_USE_KEYBOARD
    s.keyboard_button_bg.set_shadow_width(0);
    s.keyboard_button_bg
        .set_radius(if theme.size == DisplaySize::Small {
            radius_default / 2
        } else {
            radius_default
        });

    // LV_USE_TABVIEW
    s.tab_btn.set_border_color(theme.primary);
    s.tab_btn.set_border_width(border_width * 2);
    s.tab_btn.set_border_side(BorderSide::BOTTOM);
    s.tab_btn.set_pad_top(border_width * 2);

    s.tab_bg_focus.set_outline_pad(-border_width);

    // LV_USE_LIST
    s.list_bg.set_pad_hor(pad_def);
    s.list_bg.set_pad_ver(0);
    s.list_bg.set_pad_gap(0);
    s.list_bg.set_clip_corner(true);

    s.list_btn.set_border_width(dpx(1));
    s.list_btn.set_border_color(color_grey);
    s.list_btn.set_border_side(BorderSide::BOTTOM);
    s.list_btn.set_pad_all(pad_small);
    s.list_btn.set_pad_column(pad_small);

    s.list_item_grow.set_transform_width(pad_def);

    #[cfg(feature = "led")]
    {
        s.led.set_bg_opa(OPA_COVER);
        s.led.set_bg_color(Color::WHITE);
        s.led.set_bg_grad_color(palette::main(Palette::Grey));
        s.led.set_radius(RADIUS_CIRCLE);
        s.led.set_shadow_width(dpx(15));
        s.led.set_shadow_color(Color::WHITE);
        s.led.set_shadow_spread(dpx(5));
    }

    // LV_USE_SCALE
    s.scale.set_line_color(color_text);
    s.scale.set_line_width(dpx(2));
    s.scale.set_arc_color(color_text);
    s.scale.set_arc_width(dpx(2));
    s.scale.set_length(dpx(6));

    s
}

/// Attach `styles` in order with `lv_obj_add_style()`. A STYLE_CHANGED
/// callback can delete the object while a style is added: stop then, keeping
/// the C order for surviving objects.
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
fn theme_apply(ui: &mut Ui, theme: ThemeId, obj: Obj) {
    if !ui.obj_is_valid(obj) {
        return;
    }
    let Some(s) = data(ui, theme).map(|data| data.styles) else {
        return;
    };
    const DEFAULT: StyleSelector = StyleSelector::DEFAULT;
    let scrollbar: StyleSelector = Part::SCROLLBAR.into();
    let scrollbar_scrolled = Part::SCROLLBAR | State::SCROLLED;
    let focus_key: StyleSelector = State::FOCUS_KEY.into();
    let edited: StyleSelector = State::EDITED.into();

    if ui.obj_get_parent(obj).is_none() {
        add_styles(
            ui,
            obj,
            &[
                (s.scr, DEFAULT),
                (s.scrollbar, scrollbar),
                (s.scrollbar_scrolled, scrollbar_scrolled),
            ],
        );
        return;
    }

    if ui.obj_check_type(obj, &OBJ_CLASS) {
        // The tabview, win and calendar parent cases need classes that are not
        // ported yet.
        add_styles(
            ui,
            obj,
            &[
                (s.card, DEFAULT),
                (s.scrollbar, scrollbar),
                (s.scrollbar_scrolled, scrollbar_scrolled),
            ],
        );
        return;
    }

    #[cfg(feature = "button")]
    if ui.obj_check_type(obj, &crate::button::BUTTON_CLASS) {
        // The tabview-header and menu-header cases need unported classes.
        add_styles(
            ui,
            obj,
            &[
                (s.btn, DEFAULT),
                (s.bg_color_primary, DEFAULT),
                (s.transition_delayed, DEFAULT),
                (s.pressed, State::PRESSED.into()),
                (s.transition_normal, State::PRESSED.into()),
                (s.outline_primary, focus_key),
                // LV_THEME_DEFAULT_GROW
                (s.grow, State::PRESSED.into()),
                (s.bg_color_secondary, State::CHECKED.into()),
                (s.disabled, State::DISABLED.into()),
            ],
        );
        return;
    }

    #[cfg(feature = "bar")]
    if ui.obj_check_type(obj, &crate::bar::BAR_CLASS) {
        add_styles(
            ui,
            obj,
            &[
                (s.bg_color_primary_muted, DEFAULT),
                (s.circle, DEFAULT),
                (s.outline_primary, focus_key),
                (s.outline_secondary, edited),
                (s.bg_color_primary, Part::INDICATOR.into()),
                (s.circle, Part::INDICATOR.into()),
            ],
        );
        return;
    }

    #[cfg(feature = "slider")]
    if ui.obj_check_type(obj, &crate::slider::SLIDER_CLASS) {
        add_styles(
            ui,
            obj,
            &[
                (s.bg_color_primary_muted, DEFAULT),
                (s.circle, DEFAULT),
                (s.outline_primary, focus_key),
                (s.outline_secondary, edited),
                (s.bg_color_primary, Part::INDICATOR.into()),
                (s.circle, Part::INDICATOR.into()),
                (s.knob, Part::KNOB.into()),
                // LV_THEME_DEFAULT_GROW
                (s.grow, Part::KNOB | State::PRESSED),
                (s.transition_delayed, Part::KNOB.into()),
                (s.transition_normal, Part::KNOB | State::PRESSED),
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
                (s.pad_gap, DEFAULT),
                (s.outline_primary, focus_key),
                (s.disabled, Part::INDICATOR | State::DISABLED),
                (s.cb_marker, Part::INDICATOR.into()),
                (s.bg_color_primary, Part::INDICATOR | State::CHECKED),
                (s.cb_marker_checked, Part::INDICATOR | State::CHECKED),
                (s.pressed, Part::INDICATOR | State::PRESSED),
                // LV_THEME_DEFAULT_GROW
                (s.grow, Part::INDICATOR | State::PRESSED),
                (s.transition_normal, Part::INDICATOR | State::PRESSED),
                (s.transition_delayed, Part::INDICATOR.into()),
            ],
        );
        return;
    }

    #[cfg(feature = "switch")]
    if ui.obj_check_type(obj, &crate::switch::SWITCH_CLASS) {
        add_styles(
            ui,
            obj,
            &[
                (s.bg_color_grey, DEFAULT),
                (s.circle, DEFAULT),
                (s.anim_fast, DEFAULT),
                (s.disabled, State::DISABLED.into()),
                (s.outline_primary, focus_key),
                (s.bg_color_primary, Part::INDICATOR | State::CHECKED),
                (s.circle, Part::INDICATOR.into()),
                (s.knob, Part::KNOB.into()),
                (s.bg_color_white, Part::KNOB.into()),
                (s.switch_knob, Part::KNOB.into()),
                (s.transition_normal, Part::INDICATOR | State::CHECKED),
                (s.transition_normal, Part::INDICATOR.into()),
            ],
        );
        return;
    }

    // Unused when no widget branch above is compiled in.
    let _ = (focus_key, edited);
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
                        [ids.scr, ids.card, ids.scrollbar, ids.scrollbar_scrolled]
                            .map(|id| ui.style(id).unwrap().len()),
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
        let ids = data(&ui, theme).unwrap().styles;
        let [screen_style, card_style, scrollbar, scrolled] =
            [ids.scr, ids.card, ids.scrollbar, ids.scrollbar_scrolled];
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
    fn resolution_change_updates_existing_card_of_the_shared_theme() {
        let mut ui = ui();
        let a = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let b = ui.display_create_ex(320, 240, Some(init_display)).unwrap();
        let ta = ui.display_get_theme(Some(a)).unwrap();
        assert_eq!(ui.display_get_theme(Some(b)), Some(ta));
        let screen = ui.display_get_screen_active(Some(a)).unwrap();
        let card = ui.obj_create(Some(screen));
        let before = data(&ui, ta).unwrap().styles;
        ui.display_set_resolution(a, 320, 240);
        assert_eq!(data(&ui, ta).unwrap().styles, before);
        assert_eq!(ui.obj_get_style_radius(card, Part::MAIN), 8);
        assert_eq!(ui.obj_get_style_pad_left(card, Part::MAIN), 16);
        assert_eq!(data(&ui, ta).unwrap().dpi, 160);
    }

    /// `lv_display.c:133..139`: a later display gets the already inited global
    /// theme unchanged (first display's size, no resolution callback of its
    /// own), as in upstream `test_screen_load.c` / `test_display.c`.
    #[test]
    fn later_displays_get_the_inited_theme_unchanged() {
        let mut ui = ui();
        assert!(!is_inited(&ui));
        assert_eq!(get(&ui), None);
        let plain = ui.display_create(100, 100);
        let plain_events = ui.display_get_event_count(plain);
        let a = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let theme = ui.display_get_theme(Some(a)).unwrap();
        assert!(is_inited(&ui));
        assert_eq!(get(&ui), Some(theme));
        assert_eq!(ui.display_get_event_count(a), plain_events + 1);

        let b = ui.display_create_ex(100, 100, Some(init_display)).unwrap();
        assert_eq!(ui.display_get_theme(Some(b)), Some(theme));
        assert_eq!(ui.display_get_event_count(b), plain_events);
        assert_eq!(ui.theme(theme).unwrap().disp, Some(a));
        let screen = ui.display_get_screen_active(Some(b)).unwrap();
        assert_eq!(ui.obj_get_style_pad_row(screen, Part::MAIN), 14);
        let card = ui.obj_create(Some(screen));
        assert_eq!(ui.obj_get_style_pad_left(card, Part::MAIN), 24);
        assert_eq!(ui.obj_get_style_radius(card, Part::MAIN), 12);
        #[cfg(feature = "button")]
        {
            let button = crate::Button::create(&mut ui, screen).obj();
            assert_eq!(ui.obj_get_style_radius(button, Part::MAIN), 16);
        }
        // No callback on `b`: its resolution change does not rebuild styles.
        ui.display_set_resolution(b, 320, 240);
        assert_eq!(ui.obj_get_style_pad_left(card, Part::MAIN), 24);
        let medium = ui.display_create_ex(480, 320, Some(init_display)).unwrap();
        let screen = ui.display_get_screen_active(Some(medium)).unwrap();
        let card = ui.obj_create(Some(screen));
        assert_eq!(ui.obj_get_style_pad_left(card, Part::MAIN), 24);
    }

    /// `lv_theme_default_init()` re-initializes the one global theme for any
    /// display: same identity, objects of every display restyled, and the
    /// resolution callback added to the given display only.
    #[test]
    fn init_on_another_display_reinitializes_the_single_theme() {
        let mut ui = ui();
        let plain = ui.display_create(100, 100);
        let plain_events = ui.display_get_event_count(plain);
        let a = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let theme = ui.display_get_theme(Some(a)).unwrap();
        let styles = data(&ui, theme).unwrap().styles;
        let card = ui.obj_create(ui.display_get_screen_active(Some(a)));
        let b = ui.display_create(320, 240);
        let font = ui.config().font_default.clone().unwrap();
        let blue = palette::main(Palette::Blue);
        let red = palette::main(Palette::Red);
        assert_eq!(init(&mut ui, b, blue, red, false, font.clone()), Some(theme));
        assert_eq!(data(&ui, theme).unwrap().styles, styles);
        assert_eq!(data(&ui, theme).unwrap().size, DisplaySize::Small);
        assert_eq!(ui.theme(theme).unwrap().disp, Some(b));
        assert_eq!(ui.obj_get_style_pad_left(card, Part::MAIN), 16);
        assert_eq!(ui.display_get_event_count(b), plain_events + 1);
        assert_eq!(ui.display_get_event_count(a), plain_events + 1);
        // Unchanged parameters: early return, no second callback on `b`.
        assert_eq!(init(&mut ui, b, blue, red, false, font.clone()), Some(theme));
        assert_eq!(ui.display_get_event_count(b), plain_events + 1);
        // `a` keeps its callback: its resolution change rebuilds for `a`.
        ui.display_set_resolution(a, 800, 600);
        assert_eq!(ui.obj_get_style_pad_left(card, Part::MAIN), 24);
        assert_eq!(ui.theme(theme).unwrap().disp, Some(a));
        // NULL display falls back to `lv_display_get_default()`.
        ui.display_set_default(b);
        assert_eq!(init(&mut ui, None, blue, red, false, font), Some(theme));
        assert_eq!(ui.theme(theme).unwrap().disp, Some(b));
        assert_eq!(ui.obj_get_style_pad_left(card, Part::MAIN), 16);
        ui.display_set_theme(a, None);
        assert!(deinit(&mut ui, theme));
        assert_eq!(ui.display_get_event_count(a), plain_events);
        assert_eq!(ui.display_get_event_count(b), plain_events);
        assert!(!is_inited(&ui));
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

    /// Styles attached by `theme_apply()`, in `lv_obj_add_style()` call order
    /// (each new theme style is inserted before the previous ones).
    #[cfg(any(feature = "bar", feature = "slider", feature = "switch", feature = "checkbox"))]
    fn applied(ui: &Ui, obj: Obj) -> alloc::vec::Vec<(StyleId, StyleSelector)> {
        use ferrovgl_core::core::obj_style::ObjStyleSrc;
        ui.obj(obj)
            .unwrap()
            .styles
            .iter()
            .rev()
            .map(|entry| match entry.src {
                ObjStyleSrc::Shared(id) if entry.is_theme => (id, entry.selector),
                _ => panic!("only shared theme styles are expected"),
            })
            .collect()
    }

    /// Allocate a widget without running its (not yet ported) constructor and
    /// apply the theme with style refresh disabled, as `lv_obj_class_init_obj`
    /// does before `lv_obj_construct`. No event reaches the widget class.
    #[cfg(any(feature = "bar", feature = "slider", feature = "switch", feature = "checkbox"))]
    fn themed(class: &'static ferrovgl_core::ObjClass) -> (Ui, Obj, Styles<StyleId>) {
        let mut ui = ui();
        let display = ui.display_create_ex(800, 480, Some(init_display)).unwrap();
        let theme = ui.display_get_theme(Some(display)).unwrap();
        let screen = ui.display_get_screen_active(Some(display)).unwrap();
        let obj = ui.obj_class_create_obj(class, Some(screen));
        ui.obj_enable_style_refresh(false);
        ui.theme_apply(obj);
        ui.obj_enable_style_refresh(true);
        let styles = data(&ui, theme).unwrap().styles;
        (ui, obj, styles)
    }

    /// `lv_theme_default.c:880..887`
    #[cfg(feature = "bar")]
    #[test]
    fn bar_branch_matches_upstream_order() {
        let (ui, obj, s) = themed(&crate::bar::BAR_CLASS);
        assert_eq!(
            applied(&ui, obj),
            [
                (s.bg_color_primary_muted, StyleSelector::DEFAULT),
                (s.circle, StyleSelector::DEFAULT),
                (s.outline_primary, State::FOCUS_KEY.into()),
                (s.outline_secondary, State::EDITED.into()),
                (s.bg_color_primary, Part::INDICATOR.into()),
                (s.circle, Part::INDICATOR.into()),
            ]
        );
        let primary = palette::main(Palette::Blue);
        assert_eq!(ui.obj_get_style_bg_color(obj, Part::MAIN), primary);
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::MAIN), OPA_20);
        assert_eq!(ui.obj_get_style_radius(obj, Part::INDICATOR), RADIUS_CIRCLE);
        assert_eq!(ui.obj_get_style_bg_opa(obj, Part::INDICATOR), OPA_COVER);
        assert_eq!(ui.obj_get_style_text_color(obj, Part::MAIN), primary);
    }

    /// `lv_theme_default.c:890..903`
    #[cfg(feature = "slider")]
    #[test]
    fn slider_branch_matches_upstream_order() {
        let (mut ui, obj, s) = themed(&crate::slider::SLIDER_CLASS);
        assert_eq!(
            applied(&ui, obj),
            [
                (s.bg_color_primary_muted, StyleSelector::DEFAULT),
                (s.circle, StyleSelector::DEFAULT),
                (s.outline_primary, State::FOCUS_KEY.into()),
                (s.outline_secondary, State::EDITED.into()),
                (s.bg_color_primary, Part::INDICATOR.into()),
                (s.circle, Part::INDICATOR.into()),
                (s.knob, Part::KNOB.into()),
                (s.grow, Part::KNOB | State::PRESSED),
                (s.transition_delayed, Part::KNOB.into()),
                (s.transition_normal, Part::KNOB | State::PRESSED),
            ]
        );
        assert_eq!(ui.obj_get_style_pad_left(obj, Part::KNOB), 6);
        assert_eq!(ui.obj_get_style_radius(obj, Part::KNOB), RADIUS_CIRCLE);
        assert_eq!(ui.obj_get_style_transition(obj, Part::KNOB).unwrap().delay, 70);
        // Set the state directly: `obj_add_state` would send events to the
        // not yet ported widget class.
        ui.obj_mut(obj).unwrap().state |= State::PRESSED;
        assert_eq!(ui.obj_get_style_transform_width(obj, Part::KNOB), 3);
        assert_eq!(ui.obj_get_style_transition(obj, Part::KNOB).unwrap().delay, 0);
    }

    /// `lv_theme_default.c:922..936`
    #[cfg(feature = "checkbox")]
    #[test]
    fn checkbox_branch_matches_upstream_order() {
        let (mut ui, obj, s) = themed(&crate::checkbox::CHECKBOX_CLASS);
        assert_eq!(
            applied(&ui, obj),
            [
                (s.pad_gap, StyleSelector::DEFAULT),
                (s.outline_primary, State::FOCUS_KEY.into()),
                (s.disabled, Part::INDICATOR | State::DISABLED),
                (s.cb_marker, Part::INDICATOR.into()),
                (s.bg_color_primary, Part::INDICATOR | State::CHECKED),
                (s.cb_marker_checked, Part::INDICATOR | State::CHECKED),
                (s.pressed, Part::INDICATOR | State::PRESSED),
                (s.grow, Part::INDICATOR | State::PRESSED),
                (s.transition_normal, Part::INDICATOR | State::PRESSED),
                (s.transition_delayed, Part::INDICATOR.into()),
            ]
        );
        assert_eq!(ui.obj_get_style_pad_column(obj, Part::MAIN), 10);
        assert_eq!(ui.obj_get_style_pad_top(obj, Part::INDICATOR), 3);
        assert_eq!(ui.obj_get_style_border_width(obj, Part::INDICATOR), 2);
        assert_eq!(ui.obj_get_style_radius(obj, Part::INDICATOR), 6);
        assert_eq!(
            ui.obj_get_style_border_color(obj, Part::INDICATOR),
            palette::main(Palette::Blue)
        );
        assert_eq!(
            ui.obj_get_style_bg_image_src(obj, Part::INDICATOR),
            ImageSrc::None
        );
        // Set the state directly: `obj_add_state` would send events to the
        // not yet ported widget class.
        ui.obj_mut(obj).unwrap().state |= State::CHECKED;
        assert!(matches!(
            ui.obj_get_style_bg_image_src(obj, Part::INDICATOR),
            ImageSrc::Symbol(text) if &*text == "\u{F00C}"
        ));
    }

    /// `lv_theme_default.c:939..954`
    #[cfg(feature = "switch")]
    #[test]
    fn switch_branch_matches_upstream_order() {
        let (ui, obj, s) = themed(&crate::switch::SWITCH_CLASS);
        assert_eq!(
            applied(&ui, obj),
            [
                (s.bg_color_grey, StyleSelector::DEFAULT),
                (s.circle, StyleSelector::DEFAULT),
                (s.anim_fast, StyleSelector::DEFAULT),
                (s.disabled, State::DISABLED.into()),
                (s.outline_primary, State::FOCUS_KEY.into()),
                (s.bg_color_primary, Part::INDICATOR | State::CHECKED),
                (s.circle, Part::INDICATOR.into()),
                (s.knob, Part::KNOB.into()),
                (s.bg_color_white, Part::KNOB.into()),
                (s.switch_knob, Part::KNOB.into()),
                (s.transition_normal, Part::INDICATOR | State::CHECKED),
                (s.transition_normal, Part::INDICATOR.into()),
            ]
        );
        assert_eq!(ui.obj_get_style_anim_duration(obj, Part::MAIN), 120);
        assert_eq!(ui.obj_get_style_pad_left(obj, Part::KNOB), -4);
        assert_eq!(ui.obj_get_style_bg_color(obj, Part::KNOB), Color::WHITE);
        assert_eq!(ui.obj_get_style_bg_color(obj, Part::MAIN), Color::hex(0xe0e0e0));
    }

    /// Property counts of every `style_init()` style (`lv_theme_default.c:197..653`)
    /// and the size/mode dependent values that no ported widget reads yet.
    #[test]
    fn complete_style_set_matches_style_init() {
        for (width, dark) in [(800, false), (320, true)] {
            let mut ui = ui();
            let display = ui.display_create_ex(width, 240, Some(init_display)).unwrap();
            let font = ui.config().font_default.clone().unwrap();
            let theme = init(&mut ui, display, Color::BLACK, Color::WHITE, dark, font).unwrap();
            let s = data(&ui, theme).unwrap().styles;
            let len = |id: StyleId| ui.style(id).unwrap().len();
            let btn = if dark { 10 } else { 14 };
            #[allow(unused_mut)] // Extended below per enabled widget feature.
            let mut expected = alloc::vec![
                (s.scr, 7),
                (s.scrollbar, 9),
                (s.scrollbar_scrolled, 1),
                (s.card, 15),
                (s.btn, btn),
                (s.bg_color_primary, 3),
                (s.bg_color_primary_muted, 3),
                (s.bg_color_secondary, 3),
                (s.bg_color_secondary_muted, 3),
                (s.bg_color_grey, 3),
                (s.bg_color_white, 3),
                (s.pressed, 2),
                (s.disabled, 2),
                (s.pad_zero, 6),
                (s.pad_tiny, 6),
                (s.pad_small, 6),
                (s.pad_normal, 6),
                (s.pad_gap, 2),
                (s.line_space_large, 1),
                (s.text_align_center, 1),
                (s.outline_primary, 4),
                (s.outline_secondary, 3),
                (s.circle, 1),
                (s.no_radius, 1),
                (s.clip_corner, 2),
                (s.rotary_scroll, 1),
                (s.grow, 2),
                (s.transition_delayed, 1),
                (s.transition_normal, 1),
                (s.anim, 1),
                (s.anim_fast, 1),
                (s.knob, 7),
                (s.chart_series, 5),
                (s.chart_indic, 5),
                (s.chart_bg, 3),
                (s.dropdown_list, 1),
                (s.table_cell, 3),
                (s.calendar_btnm_bg, 6),
                (s.calendar_btnm_day, 4),
                (s.calendar_header, 6),
                (s.menu_bg, 9),
                (s.menu_cont, 10),
                (s.menu_sidebar_cont, 10),
                (s.menu_main_cont, 6),
                (s.menu_page, 4),
                (s.menu_header_cont, 6),
                (s.menu_header_btn, 7),
                (s.menu_section, 5),
                (s.menu_pressed, 2),
                (s.menu_separator, 3),
                (s.msgbox_backdrop_bg, 2),
                (s.keyboard_button_bg, 2),
                (s.list_bg, 7),
                (s.list_btn, 8),
                (s.list_item_grow, 1),
                (s.tab_bg_focus, 1),
                (s.tab_btn, 4),
                (s.scale, 5),
            ];
            #[cfg(feature = "arc")]
            expected.extend([(s.arc_indic, 3), (s.arc_indic_primary, 1)]);
            #[cfg(feature = "checkbox")]
            expected.extend([(s.cb_marker, 11), (s.cb_marker_checked, 1)]);
            #[cfg(feature = "switch")]
            expected.push((s.switch_knob, 5));
            #[cfg(feature = "line")]
            expected.push((s.line, 2));
            #[cfg(feature = "textarea")]
            expected.extend([(s.ta_cursor, 5), (s.ta_placeholder, 1)]);
            #[cfg(feature = "led")]
            expected.push((s.led, 7));
            for (index, &(id, count)) in expected.iter().enumerate() {
                assert_eq!(len(id), count, "style #{index}");
            }
            let mut all = 0;
            s.map(|_| all += 1);
            assert_eq!(all, expected.len());

            let get = |id: StyleId, prop: StyleProp| ui.style(id).unwrap().get_prop(prop);
            let num = |id, prop| match get(id, prop) {
                Some(&ferrovgl_core::StyleValue::Num(value)) => value,
                other => panic!("{prop:?}: {other:?}"),
            };
            // DISP_LARGE / DISP_SMALL at 160 DPI.
            let small = width == 320;
            assert_eq!(num(s.pad_tiny, StyleProp::PAD_TOP), if small { 2 } else { 8 });
            assert_eq!(
                num(s.keyboard_button_bg, StyleProp::RADIUS),
                if small { 4 } else { 12 }
            );
            assert_eq!(num(s.dropdown_list, StyleProp::MAX_HEIGHT), 320);
            assert_eq!(num(s.tab_bg_focus, StyleProp::OUTLINE_PAD), -2);
            assert_eq!(
                num(s.calendar_btnm_bg, StyleProp::PAD_ROW),
                if small { 5 } else { 7 }
            );
            assert_eq!(
                num(s.list_item_grow, StyleProp::TRANSFORM_WIDTH),
                if small { 16 } else { 24 }
            );
            #[cfg(feature = "textarea")]
            {
                assert_eq!(num(s.ta_cursor, StyleProp::PAD_LEFT), -1);
                assert_eq!(
                    get(s.ta_placeholder, StyleProp::TEXT_COLOR).cloned(),
                    Some(ferrovgl_core::StyleValue::Color(if dark {
                        palette::darken(Palette::Grey, 2)
                    } else {
                        palette::lighten(Palette::Grey, 1)
                    }))
                );
            }
        }
    }
}
