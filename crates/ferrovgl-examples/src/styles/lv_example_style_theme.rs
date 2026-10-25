//! Port of `examples/styles/lv_example_style_theme.c`.

use alloc::rc::Rc;

use ferrovgl_core::misc::event::EventTarget;
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::{Align, Event, EventCode, Obj, Style, StyleSelector, ThemeId, Ui};
use ferrovgl_widgets::{BUTTON_CLASS, Button, Label};

use crate::util::{screen_active, static_styles, styles_reinit};

/// `static lv_style_t style_btn;`
struct StyleBtn;

/// `static void new_theme_apply_cb(lv_theme_t * th, lv_obj_t * obj)`: called
/// when the styles of the base theme are already added, to add new styles.
fn new_theme_apply_cb(ui: &mut Ui, _th: ThemeId, obj: Obj) {
    if ui.obj_check_type(obj, &BUTTON_CLASS)
        && let Some([style_btn]) = static_styles::<StyleBtn, 1>(ui)
    {
        ui.obj_add_style(obj, style_btn, StyleSelector::DEFAULT);
    }
}

/// `static void on_display_delete(lv_event_t * e)`
fn on_display_delete(ui: &mut Ui, e: &mut Event<'_>) {
    let EventTarget::Display(display) = e.target() else {
        return;
    };
    if let Some(display_theme) = ui.display_get_theme(Some(display)) {
        ui.theme_delete(display_theme);
    }
}

/// `static void new_theme_init_and_set(void)`
fn new_theme_init_and_set(ui: &mut Ui) {
    let Some(display) = ui.display_get_default() else {
        return;
    };

    /*Initialize the styles*/
    styles_reinit::<StyleBtn, 1>(ui, || {
        let mut style_btn = Style::new();
        style_btn.set_bg_color(palette::main(Palette::Green));
        style_btn.set_border_color(palette::darken(Palette::Green, 3));
        style_btn.set_border_width(3);
        [style_btn]
    });

    /* Initialize the new theme with the current theme as its parent
     * The user is responsible for freeing the theme when it's no longer needed */
    let th_act = ui.display_get_theme(Some(display));
    let th_new = ui.theme_create();
    if let Some(th_act) = th_act {
        ui.theme_copy(th_new, th_act);
    }
    ui.theme_set_parent(th_new, th_act);

    /*Set the style apply callback for the new theme*/
    ui.theme_set_apply_cb(th_new, Some(new_theme_apply_cb));

    /*Assign the new theme to the current display*/
    ui.display_set_theme(display, Some(th_new));

    /* Assign a delete event callback so that we can delete the theme when the display is deleted*/
    ui.display_add_event_cb(display, Rc::new(on_display_delete), EventCode::DELETE, None);
}

/// `lv_example_style_theme()`: extend the current theme with a new one.
pub fn lv_example_style_theme(ui: &mut Ui) {
    let btn = Button::create(ui, screen_active(ui)).obj();
    ui.obj_align(btn, Align::TopMid, 0, 20);

    let label = Label::create(ui, btn);
    label.set_text(ui, "Original theme");

    new_theme_init_and_set(ui);

    let btn = Button::create(ui, screen_active(ui)).obj();
    ui.obj_align(btn, Align::BottomMid, 0, -20);

    let label = Label::create(ui, btn);
    label.set_text(ui, "New theme");
}
