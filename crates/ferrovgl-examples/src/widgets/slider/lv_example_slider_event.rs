//! Port of `examples/widgets/slider/lv_example_slider_event.c`.

use alloc::rc::Rc;

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Event, EventCode, StyleSelector, Ui};
use ferrovgl_widgets::{Label, Slider};

use crate::util::screen_active;

/// `static lv_obj_t * status_label;`
#[derive(Default)]
struct StatusLabel(Option<Label>);

/// `static void event_cb(lv_event_t * e)`
fn event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(slider) = e.target_obj().and_then(|o| Slider::from_obj(ui, o)) else {
        return;
    };
    let code = e.code();
    let value = slider.value(ui);
    let Some(status_label) = ui.widget_static_mut::<StatusLabel>().0 else {
        return;
    };

    if code == EventCode::VALUE_CHANGED {
        // LV_LOG_USER("slider value: %d", ...): not printed, the crate has no logger.
        status_label.set_text_fmt(ui, format_args!("Dragging: {value}"));
    } else if code == EventCode::RELEASED {
        // LV_LOG_USER("slider released at %d", ...): not printed, the crate has no logger.
        status_label.set_text_fmt(ui, format_args!("Released at: {value}"));
    }
}

/// `lv_example_slider_event()`: report the value while dragging and on release.
pub fn lv_example_slider_event(ui: &mut Ui) {
    let scr = screen_active(ui);
    ui.obj_set_flex_flow(scr, FlexFlow::Column);
    ui.obj_set_style_flex_cross_place(scr, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_pad_row(scr, 12, StyleSelector::DEFAULT);

    let slider = Slider::create(ui, scr);
    ui.obj_set_width(slider.obj(), 200);
    ui.obj_add_event_cb(slider.obj(), Rc::new(event_cb), EventCode::VALUE_CHANGED, None);
    ui.obj_add_event_cb(slider.obj(), Rc::new(event_cb), EventCode::RELEASED, None);

    let status_label = Label::create(ui, scr);
    ui.widget_static_mut::<StatusLabel>().0 = Some(status_label);
    status_label.set_text(ui, "Drag the slider");
}
