//! Port of `examples/event/lv_example_event_streak.c`.

use alloc::rc::Rc;

use ferrovgl_core::{Event, EventCode, EventParam, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::{obj_data, screen_active, user_obj};

/// `static void short_click_event_cb(lv_event_t * e)`
fn short_click_event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    // LV_LOG_USER("Short clicked"): logging is disabled (`LV_USE_LOG 0`).
    let Some(info_label) = user_obj(e).and_then(|o| Label::from_obj(ui, o)) else {
        return;
    };
    let EventParam::Indev(indev) = *e.param() else {
        return;
    };
    let cnt: u8 = ui.indev_get_short_click_streak(indev);
    info_label.set_text_fmt(ui, format_args!("Short click streak: {cnt}"));
}

/// `static void streak_event_cb(lv_event_t * e)`
fn streak_event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let Some(btn) = e.target_obj() else { return };
    let label = ui.obj_get_child(btn, 0).and_then(|o| Label::from_obj(ui, o));
    let text = e
        .user_data()
        .and_then(|d| d.downcast_ref::<&'static str>())
        .copied();
    if let (Some(label), Some(text)) = (label, text) {
        label.set_text(ui, text);
    }
}

/// `(void *) "..."` user data.
fn text_data(text: &'static str) -> Option<Rc<dyn core::any::Any>> {
    Some(Rc::new(text))
}

/// `lv_example_event_streak()`: handle multiple clicks.
pub fn lv_example_event_streak(ui: &mut Ui) {
    let info_label = Label::create(ui, screen_active(ui));
    info_label.set_text(ui, "No events yet");

    let btn = Button::create(ui, screen_active(ui)).obj();
    ui.obj_set_size(btn, 150, 50);
    ui.obj_center(btn);
    ui.obj_add_event_cb(
        btn,
        Rc::new(short_click_event_cb),
        EventCode::SHORT_CLICKED,
        obj_data(info_label.obj()),
    );
    ui.obj_add_event_cb(
        btn,
        Rc::new(streak_event_cb),
        EventCode::SINGLE_CLICKED,
        text_data("Single clicked"),
    );
    ui.obj_add_event_cb(
        btn,
        Rc::new(streak_event_cb),
        EventCode::DOUBLE_CLICKED,
        text_data("Double clicked"),
    );
    ui.obj_add_event_cb(
        btn,
        Rc::new(streak_event_cb),
        EventCode::TRIPLE_CLICKED,
        text_data("Triple clicked"),
    );

    let label = Label::create(ui, btn);
    label.set_text(ui, "Click me!");
    ui.obj_center(label.obj());
}
