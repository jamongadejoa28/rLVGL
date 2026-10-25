//! Port of `examples/widgets/button/lv_example_button_event.c`.

use alloc::rc::Rc;

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Event, EventCode, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `static lv_obj_t * status_label; static uint32_t click_count;`
#[derive(Default)]
struct Statics {
    status_label: Option<Label>,
    click_count: u32,
}

/// `static void event_cb(lv_event_t * e)`
fn event_cb(ui: &mut Ui, _e: &mut Event<'_>) {
    let s = ui.widget_static_mut::<Statics>();
    s.click_count = s.click_count.wrapping_add(1);
    let (click_count, status_label) = (s.click_count, s.status_label);
    // LV_LOG_USER("button clicked (%u)", ...): not printed, the crate has no logger.

    /* Any LVGL API can be called from an event handler. */
    if let Some(status_label) = status_label {
        status_label.set_text_fmt(ui, format_args!("Clicks: {click_count}"));
    }
}

/// `lv_example_button_event()`: count the clicks of a button.
pub fn lv_example_button_event(ui: &mut Ui) {
    let scr = screen_active(ui);
    ui.obj_set_flex_flow(scr, FlexFlow::Column);
    ui.obj_set_style_flex_cross_place(scr, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_pad_row(scr, 12, StyleSelector::DEFAULT);

    let button = Button::create(ui, scr).obj();
    ui.obj_add_event_cb(button, Rc::new(event_cb), EventCode::CLICKED, None);

    let btn_label = Label::create(ui, button);
    btn_label.set_text(ui, "Click me");
    ui.obj_center(btn_label.obj());

    let status_label = Label::create(ui, scr);
    ui.widget_static_mut::<Statics>().status_label = Some(status_label);
    status_label.set_text(ui, "Clicks: 0");
}
