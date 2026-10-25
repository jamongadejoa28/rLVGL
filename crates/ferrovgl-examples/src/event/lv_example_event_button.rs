//! Port of `examples/event/lv_example_event_button.c`.

use alloc::rc::Rc;

use ferrovgl_core::{Event, EventCode, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::{obj_data, screen_active, user_obj};

/// `static void event_cb(lv_event_t * e)`
fn event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let code = e.code();
    let Some(label) = user_obj(e).and_then(|o| Label::from_obj(ui, o)) else {
        return;
    };
    match code {
        EventCode::PRESSED => label.set_text(ui, "The last button event:\nLV_EVENT_PRESSED"),
        EventCode::CLICKED => label.set_text(ui, "The last button event:\nLV_EVENT_CLICKED"),
        EventCode::LONG_PRESSED => label.set_text(ui, "The last button event:\nLV_EVENT_LONG_PRESSED"),
        EventCode::LONG_PRESSED_REPEAT => {
            label.set_text(ui, "The last button event:\nLV_EVENT_LONG_PRESSED_REPEAT")
        }
        _ => {}
    }
}

/// `lv_example_event_button()`: handle multiple events.
pub fn lv_example_event_button(ui: &mut Ui) {
    let btn = Button::create(ui, screen_active(ui)).obj();
    ui.obj_set_size(btn, 100, 50);
    ui.obj_center(btn);

    let btn_label = Label::create(ui, btn);
    btn_label.set_text(ui, "Click me!");
    ui.obj_center(btn_label.obj());

    let info_label = Label::create(ui, screen_active(ui));
    info_label.set_text(ui, "The last button event:\nNone");

    ui.obj_add_event_cb(btn, Rc::new(event_cb), EventCode::ALL, obj_data(info_label.obj()));
}
