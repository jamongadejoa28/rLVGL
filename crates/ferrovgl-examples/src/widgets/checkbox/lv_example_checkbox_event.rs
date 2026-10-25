//! Port of `examples/widgets/checkbox/lv_example_checkbox_event.c`.

use alloc::rc::Rc;

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Event, EventCode, State, StyleSelector, Ui};
use ferrovgl_widgets::{Checkbox, Label};

use crate::util::screen_active;

/// `static lv_obj_t * status_label;`
#[derive(Default)]
struct StatusLabel(Option<Label>);

/// `static void event_cb(lv_event_t * e)`
fn event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(cb) = e.target_obj() else { return };
    let checked = ui.obj_has_state(cb, State::CHECKED);
    // LV_LOG_USER("checkbox %s", ...): not printed, the crate has no logger.
    if let Some(status_label) = ui.widget_static_mut::<StatusLabel>().0 {
        status_label.set_text(ui, if checked { "Feature: ON" } else { "Feature: OFF" });
    }
}

/// `lv_example_checkbox_event()`: mirror the checked state into a label.
pub fn lv_example_checkbox_event(ui: &mut Ui) {
    let scr = screen_active(ui);
    ui.obj_set_flex_flow(scr, FlexFlow::Column);
    ui.obj_set_style_flex_cross_place(scr, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_pad_row(scr, 12, StyleSelector::DEFAULT);

    let cb = Checkbox::create(ui, scr);
    cb.set_text(ui, "Enable feature");
    ui.obj_add_event_cb(cb.obj(), Rc::new(event_cb), EventCode::VALUE_CHANGED, None);

    let status_label = Label::create(ui, scr);
    ui.widget_static_mut::<StatusLabel>().0 = Some(status_label);
    status_label.set_text(ui, "Feature: OFF");
}
