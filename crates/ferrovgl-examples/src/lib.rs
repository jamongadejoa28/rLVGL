//! Ports of the LVGL v9.6.0 `lv_example_*` functions (`examples/` in the
//! upstream repository). Each example builds its UI on the active screen of
//! the default display, exactly like the C function, so the SDL and browser
//! demos and the headless smoke tests can all run the same code.
//!
//! The module tree mirrors the upstream `examples/` directory and every
//! function keeps its C name. C `static` variables (styles, counters, object
//! pointers) live in per-[`Ui`] storage ([`Ui::widget_static_mut`]), so they
//! persist between calls on the same `Ui` like the C statics persist for the
//! program's lifetime.

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

use ferrovgl_core::Ui;

pub mod anim;
pub mod assets;
pub mod event;
pub mod get_started;
pub mod layouts;
pub mod scroll;
pub mod styles;
mod util;
pub mod widgets;

/// One runnable example.
#[derive(Clone, Copy)]
pub struct Example {
    /// Upstream function name, e.g. `lv_example_get_started_button`.
    pub name: &'static str,
    /// Builds the example on the default display's active screen.
    pub run: fn(&mut Ui),
}

/// `Example { name: "<fn>", run: <module>::<fn> }` rows of [`EXAMPLES`].
macro_rules! examples {
    ($( $($m:ident)::+ => $f:ident ),* $(,)?) => {
        &[$(Example {
            name: stringify!($f),
            run: $($m::)+$f,
        }),*]
    };
}

