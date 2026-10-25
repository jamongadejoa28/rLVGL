//! Port of `examples/event/lv_example_event_click.c`.

use alloc::rc::Rc;

use ferrovgl_core::{Event, EventCode, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `static uint32_t cnt = 1;` of `event_cb`.
struct Cnt(u32);

impl Default for Cnt {
    fn default() -> Self {
        Cnt(1)
    }
}

/// `static void event_cb(lv_event_t * e)`
fn event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    // LV_LOG_USER("Clicked"): logging is disabled (`LV_USE_LOG 0`).
    let Some(btn) = e.target_obj() else { return };
    let label = ui.obj_get_child(btn, 0).and_then(|o| Label::from_obj(ui, o));
    let cnt = ui.widget_static_mut::<Cnt>().0;
    if let Some(label) = label {
        label.set_text_fmt(ui, format_args!("{cnt}"));
    }
    let cnt = &mut ui.widget_static_mut::<Cnt>().0;
    *cnt = cnt.wrapping_add(1);
}

/// `lv_example_event_click()`: add a click event to a button.
pub fn lv_example_event_click(ui: &mut Ui) {
    let btn = Button::create(ui, screen_active(ui)).obj();
    ui.obj_set_size(btn, 100, 50);
    ui.obj_center(btn);
    ui.obj_add_event_cb(btn, Rc::new(event_cb), EventCode::CLICKED, None);

    let label = Label::create(ui, btn);
    label.set_text(ui, "Click me!");
    ui.obj_center(label.obj());
}
