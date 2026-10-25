//! Port of `examples/widgets/arc/lv_example_arc_event.c`.

use alloc::rc::Rc;

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Event, EventCode, StyleSelector, Ui};
use ferrovgl_widgets::{Arc, Label};

use crate::util::screen_active;

/// `static lv_obj_t * label;`
#[derive(Default)]
struct StatusLabel(Option<Label>);

/// `static void event_cb(lv_event_t * e)`
fn event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(arc) = e.target_obj().and_then(|o| Arc::from_obj(ui, o)) else {
        return;
    };
    let value = arc.value(ui);
    // LV_LOG_USER("arc value: %d", (int)value): not printed, the crate has no logger.

    /* Any LVGL API can be called from an event handler — here we mirror the
     * value into a sibling label so the change is visible on screen. */
    if let Some(label) = ui.widget_static_mut::<StatusLabel>().0 {
        label.set_text_fmt(ui, format_args!("Value: {value}"));
    }
}

/// `lv_example_arc_event()`: mirror the arc value into a label.
pub fn lv_example_arc_event(ui: &mut Ui) {
    let scr = screen_active(ui);
    ui.obj_set_flex_flow(scr, FlexFlow::Column);
    ui.obj_set_style_flex_cross_place(scr, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_pad_row(scr, 12, StyleSelector::DEFAULT);

    let arc = Arc::create(ui, scr);
    ui.obj_set_size(arc.obj(), 150, 150);
    ui.obj_add_event_cb(arc.obj(), Rc::new(event_cb), EventCode::VALUE_CHANGED, None);

    let label = Label::create(ui, scr);
    ui.widget_static_mut::<StatusLabel>().0 = Some(label);
    let value = arc.value(ui);
    label.set_text_fmt(ui, format_args!("Value: {value}"));
}