/// Every ported example, in upstream `lv_examples.h` order.
pub const EXAMPLES: &[Example] = examples![
    anim => lv_example_anim_1,
    anim => lv_example_anim_2,
    anim => lv_example_anim_4,
    event => lv_example_event_click,
    event => lv_example_event_streak,
    event => lv_example_event_button,
    event => lv_example_event_bubble,
    event => lv_example_event_trickle,
    event => lv_example_event_draw,
    get_started => lv_example_get_started_button,
    get_started => lv_example_get_started_hello_world,
    get_started => lv_example_get_started_slider,
    get_started => lv_example_get_started_styles,
    layouts::flex => lv_example_flex_align,
    layouts::flex => lv_example_flex_flow,
    layouts::flex => lv_example_flex_grow,
    layouts::flex => lv_example_flex_ignore_layout,
    layouts::flex => lv_example_flex_internal_padding,
    layouts::flex => lv_example_flex_list,
    layouts::flex => lv_example_flex_new_track,
    layouts::flex => lv_example_flex_rtl,
    layouts::flex => lv_example_flex_win,
    layouts::grid => lv_example_grid_align,
    layouts::grid => lv_example_grid_cell_span,
    layouts::grid => lv_example_grid_descriptors,
    layouts::grid => lv_example_grid_fr,
    layouts::grid => lv_example_grid_ignore_layout,
    layouts::grid => lv_example_grid_internal_padding,
    layouts::grid => lv_example_grid_rtl,
    layouts::grid => lv_example_grid_subgrid,
    layouts::split => lv_example_split,
    layouts::split => lv_example_split_vertical,
    scroll => lv_example_scroll_chain,
    scroll => lv_example_scroll_circular,
    scroll => lv_example_scroll_events,
    scroll => lv_example_scroll_floating,
    scroll => lv_example_scroll_infinite,
    scroll => lv_example_scroll_one,
    scroll => lv_example_scroll_overview,
    scroll => lv_example_scroll_properties,
    scroll => lv_example_scroll_rtl,
    scroll => lv_example_scroll_scrollable,
    scroll => lv_example_scroll_scrollbar_mode,
    scroll => lv_example_scroll_scrollbar_style,
    scroll => lv_example_scroll_snap,
    scroll => lv_example_scroll_to,
    scroll => lv_example_scroll_translate,
    styles => lv_example_style_arc,
    styles => lv_example_style_background,
    styles => lv_example_style_blur,
    styles => lv_example_style_border,
    styles => lv_example_style_drop_shadow,
    styles => lv_example_style_gradient_buttons,
    styles => lv_example_style_gradient_conic,
    styles => lv_example_style_gradient_radial,
    styles => lv_example_style_image,
    styles => lv_example_style_line,
    styles => lv_example_style_local,
    styles => lv_example_style_margin,
    styles => lv_example_style_modal,
    styles => lv_example_style_multiple,
    styles => lv_example_style_opacity_transform,
    styles => lv_example_style_outline,
    styles => lv_example_style_padding,
    styles => lv_example_style_parts_states,
    styles => lv_example_style_shadow,
    styles => lv_example_style_size_position,
    styles => lv_example_style_text,
    styles => lv_example_style_theme,
    styles => lv_example_style_transition,
    widgets::arc => lv_example_arc_change_rate,
    widgets::arc => lv_example_arc_event,
    widgets::arc => lv_example_arc_knob_widget,
    widgets::arc => lv_example_arc_mode,
    widgets::arc => lv_example_arc_pie_chart,
    widgets::arc => lv_example_arc_rotation,
    widgets::arc => lv_example_arc_set_angles,
    widgets::arc => lv_example_arc_styling,
    widgets::bar => lv_example_bar_bind_value,
    widgets::bar => lv_example_bar_modes,
    widgets::bar => lv_example_bar_orientation,
    widgets::bar => lv_example_bar_styling,
    widgets::bar => lv_example_bar_value_range,
    widgets::button => lv_example_button_basic,
    widgets::button => lv_example_button_event,
    widgets::button => lv_example_button_states,
    widgets::button => lv_example_button_styling,
    widgets::buttonmatrix => lv_example_buttonmatrix_button_ctrl,
    widgets::buttonmatrix => lv_example_buttonmatrix_button_width,
    widgets::buttonmatrix => lv_example_buttonmatrix_event,
    widgets::buttonmatrix => lv_example_buttonmatrix_map,
    widgets::buttonmatrix => lv_example_buttonmatrix_one_checked,
    widgets::buttonmatrix => lv_example_buttonmatrix_popover,
    widgets::buttonmatrix => lv_example_buttonmatrix_recolor,
    widgets::buttonmatrix => lv_example_buttonmatrix_styling,
    widgets::checkbox => lv_example_checkbox_bind_checked,
    widgets::checkbox => lv_example_checkbox_event,
    widgets::checkbox => lv_example_checkbox_states,
    widgets::checkbox => lv_example_checkbox_styling,
    widgets::checkbox => lv_example_checkbox_text,
    widgets::image => lv_example_image_inner_align,
    widgets::image => lv_example_image_recolor,
    widgets::image => lv_example_image_src,
    widgets::image => lv_example_image_transformations,
    widgets::label => lv_example_label_bind_text,
    widgets::label => lv_example_label_long_mode,
    widgets::label => lv_example_label_recolor,
    widgets::label => lv_example_label_set_text_newline,
    widgets::label => lv_example_label_styling,
    widgets::label => lv_example_label_text_align,
    widgets::led => lv_example_led_brightness,
    widgets::led => lv_example_led_color,
    widgets::led => lv_example_led_styling,
    widgets::line => lv_example_line_points,
    widgets::obj => lv_example_obj_bind_style,
    widgets::obj => lv_example_obj_draggable,
    widgets::obj => lv_example_obj_transform,
    widgets::slider => lv_example_slider_bind_state,
    widgets::slider => lv_example_slider_bind_value,
    widgets::slider => lv_example_slider_event,
    widgets::slider => lv_example_slider_modes,
    widgets::slider => lv_example_slider_reversed_direction,
    widgets::slider => lv_example_slider_styling,
    widgets::slider => lv_example_slider_value_range_orientation,
    widgets::spinner => lv_example_spinner_basic,
    widgets::switch => lv_example_switch_event,
    widgets::switch => lv_example_switch_orientation,
    widgets::switch => lv_example_switch_state,
    widgets::switch => lv_example_switch_styling,
    widgets::textarea => lv_example_textarea_buttonmatrix,
    widgets::textarea => lv_example_textarea_cursor,
    widgets::textarea => lv_example_textarea_placeholder,
];

/// Looks up an example by its upstream name (with or without the `lv_example_` prefix).
pub fn find(name: &str) -> Option<&'static Example> {
    let short = name.strip_prefix("lv_example_").unwrap_or(name);
    EXAMPLES
        .iter()
        .find(|e| e.name.strip_prefix("lv_example_").unwrap_or(e.name) == short)
}
