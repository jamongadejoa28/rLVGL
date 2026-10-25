//! Port of `examples/get_started/lv_example_get_started_button.c`.

use alloc::rc::Rc;

use ferrovgl_core::{Event, EventCode, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `static uint8_t cnt = 0;` of `btn_event_cb`.
#[derive(Default)]
struct Cnt(u8);

/// `static void btn_event_cb(lv_event_t * e)`
fn btn_event_cb(ui: &mut Ui, e: &mut Event<'_>) {
    let code = e.code();
    let Some(btn) = e.target_obj() else { return };
    if code == EventCode::CLICKED {
        let cnt = &mut ui.widget_static_mut::<Cnt>().0;
        *cnt = cnt.wrapping_add(1);
        let cnt = *cnt;

        /*Get the first child of the button which is the label and change its text*/
        if let Some(label) = ui.obj_get_child(btn, 0).and_then(|o| Label::from_obj(ui, o)) {
            label.set_text_fmt(ui, format_args!("Button: {cnt}"));
        }
    }
}

/// `lv_example_get_started_button()`: a button with a click counter.
pub fn lv_example_get_started_button(ui: &mut Ui) {
    let btn = Button::create(ui, screen_active(ui)).obj(); /*Add a button the current screen*/
    ui.obj_set_pos(btn, 10, 10); /*Set its position*/
    ui.obj_set_size(btn, 120, 50); /*Set its size*/
    ui.obj_add_event_cb(btn, Rc::new(btn_event_cb), EventCode::ALL, None); /*Assign a callback to the button*/

    let label = Label::create(ui, btn); /*Add a label to the button*/
    label.set_text(ui, "Button"); /*Set the labels text*/
    ui.obj_center(label.obj());
}
