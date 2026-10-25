//! Port of `examples/widgets/switch/lv_example_switch_event.c`.

use alloc::rc::Rc;

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Event, EventCode, State, StyleSelector, Ui};
use ferrovgl_widgets::{Label, Switch};

use crate::util::screen_active;

/// `static lv_obj_t * status_label;`
#[derive(Default)]
struct StatusLabel(Option<Label>);

/// `static void event_cb(lv_event_t * e)`
fn event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(sw) = e.target_obj() else { return };
    let on = ui.obj_has_state(sw, State::CHECKED);
    // LV_LOG_USER("switch %s", ...): not printed, the crate has no logger.
    if let Some(status_label) = ui.widget_static_mut::<StatusLabel>().0 {
        status_label.set_text(ui, if on { "State: ON" } else { "State: OFF" });
    }
}

/// `lv_example_switch_event()`: mirror the switch state into a label.
pub fn lv_example_switch_event(ui: &mut Ui) {
    let scr = screen_active(ui);
    ui.obj_set_flex_flow(scr, FlexFlow::Column);
    ui.obj_set_style_flex_cross_place(scr, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_pad_row(scr, 12, StyleSelector::DEFAULT);

    let sw = Switch::create(ui, scr);
    ui.obj_add_event_cb(sw.obj(), Rc::new(event_cb), EventCode::VALUE_CHANGED, None);

    let status_label = Label::create(ui, scr);
    ui.widget_static_mut::<StatusLabel>().0 = Some(status_label);
    status_label.set_text(ui, "State: OFF");
}
