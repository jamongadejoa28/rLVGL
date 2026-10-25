//! Port of `examples/get_started/get_started_slider/lv_example_get_started_slider.c`.

use ferrovgl_core::core::observer::{SubjectId, SubjectType};
use ferrovgl_core::{Align, Ui, coord};
use ferrovgl_widgets::{Label, Slider};

use crate::util::screen_active;

/// `static lv_subject_t * subject_value; static bool inited = false;`
#[derive(Default)]
struct Statics {
    subject_value: Option<SubjectId>,
    inited: bool,
}

/// `lv_example_get_started_slider()`: a label mirroring a slider's value
/// through a shared subject.
pub fn lv_example_get_started_slider(ui: &mut Ui) {
    if !ui.widget_static_mut::<Statics>().inited {
        let subject_value = ui.subject_create(SubjectType::Int);
        if let Some(subject_value) = subject_value {
            ui.subject_set_min_value_int(subject_value, 0);
            ui.subject_set_max_value_int(subject_value, 100);
            ui.subject_set_int(subject_value, 50);
        }
        let s = ui.widget_static_mut::<Statics>();
        s.subject_value = subject_value;
        s.inited = true;
    }
    let Some(subject_value) = ui.widget_static_mut::<Statics>().subject_value else {
        return;
    };

    let screen = screen_active(ui);

    /* 💡 Drag the slider; the label tracks it because both share `subject_value`. */
    let label = Label::create(ui, screen);
    ui.obj_set_align(label.obj(), Align::Center);
    ui.obj_set_y(label.obj(), -20);
    label.bind_text(ui, subject_value, Some("%d"));

    let slider = Slider::create(ui, screen);
    ui.obj_set_align(slider.obj(), Align::Center);
    ui.obj_set_y(slider.obj(), 20);
    ui.obj_set_width(slider.obj(), coord::pct(90));
    slider.bind_value(ui, subject_value);
}
