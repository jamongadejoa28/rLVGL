//! Port of `examples/widgets/label/lv_example_label_bind_text.c`.

use ferrovgl_core::core::observer::{SubjectId, SubjectType};
use ferrovgl_core::{Align, EventCode, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `static lv_subject_t * subject_text; static char subject_text_buf[256];
/// static char subject_text_prev_buf[256]; static bool inited = false;`
#[derive(Default)]
struct Statics {
    subject_text: Option<SubjectId>,
    inited: bool,
}

/// `lv_example_label_bind_text()`: a label bound to a string subject that
/// two buttons rewrite on click.
pub fn lv_example_label_bind_text(ui: &mut Ui) {
    if !ui.widget_static_mut::<Statics>().inited {
        let subject_text = ui.subject_create(SubjectType::String);
        if let Some(subject_text) = subject_text {
            /*subject_text_buf, subject_text_prev_buf, 256*/
            ui.subject_set_string_buffer_static(subject_text, 256, true);
            ui.subject_set_string(subject_text, "Hello");
        }
        let s = ui.widget_static_mut::<Statics>();
        s.subject_text = subject_text;
        s.inited = true;
    }
    let Some(subject_text) = ui.widget_static_mut::<Statics>().subject_text else {
        return;
    };

    let screen = screen_active(ui);

    /* 💡 Click either button; the label updates because it reads `subject_text` live. */
    let label_1 = Label::create(ui, screen);
    label_1.bind_text(ui, subject_text, None);
    ui.obj_set_align(label_1.obj(), Align::Center);
    ui.obj_set_y(label_1.obj(), -50);

    let button_1 = Button::create(ui, screen).obj();
    ui.obj_set_align(button_1, Align::Center);
    let label_2 = Label::create(ui, button_1);
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "Idle");

    ui.obj_add_subject_set_string_event(button_1, subject_text, EventCode::CLICKED, "Idle");

    let button_2 = Button::create(ui, screen).obj();
    ui.obj_set_align(button_2, Align::Center);
    ui.obj_set_y(button_2, 50);
    let label_3 = Label::create(ui, button_2);
    ui.obj_set_align(label_3.obj(), Align::Center);
    label_3.set_text(ui, "Running");

    ui.obj_add_subject_set_string_event(button_2, subject_text, EventCode::CLICKED, "Running");
}
